//! The native activity: Android's entry point into ErgoNote.

use crate::{insets, storage, welcome};
use ergonote_engine::Session;
use ergonote_store::Settings;
use ergonote_ui_egui::ErgoNoteApp;
use std::ffi::CString;
use std::path::PathBuf;
use std::time::Duration;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::platform::android::EventLoopBuilderExtAndroid;
use winit::platform::android::activity::AndroidApp;
use winit::window::WindowId;

/// Android's loader calls this, by name, on the activity's own thread once the APK's
/// `libergonote_android.so` is loaded. The name must not be mangled; that attribute is the
/// only unsafe code in ErgoNote (see android/Cargo.toml).
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("ErgoNote"));
    std::panic::set_hook(Box::new(|info| log::error!("ErgoNote stopped: {info}")));
    if let Err(error) = run(app) {
        log::error!("ErgoNote could not start: {error}");
    }
    // winit 0.30 enforces a single EventLoop per Linux process (process-global AtomicBool).
    // Once android_main returns, Android keeps this process alive in cache unless it exits,
    // causing every later launch of the app to fail with `EventLoop can't be recreated`.
    // Give android-activity a moment to run ANativeActivity_finish() on the UI thread, then
    // exit cleanly so the next launch starts a fresh process.
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(150));
        std::process::exit(0);
    });
}

fn run(app: AndroidApp) -> Result<(), String> {
    if let Ok(offset) = time::UtcOffset::current_local_offset() {
        ergonote_model::time::set_local_offset(offset.whole_seconds());
    }
    let layout = storage::layout(app.internal_data_path().as_deref(), app.external_data_path().as_deref())
        .ok_or("Android reported no internal storage for the app")?;
    log::info!("start: ErgoNote {} notebooks={} settings={}", env!("CARGO_PKG_VERSION"), layout.notebooks.display(), layout.settings.display());
    let mut alerts: Vec<(&str, String)> = Vec::new();

    // The bundled Welcome notebook: unpacked into app storage once per version. A failure is
    // shown in the window; the app runs without it.
    let payload = layout.payload.clone();
    match CString::new(welcome::ASSET).ok().and_then(|name| app.asset_manager().open(&name)) {
        Some(asset) => match welcome::refresh(asset, &payload, env!("CARGO_PKG_VERSION")) {
            Ok(true) => log::info!("Welcome notebook payload unpacked to {}", payload.display()),
            Ok(false) => log::info!("Welcome notebook payload is current"),
            Err(error) => alerts.push(("welcome-payload", format!("The Welcome notebook bundled with ErgoNote could not be unpacked: {error}"))),
        },
        None => {}
    }

    std::fs::create_dir_all(&layout.notebooks).map_err(|e| format!("cannot create {}: {e}", layout.notebooks.display()))?;
    let settings = Settings::at(&layout.settings);
    if settings.get("ui_language").is_none() {
        let _ = settings.set("ui_language", "fr");
    }
    if settings.get("privacy_welcome_seen").is_none() {
        let _ = settings.set("privacy_welcome_seen", "true");
    }
    let root = ergonote_store::settings::configured_notebooks_dir(&settings).filter(|p| p.is_dir()).unwrap_or_else(|| layout.notebooks.clone());
    let mut session = Session::new(&root, settings).map_err(|e| format!("cannot open the notebooks folder {}: {e}", root.display()))?;
    let initial_page = if session.tree().is_empty() { ergonote_engine::sample::create_school_binder(&mut session).ok() } else { None };

    // arboard has no Android backend, so this says the clipboard is not connected and the
    // window shows that once; copy and paste inside ErgoNote still work.
    let clipboard: Box<dyn ergonote_ui_egui::clipboard::Board> = match ergonote_ui_egui::clipboard::open() {
        Ok(board) => board,
        Err(error) => {
            log::info!("clipboard: {error}");
            Box::new(ergonote_ui_egui::clipboard::Unavailable)
        }
    };

    let window = app.clone();
    let mut builder = EventLoop::<eframe::UserEvent>::with_user_event();
    builder.with_android_app(app);
    let event_loop = builder.build().map_err(|e| format!("the window failed: {e}"))?;
    let options = eframe::NativeOptions::default();
    let inner = eframe::create_native(
        "ErgoNote",
        options,
        Box::new(move |cc| {
            crate::bridge::set_repaint_ctx(cc.egui_ctx.clone());
            let setup_payload: PathBuf = payload.clone();
            let url_window = window.clone();
            let check_window = window.clone();
            let mut app = ErgoNoteApp::new(session)
                .with_simple_mobile(false)
                .with_url_opener(move |url| crate::bridge::open_external_url(&url_window, url))
                .with_welcome_notebook_setup(move |notebooks_root, format| {
                    ergonote_setup::install_edition(&setup_payload, notebooks_root, format).map_err(|error| error.to_string())
                })
                .with_clipboard(clipboard);
            crate::bridge::check_for_updates(&check_window);
            for (key, message) in alerts.drain(..) {
                app.alert(key, "ErgoNote", &message);
            }
            app.expand_all();
            if let Some(p) = initial_page.as_ref() {
                app.open_path(&p.to_string_lossy());
            }
            log::info!("window ready");
            Ok(Box::new(Shell {
                app,
                focused: true,
                window,
                ime_gen: 0,
                last_ime_synced: (false, false, None, String::new(), 0, 0),
                pending_ime_rewrite: None,
            }))
        }),
        &event_loop,
    );
    let mut handler = AndroidHandler { inner, resumed_once: false };
    event_loop.run_app(&mut handler).map_err(|e| format!("the window failed: {e}"))
}

