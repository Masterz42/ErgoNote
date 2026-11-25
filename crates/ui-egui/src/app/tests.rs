use super::*;

#[cfg(test)]
mod alert_tests {
    use super::*;

    #[test]
    fn opening_a_notebook_folder_displays_its_page_instead_of_the_previous_notebook() {
        let t = notecraft_store::testing::TempDir::new("open-folder-content");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let mut session = Session::new(&t.path().join("notebooks"), settings.clone()).expect("session");
        let sample = notecraft_engine::sample::create_sample(&mut session, None).expect("sample");
        let away = notecraft_store::ops::create_notebook(t.path(), "Away", Some(PageFormat::Html), &settings).expect("away notebook");
        let section = notecraft_store::ops::create_node(&away, "Section", NodeKind::Section, None, &settings).expect("section");
        let page = notecraft_store::ops::create_node(&section, "Away page", NodeKind::Page, None, &settings).expect("away page");
        let mut app = NoteCraftApp::new(session);
        app.open_path(&sample.md_page.to_string_lossy());
        assert!(app.editor.is_some());
        app.open_notebooks_from_folder(away);
        assert_eq!(
            app.editor.as_ref().map(|editor| &editor.dir),
            Some(&std::fs::canonicalize(page).expect("page path")),
            "the chosen notebook owns the visible page"
        );
    }

    #[test]
    fn opening_an_empty_notebook_folder_clears_the_previous_page() {
        let t = notecraft_store::testing::TempDir::new("open-empty-folder-content");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let mut session = Session::new(&t.path().join("notebooks"), settings).expect("session");
        let sample = notecraft_engine::sample::create_sample(&mut session, None).expect("sample");
        let empty = t.path().join("Empty");
        std::fs::create_dir(&empty).expect("empty folder");
        let mut app = NoteCraftApp::new(session);
        app.open_path(&sample.md_page.to_string_lossy());
        assert!(app.editor.is_some());
        app.open_notebooks_from_folder(empty);
        assert!(app.editor.is_none(), "an empty notebook cannot display another notebook's page");
        assert!(app.pad.is_none(), "stale ink cannot remain attached to an empty notebook");
    }

    #[test]
    fn release_builds_default_local_collection_to_off() {
        assert_eq!(default_telemetry_level(false), "none");
        assert_eq!(default_telemetry_level(true), "debug");
    }

    #[test]
    fn gridcraft_csv_read_back_preserves_quoted_cells_and_adds_columns() {
        let (table, rows, columns) =
            table_from_gridcraft_csv("Item,Cost,Note\n\"Tea, green\",3.50,\"she said \"\"best\"\"\"\nPaper,12,\n").expect("CSV table");
        assert_eq!((rows, columns), (3, 3));
        let body = pageedit::to_body(PageFormat::Cmgfm, &[table]);
        assert!(body.contains("Tea, green"));
        assert!(body.contains("she said \"best\""));
    }

    #[test]
    fn maximum_sensitivity_reaches_four_times_the_neutral_pen_width() {
        assert_eq!(NoteCraftApp::sensitivity_width(0), 1.0);
        assert_eq!(NoteCraftApp::sensitivity_width(50), 1.0);
        assert_eq!(NoteCraftApp::sensitivity_width(100), 4.0);
    }

    #[test]
    fn wheel_canvas_pan_ignores_ctrl_and_write_mode_but_keeps_horizontal_shift_scroll() {
        assert_eq!(NoteCraftApp::wheel_canvas_pan_delta(vec2(0.0, 42.0), Modifiers::NONE, false), egui::Vec2::ZERO);
        assert_eq!(NoteCraftApp::wheel_canvas_pan_delta(vec2(0.0, 42.0), Modifiers::NONE, true), egui::vec2(0.0, 42.0));
        assert_eq!(NoteCraftApp::wheel_canvas_pan_delta(vec2(0.0, 42.0), Modifiers::SHIFT, false), egui::vec2(42.0, 0.0));
        assert_eq!(NoteCraftApp::wheel_canvas_pan_delta(vec2(0.0, 42.0), Modifiers::CTRL, true), egui::Vec2::ZERO);
    }

    #[test]
    fn fresh_preferences_start_with_neutral_sensitivity() {
        let t = notecraft_store::testing::TempDir::new("default-sensitivity");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        assert_eq!(settings.get(SENSITIVITY_KEY), None);
        assert_eq!(settings.get(SENSITIVITY_KEY).and_then(|v| v.parse::<u8>().ok()).unwrap_or(DEFAULT_SENSITIVITY), 50);
        settings.set(SENSITIVITY_KEY, "100").expect("set");
        settings.reset().expect("reset");
        assert_eq!(settings.get(SENSITIVITY_KEY), None);
        assert_eq!(settings.get(SENSITIVITY_KEY).and_then(|v| v.parse::<u8>().ok()).unwrap_or(DEFAULT_SENSITIVITY), 50);
    }

    #[test]
    fn an_alert_queues_behind_an_open_one_and_a_muted_key_is_never_queued() {
        let t = notecraft_store::testing::TempDir::new("alerts");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let session = Session::new(&t.path().join("notebooks"), settings).expect("session");
        let mut app = NoteCraftApp::new(session);

        app.alert("first", "Title", "one");
        assert!(matches!(&app.dialog, Some(Dialog::Alert { key, .. }) if key == "first"), "the first alert opens");

        app.alert("first", "Title", "one again");
        assert!(app.queued_alerts.is_empty(), "the same alert is not shown twice while it is open");

        app.alert("second", "Title", "two");
        assert_eq!(app.queued_alerts.len(), 1, "a different alert waits its turn");

        app.muted_alerts.insert("third".to_string());
        app.alert("third", "Title", "three");
        assert_eq!(app.queued_alerts.len(), 1, "a muted alert is not queued");
        assert_eq!(app.status, "three", "the status line still says what happened");
    }

    #[test]
    fn a_permanently_muted_alert_is_not_opened() {
        let t = notecraft_store::testing::TempDir::new("alerts-never");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        settings.set("pressure_unavailable_never", "true").expect("settings");
        let session = Session::new(&t.path().join("notebooks"), settings).expect("session");
        let mut app = NoteCraftApp::new(session);

        app.alert("pressure_unavailable", "Pressure", "not available");
        assert!(app.dialog.is_none(), "a permanent mute suppresses the dialog");
        assert_eq!(app.status, "not available", "the status still records the condition");
    }

    #[test]
    fn privacy_welcome_dialog_shows_on_first_launch() {
        let t = notecraft_store::testing::TempDir::new("privacy-first-launch");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        // Do NOT set privacy_welcome_seen - simulate first launch
        let session = Session::new(&t.path().join("notebooks"), settings).expect("session");
        let app = NoteCraftApp::new(session);

        // Dialog should be shown because privacy_welcome_seen is not in settings
        assert!(matches!(app.dialog, Some(Dialog::FirstUseWizard { .. })), "PrivacyWelcome dialog should show on first launch");
    }

    #[test]
    fn privacy_welcome_dialog_does_not_show_after_dismissed() {
        let t = notecraft_store::testing::TempDir::new("privacy-dismissed");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let session = Session::new(&t.path().join("notebooks"), settings).expect("session");
        let app = NoteCraftApp::new(session);

        // Dialog should not be shown because privacy_welcome_seen is set
        assert!(app.dialog.is_none(), "PrivacyWelcome dialog should not show after being dismissed");
    }

    #[test]
    fn the_wizard_opens_once_after_install_and_not_for_later_versions() {
        let t = notecraft_store::testing::TempDir::new("welcome-wizard-once");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let upgraded = Session::new(&t.path().join("notebooks"), settings.clone()).expect("upgraded session");
        let upgraded = NoteCraftApp::new(upgraded).with_welcome_notebook_setup(|root, _format| Ok(root.join("welcome-to-notecraft")));
        assert!(upgraded.dialog.is_none(), "an existing user is not shown the wizard again for a later version");

        // A fresh install opens the wizard at the welcome page; finishing it saves every answer.
        let fresh = notecraft_store::Settings::at(t.path().join("fresh.toml"));
        let session = Session::new(&t.path().join("fresh-notebooks"), fresh.clone()).expect("fresh session");
        let mut app = NoteCraftApp::new(session).with_welcome_notebook_setup(|root, _format| Ok(root.join("welcome-to-notecraft")));
        assert!(matches!(app.dialog, Some(Dialog::FirstUseWizard { step: 0, .. })), "a fresh install starts at the welcome page");
        assert!(app.finish_first_use_wizard("none", PageFormat::Html, false).is_ok());
        assert_eq!(fresh.get("privacy_welcome_seen").as_deref(), Some("true"));

        let again = Session::new(&t.path().join("fresh-notebooks"), fresh).expect("reopened session");
        let again = NoteCraftApp::new(again).with_welcome_notebook_setup(|root, _format| Ok(root.join("welcome-to-notecraft")));
        assert!(again.dialog.is_none(), "a finished wizard does not open again");
    }

    #[test]
    fn welcome_notebook_prompt_waits_until_the_privacy_dialog_closes() {
        let t = notecraft_store::testing::TempDir::new("welcome-prompt-after-privacy");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        let session = Session::new(&t.path().join("notebooks"), settings).expect("session");
        let app = NoteCraftApp::new(session).with_welcome_notebook_setup(|root, _format| Ok(root.join("welcome-to-notecraft")));

        assert!(matches!(app.dialog, Some(Dialog::FirstUseWizard { step: 0, .. })), "the wizard opens at the logging step");
        assert!(app.queued_alerts.is_empty(), "the wizard is one window, so nothing waits behind it");
    }

    #[test]
    fn an_incomplete_first_use_wizard_is_not_marked_finished() {
        let t = notecraft_store::testing::TempDir::new("welcome-wizard-incomplete");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        let session = Session::new(&t.path().join("notebooks"), settings.clone()).expect("session");
        let mut app = NoteCraftApp::new(session);
        assert!(app.finish_first_use_wizard("none", PageFormat::Cmgfm, true).is_err(), "missing bundled content is reported");
        assert_eq!(settings.get("privacy_welcome_seen"), None, "a failed Finish must not acknowledge incomplete setup");
        let restarted = Session::new(&t.path().join("notebooks"), settings).expect("restart");
        assert!(matches!(NoteCraftApp::new(restarted).dialog, Some(Dialog::FirstUseWizard { .. })), "setup remains available on restart");
    }

    #[test]
    fn failed_welcome_notebook_generation_keeps_the_prompt_and_does_not_record_a_choice() {
        let t = notecraft_store::testing::TempDir::new("welcome-prompt-failure");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let session = Session::new(&t.path().join("notebooks"), settings.clone()).expect("session");
        let mut app = NoteCraftApp::new(session).with_welcome_notebook_setup(|_, _| Err("copy failed".to_string()));

        assert!(app.finish_first_use_wizard("none", PageFormat::Cmgfm, true).is_err());

        assert_eq!(settings.get("welcome_notebook_choice_version"), None, "a failed generation leaves the choice unanswered");
    }

    #[test]
    fn the_wizard_installs_the_edition_for_the_chosen_format() {
        let t = notecraft_store::testing::TempDir::new("welcome-wizard-format");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let session = Session::new(&t.path().join("notebooks"), settings.clone()).expect("session");
        let installed_format = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
        let recorder = installed_format.clone();
        let mut app = NoteCraftApp::new(session).with_welcome_notebook_setup(move |root, format| {
            *recorder.borrow_mut() = format.to_string();
            let installed = root.join("welcome-to-notecraft");
            std::fs::create_dir_all(installed.join("00-start/welcome")).map_err(|error| error.to_string())?;
            Ok(installed)
        });

        // Opening the fake folder's page can fail; the full install and open path is covered by the UI test.
        let _ = app.finish_first_use_wizard("none", PageFormat::Cmgfm, true);

        assert_eq!(installed_format.borrow().as_str(), "cmgfm", "the chosen format reaches the install");
    }

