//! JNI bridge between [`ErgoNoteActivity`] in Java and the Rust event loop.
//!
//! Only this module uses `unsafe` for raw JNI pointers and exported `extern "system"` callbacks.
#![allow(unsafe_code)]

use jni::errors::LogErrorAndDefault;
use jni::objects::{JClass, JObject, JString};
use jni::sys::{jint, jlong};
use jni::{EnvUnowned, JValue, JavaVM, jni_sig, jni_str};
use std::path::PathBuf;
use std::sync::Mutex;
use winit::platform::android::activity::AndroidApp;

/// Events sent asynchronously from Android's UI/binder threads into the next egui frame.
pub(crate) enum BridgeEvent {
    FilePicked(Option<PathBuf>),
    FileSaved { ok: bool, name: String },
    VoiceDictated(String),
    ReleaseJson(String),
    TtsDone,
    ImeParagraph { sync_gen: i64, text: String, sel_start_utf16: usize, sel_end_utf16: usize },
    ImeDelta { delete_before: usize, insert: String },
    ImeEnter,
    ImeBackspace,
}

static EVENTS: Mutex<Vec<BridgeEvent>> = Mutex::new(Vec::new());
static REPAINT_CTX: Mutex<Option<egui::Context>> = Mutex::new(None);

pub(crate) fn set_repaint_ctx(ctx: egui::Context) {
    if let Ok(mut guard) = REPAINT_CTX.lock() {
        *guard = Some(ctx);
    }
}

pub(crate) fn take_events() -> Vec<BridgeEvent> {
    if let Ok(mut guard) = EVENTS.lock() { std::mem::take(&mut *guard) } else { Vec::new() }
}

fn push_event(event: BridgeEvent) {
    if let Ok(mut guard) = EVENTS.lock() {
        guard.push(event);
    }
    if let Ok(guard) = REPAINT_CTX.lock()
        && let Some(ctx) = guard.as_ref()
    {
        ctx.request_repaint();
    }
}

fn with_activity<F>(app: &AndroidApp, action_name: &str, f: F)
where
    F: for<'local> FnOnce(&mut jni::Env<'local>, &JObject<'local>) -> jni::errors::Result<()>,
{
    let raw_vm = app.vm_as_ptr();
    let raw_act = app.activity_as_ptr();
    if raw_vm.is_null() || raw_act.is_null() {
        return;
    }
    let vm = unsafe { JavaVM::from_raw(raw_vm.cast()) };
    if let Err(err) = vm.attach_current_thread(|env| {
        let activity = unsafe { JObject::from_raw(env, raw_act.cast()) };
        f(env, &activity)
    }) {
        log::error!("{action_name} JNI call failed: {err}");
    }
}

pub(crate) fn open_file_picker(app: &AndroidApp, mime: &str) {
    with_activity(app, "openFilePicker", |env, activity| {
        let jmime = JString::from_str(env, mime)?;
        env.call_method(activity, jni_str!("openFilePicker"), jni_sig!("(Ljava/lang/String;)V"), &[JValue::Object(&jmime)])?;
        Ok(())
    });
}

pub(crate) fn save_file_with_picker(app: &AndroidApp, temp_path: &std::path::Path, display_name: &str, mime: &str) {
    let path_str = temp_path.to_string_lossy();
    with_activity(app, "saveFileWithPicker", |env, activity| {
        let jpath = JString::from_str(env, path_str.as_ref())?;
        let jname = JString::from_str(env, display_name)?;
        let jmime = JString::from_str(env, mime)?;
        env.call_method(
            activity,
            jni_str!("saveFileWithPicker"),
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V"),
            &[JValue::Object(&jpath), JValue::Object(&jname), JValue::Object(&jmime)],
        )?;
        Ok(())
    });
}

pub(crate) fn print_html_document(app: &AndroidApp, title: &str, html_path: &std::path::Path) {
    let path_str = html_path.to_string_lossy();
    with_activity(app, "printHtmlDocument", |env, activity| {
        let jtitle = JString::from_str(env, title)?;
        let jpath = JString::from_str(env, path_str.as_ref())?;
        env.call_method(
            activity,
            jni_str!("printHtmlDocument"),
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;)V"),
            &[JValue::Object(&jtitle), JValue::Object(&jpath)],
        )?;
        Ok(())
    });
}