/// Guards eframe against Android window/suspend events delivered before the first
/// `InitWindow` (`Event::Resumed`). In eframe 0.37, receiving any `WindowEvent` before
/// `resumed()` runs makes `WgpuWinitApp::window_event` return `EventResult::Exit` and
/// close the app immediately on startup.
struct AndroidHandler<'a> {
    inner: eframe::EframeWinitApplication<'a>,
    resumed_once: bool,
}

impl ApplicationHandler<eframe::UserEvent> for AndroidHandler<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.resumed_once = true;
        self.inner.resumed(event_loop);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
        if !self.resumed_once {
            log::debug!("ignoring pre-resume window event: {event:?}");
            return;
        }
        self.inner.window_event(event_loop, window_id, event);
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        if !self.resumed_once {
            log::debug!("ignoring pre-resume suspend");
            return;
        }
        self.inner.suspended(event_loop);
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        self.inner.new_events(event_loop, cause);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: eframe::UserEvent) {
        self.inner.user_event(event_loop, event);
    }

    fn device_event(&mut self, event_loop: &ActiveEventLoop, device_id: DeviceId, event: DeviceEvent) {
        self.inner.device_event(event_loop, device_id, event);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.inner.about_to_wait(event_loop);
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        self.inner.exiting(event_loop);
    }

    fn memory_warning(&mut self, event_loop: &ActiveEventLoop) {
        self.inner.memory_warning(event_loop);
    }
}

/// The egui app inside Android's window.
struct Shell {
    app: ErgoNoteApp,
    /// Whether the activity had the input focus last frame.
    focused: bool,
    /// The activity, asked each frame where the system bars are.
    window: AndroidApp,
    /// Generation counter incremented each time Rust pushes a new IME buffer state to Java.
    ime_gen: i64,
    /// Last `(visible, paragraph_mode, block, text, sel_start_utf16, sel_end_utf16)` sent to Java.
    last_ime_synced: (bool, bool, Option<usize>, String, usize, usize),
    /// Active pending-format rewrite so follow-up keystrokes from the prior generation get wrapped.
    pending_ime_rewrite: Option<(i64, ergonote_ui_egui::ImeRewrite)>,
}

impl Shell {
    fn handle_ime_paragraph(&mut self, ctx: &egui::Context, sync_gen: i64, text: String, sel_start_utf16: usize, sel_end_utf16: usize) {
        if sync_gen == self.ime_gen {
            self.pending_ime_rewrite = None;
            if let Some((block, rewrite)) = self.app.apply_ime_paragraph(ctx, text.clone(), sel_start_utf16, sel_end_utf16) {
                if let Some(rw) = rewrite {
                    self.pending_ime_rewrite = Some((sync_gen, rw));
                } else if self.app.ime_paragraph_state().as_ref() == Some(&(block, text.clone(), sel_start_utf16, sel_end_utf16)) {
                    self.last_ime_synced = (true, true, Some(block), text, sel_start_utf16, sel_end_utf16);
                }
            }
            return;
        }
        if let Some((from_gen, rw)) = self.pending_ime_rewrite.clone()
            && sync_gen == from_gen
            && self.app.ime_paragraph_state().is_some_and(|(block, ..)| block == rw.block)
            && let Some(inserted) = text.strip_prefix(&rw.prefix).and_then(|rest| rest.strip_suffix(&rw.suffix))
            && !inserted.is_empty()
            && sel_start_utf16 == sel_end_utf16
            && sel_end_utf16 == rw.prefix.encode_utf16().count() + inserted.encode_utf16().count()
        {
            let trimmed_start = inserted.trim_start_matches(char::is_whitespace);
            let lead = &inserted[..inserted.len() - trimmed_start.len()];
            let core = trimmed_start.trim_end_matches(char::is_whitespace);
            let trail = &trimmed_start[core.len()..];
            if !core.is_empty() {
                let wrapped = format!("{}{lead}{}{}{}{}{}", rw.prefix, rw.open, core, rw.close, trail, rw.suffix);
                let caret_utf16 = if trail.is_empty() {
                    rw.prefix.encode_utf16().count() + lead.encode_utf16().count() + rw.open.encode_utf16().count() + core.encode_utf16().count()
                } else {
                    rw.prefix.encode_utf16().count()
                        + lead.encode_utf16().count()
                        + rw.open.encode_utf16().count()
                        + core.encode_utf16().count()
                        + rw.close.encode_utf16().count()
                        + trail.encode_utf16().count()
                };
                let _ = self.app.apply_ime_paragraph(ctx, wrapped, caret_utf16, caret_utf16);
            }
        }
    }
}