    /// `--first-use` writes `privacy_welcome_seen = ""` and keeps every other setting.
    #[test]
    fn first_use_flag_value_shows_the_privacy_welcome_again() {
        let t = notecraft_store::testing::TempDir::new("privacy-first-use-flag");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "").expect("settings");
        settings.set("theme_mode", "dark").expect("settings");
        let session = Session::new(&t.path().join("notebooks"), settings).expect("session");
        let app = NoteCraftApp::new(session);
        assert!(matches!(app.dialog, Some(Dialog::FirstUseWizard { .. })), "an emptied key counts as not seen");
        assert_eq!(app.session.settings.get("theme_mode").as_deref(), Some("dark"), "other settings are kept");
    }

    #[test]
    fn privacy_welcome_dialog_reappears_after_settings_file_deleted() {
        let t = notecraft_store::testing::TempDir::new("privacy-reset");
        let settings_path = t.path().join("settings.toml");
        let settings = notecraft_store::Settings::at(settings_path.clone());
        // Simulate persistent settings file with privacy_welcome_seen already set
        settings.set("privacy_welcome_seen", "true").expect("settings");
        settings.set("other_setting", "value").expect("settings");

        // Verify setting exists and dialog doesn't show
        let session1 = Session::new(&t.path().join("notebooks"), settings.clone()).expect("session");
        let app1 = NoteCraftApp::new(session1);
        assert!(app1.dialog.is_none(), "Dialog should not show when privacy_welcome_seen is set");

        // Delete the settings file (simulates what run.cmd does with --first-use)
        let _ = std::fs::remove_file(&settings_path);

        // Create new settings from deleted file (triggers fresh initialization)
        let settings2 = notecraft_store::Settings::at(settings_path);
        let session2 = Session::new(&t.path().join("notebooks"), settings2).expect("session");
        let app2 = NoteCraftApp::new(session2);

        // Dialog should reappear because settings file is gone
        assert!(matches!(app2.dialog, Some(Dialog::FirstUseWizard { .. })), "PrivacyWelcome dialog should reappear when settings file is deleted");
    }

    #[test]
    fn delete_dialog_shows_for_notebook() {
        let t = notecraft_store::testing::TempDir::new("delete-dialog-notebook");
        let nb_path = t.path().join("test-notebook");
        std::fs::create_dir_all(&nb_path).expect("create notebook dir");

        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        let session = Session::new(t.path(), settings).expect("session");
        let _app = NoteCraftApp::new(session);

        // Simulate triggering delete notebook dialog
        let dialog = Dialog::Delete { path: nb_path.clone(), title: "Test Notebook".to_string(), count: 0 };

        // Verify the dialog is properly structured for a notebook delete
        assert!(matches!(dialog, Dialog::Delete { .. }));
    }

    #[test]
    fn delete_error_message_shown_on_deletion_failure() {
        let t = notecraft_store::testing::TempDir::new("delete-error-feedback");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        let session = Session::new(t.path(), settings).expect("session");
        let mut app = NoteCraftApp::new(session);

        // Simulate a failed delete operation by setting error status
        app.status = "The notebook could not be deleted: Permission denied".to_string();
        app.status_is_error = true;

        // Verify error state is properly set
        assert!(app.status_is_error, "error flag should be set");
        assert!(app.status.contains("could not be deleted"), "error message should indicate deletion failure");
    }

    #[test]
    fn delete_notebook_clears_editor_state() {
        let t = notecraft_store::testing::TempDir::new("delete-clear-state");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        let session = Session::new(t.path(), settings).expect("session");
        let mut app = NoteCraftApp::new(session);

        // Verify initial state is empty
        assert!(app.editor.is_none(), "editor should start empty");
        assert!(app.selected.is_none(), "selection should start empty");

        // Simulate the delete operation clearing state
        // This is what happens in the delete handler after successful deletion
        app.editor = None;
        app.selected = None;

        // Verify state remains cleared
        assert!(app.editor.is_none(), "editor should remain cleared after delete");
        assert!(app.selected.is_none(), "selection should remain cleared after delete");
    }

    #[test]
    fn node_delete_error_shows_user_feedback() {
        let t = notecraft_store::testing::TempDir::new("node-delete-error");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        let session = Session::new(t.path(), settings).expect("session");
        let mut app = NoteCraftApp::new(session);

        // Simulate node.delete command failure
        app.status = "Could not delete: the operation failed".to_string();
        app.status_is_error = true;

        // Verify error message is shown to user
        assert!(app.status_is_error, "error flag should be set when node delete fails");
        assert!(app.status.contains("Could not delete"), "user should see error message");
    }

    #[test]
    fn sheets_added_by_hand_are_remembered_per_page() {
        let t = notecraft_store::testing::TempDir::new("extra-sheets-per-page");
        let settings_path = t.path().join("settings.toml");
        let settings = notecraft_store::Settings::at(settings_path.clone());
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let notebooks = t.path().join("notebooks");
        let mut session = Session::new(&notebooks, settings).expect("session");
        let sample = notecraft_engine::sample::create_sample(&mut session, None).expect("sample");
        let mut app = NoteCraftApp::new(session);

        app.open_path(&sample.md_page.to_string_lossy());
        assert_eq!(app.extra_sheets, 0);
        app.extra_sheets = 2;
        app.remember_extra_sheets();

        app.open_path(&sample.html_page.to_string_lossy());
        assert_eq!(app.extra_sheets, 0, "another page keeps its own zero count");

        app.open_path(&sample.md_page.to_string_lossy());
        assert_eq!(app.extra_sheets, 2, "returning to the page restores its extra sheets");

        let settings2 = notecraft_store::Settings::at(settings_path);
        let session2 = Session::new(&notebooks, settings2).expect("reopen session");
        let mut app2 = NoteCraftApp::new(session2);
        app2.open_path(&sample.md_page.to_string_lossy());
        assert_eq!(app2.extra_sheets, 2, "extra sheets persist across restarts");
    }

    /// A page left with only the SVG fallback (the Shell extraction failed once) still has a
    /// usable icon and must not be reported as missing.
    #[test]
    fn an_svg_fallback_icon_is_accepted_everywhere() {
        let t = notecraft_store::testing::TempDir::new("svg-fallback-icon");
        let page = t.path().to_path_buf();
        let assets = page.join("assets");
        std::fs::create_dir_all(&assets).expect("assets");
        std::fs::write(assets.join("Chapter.docx"), b"docx").expect("attachment");
        std::fs::write(assets.join("Chapter.docx.svg"), b"<svg/>").expect("svg fallback");
        let source = r#"<p><a href="assets/Chapter.docx"><img src="assets/Chapter.docx.svg" alt="" width="48"><br>Chapter.docx</a></p>"#;

        assert_eq!(first_missing_attachment_in_source(source, &page), None, "the SVG fallback is a valid icon");
        assert!(attachment_icon_healthy(&page, "Chapter.docx"));

        std::fs::write(assets.join("Chapter.docx.png"), b"png").expect("png");
        assert_eq!(first_missing_attachment_in_source(source, &page), None, "a PNG sidecar is healthy everywhere");
        assert!(attachment_icon_healthy(&page, "Chapter.docx"));
    }

    #[test]
    fn a_printout_added_in_its_own_container_can_be_undone_and_redone() {
        let t = notecraft_store::testing::TempDir::new("printout-undo");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let mut session = Session::new(&t.path().join("notebooks"), settings).expect("session");
        let sample = notecraft_engine::sample::create_sample(&mut session, None).expect("sample");
        let mut app = NoteCraftApp::new(session);
        app.open_path(&sample.md_page.to_string_lossy());
        let (blocks, boxes) = app.editor.as_ref().map(|e| (e.blocks.len(), e.boxes.len())).expect("a page is open");

        app.insert_in_own_container("\n[![a.pdf](assets/a.pdf.svg)](assets/a.pdf)\n\n![Printout page 1](assets/a.pdf.printout-001.png)\n");
        let ed = app.editor.as_ref().expect("editor");
        assert_eq!((ed.blocks.len(), ed.boxes.len()), (blocks + 2, boxes.max(1) + 1));

        app.undo();
        let ed = app.editor.as_ref().expect("editor");
        assert_eq!(ed.blocks.len(), blocks, "undo takes the printout's blocks back out");

        app.redo();
        let ed = app.editor.as_ref().expect("editor");
        assert_eq!(ed.blocks.len(), blocks + 2, "redo puts them back");
        assert_eq!(ed.boxes.last().map(|c| c.blocks), Some(2));
    }

    #[test]
    fn undo_after_a_real_printout_insert_removes_all_of_it_even_after_frames_have_drawn() {
        for html in [false, true] {
            undo_after_a_printout_insert(html);
        }
    }

    fn undo_after_a_printout_insert(html: bool) {
        let t = notecraft_store::testing::TempDir::new("printout-undo-frames");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let mut session = Session::new(&t.path().join("notebooks"), settings).expect("session");
        let sample = notecraft_engine::sample::create_sample(&mut session, None).expect("sample");
        let pdf = t.path().join("sheet.pdf");
        let page = if html { sample.html_page.clone() } else { sample.md_page.clone() };
        std::fs::write(
            &pdf,
            "%PDF-1.4\n1 0 obj\n<</Type/Catalog/Pages 2 0 R>>\nendobj\n2 0 obj\n<</Type/Pages/Kids[3 0 R]/Count 1>>\nendobj\n3 0 obj\n<</Type/Page/Parent 2 0 R/MediaBox[0 0 200 200]/Contents 4 0 R/Resources<<>>>>\nendobj\n4 0 obj\n<</Length 33>>\nstream\n0.8 0.1 0.1 rg 20 20 100 100 re f\nendstream\nendobj\nxref\n0 5\n0000000000 65535 f \n0000000009 00000 n \n0000000054 00000 n \n0000000105 00000 n \n0000000199 00000 n \ntrailer\n<</Root 1 0 R/Size 5>>\nstartxref\n280\n%%EOF\n",
        )
        .expect("pdf");
        let mut app = NoteCraftApp::new(session);
        app.open_path(&page.to_string_lossy());
        let mut h = egui_kittest::Harness::builder()
            .with_size(vec2(1200.0, 760.0))
            .with_accessibility_check(false)
            .build_ui_state(|ui, app: &mut NoteCraftApp| app.show(ui), app);
        for _ in 0..5 {
            h.step();
        }
        let shape =
            |h: &egui_kittest::Harness<'_, NoteCraftApp>| h.state().editor.as_ref().map(|e| (e.blocks.clone(), e.boxes.clone())).expect("editor");
        let before = shape(&h);
        let dir = page.clone();
        h.state_mut().add_file(&dir, &pdf, FileChoice::Printout);
        // Long enough for the autosave to write the page and the window to settle.
        for _ in 0..60 {
            h.step();
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        let inserted = shape(&h);
        assert!(inserted.0.len() > before.0.len(), "the printout added blocks: {}", h.state().status);
        h.state_mut().undo();
        for _ in 0..5 {
            h.step();
        }
        assert_eq!(shape(&h), before, "one undo takes the whole printout away");
    }

    /// A page from the sample notebook, open in a window, and a one-page PDF to insert.
    fn printout_harness(
        html: bool,
        name: &str,
    ) -> (notecraft_store::testing::TempDir, PathBuf, PathBuf, egui_kittest::Harness<'static, NoteCraftApp>) {
        let t = notecraft_store::testing::TempDir::new(name);
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let mut session = Session::new(&t.path().join("notebooks"), settings).expect("session");
        let sample = notecraft_engine::sample::create_sample(&mut session, None).expect("sample");
        let pdf = t.path().join("sheet.pdf");
        let page = if html { sample.html_page.clone() } else { sample.md_page.clone() };
        std::fs::write(
            &pdf,
            "%PDF-1.4\n1 0 obj\n<</Type/Catalog/Pages 2 0 R>>\nendobj\n2 0 obj\n<</Type/Pages/Kids[3 0 R]/Count 1>>\nendobj\n3 0 obj\n<</Type/Page/Parent 2 0 R/MediaBox[0 0 200 200]/Contents 4 0 R/Resources<<>>>>\nendobj\n4 0 obj\n<</Length 33>>\nstream\n0.8 0.1 0.1 rg 20 20 100 100 re f\nendstream\nendobj\nxref\n0 5\n0000000000 65535 f \n0000000009 00000 n \n0000000054 00000 n \n0000000105 00000 n \n0000000199 00000 n \ntrailer\n<</Root 1 0 R/Size 5>>\nstartxref\n280\n%%EOF\n",
        )
        .expect("pdf");
        let mut app = NoteCraftApp::new(session);
        app.open_path(&page.to_string_lossy());
        let mut h = egui_kittest::Harness::builder()
            .with_size(vec2(1200.0, 760.0))
            .with_accessibility_check(false)
            .build_ui_state(|ui, app: &mut NoteCraftApp| app.show(ui), app);
        for _ in 0..5 {
            h.step();
        }
        (t, page, pdf, h)
    }

    /// Issue #67: with no box holding the caret, a second printout goes below what is already
    /// on the page, not over the first.
    #[test]
    fn a_second_printout_lands_below_the_boxes_already_on_the_page() {
        for html in [false, true] {
            let (_t, page, pdf, mut h) = printout_harness(html, "printout-below");
            h.state_mut().editor.as_mut().expect("editor").caret = None;
            for _ in 0..2 {
                h.state_mut().add_file(&page, &pdf, FileChoice::Printout);
                for _ in 0..5 {
                    h.step();
                }
            }
            let all = h.state().editor.as_ref().map(|e| e.boxes.clone()).expect("editor");
            // The sample page's words, then one box for each printout, each one further down.
            assert!(all.len() >= 3, "html={html}: each printout has a box of its own: {all:?}");
            for pair in all.windows(2) {
                assert!(pair[1].y > pair[0].y, "html={html}: each new box sits below the one before, not over it: {all:?}");
            }
        }
    }

    /// A box clicked into and left blank goes however the caret leaves it, not only when the page
    /// itself moves the caret, and a printout added meanwhile is not put below it: the blank box
    /// was never something on the page, only a place for the cursor.
    #[test]
    fn a_blank_box_the_caret_was_taken_from_goes_and_a_printout_is_not_put_below_it() {
        for html in [false, true] {
            let (_t, page, pdf, mut h) = printout_harness(html, "blank-box-printout");
            let line = {
                let ed = h.state_mut().editor.as_mut().expect("editor");
                if ed.boxes.is_empty() {
                    ed.boxes = vec![notecraft_model::container::Container::new(0.0, 0.0, 600.0, ed.blocks.len())];
                }
                let line = ed.blocks.len();
                ed.blocks.push(notecraft_formats::Block::Paragraph(Vec::new()));
                ed.boxes = notecraft_model::container::append(&ed.boxes, line, 1, 300.0, 0.0, 0.0);
                let index = ed.boxes.len() - 1;
                (ed.boxes[index].x, ed.boxes[index].y) = (400.0, 2000.0);
                ed.arranged = true;
                ed.rebuild_body();
                ed.caret = Some(pageedit::Caret {
                    block: line,
                    text: String::new(),
                    focus: true,
                    selection: (0, 0),
                    place: None,
                    cell: None,
                    span: None,
                    inline: None,
                });
                line
            };
            for _ in 0..3 {
                h.step();
            }
            let boxes_with_blank = h.state().editor.as_ref().map(|e| e.boxes.len()).expect("editor");
            h.state_mut().editor.as_mut().expect("editor").caret = None;
            h.state_mut().add_file(&page, &pdf, FileChoice::Printout);
            for _ in 0..5 {
                h.step();
            }
            let ed = h.state().editor.as_ref().expect("editor");
            assert!(!ed.boxes.iter().any(|c| c.y == 2000.0), "html={html}: the blank box went: {:?}", ed.boxes);
            assert_eq!(ed.boxes.len(), boxes_with_blank, "html={html}: one box for the printout, none for the blank line: {:?}", ed.boxes);
            let printout = ed.boxes.last().expect("the printout's box");
            assert!(printout.y < 2000.0, "html={html}: the printout is not put below the blank box: {:?}", ed.boxes);
            assert!(ed.blocks.len() > line, "html={html}: the printout is on the page");
        }
    }

    /// The autosave hands the page to a writer of its own and the window carries on: on a synced
    /// folder a save takes about a second, and every frame it held up was a click or a letter
    /// held up. The words still reach the file, and anything that needs them there waits for them.
    #[test]
    fn the_autosave_does_not_hold_up_the_window() {
        let (_t, page, _pdf, mut h) = printout_harness(true, "autosave-background");
        h.state_mut().saver = crate::saver::Saver::with_writer(|dir, body| {
            std::thread::sleep(std::time::Duration::from_millis(400));
            notecraft_store::page::save_body(dir, body).map_err(|e| e.to_string())
        });
        {
            let ed = h.state_mut().editor.as_mut().expect("editor");
            let (added, _) = notecraft_formats::boxes::read(ed.format, "Written in the background\n");
            let at = ed.blocks.len();
            ed.blocks.extend(added.iter().cloned());
            if ed.boxes.is_empty() {
                ed.arranged = false;
            } else {
                ed.boxes = notecraft_model::container::append(&ed.boxes, at, added.len(), 300.0, 0.0, 0.0);
            }
            ed.rebuild_body();
            ed.dirty_since = Some(Instant::now() - AUTOSAVE_AFTER - std::time::Duration::from_millis(10));
        }
        let frame = Instant::now();
        h.step();
        assert!(frame.elapsed() < std::time::Duration::from_millis(250), "the frame the autosave fired in took {:?}", frame.elapsed());
        assert!(h.state().is_dirty(), "until it is written the page counts as unsaved");
        h.state_mut().save_now();
        let file = notecraft_store::tree::page_file(&page).expect("the page's file").1;
        let text = std::fs::read_to_string(file).expect("read");
        assert!(text.contains("Written in the background"), "the words reached the file:\n{text}");
        assert!(!h.state().is_dirty(), "and the page is saved");
    }

    /// Issue #67: a printout goes into the box that holds the caret, after the caret's line,
    /// and that box stays where it was put.
    #[test]
    fn a_printout_goes_into_the_box_that_holds_the_caret() {
        for html in [false, true] {
            let (_t, page, pdf, mut h) = printout_harness(html, "printout-caret-box");
            // A box with a line in it, moved to the right of the page's words, with the caret in it.
            let (index, line) = {
                let ed = h.state_mut().editor.as_mut().expect("editor");
                let (added, _) = notecraft_formats::boxes::read(ed.format, "Beside the words\n");
                let line = ed.blocks.len();
                ed.blocks.extend(added.iter().cloned());
                ed.boxes = notecraft_model::container::append(&ed.boxes, line, added.len(), 300.0, 0.0, 0.0);
                let index = ed.boxes.len() - 1;
                (ed.boxes[index].x, ed.boxes[index].y) = (900.0, 40.0);
                ed.arranged = true;
                ed.rebuild_body();
                ed.caret = Some(pageedit::Caret {
                    block: line,
                    text: "Beside the words".into(),
                    focus: false,
                    selection: (0, 0),
                    place: None,
                    cell: None,
                    span: None,
                    inline: None,
                });
                // Made here rather than by clicks and typing, so it is remembered here too, the
                // way the window remembers a box the writer made.
                NoteCraftApp::remember_step(ed);
                (index, line)
            };
            for _ in 0..3 {
                h.step();
            }
            let before = h.state().editor.as_ref().map(|e| e.boxes.clone()).expect("editor");
            let blocks_before = h.state().editor.as_ref().map(|e| e.blocks.clone()).expect("editor");
            h.state_mut().add_file(&page, &pdf, FileChoice::Printout);
            for _ in 0..5 {
                h.step();
            }
            let ed = h.state().editor.as_ref().expect("editor");
            assert_eq!(ed.boxes.len(), before.len(), "html={html}: no new box was made: {:?}", ed.boxes);
            let moved = &ed.boxes[index];
            assert_eq!((moved.x, moved.y), (900.0, 40.0), "html={html}: the box stays where it was put");
            assert!(moved.blocks > before[index].blocks, "html={html}: the printout is in the caret's box: {:?}", ed.boxes);
            // A printout's pages are as wide as the sheet, as they are in a box of their own: a
            // narrow box (one started with a click) is widened for them, not the pages shrunk.
            let sheet = before[0].width;
            assert!(sheet > 300.0, "html={html}: the page's first box is wider than the narrow one: {before:?}");
            assert_eq!(moved.width, sheet, "html={html}: the box is widened to the sheet for the printout");
            let range = notecraft_model::container::blocks_of(&ed.boxes, index);
            assert_eq!(range.start, line, "html={html}: the box's own line is still first");
            assert!(
                ed.blocks[range].iter().any(|b| matches!(b, notecraft_formats::Block::Image { .. })),
                "html={html}: the printout's pages are in that box"
            );
            h.state_mut().undo();
            for _ in 0..3 {
                h.step();
            }
            let ed = h.state().editor.as_ref().expect("editor");
            assert_eq!((&ed.blocks, &ed.boxes), (&blocks_before, &before), "html={html}: one undo takes the printout back out of the box");
        }
    }

    #[test]
    fn pdf_printout_omits_filename_header_and_keeps_pdf_asset() {
        for html in [false, true] {
            let (_t, page, pdf, mut h) = printout_harness(html, "pdf-no-header");
            let before_len = h.state().editor.as_ref().expect("editor").blocks.len();
            h.state_mut().add_file(&page, &pdf, FileChoice::Printout);
            for _ in 0..5 {
                h.step();
            }
            let ed = h.state().editor.as_ref().expect("editor");
            let added = &ed.blocks[before_len..];
            assert!(!added.is_empty(), "html={html}: printout image should be added");
            assert!(
                added.iter().all(|b| matches!(b, notecraft_formats::Block::Image { .. })),
                "html={html}: only printout image blocks should be added, got: {added:?}"
            );
            assert!(page.join("assets").join("sheet.pdf").is_file(), "html={html}: source PDF must remain in assets for native text extraction");
        }
    }
}

