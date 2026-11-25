use super::*;

impl ErgoNoteApp {
    pub(super) fn templates_panel(&mut self, ui: &mut Ui) {
        let Some(pane) = self.templates_pane.as_mut() else { return };
        let tokens = self.tokens;
        let has_page = self.editor.is_some();
        let mut close = false;
        let mut chosen: Option<(String, String)> = None;
        let mut keep: Option<String> = None;
        // A phone-width window gives the pane the whole width, as it does the Tags Summary.
        let panel = if self.compact {
            egui::Panel::right("page-templates-compact").resizable(false).exact_size(ui.available_width())
        } else {
            egui::Panel::right("page-templates").resizable(true).default_size(280.0).size_range(220.0..=520.0)
        };
        panel.show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.strong(crate::tl_id!("ui.page_templates_title"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // A cross drawn with lines: the interface fonts have no glyph for one.
                    let (rect, x) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                    x.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, crate::tl_id!("ui.close_the_page_templates")));
                    if x.hovered() {
                        ui.painter().rect_filled(rect, 3.0, tokens.row_hover);
                    }
                    let c = rect.shrink(7.0);
                    let stroke = egui::Stroke::new(1.4, tokens.text);
                    ui.painter().line_segment([c.left_top(), c.right_bottom()], stroke);
                    ui.painter().line_segment([c.right_top(), c.left_bottom()], stroke);
                    if x.on_hover_text(crate::tl_id!("ui.close_the_page_templates")).clicked() {
                        close = true;
                    }
                });
            });
            ui.label(egui::RichText::new(crate::tl_id!("ui.page_templates_hint")).small().color(tokens.muted));
            ui.separator();
            let list_height = (ui.available_height() - 110.0).max(120.0);
            egui::ScrollArea::vertical().id_salt("page-templates-list").max_height(list_height).auto_shrink([false, true]).show(ui, |ui| {
                for category in ergonote_store::page_templates::Category::ALL {
                    let mut items = pane.templates.iter().filter(|t| t.category == category).peekable();
                    let mine = category == ergonote_store::page_templates::Category::Mine;
                    if items.peek().is_none() && !mine {
                        continue;
                    }
                    ui.add_space(4.0);
                    ui.strong(crate::tl!(category.label()));
                    if items.peek().is_none() {
                        ui.weak(crate::tl_id!("ui.no_saved_templates"));
                    }
                    for template in items {
                        let label = if mine { template.name.clone() } else { crate::tl!(&template.name).to_string() };
                        let width = ui.available_width().max(60.0);
                        let (rect, row) = ui.allocate_exact_size(vec2(width, 24.0), Sense::click());
                        row.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone()));
                        if row.hovered() {
                            ui.painter().rect_filled(rect, 3.0, tokens.row_hover);
                        }
                        ui.painter().text(
                            rect.left_center() + vec2(10.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            &label,
                            egui::FontId::proportional(13.5),
                            tokens.text,
                        );
                        if row.on_hover_text(crate::tl_id!("ui.add_a_page_from_this_template")).clicked() {
                            chosen = Some((template.id.clone(), label));
                        }
                    }
                }
            });
            ui.separator();
            ui.label(crate::tl_id!("ui.save_current_page_as_a_template"));
            ui.add_enabled_ui(has_page, |ui| {
                let field = egui::TextEdit::singleline(&mut pane.name).hint_text(crate::tl_id!("ui.template_name")).desired_width(f32::INFINITY);
                let field = ui.add(field);
                // Named for a screen reader: the grey hint inside the box is not a name.
                field.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextInput, has_page, crate::tl_id!("ui.template_name")));
                let entered = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                let named = !pane.name.trim().is_empty();
                let clicked = ui.add_enabled(named, egui::Button::new(crate::tl_id!("ui.save_as_template"))).clicked();
                if named && (entered || clicked) {
                    keep = Some(pane.name.trim().to_string());
                }
            });
        });
        if close {
            self.templates_pane = None;
            return;
        }
        if let Some((id, name)) = chosen {
            self.page_from_template(&id, &name);
        }
        if let Some(name) = keep {
            self.save_page_as_template(&name);
        }
    }

    pub(super) fn tags_panel(&mut self, ui: &mut Ui) {
        let Some(pane) = self.tags_pane.as_mut() else { return };
        let tokens = self.tokens;
        let mut close = false;
        let mut refresh = false;
        let mut chosen: Option<(PathBuf, usize)> = None;
        let panel = if self.compact {
            egui::Panel::right("tags-summary-compact").resizable(false).exact_size(ui.available_width())
        } else {
            egui::Panel::right("tags-summary").resizable(true).default_size(300.0).size_range(220.0..=600.0)
        };
        panel.show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.strong(crate::tl_id!("ui.tags_summary"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // A cross drawn with lines: the interface fonts have no glyph for one.
                    let (rect, x) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                    x.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, crate::tl_id!("ui.close_the_tags_summary")));
                    if x.hovered() {
                        ui.painter().rect_filled(rect, 3.0, tokens.row_hover);
                    }
                    let c = rect.shrink(7.0);
                    let stroke = egui::Stroke::new(1.4, tokens.text);
                    ui.painter().line_segment([c.left_top(), c.right_bottom()], stroke);
                    ui.painter().line_segment([c.right_top(), c.left_bottom()], stroke);
                    if x.on_hover_text(crate::tl_id!("ui.close_the_tags_summary")).clicked() {
                        close = true;
                    }
                });
            });
            ui.label(egui::RichText::new(crate::tl_id!("ui.tags_look_in")).small().color(tokens.muted));
            ui.horizontal_wrapped(|ui| {
                for scope in TagScope::ALL {
                    if ui.selectable_label(pane.scope == scope, scope.label()).clicked() && pane.scope != scope {
                        pane.scope = scope;
                        refresh = true;
                    }
                }
            });
            if ui.checkbox(&mut pane.unchecked, crate::tl_id!("ui.tags_only_unchecked_to_do_items")).changed() {
                refresh = true;
            }
            if ui.button(crate::tl_id!("ui.refresh")).on_hover_text(crate::tl_id!("ui.look_again_for_tagged_notes")).clicked() {
                refresh = true;
            }
            ui.separator();
            egui::ScrollArea::vertical().id_salt("tags-summary-list").auto_shrink([false, false]).show(ui, |ui| {
                if pane.found.is_empty() {
                    ui.weak(crate::tl_id!("ui.no_tagged_notes_here"));
                }
                for tag in ergonote_model::Tag::all() {
                    let mut lines = pane.found.iter().filter(|f| f.tag == tag).peekable();
                    if lines.peek().is_none() {
                        continue;
                    }
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let (rect, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                        ribbon::paint_tag(ui.painter(), rect, tag, tokens.text, tokens.accent);
                        ui.strong(crate::tl!(tag.label()));
                    });
                    for line in lines {
                        let words = if line.text.is_empty() { crate::tl_id!("ui.tags_empty_line").to_string() } else { line.text.clone() };
                        let place = if line.section.is_empty() { line.page.clone() } else { format!("{} > {}", line.section, line.page) };
                        // Painted rather than laid out as labels, so the row is one target that
                        // a screen reader names once, with a To Do's box ticked or not.
                        let width = ui.available_width().max(60.0);
                        let indent = 22.0;
                        let text = ui.painter().layout(words.clone(), egui::FontId::proportional(13.5), tokens.text, width - indent);
                        let under = ui.painter().layout(place, egui::FontId::proportional(11.5), tokens.muted, width - indent);
                        let (rect, row) = ui.allocate_exact_size(vec2(width, text.size().y + under.size().y + 8.0), Sense::click());
                        row.widget_info(|| match line.done {
                            Some(done) => egui::WidgetInfo::selected(egui::WidgetType::CheckBox, true, done, words.clone()),
                            None => egui::WidgetInfo::labeled(egui::WidgetType::Button, true, words.clone()),
                        });
                        let painter = ui.painter();
                        if row.hovered() {
                            painter.rect_filled(rect, 3.0, tokens.row_hover);
                        }
                        if let Some(done) = line.done {
                            let tick = egui::Rect::from_min_size(rect.min + vec2(4.0, 4.0), vec2(12.0, 12.0));
                            painter.rect_stroke(tick, 2.0, egui::Stroke::new(1.2, tokens.muted), egui::StrokeKind::Inside);
                            if done {
                                let at = |x: f32, y: f32| tick.min + vec2(x, y);
                                painter.line(vec![at(2.5, 6.5), at(5.0, 9.0), at(9.5, 3.0)], egui::Stroke::new(1.6, tokens.accent));
                            }
                        }
                        let below = text.size().y + 3.0;
                        painter.galley(rect.min + vec2(indent, 2.0), text, tokens.text);
                        painter.galley(rect.min + vec2(indent, 2.0 + below), under, tokens.muted);
                        if row.clicked() {
                            chosen = Some((line.path.clone(), line.block));
                        }
                    }
                }
            });
        });
        if close {
            self.tags_pane = None;
            return;
        }
        if refresh {
            self.refresh_tags();
        }
        if let Some((page, block)) = chosen {
            self.go_to_tagged(&page, block);
        }
    }

    /// Opens the page a tagged line is on and puts the caret at the start of that line.
    pub(super) fn go_to_tagged(&mut self, page: &Path, block: usize) {
        let mut parent = page.parent();
        while let Some(d) = parent {
            if d == self.session.root() {
                break;
            }
            self.open.insert(d.to_path_buf());
            parent = d.parent();
        }
        if self.editor.as_ref().is_none_or(|ed| ed.dir != page) {
            self.open_page(page);
        }
        let Some(ed) = self.editor.as_mut().filter(|ed| ed.dir == page && ed.writable) else { return };
        let Some(b) = ed.blocks.get(block).filter(|b| pageedit::is_writable(b)) else { return };
        ed.caret = Some(pageedit::Caret {
            block,
            text: pageedit::caret_text(b),
            focus: true,
            selection: (0, 0),
            place: None,
            cell: None,
            span: None,
            inline: None,
        });
    }

    pub(super) fn side_panel(&mut self, ui: &mut Ui) {
        // Full page view: the notebooks step aside until the user asks for them again.
        if self.full_page && self.editor.is_some() {
            return;
        }
        let mut acts = Vec::new();
        // A phone-width window has room for one column, so it always shows the one tree.
        let compact = self.compact;
        if self.split_panes && !compact && !self.is_simple_mobile() {
            self.organiser_panes(ui, &mut acts);
            for a in acts {
                self.apply(a);
            }
            return;
        }
        let mut split_now = false;
        // Compact or simple mode: the tile browser fills the window.
        let panel = if compact || self.is_simple_mobile() {
            egui::Panel::left("tree-compact").resizable(false).exact_size(ui.available_width())
        } else {
            egui::Panel::left("tree").resizable(true).default_size(300.0).size_range(200.0..=700.0)
        };
        panel.show(ui, |ui| {
            if self.is_simple_mobile() {
                self.mobile_tiles_view(ui, &mut acts);
                return;
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                // In the one-tree layout the same button splits the panel and hides the
                // notebooks, which is what pressing it in OneNote does. A phone-width window
                // has no room for the split, so it has no button for it.
                if !compact {
                    split_now = Self::bars_button_at(ui).clicked();
                }
                ui.label(egui::RichText::new(crate::tl_id!("ui.notebooks_2")).small().color(self.tokens.muted));
            });
            ui.horizontal_wrapped(|ui| {
                if ui.small_button(crate::tl_id!("ui.new_notebook")).clicked() {
                    self.dialog = Some(Dialog::NewNotebook { name: String::new(), format: None });
                }
                if ui.small_button(crate::tl_id!("ui.expand_all")).on_hover_text(crate::tl_id!("ui.open_every_level")).clicked() {
                    self.open = self.collect_branches().into_iter().collect();
                }
                if ui.small_button(crate::tl_id!("ui.collapse_all")).clicked() {
                    self.open.clear();
                }
                self.new_section_button(ui, &mut acts);
            });
            ui.add_space(2.0);
            // A mouse drag in a pane must not scroll it: in the tree it is a click that
            // slipped. The wheel and the bar still work, and on a touch screen a finger drag
            // scrolls, which is the only way a phone has.
            let scroll = egui::containers::scroll_area::ScrollSource {
                drag: egui::containers::scroll_area::DragScroll::OnTouch,
                ..egui::containers::scroll_area::ScrollSource::default()
            };
            egui::ScrollArea::vertical().auto_shrink([false, false]).scroll_source(scroll).show(ui, |ui| {
                if self.session.tree().is_empty() {
                    ui.weak(crate::tl_id!("ui.no_notebooks_yet_use_new_notebook"));
                }
                for n in self.session.tree() {
                    self.draw_node(ui, n, 0, &mut acts);
                }
                self.empty_space_menu(ui, None, &mut acts);
            });
        });
        if split_now {
            self.split_panes = true;
            self.organisers = false;
            let _ = self.session.settings.set("split_panes", "true");
        }
        for a in acts {
            self.apply(a);
        }
    }

    /// The empty space under the rows of a column, with a menu of its own: right-clicking
    /// where there is no row is how a new notebook or a new page is started in OneNote.
    pub(super) fn empty_space_menu(&self, ui: &mut Ui, section: Option<&Path>, acts: &mut Vec<Act>) {
        let left = ui.available_size();
        if left.y < 2.0 {
            return;
        }
        let (rect, resp) = ui.allocate_exact_size(left, Sense::click());
        let _ = rect;
        let mut reveal = false;
        let mut choose = false;
        resp.context_menu(|ui| {
            match section {
                // Under the pages: a page at the end of this section.
                Some(path) => {
                    if ui.button(crate::tl_id!("ui.new_page_2")).clicked() {
                        acts.push(Act::AddKind(path.to_path_buf(), NodeKind::Section, NodeKind::Page));
                        ui.close();
                    }
                }
                // Under the notebooks: a notebook, and the ways into the folder itself.
                None => {
                    if ui.button(crate::tl_id!("ui.new_notebook_2")).clicked() {
                        acts.push(Act::NewNotebook);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(crate::tl_id!("ui.open_a_folder")).clicked() {
                        choose = true;
                        ui.close();
                    }
                    if ui.button(crate::tl_id!("ui.show_the_notebooks_folder")).clicked() {
                        reveal = true;
                        ui.close();
                    }
                }
            }
        });
        if reveal {
            acts.push(Act::RevealRoot);
        }
        if choose {
            acts.push(Act::ChooseRoot);
        }
    }

    /// The gap between two pages in the pages column. Hovering it draws the line OneNote
    /// draws there, and clicking it makes a page in that place rather than at the end.
    ///
    /// `before` and `after` are the orders of the pages on either side; the new page takes
    /// the number in between, which is why siblings are numbered ten apart.
    pub(super) fn page_gap(&self, ui: &mut Ui, section: &Path, before: Option<u32>, after: Option<u32>, acts: &mut Vec<Act>) {
        /// How big the round "+" is. The gap itself is shorter than this on purpose: it is
        /// a thin strip between two pages, and the button stands out of it.
        const PLUS: f32 = 8.0;
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 7.0), Sense::click());
        let hot = resp.hovered();
        if hot {
            // The gap is 7 points tall and the circle is 13 across, so the circle has to
            // be drawn outside the strip it belongs to. `painter_at` cannot: it narrows
            // the clip it is given to whatever the ui already has, which is the strip, and
            // the circle came out a lozenge, flat at the top and the bottom. Painting on
            // the layer starts from the whole window instead.
            let room = (PLUS + 1.0 - rect.height() / 2.0).max(2.0);
            let painter = ui.ctx().layer_painter(ui.layer_id()).with_clip_rect(rect.expand(room));
            let y = rect.center().y;
            let t = &self.tokens;
            painter.line_segment([egui::pos2(rect.min.x + 21.0, y), egui::pos2(rect.max.x - 6.0, y)], egui::Stroke::new(1.5, t.accent));
            let c = egui::pos2(rect.min.x + 10.0, y);
            painter.circle_filled(c, PLUS, t.accent);
            // Draw the cross as two equal strokes around the circle's center. A font glyph's
            // baseline and line box can make a visually off-center plus even with centered text.
            let cross = egui::Stroke::new(2.0, t.panel);
            painter.line_segment([c - vec2(4.0, 0.0), c + vec2(4.0, 0.0)], cross);
            painter.line_segment([c - vec2(0.0, 4.0), c + vec2(0.0, 4.0)], cross);
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let enabled = ui.is_enabled();
        let name = crate::tl_id!("ui.add_a_page_here");
        resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name));
        let resp = resp.on_hover_text(crate::tl_id!("ui.a_page_here"));
        if resp.clicked() {
            // Halfway between the two, or ten past the last one.
            match (before, after) {
                // No number between them: renumber first, so the page can go between.
                (Some(b), Some(a)) if a <= b + 1 => acts.push(Act::RenumberAndAddPage(section.to_path_buf(), b)),
                (Some(b), Some(a)) => acts.push(Act::AddPageAt(section.to_path_buf(), b + (a - b) / 2)),
                (Some(b), None) => acts.push(Act::AddPageAt(section.to_path_buf(), b.saturating_add(10))),
                (None, Some(a)) => acts.push(Act::AddPageAt(section.to_path_buf(), a / 2)),
                (None, None) => acts.push(Act::AddPageAt(section.to_path_buf(), 10)),
            }
        }
    }

    /// Where a new section goes: the notebook or section group the chosen section or page is in,
    /// a section group that is itself chosen, or else the first notebook. `None` with no notebook.
    pub(super) fn new_section_parent(&self) -> Option<(PathBuf, NodeKind)> {
        let mut at = self.selected.clone().or_else(|| self.editor.as_ref().map(|e| e.dir.clone()));
        while let Some(path) = at {
            match ergonote_store::tree::read_node(&path).map(|n| n.kind) {
                Some(kind @ (NodeKind::Notebook | NodeKind::SectionGroup)) => return Some((path, kind)),
                Some(_) => at = path.parent().map(Path::to_path_buf),
                None => break,
            }
        }
        self.session.tree().first().map(|n| (n.path.clone(), NodeKind::Notebook))
    }

    /// A plain, always visible "+ New section" button for the chosen notebook, as OneNote has
    /// under its sections, so a section can be added without finding the hover-only plus or the
    /// right-click menu (issue #139). It opens the same dialog those do.
    pub(super) fn new_section_button(&self, ui: &mut Ui, acts: &mut Vec<Act>) {
        let target = self.new_section_parent();
        let text = egui::RichText::new(crate::tl_id!("ui.new_section_button")).size(if self.compact { 14.0 } else { 13.0 }).color(self.tokens.accent);
        let mut button = egui::Button::new(text).stroke(egui::Stroke::new(1.0, self.tokens.container_border)).corner_radius(8.0);
        // On a phone the button stands on its own under the tiles, so it takes the height and
        // the surface of the other controls in that bar.
        if self.compact {
            button = button.fill(self.tokens.panel).min_size(vec2(0.0, 36.0));
        }
        let resp = ui.add_enabled(target.is_some(), button).on_hover_text(crate::tl_id!("ui.a_new_section_in_the_chosen_notebook"));
        if resp.clicked()
            && let Some((parent, kind)) = target
        {
            acts.push(Act::AddKind(parent, kind, NodeKind::Section));
        }
    }

    /// The three-bar button, at the top left of the side panel.
    pub(super) fn bars_button_at(ui: &mut Ui) -> egui::Response {
        // Drawn rather than typed: the font's own three-bar glyph is heavy and tight.
        let (rect, resp) = ui.allocate_exact_size(vec2(22.0, 20.0), Sense::click());
        let painter = ui.painter_at(rect);
        if resp.hovered() {
            painter.rect_filled(rect, 3.0, ui.visuals().widgets.hovered.bg_fill);
        }
        let colour = ui.visuals().widgets.inactive.fg_stroke.color;
        icons::paint(&painter, egui::Rect::from_center_size(rect.center(), vec2(16.0, 16.0)), Icon::Bars, colour);
        let enabled = ui.is_enabled();
        resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, BARS));
        resp.on_hover_text(crate::tl_id!("ui.show_or_hide_the_notebooks_column"))
    }

    /// The side panel as two columns: the organisers (notebooks, section groups and
    /// sections) and then the pages of whichever section is chosen, the way OneNote splits
    /// them. The three-bar button in the top bar hides the first column.
    pub(super) fn organiser_panes(&mut self, ui: &mut Ui, acts: &mut Vec<Act>) {
        let scroll = || egui::containers::scroll_area::ScrollSource {
            drag: egui::containers::scroll_area::DragScroll::OnTouch,
            ..egui::containers::scroll_area::ScrollSource::default()
        };
        let mut toggle = false;
        if self.organisers {
            egui::Panel::left("organisers").resizable(true).default_size(240.0).size_range(160.0..=520.0).show(ui, |ui| {
                ui.add_space(4.0);
                // The three-bar button sits at the top left of the notebooks, where
                // OneNote keeps it, and hides this column.
                ui.horizontal(|ui| {
                    toggle |= Self::bars_button_at(ui).clicked();
                    ui.label(egui::RichText::new(crate::tl_id!("ui.notebooks_2")).small().color(self.tokens.muted));
                });
                ui.horizontal(|ui| self.new_section_button(ui, acts));
                ui.add_space(2.0);
                egui::ScrollArea::vertical().id_salt("organisers").auto_shrink([false, false]).scroll_source(scroll()).show(ui, |ui| {
                    if self.session.tree().is_empty() {
                        ui.weak(crate::tl_id!("ui.no_notebooks_yet_use_new_notebook"));
                    }
                    for n in self.session.tree() {
                        self.draw_row(ui, n, 0, acts, false);
                    }
                    self.empty_space_menu(ui, None, acts);
                });
            });
        }
        let section = self.current_section(self.session.tree()).map(|n| (n.path.clone(), n.info.title.clone(), n.children.clone()));
        egui::Panel::left("pages").resizable(true).default_size(230.0).size_range(150.0..=520.0).show(ui, |ui| {
            ui.add_space(4.0);
            match &section {
                Some((path, title, pages)) => {
                    ui.horizontal(|ui| {
                        // With the notebooks hidden the button lives here instead, so it is
                        // always at the top left of the panel.
                        if !self.organisers {
                            toggle |= Self::bars_button_at(ui).clicked();
                        }
                        let add_page = egui::Button::new(crate::tl_id!("ui.add_page")).stroke(egui::Stroke::new(1.0, self.tokens.container_border));
                        if ui.add(add_page).on_hover_text(crate::tl_id!("ui.a_new_page_in_this_section")).clicked() {
                            acts.push(Act::Add(path.clone(), NodeKind::Section));
                        }
                    });
                    ui.label(egui::RichText::new(title.to_uppercase()).small().color(self.tokens.muted));
                    ui.add_space(2.0);
                    egui::ScrollArea::vertical().id_salt("pages").auto_shrink([false, false]).scroll_source(scroll()).show(ui, |ui| {
                        let list: Vec<&Node> = pages.iter().filter(|p| p.info.kind == NodeKind::Page).collect();
                        if list.is_empty() {
                            ui.weak(crate::tl_id!("ui.no_pages_in_this_section_yet"));
                        }
                        // A line between every pair, and above the first and below the
                        // last: hovering one offers a page in that place.
                        let mut last: Option<u32> = None;
                        for p in &list {
                            self.page_gap(ui, path, last, Some(p.info.order), acts);
                            self.draw_row(ui, p, 0, acts, true);
                            last = Some(p.info.order);
                        }
                        self.page_gap(ui, path, last, None, acts);
                        self.empty_space_menu(ui, Some(path), acts);
                    });
                }
                None => {
                    ui.horizontal(|ui| {
                        if !self.organisers {
                            toggle |= Self::bars_button_at(ui).clicked();
                        }
                        ui.weak(crate::tl_id!("ui.choose_a_section_to_see_its_pages"));
                    });
                }
            }
        });
        if toggle {
            self.organisers = !self.organisers;
        }
    }

    /// The search pane on the right: OneNote's search, laid out the way Word lays its
    /// navigation pane out, one card for each page and the words that matched marked in its
    /// snippet. Ctrl+F, Ctrl+E or the search button opens it; Escape or Android's Back closes
    /// it; a click outside it closes it without swallowing that click.
    pub(super) fn search_panel(&mut self, ui: &mut Ui) {
        if !self.search_open {
            return;
        }
        // A dialog owns the window; the pane goes as the transient surface it is.
        if self.dialog.is_some() {
            self.dismiss_search();
            return;
        }
        let tokens = self.tokens;
        let mut close = false;
        let mut chosen: Option<PathBuf> = None;
        let mut open_first = false;
        let panel = if self.compact {
            egui::Panel::right("page-search-compact").resizable(false).exact_size(ui.available_width())
        } else {
            egui::Panel::right("page-search").resizable(true).default_size(320.0).size_range(240.0..=560.0)
        };
        let shown = panel.show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.strong(crate::tl_id!("ui.search_all_pages"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // A cross drawn with lines: the interface fonts have no glyph for one.
                    let (rect, x) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                    x.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, crate::tl_id!("ui.close_search")));
                    if x.hovered() {
                        ui.painter().rect_filled(rect, 3.0, tokens.row_hover);
                    }
                    let c = rect.shrink(7.0);
                    let stroke = egui::Stroke::new(1.4, tokens.text);
                    ui.painter().line_segment([c.left_top(), c.right_bottom()], stroke);
                    ui.painter().line_segment([c.right_top(), c.left_bottom()], stroke);
                    if x.on_hover_text(crate::tl_id!("ui.close_search")).clicked() {
                        close = true;
                    }
                });
            });
            ui.add_space(4.0);
            let field = egui::TextEdit::singleline(&mut self.search)
                .id(Id::new("page-search"))
                .hint_text(crate::tl_id!("ui.search_all_pages"))
                .desired_width(f32::INFINITY);
            let resp = ui.add(field);
            // Named for a screen reader: the grey hint inside the box is not a name.
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextInput, true, crate::tl_id!("ui.search_all_pages")));
            if self.focus_search {
                resp.request_focus();
                self.focus_search = false;
            }
            if resp.changed() && self.search != self.searched {
                self.searched = self.search.clone();
                self.hits = match self.session.run("search.query", &params(&[("q", &self.search), ("limit", "20")])) {
                    Ok(Output::Hits(h)) => h,
                    _ => Vec::new(),
                };
            }
            // Enter opens the first match rather than leaving the user with a mouse-only list.
            if resp.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) && !self.hits.is_empty() {
                open_first = true;
            }
            ui.separator();

            if self.hits.is_empty() {
                ui.weak(crate::tl_id!("ui.no_pages_match"));
            } else {
                egui::ScrollArea::vertical().id_salt("search-results-list").auto_shrink([false, true]).show(ui, |ui| {
                    for h in &self.hits {
                        let card = egui::Frame::new()
                            .fill(tokens.panel)
                            .stroke(egui::Stroke::new(1.0, tokens.faint))
                            .corner_radius(6.0)
                            .inner_margin(egui::Margin::symmetric(10, 8))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.label(egui::RichText::new(&h.title).strong().color(tokens.text));
                                ui.add_space(2.0);
                                let mut job = egui::text::LayoutJob::default();
                                let font = egui::FontId::proportional(12.5);
                                let run = |color: Color32, background: Color32| egui::TextFormat {
                                    font_id: font.clone(),
                                    color,
                                    background,
                                    ..Default::default()
                                };
                                match ergonote_store::search::match_span(&h.snippet, self.search.trim()) {
                                    Some((start, end)) => {
                                        job.append(&h.snippet[..start], 0.0, run(tokens.muted, Color32::TRANSPARENT));
                                        job.append(&h.snippet[start..end], 0.0, run(tokens.text, tokens.row_selected));
                                        job.append(&h.snippet[end..], 0.0, run(tokens.muted, Color32::TRANSPARENT));
                                    }
                                    None => job.append(&h.snippet, 0.0, run(tokens.muted, Color32::TRANSPARENT)),
                                }
                                ui.label(job);
                            });
                        let resp = ui.interact(card.response.rect, ui.make_persistent_id(("search-hit", &h.path)), Sense::click());
                        if resp.hovered() {
                            ui.painter().rect_stroke(card.response.rect, 6.0, egui::Stroke::new(1.5, tokens.accent), egui::StrokeKind::Inside);
                        }
                        let label = format!("{} {}", h.title, h.snippet);
                        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone()));
                        if resp.clicked() {
                            chosen = Some(h.path.clone());
                        }
                        ui.add_space(6.0);
                    }
                });
            }
        });
        // An outside press dismisses the pane without consuming the press, so the clicked
        // ribbon, tree row or page still does its own work.
        let pressed_outside =
            ui.ctx().input(|input| input.pointer.any_pressed() && input.pointer.interact_pos().is_some_and(|pos| !shown.response.rect.contains(pos)));
        if close || pressed_outside {
            self.dismiss_search();
            return;
        }
        if let Some(path) = chosen.or_else(|| open_first.then(|| self.hits.first().map(|h| h.path.clone())).flatten()) {
            self.open_search_result(&path);
        }
    }
}
