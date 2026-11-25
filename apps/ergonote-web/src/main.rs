fn main() {}

#[cfg(target_arch = "wasm32")]
mod web {
    use eframe::wasm_bindgen::{self, JsCast};
    use ergonote_engine::Session;
    use ergonote_store::Settings;
    use ergonote_ui_egui::ErgoNoteApp;
    use std::path::PathBuf;

    struct WebShell {
        app: ErgoNoteApp,
    }

    impl eframe::App for WebShell {
        fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
            raw_input.events.extend(self.app.take_events());
        }

        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            self.app.show(ui);
            for file in self.app.take_file_downloads() {
                trigger_browser_download(&file.name, file.mime, &file.bytes);
            }
            if let Some(speech) = self.app.take_speech_request() {
                trigger_browser_speech(&speech);
            }
            if let Some(cmd) = self.app.take_speech_cmd() {
                trigger_browser_speech_cmd(cmd);
            }
        }
    }

    fn trigger_browser_speech_cmd(cmd: &str) {
        let Some(window) = web_sys::window() else { return };
        let Ok(func) = js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str("ergonoteSpeechCmd")) else {
            return;
        };
        let Ok(func) = func.dyn_into::<js_sys::Function>() else { return };
        let _ = func.call1(&window, &wasm_bindgen::JsValue::from_str(cmd));
    }

    fn trigger_browser_speech(text: &str) {
        let Some(window) = web_sys::window() else { return };
        let Ok(func) = js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str("ergonoteSpeak")) else {
            return;
        };
        let Ok(func) = func.dyn_into::<js_sys::Function>() else { return };
        let _ = func.call1(&window, &wasm_bindgen::JsValue::from_str(text));
    }

    fn trigger_browser_download(name: &str, mime: &str, bytes: &[u8]) {
        let Some(window) = web_sys::window() else { return };
        let Some(document) = window.document() else { return };
        let array = js_sys::Uint8Array::from(bytes);
        let parts = js_sys::Array::of1(&array.buffer());
        let opts = web_sys::BlobPropertyBag::new();
        opts.set_type(mime);
        let Ok(blob) = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts) else {
            return;
        };
        let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else {
            return;
        };
        let Ok(el) = document.create_element("a") else {
            let _ = web_sys::Url::revoke_object_url(&url);
            return;
        };
        let Ok(anchor) = el.dyn_into::<web_sys::HtmlAnchorElement>() else {
            let _ = web_sys::Url::revoke_object_url(&url);
            return;
        };
        anchor.set_href(&url);
        anchor.set_download(name);
        anchor.click();
        let _ = web_sys::Url::revoke_object_url(&url);
    }

    #[wasm_bindgen::prelude::wasm_bindgen(start)]
    pub fn start() {
        eframe::WebLogger::init(log::LevelFilter::Info).ok();
        wasm_bindgen_futures::spawn_local(async {
            let Some(window) = web_sys::window() else { return };
            let force_mobile = window.location().search().ok().is_some_and(|s| s.contains("mobile=1") || s.contains("phone=1"));
            let Some(document) = window.document() else { return };
            let Some(canvas) = document.get_element_by_id("ergonote_canvas") else { return };
            let Ok(canvas) = canvas.dyn_into::<web_sys::HtmlCanvasElement>() else { return };

            let root = PathBuf::from("/ergonote-ipad");
            let _ = ergonote_store::vfs::create_dir_all(&root);
            let settings = Settings::at("/ergonote-settings.toml");
            let _ = settings.set("ui_language", "fr");
            let _ = settings.set("theme_mode", "light");
            let _ = settings.set("privacy_welcome_seen", "true");

            let runner = eframe::WebRunner::new();
            let _ = runner
                .start(
                    canvas,
                    eframe::WebOptions::default(),
                    Box::new(move |_cc| {
                        let mut session = Session::new(&root, settings.clone()).map_err(|e| format!("Session init failed: {e}"))?;
                        if let Ok(first_page) = ergonote_engine::sample::create_school_binder(&mut session) {
                            let _ = session.settings.set("last_page", &first_page.to_string_lossy());
                        }
                        let mut app = ErgoNoteApp::new(session).with_session_only_storage().with_simple_mobile(force_mobile);
                        app.enable_file_downloads();
                        app.expand_all();
                        Ok(Box::new(WebShell { app }) as Box<dyn eframe::App>)
                    }),
                )
                .await;
        });
    }
}
