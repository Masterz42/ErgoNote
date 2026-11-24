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
    VoiceDictated(String),
    ReleaseJson(String),
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
pub extern "system" fn Java_com_alientechsoftware_notecraft_ErgoNoteActivity_nativeOnFilePicked<'caller>(
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
pub extern "system" fn Java_com_alientechsoftware_notecraft_ErgoNoteActivity_nativeOnFilePickCancelled<'caller>(
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
pub extern "system" fn Java_com_alientechsoftware_notecraft_ErgoNoteActivity_nativeOnVoiceDictated<'caller>(
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
pub extern "system" fn Java_com_alientechsoftware_notecraft_ErgoNoteActivity_nativeOnReleaseJson<'caller>(
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
pub extern "system" fn Java_com_alientechsoftware_notecraft_ErgoNoteActivity_nativeOnImeParagraph<'caller>(
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
pub extern "system" fn Java_com_alientechsoftware_notecraft_ErgoNoteActivity_nativeOnImeDelta<'caller>(
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
pub extern "system" fn Java_com_alientechsoftware_notecraft_ErgoNoteActivity_nativeOnImeKey<'caller>(
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
