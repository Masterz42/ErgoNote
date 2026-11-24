use super::*;

impl NoteCraftApp {
    // ---------- control channel ----------

    /// Attaches a control channel (see `control.rs` and `docs/control-protocol.md`).
    pub fn with_control(mut self, server: ControlServer, rx: Receiver<ControlRequest>) -> Self {
        self.control_server = Some(server);
        self.control = Some(rx);
        self
    }

    /// Says where GridCraft is, or that it is not installed, instead of looking for it on this
    /// machine. Tests use this so a table's spreadsheet buttons do not depend on the machine.
    pub fn with_gridcraft(mut self, exe: Option<std::path::PathBuf>) -> Self {
        self.gridcraft = exe;
        self
    }

    /// Attaches only a request receiver, with no network listener. The headless screenshot
    /// runner uses this to feed scripted requests.
    pub fn with_requests(mut self, rx: Receiver<ControlRequest>) -> Self {
        self.control = Some(rx);
        self
    }

    /// The input events for the next frame, from clicks, keys and text that control requests
    /// queued. A click is spread over three frames (move, press, release) the way a real mouse
    /// delivers it. The window's shell calls this once per frame and adds the events to its input.
    pub fn take_events(&mut self) -> Vec<egui::Event> {
        self.event_frames.pop_front().unwrap_or_default()
    }

    pub(super) fn poll_control(&mut self, ctx: &Context) {
        let mut pending = Vec::new();
        if let Some(rx) = &self.control {
            while let Ok(req) = rx.try_recv() {
                pending.push(req);
            }
        }
        for req in pending {
            self.handle_control(ctx, req);
        }
    }

    /// Answers one control request. Methods: see `docs/control-protocol.md`.
    pub fn handle_control(&mut self, ctx: &Context, req: ControlRequest) {
        if req.method == "ui.screenshot" {
            let path = req.params.get("path").and_then(Value::as_str).map(PathBuf::from);
            ctx.request_screenshot(move |image| {
                req.respond(encode_screenshot(&image, path.as_deref()));
            });
            ctx.request_repaint();
            return;
        }
        let result = self.control_method(ctx, &req.method, &req.params);
        req.respond(result);
    }

