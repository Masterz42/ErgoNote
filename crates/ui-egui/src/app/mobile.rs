use super::*;

impl ErgoNoteApp {
    pub(super) fn find_tree_node<'a>(nodes: &'a [Node], target: &Path) -> Option<&'a Node> {
        for n in nodes {
            if n.path == target {
                return Some(n);
            }
            if let Some(found) = Self::find_tree_node(&n.children, target) {
                return Some(found);
            }
        }
        None
    }

    pub(super) fn find_parent_node_path(nodes: &[Node], target: &Path) -> Option<PathBuf> {
        for n in nodes {
            if n.children.iter().any(|c| c.path == target) {
                return Some(n.path.clone());
            }
            if let Some(found) = Self::find_parent_node_path(&n.children, target) {
                return Some(found);
            }
        }
        None
    }

    pub(super) fn mobile_can_go_up(&self) -> bool {
        let in_page = !self.compact_list_fills() && self.editor.is_some() && self.backstage.is_none();
        in_page || self.mobile_browse_dir.is_some()
    }

    /// Moves one level up in smartphone portrait navigation:
    /// open page -> its parent section tiles -> parent notebook/root tiles.
    pub(super) fn mobile_go_up(&mut self) -> bool {
        let in_page = !self.compact_list_fills() && self.editor.is_some() && self.backstage.is_none();
        if in_page {
            if let Some(page_path) = self.editor.as_ref().map(|e| e.dir.clone())
                && let Some(parent) = Self::find_parent_node_path(self.session.tree(), &page_path)
            {
                self.mobile_browse_dir = Some(parent);
            }
            self.full_page = false;
            self.compact_full_page = false;
            self.save_in_background();
            return true;
        }
        if let Some(dir) = self.mobile_browse_dir.clone() {
            let parent = Self::find_parent_node_path(self.session.tree(), &dir);
            let single_root =
                self.session.tree().len() == 1 && self.session.tree().first().is_some_and(|nb| parent.as_deref() == Some(nb.path.as_path()));
            self.mobile_browse_dir = if single_root { None } else { parent };
            return true;
        }
        false
    }

    /// Streamlined smartphone portrait top bar: Back arrow on the left, compact icon-only
    /// Coffee, Help (?), and Theme (sun/moon) on the right, with zero text overlap and no version label.
    pub(super) fn mobile_title_band(&mut self, ui: &mut Ui) {
        let on = self.tokens.on_band();
        let in_page = !self.compact_list_fills() && self.editor.is_some() && self.backstage.is_none();
        let can_go_up = self.mobile_can_go_up();
        let mut go_up = false;
        let mut exit_simple_mode = false;
        let show_exit_simple = self.simple_mode;
        let heading = if in_page {
            self.current_section(self.session.tree())
                .map(|s| s.info.title.clone())
                .or_else(|| self.editor.as_ref().map(|e| e.title.clone()))
                .unwrap_or_else(|| "ErgoNote".to_string())
        } else if let Some(dir) = self.mobile_browse_dir.as_deref() {
            Self::find_tree_node(self.session.tree(), dir).map(|n| n.info.title.clone()).unwrap_or_else(|| "ErgoNote".to_string())
        } else {
            "ErgoNote".to_string()
        };
        let band = egui::Panel::top("band")
            .frame(egui::Frame::new().fill(self.tokens.band).inner_margin(egui::Margin::symmetric(8, 6)))
            .show_separator_line(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
                    let total_w = ui.available_width();
                    if can_go_up {
                        let back = egui::Button::new(egui::RichText::new("←").size(18.0).strong().color(on.text))
                            .fill(on.panel)
                            .stroke(egui::Stroke::new(1.0, on.container_border))
                            .corner_radius(8.0)
                            .min_size(vec2(36.0, 32.0));
                        if ui.add(back).on_hover_text("Remonter dans le dossier").clicked() {
                            go_up = true;
                        }
                    } else {
                        ui.add(egui::Image::from_bytes("bytes://ergonote-logo", crate::LOGO_PNG).fit_to_exact_size(vec2(24.0, 24.0)));
                    }
                    let right_reserve = if show_exit_simple { 244.0 } else { 206.0 };
                    let title_max_w = (total_w - right_reserve).max(40.0);
                    ui.add_sized(
                        vec2(title_max_w, 32.0),
                        egui::Label::new(egui::RichText::new(&heading).size(15.5).strong().color(on.text)).truncate(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
                        if show_exit_simple
                            && Self::mobile_icon_button(
                                ui,
                                Icon::ExitFullPage,
                                on.text,
                                on.panel,
                                on.container_border,
                                "Quitter le mode simple (revenir à l'interface complète)",
                            )
                        {
                            exit_simple_mode = true;
                        }
                        self.theme_menu(ui, on);
                        if Self::mobile_floppy_button(ui, on.text, on.panel, on.container_border, "Sauvegarder / Récupérer tout le cartable (.zip)")
                        {
                            self.dialog = Some(Dialog::BackupRecovery { feedback: None });
                        }
                        if Self::mobile_icon_button(ui, Icon::Help, on.text, on.panel, on.container_border, "Signaler un bug ou donner un avis") {
                            self.dialog = Some(Dialog::ReportBug);
                        }
                        let warm = Color32::from_rgb(180, 95, 20);
                        let warm_bg = Color32::from_rgb(255, 244, 214);
                        if Self::mobile_icon_button(
                            ui,
                            Icon::Coffee,
                            warm,
                            warm_bg,
                            crate::theme::mix(warm, on.container_border, 0.35),
                            "Offrir un café pour soutenir ErgoNote (Ko-fi)",
                        ) {
                            self.dialog = Some(Dialog::SupportCreator);
                        }
                    });
                });
            });
        self.band_bottom = band.response.rect.max.y;
        if go_up {
            self.mobile_go_up();
        }
        if exit_simple_mode {
            self.ribbon_act(ribbon::Act::ToggleSimpleMode);
        }
        self.update_banner(ui);
    }

    pub(super) fn mobile_icon_button(ui: &mut Ui, icon: Icon, fg: Color32, bg: Color32, border: Color32, tip: &str) -> bool {
        let (rect, resp) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::click());
        let fill = if resp.hovered() { crate::theme::mix(bg, fg, 0.14) } else { bg };
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 8.0, fill);
        painter.rect_stroke(rect, 8.0, egui::Stroke::new(1.0, border), egui::StrokeKind::Inside);
        icons::paint(&painter, egui::Rect::from_center_size(rect.center(), vec2(17.0, 17.0)), icon, fg);
        resp.on_hover_text(tip).clicked()
    }

    /// The floppy disk of the top bar: save or recover the whole notebook folder, the same
    /// action as the floppy in the desktop title row.
    pub(super) fn mobile_floppy_button(ui: &mut Ui, fg: Color32, bg: Color32, border: Color32, tip: &str) -> bool {
        let (rect, resp) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::click());
        let fill = if resp.hovered() { crate::theme::mix(bg, fg, 0.14) } else { bg };
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 8.0, fill);
        painter.rect_stroke(rect, 8.0, egui::Stroke::new(1.0, border), egui::StrokeKind::Inside);
        Self::paint_floppy(&painter, egui::Rect::from_center_size(rect.center(), vec2(17.0, 17.0)), fg, bg);
        let enabled = ui.is_enabled();
        resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, tip));
        resp.on_hover_text(tip).clicked()
    }

    /// Simplified single-row top ribbon for smartphone mode (`< 600 pt`): keeps only essential
    /// formatting, list, native image insertion, drawing toggle and TND comfort options.
    pub(super) fn mobile_simple_ribbon(&mut self, ui: &mut Ui) {
        if self.compact_list_fills() || self.editor.is_none() || self.backstage.is_some() || self.dialog.is_some() || self.show_source {
            return;
        }
        let t = self.tokens;
        let style = self.caret_style(ui.ctx());
        let font_sel = self.font_selection(ui.ctx());
        let active_nib = self.rack.get(self.nib).copied().unwrap_or_else(|| ergonote_ink::rack::new_nib(ergonote_ink::Tool::Pen));
        let mut acts: Vec<ribbon::Act> = Vec::new();
        let mut take_photo = false;
        let pill_rich = |ui: &mut Ui, label: egui::RichText, active: bool, custom_fill: Option<(Color32, Color32)>| -> egui::Response {
            let (fill, text_col) = match (active, custom_fill) {
                (_, Some((bg, fg))) => (bg, fg),
                (true, None) => (t.row_selected, t.text),
                (false, None) => (t.editor, t.text),
            };
            let stroke_width = if active && custom_fill.is_some() { 2.0 } else { 1.0 };
            let stroke = egui::Stroke::new(stroke_width, if active { t.accent } else { t.container_border });
            let btn =
                egui::Button::new(label.size(13.5).strong().color(text_col)).fill(fill).stroke(stroke).corner_radius(8.0).min_size(vec2(0.0, 30.0));
            ui.add(btn)
        };
        let pill = |ui: &mut Ui, label: &str, active: bool| -> egui::Response { pill_rich(ui, egui::RichText::new(label), active, None) };
        let dark = t.editor.r() < 128;
        // The formatting marks as the desktop ribbon draws them, rather than a letter or an
        // abbreviation: a B, a slanted I, a U with its bar and a highlighter, which a phone
        // shows clearly at this size and a child recognises.
        let mark_pill = |ui: &mut Ui, mark: crate::ribbon_icons::Mark, label: &str, active: bool| -> egui::Response {
            let (rect, resp) = ui.allocate_exact_size(vec2(36.0, 32.0), Sense::click());
            let fill = if active {
                t.row_selected
            } else if resp.hovered() {
                t.row_hover
            } else {
                t.editor
            };
            let border = if active { t.accent } else { t.container_border };
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 9.0, fill);
            painter.rect_stroke(rect, 9.0, egui::Stroke::new(if active { 1.8 } else { 1.0 }, border), egui::StrokeKind::Inside);
            crate::ribbon_icons::paint(&painter, egui::Rect::from_center_size(rect.center(), vec2(19.0, 19.0)), mark, mark.color(t.text, t.accent));
            let spoken = label.to_string();
            let enabled = ui.is_enabled();
            resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, active, spoken.clone()));
            resp.on_hover_text(label)
        };
        // A capital A and a small a: the text style in use, as Office shows a style.
        let title_pill = |ui: &mut Ui, active: bool| -> egui::Response {
            let (rect, resp) = ui.allocate_exact_size(vec2(40.0, 32.0), Sense::click());
            let fill = if active {
                t.row_selected
            } else if resp.hovered() {
                t.row_hover
            } else {
                t.editor
            };
            let border = if active { t.accent } else { t.container_border };
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 9.0, fill);
            painter.rect_stroke(rect, 9.0, egui::Stroke::new(if active { 1.8 } else { 1.0 }, border), egui::StrokeKind::Inside);
            let col = if active { t.accent } else { t.text };
            painter.text(
                egui::pos2(rect.center().x - 5.0, rect.center().y + 1.0),
                egui::Align2::CENTER_CENTER,
                "A",
                egui::FontId::new(17.0, crate::fonts::bold_family()),
                col,
            );
            painter.text(
                egui::pos2(rect.center().x + 8.0, rect.center().y + 4.0),
                egui::Align2::CENTER_CENTER,
                "a",
                egui::FontId::proportional(11.0),
                col,
            );
            let enabled = ui.is_enabled();
            resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, active, "Titre".to_string()));
            resp.on_hover_text("Titre (H1)")
        };

        let icon_pill = |ui: &mut Ui, icon: Icon, active: bool, tint: Option<(Color32, Color32)>, use_color_svg: bool, tip: &str| -> egui::Response {
            let (bg, fg) = match (active, tint) {
                (true, _) => (t.accent, t.on_band().text),
                (false, Some((b, f))) => (b, f),
                (false, None) => (t.editor, t.text),
            };
            let (rect, resp) = ui.allocate_exact_size(vec2(36.0, 32.0), Sense::click());
            let fill = if resp.hovered() && !active { crate::theme::mix(bg, fg, 0.12) } else { bg };
            let border = if active { t.accent } else { t.container_border };
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 9.0, fill);
            painter.rect_stroke(rect, 9.0, egui::Stroke::new(if active { 1.8 } else { 1.0 }, border), egui::StrokeKind::Inside);
            let ico_rect = egui::Rect::from_center_size(rect.center(), vec2(19.0, 19.0));
            if use_color_svg && !active {
                icons::paint_ribbon(ui, ico_rect, icon, fg);
            } else {
                icons::paint(&painter, ico_rect, icon, fg);
            }
            resp.on_hover_text(tip)
        };
        let img_tint = if dark {
            (Color32::from_rgb(24, 54, 68), Color32::from_rgb(125, 211, 252))
        } else {
            (Color32::from_rgb(224, 242, 254), Color32::from_rgb(3, 105, 161))
        };
        let cam_tint = if dark {
            (Color32::from_rgb(46, 38, 74), Color32::from_rgb(196, 181, 253))
        } else {
            (Color32::from_rgb(237, 233, 254), Color32::from_rgb(109, 40, 217))
        };
        let pen_tint = if dark {
            (Color32::from_rgb(68, 42, 24), Color32::from_rgb(253, 186, 116))
        } else {
            (Color32::from_rgb(255, 237, 213), Color32::from_rgb(194, 65, 12))
        };

        egui::Panel::top("mobile-ribbon")
            .frame(egui::Frame::new().fill(t.panel).inner_margin(egui::Margin::symmetric(8, 5)).stroke(egui::Stroke::new(1.0, t.container_border)))
            .show_separator_line(false)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                    if mark_pill(ui, crate::ribbon_icons::Mark::Bold, "Gras", font_sel.marks.bold).clicked() {
                        acts.push(ribbon::Act::Mark("bold"));
                    }
                    if mark_pill(ui, crate::ribbon_icons::Mark::Italic, "Italique", font_sel.marks.italic).clicked() {
                        acts.push(ribbon::Act::Mark("italic"));
                    }
                    if mark_pill(ui, crate::ribbon_icons::Mark::Underline, "Souligné", font_sel.marks.underline).clicked() {
                        acts.push(ribbon::Act::Mark("underline"));
                    }
                    let hl_active = font_sel.marks.highlight;
                    if mark_pill(ui, crate::ribbon_icons::Mark::Highlighter, "Surligné", hl_active).clicked() {
                        let next = if hl_active { None } else { Some(NodeColor::Yellow) };
                        acts.push(ribbon::Act::HighlightColor(next));
                    }
                    if title_pill(ui, style == "h1").clicked() {
                        acts.push(ribbon::Act::Block(if style == "h1" { "p" } else { "h1" }));
                    }
                    if icon_pill(ui, Icon::ListBullet, style == "bullet", None, false, "Liste à puces").clicked() {
                        acts.push(ribbon::Act::Block(if style == "bullet" { "p" } else { "bullet" }));
                    }
                    if icon_pill(ui, Icon::Picture, false, Some(img_tint), true, "Insérer une image").clicked() {
                        acts.push(ribbon::Act::AddPicture);
                    }
                    if cfg!(target_os = "android") && icon_pill(ui, Icon::Camera, false, Some(cam_tint), false, "Prendre une photo").clicked() {
                        take_photo = true;
                    }
                    let drawing = self.pointer != ribbon::Pointer::Write;
                    if icon_pill(ui, Icon::Pen, drawing, Some(pen_tint), false, "Dessiner au stylo").clicked() {
                        let next = if drawing { ribbon::Pointer::Write } else { ribbon::Pointer::Draw };
                        acts.push(ribbon::Act::InkPointer(next));
                    }
                });
            });
        if take_photo {
            self.request_android_camera();
        }

        if self.pointer != ribbon::Pointer::Write {
            egui::Panel::top("mobile-draw-bar")
                .frame(
                    egui::Frame::new().fill(t.panel).inner_margin(egui::Margin::symmetric(8, 3)).stroke(egui::Stroke::new(1.0, t.container_border)),
                )
                .show_separator_line(false)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                        if pill(ui, "Écrire", false).clicked() {
                            acts.push(ribbon::Act::InkPointer(ribbon::Pointer::Write));
                        }
                        if pill(ui, "Stylo", self.pointer == ribbon::Pointer::Draw).clicked() {
                            acts.push(ribbon::Act::InkPointer(ribbon::Pointer::Draw));
                        }
                        if pill(ui, "Gomme", self.pointer == ribbon::Pointer::Erase).clicked() {
                            acts.push(ribbon::Act::InkPointer(ribbon::Pointer::Erase));
                        }
                        for rgb in [
                            ergonote_ink::Rgb(28, 30, 36),
                            ergonote_ink::Rgb(32, 96, 210),
                            ergonote_ink::Rgb(205, 48, 48),
                            ergonote_ink::Rgb(34, 139, 76),
                        ] {
                            let (rect, resp) = ui.allocate_exact_size(vec2(28.0, 30.0), Sense::click());
                            let chosen = active_nib.colour == rgb && self.pointer == ribbon::Pointer::Draw;
                            let c = Color32::from_rgb(rgb.0, rgb.1, rgb.2);
                            let painter = ui.painter_at(rect);
                            if chosen {
                                painter.circle_stroke(rect.center(), 12.0, egui::Stroke::new(2.0, t.accent));
                            }
                            painter.circle_filled(rect.center(), 9.0, c);
                            if resp.clicked() {
                                acts.push(ribbon::Act::InkRack(ribbon::RackEdit::Set(
                                    self.nib,
                                    ergonote_ink::rack::Nib::new(active_nib.tool, rgb, active_nib.tenths),
                                )));
                                acts.push(ribbon::Act::InkPointer(ribbon::Pointer::Draw));
                            }
                        }
                        let (u_rect, u_resp) = ui.allocate_exact_size(vec2(32.0, 30.0), Sense::click());
                        let u_painter = ui.painter_at(u_rect);
                        u_painter.rect_filled(u_rect, 8.0, t.editor);
                        u_painter.rect_stroke(u_rect, 8.0, egui::Stroke::new(1.0, t.container_border), egui::StrokeKind::Inside);
                        icons::paint(&u_painter, egui::Rect::from_center_size(u_rect.center(), vec2(16.0, 16.0)), Icon::Undo, t.text);
                        if u_resp.clicked() {
                            acts.push(ribbon::Act::InkUndo);
                        }
                    });
                });
        } else if self.caret_in_table() {
            egui::Panel::top("mobile-table-bar")
                .frame(
                    egui::Frame::new().fill(t.panel).inner_margin(egui::Margin::symmetric(8, 3)).stroke(egui::Stroke::new(1.0, t.container_border)),
                )
                .show_separator_line(false)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                        if pill(ui, "+ Ligne \u{2191}", false).clicked() {
                            acts.push(ribbon::Act::Table(crate::pageedit::TableEdit::InsertRow(true)));
                        }
                        if pill(ui, "+ Ligne \u{2193}", false).clicked() {
                            acts.push(ribbon::Act::Table(crate::pageedit::TableEdit::InsertRow(false)));
                        }
                        if pill(ui, "\u{2212} Ligne", false).clicked() {
                            acts.push(ribbon::Act::Table(crate::pageedit::TableEdit::DeleteRow));
                        }
                        if pill(ui, "+ Col.", false).clicked() {
                            acts.push(ribbon::Act::Table(crate::pageedit::TableEdit::InsertColumn(false)));
                        }
                        if pill(ui, "\u{2212} Col.", false).clicked() {
                            acts.push(ribbon::Act::Table(crate::pageedit::TableEdit::DeleteColumn));
                        }
                    });
                });
        }

        let picked = self.editor.as_ref().and_then(|e| crate::containers::selected(ui.ctx(), &e.dir));
        for a in acts {
            if let Some(i) = picked
                && self.act_on_box(ui.ctx(), i, &a)
            {
                continue;
            }
            self.ribbon_act(a);
        }
    }

    /// Windows-style colored tile browser for smartphone portrait mode.
    pub(super) fn mobile_tiles_view(&mut self, ui: &mut Ui, acts: &mut Vec<Act>) {
        // If the stored browse path was deleted or moved, fall back cleanly to the root tiles.
        if let Some(dir) = self.mobile_browse_dir.as_deref()
            && Self::find_tree_node(self.session.tree(), dir).is_none()
        {
            self.mobile_browse_dir = None;
        }
        let tree = self.session.tree();
        let active_dir = self.mobile_browse_dir.clone();
        let current_node = active_dir.as_deref().and_then(|dir| Self::find_tree_node(tree, dir));
        struct MobileTile {
            path: PathBuf,
            title: String,
            kind: NodeKind,
            color: Option<NodeColor>,
            children_len: usize,
        }
        let to_tiles = |nodes: &[Node]| -> Vec<MobileTile> {
            nodes
                .iter()
                .map(|n| MobileTile {
                    path: n.path.clone(),
                    title: n.info.title.clone(),
                    kind: n.info.kind,
                    color: n.info.color,
                    children_len: n.children.len(),
                })
                .collect()
        };
        let (items, parent_color, current_section_path): (Vec<MobileTile>, Option<NodeColor>, Option<PathBuf>) = match current_node {
            Some(node) => {
                let sec = (node.info.kind == NodeKind::Section).then(|| node.path.clone());
                (to_tiles(&node.children), node.info.color, sec)
            }
            None => {
                if tree.len() == 1 {
                    let nb = &tree[0];
                    (to_tiles(&nb.children), nb.info.color, None)
                } else {
                    (to_tiles(tree), None, None)
                }
            }
        };

        let tokens = self.tokens;
        let dark = tokens.editor.r() < 128;
        let palette = [
            Color32::from_rgb(31, 122, 201),
            Color32::from_rgb(13, 148, 136),
            Color32::from_rgb(217, 119, 6),
            Color32::from_rgb(124, 58, 237),
            Color32::from_rgb(22, 163, 74),
            Color32::from_rgb(220, 38, 38),
        ];

        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
            let on_band = tokens.on_band();
            if let Some(sec) = current_section_path.clone() {
                let btn = egui::Button::new(egui::RichText::new("+ Nouvelle page").size(14.0).strong().color(on_band.text))
                    .fill(tokens.accent)
                    .corner_radius(10.0)
                    .min_size(vec2(0.0, 36.0));
                if ui.add(btn).clicked() {
                    acts.push(Act::NewPage(sec));
                }
            } else {
                self.new_section_button(ui, acts);
            }
        });
        ui.add_space(8.0);

        let mut open_folder: Option<PathBuf> = None;
        let mut open_page: Option<(PathBuf, Option<PathBuf>)> = None;
        let scroll = egui::containers::scroll_area::ScrollSource {
            drag: egui::containers::scroll_area::DragScroll::Always,
            ..egui::containers::scroll_area::ScrollSource::default()
        };
        egui::ScrollArea::vertical().id_salt("mobile-tiles-scroll").auto_shrink([false, false]).scroll_source(scroll).show(ui, |ui| {
            if items.is_empty() {
                ui.add_space(16.0);
                ui.label(egui::RichText::new("Ce dossier est vide.").size(14.5).color(tokens.muted));
                return;
            }
            let gap = 10.0;
            let avail_w = (ui.available_width() - 4.0).max(160.0);
            let cols = if avail_w >= 760.0 {
                4usize
            } else if avail_w >= 520.0 {
                3usize
            } else {
                2usize
            };
            let tile_w = ((avail_w - gap * (cols as f32 - 1.0)) / cols as f32).max(120.0);
            for (row_idx, chunk) in items.chunks(cols).enumerate() {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(gap, gap);
                    for (col_idx, node) in chunk.iter().enumerate() {
                        let idx = row_idx * cols + col_idx;
                        let is_page = node.kind == NodeKind::Page;
                        let tile_h = if is_page { 86.0 } else { 104.0 };
                        let (rect, resp) = ui.allocate_exact_size(vec2(tile_w, tile_h), Sense::click());
                        let painter = ui.painter_at(rect);
                        let accent = node
                            .color
                            .or(parent_color)
                            .map(|c| {
                                let (r, g, b) = c.rgb(dark);
                                Color32::from_rgb(r, g, b)
                            })
                            .unwrap_or(palette[idx % palette.len()]);

                        if is_page {
                            let bg = if resp.hovered() { crate::theme::mix(tokens.editor, accent, 0.14) } else { tokens.editor };
                            painter.rect_filled(rect, 12.0, bg);
                            painter.rect_stroke(rect, 12.0, egui::Stroke::new(1.0, tokens.container_border), egui::StrokeKind::Inside);
                            let strip = egui::Rect::from_min_max(rect.min, egui::pos2(rect.min.x + 7.0, rect.max.y));
                            painter.rect_filled(strip, 12.0, accent);
                            let galley = painter.layout(node.title.clone(), egui::FontId::proportional(15.0), tokens.text, (tile_w - 24.0).max(60.0));
                            painter.galley(egui::pos2(rect.min.x + 15.0, rect.min.y + 14.0), galley, tokens.text);
                        } else {
                            let bg = if resp.hovered() { crate::theme::mix(accent, Color32::WHITE, 0.12) } else { accent };
                            painter.rect_filled(rect, 12.0, bg);
                            let fg = Color32::WHITE;
                            let galley = painter.layout(node.title.clone(), egui::FontId::proportional(16.0), fg, (tile_w - 22.0).max(60.0));
                            painter.galley(egui::pos2(rect.min.x + 12.0, rect.min.y + 14.0), galley, fg);
                            let count = node.children_len;
                            let sub = match node.kind {
                                NodeKind::Section => {
                                    if count <= 1 {
                                        format!("{count} page")
                                    } else {
                                        format!("{count} pages")
                                    }
                                }
                                _ => {
                                    if count <= 1 {
                                        format!("{count} dossier")
                                    } else {
                                        format!("{count} dossiers")
                                    }
                                }
                            };
                            painter.text(
                                egui::pos2(rect.min.x + 12.0, rect.max.y - 14.0),
                                Align2::LEFT_BOTTOM,
                                sub,
                                egui::FontId::proportional(12.5),
                                Color32::from_rgba_unmultiplied(255, 255, 255, 215),
                            );
                        }

                        if resp.clicked() {
                            if is_page {
                                open_page = Some((node.path.clone(), current_section_path.clone()));
                            } else {
                                open_folder = Some(node.path.clone());
                            }
                        }
                    }
                });
                ui.add_space(gap);
            }
        });

        if let Some(folder) = open_folder {
            self.mobile_browse_dir = Some(folder);
        }
        if let Some((page, sec)) = open_page {
            if let Some(sec) = sec {
                self.mobile_browse_dir = Some(sec);
            }
            acts.push(Act::Select(page));
        }
    }
}
