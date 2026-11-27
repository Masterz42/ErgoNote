use super::*;

pub(crate) const RELEASE_NOTES: &[(&str, &str)] = &[
    (
        "Nouveau en v2026.11.27 (27/11/2026) — Zones de texte personnalisables, ruban Accueil unifié, Émojis saisonniers & Labo",
        "Onglets matières agrandis (suppression au clic droit, réorganisation par glisser-déposer, page vierge auto) ; zones de texte encadrées, personnalisables (contour couleur/épaisseur, fond, suppression rapide) et redimensionnables par les bords ; ruban Accueil unifié (Styles à gauche, Texte de base aligné avec bouton Écrire clignotant pendant le dessin, bloc Dessin & Formes dans Insérer, gros boutons Émojis saisonniers (36 smileys, 42 émojis) et Labo chimie 2D (18 schémas), Mise en page optimisée) ; correctif gomme en mode portrait smartphone.",
    ),
    (
        "v2026.11.26 (26/11/2026) — Scanner de documents ML Kit, rognage tactile direct, compteur de mots & macOS/iPadOS",
        "Scanner automatique de documents Google ML Kit sur Android (détection des 4 coins et redressage depuis le bouton Photo), rognage tactile direct à 8 poignées sur l'image, compteur de mots dynamique dans la barre d'état, zones de texte vides directement prêtes à écrire, déploiement macOS (.app / .dmg) et application native iPadOS.",
    ),
    (
        "v2026.11.25 (25/11/2026) — Onglet Image & retouche, OCR français, ruban Tableau & exports Android",
        "Nouvel onglet Image (rognage, contraste / Scan N&B, rotations, verrouillage et OCR français hors-ligne + lecture à voix haute), ruban Tableau réorganisé avec couleur des cellules, renommage complet en ErgoNote et exports natifs sur Android (PDF, HTML, DOC, sauvegarde ZIP et Partager).",
    ),
    (
        "v2026.11.24 (24/11/2026) — Mode simple en tuiles, navigation tactile & Mise en page compacte",
        "Passage à la numérotation par date (AAAA.MM.JJ), interface ultra simple en tuiles colorées façon Windows (en portrait smartphone et via Accueil > Mise en page > Simple sur PC), défilement naturel à 1 doigt + double-tap tolérant / appui long avec centrage automatique sur Android, icônes ergonomiques Image/Photo/Stylo, barre du bas épurée et groupe Mise en page compacté.",
    ),
    (
        "v1.7.2 (23/11/2026) — Notifications automatiques de mise à jour & signalement de bug",
        "Vérification discrète des nouvelles versions sur GitHub au démarrage et dans Fichier > À propos (bandeau non bloquant sans jamais écraser vos carnets), bouton Signaler un bug (?) avec formulaire anonyme et conservation permanente des 3 versions précédentes.",
    ),
    (
        "v1.7.1 (23/11/2026) — Correcteur Hunspell complet (spellbook), effacement propre du gras & Offrir un café",
        "Intégration pure Rust de spellbook 0.4.2 lisant toutes les règles flexionnelles (.aff + .dic) des dictionnaires Hunspell du système pour éliminer les faux positifs sur les conjugaisons, accords, noms invariables en -s et noms propres ; suppression des astérisques résiduels (**) lors de l'effacement d'un mot en gras ; et bouton Offrir un café dédié (don ponctuel Ko-fi).",
    ),
    (
        "v1.7.0 (23/11/2026) — Prédiction contextuelle, Correcteur & syntaxe, Dictée vocale & Lecture synchronisée",
        "Prédiction intelligente à 3 mots dès la 1re lettre et mot suivant (navigation par flèches + lecture vocale + Entrée pour valider, désactivable pour les dictées), bouton Corriger orthographe (ruban & clic droit) analysant fautes, homophones (a/à, et/est, son/sont, ce/se, ou/où), accords et -er/-é avec appui optionnel LanguageTool au clic, dictée vocale multi-OS, lecture à voix haute synchronisée directement sur la page (0,75× / 1× / 1,25×), export PDF direct, Tout sélectionner à 2 niveaux, pose d'opérations M/C/D/U et mise en forme dans les tableaux.",
    ),
    (
        "v1.6 (20/11/2026) — Interface bleu clair, ruban compact & zoom façon Word",
        "Thème bleu clair et onglets matières actifs par défaut, onglet Affichage fusionné dans Accueil, Presse-papiers et Texte de base compactés avec sélecteur de couleur de soulignage (16 couleurs), zoom déplacé en bas à droite façon Word, 134 symboles en 5 catégories, icônes couleur dédiées et installation Linux simplifiée (.deb et archive portable).",
    ),
    (
        "v1.5 (19/11/2026) — Lisibilité, saisie et langue",
        "Titre et date enfin lisibles en thème sombre, texte tapé après une coupe au milieu d'un mot gras qui reste en gras, feuilles ajoutées conservées avec leur page, date de page dans la langue de l'interface et interface limitée au français et à l'anglais.",
    ),
    (
        "v1.4 (19/11/2026) — Gras/Italique fiabilisé, touche Entrée corrigée & améliorations mobile",
        "Séparation propre des lignes au clavier sans étoiles résiduelles ni plantage, prise de photo directe et barre d'état sans chevauchement sur smartphone.",
    ),
    (
        "v1.3 (19/11/2026) — Feuilles A4 entières, pagination & ErgoNote",
        "Écriture sur toute la hauteur de la feuille A4 sans ligne de blocage au milieu, ajout automatique d'une 2ᵉ feuille dès dépassement, compteur < Feuille 1 / 2 > en bas, barre d'état épurée sans jargon technique et nouvelle identité ErgoNote.",
    ),
    (
        "v1.2 (19/11/2026) — Ruban simplifié & fusion Accueil / Insérer",
        "Fusion des onglets Accueil et Insérer en un seul onglet clair, suppression des doublons, barre flottante Play/Pause/Fin pour la lecture à voix haute et mode écriture facilité sur tablette.",
    ),
    (
        "v1.1 (18/11/2026) — Verrouillage PDF / Images en fond",
        "Bouton Verrouiller PDF / Images dans Accueil (et au clic droit sur une image) pour écrire par-dessus un exercice sans jamais le déplacer par erreur.",
    ),
    (
        "v1.1 (18/11/2026) — Police OpenDyslexic & Texte aéré TND",
        "Police OpenDyslexic intégrée sur toutes les plateformes et bouton Texte aéré TND (espacement élargi des lettres et des lignes).",
    ),
    (
        "v1.1 (18/11/2026) — Règle de lecture visuelle",
        "Guide-ligne horizontal coloré qui suit le curseur pour aider les élèves avec TND à ne pas sauter de ligne.",
    ),
    (
        "v1.1 (18/11/2026) — Lecture à voix haute",
        "Bouton Lire à voix haute dans Accueil, au clic droit et sur mobile pour écouter la sélection ou toute la page.",
    ),
    (
        "v1.1 (18/11/2026) — Exposant (x²), Indice (x₂) et Retrait",
        "Boutons directs dans le ruban Accueil pour les cours de Maths/Sciences et l'indentation des listes.",
    ),
    (
        "v1.1 (18/11/2026) — Gabarits scolaires TND en 1 clic",
        "Tableaux de conversion (km -> mm), de numération (Mille / Unités / Décimales) et de proportionnalité dans Insertion -> Tableau et Modèles de page.",
    ),
    (
        "v1.0 (18/11/2026) — Onglets matières & Palette 16 couleurs",
        "Barre d'onglets matières sous le ruban (masquant le bloc-notes à gauche) et palette de 16 couleurs avec vrai Jaune vif.",
    ),
];

impl ErgoNoteApp {
    // ---------- dialogs ----------