    pub(super) fn control_method(&mut self, ctx: &Context, method: &str, p: &Value) -> Result<Value, String> {
        let num = |k: &str| p.get(k).and_then(Value::as_f64).map(|v| v as f32);
        let flag = |k: &str| p.get(k).and_then(Value::as_bool).unwrap_or(false);
        let text = |k: &str| p.get(k).and_then(Value::as_str);
        match method {
            "engine.commands" => Ok(notecraft_engine::commands_json()),
            "engine.execute" => {
                let id = text("command").ok_or("missing command")?;
                let params = notecraft_engine::params_from_json(p.get("params"));
                self.save_now();
                let out = self.session.run(id, &params).map_err(|e| e.to_string())?;
                if self.editor.as_ref().is_some_and(|ed| !ed.dir.exists()) {
                    self.editor = None;
                }
                Ok(notecraft_engine::output_json(&out))
            }
            "ui.inspect" => Ok(self.inspect(ctx)),
            "ui.open" => {
                self.open_path(text("path").ok_or("missing path")?);
                if self.status_is_error { Err(self.status.clone()) } else { Ok(self.inspect(ctx)) }
            }
            // Put the caret in a block of the open page, the way a click does.
            "ui.caret" => {
                let Some(ed) = self.editor.as_mut() else { return Err("no page is open".into()) };
                if !ed.writable {
                    return Err("this page is read-only until the block editor can keep its markup".into());
                }
                let n = p.get("block").and_then(Value::as_u64).ok_or("missing block")? as usize;
                // A heading that only repeats the page's title is not drawn, so the caret
                // cannot go there: the next block is what a person would have clicked.
                let n = if pageedit::is_title_echo(&ed.blocks, n, &ed.title) { n + 1 } else { n };
                let block = ed.blocks.get(n).ok_or_else(|| format!("the page has no block {n}"))?;
                if !pageedit::is_writable(block) {
                    return Err(format!("block {n} is a table, a rule or a picture; the caret cannot go in it yet"));
                }
                ed.caret = Some(pageedit::Caret {
                    block: n,
                    text: pageedit::caret_text(block),
                    focus: true,
                    selection: (0, 0),
                    place: None,
                    cell: None,
                    span: None,
                    inline: None,
                });
                ctx.request_repaint();
                Ok(json!({"block": n, "text": ed.caret.as_ref().map(|c| c.text.clone())}))
            }
            "ui.back" => {
                self.go_back();
                ctx.request_repaint();
                Ok(json!({}))
            }
            "ui.forward" => {
                self.go_forward();
                ctx.request_repaint();
                Ok(json!({}))
            }
            "ui.undo" => {
                self.undo();
                ctx.request_repaint();
                Ok(json!({}))
            }
            "ui.redo" => {
                self.redo();
                ctx.request_repaint();
                Ok(json!({}))
            }
            "ui.expandAll" => {
                self.expand_all();
                Ok(json!({}))
            }
            "ui.collapseAll" => {
                self.open.clear();
                Ok(json!({}))
            }
            "ui.theme" => {
                let mode = text("mode").map(|m| ModePref::parse(m).ok_or("mode must be light, dark or system")).transpose()?;
                let accent =
                    text("accent").map(|a| Accent::parse(a).ok_or("accent must be default, red, green, yellow, blue, gray or purple")).transpose()?;
                let accent = accent.unwrap_or(self.accent);
                match mode.unwrap_or(self.mode_pref) {
                    ModePref::Light => self.set_theme(Theme { mode: Mode::Light, accent }),
                    ModePref::Dark => self.set_theme(Theme { mode: Mode::Dark, accent }),
                    ModePref::System => self.follow_system_theme(accent),
                }
                Ok(json!({"theme": self.theme().id(), "modePref": self.mode_pref.id()}))
            }
            "ui.click" | "ui.move" | "ui.press" | "ui.release" => {
                let pos = egui::pos2(num("x").ok_or("missing x")?, num("y").ok_or("missing y")?);
                let button = match text("button") {
                    Some("right") => egui::PointerButton::Secondary,
                    Some("middle") => egui::PointerButton::Middle,
                    _ => egui::PointerButton::Primary,
                };
                let modifiers = egui::Modifiers { alt: flag("alt"), ctrl: flag("ctrl"), shift: flag("shift"), mac_cmd: false, command: flag("ctrl") };
                self.event_frames.push_back(vec![egui::Event::PointerMoved(pos)]);
                let press = |pressed| vec![egui::Event::PointerButton { pos, button, pressed, modifiers }];
                match method {
                    "ui.click" => {
                        for _ in 0..p.get("count").and_then(Value::as_u64).unwrap_or(1).clamp(1, 3) {
                            self.event_frames.push_back(press(true));
                            self.event_frames.push_back(press(false));
                        }
                    }
                    "ui.press" => self.event_frames.push_back(press(true)),
                    "ui.release" => self.event_frames.push_back(press(false)),
                    _ => {}
                }
                ctx.request_repaint();
                Ok(json!({}))
            }
            "ui.key" => {
                let name = text("key").ok_or("missing key")?;
                let key = egui::Key::from_name(name).ok_or_else(|| format!("unknown key '{name}'"))?;
                let modifiers = egui::Modifiers { alt: flag("alt"), ctrl: flag("ctrl"), shift: flag("shift"), mac_cmd: false, command: flag("ctrl") };
                for pressed in [true, false] {
                    self.event_frames.push_back(vec![egui::Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers }]);
                }
                ctx.request_repaint();
                Ok(json!({}))
            }
            // Pasting is its own event, not typing: a table from another app arrives this
            // way, and the page editor turns it back into a table.
            "ui.paste" => {
                self.event_frames.push_back(vec![egui::Event::Paste(text("text").ok_or("missing text")?.to_string())]);
                ctx.request_repaint();
                Ok(json!({}))
            }
            "ui.text" => {
                self.event_frames.push_back(vec![egui::Event::Text(text("text").ok_or("missing text")?.to_string())]);
                ctx.request_repaint();
                Ok(json!({}))
            }
            // Scrolling, so a script can reach what is below the fold: the list of files
            // in a page's folder sits under a whole sheet of paper, and nothing could
            // reach it from here before.
            "ui.scroll" => {
                let (dx, dy) = (num("x").unwrap_or(0.0), num("y").ok_or("missing y")?);
                self.event_frames.push_back(vec![egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(dx, dy),
                    modifiers: egui::Modifiers { shift: flag("shift"), ..egui::Modifiers::NONE },
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Unknown,
                }]);
                ctx.request_repaint();
                Ok(json!({}))
            }
            "ui.resize" => {
                let (w, h) = (num("width").ok_or("missing width")?, num("height").ok_or("missing height")?);
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(vec2(w.clamp(400.0, 8000.0), h.clamp(300.0, 8000.0))));
                Ok(json!({}))
            }
            "app.quit" => {
                self.save_now();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                Ok(json!({}))
            }
            other => Err(format!("unknown method '{other}'")),
        }
    }

    /// What the window shows now: the visible tree rows, the open page, the theme and the size.
    pub fn inspect(&self, ctx: &Context) -> Value {
        // The Mermaid diagrams of the page as it is drawn, not of the code view: how many, and how
        // many of them are drawn, still being drawn, or could not be.
        let in_page: Vec<String> = self
            .editor
            .as_ref()
            .map(|e| {
                e.blocks
                    .iter()
                    .filter_map(|b| match b {
                        notecraft_formats::markdown::Block::Code { lang, text } if lang == "mermaid" => Some(text.clone()),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let in_page_counts = crate::diagramview::counts(ctx, &in_page, &self.tokens);
        let mut rows = Vec::new();
        fn walk(app: &NoteCraftApp, n: &Node, depth: usize, rows: &mut Vec<Value>) {
            let open = app.open.contains(&n.path);
            rows.push(json!({
                "path": notecraft_store::tree::display_path(n.path.strip_prefix(app.session.root()).unwrap_or(&n.path)).replace('\\', "/"),
                "title": n.info.title,
                "kind": n.info.kind.id(),
                "format": n.info.format.map(|f| f.id()),
                "depth": depth,
                "hasChildren": !n.children.is_empty(),
                "open": open,
                "selected": app.selected.as_deref() == Some(n.path.as_path()),
            }));
            if open {
                for c in &n.children {
                    walk(app, c, depth + 1, rows);
                }
            }
        }
        for n in self.session.tree() {
            walk(self, n, 0, &mut rows);
        }
        let screen = ctx.content_rect();
        json!({
            "window": {"width": screen.width(), "height": screen.height(), "pixelsPerPoint": ctx.pixels_per_point()},
            "canvas": {
                "viewOffset": [self.canvas_pan.x, self.canvas_pan.y],
                // The sideways scroll bar along the page's bottom and its thumb, as
                // [left, top, right, bottom] in screen points, or null when everything fits.
                "horizontalBar": self.page_hbar.map(|(track, _)| [track.min.x, track.min.y, track.max.x, track.max.y]),
                "horizontalThumb": self.page_hbar.map(|(_, thumb)| [thumb.min.x, thumb.min.y, thumb.max.x, thumb.max.y]),
                // True while Insert Space is in hand (issue #164).
                "insertSpace": self.space_tool,
            },
            // How the side panel is arranged: one tree, or the organisers and the pages in
            // columns of their own, with the first one hidden or not.
            "panes": {
                "split": self.split_panes,
                "organisers": self.organisers,
                "compact": self.compact,
                "fullPage": self.full_page,
                "list": self.compact_list_fills(),
            },
            // Home > Find Tags: where the Tags Summary looks and how many lines it lists, or
            // null while it is closed.
            "pageTemplates": self.templates_pane.as_ref().map(|p| json!({
                "templates": p.templates.iter().map(|t| json!({"id": t.id, "name": t.name, "category": t.category.id()})).collect::<Vec<_>>(),
                "name": p.name,
            })),
            "tagsSummary": self.tags_pane.as_ref().map(|p| json!({
                "scope": p.scope.id(),
                "unchecked": p.unchecked,
                "lines": p.found.iter().map(|l| json!({"tag": l.tag.id(), "text": l.text, "done": l.done, "page": l.page, "section": l.section})).collect::<Vec<_>>(),
            })),
            // Back and Forward, and the Quick Access Toolbar's choices.
            "nav": {
                "back": self.nav.back_count(),
                "forward": self.nav.forward_count(),
                "current": self.nav.current().map(|p| self.shown(p)),
            },
            "windowFrame": if self.frameless { "own" } else { "system" },
            "quickAccess": {"shown": self.qat.show, "below": self.qat.below, "hidden": self.qat.hidden},
            // Folded to its tab names (Ctrl+F1), and whether a clicked tab shows it over the page.
            "ribbon": {"collapsed": self.ribbon_collapsed, "shown": !self.ribbon_folded() || self.ribbon_peek, "tab": self.ribbon_tab},
            // Whether a dialog is up over the window (Android's Back closes it first).
            "dialogOpen": self.dialog.is_some(),
            "theme": {"id": self.theme().id(), "modePref": self.mode_pref.id(), "accent": self.accent.id()},
            // The paper behind the pages, as the View tab's gallery sets it.
            "paper": self.paper_now().id(),
            "paperSize": self.editor.as_ref().and_then(|e| e.paper_size).map_or_else(|| "auto".to_string(), notecraft_model::paper::Paper::write),
            "ruleColour": self.editor.as_ref().and_then(|e| e.rule_colour).map(notecraft_model::NodeColor::id),
            "pageColour": self.editor.as_ref().and_then(|e| e.colour).map(notecraft_model::NodeColor::id),
            // The pen the Draw tab is set to, and the drawing it writes in.
            "ink": {
                "pointer": self.pointer.id(),
                "tool": self.pen().tool.id(),
                "colour": self.pen().colour.hex(),
                "width": self.pen().width,
                "rack": notecraft_ink::rack::write(&self.rack),
                "nib": self.nib,
                "sensitivity": self.sensitivity,
                "holding": self.pad.as_ref().map_or(0, crate::inkpad::Pad::holding),
                "bounds": self.pad.as_ref().and_then(crate::inkpad::Pad::bounds),
                "gain": self.gain(),
                "strokes": self.pad.as_ref().map(crate::inkpad::Pad::strokes),
                "inked": self.pad.as_ref().is_some_and(|p| !p.is_empty()),
            },
            // What a pen has told this window. `pen` is false until a device reports a
            // pressure at all, and `hardest` is the hardest press since the window opened.
            "pen": {"seen": self.stylus.sample().pen, "pressure": self.stylus.sample().pressure, "hardest": self.stylus.seen().0, "readings": self.stylus.seen().1},
            "defaultFormat": self.session.settings.global_format().0.id(),
            // Where GridCraft is, when a table can be handed to it.
            "gridcraft": self.gridcraft.as_ref().map(|p| p.to_string_lossy()),
            // The code view: whether it is open, which of the page's files is picked, and
            // how much of that file is in hand.
            "codeView": {
                "open": self.show_source,
                "picked": self.code.picked,
                "characters": self.code.text.chars().count(),
                // The colours the picked file is shown in, and the Find bar: whether it is open,
                // what it looks for, how many matches there are and which one (from 1) it is on.
                "language": self.code.picked.as_deref().map_or("none", |n| crate::highlight::Lang::of_file(n).label()),
                "colouredStretches": self.code.coloured.len(),
                "find": {
                    "open": self.code.find.open,
                    "query": self.code.find.query,
                    "matchCase": self.code.find.case,
                    "matches": self.code.find.matches.len(),
                    "current": if self.code.find.matches.is_empty() { Value::Null } else { json!(self.code.find.current + 1) },
                },
            },
            "pageDiagrams": {
                "found": in_page.len(),
                "drawn": in_page_counts.0,
                "drawing": in_page_counts.1,
                "failed": in_page_counts.2,
            },
            "notebooksFolder": self.shown(self.session.root()),
            "status": {"text": self.status, "error": self.status_is_error},
            "rows": rows,
            "unread": self.unread_list(),
            "page": self.editor.as_ref().map(|e| json!({
                "path": e.dir.strip_prefix(self.session.root()).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default(),
                "title": e.title,
                // What the title area actually shows: blank for a page with no title of its
                // own, unlike `title`, which is always at least the "Untitled page" fallback.
                "titleArea": e.title_buf,
                "format": e.format.id(),
                "dirty": e.dirty_since.is_some() || e.saving.is_some(),
                "bodyCharacters": e.body.chars().count(),
                "blocks": e.blocks.len(),
                "attachments": e.attachments.len(),
                "writable": e.writable,
                // `selection` is (where the caret is, where the selection began), in
                // characters: the two are equal when nothing is selected.
                "caret": e.caret.as_ref().map(|c| json!({"block": c.block, "text": c.text, "at": c.selection.0, "anchor": c.selection.1})),
                // A selection dragged across more than one block, as block and byte at each
                // end. Null for the ordinary selection inside one block, which `caret` has.
                "across": e.across.map(|a| json!({"from": [a.from.0, a.from.1], "to": [a.to.0, a.to.1]})),
                // The boxes the blocks are arranged in, as the page keeps them.
                "containers": e.boxes.iter().map(|b| json!({"x": b.x, "y": b.y, "width": b.width, "blocks": b.blocks})).collect::<Vec<_>>(),
                // The container that is selected, with everything in it, or null.
                "selectedContainer": crate::containers::selected(ctx, &e.dir),
                "selectedContainers": crate::containers::selected_boxes(ctx, &e.dir),
                // The image block selected independently of its containing text box, or null.
                "selectedImage": crate::containers::selected_image(ctx, &e.dir),
                "selectedObject": crate::containers::selected_object(ctx, &e.dir).map(|(block, kind)| json!({"block": block, "kind": kind})),
                "origin": [e.origin.x, e.origin.y],
                // What can be taken back, and how many steps are kept.
                "history": {"steps": e.history.len(), "at": e.history.at(), "canUndo": e.history.can_undo(), "canRedo": e.history.can_redo()},
            })),
        })
    }
}
