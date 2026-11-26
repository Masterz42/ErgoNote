#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
//! ErgoNote for iOS and iPadOS.
//!
//! `main.c` inside `ios/ErgoNote.xcodeproj` calls [`ergonote_ios_main`], which prepares the
//! iOS sandbox directories, configures local notebooks and settings, and runs the same egui
//! application through `eframe` + Metal (`wgpu`).

use std::path::{Path, PathBuf};

/// Where ErgoNote keeps notebooks, settings, and caches inside the iOS app sandbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// Visible in the iPadOS Files app (`UIFileSharingEnabled = true`).
    pub notebooks: PathBuf,
    /// Persistent app settings inside `Library/Application Support`.
    pub settings: PathBuf,
    /// Cached OCR assets and temporary pages inside `Library/Caches`.
    pub ocr_cache: PathBuf,
}

/// Computes sandbox paths from `HOME` without touching the filesystem.
pub fn layout(home: &Path) -> Layout {
    Layout {
        notebooks: home.join("Documents").join("Notebooks"),
        settings: home.join("Library").join("Application Support").join("ErgoNote").join("settings.toml"),
        ocr_cache: home.join("Library").join("Caches").join("ErgoNote").join("ocr_cache"),
    }
}

#[cfg(target_os = "ios")]
struct Shell {
    app: ergonote_ui_egui::ErgoNoteApp,
}

#[cfg(target_os = "ios")]
impl eframe::App for Shell {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.app.show(ui);
    }
}

#[cfg(target_os = "ios")]
fn run() -> Result<(), String> {
    if let Ok(offset) = time::UtcOffset::current_local_offset() {
        ergonote_model::time::set_local_offset(offset.whole_seconds());
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| "HOME is not set in iOS sandbox".to_string())?;
    let paths = layout(&home);
    ergonote_ui_egui::ocr::set_cache_dir(paths.ocr_cache.clone());
    std::fs::create_dir_all(&paths.notebooks).map_err(|e| format!("cannot create {}: {e}", paths.notebooks.display()))?;
    if let Some(parent) = paths.settings.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }

    let settings = ergonote_store::Settings::at(&paths.settings);
    if settings.get("ui_language").is_none() {
        let _ = settings.set("ui_language", "fr");
    }
    if settings.get("privacy_welcome_seen").is_none() {
        let _ = settings.set("privacy_welcome_seen", "true");
    }

    let root = ergonote_store::settings::configured_notebooks_dir(&settings).filter(|p| p.is_dir()).unwrap_or_else(|| paths.notebooks.clone());
    let mut session = ergonote_engine::Session::new(&root, settings).map_err(|e| format!("cannot open {}: {e}", root.display()))?;
    let initial_page = if session.tree().is_empty() { ergonote_engine::sample::create_school_binder(&mut session).ok() } else { None };

    let mut app =
        ergonote_ui_egui::ErgoNoteApp::new(session).with_simple_mobile(false).with_clipboard(Box::new(ergonote_ui_egui::clipboard::Unavailable));
    app.enable_file_downloads();
    app.expand_all();
    if let Some(page) = initial_page.as_ref() {
        app.open_path(&page.to_string_lossy());
    }

    let options = eframe::NativeOptions::default();
    eframe::run_native("ErgoNote", options, Box::new(move |_cc| Ok(Box::new(Shell { app })))).map_err(|e| format!("eframe failed: {e}"))
}

/// Entry point called by `ios/App/main.c`.
#[cfg(target_os = "ios")]
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn ergonote_ios_main() {
    if let Err(err) = run() {
        log::error!("ErgoNote iOS startup failed: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_uses_ios_sandbox_directories() {
        let paths = layout(Path::new("/var/mobile/Containers/Data/Application/1234"));
        assert_eq!(paths.notebooks, PathBuf::from("/var/mobile/Containers/Data/Application/1234/Documents/Notebooks"));
        assert_eq!(paths.settings, PathBuf::from("/var/mobile/Containers/Data/Application/1234/Library/Application Support/ErgoNote/settings.toml"));
        assert_eq!(paths.ocr_cache, PathBuf::from("/var/mobile/Containers/Data/Application/1234/Library/Caches/ErgoNote/ocr_cache"));
    }
}