    pub(super) fn dialogs(&mut self, ctx: &Context) {
        if let Some(mut dialog) = self.custom_tags.take() {
            let out = dialog.show(ctx, &self.tokens);
            if let Some(list) = out.changed {
                if let Err(e) = self.session.settings.set_custom_tags(&list) {
                    self.say(crate::i18n::format_message_id("ui.could_not_save_your_tags_error", &[("error", &e.to_string())]));
                }
                if self.tags_pane.is_some() {
                    self.refresh_tags();
                }
            }
            if let Some(said) = out.said {
                self.say(said);
            }
            if !out.closed {
                self.custom_tags = Some(dialog);
            }
        }
        while self.dialog.is_none() {
            let Some(next) = self.queued_alerts.pop_front() else { break };
            if !matches!(&next, Dialog::Alert { key, .. } if self.muted_alerts.contains(key)) {
                self.dialog = Some(next);
            }
        }
        let Some(dialog) = self.dialog.take() else { return };
        // Page loading and native Shell icon generation can finish between the
        // recovery scan and the frame that presents this modal. Re-check the
        // filesystem so a healthy sidecar never produces a stale warning.
        if let Dialog::MissingAttachmentIcon { page, name } = &dialog
            && attachment_icon_healthy(page, name)
        {
            log::debug!("discarding stale missing-icon warning: page={page:?} attachment={name:?}");
            return;
        }
        let mut keep = true;
        let mut next = dialog;
        match &mut next {
            Dialog::ClearTelemetry { path } => {
                let mut clear = false;
                let mut cancel = false;
                egui::Window::new(crate::tl_id!("ui.clear_telemetry"))
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        ui.set_min_width(440.0);
                        modal_title(ui, &self.tokens, crate::tl_id!("ui.clear_telemetry"));
                        ui.label(crate::tl_id!("ui.clear_all_local_telemetry_sessions_and_exported_tele_e4f5e2340d"));
                        ui.add_space(5.0);
                        ui.colored_label(self.tokens.warn, crate::tl_id!("ui.this_cannot_be_undone"));
                        ui.add_space(14.0);
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.cancel"), false).clicked() {
                                cancel = true;
                            }
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.clear_telemetry"), true).clicked() {
                                clear = true;
                            }
                        });
                    });
                if clear {
                    let _ = std::fs::remove_dir_all(path);
                    keep = false;
                } else if cancel {
                    keep = false;
                }
            }
            Dialog::FirstUseWizard { step, level, format, add_demo, error } => {
                let mut back = false;
                let mut next = false;
                let mut finish = false;
                let last_step: u8 = 4;
                let escape = ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape));
                let backdrop = modal_backdrop(ctx, "first-use-wizard-modal-backdrop");
                let panel = self.tokens.panel;
                let dark = (u32::from(panel.r()) + u32::from(panel.g()) + u32::from(panel.b())) < 384;
                let (banner_uri, banner_bytes) = if dark {
                    ("bytes://ergonote-wizard-banner-dark", crate::BANNER_DARK_PNG)
                } else {
                    ("bytes://ergonote-wizard-banner-light", crate::BANNER_LIGHT_PNG)
                };
                let title = match *step {
                    0 => crate::tl_id!("ui.welcome_to_ergonote"),
                    1 => crate::tl_id!("ui.local_collection_level"),
                    2 => crate::tl_id!("ui.welcome_format_title"),
                    3 => crate::tl_id!("ui.welcome_notebook_prompt_title"),
                    _ => crate::tl_id!("ui.welcome_wizard_done_title"),
                }
                .to_string();
                egui::Window::new(title.as_str())
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        stay_above(ui, backdrop);
                        ui.set_min_width(520.0);
                        if *step == 0 {
                            ui.add(egui::Image::from_bytes(banner_uri, banner_bytes).fit_to_exact_size(vec2(480.0, 240.0)));
                            ui.add_space(8.0);
                        }
                        modal_title(ui, &self.tokens, title.as_str());
                        match *step {
                            0 => {}
                            1 => {
                                ui.label(crate::tl_id!("ui.privacy_is_our_top_priority_all_logs_and_telemetry_s_d87cae4eac"));
                                ui.add_space(6.0);
                                ui.label(crate::tl_id!("ui.choose_a_local_collection_level_logging_is_off_by_de_5dfbe386e0"));
                                egui::ComboBox::from_label(crate::tl_id!("ui.local_collection_level"))
                                    .selected_text(crate::tl!(level.as_str()))
                                    .show_ui(ui, |ui| {
                                        for choice in ["none", "usage", "warning", "error", "debug"] {
                                            ui.selectable_value(level, choice.into(), crate::tl!(choice));
                                        }
                                    });
                            }
                            2 => {
                                ui.label(crate::tl_id!("ui.welcome_format_body"));
                                ui.add_space(8.0);
                                for choice in PageFormat::ALL {
                                    ui.radio_value(&mut *format, choice, choice.label());
                                }
                            }
                            3 => {
                                ui.label(crate::tl_id!("ui.welcome_notebook_prompt_body"));
                                ui.label(crate::i18n::format_message_id("ui.welcome_notebook_prompt_format", &[("format", format.label())]));
                                ui.add_space(6.0);
                                ui.colored_label(self.tokens.warn, crate::tl_id!("ui.welcome_notebook_replace_warning"));
                                ui.add_space(8.0);
                                ui.radio_value(&mut *add_demo, true, crate::tl_id!("ui.welcome_notebook_add_yes"));
                                ui.radio_value(&mut *add_demo, false, crate::tl_id!("ui.welcome_notebook_add_no"));
                            }
                            _ => {
                                ui.label(crate::tl_id!("ui.welcome_wizard_done_body"));
                            }
                        }
                        if let Some(message) = error {
                            ui.add_space(8.0);
                            ui.colored_label(
                                self.tokens.warn,
                                crate::i18n::format_message_id("ui.welcome_notebook_action_failed", &[("error", message)]),
                            );
                        }
                        ui.add_space(14.0);
                        ui.horizontal(|ui| {
                            if *step > 0 && modal_button(ui, &self.tokens, crate::tl_id!("ui.welcome_wizard_back"), false).clicked() {
                                back = true;
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if *step < last_step {
                                    if modal_button(ui, &self.tokens, crate::tl_id!("ui.welcome_format_next"), true).clicked() {
                                        next = true;
                                    }
                                } else if modal_button(ui, &self.tokens, crate::tl_id!("ui.welcome_wizard_finish"), true).clicked() {
                                    finish = true;
                                }
                            });
                        });
                    });
                if back {
                    *step = step.saturating_sub(1);
                }
                if next {
                    *step = (*step + 1).min(last_step);
                }
                if escape && !finish && *step > 0 {
                    *step -= 1;
                }
                if finish {
                    let level = level.clone();
                    match self.finish_first_use_wizard(&level, *format, *add_demo) {
                        Ok(()) => keep = false,
                        Err(message) => *error = Some(message),
                    }
                }
            }
            Dialog::Alert { key, title, message, suppress, suppress_forever } => {
                let mut ok = false;
                egui::Window::new(title.as_str())
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        modal_title(ui, &self.tokens, title.as_str());
                        ui.set_min_width(420.0);
                        ui.label(crate::i18n::pseudo_dynamic(message));
                        ui.add_space(8.0);
                        ui.checkbox(suppress, crate::tl_id!("ui.don_t_show_this_again_in_this_session"));
                        ui.checkbox(suppress_forever, crate::tl_id!("ui.don_t_show_this_ever"));
                        ui.add_space(12.0);
                        if modal_button(ui, &self.tokens, crate::tl_id!("ui.ok"), true).clicked() {
                            ok = true;
                        }
                    });
                if ok {
                    if *suppress {
                        self.muted_alerts.insert(key.clone());
                    }
                    if *suppress_forever {
                        let _ = self.session.settings.set(&format!("{key}_never"), "true");
                    }
                    keep = false;
                }
            }
            Dialog::BackupRecovery { feedback } => {
                let mut do_save_now = false;
                let mut do_export_zip = false;
                let mut do_import_zip = false;
                let mut do_open_folder = false;
                let mut close = false;
                // On a phone the dialog must fit the screen: a fixed 480 points ran off its
                // left edge, and the long labels pushed it wider still (owner, 2026-11-24).
                let narrow = ctx.content_rect().width() < 560.0;
                let width = (ctx.content_rect().width() - 28.0).clamp(280.0, 480.0);
                let backdrop = modal_backdrop(ctx, "backup-recovery-backdrop");
                egui::Window::new(crate::tl_id!("ui.backup_recovery"))
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        stay_above(ui, backdrop);
                        modal_title(ui, &self.tokens, crate::tl_id!("ui.backup_recovery"));
                        ui.set_width(width);
                        ui.label(egui::RichText::new(crate::tl_id!("ui.student_autosave_notice")).size(14.0).color(self.tokens.text));
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(format!("Dossier des classeurs : {}", self.session.root().display()))
                                .size(12.0)
                                .color(self.tokens.muted),
                        );
                        ui.add_space(12.0);
                        let save_label = if narrow { "✓ Enregistrer" } else { "✓  Enregistrer maintenant" };
                        if modal_button(ui, &self.tokens, save_label, true).clicked() {
                            do_save_now = true;
                        }
                        ui.add_space(8.0);
                        let export_label = if narrow { "Sauvegarder (.zip)" } else { "Sauvegarder tout le cartable (.zip) — clé USB / transfert" };
                        if modal_button(ui, &self.tokens, export_label, true).clicked() {
                            do_export_zip = true;
                        }
                        ui.add_space(8.0);
                        let import_label = if narrow { "Récupérer (.zip)" } else { "Récupérer une sauvegarde (.zip)" };
                        if modal_button(ui, &self.tokens, import_label, false).clicked() {
                            do_import_zip = true;
                        }
                        ui.add_space(8.0);
                        let folder_label = if narrow { "Ouvrir le dossier" } else { "Ouvrir le dossier des classeurs" };
                        if modal_button(ui, &self.tokens, folder_label, false).clicked() {
                            do_open_folder = true;
                        }
                        if let Some(msg) = feedback.as_deref() {
                            ui.add_space(10.0);
                            ui.label(egui::RichText::new(msg).strong().color(self.tokens.ok));
                        }
                        ui.add_space(14.0);
                        if modal_button(ui, &self.tokens, crate::tl_id!("ui.close"), false).clicked() {
                            close = true;
                        }
                    });
                if do_save_now {
                    self.save_now();
                    if let Some(pad) = self.pad.as_mut()
                        && pad.dirty
                    {
                        let _ = pad.save();
                    }
                    let msg = "Tout a bien été enregistré à l'instant !".to_string();
                    self.say(msg.clone());
                    *feedback = Some(msg);
                } else if do_export_zip {
                    if let Some(msg) = self.handle_export_notebook_zip() {
                        *feedback = Some(msg);
                    }
                } else if do_import_zip {
                    if let Some(msg) = self.handle_import_zip() {
                        *feedback = Some(msg);
                    }
                } else if do_open_folder {
                    let root = self.session.root().to_path_buf();
                    self.reveal(&root);
                } else if close || ctx.input(|i| i.key_pressed(Key::Escape)) {
                    keep = false;
                }
            }
            Dialog::OrthographyCheck { analyzed_text, target_blocks, issues, online_checked } => {
                let mut fix_one: Option<(usize, String)> = None;
                let mut ignore_one: Option<usize> = None;
                let mut speak_one: Option<String> = None;
                let mut fix_all = false;
                let mut close = false;
                let width = (ctx.content_rect().width() - 32.0).clamp(300.0, 600.0);
                let backdrop = modal_backdrop(ctx, "orthography-check-backdrop");
                let title = "Correcteur d'orthographe & syntaxe";
                let corrected_preview = Self::orthography_corrected_preview(target_blocks, issues);
                egui::Window::new(title)
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        stay_above(ui, backdrop);
                        modal_title(ui, &self.tokens, title);
                        ui.set_width(width);
                        let subtitle = if *online_checked {
                            format!("Langue : {} — Analyse pédagogique locale + LanguageTool.", self.spell_lang.label())
                        } else {
                            format!("Langue : {} — Analyse orthographe, homophones & règles hors-ligne.", self.spell_lang.label())
                        };
                        ui.label(egui::RichText::new(subtitle).size(12.0).color(self.tokens.muted));
                        if !analyzed_text.trim().is_empty() {
                            ui.add_space(6.0);
                            egui::Frame::new()
                                .fill(self.tokens.panel)
                                .stroke(egui::Stroke::new(1.0, self.tokens.faint))
                                .corner_radius(7.0)
                                .inner_margin(egui::Margin::same(8))
                                .show(ui, |ui| {
                                    ui.set_width((width - 20.0).max(220.0));
                                    ui.label(
                                        egui::RichText::new(crate::tl_id!("ui.student_spell_analysed")).strong().size(11.5).color(self.tokens.muted),
                                    );
                                    ui.label(egui::RichText::new(analyzed_text.trim()).size(12.5).color(self.tokens.text));
                                    if !issues.is_empty() && corrected_preview.trim() != analyzed_text.trim() {
                                        ui.add_space(4.0);
                                        ui.label(
                                            egui::RichText::new(crate::tl_id!("ui.student_spell_preview"))
                                                .strong()
                                                .size(11.5)
                                                .color(Color32::from_rgb(24, 128, 60)),
                                        );
                                        ui.label(egui::RichText::new(corrected_preview.trim()).size(12.5).color(self.tokens.strong));
                                    }
                                });
                        }
                        ui.add_space(8.0);
                        if issues.is_empty() {
                            egui::Frame::new()
                                .fill(crate::theme::mix(self.tokens.panel, Color32::from_rgb(35, 145, 70), 0.12))
                                .corner_radius(8.0)
                                .inner_margin(egui::Margin::same(12))
                                .show(ui, |ui| {
                                    ui.label(egui::RichText::new(crate::tl_id!("ui.student_spell_none")).strong().color(self.tokens.strong));
                                });
                        } else {
                            egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                                ui.spacing_mut().item_spacing = vec2(6.0, 8.0);
                                for (idx, (_b_idx, issue)) in issues.iter().enumerate() {
                                    egui::Frame::new()
                                        .fill(self.tokens.editor)
                                        .stroke(egui::Stroke::new(1.0, self.tokens.faint))
                                        .corner_radius(8.0)
                                        .inner_margin(egui::Margin::same(9))
                                        .show(ui, |ui| {
                                            ui.set_width((width - 24.0).max(220.0));
                                            ui.horizontal_wrapped(|ui| {
                                                ui.label(
                                                    egui::RichText::new(format!("« {} »", issue.original))
                                                        .strong()
                                                        .size(14.0)
                                                        .color(Color32::from_rgb(205, 50, 50)),
                                                );
                                                ui.label(egui::RichText::new(&issue.message).size(12.5).color(self.tokens.muted));
                                            });
                                            ui.add_space(4.0);
                                            ui.horizontal_wrapped(|ui| {
                                                for sugg in issue.suggestions.iter().take(crate::spellcheck::MAX_SUGGESTIONS) {
                                                    let btn = egui::Button::new(egui::RichText::new(sugg).strong().color(self.tokens.strong))
                                                        .fill(crate::theme::mix(self.tokens.panel, self.tokens.accent, 0.16))
                                                        .corner_radius(5.0)
                                                        .min_size(vec2(0.0, 24.0));
                                                    if ui.add(btn).clicked() {
                                                        fix_one = Some((idx, sugg.clone()));
                                                    }
                                                    let (sr, sresp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
                                                    let sp = ui.painter_at(sr);
                                                    sp.rect_filled(
                                                        sr,
                                                        5.0,
                                                        if sresp.hovered() {
                                                            crate::theme::mix(self.tokens.panel, self.tokens.accent, 0.28)
                                                        } else {
                                                            crate::theme::mix(self.tokens.panel, self.tokens.accent, 0.12)
                                                        },
                                                    );
                                                    icons::paint(&sp, sr.shrink(4.0), Icon::ReadAloud, self.tokens.accent);
                                                    if sresp.on_hover_text(crate::tl_id!("ui.student_listen_correction")).clicked() {
                                                        speak_one = Some(sugg.clone());
                                                    }
                                                }
                                                if ui
                                                    .add(
                                                        egui::Button::new(
                                                            egui::RichText::new(crate::tl_id!("ui.student_ignore"))
                                                                .size(12.0)
                                                                .color(self.tokens.muted),
                                                        )
                                                        .fill(self.tokens.panel)
                                                        .corner_radius(5.0)
                                                        .min_size(vec2(0.0, 24.0)),
                                                    )
                                                    .clicked()
                                                {
                                                    ignore_one = Some(idx);
                                                }
                                            });
                                        });
                                }
                            });
                        }
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            if !issues.is_empty() && modal_button(ui, &self.tokens, "Tout corriger", true).clicked() {
                                fix_all = true;
                            }
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.close"), issues.is_empty()).clicked() {
                                close = true;
                            }
                        });
                    });
                if let Some(word) = speak_one {
                    self.speak_word_preview(&word);
                } else if let Some(idx) = ignore_one {
                    if idx < issues.len() {
                        issues.remove(idx);
                    }
                } else if let Some((idx, rep)) = fix_one {
                    if idx < issues.len() {
                        let (b_idx, removed) = issues.remove(idx);
                        let orig_end = removed.char_range.1;
                        let delta = self.apply_orthography_fix(b_idx, removed.char_range, &rep);
                        for (tb_b, tb_start, tb_txt) in target_blocks.iter_mut() {
                            if *tb_b == b_idx {
                                let len = tb_txt.chars().count();
                                if removed.char_range.0 >= *tb_start && removed.char_range.1 <= *tb_start + len {
                                    let mut chs: Vec<char> = tb_txt.chars().collect();
                                    let lo = removed.char_range.0 - *tb_start;
                                    let hi = removed.char_range.1 - *tb_start;
                                    chs.splice(lo..hi, rep.chars());
                                    *tb_txt = chs.into_iter().collect();
                                } else if orig_end <= *tb_start && delta != 0 {
                                    *tb_start = tb_start.saturating_add_signed(delta);
                                }
                            }
                        }
                        if delta != 0 {
                            for (ob, oi) in issues.iter_mut() {
                                if *ob == b_idx && oi.char_range.0 >= orig_end {
                                    oi.char_range.0 = oi.char_range.0.saturating_add_signed(delta);
                                    oi.char_range.1 = oi.char_range.1.saturating_add_signed(delta);
                                }
                            }
                        }
                        self.say(format!("Corrigé : « {} » -> « {rep} ».", removed.original));
                        if issues.is_empty() {
                            keep = false;
                        }
                    }
                } else if fix_all {
                    let to_apply: Vec<(usize, (usize, usize), String)> =
                        issues.drain(..).filter_map(|(b, iss)| iss.suggestions.first().map(|s| (b, iss.char_range, s.clone()))).collect();
                    let total = to_apply.len();
                    for (b_idx, range, rep) in to_apply.into_iter().rev() {
                        self.apply_orthography_fix(b_idx, range, &rep);
                    }
                    self.say(format!("{total} correction(s) appliquée(s)."));
                    keep = false;
                } else if close || ctx.input(|i| i.key_pressed(Key::Escape)) {
                    keep = false;
                }
            }
            Dialog::SupportCreator => {
                let mut open_local = false;
                let mut open_url: Option<&'static str> = None;
                let mut close = false;
                let width = (ctx.content_rect().width() - 32.0).clamp(280.0, 460.0);
                let backdrop = modal_backdrop(ctx, "support-creator-backdrop");
                let title = "Soutenir ErgoNote — Offrir un café";
                egui::Window::new(title)
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        stay_above(ui, backdrop);
                        modal_title(ui, &self.tokens, title);
                        ui.set_width(width);
                        ui.label(egui::RichText::new(crate::tl_id!("ui.student_support_about")).size(13.0).color(self.tokens.text));
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new(crate::tl_id!("ui.student_support_ask")).size(12.5).color(self.tokens.muted));
                        ui.add_space(10.0);
                        let warm = Color32::from_rgb(180, 95, 20);
                        if ui
                            .add(
                                egui::Button::new(egui::RichText::new(crate::tl_id!("ui.student_open_support")).strong().color(Color32::WHITE))
                                    .fill(warm)
                                    .corner_radius(7.0)
                                    .min_size(vec2(width, 32.0)),
                            )
                            .clicked()
                        {
                            open_local = true;
                        }
                        ui.add_space(6.0);
                        if ui
                            .add(
                                egui::Button::new(egui::RichText::new(crate::tl_id!("ui.student_buy_coffee")).color(self.tokens.strong))
                                    .fill(crate::theme::mix(self.tokens.panel, self.tokens.accent, 0.14))
                                    .corner_radius(7.0)
                                    .min_size(vec2(width, 28.0)),
                            )
                            .clicked()
                        {
                            open_url = Some("https://ko-fi.com/ergonote");
                        }
                        ui.add_space(10.0);
                        if modal_button(ui, &self.tokens, crate::tl_id!("ui.close"), true).clicked() {
                            close = true;
                        }
                    });
                if open_local {
                    #[cfg(target_arch = "wasm32")]
                    ctx.open_url(egui::OpenUrl::new_tab(Self::support_page_url()));
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        const SUPPORT_HTML: &str = include_str!("../../../../presentation/soutenir.html");
                        let candidate = Self::support_page_url();
                        if std::path::Path::new(&candidate).is_file() {
                            let raw = candidate.replace('\\', "/");
                            let url = if raw.starts_with('/') { format!("file://{raw}") } else { format!("file:///{raw}") };
                            let _ = open_url_in_system(&url);
                        } else {
                            let tmp = std::env::temp_dir().join("ergonote-soutenir.html");
                            if std::fs::write(&tmp, SUPPORT_HTML).is_ok() {
                                let raw = tmp.to_string_lossy().replace('\\', "/");
                                let url = if raw.starts_with('/') { format!("file://{raw}") } else { format!("file:///{raw}") };
                                let _ = open_url_in_system(&url);
                            } else {
                                let _ = open_url_in_system(&candidate);
                            }
                        }
                    }
                    self.say("Page de soutien ouverte dans votre navigateur.");
                } else if let Some(url) = open_url {
                    #[cfg(target_arch = "wasm32")]
                    ctx.open_url(egui::OpenUrl::new_tab(url));
                    #[cfg(not(target_arch = "wasm32"))]
                    let _ = open_url_in_system(url);
                    self.say("Lien de soutien ouvert dans votre navigateur.");
                } else if close || ctx.input(|i| i.key_pressed(Key::Escape)) {
                    keep = false;
                }
            }
            Dialog::ReportBug => {
                let mut close = false;
                let mut open_web = false;
                let width = (ctx.content_rect().width() - 32.0).clamp(280.0, 460.0);
                let backdrop = modal_backdrop(ctx, "report-bug-backdrop");
                let title = "Signaler un bug ou donner un avis";
                let os_label = Self::feedback_platform_label();
                egui::Window::new(title)
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        stay_above(ui, backdrop);
                        modal_title(ui, &self.tokens, title);
                        ui.set_width(width);
                        ui.label(egui::RichText::new(crate::tl_id!("ui.student_feedback_prompt")).size(13.0).color(self.tokens.text));
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "Version pré-remplie : ErgoNote v{} ({}). Aucun compte requis ; l'adresse e-mail reste facultative.",
                                crate::backstage::VERSION,
                                os_label
                            ))
                            .size(12.0)
                            .color(self.tokens.muted),
                        );
                        ui.add_space(10.0);
                        if ui
                            .add(
                                egui::Button::new(egui::RichText::new(crate::tl_id!("ui.student_open_feedback")).strong().color(Color32::WHITE))
                                    .fill(self.tokens.accent)
                                    .corner_radius(7.0)
                                    .min_size(vec2(width, 32.0)),
                            )
                            .clicked()
                        {
                            open_web = true;
                        }
                        ui.add_space(10.0);
                        if modal_button(ui, &self.tokens, crate::tl_id!("ui.close"), true).clicked() {
                            close = true;
                        }
                    });
                if open_web {
                    let url = Self::feedback_page_url();
                    self.open_external_url(ctx, &url);
                    self.say("Formulaire de retour ouvert dans votre navigateur.");
                    keep = false;
                } else if close || ctx.input(|i| i.key_pressed(Key::Escape)) {
                    keep = false;
                }
            }
            Dialog::ReleaseNotes => {
                let mut close = false;
                let short = crate::backstage::VERSION.strip_suffix(".0").unwrap_or(crate::backstage::VERSION);
                let title = format!("Nouveautés — ErgoNote v{short}");
                let width = (ctx.content_rect().width() - 32.0).clamp(260.0, 460.0);
                let backdrop = modal_backdrop(ctx, "release-notes-backdrop");
                egui::Window::new(&title)
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        stay_above(ui, backdrop);
                        modal_title(ui, &self.tokens, &title);
                        ui.set_width(width);
                        ui.label(
                            egui::RichText::new(format!("Version complète : v{}", crate::backstage::VERSION)).size(12.5).color(self.tokens.muted),
                        );
                        ui.add_space(8.0);
                        egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                            for (heading, desc) in RELEASE_NOTES {
                                ui.label(egui::RichText::new(format!("• {heading}")).strong().size(13.5).color(self.tokens.text));
                                ui.label(egui::RichText::new(*desc).size(12.5).color(self.tokens.muted));
                                ui.add_space(6.0);
                            }
                        });
                        ui.add_space(10.0);
                        if modal_button(ui, &self.tokens, crate::tl_id!("ui.close"), true).clicked() {
                            close = true;
                        }
                    });
                if close || ctx.input(|i| i.key_pressed(Key::Escape)) {
                    keep = false;
                }
            }
            Dialog::ResetPreferences => {
                let mut reset = false;
                let mut cancel = false;
                egui::Window::new(crate::tl_id!("ui.reset_app_preferences"))
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        modal_title(ui, &self.tokens, crate::tl_id!("ui.reset_app_preferences"));
                        ui.set_min_width(460.0);
                        ui.label(crate::tl_id!("ui.reset_ergonote_s_saved_preferences_to_their_built_in_defaults"));
                        ui.add_space(6.0);
                        ui.colored_label(self.tokens.warn, crate::tl_id!("ui.this_does_not_delete_notebooks_or_pages_it_cannot_be_undone"));
                        ui.add_space(14.0);
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.reset_preferences"), true).clicked() {
                                reset = true;
                            }
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.cancel"), false).clicked() {
                                cancel = true;
                            }
                        });
                    });
                if reset {
                    match self.session.settings.reset() {
                        Ok(()) => {
                            self.ui_scale = 100;
                            self.ui_scale_pending = true;
                            self.dialog = Some(self.first_use_wizard());
                        }
                        Err(e) => {
                            self.complain(&crate::i18n::format_message_id("ui.could_not_reset_app_preferences_error", &[("error", &e.to_string())]))
                        }
                    }
                    keep = false;
                } else if cancel {
                    keep = false;
                }
            }
            Dialog::MissingAttachmentIcon { page, name } => {
                let mut repair = false;
                let mut leave = false;
                let mut suppress = self.icon_recovery_suppressed;
                let backdrop = modal_backdrop(ctx, "missing-attachment-icon-backdrop");
                egui::Window::new(crate::tl_id!("ui.attachment_recovery"))
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        stay_above(ui, backdrop);
                        modal_title(ui, &self.tokens, crate::tl_id!("ui.attachment_icon_missing"));
                        ui.set_min_width(520.0);
                        let message =
                            crate::i18n::format_message_id("ui.ergonote_found_an_attachment_without_its_generated_icon_name", &[("name", name)]);
                        ui.label(egui::RichText::new(message).strong());
                        ui.add_space(6.0);
                        ui.label(crate::tl_id!("ui.the_original_file_is_still_safe_ergonote_can_regene_f10ded524a"));
                        ui.add_space(8.0);
                        ui.checkbox(&mut suppress, crate::tl_id!("ui.do_not_warn_about_similar_missing_icons_again_this_session"));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.repair_icon"), true).clicked() {
                                repair = true;
                            }
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.leave_it"), false).clicked() {
                                leave = true;
                            }
                        });
                    });
                if repair {
                    self.icon_recovery_suppressed = suppress;
                    match ergonote_store::assets::ensure_icon(page, name).and_then(|()| {
                        let icon_exists = attachment_icon_exists(page, name);
                        if !icon_exists {
                            return Err(ergonote_store::StoreError::invalid("the icon generator produced no sidecar file"));
                        }
                        self.repair_attachment_reference(page, name).map_err(ergonote_store::StoreError::invalid).map(|_| ())
                    }) {
                        Ok(()) => {
                            log::info!("repaired missing attachment icon and reference: page={:?} attachment={name:?}", page);
                            self.say(crate::i18n::format_message_id("ui.repaired_the_attachment_icon_for_name", &[("name", name)]));
                        }
                        Err(e) => {
                            log::error!("could not repair missing attachment icon: page={:?} attachment={name:?} error={e}", page);
                            self.complain(&crate::i18n::format_message_id(
                                "ui.could_not_repair_the_icon_for_name_error",
                                &[("name", name), ("error", &e.to_string())],
                            ));
                        }
                    }
                    keep = false;
                } else if leave {
                    self.icon_recovery_suppressed = suppress;
                    log::error!("user declined missing attachment icon repair: page={:?} attachment={name:?}", page);
                    keep = false;
                } else {
                    self.icon_recovery_suppressed = suppress;
                }
            }
            Dialog::UnreferencedAttachment { page, name } => {
                let mut restore = false;
                let mut leave = false;
                let mut suppress = self.icon_recovery_suppressed;
                let backdrop = modal_backdrop(ctx, "unreferenced-attachment-backdrop");
                egui::Window::new(crate::tl_id!("ui.attachment_recovery"))
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        stay_above(ui, backdrop);
                        modal_title(ui, &self.tokens, crate::tl_id!("ui.attachment_not_linked"));
                        ui.set_min_width(520.0);
                        let message =
                            crate::i18n::format_message_id("ui.ergonote_found_an_attachment_that_is_not_referenced_8c0eb51257", &[("name", name)]);
                        ui.label(egui::RichText::new(message).strong());
                        ui.add_space(6.0);
                        ui.label(crate::tl_id!("ui.the_file_is_safe_ergonote_can_append_a_link_to_it_a_c31b3d9716"));
                        ui.add_space(8.0);
                        ui.checkbox(&mut suppress, crate::tl_id!("ui.do_not_warn_about_similar_attachment_issues_again_this_session"));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.append_link"), true).clicked() {
                                restore = true;
                            }
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.leave_it"), false).clicked() {
                                leave = true;
                            }
                        });
                    });
                if restore {
                    self.icon_recovery_suppressed = suppress;
                    match self.repair_attachment_reference(page, name) {
                        Ok(true) => {
                            log::info!("restored unreferenced attachment link: page={:?} attachment={name:?}", page);
                            self.say(crate::i18n::format_message_id("ui.added_a_link_to_name_at_the_end_of_the_page", &[("name", name)]));
                        }
                        Ok(false) => log::debug!("attachment recovery found no change needed: page={:?} attachment={name:?}", page),
                        Err(e) => {
                            log::error!("could not restore unreferenced attachment link: page={:?} attachment={name:?} error={e}", page);
                            self.complain(&crate::i18n::format_message_id(
                                "ui.could_not_restore_the_link_for_name_error",
                                &[("name", name), ("error", &e.to_string())],
                            ));
                        }
                    }
                    keep = false;
                } else if leave {
                    self.icon_recovery_suppressed = suppress;
                    log::error!("user declined unreferenced attachment recovery: page={:?} attachment={name:?}", page);
                    keep = false;
                } else {
                    self.icon_recovery_suppressed = suppress;
                }
            }
            Dialog::OpenAttachmentWarning { path } => {
                let mut open = false;
                let mut cancel = false;
                let mut suppress = self.open_attachment_warning_suppressed;
                let backdrop = modal_backdrop(ctx, "open-attachment-warning-backdrop");
                egui::Window::new(crate::tl_id!("ui.open_attachment"))
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        stay_above(ui, backdrop);
                        modal_title(ui, &self.tokens, crate::tl_id!("ui.open_attachment"));
                        ui.set_min_width(520.0);
                        ui.label(egui::RichText::new(crate::tl_id!("ui.this_file_will_be_opened_by_its_associated_application")).strong());
                        ui.add_space(6.0);
                        ui.label(self.shown(path).to_string());
                        ui.add_space(6.0);
                        ui.label(crate::tl_id!("ui.only_open_it_if_you_trust_the_file_and_the_person_who_created_it"));
                        ui.add_space(8.0);
                        ui.checkbox(&mut suppress, crate::tl_id!("ui.do_not_warn_again_for_this_file_type_this_session"));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.open"), true).clicked() {
                                open = true;
                            }
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.cancel"), false).clicked() {
                                cancel = true;
                            }
                        });
                    });
                if open {
                    self.open_attachment_warning_suppressed = suppress;
                    match open_file_for_viewer(path) {
                        Ok(()) => self.say(crate::i18n::format_message_id("ui.opening_path", &[("path", &self.shown(path))])),
                        Err(e) => self.complain(&crate::i18n::format_message_id("ui.could_not_open_the_file_error", &[("error", &e.to_string())])),
                    }
                    keep = false;
                } else if cancel {
                    self.open_attachment_warning_suppressed = suppress;
                    log::info!("user cancelled opening attachment: file={:?}", path);
                    keep = false;
                } else {
                    self.open_attachment_warning_suppressed = suppress;
                }
            }
            Dialog::FileOptions { dir, files } => {
                let (dir, file) = (dir.clone(), files.first().cloned().unwrap_or_default());
                let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let left = files.len().saturating_sub(1);
                let kind = ergonote_store::assets::media_kind(&name);
                let mut chosen: Option<FileChoice> = None;
                let title = if left > 0 {
                    crate::i18n::format_message_id("ui.add_name_left_more", &[("name", &name), ("left", &left.to_string())])
                } else {
                    crate::i18n::format_message_id("ui.add_name", &[("name", &name)])
                };
                egui::Window::new(crate::tl_id!("ui.file_options"))
                    .collapsible(false)
                    .resizable(false)
                    .title_bar(false)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        ui.set_min_width(380.0);
                        modal_title(ui, &self.tokens, &title);
                        ui.label(
                            egui::RichText::new(crate::tl_id!("ui.what_should_happen_to_this_file")).strong().size(16.0).color(self.tokens.text),
                        );
                        ui.add_space(6.0);
                        let copy_label = match kind {
                            ergonote_store::assets::MediaKind::Image => crate::tl_id!("ui.insert_the_picture"),
                            ergonote_store::assets::MediaKind::Video => crate::tl_id!("ui.insert_the_video"),
                            ergonote_store::assets::MediaKind::Audio => crate::tl_id!("ui.insert_the_recording"),
                            ergonote_store::assets::MediaKind::Other => crate::tl_id!("ui.attach_the_file"),
                        };
                        if modal_button(ui, &self.tokens, copy_label, true).clicked() {
                            log::debug!("file attach choice clicked: choice=copy name={name:?}");
                            chosen = Some(FileChoice::Copy);
                        }
                        ui.label(
                            egui::RichText::new(crate::tl_id!("ui.a_copy_goes_into_this_page_s_folder_so_the_notebook_5f331e4138"))
                                .small()
                                .color(self.tokens.muted),
                        );
                        ui.add_space(6.0);
                        let can_printout = printout::supports(&file);
                        if modal_button_enabled(ui, &self.tokens, crate::tl_id!("ui.insert_printout"), false, can_printout).clicked() {
                            log::debug!("file attach choice clicked: choice=printout name={name:?}");
                            chosen = Some(FileChoice::Printout);
                        }
                        ui.label(
                            egui::RichText::new(if can_printout {
                                crate::tl_id!("ui.copies_the_file_and_inserts_one_picture_for_each_printed_page")
                            } else {
                                crate::tl_id!("ui.printout_is_available_for_pdf_and_office_documents")
                            })
                            .small()
                            .color(self.tokens.muted),
                        );
                        ui.add_space(6.0);
                        if modal_button(ui, &self.tokens, crate::tl_id!("ui.link_to_the_file_where_it_is"), false).clicked() {
                            log::debug!("file attach choice clicked: choice=link name={name:?}");
                            chosen = Some(FileChoice::Link);
                        }
                        ui.label(
                            egui::RichText::new(crate::tl_id!("ui.nothing_is_copied_the_link_breaks_if_the_file_moves"))
                                .small()
                                .color(self.tokens.muted),
                        );
                        ui.add_space(6.0);
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.cancel"), false).clicked() {
                                keep = false;
                            }
                        });
                    });
                if let Some(how) = chosen {
                    self.add_file(&dir, &file, how);
                    if let Dialog::FileOptions { files, .. } = &mut next {
                        files.remove(0);
                        keep = !files.is_empty();
                    }
                }
            }
            Dialog::NewNotebook { name, format } => {
                let mut go = false;
                egui::Window::new(crate::tl_id!("ui.new_notebook"))
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        modal_title(ui, &self.tokens, crate::tl_id!("ui.new_notebook"));
                        ui.set_min_width(420.0);
                        ui.label(crate::tl_id!("ui.name_2"));
                        let r = ui.text_edit_singleline(name);
                        // Read Enter before asking for focus again: a new focus request hides the lost focus.
                        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        r.request_focus();
                        format_picker(ui, format);
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.cancel"), false).clicked() {
                                keep = false;
                            }
                            let ready = !name.trim().is_empty();
                            let create =
                                ui.add_enabled_ui(ready, |ui| modal_button(ui, &self.tokens, crate::tl_id!("ui.create"), true).clicked()).inner;
                            if (create || enter) && ready {
                                go = true;
                            }
                        });
                    });
                if go {
                    let mut p = params(&[("name", name.trim())]);
                    if let Some(f) = format {
                        p.insert("format".into(), f.id().into());
                    }
                    if let Some(Output::Path(dir)) = self.run("notebook.create", &p) {
                        self.open.insert(dir);
                        self.say(crate::tl_id!("ui.notebook_created"));
                    }
                    keep = false;
                }
            }
            Dialog::Add { parent, parent_kind, title, kind, format } => {
                let mut go = false;
                let heading = match *parent_kind {
                    NodeKind::Page => crate::tl_id!("ui.add_a_sub_page").to_string(),
                    NodeKind::Section => crate::tl_id!("ui.add_inside_this_section").to_string(),
                    _ => {
                        let kind = crate::tl!(parent_kind.label());
                        crate::i18n::format_message_id("ui.add_inside_this_kind", &[("kind", &kind.to_lowercase())])
                    }
                };
                egui::Window::new(heading.clone())
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        modal_title(ui, &self.tokens, &heading);
                        ui.set_min_width(420.0);
                        let kinds = parent_kind.may_contain();
                        if kinds.len() > 1 {
                            ui.label(crate::tl_id!("ui.what_to_add"));
                            ui.horizontal(|ui| {
                                for k in kinds {
                                    ui.selectable_value(kind, *k, crate::tl!(k.label()));
                                }
                            });
                        }
                        ui.label(crate::tl_id!("ui.title_2"));
                        let r = ui.text_edit_singleline(title);
                        // Read Enter before asking for focus again: a new focus request hides the lost focus.
                        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        r.request_focus();
                        ui.label(if *kind == NodeKind::Page {
                            crate::tl_id!("ui.format_for_this_page")
                        } else {
                            crate::tl_id!("ui.default_format_for_pages_below")
                        });
                        format_picker(ui, format);
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.cancel"), false).clicked() {
                                keep = false;
                            }
                            let ready = !title.trim().is_empty();
                            let create =
                                ui.add_enabled_ui(ready, |ui| modal_button(ui, &self.tokens, crate::tl_id!("ui.create"), true).clicked()).inner;
                            if (create || enter) && ready {
                                go = true;
                            }
                        });
                    });
                if go {
                    let mut p = params(&[("parent", &path_str(parent)), ("kind", kind.id()), ("title", title.trim())]);
                    if let Some(f) = format {
                        p.insert("format".into(), f.id().into());
                    }
                    if let Some(Output::Path(dir)) = self.run("node.create", &p) {
                        self.open.insert(parent.clone());
                        if *kind == NodeKind::Page {
                            self.open_page(&dir);
                        } else if *kind == NodeKind::Section {
                            self.open.insert(dir.clone());
                            self.selected = Some(dir.clone());
                            if let Some(page) = first_page(self.session.tree(), &dir) {
                                self.open_page(&page);
                            } else if let Some(Output::Path(page)) =
                                self.run("node.create", &params(&[("parent", &path_str(&dir)), ("kind", "page")]))
                            {
                                self.open_page(&page);
                            }
                        }
                        self.say(crate::tl_id!("ui.created"));
                    }
                    keep = false;
                }
            }
            Dialog::MoveOrCopy { path, title, kind, dest } => {
                let places = self.move_destinations(path, *kind);
                let here = path.parent().map(Path::to_path_buf);
                let fits = |p: &Path| ergonote_store::tree::read_node(p).is_some_and(|n| n.kind.may_contain().contains(kind));
                let mut chosen: Option<bool> = None;
                egui::Window::new(crate::tl_id!("ui.move_or_copy"))
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .show(ctx, |ui| {
                        ui.label(crate::i18n::format_message_id("ui.move_or_copy_title_to", &[("title", title.as_str())]));
                        let now = places
                            .iter()
                            .find(|(p, _, _)| here.as_deref() == Some(p.as_path()))
                            .map(|(_, name, _)| name.clone())
                            .or_else(|| here.as_deref().and_then(ergonote_store::tree::read_node).map(|n| n.title))
                            .unwrap_or_default();
                        ui.label(
                            egui::RichText::new(crate::i18n::format_message_id("ui.it_is_in_place_now", &[("place", now.as_str())]))
                                .small()
                                .color(self.tokens.muted),
                        );
                        egui::ScrollArea::vertical().max_height(300.0).min_scrolled_height(160.0).show(ui, |ui| {
                            ui.set_min_width(320.0);
                            for (place, name, depth) in &places {
                                ui.horizontal(|ui| {
                                    ui.add_space(*depth as f32 * 16.0);
                                    if fits(place) {
                                        let picked = dest.as_deref() == Some(place.as_path());
                                        let shown = if here.as_deref() == Some(place.as_path()) { format!("{name} \u{2022}") } else { name.clone() };
                                        if ui.selectable_label(picked, shown).clicked() {
                                            *dest = Some(place.clone());
                                        }
                                    } else {
                                        ui.label(egui::RichText::new(name).color(self.tokens.muted));
                                    }
                                });
                            }
                        });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.cancel"), false).clicked() || ui.input(|i| i.key_pressed(Key::Escape))
                            {
                                keep = false;
                            }
                            let can = dest.is_some();
                            let moves = can && dest.as_deref() != here.as_deref();
                            if modal_button_enabled(ui, &self.tokens, crate::tl_id!("ui.copy"), false, can).clicked() {
                                chosen = Some(true);
                            }
                            if modal_button_enabled(ui, &self.tokens, crate::tl_id!("ui.move"), true, moves).clicked() {
                                chosen = Some(false);
                            }
                        });
                    });
                if let (Some(copy), Some(to)) = (chosen, dest.clone()) {
                    let from = path.clone();
                    self.move_or_copy(&from, &to, copy);
                    keep = false;
                }
            }
            Dialog::Link { block, cell, range, text, address, editing, first } => {
                let (mut go, mut remove) = (false, false);
                let usable = pageedit::linkable_address(address);
                egui::Window::new(crate::tl_id!("ui.link")).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(
                    ctx,
                    |ui| {
                        egui::Grid::new("link-dialog").num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
                            ui.label(crate::tl_id!("ui.text_to_display"));
                            let words = ui.add(egui::TextEdit::singleline(text).desired_width(320.0));
                            ui.end_row();
                            ui.label(crate::tl_id!("ui.address"));
                            let to = ui.add(egui::TextEdit::singleline(address).desired_width(320.0).hint_text("https://"));
                            ui.end_row();
                            // The address is what is missing first, unless the words are.
                            if *first {
                                if text.is_empty() && !*editing {
                                    words.request_focus()
                                } else {
                                    to.request_focus()
                                }
                                *first = false;
                            }
                            if (words.lost_focus() || to.lost_focus()) && ui.input(|i| i.key_pressed(Key::Enter)) && usable {
                                go = true;
                            }
                        });
                        if !address.trim().is_empty() && !usable {
                            ui.colored_label(self.tokens.warn, crate::tl_id!("ui.that_address_cannot_be_used_for_a_link"));
                        }
                        ui.horizontal(|ui| {
                            if *editing && modal_button(ui, &self.tokens, crate::tl_id!("ui.remove_link"), false).clicked() {
                                remove = true;
                            }
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.cancel"), false).clicked() || ui.input(|i| i.key_pressed(Key::Escape))
                            {
                                keep = false;
                            }
                            if modal_button_enabled(ui, &self.tokens, crate::tl_id!("ui.ok"), true, usable).clicked() {
                                go = true;
                            }
                        });
                    },
                );
                if go || remove {
                    let (block, cell, range) = (*block, *cell, *range);
                    // Removing keeps the words and lets go of the address.
                    let replacement = if remove { text.trim().to_string() } else { pageedit::link_markup(text, address) };
                    self.replace_in_caret_line(block, cell, range, &replacement);
                    keep = false;
                }
            }
            Dialog::Rename { path, title } => {
                let mut go = false;
                egui::Window::new(crate::tl_id!("ui.rename")).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(
                    ctx,
                    |ui| {
                        ui.label(crate::tl_id!("ui.title_the_folder_name_does_not_change"));
                        let r = ui.text_edit_singleline(title);
                        // Read Enter before asking for focus again: a new focus request hides the lost focus.
                        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        r.request_focus();
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.cancel"), false).clicked() {
                                keep = false;
                            }
                            if (modal_button(ui, &self.tokens, crate::tl_id!("ui.rename"), true).clicked() || enter) && !title.trim().is_empty() {
                                go = true;
                            }
                        });
                    },
                );
                if go {
                    if self.run("node.title", &params(&[("path", &path_str(path)), ("title", title.trim())])).is_some() {
                        if self.editor.as_ref().map(|e| e.dir == *path).unwrap_or(false) {
                            self.open_page(path);
                        }
                        self.say(crate::tl_id!("ui.renamed"));
                    }
                    keep = false;
                }
            }
            Dialog::InsertTable { rows, columns, first } => {
                let size = table_size(rows, columns);
                let mut go = false;
                let backdrop = modal_backdrop(ctx, "insert-table-backdrop");
                egui::Window::new(crate::tl_id!("ui.insert_table"))
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        stay_above(ui, backdrop);
                        modal_title(ui, &self.tokens, crate::tl_id!("ui.insert_table"));
                        egui::Grid::new("insert-table-dialog").num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
                            ui.label(crate::tl_id!("ui.number_of_columns"));
                            let c = ui.add(egui::TextEdit::singleline(columns).desired_width(80.0));
                            ui.end_row();
                            ui.label(crate::tl_id!("ui.number_of_rows"));
                            ui.add(egui::TextEdit::singleline(rows).desired_width(80.0));
                            ui.end_row();
                            if *first {
                                c.request_focus();
                                *first = false;
                            }
                        });
                        if size.is_none() {
                            let limits = crate::i18n::format_message_id(
                                "ui.table_size_limits",
                                &[("columns", &TABLE_MAX_COLUMNS.to_string()), ("rows", &TABLE_MAX_ROWS.to_string())],
                            );
                            ui.colored_label(self.tokens.warn, limits);
                        }
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.cancel"), false).clicked() {
                                keep = false;
                            }
                            if modal_button_enabled(ui, &self.tokens, crate::tl_id!("ui.ok"), true, size.is_some()).clicked() {
                                go = true;
                            }
                        });
                    });
                if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
                    keep = false;
                } else if ctx.input(|input| input.key_pressed(Key::Enter)) && size.is_some() {
                    go = true;
                }
                if go && let Some((r, c)) = size {
                    keep = false;
                    self.insert_table(r, c);
                }
            }
            Dialog::ParagraphSpacing { blocks, before, after, line, first } => {
                let values = [spacing_points(before), spacing_points(after), spacing_points(line)];
                let valid = values.iter().all(Result::is_ok);
                let mut go = false;
                let backdrop = modal_backdrop(ctx, "paragraph-spacing-backdrop");
                egui::Window::new(crate::tl_id!("ui.paragraph_spacing"))
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        stay_above(ui, backdrop);
                        modal_title(ui, &self.tokens, crate::tl_id!("ui.paragraph_spacing"));
                        egui::Grid::new("paragraph-spacing-dialog").num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
                            let mixed = crate::tl_id!("ui.spacing_mixed");
                            for (label, text) in [
                                (crate::tl_id!("ui.space_before_points"), &mut *before),
                                (crate::tl_id!("ui.space_after_points"), &mut *after),
                                (crate::tl_id!("ui.least_line_height_points"), &mut *line),
                            ] {
                                let name = ui.label(label);
                                let field = ui.add(egui::TextEdit::singleline(text).hint_text(mixed).desired_width(80.0)).labelled_by(name.id);
                                if *first {
                                    field.request_focus();
                                    *first = false;
                                }
                                ui.end_row();
                            }
                        });
                        if !valid {
                            let limits =
                                crate::i18n::format_message_id("ui.spacing_limits", &[("max", &ergonote_model::justify::MAX_POINTS.to_string())]);
                            ui.colored_label(self.tokens.warn, limits);
                        }
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.cancel"), false).clicked() {
                                keep = false;
                            }
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.spacing_reset"), false).clicked() {
                                for text in [&mut *before, &mut *after, &mut *line] {
                                    *text = "0".into();
                                }
                            }
                            if modal_button_enabled(ui, &self.tokens, crate::tl_id!("ui.ok"), true, valid).clicked() {
                                go = true;
                            }
                        });
                    });
                if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
                    keep = false;
                } else if ctx.input(|input| input.key_pressed(Key::Enter)) && valid {
                    go = true;
                }
                if go && let [Ok(b), Ok(a), Ok(l)] = values {
                    keep = false;
                    let blocks = blocks.clone();
                    self.apply_spacing(&blocks, [b, a, l]);
                }
            }
            Dialog::DeleteFile { page, name } => {
                let mut go = false;
                let mut cancel = false;
                let backdrop = modal_backdrop(ctx, "delete-file-modal-backdrop");
                egui::Window::new(crate::tl_id!("ui.delete_file"))
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        stay_above(ui, backdrop);
                        modal_title(ui, &self.tokens, crate::tl_id!("ui.delete_file"));
                        ui.set_min_width(420.0);
                        ui.label(crate::i18n::format_message_id("ui.delete_assets_name_from_this_page_s_folder", &[("name", name)]));
                        ui.add_space(6.0);
                        ui.colored_label(self.tokens.warn, crate::tl_id!("ui.the_file_is_removed_from_disk_undo_does_not_bring_a_file_back"));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.cancel"), false).clicked() {
                                keep = false;
                            }
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.delete"), true).clicked() {
                                go = true;
                            }
                        });
                    });
                if ctx.input(|input| input.key_pressed(Key::Escape)) {
                    cancel = true;
                } else if ctx.input(|input| input.key_pressed(Key::Enter)) {
                    go = true;
                }
                if cancel {
                    keep = false;
                }
                if go {
                    let page = page.clone();
                    if self.run("page.detach", &params(&[("path", &path_str(&page)), ("name", name)])).is_some() {
                        if let Some(ed) = self.editor.as_mut() {
                            ed.attachments = ergonote_store::assets::list(&page);
                        }
                        self.say(crate::tl_id!("ui.the_file_is_gone"));
                        // The page may have named the drawing it no longer has.
                        self.sync_ink_reference();
                    }
                    keep = false;
                }
            }
            Dialog::DeleteNotebook { path, title } => {
                let mut go = false;
                egui::Window::new(crate::tl_id!("ui.delete_notebook"))
                    .collapsible(false)
                    .resizable(false)
                    .title_bar(false)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        ui.set_min_width(440.0);
                        modal_title(ui, &self.tokens, crate::tl_id!("ui.delete_notebook"));
                        ui.label(crate::i18n::format_message_id("ui.delete_notebook_title_and_everything_inside_it", &[("title", title)]));
                        ui.add_space(4.0);
                        ui.colored_label(self.tokens.warn, crate::tl_id!("ui.this_removes_the_notebook_folder_from_disk_and_cannot_be_undone"));
                        ui.add_space(14.0);
                        ui.separator();
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.cancel"), false).clicked() {
                                keep = false;
                            }
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.delete_notebook"), true).clicked() {
                                go = true;
                            }
                        });
                    });
                if go {
                    let path = path.clone();
                    let was_open = self.editor.as_ref().and_then(|editor| notebook_for(&editor.dir)).is_some_and(|notebook| notebook == path);
                    log::info!("DeleteNotebook dialog: deleting {:?}", path);
                    match ergonote_store::ops::remove_tree(&path) {
                        Ok(()) => {
                            log::info!("notebook deleted successfully: {:?}", path);
                            self.remove_recent_notebook(&path);
                            if was_open {
                                self.editor = None;
                                self.pad = None;
                                self.selected = None;
                                self.backstage = Some("Open");
                            }
                            let _ = self.run("tree.refresh", &Params::new());
                            self.say(crate::i18n::format_message_id("ui.deleted_notebook_path", &[("path", &self.shown(&path))]));
                        }
                        Err(error) => {
                            log::error!("notebook delete failed: {:?}: {}", path, error);
                            let message =
                                crate::i18n::format_message_id("ui.the_notebook_could_not_be_deleted_error", &[("error", &error.to_string())]);
                            self.alert("delete-failed", crate::tl_id!("ui.delete_notebook"), &message);
                        }
                    }
                    keep = false;
                }
            }
            Dialog::Delete { path, title, count } => {
                let mut go = false;
                let is_notebook = ergonote_store::tree::read_node(path).is_some_and(|node| node.kind == NodeKind::Notebook);
                egui::Window::new(crate::tl_id!("ui.delete"))
                    .collapsible(false)
                    .resizable(false)
                    .title_bar(false)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        ui.set_min_width(420.0);
                        modal_title(ui, &self.tokens, crate::tl_id!("ui.delete_notebook"));
                        if is_notebook {
                            ui.label(crate::i18n::format_message_id("ui.delete_notebook_title_and_everything_inside_it", &[("title", title)]));
                            ui.colored_label(self.tokens.warn, crate::tl_id!("ui.this_removes_the_notebook_folder_from_disk_and_cannot_be_undone"));
                        } else {
                            ui.label(crate::i18n::format_count_message_id(
                                "ui.move_title_and_everything_inside_it_n_item_to_this_n_20c3146ea1",
                                *count as u64,
                                &[("title", title)],
                            ));
                            ui.label(crate::tl_id!("ui.you_can_restore_it_from_history"));
                        }
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.cancel"), false).clicked() {
                                keep = false;
                            }
                            if modal_button(
                                ui,
                                &self.tokens,
                                if is_notebook { crate::tl_id!("ui.delete_notebook") } else { crate::tl_id!("ui.move_to_recycle_bin") },
                                true,
                            )
                            .clicked()
                            {
                                go = true;
                            }
                        });
                    });
                if go {
                    if is_notebook {
                        let path = path.clone();
                        log::info!("deleting notebook: {:?}", path);
                        match ergonote_store::ops::remove_tree(&path) {
                            Ok(()) => {
                                log::info!("notebook deleted successfully: {:?}", path);
                                self.remove_recent_notebook(&path);
                                self.editor = None;
                                self.pad = None;
                                self.selected = None;
                                self.open.remove(&path);
                                let _ = self.run("tree.refresh", &Params::new());
                                self.say(crate::tl_id!("ui.deleted_notebook"));
                            }
                            Err(error) => {
                                log::error!("notebook delete failed: {:?}: {}", path, error);
                                let message =
                                    crate::i18n::format_message_id("ui.the_notebook_could_not_be_deleted_error", &[("error", &error.to_string())]);
                                self.alert("delete-failed", crate::tl_id!("ui.delete_notebook"), &message);
                            }
                        }
                    } else {
                        let was_open = self.editor.as_ref().map(|e| e.dir.starts_with(&*path)).unwrap_or(false);
                        log::info!("deleting node: {:?}", path);
                        match self.run("node.delete", &params(&[("path", &path_str(path))])) {
                            Some(_) => {
                                log::info!("node deleted successfully: {:?}", path);
                                if was_open {
                                    self.editor = None;
                                }
                                self.selected = None;
                                self.say(crate::tl_id!("ui.moved_to_the_notebook_recycle_bin"));
                            }
                            None => {
                                log::error!("node.delete command failed: {:?}", path);
                                self.alert(
                                    "delete-failed",
                                    crate::tl_id!("ui.delete"),
                                    crate::tl_id!("ui.this_could_not_be_moved_to_the_recycle_bin_nothing_was_deleted"),
                                );
                            }
                        }
                    }
                    keep = false;
                }
            }
            Dialog::DeleteTable { block } => {
                let block = *block;
                let mut go = false;
                egui::Window::new("Supprimer le tableau")
                    .collapsible(false)
                    .resizable(false)
                    .title_bar(false)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        ui.set_min_width(360.0);
                        modal_title(ui, &self.tokens, "Supprimer le tableau");
                        ui.label(crate::tl_id!("ui.student_delete_table_confirm"));
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.cancel"), false).clicked()
                                || ctx.input(|i| i.key_pressed(egui::Key::Escape))
                            {
                                keep = false;
                            }
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.delete"), true).clicked() {
                                go = true;
                            }
                        });
                    });
                if go {
                    self.delete_table_at(block);
                    keep = false;
                }
            }
            Dialog::RecentEdits { range, this_notebook, filter, items } => {
                let mut changed = false;
                let mut go: Option<PathBuf> = None;
                let t = self.tokens;
                let has_page = self.editor.is_some();
                egui::Window::new(crate::tl_id!("ui.recent_edits"))
                    .collapsible(false)
                    .resizable(true)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .show(ctx, |ui| {
                        ui.set_min_width(520.0);
                        ui.horizontal_wrapped(|ui| {
                            for r in RecentRange::ALL {
                                if ui.selectable_label(*range == r, r.label()).clicked() && *range != r {
                                    *range = r;
                                    changed = true;
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui.selectable_label(!*this_notebook, crate::tl_id!("ui.all_open_notebooks")).clicked() && *this_notebook {
                                *this_notebook = false;
                                changed = true;
                            }
                            if ui.add_enabled(has_page, egui::Button::selectable(*this_notebook, crate::tl_id!("ui.this_notebook"))).clicked()
                                && !*this_notebook
                            {
                                *this_notebook = true;
                                changed = true;
                            }
                        });
                        ui.horizontal(|ui| {
                            let name = ui.label(crate::tl_id!("ui.filter"));
                            ui.add(
                                egui::TextEdit::singleline(filter)
                                    .hint_text(crate::tl_id!("ui.filter_by_title_section_or_notebook"))
                                    .desired_width(f32::INFINITY),
                            )
                            .labelled_by(name.id);
                        });
                        ui.separator();
                        let needle = filter.trim().to_lowercase();
                        let shown: Vec<&ergonote_store::recent::Edited> = items
                            .iter()
                            .filter(|e| {
                                needle.is_empty()
                                    || e.title.to_lowercase().contains(&needle)
                                    || e.notebook.to_lowercase().contains(&needle)
                                    || e.section.iter().any(|s| s.to_lowercase().contains(&needle))
                            })
                            .collect();
                        if shown.is_empty() {
                            ui.label(egui::RichText::new(crate::tl_id!("ui.no_pages_were_changed_in_this_time")).color(t.muted));
                        }
                        egui::ScrollArea::vertical().max_height(380.0).auto_shrink([false, true]).show(ui, |ui| {
                            for e in shown {
                                ui.horizontal(|ui| {
                                    let title = if e.title.is_empty() { crate::tl_id!("ui.untitled_page").to_string() } else { e.title.clone() };
                                    if ui.link(title).clicked() {
                                        go = Some(e.page.clone());
                                    }
                                    let mut place = vec![e.notebook.clone()];
                                    place.extend(e.section.iter().cloned());
                                    ui.label(
                                        egui::RichText::new(place.join(&format!(" {} ", crate::tl_id!("ui.place_separator")))).small().color(t.muted),
                                    );
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        let when =
                                            ergonote_model::time::long_from_iso(&ergonote_model::time::iso_from_secs(e.at)).unwrap_or_default();
                                        let label = egui::RichText::new(when).small().color(t.muted);
                                        let resp = ui.label(label);
                                        if !e.stamped {
                                            resp.on_hover_text(crate::tl_id!(
                                                "ui.this_page_keeps_no_time_of_its_own_this_is_when_its_file_was_last_changed"
                                            ));
                                        }
                                    });
                                });
                            }
                        });
                        ui.add_space(8.0);
                        if modal_button(ui, &t, crate::tl_id!("ui.close"), false).clicked() {
                            keep = false;
                        }
                    });
                if changed {
                    *items = self.recent_edits(*range, *this_notebook);
                }
                if let Some(page) = go {
                    self.open_path(&path_str(&page));
                    keep = false;
                }
            }
            Dialog::PageVersions { page, versions, preview } => {
                let mut restore: Option<String> = None;
                let mut open: Option<(String, String)> = None;
                let mut back = false;
                let t = self.tokens;
                egui::Window::new(crate::tl_id!("ui.page_versions_2"))
                    .collapsible(false)
                    .resizable(true)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .show(ctx, |ui| {
                        ui.set_min_width(420.0);
                        if let Some(p) = preview.as_mut() {
                            // An older version, read-only (issue #278): said in the window
                            // itself, so it is never taken for the page.
                            ui.set_min_width(560.0);
                            ui.label(egui::RichText::new(crate::i18n::format_message_id("ui.version_saved_when", &[("when", &p.when)])).strong());
                            ui.colored_label(t.warn, crate::tl_id!("ui.an_older_version_read_only_the_page_is_not_changed_unless_you_restore_it"));
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                if ui.selectable_label(!p.comparing, crate::tl_id!("ui.this_version")).clicked() {
                                    p.comparing = false;
                                }
                                if ui.selectable_label(p.comparing, crate::tl_id!("ui.compare_with_the_page")).clicked() {
                                    p.comparing = true;
                                }
                            });
                            ui.separator();
                            egui::ScrollArea::vertical().max_height(420.0).auto_shrink([false, true]).show(ui, |ui| {
                                if p.comparing {
                                    let changed = p.changes.iter().any(|c| !matches!(c, crate::versions::Change::Same(_)));
                                    if !changed {
                                        ui.label(crate::tl_id!("ui.this_version_has_the_same_words_as_the_page"));
                                    }
                                    for change in &p.changes {
                                        let (mark, line, colour) = match change {
                                            crate::versions::Change::Same(l) => ("  ", l, t.muted),
                                            crate::versions::Change::OnlyThen(l) => ("− ", l, t.warn),
                                            crate::versions::Change::OnlyNow(l) => ("+ ", l, t.accent),
                                        };
                                        ui.label(egui::RichText::new(format!("{mark}{line}")).monospace().color(colour));
                                    }
                                    ui.add_space(4.0);
                                    ui.label(
                                        egui::RichText::new(crate::tl_id!(
                                            "ui.lines_marked_are_only_in_this_version_lines_marked_are_only_in_the_page"
                                        ))
                                        .small()
                                        .color(t.muted),
                                    );
                                } else {
                                    let _ = crate::preview::show(ui, &p.blocks, page, &t, None, &ergonote_model::justify::Justified::default());
                                }
                            });
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if modal_button(ui, &t, crate::tl_id!("ui.restore_this_version"), true).clicked() {
                                    restore = Some(p.id.clone());
                                }
                                if modal_button(ui, &t, crate::tl_id!("ui.back_to_the_list"), false).clicked() {
                                    back = true;
                                }
                                if modal_button(ui, &t, crate::tl_id!("ui.close"), false).clicked() {
                                    keep = false;
                                }
                            });
                            return;
                        }
                        ui.label(crate::tl_id!("ui.saved_copies_of_this_page"));
                        ui.add_space(6.0);
                        if versions.is_empty() {
                            ui.label(crate::tl_id!("ui.no_saved_versions_yet_ergonote_keeps_a_copy_when_th_aa2e25acad"));
                        } else {
                            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                                for version in versions.iter() {
                                    ui.horizontal(|ui| {
                                        ui.label(crate::versions::when(&version.created));
                                        let size = (version.bytes.div_ceil(1024)).to_string();
                                        let size = crate::i18n::format_message_id("ui.size_kb", &[("size", &size)]);
                                        ui.label(egui::RichText::new(size).small().color(t.muted));
                                        if modal_button(ui, &t, crate::tl_id!("ui.preview"), true).clicked() {
                                            open = Some((version.id.clone(), version.created.clone()));
                                        }
                                        if modal_button(ui, &t, crate::tl_id!("ui.restore"), true).clicked() {
                                            restore = Some(version.id.clone());
                                        }
                                    });
                                }
                            });
                        }
                        ui.add_space(8.0);
                        if modal_button(ui, &t, crate::tl_id!("ui.close"), false).clicked() {
                            keep = false;
                        }
                    });
                if back {
                    *preview = None;
                }
                if let Some((id, created)) = open {
                    // Compared with the page as it stands, unsaved words included.
                    let now = self.editor.as_ref().filter(|e| e.dir == *page).map(|e| e.body.clone());
                    let now = now.or_else(|| ergonote_store::page::load_page(page).ok().map(|p| p.body)).unwrap_or_default();
                    match crate::versions::open(page, &id, &created, &now) {
                        Ok(p) => *preview = Some(Box::new(p)),
                        Err(e) => self.complain(&crate::i18n::format_message_id("ui.cannot_open_that_version_error", &[("error", &e)])),
                    }
                }
                if let Some(id) = restore {
                    if self.run("page.version.restore", &params(&[("path", &path_str(page)), ("id", &id)])).is_some() {
                        self.open_page(page);
                        self.say(crate::tl_id!("ui.restored_the_saved_page_version"));
                    }
                    keep = false;
                }
            }
            Dialog::RecycleBin { notebook, items } => {
                let mut restore: Option<String> = None;
                let mut purge: Option<(String, String)> = None;
                egui::Window::new(crate::tl_id!("ui.notebook_recycle_bin_2"))
                    .collapsible(false)
                    .resizable(true)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .show(ctx, |ui| {
                        ui.set_min_width(460.0);
                        if items.is_empty() {
                            ui.label(crate::tl_id!("ui.this_notebook_s_recycle_bin_is_empty"));
                        } else {
                            egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                                for item in items.iter() {
                                    ui.horizontal(|ui| {
                                        ui.label(&item.title);
                                        ui.label(
                                            egui::RichText::new(crate::i18n::format_message_id(
                                                "ui.count_deleted",
                                                &[
                                                    ("count", &crate::i18n::format_count_id("ui.n_item_n_items", item.count as u64)),
                                                    ("deleted", &item.deleted),
                                                ],
                                            ))
                                            .small()
                                            .color(self.tokens.muted),
                                        );
                                        if modal_button(ui, &self.tokens, crate::tl_id!("ui.restore"), true).clicked() {
                                            restore = Some(item.id.clone());
                                        }
                                        if modal_button(ui, &self.tokens, crate::tl_id!("ui.delete_permanently"), true).clicked() {
                                            purge = Some((item.id.clone(), item.title.clone()));
                                        }
                                    });
                                }
                            });
                        }
                        if modal_button(ui, &self.tokens, crate::tl_id!("ui.close"), false).clicked() {
                            keep = false;
                        }
                    });
                if let Some(id) = restore {
                    if self.run("recycle.restore", &params(&[("notebook", &path_str(notebook)), ("id", &id)])).is_some() {
                        self.say(crate::tl_id!("ui.restored_to_the_original_location"));
                        keep = false;
                    }
                } else if let Some((id, title)) = purge {
                    next = Dialog::PurgeRecycleItem { notebook: notebook.clone(), id, title };
                }
            }
            Dialog::PurgeRecycleItem { notebook, id, title } => {
                let mut purge = false;
                egui::Window::new(crate::tl_id!("ui.permanently_delete"))
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .show(ctx, |ui| {
                        ui.label(crate::i18n::format_message_id("ui.permanently_delete_title_and_everything_inside_it", &[("title", title)]));
                        ui.colored_label(self.tokens.warn, crate::tl_id!("ui.this_cannot_be_undone"));
                        ui.horizontal(|ui| {
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.cancel"), false).clicked() {
                                keep = false;
                            }
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.delete_permanently"), true).clicked() {
                                purge = true;
                            }
                        });
                    });
                if purge && self.run("recycle.purge", &params(&[("notebook", &path_str(notebook)), ("id", id)])).is_some() {
                    self.say(crate::tl_id!("ui.permanently_deleted"));
                    keep = false;
                }
            }
            Dialog::Symbols => {
                let mut chosen: Option<&'static str> = None;
                // A modal like the other dialogs (issue #262): the page behind is dimmed and
                // takes no clicks, so the caret stays where the symbol will go, and Escape
                // closes it with nothing inserted.
                if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
                    keep = false;
                }
                let backdrop = modal_backdrop(ctx, "insert-symbol-backdrop");
                egui::Window::new(crate::tl_id!("ui.insert_symbol"))
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        stay_above(ui, backdrop);
                        modal_title(ui, &self.tokens, crate::tl_id!("ui.insert_symbol"));
                        ui.label(crate::tl_id!("ui.choose_a_symbol_to_insert_at_the_caret"));
                        ui.add_space(6.0);
                        egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                            let (year, month, day) = crate::seasons::today();
                            let mut categories = crate::seasons::seasonal_categories(year, month, day);
                            categories.extend(INSERT_SYMBOL_CATEGORIES.iter().copied());
                            for (category, symbols) in categories {
                                ui.label(egui::RichText::new(category).small().strong().color(self.tokens.muted));
                                ui.add_space(2.0);
                                for row in symbols.chunks(10) {
                                    ui.horizontal(|ui| {
                                        for symbol in row {
                                            let button = egui::Button::new(egui::RichText::new(*symbol).size(18.0).color(self.tokens.text))
                                                .min_size(vec2(38.0, 32.0))
                                                .fill(self.tokens.panel)
                                                .stroke(egui::Stroke::new(0.8, self.tokens.faint));
                                            if ui.add(button).clicked() {
                                                chosen = Some(symbol);
                                            }
                                        }
                                    });
                                }
                                ui.add_space(6.0);
                            }
                        });
                        ui.add_space(6.0);
                        if modal_button(ui, &self.tokens, crate::tl_id!("ui.close"), false).clicked() {
                            keep = false;
                        }
                    });
                if let Some(symbol) = chosen {
                    self.insert_in_page(symbol);
                    keep = false;
                }
            }
            Dialog::OcrResult { block, doc, text } => {
                let block = *block;
                let source_label = doc.source_label;
                let mut insert_below = false;
                let mut copy_now = false;
                let mut speak_now = false;
                if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
                    keep = false;
                }
                let box_w = (ctx.content_rect().width() - 48.0).clamp(260.0, 500.0);
                let backdrop = modal_backdrop(ctx, "ocr-result-backdrop");
                egui::Window::new("Texte reconnu (OCR)")
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(true)
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .frame(modal_frame(&self.tokens))
                    .show(ctx, |ui| {
                        stay_above(ui, backdrop);
                        ui.set_min_width(box_w);
                        ui.set_max_width(box_w);
                        modal_title(ui, &self.tokens, "Texte extrait de la page");
                        ui.label(egui::RichText::new(format!("Source : {source_label}")).small().color(self.tokens.accent));
                        ui.label(crate::tl_id!("ui.student_ocr_edit_hint"));
                        ui.add_space(4.0);
                        egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                            ui.add_sized([box_w, 200.0], egui::TextEdit::multiline(text).desired_width(box_w));
                        });
                        ui.add_space(6.0);
                        ui.horizontal_wrapped(|ui| {
                            if modal_button(ui, &self.tokens, "Insérer en dessous", true).clicked() {
                                insert_below = true;
                            }
                            if modal_button(ui, &self.tokens, "Copier", false).clicked() {
                                copy_now = true;
                            }
                            if modal_button(ui, &self.tokens, "Lire à voix haute", false).clicked() {
                                speak_now = true;
                            }
                            if modal_button(ui, &self.tokens, crate::tl_id!("ui.close"), false).clicked() {
                                keep = false;
                            }
                        });
                    });
                let snapshot = text.clone();
                if copy_now {
                    self.pending_copy = Some(snapshot.clone());
                    self.say("Texte copié.");
                }
                if speak_now {
                    self.speak_text_with_speed(&snapshot);
                }
                if insert_below {
                    self.insert_ocr_text_below(block, &snapshot);
                    keep = false;
                }
            }
        }
        if keep {
            self.dialog = Some(next);
        }
    }
}