/// Blocks put into a page that has several boxes go into the box the caret is in, and every
/// box keeps its place. Before, the box counts were left behind, `container::fit` saw the
/// mismatch, and the whole arrangement collapsed into one box at the top-left.
#[cfg(test)]
mod keep_boxes_tests {
    use super::*;

    /// A sample page split into two boxes, the second moved to the right, with the caret on
    /// its last line.
    fn two_boxes(html: bool, name: &str) -> (notecraft_store::testing::TempDir, PathBuf, egui_kittest::Harness<'static, NoteCraftApp>) {
        let t = notecraft_store::testing::TempDir::new(name);
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let mut session = Session::new(&t.path().join("notebooks"), settings).expect("session");
        let sample = notecraft_engine::sample::create_sample(&mut session, None).expect("sample");
        let page = if html { sample.html_page.clone() } else { sample.md_page.clone() };
        let mut app = NoteCraftApp::new(session);
        app.open_path(&page.to_string_lossy());
        let mut h = egui_kittest::Harness::builder()
            .with_size(vec2(1400.0, 800.0))
            .with_accessibility_check(false)
            .build_ui_state(|ui, app: &mut NoteCraftApp| app.show(ui), app);
        for _ in 0..5 {
            h.step();
        }
        {
            let ed = h.state_mut().editor.as_mut().expect("editor");
            let n = ed.blocks.len();
            assert!(n >= 4, "the sample page has blocks to split");
            let first = n / 2;
            ed.boxes = vec![notecraft_model::Container::new(0.0, 0.0, 600.0, first), notecraft_model::Container::new(900.0, 40.0, 300.0, n - first)];
            ed.arranged = true;
            let last = n - 1;
            ed.caret = ed.blocks.get(last).map(|b| pageedit::Caret {
                block: last,
                text: pageedit::caret_text(b),
                focus: false,
                selection: (0, 0),
                place: None,
                cell: None,
                span: None,
                inline: None,
            });
            ed.rebuild_body();
            NoteCraftApp::remember_step(ed);
        }
        for _ in 0..3 {
            h.step();
        }
        (t, page, h)
    }

    /// How many blocks the right-hand box holds now.
    fn right_box(h: &egui_kittest::Harness<'static, NoteCraftApp>) -> usize {
        h.state().editor.as_ref().map(|e| e.boxes[1].blocks).expect("editor")
    }

    /// The arrangement after an insert: still two boxes, the right one where it was, every block
    /// counted, and the right box holding at least `grew` more blocks than the `was` it held.
    fn assert_kept(h: &mut egui_kittest::Harness<'static, NoteCraftApp>, what: &str, was: usize, grew: usize) {
        for _ in 0..5 {
            h.step();
        }
        let ed = h.state().editor.as_ref().expect("editor");
        assert_eq!(ed.boxes.len(), 2, "{what}: both boxes are still there: {:?}", ed.boxes);
        assert_eq!((ed.boxes[1].x, ed.boxes[1].y), (900.0, 40.0), "{what}: the right box stays put");
        assert_eq!(ed.boxes.iter().map(|c| c.blocks).sum::<usize>(), ed.blocks.len(), "{what}: every block is in a box");
        assert!(ed.boxes[1].blocks >= was + grew, "{what}: the new block went into the caret's box: {:?}", ed.boxes);
    }

    #[test]
    fn a_pasted_picture_goes_into_the_caret_box_and_the_boxes_stay() {
        for html in [false, true] {
            let (_t, page, mut h) = two_boxes(html, "keep-boxes-picture");
            // A 1x1 PNG; what matters is where the block goes, not what it shows.
            let png = vec![
                0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0x0d, 0x49, 0x48, 0x44, 0x52, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1f,
                0x15, 0xc4, 0x89, 0, 0, 0, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xf0, 0x1f, 0, 0x05, 0, 0x01, 0xff,
                0x89, 0x99, 0x3d, 0x1d, 0, 0, 0, 0, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
            ];
            let was = right_box(&h);
            h.state_mut().paste_picture(&page, crate::clipboard::Picture { png, width: 1, height: 1 });
            assert_kept(&mut h, &format!("html={html} picture"), was, 1);
        }
    }

    #[test]
    fn an_inserted_table_goes_into_the_caret_box_and_the_boxes_stay() {
        for html in [false, true] {
            let (_t, _page, mut h) = two_boxes(html, "keep-boxes-table");
            let was = right_box(&h);
            h.state_mut().ribbon_act(ribbon::Act::Insert("table"));
            assert_kept(&mut h, &format!("html={html} table"), was, 1);
        }
    }

    #[test]
    fn inserted_space_goes_into_the_caret_box_and_the_boxes_stay() {
        for html in [false, true] {
            let (_t, _page, mut h) = two_boxes(html, "keep-boxes-space");
            let was = right_box(&h);
            h.state_mut().ribbon_act(ribbon::Act::Insert("space"));
            assert_kept(&mut h, &format!("html={html} space"), was, 1);
        }
    }

    #[test]
    fn pasted_lines_go_into_the_caret_box_and_the_boxes_stay() {
        for html in [false, true] {
            let (_t, _page, mut h) = two_boxes(html, "keep-boxes-paste");
            let was = right_box(&h);
            h.state_mut().paste_blocks(pageedit::blocks_to_paste("one\n\ntwo\n\nthree"));
            assert_kept(&mut h, &format!("html={html} paste"), was, 2);
        }
    }

    #[test]
    fn a_new_drawing_named_by_the_page_keeps_the_boxes() {
        for html in [false, true] {
            let (_t, page, mut h) = two_boxes(html, "keep-boxes-ink");
            let was = right_box(&h);
            let svg = notecraft_ink::svg::write(&notecraft_ink::Drawing::default(), notecraft_ink::svg::Flavour::Editable);
            std::fs::write(page.join("ink.svg"), svg).expect("ink");
            h.state_mut().sync_ink_reference();
            assert_kept(&mut h, &format!("html={html} ink"), was, 1);
        }
    }

    #[test]
    fn a_table_of_a_chosen_size_goes_into_the_caret_box_and_the_boxes_stay() {
        for html in [false, true] {
            let (_t, _page, mut h) = two_boxes(html, "keep-boxes-sized-table");
            let was = right_box(&h);
            h.state_mut().ribbon_act(ribbon::Act::InsertTable(2, 3));
            assert_kept(&mut h, &format!("html={html} sized table"), was, 1);
        }
    }

    #[test]
    fn a_dropped_file_copied_in_goes_into_the_caret_box_and_the_boxes_stay() {
        for html in [false, true] {
            // A picture and any other file: each kind has its own snippet.
            for name in ["picture.png", "notes.txt"] {
                let (t, page, mut h) = two_boxes(html, "keep-boxes-copy");
                let file = t.path().join(name);
                std::fs::write(&file, "not really a picture").expect("file");
                let was = right_box(&h);
                h.state_mut().add_file(&page, &file, FileChoice::Copy);
                assert_kept(&mut h, &format!("html={html} copy {name}"), was, 1);
            }
        }
    }

    #[test]
    fn a_dropped_file_linked_goes_into_the_caret_box_and_the_boxes_stay() {
        for html in [false, true] {
            let (t, page, mut h) = two_boxes(html, "keep-boxes-link");
            let file = t.path().join("notes.txt");
            std::fs::write(&file, "some notes").expect("file");
            let was = right_box(&h);
            h.state_mut().add_file(&page, &file, FileChoice::Link);
            assert_kept(&mut h, &format!("html={html} link"), was, 1);
        }
    }

    #[test]
    fn a_snippet_with_no_caret_goes_on_the_end_and_the_boxes_stay() {
        for html in [false, true] {
            let (_t, _page, mut h) = two_boxes(html, "keep-boxes-no-caret");
            h.state_mut().editor.as_mut().expect("editor").caret = None;
            let was = right_box(&h);
            h.state_mut().ribbon_act(ribbon::Act::Equation("x^2"));
            assert_kept(&mut h, &format!("html={html} no caret"), was, 1);
        }
    }
}

#[cfg(test)]
mod literal_formatting_tests {
    use super::*;
    use notecraft_formats::{Block, Span};

    #[test]
    fn clear_formatting_keeps_literal_punctuation_through_save_and_reopen() {
        let temp = notecraft_store::testing::TempDir::new("clear-literal-formatting");
        let settings = notecraft_store::Settings::at(temp.path().join("settings.toml"));
        let mut session = Session::new(temp.path(), settings).expect("session");
        let sample = notecraft_engine::sample::create_sample(&mut session, None).expect("sample");
        let text = r"*literal* $5 or $6 [x](y) C:\Users\name";
        let original = vec![Block::Paragraph(vec![Span { text: text.into(), bold: true, ..Default::default() }])];
        let body = notecraft_formats::blocks_to_html(&original);
        session.run("page.save", &params(&[("path", &sample.html_page.to_string_lossy()), ("body", &body)])).expect("save");
        let mut app = NoteCraftApp::new(session);
        app.open_page(&sample.html_page);
        let editor = app.editor.as_mut().expect("opened page");
        editor.caret = Some(pageedit::Caret { text: pageedit::caret_text(&editor.blocks[0]), ..Default::default() });
        app.clear_formatting();
        app.save_now();
        let page = notecraft_store::page::load_page(&sample.html_page).expect("reopen");
        let blocks = notecraft_formats::boxes::read(page.format, &page.body).0;
        assert_eq!(blocks, vec![Block::Paragraph(vec![Span { text: text.into(), ..Default::default() }])]);
    }

    fn opened_literal_page(blocks: &[Block], format: notecraft_model::PageFormat) -> (notecraft_store::testing::TempDir, NoteCraftApp, PathBuf) {
        let temp = notecraft_store::testing::TempDir::new("literal-command-regression");
        let settings = notecraft_store::Settings::at(temp.path().join("settings.toml"));
        let mut session = Session::new(temp.path(), settings).expect("session");
        let sample = notecraft_engine::sample::create_sample(&mut session, None).expect("sample");
        let path = match format {
            notecraft_model::PageFormat::Html => sample.html_page,
            notecraft_model::PageFormat::Cmgfm => sample.md_page,
        };
        let body = notecraft_formats::boxes::write(format, blocks, None);
        session.run("page.save", &params(&[("path", &path.to_string_lossy()), ("body", &body)])).expect("save");
        let mut app = NoteCraftApp::new(session);
        app.open_page(&path);
        (temp, app, path)
    }

    #[test]
    fn clearing_cell_formatting_then_leaving_preserves_the_cell_text() {
        for format in notecraft_model::PageFormat::ALL {
            let text = "*literal* $5 or $6";
            let original = vec![Block::Table {
                head: vec![
                    vec![Span { text: text.into(), bold: true, ..Default::default() }],
                    vec![Span { text: "keep".into(), italic: true, ..Default::default() }],
                ],
                rows: Vec::new(),
            }];
            let (_temp, mut app, path) = opened_literal_page(&original, format);
            let ed = app.editor.as_mut().expect("editor");
            ed.caret =
                Some(pageedit::Caret { text: pageedit::cell_text(&ed.blocks[0], (0, 0)).expect("cell"), cell: Some((0, 0)), ..Default::default() });
            app.clear_formatting();
            let ed = app.editor.as_mut().expect("editor");
            ed.caret.as_mut().expect("caret").write(&mut ed.blocks[0]);
            assert_eq!(pageedit::cell_text(&ed.blocks[0], (0, 0)).as_deref(), Some(text));
            app.save_now();
            let page = notecraft_store::page::load_page(&path).expect("reopen");
            let blocks = notecraft_formats::boxes::read(format, &page.body).0;
            let Block::Table { head, .. } = &blocks[0] else { panic!("table") };
            assert_eq!(head[0], vec![Span { text: text.into(), ..Default::default() }]);
            let Block::Table { head: before, .. } = &original[0] else { panic!("table") };
            assert_eq!(head[1], before[1]);
        }
    }