pub(crate) fn share_file(app: &AndroidApp, temp_path: &std::path::Path, display_name: &str, mime: &str, subject: &str) {
    let path_str = temp_path.to_string_lossy();
    with_activity(app, "shareFile", |env, activity| {
        let jpath = JString::from_str(env, path_str.as_ref())?;
        let jname = JString::from_str(env, display_name)?;
        let jmime = JString::from_str(env, mime)?;
        let jsubject = JString::from_str(env, subject)?;
        env.call_method(
            activity,
            jni_str!("shareFile"),
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V"),
            &[JValue::Object(&jpath), JValue::Object(&jname), JValue::Object(&jmime), JValue::Object(&jsubject)],
        )?;
        Ok(())
    });
}

pub(crate) fn open_external_url(app: &AndroidApp, url: &str) {
    with_activity(app, "openExternalUrl", |env, activity| {
        let jurl = JString::from_str(env, url)?;
        env.call_method(activity, jni_str!("openExternalUrl"), jni_sig!("(Ljava/lang/String;)V"), &[JValue::Object(&jurl)])?;
        Ok(())
    });
}

pub(crate) fn check_for_updates(app: &AndroidApp) {
    with_activity(app, "checkForUpdates", |env, activity| {
        env.call_method(activity, jni_str!("checkForUpdates"), jni_sig!("()V"), &[])?;
        Ok(())
    });
}

pub(crate) fn speak_tts(app: &AndroidApp, text: &str, locale: &str, rate: f32) {
    with_activity(app, "speakTts", |env, activity| {
        let jtext = JString::from_str(env, text)?;
        let jlocale = JString::from_str(env, locale)?;
        env.call_method(
            activity,
            jni_str!("speakTts"),
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;F)V"),
            &[JValue::Object(&jtext), JValue::Object(&jlocale), JValue::Float(rate)],
        )?;
        Ok(())
    });
}

pub(crate) fn stop_tts(app: &AndroidApp) {
    with_activity(app, "stopTts", |env, activity| {
        env.call_method(activity, jni_str!("stopTts"), jni_sig!("()V"), &[])?;
        Ok(())
    });
}

pub(crate) fn run_mlkit_ocr(app: &AndroidApp, tmp_dir: &std::path::Path, image_bytes: &[u8]) -> Result<ergonote_ui_egui::ocr::OcrDocument, String> {
    let raw_vm = app.vm_as_ptr();
    let raw_act = app.activity_as_ptr();
    if raw_vm.is_null() || raw_act.is_null() {
        return Err("Activité Android indisponible pour Google ML Kit.".to_string());
    }
    let _ = std::fs::create_dir_all(tmp_dir);
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let tmp_img = tmp_dir.join(format!("mlkit_ocr_{stamp}.img"));
    std::fs::write(&tmp_img, image_bytes).map_err(|e| format!("Écriture image temporaire OCR : {e}"))?;
    let path_str = tmp_img.to_string_lossy().into_owned();

    let vm = unsafe { JavaVM::from_raw(raw_vm.cast()) };
    let call_res: Result<String, String> = vm
        .attach_current_thread(|env| -> jni::errors::Result<String> {
            let activity = unsafe { JObject::from_raw(env, raw_act.cast()) };
            let jpath = JString::from_str(env, &path_str)?;
            let ret = env
                .call_method(&activity, jni_str!("runMlKitOcr"), jni_sig!("(Ljava/lang/String;)Ljava/lang/String;"), &[JValue::Object(&jpath)])?
                .l()?;
            if ret.is_null() {
                return Ok("ERR|Réponse vide de Google ML Kit".to_string());
            }
            let jstr = env.cast_local::<JString>(ret)?;
            jstr.try_to_string(env)
        })
        .map_err(|e| format!("Appel JNI Google ML Kit échoué : {e}"));
    let _ = std::fs::remove_file(&tmp_img);

    let payload = call_res?;
    let mut iter = payload.lines();
    let header = iter.next().ok_or_else(|| "Réponse ML Kit invalide.".to_string())?;
    if let Some(err) = header.strip_prefix("ERR|") {
        return Err(err.to_string());
    }
    let dims = header.strip_prefix("OK|").ok_or_else(|| "En-tête ML Kit invalide.".to_string())?;
    let (w_str, h_str) = dims.split_once('|').ok_or_else(|| "Dimensions ML Kit invalides.".to_string())?;
    let img_w: f32 = w_str.parse().map_err(|_| "Largeur ML Kit invalide.".to_string())?;
    let img_h: f32 = h_str.parse().map_err(|_| "Hauteur ML Kit invalide.".to_string())?;

    let mut lines = Vec::new();
    for row in iter {
        let mut cols = row.splitn(5, '\t');
        let (Some(x0), Some(y0), Some(x1), Some(y1), Some(text)) = (cols.next(), cols.next(), cols.next(), cols.next(), cols.next()) else {
            continue;
        };
        if let (Ok(x0), Ok(y0), Ok(x1), Ok(y1)) = (x0.parse::<f32>(), y0.parse::<f32>(), x1.parse::<f32>(), y1.parse::<f32>()) {
            lines.push((x0, y0, x1, y1, text.to_string()));
        }
    }
    ergonote_ui_egui::ocr::document_from_lines("Google ML Kit (Français)", img_w, img_h, lines)
}

