//! The page's own menu, under the right button.
//!
//! Nothing is laid over the page to carry it. A widget that senses clicks would take the
//! primary click away from the words, the ink and the caret - the same mistake that made
//! seven of eight pens unusable an hour earlier - and a widget that does not sense clicks
//! cannot carry a context menu at all. So the secondary click is watched for directly and
//! the menu is drawn where it fell.
//!
//! It is drawn the way OneNote draws one: a picture beside each entry, and the mini toolbar
//! floating over it ([`crate::floating`]).

use crate::floating::{self, Glyph};
use crate::icons::Icon;
use crate::pageedit::TableEdit;
use crate::ribbon::Act;
use crate::ribbon_icons::Mark;
use crate::theme::Tokens;
use egui::Ui;

/// Where the menu is open, if it is. Kept in the ui's own store so it survives frames
/// without the window having to hold it.
fn state(ui: &Ui) -> Option<egui::Pos2> {
    ui.data(|d| d.get_temp::<egui::Pos2>(id()))
}

fn id() -> egui::Id {
    egui::Id::new("page-menu")
}

/// Draws the page's menu when the page has been right-clicked, and puts what was picked
/// into `acts`.
pub fn show(ui: &mut Ui, body: egui::Rect, in_table: bool, acts: &mut Vec<Act>, t: &Tokens) {
    // A right click on a container belongs to the container's own menu. Close this one if it
    // was open, and leave the click alone.
    if crate::containers::took_right_click(ui.ctx()) {
        ui.data_mut(|d| d.remove::<egui::Pos2>(id()));
        return;
    }
    let pointer = ui.ctx().pointer_latest_pos();
    let opened_now = ui.input(|i| i.pointer.secondary_clicked());
    if opened_now && let Some(at) = pointer.filter(|p| body.contains(*p)) {
        log::debug!("context menu: right-clicked at {:?}", at);
        ui.data_mut(|d| d.insert_temp(id(), at));
    }
    let Some(at) = state(ui) else { return };

    let mut picked = false;
    let area = egui::Area::new(id().with("area"))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).inner_margin(egui::Margin::same(4)).show(ui, |ui| {
                ui.set_width(floating::MENU_W);
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                picked = items(ui, acts, true, in_table, t);
            });
        })
        .response;
    // The mini toolbar floats over the menu, as it does in OneNote.
    let fonts = ui.ctx().data(|d| d.get_temp::<floating::FontBoxes>(egui::Id::new(floating::FONT_BOXES_KEY))).unwrap_or_default();
    let bar = floating::show_bar(ui.ctx(), id().with("bar"), area.rect, &floating::text_bar_with_fonts(fonts), t, acts);
    picked |= bar.picked;

    let away = |p: egui::Pos2| !area.rect.contains(p) && !bar.rect.contains(p);
    let out = ui.input(|i| i.pointer.any_click()) && !opened_now && away(pointer.unwrap_or_default());
    if picked || out || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        ui.data_mut(|d| d.remove::<egui::Pos2>(id()));
    }
}

/// The page's menu entries: what a right click on the page offers, each with its picture.
/// `clipboard` puts Paste, Cut and Copy first; a container's menu has its own Cut and Copy for
/// the whole box, so it asks for the rest only. When `in_table` is true, Word-style row and
/// column actions are offered as well. True when one was picked.
pub fn items(ui: &mut Ui, acts: &mut Vec<Act>, clipboard: bool, in_table: bool, t: &Tokens) -> bool {
    let mut picked = false;
    let mut item = |ui: &mut Ui, glyph: Glyph, label: &str, act: Act| {
        if floating::menu_row(ui, Some(glyph), label, t).clicked() {
            acts.push(act);
            picked = true;
        }
    };
    item(ui, Glyph::Mark(Mark::Paste), "Paste", Act::Paste);
    if clipboard {
        item(ui, Glyph::Mark(Mark::Cut), "Cut", Act::Clipboard("cut"));
        item(ui, Glyph::Mark(Mark::Copy), "Copy", Act::Clipboard("copy"));
    }
    item(ui, Glyph::Icon(Icon::Pointer), "Tout sélectionner", Act::SelectAll);
    item(ui, Glyph::Icon(Icon::SpellCheck), "Corriger l'orthographe", Act::CheckOrthography);
    if in_table {
        floating::menu_separator(ui, t);
        item(ui, Glyph::Icon(Icon::TableRow), "Insérer une ligne au-dessus", Act::Table(TableEdit::InsertRow(true)));
        item(ui, Glyph::Icon(Icon::TableRow), "Insérer une ligne en dessous", Act::Table(TableEdit::InsertRow(false)));
        item(ui, Glyph::Icon(Icon::TableColumn), "Insérer une colonne à gauche", Act::Table(TableEdit::InsertColumn(true)));
        item(ui, Glyph::Icon(Icon::TableColumn), "Insérer une colonne à droite", Act::Table(TableEdit::InsertColumn(false)));
        item(ui, Glyph::Mark(Mark::Delete), "Supprimer la ligne", Act::Table(TableEdit::DeleteRow));
        item(ui, Glyph::Mark(Mark::Delete), "Supprimer la colonne", Act::Table(TableEdit::DeleteColumn));
        item(ui, Glyph::Mark(Mark::Delete), "Supprimer le tableau", Act::ConfirmDeleteTable(None));
    }
    picked
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    #[test]
    fn table_actions_appear_only_when_inside_a_table() {
        let mut h = Harness::new_ui(|ui| {
            let t = Theme::DEFAULT.tokens();
            let mut acts = Vec::new();
            items(ui, &mut acts, true, false, &t);
        });
        h.run();
        assert!(h.query_by_label_contains("Insérer une ligne en dessous").is_none());
        assert!(h.query_by_label_contains("Supprimer le tableau").is_none());
        assert!(h.query_by_label("Table").is_none());
        assert!(h.query_by_label("Date and time").is_none());
        assert!(h.query_by_label("Picture from a file").is_none());
        assert!(h.query_by_label("Attach a file").is_none());
        assert!(h.query_by_label("Link").is_none());
        assert!(h.query_by_label("Equation").is_none());

        let picked = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let picked_out = picked.clone();
        let mut h_table = Harness::new_ui(move |ui| {
            let t = Theme::DEFAULT.tokens();
            let mut acts = Vec::new();
            items(ui, &mut acts, true, true, &t);
            if !acts.is_empty()
                && let Ok(mut guard) = picked_out.lock()
            {
                guard.extend(acts);
            }
        });
        h_table.run();
        h_table.get_by_label_contains("Insérer une ligne en dessous").click();
        h_table.run();
        h_table.get_by_label_contains("Supprimer la ligne").click();
        h_table.run();
        h_table.get_by_label_contains("Supprimer le tableau").click();
        h_table.run();
        let got = picked.lock().expect("acts").clone();
        assert!(got.iter().any(|a| matches!(a, Act::Table(TableEdit::InsertRow(false)))));
        assert!(got.iter().any(|a| matches!(a, Act::Table(TableEdit::DeleteRow))));
        assert!(got.iter().any(|a| matches!(a, Act::ConfirmDeleteTable(None))));
    }
}