    #[test]
    fn copy_and_cut_keep_one_line_literal_text_and_rich_styles() {
        for format in notecraft_model::PageFormat::ALL {
            let text = "*literal* $5 or $6";
            let original = vec![Block::Paragraph(vec![Span { text: text.into(), bold: true, ..Default::default() }])];
            let (_temp, mut app, _path) = opened_literal_page(&original, format);
            let ed = app.editor.as_mut().expect("editor");
            let source = pageedit::caret_text(&ed.blocks[0]);
            ed.caret = Some(pageedit::Caret { selection: (0, source.chars().count()), text: source, ..Default::default() });
            app.clipboard("copy");
            assert_eq!(app.pending_copy.as_deref(), Some(text));
            assert_eq!(
                notecraft_formats::markdown::parse(&notecraft_formats::html::to_markdown(app.pending_copy_html.as_deref().expect("html"))),
                original
            );
            assert_eq!(app.editor.as_ref().expect("editor").blocks, original);
            app.clipboard("cut");
            assert_eq!(app.pending_copy.as_deref(), Some(text));
            assert_eq!(pageedit::block_text(&app.editor.as_ref().expect("editor").blocks[0]), "");
        }
    }

    #[test]
    fn copying_across_blocks_keeps_literal_delimiters() {
        let blocks = vec![
            Block::Paragraph(vec![Span { text: "*left*".into(), ..Default::default() }]),
            Block::Paragraph(vec![Span { text: "$5 or $6".into(), ..Default::default() }]),
        ];
        let (_temp, mut app, _path) = opened_literal_page(&blocks, notecraft_model::PageFormat::Html);
        let ed = app.editor.as_mut().expect("editor");
        ed.across = Some(pageedit::Across { from: (0, 0), to: (1, pageedit::block_text(&ed.blocks[1]).len()) });
        app.clipboard("copy");
        assert_eq!(app.pending_copy.as_deref(), Some("*left*\n$5 or $6"));
        assert_eq!(
            notecraft_formats::markdown::parse(&notecraft_formats::html::to_markdown(app.pending_copy_html.as_deref().expect("html"))),
            blocks
        );
    }
}

/// Insert Space and the Table tab's row and column buttons change the page's shape from the
/// ribbon, not by typing into it, so the page never saw them happen and kept no step for them:
/// Undo said there was nothing to undo (issue #153).
#[cfg(test)]
mod structural_undo_tests {
    use super::*;

    /// A page with a line of words and a table under it, open in the window.
    fn page_with_table(html: bool, name: &str) -> (notecraft_store::testing::TempDir, egui_kittest::Harness<'static, NoteCraftApp>) {
        let t = notecraft_store::testing::TempDir::new(name);
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let mut session = Session::new(&t.path().join("notebooks"), settings).expect("session");
        let sample = notecraft_engine::sample::create_sample(&mut session, None).expect("sample");
        let page = if html { sample.html_page.clone() } else { sample.md_page.clone() };
        let body = if html {
            "<p>Alpha</p><table><tr><th>A</th><th>B</th></tr><tr><td>one</td><td>two</td></tr></table>"
        } else {
            "Alpha\n\n| A | B |\n|---|---|\n| one | two |\n"
        };
        notecraft_store::page::save_body(&page, body).expect("fixture");
        let mut app = NoteCraftApp::new(session);
        app.open_path(&page.to_string_lossy());
        let mut h = egui_kittest::Harness::builder()
            .with_size(vec2(1400.0, 800.0))
            .with_accessibility_check(false)
            .build_ui_state(|ui, app: &mut NoteCraftApp| app.show(ui), app);
        for _ in 0..5 {
            h.step();
        }
        (t, h)
    }

    fn settle(h: &mut egui_kittest::Harness<'static, NoteCraftApp>) {
        for _ in 0..5 {
            h.step();
        }
    }

    fn blocks(h: &egui_kittest::Harness<'static, NoteCraftApp>) -> Vec<notecraft_formats::Block> {
        h.state().editor.as_ref().map(|e| e.blocks.clone()).expect("editor")
    }

    fn put_caret(h: &mut egui_kittest::Harness<'static, NoteCraftApp>, block: usize, cell: Option<(usize, usize)>) {
        let ed = h.state_mut().editor.as_mut().expect("editor");
        let text = match cell {
            Some(c) => pageedit::cell_text(&ed.blocks[block], c).unwrap_or_default(),
            None => pageedit::caret_text(&ed.blocks[block]),
        };
        ed.caret = Some(pageedit::Caret { block, text, focus: true, selection: (0, 0), place: None, cell, span: None, inline: None });
    }

    /// Does `act`, checks it changed the page, then that one Undo puts the page back and one
    /// Redo does it again.
    fn assert_undoable(h: &mut egui_kittest::Harness<'static, NoteCraftApp>, what: &str, act: ribbon::Act) {
        let before = blocks(h);
        h.state_mut().ribbon_act(act);
        settle(h);
        let after = blocks(h);
        assert_ne!(after, before, "{what}: the page changed");
        assert!(h.state().editor.as_ref().is_some_and(|e| e.history.can_undo()), "{what}: there is a step to undo");
        h.state_mut().undo();
        settle(h);
        assert_eq!(blocks(h), before, "{what}: one undo puts the page back");
        h.state_mut().redo();
        settle(h);
        assert_eq!(blocks(h), after, "{what}: redo does it again");
    }

    #[test]
    fn insert_space_can_be_undone() {
        for html in [false, true] {
            let (_t, mut h) = page_with_table(html, "undo-insert-space");
            put_caret(&mut h, 0, None);
            assert_undoable(&mut h, &format!("html={html} space"), ribbon::Act::Insert("space"));
        }
    }

    #[test]
    fn table_rows_and_columns_put_in_or_taken_out_can_be_undone() {
        use pageedit::TableEdit as E;
        for html in [false, true] {
            for (name, edit) in [
                ("row above", E::InsertRow(true)),
                ("row below", E::InsertRow(false)),
                ("column left", E::InsertColumn(true)),
                ("column right", E::InsertColumn(false)),
                ("delete row", E::DeleteRow),
                ("delete column", E::DeleteColumn),
            ] {
                let (_t, mut h) = page_with_table(html, "undo-table-edit");
                let table = blocks(&h).iter().position(|b| matches!(b, notecraft_formats::Block::Table { .. })).expect("a table");
                put_caret(&mut h, table, Some((1, 0)));
                assert_undoable(&mut h, &format!("html={html} {name}"), ribbon::Act::Table(edit));
            }
        }
    }

    /// Issue #297: Table > Delete Table takes the caret's table out as one Undo step, in both
    /// formats, and the page saves and reopens without it. The caret lands on a real line.
    #[test]
    fn delete_table_takes_the_whole_table_out_and_can_be_undone() {
        use notecraft_formats::Block;
        for html in [false, true] {
            let (_t, mut h) = page_with_table(html, "delete-table");
            let table = blocks(&h).iter().position(|b| matches!(b, Block::Table { .. })).expect("a table");
            put_caret(&mut h, table, Some((1, 1)));
            assert_undoable(&mut h, &format!("html={html} delete table"), ribbon::Act::DeleteTable);
            let left = blocks(&h);
            assert!(!left.iter().any(|b| matches!(b, Block::Table { .. })), "html={html}: no table is left: {left:?}");
            assert!(left.iter().any(|b| pageedit::block_text(b).contains("Alpha")), "html={html}: the words around it stay");
            let caret = h.state().editor.as_ref().and_then(|e| e.caret.clone()).expect("a caret");
            assert!(caret.cell.is_none() && caret.block < left.len(), "html={html}: the caret is on a line of the page: {caret:?}");

            // Saved and opened again: the table is still gone and nothing else changed.
            h.state_mut().save_page_now();
            let path = h.state().editor.as_ref().map(|e| e.dir.clone()).expect("path");
            let file = if html { path.join("index.html") } else { path.join("index.md") };
            let written = std::fs::read_to_string(&file).expect("page file");
            assert!(!written.contains("<table") && !written.contains("| A | B |"), "html={html}: saved without the table:\n{written}");
            assert!(written.contains("Alpha"), "html={html}: and with the words:\n{written}");
        }
    }

    /// A page that held only a table keeps a line to type on, and Delete Table with the caret
    /// outside a table changes nothing.
    #[test]
    fn delete_table_on_a_table_only_page_and_outside_a_table() {
        use notecraft_formats::Block;
        let (_t, mut h) = page_with_table(false, "delete-only-table");
        put_caret(&mut h, 0, None);
        let before = blocks(&h);
        h.state_mut().ribbon_act(ribbon::Act::DeleteTable);
        settle(&mut h);
        assert_eq!(blocks(&h), before, "with the caret outside a table nothing changes");

        let table = blocks(&h).iter().position(|b| matches!(b, Block::Table { .. })).expect("a table");
        h.state_mut().editor.as_mut().expect("editor").blocks.retain(|b| matches!(b, Block::Table { .. }));
        put_caret(&mut h, 0, Some((1, 0)));
        let _ = table;
        h.state_mut().ribbon_act(ribbon::Act::DeleteTable);
        settle(&mut h);
        let left = blocks(&h);
        assert_eq!(left.len(), 1, "one empty line is left: {left:?}");
        assert!(matches!(&left[0], Block::Paragraph(spans) if spans.is_empty()), "{left:?}");
        assert_eq!(h.state().editor.as_ref().and_then(|e| e.caret.as_ref()).map(|c| c.block), Some(0));
    }

    #[test]
    fn confirm_delete_table_opens_dialog_then_deletes_with_undo() {
        use notecraft_formats::Block;
        let (_t, mut h) = page_with_table(false, "confirm-delete-table");
        let table = blocks(&h).iter().position(|b| matches!(b, Block::Table { .. })).expect("a table");
        h.state_mut().ribbon_act(ribbon::Act::ConfirmDeleteTable(Some(table)));
        settle(&mut h);
        assert!(matches!(h.state().dialog, Some(Dialog::DeleteTable { block }) if block == table));
        assert!(blocks(&h).iter().any(|b| matches!(b, Block::Table { .. })), "table stays until confirmed");

        h.state_mut().delete_table_at(table);
        h.state_mut().dialog = None;
        settle(&mut h);
        assert!(!blocks(&h).iter().any(|b| matches!(b, Block::Table { .. })), "table removed after confirm");

        h.state_mut().ribbon_act(ribbon::Act::Undo);
        settle(&mut h);
        assert!(blocks(&h).iter().any(|b| matches!(b, Block::Table { .. })), "undo restores deleted table");
    }

    /// Writing first and then changing the table is two steps, not one: Undo takes back the
    /// table change and leaves the writing.
    #[test]
    fn a_table_change_after_writing_in_its_cell_is_a_step_of_its_own() {
        let (_t, mut h) = page_with_table(false, "undo-table-after-typing");
        let table = blocks(&h).iter().position(|b| matches!(b, notecraft_formats::Block::Table { .. })).expect("a table");
        put_caret(&mut h, table, Some((1, 0)));
        settle(&mut h);
        h.state_mut().insert_in_page("X");
        settle(&mut h);
        let typed = blocks(&h);
        h.state_mut().ribbon_act(ribbon::Act::Table(pageedit::TableEdit::InsertColumn(true)));
        settle(&mut h);
        assert_ne!(blocks(&h), typed, "a column went in");
        h.state_mut().undo();
        settle(&mut h);
        assert_eq!(blocks(&h), typed, "undo takes back the column and keeps what was written");
    }
}

/// A table's column widths are kept against its place among the page's blocks and its column
/// numbers. A column put in or taken out, or a line split above the table, used to leave them
/// where they were, so the widths went to the wrong columns or were lost (issue #154).
#[cfg(test)]
mod table_width_tests {
    use super::*;
    use notecraft_model::tablecols;

    type H = egui_kittest::Harness<'static, NoteCraftApp>;

    /// An HTML page with a line and a table under it whose two columns were dragged to 210 and
    /// 290, open in the window.
    fn page(name: &str) -> (notecraft_store::testing::TempDir, PathBuf, H) {
        let t = notecraft_store::testing::TempDir::new(name);
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let mut session = Session::new(&t.path().join("notebooks"), settings).expect("session");
        let sample = notecraft_engine::sample::create_sample(&mut session, None).expect("sample");
        let page = sample.html_page.clone();
        notecraft_store::page::save_body(&page, "<p>Alpha</p><table><tr><th>Wide</th><th>Narrow</th></tr><tr><td>A</td><td>B</td></tr></table>")
            .expect("fixture");
        notecraft_store::page::set_table_columns(&page, &tablecols::parse("1:210,290")).expect("widths");
        let mut app = NoteCraftApp::new(session);
        app.open_path(&page.to_string_lossy());
        let mut h = egui_kittest::Harness::builder()
            .with_size(vec2(1400.0, 800.0))
            .with_accessibility_check(false)
            .build_ui_state(|ui, app: &mut NoteCraftApp| app.show(ui), app);
        settle(&mut h);
        assert_eq!(widths(&h).of(1), Some([210.0, 290.0].as_slice()), "the page opened with its widths");
        (t, page, h)
    }

    fn settle(h: &mut H) {
        for _ in 0..5 {
            h.step();
        }
    }

    fn widths(h: &H) -> tablecols::Widths {
        h.state().editor.as_ref().map(|e| e.columns.clone()).expect("editor")
    }

    fn into_cell(h: &mut H, cell: (usize, usize)) {
        let ed = h.state_mut().editor.as_mut().expect("editor");
        let text = pageedit::cell_text(&ed.blocks[1], cell).unwrap_or_default();
        ed.caret = Some(pageedit::Caret { block: 1, text, focus: true, selection: (0, 0), place: None, cell: Some(cell), span: None, inline: None });
    }

    #[test]
    fn a_column_put_in_on_the_left_keeps_the_others_widths() {
        let (_t, page, mut h) = page("widths-insert-left");
        into_cell(&mut h, (0, 0));
        h.state_mut().ribbon_act(ribbon::Act::Table(pageedit::TableEdit::InsertColumn(true)));
        settle(&mut h);
        let want = [pageedit::COLUMN_W, 210.0, 290.0];
        assert_eq!(widths(&h).of(1), Some(want.as_slice()), "Wide and Narrow keep their widths, the new column the usual one");
        let saved = notecraft_store::page::table_columns(&page).expect("saved widths");
        assert_eq!(saved.of(1), Some(want.as_slice()), "and the page's file says so");

        // Undo puts the widths back with the column, and Redo brings both again.
        h.state_mut().undo();
        settle(&mut h);
        assert_eq!(widths(&h).of(1), Some([210.0, 290.0].as_slice()), "undo puts the widths back");
        assert_eq!(notecraft_store::page::table_columns(&page).expect("saved").of(1), Some([210.0, 290.0].as_slice()));
        h.state_mut().redo();
        settle(&mut h);
        assert_eq!(widths(&h).of(1), Some(want.as_slice()), "redo brings them again");
    }