pub(crate) fn sync_ime_state(
    app: &AndroidApp,
    sync_gen: i64,
    visible: bool,
    paragraph_mode: bool,
    text: &str,
    sel_start_utf16: usize,
    sel_end_utf16: usize,
) {
    let sel_start = jint::try_from(sel_start_utf16).unwrap_or(jint::MAX);
    let sel_end = jint::try_from(sel_end_utf16).unwrap_or(jint::MAX);
    with_activity(app, "syncImeState", |env, activity| {
        let jtext = JString::from_str(env, text)?;
        env.call_method(
            activity,
            jni_str!("syncImeState"),
            jni_sig!("(JZZLjava/lang/String;II)V"),
            &[
                JValue::Long(sync_gen),
                JValue::Bool(visible),
                JValue::Bool(paragraph_mode),
                JValue::Object(&jtext),
                JValue::Int(sel_start),
                JValue::Int(sel_end),
            ],
        )?;
        Ok(())
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_ergonote_app_ErgoNoteActivity_nativeOnFilePicked<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    path: JString<'caller>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let s = path.try_to_string(env)?;
            push_event(BridgeEvent::FilePicked(Some(PathBuf::from(s))));
            Ok(())
        })
        .resolve::<LogErrorAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_ergonote_app_ErgoNoteActivity_nativeOnFilePickCancelled<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
) {
    unowned_env
        .with_env(|_env| -> jni::errors::Result<()> {
            push_event(BridgeEvent::FilePicked(None));
            Ok(())
        })
        .resolve::<LogErrorAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_ergonote_app_ErgoNoteActivity_nativeOnFileSaved<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    ok: jni::sys::jboolean,
    display_name: JString<'caller>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let name = display_name.try_to_string(env).unwrap_or_default();
            push_event(BridgeEvent::FileSaved { ok, name });
            Ok(())
        })
        .resolve::<LogErrorAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_ergonote_app_ErgoNoteActivity_nativeOnVoiceDictated<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    text: JString<'caller>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let s = text.try_to_string(env)?;
            push_event(BridgeEvent::VoiceDictated(s));
            Ok(())
        })
        .resolve::<LogErrorAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_ergonote_app_ErgoNoteActivity_nativeOnReleaseJson<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    json: JString<'caller>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let s = json.try_to_string(env)?;
            push_event(BridgeEvent::ReleaseJson(s));
            Ok(())
        })
        .resolve::<LogErrorAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_ergonote_app_ErgoNoteActivity_nativeOnImeParagraph<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    sync_gen: jlong,
    text: JString<'caller>,
    sel_start_utf16: jint,
    sel_end_utf16: jint,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let s = text.try_to_string(env)?;
            push_event(BridgeEvent::ImeParagraph {
                sync_gen,
                text: s,
                sel_start_utf16: usize::try_from(sel_start_utf16).unwrap_or(0),
                sel_end_utf16: usize::try_from(sel_end_utf16).unwrap_or(0),
            });
            Ok(())
        })
        .resolve::<LogErrorAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_ergonote_app_ErgoNoteActivity_nativeOnImeDelta<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    delete_before: jint,
    insert: JString<'caller>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let s = insert.try_to_string(env)?;
            push_event(BridgeEvent::ImeDelta { delete_before: usize::try_from(delete_before).unwrap_or(0), insert: s });
            Ok(())
        })
        .resolve::<LogErrorAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_ergonote_app_ErgoNoteActivity_nativeOnImeKey<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    key_kind: jint,
) {
    unowned_env
        .with_env(|_env| -> jni::errors::Result<()> {
            match key_kind {
                1 => push_event(BridgeEvent::ImeEnter),
                2 => push_event(BridgeEvent::ImeBackspace),
                other => log::debug!("unknown IME key kind: {other}"),
            }
            Ok(())
        })
        .resolve::<LogErrorAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_ergonote_app_ErgoNoteActivity_nativeOnTtsDone<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
) {
    unowned_env
        .with_env(|_env| -> jni::errors::Result<()> {
            push_event(BridgeEvent::TtsDone);
            Ok(())
        })
        .resolve::<LogErrorAndDefault>();
}