fn push_key(events: &mut Vec<egui::Event>, key: egui::Key) {
    for pressed in [true, false] {
        events.push(egui::Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers: egui::Modifiers::NONE });
    }
}

impl eframe::App for Shell {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        // The status bar and the navigation bar lie over the window's surface: what Android
        // leaves free between them becomes egui's content area, so nothing is drawn under them.
        let pixels_per_point = raw_input.viewport().native_pixels_per_point.unwrap_or_else(|| ctx.pixels_per_point());
        let window = raw_input.screen_rect.map(|r| r.size() * pixels_per_point);
        let content = self.window.content_rect();
        if let Some(size) = window
            && let Some(free) = insets::safe_area((content.left, content.top, content.right, content.bottom), size.x, size.y, pixels_per_point)
        {
            let margin = egui::epaint::MarginF32 { left: free.left, right: free.right, top: free.top, bottom: free.bottom };
            raw_input.safe_area_insets = Some(egui::SafeAreaInsets(margin));
        }
        for event in crate::bridge::take_events() {
            match event {
                crate::bridge::BridgeEvent::FilePicked(path) => self.app.complete_android_pick(path),
                crate::bridge::BridgeEvent::VoiceDictated(text) => self.app.finish_android_dictation(text),
                crate::bridge::BridgeEvent::ReleaseJson(json) => self.app.apply_release_json(&json, "android"),
                crate::bridge::BridgeEvent::TtsDone => self.app.mark_tts_done(),
                crate::bridge::BridgeEvent::ImeParagraph { sync_gen, text, sel_start_utf16, sel_end_utf16 } => {
                    self.handle_ime_paragraph(ctx, sync_gen, text, sel_start_utf16, sel_end_utf16);
                }
                crate::bridge::BridgeEvent::ImeDelta { delete_before, insert } => {
                    for _ in 0..delete_before {
                        push_key(&mut raw_input.events, egui::Key::Backspace);
                    }
                    if !insert.is_empty() {
                        raw_input.events.push(egui::Event::Text(insert));
                    }
                }
                crate::bridge::BridgeEvent::ImeEnter => {
                    self.pending_ime_rewrite = None;
                    if !self.app.apply_ime_enter(ctx) {
                        push_key(&mut raw_input.events, egui::Key::Enter);
                    }
                }
                crate::bridge::BridgeEvent::ImeBackspace => push_key(&mut raw_input.events, egui::Key::Backspace),
            }
        }
        // Android's Back button: it closes what is open over the page (a dialog, the
        // notebooks drawer, the File screen) before it does anything else.
        let mut back = false;
        raw_input.events.retain(|event| {
            let is_back = matches!(event, egui::Event::Key { key: egui::Key::BrowserBack, .. });
            if is_back && matches!(event, egui::Event::Key { pressed: true, .. }) {
                back = true;
            }
            !is_back
        });
        if back {
            self.app.system_back();
        }
        raw_input.events.extend(self.app.take_events());
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Android may stop the app at any time once it leaves the screen: losing the focus
        // writes the page now rather than when the autosave would.
        let focused = ctx.input(|i| i.focused);
        if self.focused && !focused {
            self.app.save_page_now();
        }
        self.focused = focused;
        self.app.show(ui);
        if let Some(mime) = self.app.take_android_pick_request() {
            crate::bridge::open_file_picker(&self.window, &mime);
        }
        if self.app.take_android_update_check_request() {
            crate::bridge::check_for_updates(&self.window);
        }
        if let Some(text) = self.app.take_speech_request() {
            let (locale, rate) = self.app.speech_locale_and_rate();
            crate::bridge::speak_tts(&self.window, &text, locale, rate);
        }
        if let Some(cmd) = self.app.take_speech_cmd()
            && matches!(cmd, "stop" | "pause")
        {
            crate::bridge::stop_tts(&self.window);
        }
        // Prevent egui-winit from calling winit's decorView soft-input toggle, which fights our
        // ImeProxyView in ErgoNoteActivity.
        let egui_wants_ime = ctx.output(|o| o.ime.is_some());
        ctx.output_mut(|o| o.ime = None);
        #[allow(deprecated)]
        let title_focused = ctx.memory(|m| m.has_focus(egui::Id::new("page-title-box")));
        let desired = if !title_focused && let Some((block, text, s, e)) = self.app.ime_paragraph_state() {
            (true, true, Some(block), text, s, e)
        } else if egui_wants_ime || (!title_focused && self.app.ime_table_cell_active()) {
            (true, false, None, String::new(), 0, 0)
        } else {
            (false, false, None, String::new(), 0, 0)
        };
        if desired != self.last_ime_synced {
            self.ime_gen = self.ime_gen.saturating_add(1);
            crate::bridge::sync_ime_state(&self.window, self.ime_gen, desired.0, desired.1, &desired.3, desired.4, desired.5);
            self.last_ime_synced = desired;
        }
    }

    fn on_exit(&mut self) {
        self.app.save_page_now();
    }
}