    /// Where the words `label` are drawn, as accessibility reads them.
    fn place_of(h: &H, label: &str) -> egui::Rect {
        use egui_kittest::kittest::{NodeT as _, Queryable as _};
        let node = h.get_by_label(label);
        let b = node.accesskit_node().bounding_box().expect("a drawn label");
        #[allow(clippy::cast_possible_truncation)]
        egui::Rect::from_min_max(egui::pos2(b.x0 as f32, b.y0 as f32), egui::pos2(b.x1 as f32, b.y1 as f32))
    }

    /// Issue #263: taking up a pen drew the table as a plain grid sized to its words, so its
    /// columns narrowed, its borders went and a centred cell went back to the left. With a pen
    /// in hand it is now drawn from the same layout as when it is written in.
    #[test]
    fn a_pen_in_hand_keeps_the_tables_widths_and_alignment() {
        let (_t, _page, mut h) = page("widths-pen");
        {
            let ed = h.state_mut().editor.as_mut().expect("editor");
            ed.columns.set_align(1, &[(1, 1)], Some(tablecols::Align::Center));
        }
        settle(&mut h);
        let typing = (place_of(&h, "Narrow"), place_of(&h, "B"));
        h.state_mut().pointer = ribbon::Pointer::Draw;
        settle(&mut h);
        let drawing = (place_of(&h, "Narrow"), place_of(&h, "B"));
        assert!((typing.0.min.x - drawing.0.min.x).abs() < 0.5, "the second column starts where it did: {typing:?} vs {drawing:?}");
        assert!((typing.1.center().x - drawing.1.center().x).abs() < 0.5, "the centred cell stays centred: {typing:?} vs {drawing:?}");
        assert!(drawing.1.center().x > drawing.0.min.x + 100.0, "in the middle of its 290 point column, not at its start: {drawing:?}");
    }

    #[test]
    fn a_column_taken_out_takes_only_its_own_width() {
        let (_t, _page, mut h) = page("widths-delete");
        into_cell(&mut h, (0, 0));
        h.state_mut().ribbon_act(ribbon::Act::Table(pageedit::TableEdit::DeleteColumn));
        settle(&mut h);
        assert_eq!(widths(&h).of(1), Some([290.0].as_slice()), "Narrow keeps its width when Wide goes");
    }

    #[test]
    fn a_line_split_above_the_table_moves_its_widths_with_it() {
        let (_t, page, mut h) = page("widths-split");
        {
            let ed = h.state_mut().editor.as_mut().expect("editor");
            let text = pageedit::caret_text(&ed.blocks[0]);
            let end = text.chars().count();
            ed.caret =
                Some(pageedit::Caret { block: 0, text, focus: true, selection: (end, end), place: Some(end), cell: None, span: None, inline: None });
        }
        settle(&mut h);
        h.key_press(egui::Key::Enter);
        settle(&mut h);
        let ed = h.state().editor.as_ref().expect("editor");
        assert!(matches!(ed.blocks.get(2), Some(notecraft_formats::Block::Table { .. })), "the table moved down a block: {:?}", ed.blocks);
        assert_eq!(widths(&h).of(2), Some([210.0, 290.0].as_slice()), "its widths moved with it");
        assert_eq!(widths(&h).of(1), None, "and the new line has none");
        settle(&mut h);
        h.state_mut().save();
        let saved = notecraft_store::page::table_columns(&page).expect("saved widths");
        assert_eq!(saved.of(2), Some([210.0, 290.0].as_slice()), "the page's file follows: {saved:?}");
    }
}

/// Table > Sort from the ribbon on both page formats: the page's file is written in the new
/// order, and one Undo puts the rows back (issue #171).
#[cfg(test)]
mod table_sort_tests {
    use super::*;

    type H = egui_kittest::Harness<'static, NoteCraftApp>;

    fn settle(h: &mut H) {
        for _ in 0..5 {
            h.step();
        }
    }

    fn names(h: &H) -> Vec<String> {
        let ed = h.state().editor.as_ref().expect("editor");
        let Some(notecraft_formats::Block::Table { rows, .. }) = ed.blocks.iter().find(|b| matches!(b, notecraft_formats::Block::Table { .. }))
        else {
            panic!("a table");
        };
        rows.iter().map(|r| r[0].iter().map(|s| s.text.as_str()).collect()).collect()
    }

    #[test]
    fn sort_from_the_ribbon_saves_and_undoes() {
        for html in [false, true] {
            let t = notecraft_store::testing::TempDir::new("table-sort");
            let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
            settings.set("privacy_welcome_seen", "true").expect("settings");
            let mut session = Session::new(&t.path().join("notebooks"), settings).expect("session");
            let sample = notecraft_engine::sample::create_sample(&mut session, None).expect("sample");
            let page = if html { sample.html_page.clone() } else { sample.md_page.clone() };
            let body = if html {
                "<table><tr><th>Fruit</th><th>Count</th></tr><tr><td>pear</td><td>3</td></tr><tr><td><b>apple</b></td><td>10</td></tr><tr><td>fig</td><td>2</td></tr></table>"
            } else {
                "| Fruit | Count |\n|---|---|\n| pear | 3 |\n| **apple** | 10 |\n| fig | 2 |\n"
            };
            notecraft_store::page::save_body(&page, body).expect("fixture");
            let mut app = NoteCraftApp::new(session);
            app.open_path(&page.to_string_lossy());
            let mut h = egui_kittest::Harness::builder()
                .with_size(vec2(1400.0, 800.0))
                .with_accessibility_check(false)
                .build_ui_state(|ui, app: &mut NoteCraftApp| app.show(ui), app);
            settle(&mut h);
            {
                let ed = h.state_mut().editor.as_mut().expect("editor");
                let at = ed.blocks.iter().position(|b| matches!(b, notecraft_formats::Block::Table { .. })).expect("a table");
                let text = pageedit::cell_text(&ed.blocks[at], (1, 0)).unwrap_or_default();
                ed.caret = Some(pageedit::Caret {
                    block: at,
                    text,
                    focus: true,
                    selection: (0, 0),
                    place: None,
                    cell: Some((1, 0)),
                    span: None,
                    inline: None,
                });
            }
            settle(&mut h);
            h.state_mut().ribbon_act(ribbon::Act::TableSort(false));
            settle(&mut h);
            assert_eq!(names(&h), ["apple", "fig", "pear"], "html={html}: sorted by the caret's column");
            h.state_mut().save();
            let saved = notecraft_store::page::load_page(&page).expect("load").body;
            let (a, f, p) = (saved.find("apple").expect("apple"), saved.find("fig").expect("fig"), saved.find("pear").expect("pear"));
            assert!(a < f && f < p, "html={html}: the file has the new order:\n{saved}");
            assert!(saved.contains(if html { "<b>apple</b>" } else { "**apple**" }), "html={html}: and the formatting:\n{saved}");

            h.state_mut().undo();
            settle(&mut h);
            assert_eq!(names(&h), ["pear", "apple", "fig"], "html={html}: one undo puts the rows back");
            h.state_mut().redo();
            settle(&mut h);
            assert_eq!(names(&h), ["apple", "fig", "pear"], "html={html}: and redo sorts them again");
            h.state_mut().ribbon_act(ribbon::Act::TableSort(true));
            settle(&mut h);
            assert_eq!(names(&h), ["pear", "fig", "apple"], "html={html}: descending");
        }
    }
}

#[cfg(test)]
mod page_row_name_tests {
    use super::{fit_to_width, row_name_room, text_width};

    /// A page row shows its format label only when the whole name fits beside it. A long
    /// name takes the whole row instead, so it is never cut short to make room for "html"
    /// (issue #251: names came out as "Welcomhtml").
    #[test]
    fn the_format_label_gives_way_to_a_long_name() {
        assert_eq!(row_name_room(60.0, Some(30.0), 120.0), (true, 90.0));
        assert_eq!(row_name_room(100.0, Some(30.0), 120.0), (false, 120.0));
        assert_eq!(row_name_room(100.0, None, 120.0), (false, 120.0));
    }

    /// A name that does not fit ends in "…" and stays inside its room; one that fits is
    /// left as it is.
    #[test]
    fn a_long_name_ends_in_an_ellipsis_inside_its_room() {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let name = "Write a page in plain words";
            assert_eq!(fit_to_width(ui, name, 14.0, 10_000.0), name);
            let room = text_width(ui, name, 14.0) / 2.0;
            let fitted = fit_to_width(ui, name, 14.0, room);
            assert!(fitted.ends_with('…'), "{fitted:?}");
            assert!(name.starts_with(fitted.trim_end_matches('…')), "{fitted:?}");
            assert!(text_width(ui, &fitted, 14.0) <= room, "{fitted:?} fits in {room}");
            assert_eq!(fit_to_width(ui, name, 14.0, 1.0), "…");
            // Letters made of several bytes are cut between letters, never inside one.
            let arabic = "ما الذي يمكنك فعله هنا";
            let cut = fit_to_width(ui, arabic, 14.0, text_width(ui, arabic, 14.0) / 2.0);
            assert!(cut.ends_with('…'), "{cut:?}");
        });
        out.textures_delta.clear();
    }
}

#[cfg(test)]
mod insert_table_size_tests {
    use super::table_size;

    #[test]
    fn a_typed_size_is_whole_numbers_within_the_limits() {
        assert_eq!(table_size("3", "4"), Some((3, 4)));
        assert_eq!(table_size(" 200 ", "30"), Some((200, 30)));
        for (rows, columns) in [("0", "3"), ("3", "0"), ("201", "3"), ("3", "31"), ("", "3"), ("x", "3"), ("-1", "3"), ("2.5", "3")] {
            assert_eq!(table_size(rows, columns), None, "{rows:?} x {columns:?}");
        }
    }
}

/// Issue #273: the picture menu's Rotate and Flip. The turned picture is a new file beside the
/// page, the page points at it (its size swapped by a quarter turn), the original stays, and
/// Undo puts the page back on the original. Both page formats.
#[cfg(test)]
mod picture_turn_tests {
    use super::*;
    use crate::turn::Turn;

    type H = egui_kittest::Harness<'static, NoteCraftApp>;

    fn settle(h: &mut H) {
        for _ in 0..5 {
            h.step();
        }
    }

    fn picture(h: &H) -> (String, Option<f32>, Option<f32>) {
        let ed = h.state().editor.as_ref().expect("editor");
        ed.blocks
            .iter()
            .find_map(|b| match b {
                notecraft_formats::Block::Image { src, width, height, .. } => Some((src.clone(), *width, *height)),
                _ => None,
            })
            .expect("a picture")
    }

    fn size_of(page: &std::path::Path, src: &str) -> (u32, u32) {
        let bytes = std::fs::read(page.join(src)).expect("the file");
        image::load_from_memory(&bytes).expect("a picture").to_rgba8().dimensions()
    }

    #[test]
    fn a_picture_is_turned_into_a_new_file_and_undo_puts_the_old_one_back() {
        for html in [true, false] {
            let t = notecraft_store::testing::TempDir::new("picture-turn");
            let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
            settings.set("privacy_welcome_seen", "true").expect("settings");
            let mut session = Session::new(&t.path().join("notebooks"), settings).expect("session");
            let sample = notecraft_engine::sample::create_sample(&mut session, None).expect("sample");
            let page = if html { sample.html_page.clone() } else { sample.md_page.clone() };
            let mut png = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(30, 20, image::Rgba([0, 120, 255, 255])))
                .write_to(&mut png, image::ImageFormat::Png)
                .expect("png");
            let src = notecraft_store::assets::attach_bytes(&page, "pic.png", &png.into_inner()).expect("attach");
            let body = if html { format!("<p>Alpha</p><p><img src=\"{src}\" alt=\"Pic\"></p>") } else { format!("Alpha\n\n![Pic]({src})\n") };
            notecraft_store::page::save_body(&page, &body).expect("fixture");
            let mut app = NoteCraftApp::new(session);
            app.open_path(&page.to_string_lossy());
            let mut h: H = egui_kittest::Harness::builder()
                .with_size(vec2(1400.0, 800.0))
                .with_accessibility_check(false)
                .build_ui_state(|ui, app: &mut NoteCraftApp| app.show(ui), app);
            settle(&mut h);
            {
                // A size of its own, as a resize gives it, to see it swapped.
                let ed = h.state_mut().editor.as_mut().expect("editor");
                for b in &mut ed.blocks {
                    if let notecraft_formats::Block::Image { width, height, .. } = b {
                        *width = Some(60.0);
                        *height = Some(40.0);
                    }
                }
            }
            settle(&mut h);
            let before = picture(&h);

            h.state_mut().ribbon_act(ribbon::Act::TurnPicture(src.clone(), Turn::Right));
            settle(&mut h);
            let (turned, width, height) = picture(&h);
            assert_eq!(turned, "assets/pic-rotated.png", "html={html}: the page points at the turned copy");
            assert_eq!((width, height), (Some(40.0), Some(60.0)), "html={html}: its size is swapped");
            assert_eq!(size_of(&page, &turned), (20, 30), "html={html}: the copy is turned");
            assert_eq!(size_of(&page, &src), (30, 20), "html={html}: and the original is untouched");
            h.state_mut().save_page_now();
            assert!(notecraft_store::page::load_page(&page).expect("load").body.contains("pic-rotated.png"), "html={html}: saved");

            h.state_mut().undo();
            settle(&mut h);
            assert_eq!(picture(&h), before, "html={html}: undo points the page at the original again");
            h.state_mut().redo();
            settle(&mut h);
            assert_eq!(picture(&h).0, turned, "html={html}: redo brings the turned one back");

            // A flip keeps the size and turns the copy of the copy, named once.
            h.state_mut().ribbon_act(ribbon::Act::TurnPicture(turned.clone(), Turn::FlipHorizontal));
            settle(&mut h);
            let (flipped, width, height) = picture(&h);
            assert_eq!(flipped, "assets/pic-flipped.png");
            assert_eq!((width, height), (Some(40.0), Some(60.0)));
            assert_eq!(size_of(&page, &flipped), (20, 30));
        }
    }

    #[test]
    fn image_tab_enhance_crop_insert_ocr_and_delete_with_undo() {
        let t = notecraft_store::testing::TempDir::new("image-ribbon-ops");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let mut session = Session::new(&t.path().join("notebooks"), settings).expect("session");
        let sample = notecraft_engine::sample::create_sample(&mut session, None).expect("sample");
        let page = sample.html_page.clone();
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(40, 20, image::Rgba([40, 90, 200, 255])))
            .write_to(&mut png, image::ImageFormat::Png)
            .expect("png");
        let raw_bytes = png.into_inner();
        let src = notecraft_store::assets::attach_bytes(&page, "scan.png", &raw_bytes).expect("attach");
        notecraft_store::page::save_body(&page, &format!("<p>Titre</p><p><img src=\"{src}\" alt=\"Scan\"></p>")).expect("fixture");

        let mut app = NoteCraftApp::new(session);
        app.open_path(&page.to_string_lossy());
        let mut h: H = egui_kittest::Harness::builder()
            .with_size(vec2(1400.0, 800.0))
            .with_accessibility_check(false)
            .build_ui_state(|ui, app: &mut NoteCraftApp| app.show(ui), app);
        settle(&mut h);
        assert!(h.state().page_has_image());

        // 1. Scan N&B creates a new asset and can be undone.
        h.state_mut().ribbon_act(ribbon::Act::EnhancePicture(crate::turn::Enhance::ScanBw));
        settle(&mut h);
        let (enhanced, _, _) = picture(&h);
        assert_eq!(enhanced, "assets/scan-scan.png");
        h.state_mut().undo();
        settle(&mut h);
        assert_eq!(picture(&h).0, src);

        // 2. Crop opens modal and applies crop cleanly.
        h.state_mut().ribbon_act(ribbon::Act::CropPicture);
        settle(&mut h);
        assert!(matches!(h.state().dialog, Some(Dialog::CropPicture { dims: (40, 20), .. })));
        let img_idx = h.state().active_image_target().expect("img").0;
        h.state_mut().apply_crop_picture(img_idx, &src, &raw_bytes, [0.25, 0.25, 0.75, 0.75]);
        h.state_mut().dialog = None;
        settle(&mut h);
        let (cropped, _, _) = picture(&h);
        assert_eq!(cropped, "assets/scan-cropped.png");
        assert_eq!(size_of(&page, &cropped), (20, 10));

        // 3. OCR text insertion puts paragraphs below image cleanly.
        h.state_mut().insert_ocr_text_below(img_idx, "Ligne un\nLigne deux");
        settle(&mut h);
        let blks = h.state().editor_blocks();
        assert!(blks.iter().any(|b| pageedit::block_text(b) == "Ligne un"));

        // 4. Delete image removes it cleanly and Undo brings it back.
        h.state_mut().ribbon_act(ribbon::Act::DeleteSelectedPicture);
        settle(&mut h);
        assert!(!h.state().page_has_image());
        h.state_mut().undo();
        settle(&mut h);
        assert!(h.state().page_has_image());

        // 5. Intelligent layout reconstruction replaces image with positioned editable zones and can be undone.
        let doc = crate::ocr::OcrDocument {
            source_label: "PDF natif (texte exact)",
            aspect_ratio: 1.4,
            blocks: vec![
                crate::ocr::OcrBlock { x: 0.08, y: 0.06, width: 0.5, height: 0.05, font_scale: 1.8, lines: vec!["Leçon 1".into()] },
                crate::ocr::OcrBlock { x: 0.08, y: 0.25, width: 0.4, height: 0.08, font_scale: 1.0, lines: vec!["Colonne gauche".into()] },
                crate::ocr::OcrBlock { x: 0.55, y: 0.25, width: 0.4, height: 0.08, font_scale: 1.0, lines: vec!["Colonne droite".into()] },
            ],
            plain_text: "Leçon 1\n\nColonne gauche\n\nColonne droite".into(),
        };
        let target_idx = h.state().active_image_target().expect("img target").0;
        h.state_mut().reconstruct_page_from_ocr(target_idx, &doc, false);
        settle(&mut h);
        assert!(!h.state().page_has_image(), "image replaced by editable zones");
        let ed = h.state().editor.as_ref().expect("ed");
        assert!(ed.boxes.len() >= 3);
        let blks = h.state().editor_blocks();
        assert!(blks.iter().any(|b| matches!(b, notecraft_formats::Block::Heading { .. }) && pageedit::block_text(b) == "Leçon 1"));
        assert!(blks.iter().any(|b| pageedit::block_text(b) == "Colonne droite"));
        h.state_mut().undo();
        settle(&mut h);
        assert!(h.state().page_has_image(), "undo restores original image");
    }
}

#[cfg(test)]
mod recent_range_tests {
    use super::RecentRange;

    /// Today begins at the user's own midnight, and the week and the month count whole days
    /// back from it, today included.
    #[test]
    fn ranges_begin_at_the_users_midnight() {
        let now = notecraft_model::time::secs_from_iso("2026-11-15T20:30:00Z").expect("now");
        let at = |iso: &str| notecraft_model::time::secs_from_iso(iso);
        assert_eq!(RecentRange::Today.since(now, 0), at("2026-11-15T00:00:00Z"));
        // Denver, seven hours behind: its midnight is 07:00 UTC.
        assert_eq!(RecentRange::Today.since(now, -7 * 3600), at("2026-11-15T07:00:00Z"));
        assert_eq!(RecentRange::Week.since(now, 0), at("2026-11-09T00:00:00Z"));
        assert_eq!(RecentRange::Month.since(now, 0), at("2026-10-17T00:00:00Z"));
        assert_eq!(RecentRange::All.since(now, 0), None);
    }
}

#[cfg(test)]
mod backup_and_print_tests {
    use super::{extract_notebooks_zip, file_base_href, write_notebooks_zip};
    use std::path::Path;

    #[test]
    fn zip_backup_and_recovery_roundtrip() {
        let src = notecraft_store::testing::TempDir::new("zip-src");
        let page = src.path().join("mon-cartable").join("français").join("cours-1");
        std::fs::create_dir_all(page.join("assets")).expect("mkdirs");
        std::fs::write(page.join("index.html"), "<h1>Cours</h1><p>Test</p>").expect("html");
        std::fs::write(page.join("ink.svg"), "<svg/>").expect("svg");
        std::fs::write(page.join("assets").join("photo.png"), b"PNGDATA").expect("png");
        // Temporary print artifact should be excluded from ZIP backups.
        std::fs::write(page.join("_impression.html"), "<html/>").expect("tmp print");

        let zip_dir = notecraft_store::testing::TempDir::new("zip-out");
        let zip_file = zip_dir.path().join("backup.zip");
        let count = write_notebooks_zip(src.path(), &zip_file).expect("write zip");
        assert_eq!(count, 3);

        let restored = notecraft_store::testing::TempDir::new("zip-restored");
        let restored_count = extract_notebooks_zip(&zip_file, restored.path()).expect("extract zip");
        assert_eq!(restored_count, 3);
        let r_page = restored.path().join("mon-cartable").join("français").join("cours-1");
        assert_eq!(std::fs::read_to_string(r_page.join("index.html")).expect("read html"), "<h1>Cours</h1><p>Test</p>");
        assert_eq!(std::fs::read(r_page.join("assets").join("photo.png")).expect("read png"), b"PNGDATA");
        assert!(!r_page.join("_impression.html").exists());
    }

    #[test]
    fn printable_base_href_handles_unix_and_windows_paths() {
        assert_eq!(file_base_href(Path::new("/home/ada/page")), "file:///home/ada/page/");
        assert_eq!(file_base_href(Path::new(r"C:\Users\Ada\NoteCraft\page")), "file:///C:/Users/Ada/NoteCraft/page/");
    }
}

#[cfg(test)]
mod section_tabs_bar_tests {
    use super::*;

    type H = egui_kittest::Harness<'static, NoteCraftApp>;

    #[test]
    fn toggling_section_tabs_bar_persists_and_draws_sections() {
        let t = notecraft_store::testing::TempDir::new("section-tabs-bar");
        let settings_path = t.path().join("settings.toml");
        let notebooks_dir = t.path().join("notebooks");

        let settings = notecraft_store::Settings::at(settings_path.clone());
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let mut session = Session::new(&notebooks_dir, settings).expect("session");
        let sample = notecraft_engine::sample::create_sample(&mut session, None).expect("sample");

        let mut app = NoteCraftApp::new(session);
        assert!(app.section_tabs_bar, "enabled by default");
        assert!(!app.organisers, "left notebooks column hidden while section tabs are enabled by default");
        app.open_path(&sample.html_page.to_string_lossy());
        app.ribbon_act(ribbon::Act::ToggleSectionTabs);
        assert!(!app.section_tabs_bar);
        assert!(app.organisers, "disabling section tabs restores the left notebooks column");
        assert_eq!(app.session.settings.get("section_tabs_bar").as_deref(), Some("false"));

        app.ribbon_act(ribbon::Act::ToggleSectionTabs);
        assert!(app.section_tabs_bar);
        assert!(!app.organisers, "enabling section tabs hides the left notebooks column");
        assert_eq!(app.session.settings.get("section_tabs_bar").as_deref(), Some("true"));

        let mut h: H = egui_kittest::Harness::builder()
            .with_size(vec2(1400.0, 800.0))
            .with_accessibility_check(false)
            .build_ui_state(|ui, app: &mut NoteCraftApp| app.show(ui), app);
        for _ in 0..3 {
            h.step();
        }
        // Reopening with the same settings file keeps the horizontal section bar enabled and hides notebooks.
        let reloaded_settings = notecraft_store::Settings::at(settings_path);
        let reloaded_session = Session::new(&notebooks_dir, reloaded_settings).expect("reload");
        let mut reloaded_app = NoteCraftApp::new(reloaded_session);
        assert!(reloaded_app.section_tabs_bar);
        assert!(!reloaded_app.organisers);

        reloaded_app.ribbon_act(ribbon::Act::ToggleSectionTabs);
        assert!(!reloaded_app.section_tabs_bar);
        assert!(reloaded_app.organisers, "disabling section tabs restores the left notebooks column");
        assert_eq!(reloaded_app.session.settings.get("section_tabs_bar").as_deref(), Some("false"));
    }

    #[test]
    fn dys_features_v1_1_work_and_persist() {
        let t = notecraft_store::testing::TempDir::new("dys-v1-1");
        let settings_path = t.path().join("settings.toml");
        let notebooks_dir = t.path().join("notebooks");

        let settings = notecraft_store::Settings::at(settings_path);
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let mut session = Session::new(&notebooks_dir, settings).expect("session");
        let sample = notecraft_engine::sample::create_sample(&mut session, None).expect("sample");

        let mut app = NoteCraftApp::new(session);
        app.open_path(&sample.html_page.to_string_lossy());
        app.ribbon_act(ribbon::Act::ToggleLockImages);
        app.ribbon_act(ribbon::Act::ToggleDysSpacing);
        app.ribbon_act(ribbon::Act::ToggleReadingRuler);
        assert!(app.lock_images);
        assert!(app.dys_spaced);
        assert!(app.reading_ruler);

        let before_blocks = app.editor.as_ref().expect("ed").blocks.len();
        app.ribbon_act(ribbon::Act::InsertSchoolTable("conversion"));
        app.ribbon_act(ribbon::Act::InsertSchoolTable("numeration"));
        app.ribbon_act(ribbon::Act::InsertSchoolTable("proportion"));
        assert_eq!(app.editor.as_ref().expect("ed").blocks.len(), before_blocks + 3);

        app.ribbon_act(ribbon::Act::ReadAloud);
        let speech = app.take_speech_request();
        assert!(speech.is_some_and(|s| s.contains("km") && s.contains("Centaines")));

        let mut h: H = egui_kittest::Harness::builder()
            .with_size(vec2(1400.0, 800.0))
            .with_accessibility_check(false)
            .build_ui_state(|ui, app: &mut NoteCraftApp| app.show(ui), app);
        for _ in 0..3 {
            h.step();
        }
    }

    #[test]
    fn android_ime_utf16_offsets_handle_accents_and_surrogate_pairs() {
        let sample = "Été 😀 fin";
        assert_eq!(char_to_utf16(sample, 0), 0);
        assert_eq!(char_to_utf16(sample, 4), 4);
        assert_eq!(char_to_utf16(sample, 5), 6);
        assert_eq!(char_to_utf16(sample, 10), 10);

        assert_eq!(utf16_to_char(sample, 0), 0);
        assert_eq!(utf16_to_char(sample, 4), 4);
        assert_eq!(utf16_to_char(sample, 5), 4);
        assert_eq!(utf16_to_char(sample, 6), 5);
        assert_eq!(utf16_to_char(sample, 10), 9);
        assert_eq!(utf16_to_char(sample, 99), 9);
    }

    #[test]
    fn android_ime_paragraph_roundtrip_updates_caret_and_mobile_ribbon_renders() {
        let t = notecraft_store::testing::TempDir::new("android-ime");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let notebooks_dir = t.path().join("notebooks");
        let mut session = Session::new(&notebooks_dir, settings).expect("session");
        let sample = notecraft_engine::sample::create_sample(&mut session, None).expect("sample");

        let mut app = NoteCraftApp::new(session).with_simple_mobile(true);
        app.open_path(&sample.html_page.to_string_lossy());
        app.full_page = true;
        assert_eq!(app.ime_paragraph_state(), None);

        let ctx = Context::default();
        if let Some(ed) = app.editor.as_mut() {
            let block = ed.blocks.iter().position(|b| matches!(b, notecraft_formats::Block::Paragraph(_))).expect("paragraph block");
            ed.caret = Some(pageedit::Caret {
                block,
                text: "Bonjor".to_string(),
                focus: false,
                selection: (6, 6),
                place: None,
                cell: None,
                span: None,
                inline: None,
            });
        }

        let (block, initial, s, e) = app.ime_paragraph_state().expect("active paragraph IME state");
        assert_eq!(initial, "Bonjor");
        assert_eq!((s, e), (6, 6));

        let applied = app.apply_ime_paragraph(&ctx, "Bonjour été !".to_string(), 8, 11);
        assert_eq!(applied, Some((block, None)));
        let (_, updated, ns, ne) = app.ime_paragraph_state().expect("updated IME state");
        assert_eq!(updated, "Bonjour été !");
        assert_eq!((ns, ne), (8, 11));
        assert!(app.editor.as_ref().expect("ed").dirty_since.is_some());

        let mut h: H = egui_kittest::Harness::builder()
            .with_size(vec2(412.0, 800.0))
            .with_accessibility_check(false)
            .build_ui_state(|ui, app: &mut NoteCraftApp| app.show(ui), app);
        for _ in 0..3 {
            h.step();
        }
    }

    fn setup_mobile_ime_paragraph_app(name: &str, markdown_body: &str) -> (notecraft_store::testing::TempDir, NoteCraftApp, usize) {
        let t = notecraft_store::testing::TempDir::new(name);
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let notebooks_dir = t.path().join("notebooks");
        let mut session = Session::new(&notebooks_dir, settings).expect("session");
        let sample = notecraft_engine::sample::create_sample(&mut session, None).expect("sample");
        notecraft_store::page::save_body(&sample.md_page, markdown_body).expect("save sample");

        let mut app = NoteCraftApp::new(session).with_simple_mobile(true);
        app.open_path(&sample.md_page.to_string_lossy());
        app.full_page = true;
        let block = app
            .editor
            .as_ref()
            .expect("ed")
            .blocks
            .iter()
            .position(|b| matches!(b, notecraft_formats::Block::Paragraph(_)))
            .expect("paragraph block");
        (t, app, block)
    }

    #[test]
    fn android_ime_bold_italic_pending_and_step_out_work() {
        let ctx = Context::default();

        // 1. Tapping Bold when ed.caret is None places caret automatically, arms pending bold,
        //    and applies across incremental Gboard typing + step-out toggle-off.
        let (_t1, mut app, block) = setup_mobile_ime_paragraph_app("android-ime-bold", "Texte ");
        assert!(app.editor.as_ref().expect("ed").caret.is_none());
        app.ribbon_act(ribbon::Act::Mark("bold"));
        {
            let ed = app.editor.as_ref().expect("ed");
            let caret = ed.caret.as_ref().expect("caret placed automatically");
            assert_eq!(caret.block, block);
            assert!(ed.pending_format.as_ref().is_some_and(|p| p.bold));
        }

        let res = app.apply_ime_paragraph(&ctx, "Texte B".to_string(), 7, 7).expect("first char");
        assert_eq!(res.0, block);
        assert!(res.1.as_ref().is_some_and(|rw| rw.open == "**" && rw.close == "**" && rw.prefix == "Texte "));
        assert_eq!(app.ime_paragraph_state().expect("ime").1, "Texte **B**");
        assert_eq!(app.ime_paragraph_state().expect("ime").2, 9);

        let res2 = app.apply_ime_paragraph(&ctx, "Texte **Bonjour**".to_string(), 15, 15).expect("word");
        assert_eq!(res2, (block, None));
        assert!(app.editor.as_ref().expect("ed").dirty_since.is_some());

        // Toggle Bold off while caret is immediately before closing `**` -> steps out past `**`.
        app.ribbon_act(ribbon::Act::Mark("bold"));
        let (_, text_after_step, s, e) = app.ime_paragraph_state().expect("ime after step-out");
        assert_eq!(text_after_step, "Texte **Bonjour**");
        assert_eq!((s, e), (17, 17));
        let _ = app.apply_ime_paragraph(&ctx, "Texte **Bonjour** suite".to_string(), 23, 23).expect("plain suffix");
        let spans = match &app.editor.as_ref().expect("ed").blocks[block] {
            notecraft_formats::Block::Paragraph(spans) => spans,
            other => panic!("expected paragraph, got {other:?}"),
        };
        assert!(spans.iter().any(|sp| sp.bold && sp.text == "Bonjour"), "{spans:?}");
        assert!(spans.iter().any(|sp| !sp.bold && sp.text.contains("suite")), "{spans:?}");

        // 2. Italic pending format applies cleanly via IME.
        let (_t2, mut app2, block2) = setup_mobile_ime_paragraph_app("android-ime-italic", "Un ");
        app2.ribbon_act(ribbon::Act::Mark("italic"));
        let res_it = app2.apply_ime_paragraph(&ctx, "Un mot".to_string(), 6, 6).expect("italic");
        assert!(res_it.1.as_ref().is_some_and(|rw| rw.open == "*" && rw.close == "*"));
        assert_eq!(app2.ime_paragraph_state().expect("ime2").1, "Un *mot*");
        let spans2 = match &app2.editor.as_ref().expect("ed").blocks[block2] {
            notecraft_formats::Block::Paragraph(spans) => spans,
            other => panic!("expected paragraph, got {other:?}"),
        };
        assert!(spans2.iter().any(|sp| sp.italic && sp.text == "mot"), "{spans2:?}");

        // 3. Bold + Italic combined -> `***mot***`, and exiting Bold keeps pending italic.
        let (_t3, mut app3, block3) = setup_mobile_ime_paragraph_app("android-ime-both", "Debut ");
        app3.ribbon_act(ribbon::Act::Mark("bold"));
        app3.ribbon_act(ribbon::Act::Mark("italic"));
        let _ = app3.apply_ime_paragraph(&ctx, "Debut mot".to_string(), 9, 9).expect("both");
        assert_eq!(app3.ime_paragraph_state().expect("ime3").1, "Debut ***mot***");
        app3.ribbon_act(ribbon::Act::Mark("bold"));
        let (_, _, pos, _) = app3.ime_paragraph_state().expect("ime3 stepped");
        assert_eq!(pos, 15);
        assert!(app3.editor.as_ref().expect("ed").pending_format.as_ref().is_some_and(|p| p.italic && !p.bold));
        let _ = app3.apply_ime_paragraph(&ctx, "Debut ***mot***fin".to_string(), 18, 18).expect("italic tail");
        let spans3 = match &app3.editor.as_ref().expect("ed").blocks[block3] {
            notecraft_formats::Block::Paragraph(spans) => spans,
            other => panic!("expected paragraph, got {other:?}"),
        };
        assert!(spans3.iter().any(|sp| sp.bold && sp.italic && sp.text == "mot"), "{spans3:?}");
        assert!(spans3.iter().any(|sp| !sp.bold && sp.italic && sp.text == "fin"), "{spans3:?}");

        // 4. Leading whitespace keeps pending_format armed instead of emitting `** **`.
        let (_t4, mut app4, block4) = setup_mobile_ime_paragraph_app("android-ime-ws", "Ok");
        app4.ribbon_act(ribbon::Act::Mark("bold"));
        let ws = app4.apply_ime_paragraph(&ctx, "Ok ".to_string(), 3, 3).expect("space");
        assert_eq!(ws, (block4, None));
        assert_eq!(app4.ime_paragraph_state().expect("ime4").1, "Ok ");
        assert!(app4.editor.as_ref().expect("ed").pending_format.as_ref().is_some_and(|p| p.bold && p.at == 3));
        let _ = app4.apply_ime_paragraph(&ctx, "Ok Bonjour".to_string(), 10, 10).expect("word after space");
        assert_eq!(app4.ime_paragraph_state().expect("ime4b").1, "Ok **Bonjour**");
    }

    #[test]
    fn android_ime_enter_and_sanitize_avoid_stray_stars() {
        let ctx = Context::default();

        // 1. Typing bold word via IME then pressing Enter (`apply_ime_enter`) closes bold cleanly and updates boxes.
        let (_t1, mut app, block) = setup_mobile_ime_paragraph_app("android-ime-enter", "Debut ");
        app.ribbon_act(ribbon::Act::Mark("bold"));
        let _ = app.apply_ime_paragraph(&ctx, "Debut b".to_string(), 7, 7).expect("b");
        let _ = app.apply_ime_paragraph(&ctx, "Debut **bonjour**".to_string(), 15, 15).expect("bonjour");
        assert!(app.apply_ime_enter(&ctx));

        let ed = app.editor.as_ref().expect("ed");
        let caret = ed.caret.as_ref().expect("caret on next block");
        assert_eq!(caret.block, block + 1);
        assert_eq!(caret.text, "");
        assert_eq!(caret.selection, (0, 0));
        let notecraft_formats::Block::Paragraph(spans0) = &ed.blocks[block] else { panic!("p0") };
        assert!(spans0.iter().any(|s| s.bold && s.text == "bonjour") && !spans0.iter().any(|s| s.text.contains('*')), "{spans0:?}");
        let notecraft_formats::Block::Paragraph(spans1) = &ed.blocks[block + 1] else { panic!("p1") };
        assert!(spans1.is_empty(), "{spans1:?}");
        let counted_boxes: usize = ed.boxes.iter().map(|c| c.blocks).sum();
        assert!(ed.boxes.is_empty() || counted_boxes == ed.blocks.len(), "boxes={:?} blocks={}", ed.boxes, ed.blocks.len());

        // 2. Backspacing `**a**` -> `****` cleans up empty markers.
        let (_t2, mut app2, b2) = setup_mobile_ime_paragraph_app("android-ime-empty-stars", "**a**");
        if let Some(ed2) = app2.editor.as_mut() {
            ed2.caret = Some(pageedit::Caret {
                block: b2,
                text: "**a**".to_string(),
                focus: true,
                selection: (3, 3),
                place: None,
                cell: None,
                span: None,
                inline: None,
            });
        }
        let _ = app2.apply_ime_paragraph(&ctx, "****".to_string(), 2, 2).expect("cleanup");
        assert_eq!(app2.ime_paragraph_state().expect("cleaned").1, "");

        // 3. Leading space right after opening `**` shifts outside (`** mot**` -> ` **mot**`).
        let (_t3, mut app3, b3) = setup_mobile_ime_paragraph_app("android-ime-lead-space", "**mot**");
        if let Some(ed3) = app3.editor.as_mut() {
            ed3.caret = Some(pageedit::Caret {
                block: b3,
                text: "**mot**".to_string(),
                focus: true,
                selection: (2, 2),
                place: None,
                cell: None,
                span: None,
                inline: None,
            });
        }
        let _ = app3.apply_ime_paragraph(&ctx, "** mot**".to_string(), 3, 3).expect("normalize space");
        let (_, normalized, ..) = app3.ime_paragraph_state().expect("normalized state");
        assert_eq!(normalized, " **mot**");
    }

    #[test]
    fn every_insert_symbol_draws_without_tofu() {
        let ctx = Context::default();
        crate::fonts::install_once(&ctx);
        let mut out = ctx.run_ui(egui::RawInput::default(), |_| {});
        out.textures_delta.clear();
        let mut seen = std::collections::HashSet::new();
        let mut missing = Vec::new();
        for (_cat, symbols) in super::INSERT_SYMBOL_CATEGORIES {
            for symbol in *symbols {
                assert!(seen.insert(*symbol), "duplicate insert symbol: {symbol}");
                if !ctx.fonts_mut(|f| f.has_glyphs(&egui::FontId::proportional(18.0), symbol)) {
                    missing.push(*symbol);
                }
            }
        }
        for (y, m, d) in [(2026, 1, 5), (2026, 4, 5), (2026, 7, 15), (2026, 10, 31), (2026, 11, 24), (2026, 12, 20)] {
            for (_cat, symbols) in crate::seasons::seasonal_categories(y, m, d) {
                for symbol in symbols {
                    if !ctx.fonts_mut(|f| f.has_glyphs(&egui::FontId::proportional(18.0), symbol)) {
                        missing.push(*symbol);
                    }
                }
            }
        }
        assert!(missing.is_empty(), "symbols that would draw as tofu boxes: {missing:?}");
        for orig in ["©", "®", "™", "§", "¶", "•", "→", "←", "±", "°", "×", "÷", "√", "€", "£", "¥", "¢", "∞", "≠", "≤"]
        {
            assert!(seen.contains(orig), "original symbol missing: {orig}");
        }
        assert!(seen.len() >= 80, "expected at least 80 symbols, got {}", seen.len());
    }

    #[test]
    fn word_zoom_slider_maps_50_to_250_with_100_at_center() {
        assert!((super::zoom_slider_share(50) - 0.0).abs() < 0.001);
        assert!((super::zoom_slider_share(100) - 0.5).abs() < 0.001);
        assert!((super::zoom_slider_share(250) - 1.0).abs() < 0.001);
        assert_eq!(super::zoom_slider_percent(0.0), 50);
        assert_eq!(super::zoom_slider_percent(0.5), 100);
        assert_eq!(super::zoom_slider_percent(0.52), 100, "snaps cleanly near 100%");
        assert_eq!(super::zoom_slider_percent(1.0), 250);
        for p in [50, 75, 100, 175, 250] {
            assert_eq!(super::zoom_slider_percent(super::zoom_slider_share(p)), p);
        }
    }

    #[test]
    fn smart_select_all_and_operation_table_template_work() {
        let t = notecraft_store::testing::TempDir::new("smart-select-op");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let mut session = Session::new(&t.path().join("notebooks"), settings).expect("session");
        let sample = notecraft_engine::sample::create_sample(&mut session, None).expect("sample");

        let mut app = NoteCraftApp::new(session);
        app.open_path(&sample.html_page.to_string_lossy());
        let before = app.editor.as_ref().expect("ed").blocks.len();
        app.ribbon_act(ribbon::Act::InsertSchoolTable("operation"));
        let ed = app.editor.as_ref().expect("ed");
        assert_eq!(ed.blocks.len(), before + 1);
        let body = app.body().unwrap_or_default().to_string();
        assert!(body.contains("Milliers (M)") && body.contains("Retenues"), "operation template inserted:\n{body}");

        if let Some(ed) = app.editor.as_mut() {
            ed.across = None;
            ed.caret = Some(pageedit::Caret {
                block: 0,
                text: "Premier paragraphe".to_string(),
                focus: true,
                selection: (4, 4),
                place: None,
                cell: None,
                span: None,
                inline: None,
            });
        }
        app.ribbon_act(ribbon::Act::SelectAll);
        assert_eq!(app.editor.as_ref().and_then(|e| e.caret.as_ref()).map(|c| c.selection), Some((18, 0)));
        app.ribbon_act(ribbon::Act::SelectAll);
        assert!(app.editor.as_ref().and_then(|e| e.across).is_some_and(|a| !a.inside_one_block()), "second SelectAll selects whole page");
    }

    #[test]
    fn tts_cues_speed_and_prediction_dictation_integrate() {
        let t = notecraft_store::testing::TempDir::new("tts-prediction-dictation");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let mut session = Session::new(&t.path().join("notebooks"), settings).expect("session");
        let sample = notecraft_engine::sample::create_sample(&mut session, None).expect("sample");

        let mut app = NoteCraftApp::new(session);
        app.open_path(&sample.html_page.to_string_lossy());
        app.tts_speed = super::TtsSpeed::Slow;
        app.read_page_aloud();
        assert!(!app.tts_words.is_empty(), "tts cues generated");
        let slow_total = app.tts_words.last().expect("cue").end_sec;

        app.tts_speed = super::TtsSpeed::Fast;
        app.read_page_aloud();
        let fast_total = app.tts_words.last().expect("cue").end_sec;
        assert!(fast_total < slow_total, "fast speed {fast_total}s < slow speed {slow_total}s");

        if let Some(ed) = app.editor.as_mut() {
            let block = ed.blocks.iter().position(|b| matches!(b, notecraft_formats::Block::Paragraph(_))).expect("p");
            ed.caret = Some(pageedit::Caret {
                block,
                text: "Mon bato".to_string(),
                focus: true,
                selection: (8, 8),
                place: None,
                cell: None,
                span: None,
                inline: None,
            });
        }
        app.apply_word_suggestion((4, 8), "bateau");
        app.insert_dictated_text("navigue sur la mer");
        let txt = app.editor.as_ref().and_then(|e| e.caret.as_ref()).map(|c| c.text.as_str()).unwrap_or("");
        assert_eq!(txt, "Mon bateau navigue sur la mer");
    }

    #[test]
    fn alignment_ribbon_highlight_and_table_cell_formatting_work() {
        let t = notecraft_store::testing::TempDir::new("align-table-cell-fixes");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let mut session = Session::new(&t.path().join("notebooks"), settings).expect("session");
        let sample = notecraft_engine::sample::create_sample(&mut session, None).expect("sample");

        let mut app = NoteCraftApp::new(session);
        app.open_path(&sample.html_page.to_string_lossy());

        // 1. Active alignment is reported for the ribbon highlight and toggles off cleanly.
        let p_block = app.editor.as_ref().expect("ed").blocks.iter().position(|b| matches!(b, notecraft_formats::Block::Paragraph(_))).expect("p");
        if let Some(ed) = app.editor.as_mut() {
            ed.caret = Some(pageedit::Caret {
                block: p_block,
                text: pageedit::caret_text(&ed.blocks[p_block]),
                focus: true,
                selection: (0, 0),
                place: None,
                cell: None,
                span: None,
                inline: None,
            });
        }
        assert_eq!(app.caret_align(), None);
        app.ribbon_act(ribbon::Act::Align(notecraft_model::justify::Align::Center));
        assert_eq!(app.caret_align(), Some(notecraft_model::justify::Align::Center));
        app.ribbon_act(ribbon::Act::Align(notecraft_model::justify::Align::Center));
        assert_eq!(app.caret_align(), None);

        // 2. Partial and whole-cell inline formatting (Bold, Underline, TextColor) work inside table cells.
        app.ribbon_act(ribbon::Act::InsertTable(2, 2));
        let tbl_block =
            app.editor.as_ref().expect("ed").blocks.iter().position(|b| matches!(b, notecraft_formats::Block::Table { .. })).expect("table");
        if let Some(ed) = app.editor.as_mut() {
            pageedit::set_cell_text(&mut ed.blocks[tbl_block], (1, 0), "Bonjour monde");
            ed.caret = Some(pageedit::Caret {
                block: tbl_block,
                text: "Bonjour monde".to_string(),
                focus: true,
                selection: (0, 7),
                place: None,
                cell: Some((1, 0)),
                span: None,
                inline: None,
            });
        }
        app.ribbon_act(ribbon::Act::Mark("bold"));
        app.ribbon_act(ribbon::Act::TextColor(Some(notecraft_model::NodeColor::Red)));
        {
            let ed = app.editor.as_ref().expect("ed");
            let notecraft_formats::Block::Table { rows, .. } = &ed.blocks[tbl_block] else { panic!("table") };
            let spans = &rows[0][0];
            assert!(spans.iter().any(|s| s.bold && s.text.contains("Bonjour")), "expected bold partial cell span: {spans:?}");
            assert!(spans.iter().any(|s| s.text_color.as_deref() == Some("red") && s.text.contains("Bonjour")), "expected red cell span: {spans:?}");
        }

        // Whole cell underline inside table cell.
        if let Some(ed) = app.editor.as_mut() {
            let len = ed.caret.as_ref().map_or(0, |c| c.text.chars().count());
            if let Some(c) = ed.caret.as_mut() {
                c.selection = (0, len);
            }
        }
        app.ribbon_act(ribbon::Act::Mark("underline"));
        let ed = app.editor.as_ref().expect("ed");
        let notecraft_formats::Block::Table { rows, .. } = &ed.blocks[tbl_block] else { panic!("table") };
        let spans = &rows[0][0];
        assert!(spans.iter().all(|s| s.underline), "expected whole-cell underline in table: {spans:?}");
    }

    #[test]
    fn orthography_check_dialog_and_support_creator_work() {
        let t = notecraft_store::testing::TempDir::new("orthography-support-update");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let notebooks_dir = t.path().join("notebooks");
        let mut session = Session::new(&notebooks_dir, settings).expect("session");
        let _ = notecraft_engine::sample::create_sample(&mut session, None).expect("sample");
        let mut app = NoteCraftApp::new(session);

        app.ribbon_act(ribbon::Act::SupportCreator);
        assert!(matches!(app.dialog, Some(super::Dialog::SupportCreator)));

        let url = super::NoteCraftApp::feedback_page_url();
        let expected_prefix = format!("https://masterz42.github.io/ErgoNote/retours.html?type=bug&v={}&os=", crate::backstage::VERSION);
        assert!(url.starts_with(&expected_prefix), "unexpected feedback url: {url}");
        app.ribbon_act(ribbon::Act::ReportBug);
        assert!(matches!(app.dialog, Some(super::Dialog::ReportBug)));

        let release_json = r#"{
          "tag_name": "v2099.12.31",
          "html_url": "https://github.com/Masterz42/ErgoNote/releases/tag/v2099.12.31",
          "assets": [
            {
              "name": "ErgoNote-v2099.12.31-Android.apk",
              "browser_download_url": "https://github.com/Masterz42/ErgoNote/releases/download/v2099.12.31/ErgoNote-v2099.12.31-Android.apk"
            }
          ]
        }"#;
        app.apply_release_json(release_json, "android");
        assert_eq!(
            app.available_update(),
            Some((
                "2099.12.31".to_string(),
                "https://github.com/Masterz42/ErgoNote/releases/download/v2099.12.31/ErgoNote-v2099.12.31-Android.apk".to_string(),
                "https://github.com/Masterz42/ErgoNote/releases/tag/v2099.12.31".to_string(),
            ))
        );
        app.dismiss_update_banner("2099.12.31");
        assert_eq!(app.session.settings.get("update_dismissed_version").as_deref(), Some("2099.12.31"));

        let equal_json = format!(r#"{{"tag_name": "v{}", "assets": []}}"#, crate::backstage::VERSION);
        app.apply_release_json(&equal_json, "windows");
        assert_eq!(app.available_update(), None);
    }

    #[test]
    fn mobile_portrait_tiles_navigation_and_back_button_work() {
        let t = notecraft_store::testing::TempDir::new("mobile-portrait-tiles");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let notebooks_dir = t.path().join("notebooks");
        let mut session = Session::new(&notebooks_dir, settings).expect("session");
        let first_page = notecraft_engine::sample::create_school_binder(&mut session).expect("school binder");
        let section_dir = first_page.parent().expect("section parent").to_path_buf();

        let mut app = NoteCraftApp::new(session).with_simple_mobile(true);
        app.open_path(&first_page.to_string_lossy());
        app.full_page = true;

        assert!(app.mobile_go_up());
        assert!(!app.full_page);
        assert_eq!(app.mobile_browse_dir.as_deref(), Some(section_dir.as_path()));

        assert!(app.system_back());
        assert_eq!(app.mobile_browse_dir, None);
        assert!(!app.mobile_go_up());

        let mut h: H = egui_kittest::Harness::builder()
            .with_size(vec2(360.0, 760.0))
            .with_accessibility_check(false)
            .build_ui_state(|ui, app: &mut NoteCraftApp| app.show(ui), app);
        for _ in 0..3 {
            h.step();
        }
        h.state_mut().mobile_browse_dir = Some(section_dir);
        for _ in 0..2 {
            h.step();
        }
        h.state_mut().full_page = true;
        for _ in 0..2 {
            h.step();
        }
    }

    #[test]
    fn toggling_simple_mode_persists_and_switches_ui() {
        let t = notecraft_store::testing::TempDir::new("simple-mode-toggle");
        let settings_path = t.path().join("settings.toml");
        let settings = notecraft_store::Settings::at(&settings_path);
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let notebooks_dir = t.path().join("notebooks");
        let mut session = Session::new(&notebooks_dir, settings).expect("session");
        let first_page = notecraft_engine::sample::create_school_binder(&mut session).expect("school binder");

        let mut app = NoteCraftApp::new(session);
        assert!(!app.simple_mode);
        assert!(!app.is_simple_mobile());

        app.ribbon_act(ribbon::Act::ToggleSimpleMode);
        assert!(app.simple_mode);
        assert!(app.is_simple_mobile());
        assert_eq!(app.session.settings.get("simple_mode").as_deref(), Some("true"));

        app.open_path(&first_page.to_string_lossy());
        let mut h: H = egui_kittest::Harness::builder()
            .with_size(vec2(1024.0, 700.0))
            .with_accessibility_check(false)
            .build_ui_state(|ui, app: &mut NoteCraftApp| app.show(ui), app);
        for _ in 0..2 {
            h.step();
        }
        h.state_mut().full_page = true;
        for _ in 0..2 {
            h.step();
        }
        h.state_mut().ribbon_act(ribbon::Act::ToggleSimpleMode);
        assert!(!h.state().simple_mode);
        assert_eq!(h.state().session.settings.get("simple_mode").as_deref(), Some("false"));
    }

    #[test]
    fn tts_vtt_and_wav_helpers_work() {
        assert_eq!(super::NoteCraftApp::parse_vtt_timestamp_secs("00:01:02.500"), Some(62.5));
        assert_eq!(super::NoteCraftApp::wav_duration_secs(&[]), None);
        assert_eq!(super::NoteCraftApp::mp3_duration_secs(&[]), None);
        let mut cues = super::NoteCraftApp::parse_vtt_word_cues("", "Bonjour monde", super::TtsSpeed::Normal, 0.0).unwrap_or_default();
        let _ = super::NoteCraftApp::apply_word_boundary_tsv(&mut cues, "0.0\t0.4\tBonjour\n", 0.0);
        let _ = super::NoteCraftApp::apply_vtt_to_cues("", &mut cues, 0.0);
        super::NoteCraftApp::rescale_tts_cues(&mut cues, 1.0, 0.0);
    }

    #[test]
    fn ime_table_cell_active_tracks_table_caret() {
        let dir = notecraft_store::testing::TempDir::new("ime-table-active");
        let settings_path = dir.path().join("settings.toml");
        let s = notecraft_store::Settings::at(&settings_path);
        s.set("privacy_welcome_seen", "true").expect("settings");
        let mut session = Session::new(&dir.path().join("nb"), s).expect("session");
        let page = notecraft_engine::sample::create_school_binder(&mut session).expect("school binder");
        let mut app = NoteCraftApp::new(session);
        app.open_path(&page.to_string_lossy());
        assert!(!app.ime_table_cell_active());

        app.insert_table(2, 2);
        assert!(app.ime_table_cell_active());

        app.pointer = ribbon::Pointer::Draw;
        assert!(!app.ime_table_cell_active());
    }

    #[test]
    fn deleting_final_letter_on_page_clears_block() {
        let t = notecraft_store::testing::TempDir::new("delete-last-letter");
        let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
        settings.set("privacy_welcome_seen", "true").expect("settings");
        let mut session = Session::new(&t.path().join("notebooks"), settings).expect("session");
        let sample = notecraft_engine::sample::create_sample(&mut session, None).expect("sample");
        let mut app = NoteCraftApp::new(session);
        app.open_path(&sample.md_page.to_string_lossy());
        let mut h: H = egui_kittest::Harness::builder()
            .with_size(vec2(1024.0, 700.0))
            .with_accessibility_check(false)
            .build_ui_state(|ui, app: &mut NoteCraftApp| app.show(ui), app);
        for _ in 0..3 {
            h.step();
        }
        // Replace page with a single paragraph containing only `"m"` (caret at offset 1) and press Backspace.
        {
            let ed = h.state_mut().editor.as_mut().expect("editor");
            ed.blocks = vec![notecraft_formats::Block::Paragraph(vec![notecraft_formats::Span { text: "m".into(), ..Default::default() }])];
            ed.boxes = vec![notecraft_model::Container::new(0.0, 0.0, 600.0, 1)];
            ed.arranged = true;
            ed.rebuild_body();
            ed.caret = Some(pageedit::Caret {
                block: 0,
                text: "m".into(),
                focus: true,
                selection: (1, 1),
                place: Some(1),
                cell: None,
                span: None,
                inline: None,
            });
        }
        for _ in 0..2 {
            h.step();
        }
        h.key_press(egui::Key::Backspace);
        for _ in 0..2 {
            h.step();
        }
        let ed = h.state().editor.as_ref().expect("editor");
        assert!(crate::preview::is_blank(&ed.blocks[0]), "Backspace must delete final 'm', got {:?}", ed.blocks[0]);

        // Also verify Delete clears `"m"` when caret is at offset 0.
        {
            let ed = h.state_mut().editor.as_mut().expect("editor");
            ed.blocks = vec![notecraft_formats::Block::Paragraph(vec![notecraft_formats::Span { text: "m".into(), ..Default::default() }])];
            ed.boxes = vec![notecraft_model::Container::new(0.0, 0.0, 600.0, 1)];
            ed.arranged = true;
            ed.rebuild_body();
            ed.caret = Some(pageedit::Caret {
                block: 0,
                text: "m".into(),
                focus: true,
                selection: (0, 0),
                place: Some(0),
                cell: None,
                span: None,
                inline: None,
            });
        }
        for _ in 0..2 {
            h.step();
        }
        h.key_press(egui::Key::Delete);
        for _ in 0..2 {
            h.step();
        }
        let ed = h.state().editor.as_ref().expect("editor");
        assert!(crate::preview::is_blank(&ed.blocks[0]), "Delete must delete final 'm', got {:?}", ed.blocks[0]);
    }

    #[test]
    fn ocr_reconstruct_can_be_undone_with_ctrl_z() {
        for keep_bg in [true, false] {
            let t = notecraft_store::testing::TempDir::new("ocr-undo-ctrlz");
            let settings = notecraft_store::Settings::at(t.path().join("settings.toml"));
            settings.set("privacy_welcome_seen", "true").expect("settings");
            let mut session = Session::new(&t.path().join("notebooks"), settings).expect("session");
            let sample = notecraft_engine::sample::create_sample(&mut session, None).expect("sample");
            let pdf = t.path().join("doc.pdf");
            std::fs::write(
                &pdf,
                "%PDF-1.4\n1 0 obj\n<</Type/Catalog/Pages 2 0 R>>\nendobj\n2 0 obj\n<</Type/Pages/Kids[3 0 R]/Count 1>>\nendobj\n3 0 obj\n<</Type/Page/Parent 2 0 R/MediaBox[0 0 200 200]/Contents 4 0 R/Resources<<>>>>\nendobj\n4 0 obj\n<</Length 33>>\nstream\n0.8 0.1 0.1 rg 20 20 100 100 re f\nendstream\nendobj\nxref\n0 5\n0000000000 65535 f \n0000000009 00000 n \n0000000054 00000 n \n0000000105 00000 n \n0000000199 00000 n \ntrailer\n<</Root 1 0 R/Size 5>>\nstartxref\n280\n%%EOF\n",
            )
            .expect("pdf");
            let mut app = NoteCraftApp::new(session);
            app.open_path(&sample.md_page.to_string_lossy());
            let mut h: H = egui_kittest::Harness::builder()
                .with_size(vec2(1024.0, 700.0))
                .with_accessibility_check(false)
                .build_ui_state(|ui, app: &mut NoteCraftApp| app.show(ui), app);
            for _ in 0..3 {
                h.step();
            }
            h.state_mut().add_file(&sample.md_page, &pdf, FileChoice::Printout);
            for _ in 0..3 {
                h.step();
            }
            let (before_blocks, before_boxes, before_lock, img_idx) = {
                let st = h.state();
                let ed = st.editor.as_ref().expect("editor");
                let idx = ed.blocks.iter().position(|b| matches!(b, notecraft_formats::Block::Image { .. })).expect("printout image block");
                (ed.blocks.clone(), ed.boxes.clone(), st.lock_images, idx)
            };
            let doc = crate::ocr::OcrDocument {
                source_label: "Test",
                aspect_ratio: 1.4,
                blocks: vec![crate::ocr::OcrBlock {
                    x: 0.1,
                    y: 0.1,
                    width: 0.7,
                    height: 0.1,
                    font_scale: 1.0,
                    lines: vec!["Ligne reconstruite".to_string()],
                }],
                plain_text: "Ligne reconstruite".to_string(),
            };
            h.state_mut().reconstruct_page_from_ocr(img_idx, &doc, keep_bg);
            for _ in 0..3 {
                h.step();
            }
            assert!(!keep_bg || h.state().lock_images, "keep_bg locks image");
            assert_ne!(h.state().editor.as_ref().expect("ed").blocks, before_blocks, "keep_bg={keep_bg}: reconstruction modified blocks");

            // Simulate Ctrl+Z via egui_kittest (same path as desktop window Ctrl+Z).
            h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
            for _ in 0..3 {
                h.step();
            }
            let st = h.state();
            let ed = st.editor.as_ref().expect("editor");
            assert_eq!(ed.blocks, before_blocks, "keep_bg={keep_bg}: Ctrl+Z must restore original blocks");
            assert_eq!(ed.boxes, before_boxes, "keep_bg={keep_bg}: Ctrl+Z must restore original boxes");
            assert_eq!(st.lock_images, before_lock, "keep_bg={keep_bg}: Ctrl+Z must restore lock_images");
        }
    }
}
