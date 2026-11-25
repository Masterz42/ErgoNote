//! The ErgoNote window: top bar, notebook tree, page editor with live preview.
//!
//! Every action goes through `Session::run`, so the window, the command line and any
//! later control channel share one code path.

use crate::backstage;
use crate::control::{ControlRequest, ControlServer, base64};
// The file dialogs: `rfd` everywhere it has a backend, and a stand-in on Android and Web that says so.
#[cfg(any(target_os = "android", target_arch = "wasm32"))]
use crate::filepicker as rfd;
use crate::floating;
use crate::icons::{self, Icon};
use crate::navhistory::NavHistory;
use crate::pageedit;
use crate::preview;
use crate::printout;
use crate::quickaccess;
use crate::ribbon;
use crate::snippets;
use crate::theme::{Accent, Mode, ModePref, Theme, Tokens};
use egui::{Align2, Color32, Context, Id, Key, Modifiers, Sense, Ui, vec2};
use ergonote_engine::{EngineError, Output, Params, Session};
use ergonote_model::{NodeColor, NodeKind, PageFormat};
use ergonote_store::search::Hit;
use ergonote_store::tree::Node;
use serde_json::{Value, json};
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
// std's own Instant panics on wasm32-unknown-unknown, which has no clock; web_time is the
// same type everywhere else and performance.now()/Date.now() there.
use web_time::Instant;

/// The grid boxes snap to on plain paper, in page points: the smallest squared paper's step.
const PLAIN_SNAP_STEP: f32 = 12.0;
const AUTOSAVE_AFTER: Duration = Duration::from_millis(900);
const MINUTE_AUTOSAVE_INTERVAL: Duration = Duration::from_secs(60);
/// The three-bar button: what OneNote puts at the top left of the navigation pane.
const BARS: &str = "\u{2630}";

pub(crate) const INSERT_SYMBOL_CATEGORIES: &[(&str, &[&str])] = &[
    (
        "Mathématiques & sciences",
        &[
            "±", "×", "÷", "·", "≠", "≈", "≤", "≥", "≡", "√", "∞", "∝", "°", "′", "″", "‰", "¹", "²", "³", "½", "¼", "¾", "∈", "∉", "⊂", "⊃", "⊆",
            "∪", "∩", "∅", "∀", "∃", "∠", "⊥", "∥", "△", "∑", "∏", "∫", "∂", "∇", "¬", "∧", "∨",
        ],
    ),
    (
        "Lettres grecques",
        &[
            "α", "β", "γ", "δ", "ε", "ζ", "η", "θ", "λ", "μ", "ν", "ξ", "π", "ρ", "σ", "τ", "φ", "χ", "ψ", "ω", "Γ", "Δ", "Θ", "Λ", "Π", "Σ", "Φ",
            "Ψ", "Ω",
        ],
    ),
    ("Flèches", &["←", "→", "↑", "↓", "↔", "↕", "↖", "↗", "↘", "↙", "⇐", "⇒", "⇔"]),
    (
        "Français & ponctuation",
        &[
            "«", "»", "…", "—", "–", "•", "§", "¶", "†", "‡", "É", "È", "Ê", "Ë", "À", "Â", "Ç", "Ù", "Û", "Î", "Ï", "Ô", "Œ", "œ", "Æ", "æ", "Ñ",
            "ñ", "ß", "¿", "¡",
        ],
    ),
    ("Monnaies & repères", &["€", "£", "¥", "¢", "$", "©", "®", "™", "✓", "★", "☆", "♥", "♦", "♣", "♠", "♪", "♫"]),
];

#[allow(clippy::cast_precision_loss)]
pub(crate) fn zoom_slider_share(percent: u32) -> f32 {
    let p = percent.clamp(50, 250);
    if p <= 100 { (p - 50) as f32 / 100.0 } else { 0.5 + (p - 100) as f32 / 300.0 }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(crate) fn zoom_slider_percent(share: f32) -> u32 {
    let share = share.clamp(0.0, 1.0);
    if (share - 0.5).abs() < 0.035 {
        100
    } else if share < 0.5 {
        50 + (share * 100.0).round() as u32
    } else {
        100 + ((share - 0.5) * 300.0).round() as u32
    }
}
/// The ArtCraft community chat. Opened in the browser, never fetched by the app.
#[allow(dead_code)]
const DISCORD_URL: &str = "https://discord.gg/artcraft";

/// Where the pen rack and the tool in hand are remembered between runs.
const RACK_KEY: &str = "pen_rack";
/// The size the whole window is drawn at, in percent: see `backstage::UI_SCALES`.
const UI_SCALE_KEY: &str = "ui_scale";
const NIB_KEY: &str = "pen_in_hand";
const SENSITIVITY_KEY: &str = "pen_sensitivity";
const DEFAULT_SENSITIVITY: u8 = 50;

fn default_telemetry_level(debug_build: bool) -> &'static str {
    if debug_build { "debug" } else { "none" }
}

const ROW_HEIGHT: f32 = 30.0;

/// Space between the end of a page name and its format label in the page list.
const ROW_LABEL_GAP: f32 = 6.0;

/// How wide `text` is when drawn on one line at `size` points.
fn text_width(ui: &Ui, text: &str, size: f32) -> f32 {
    ui.fonts_mut(|f| f.layout_no_wrap(text.to_owned(), egui::FontId::proportional(size), egui::Color32::WHITE).rect.width())
}

/// Whether a row's format label is shown, and how much room its name gets. The label is
/// shown only when the whole name fits beside it; otherwise the name gets the whole row.
fn row_name_room(name_width: f32, label_width: Option<f32>, room: f32) -> (bool, f32) {
    match label_width {
        Some(label) if name_width + label <= room => (true, room - label),
        _ => (false, room),
    }
}

/// `text` as it fits in `room` points: whole when it fits, otherwise cut at a character
/// and ended with "…", so a long name never runs into what is drawn after it.
fn fit_to_width(ui: &Ui, text: &str, size: f32, room: f32) -> String {
    if text_width(ui, text, size) <= room {
        return text.to_owned();
    }
    let ends: Vec<usize> = text.char_indices().map(|(i, _)| i).skip(1).chain(std::iter::once(text.len())).collect();
    // The longest start of the name that still fits with the ellipsis after it.
    let (mut lo, mut hi) = (0usize, ends.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let candidate = format!("{}…", text[..ends[mid - 1]].trim_end());
        if text_width(ui, &candidate, size) <= room { lo = mid } else { hi = mid - 1 }
    }
    if lo == 0 { "…".to_owned() } else { format!("{}…", text[..ends[lo - 1]].trim_end()) }
}
const INDENT: f32 = 14.0;

/// Shared title treatment for modal dialogs. Keeping this here makes dialogs use the
/// same opaque, theme-owned title band instead of inheriting egui's platform-gray title bar.
fn modal_frame(tokens: &Tokens) -> egui::Frame {
    egui::Frame::NONE.fill(tokens.panel).stroke(egui::Stroke::new(1.5, tokens.accent)).inner_margin(egui::Margin::same(16))
}

// i18n-ui-text: title
fn modal_title(ui: &mut Ui, tokens: &Tokens, title: &str) {
    egui::Frame::NONE.fill(tokens.band).inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(egui::RichText::new(crate::tl!(title)).strong().size(17.0).color(tokens.band_text));
    });
    ui.add_space(10.0);
}

/// Dims the window behind a modal dialog and takes the clicks that miss the dialog, so they do
/// not reach the page underneath. Returns the backdrop's layer: the dialog's window keeps itself
/// above it with [`stay_above`].
fn modal_backdrop(ctx: &egui::Context, id: &str) -> egui::LayerId {
    let screen = ctx.viewport_rect();
    egui::Area::new(egui::Id::new(id))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let response = ui.allocate_rect(egui::Rect::from_min_size(egui::Pos2::ZERO, screen.size()), egui::Sense::click());
            ui.painter().rect_filled(response.rect, 0.0, egui::Color32::from_black_alpha(150));
        })
        .response
        .layer_id
}

/// Keeps a modal dialog's window directly above its backdrop. Both are foreground areas, and egui
/// raises an area that is clicked to the top of its order: a click on the dimmed backdrop used to
/// lift it over the dialog, whose buttons could then never be reached again (issue #98).
fn stay_above(ui: &Ui, backdrop: egui::LayerId) {
    ui.ctx().set_sublayer(backdrop, ui.layer_id());
}

/// Modal actions use a readable size based on their label instead of egui's compact default.
// i18n-ui-text: label
fn modal_button(ui: &mut Ui, tokens: &Tokens, label: &str, primary: bool) -> egui::Response {
    modal_button_enabled(ui, tokens, label, primary, true)
}

// i18n-ui-text: label
fn modal_button_enabled(ui: &mut Ui, tokens: &Tokens, label: &str, primary: bool, enabled: bool) -> egui::Response {
    let label = crate::tl!(label);
    let width = (label.chars().count() as f32 * 8.0 + 32.0).max(92.0);
    let fill = if primary { tokens.accent } else { tokens.panel };
    let text = if primary { tokens.band_text } else { tokens.text };
    ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(label).size(14.0).color(text))
            .min_size(vec2(width, 32.0))
            .fill(fill)
            .stroke(egui::Stroke::new(1.0, tokens.accent)),
    )
}

fn params(pairs: &[(&str, &str)]) -> Params {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn path_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn missing_attachment_icon(page: &Path, src: &str) -> Option<String> {
    let name = src.strip_prefix("assets/")?;
    if name.is_empty() || name.contains('/') || name.contains('\\') {
        return None;
    }
    // Generated icon sidecars are images too, but they are not attachments that
    // need another icon. Treat `file.ext.svg`/`file.ext.png` as implementation
    // details when the attachment they belong to is present.
    if let Some((source, extension)) = name.rsplit_once('.')
        && matches!(extension.to_ascii_lowercase().as_str(), "png" | "svg")
        && ergonote_store::vfs::is_file(&page.join("assets").join(source))
    {
        return None;
    }
    if ergonote_store::assets::media_kind(src) != ergonote_store::assets::MediaKind::Other {
        return None;
    }
    // A missing attachment is not a missing-icon case. Do not warn about an icon
    // when the original file itself is no longer present.
    if !ergonote_store::vfs::is_file(&page.join("assets").join(name)) {
        return None;
    }
    (!attachment_icon_healthy(page, name)).then(|| name.to_string())
}

/// A generated PNG is preferred on Windows, but the deterministic SVG fallback is also a
/// complete icon. In particular, Shell extraction can fail for files on a UNC-mounted
/// enlistment even though the fallback renders correctly.
fn attachment_icon_healthy(page: &Path, name: &str) -> bool {
    attachment_icon_exists(page, name)
}

fn attachment_icon_exists(page: &Path, name: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        // Native Shell icons are PNG, but the deterministic SVG fallback is valid
        // when Shell extraction is unavailable on this Windows installation.
        ergonote_store::vfs::is_file(&page.join("assets").join(format!("{name}.png")))
            || ergonote_store::vfs::is_file(&page.join("assets").join(format!("{name}.svg")))
    }
    #[cfg(not(target_os = "windows"))]
    {
        ergonote_store::vfs::is_file(&page.join("assets").join(format!("{name}.svg")))
            || ergonote_store::vfs::is_file(&page.join("assets").join(format!("{name}.png")))
    }
}

fn attachment_icon_link(page: &Path, name: &str) -> String {
    let dir = page.join("assets");
    if ergonote_store::vfs::is_file(&dir.join(format!("{name}.png"))) { format!("assets/{name}.png") } else { format!("assets/{name}.svg") }
}

fn replace_attachment_icon(format: PageFormat, snippet: &str, name: &str, icon_link: &str) -> String {
    match format {
        PageFormat::Html => snippet
            .replace(&format!("src=\"assets/{name}.png\""), &format!("src=\"{icon_link}\""))
            .replace(&format!("src=\"assets/{name}.svg\""), &format!("src=\"{icon_link}\"")),
        PageFormat::Cmgfm => {
            let png = format!("](assets/{name}.png)");
            let svg = format!("](assets/{name}.svg)");
            snippet.replacen(&png, &format!("]({icon_link})"), 1).replacen(&svg, &format!("]({icon_link})"), 1)
        }
    }
}

#[cfg(test)]
mod attachment_reference_tests {
    use super::{PageFormat, replace_attachment_icon};

    #[cfg(not(target_os = "windows"))]
    use super::attachment_icon_exists;

    #[test]
    fn html_repair_changes_only_the_image_source() {
        let input = r#"<p><a href="assets/brief.docx"><img src="assets/brief.docx.svg" alt="" width="48"><br>brief.docx</a></p>"#;
        let output = replace_attachment_icon(PageFormat::Html, input, "brief.docx", "assets/brief.docx.png");
        assert!(output.contains(r#"href="assets/brief.docx""#));
        assert!(output.contains(r#"src="assets/brief.docx.png""#));
    }

    #[test]
    fn markdown_repair_changes_only_the_inner_image_link() {
        let input = "[![brief.docx](assets/brief.docx.svg) brief.docx](assets/brief.docx)";
        let output = replace_attachment_icon(PageFormat::Cmgfm, input, "brief.docx", "assets/brief.docx.png");
        assert_eq!(output, "[![brief.docx](assets/brief.docx.png) brief.docx](assets/brief.docx)");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn a_windows_png_sidecar_is_a_valid_icon_on_posix() {
        let t = ergonote_store::testing::TempDir::new("attachment-icon");
        let page = t.path().join("page");
        std::fs::create_dir_all(page.join("assets")).expect("assets");
        std::fs::write(page.join("assets/brief.docx"), b"attachment").expect("attachment");
        std::fs::write(page.join("assets/brief.docx.png"), b"png sidecar").expect("icon");
        assert!(attachment_icon_exists(&page, "brief.docx"));
    }
}

fn first_missing_attachment_in_source(source: &str, page: &Path) -> Option<String> {
    let mut rest = source;
    while let Some(at) = rest.find("assets/") {
        rest = &rest[at + "assets/".len()..];
        let end = rest.find(['"', '\'', ')', ']', '<', '>', ' ', '\n', '\r']).unwrap_or(rest.len());
        let name = &rest[..end];
        let candidate = format!("assets/{name}");
        if let Some(name) = (!name.is_empty()).then(|| missing_attachment_icon(page, &candidate)).flatten() {
            return Some(name);
        }
        rest = &rest[end..];
    }
    None
}

#[allow(dead_code)]
fn first_unreferenced_attachment(attachments: &[(String, u64)], source: &str) -> Option<String> {
    attachments.iter().find_map(|(name, _)| {
        let reference = format!("assets/{name}");
        (!source.contains(&reference)).then(|| name.clone())
    })
}

#[allow(dead_code)]
fn prepare_unreferenced_attachment(page: &Path, name: &str) -> bool {
    match ergonote_store::assets::ensure_icon(page, name) {
        Ok(()) => {
            log::debug!("generated icon before detached attachment warning: page={:?} attachment={name:?}", page);
            true
        }
        Err(error) => {
            log::error!("could not generate icon before detached attachment warning: page={:?} attachment={name:?} error={error}", page);
            false
        }
    }
}

fn first_legacy_attachment_in_source(source: &str) -> Option<String> {
    let mut rest = source;
    while let Some(at) = rest.find("assets/") {
        rest = &rest[at + "assets/".len()..];
        let end = rest.find(['"', '\'', ')', ']', '<', '>', ' ', '\n', '\r']).unwrap_or(rest.len());
        let name = &rest[..end];
        if !name.is_empty()
            && !name.ends_with(".svg")
            && ergonote_store::assets::media_kind(&format!("assets/{name}")) == ergonote_store::assets::MediaKind::Other
            && (source.contains(&format!("<a href=\"assets/{name}\">")) || source.contains(&format!("[{name}](assets/{name})")))
        {
            return Some(name.to_string());
        }
        rest = &rest[end..];
    }
    None
}

fn shown(p: &Path) -> String {
    ergonote_store::tree::display_path(p)
}

/// The widest empty stretch Collect All leaves between things on a page, in page points.
const COLLECT_GAP: f32 = 24.0;
/// The Insert Space tool's own layer over the visible page (issue #164): a vertical-resize
/// pointer, a guide at the pointer's height, and while it is dragged, the band of room being
/// added (below the start) or taken away (above it). Returns where the drag began and how far it
/// went, in screen points, on the frame it is let go.
fn insert_space_overlay(ctx: &Context, visible: egui::Rect, t: &Tokens) -> Option<(f32, f32)> {
    let id = Id::new("insert-space-tool");
    let from_id = id.with("from");
    let mut done = None;
    egui::Area::new(id).order(egui::Order::Foreground).fixed_pos(visible.min).constrain(false).movable(false).show(ctx, |ui| {
        let (rect, resp) = ui.allocate_exact_size(visible.size(), Sense::drag());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, crate::tl_id!("ui.insert_space_how")));
        if resp.hovered() || resp.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
        if resp.drag_started()
            && let Some(p) = resp.interact_pointer_pos()
        {
            ui.data_mut(|d| d.insert_temp(from_id, p.y));
        }
        let from = ui.data(|d| d.get_temp::<f32>(from_id));
        let now = ui.ctx().pointer_latest_pos().filter(|p| rect.contains(*p) || resp.dragged()).map(|p| p.y.clamp(rect.min.y, rect.max.y));
        let painter = ui.painter();
        let line = egui::Stroke::new(1.5, t.accent);
        match (from, now) {
            (Some(a), Some(b)) => {
                let band = egui::Rect::from_x_y_ranges(rect.x_range(), egui::Rangef::new(a.min(b), a.max(b)));
                painter.rect_filled(band, 0.0, t.accent.gamma_multiply(0.12));
                painter.hline(rect.x_range(), a, line);
                painter.hline(rect.x_range(), b, egui::Stroke::new(1.0, t.accent.gamma_multiply(0.7)));
            }
            (None, Some(y)) => {
                painter.hline(rect.x_range(), y, line);
            }
            _ => {}
        }
        if resp.drag_stopped() {
            if let (Some(a), Some(p)) = (from, resp.interact_pointer_pos()) {
                done = Some((a, p.y.clamp(rect.min.y, rect.max.y) - a));
            }
            ui.data_mut(|d| d.remove::<f32>(from_id));
        }
    });
    done
}

/// Overlay for placing a new floating text box anywhere on the page (including on top of a PDF or picture).
fn insert_textbox_overlay(ctx: &Context, visible: egui::Rect, t: &Tokens, zoom: f32) -> Option<egui::Pos2> {
    let id = Id::new("insert-textbox-tool");
    let mut clicked_at = None;
    egui::Area::new(id).order(egui::Order::Foreground).fixed_pos(visible.min).constrain(false).movable(false).show(ctx, |ui| {
        let (rect, resp) = ui.allocate_exact_size(visible.size(), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, crate::tl_id!("ui.student_textbox_prompt")));
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            if let Some(pos) = ui.ctx().pointer_latest_pos().filter(|p| rect.contains(*p)) {
                let preview = egui::Rect::from_min_size(pos, egui::vec2(260.0 * zoom, 38.0 * zoom));
                let painter = ui.painter();
                painter.rect_filled(preview, 4.0, t.editor.gamma_multiply(0.88));
                painter.rect_stroke(preview, 4.0, egui::Stroke::new(2.0, t.accent), egui::StrokeKind::Inside);
                painter.text(
                    egui::pos2(preview.min.x + 10.0 * zoom, preview.center().y),
                    egui::Align2::LEFT_CENTER,
                    crate::tl_id!("ui.student_textbox_label").replace('\n', " "),
                    egui::FontId::proportional(13.0 * zoom),
                    t.accent,
                );
            }
        }
        if resp.clicked() {
            clicked_at = resp.interact_pointer_pos().or_else(|| ui.ctx().pointer_latest_pos());
        }
    });
    clicked_at
}

/// How far back History > Recent Edits looks (issue #279).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecentRange {
    Today,
    Week,
    Month,
    All,
}

impl RecentRange {
    pub const ALL: [RecentRange; 4] = [RecentRange::Today, RecentRange::Week, RecentRange::Month, RecentRange::All];

    fn label(self) -> &'static str {
        match self {
            RecentRange::Today => crate::tl_id!("ui.today"),
            RecentRange::Week => crate::tl_id!("ui.last_7_days"),
            RecentRange::Month => crate::tl_id!("ui.last_30_days"),
            RecentRange::All => crate::tl_id!("ui.all_time"),
        }
    }

    /// The earliest moment in the range, seconds since 1970, for `now` on a clock `offset`
    /// seconds ahead of UTC: today begins at the user's own midnight.
    pub fn since(self, now: i64, offset: i32) -> Option<i64> {
        let midnight = now - (now + i64::from(offset)).rem_euclid(86_400);
        match self {
            RecentRange::Today => Some(midnight),
            RecentRange::Week => Some(midnight - 6 * 86_400),
            RecentRange::Month => Some(midnight - 29 * 86_400),
            RecentRange::All => None,
        }
    }
}

fn notebook_for(path: &Path) -> Option<PathBuf> {
    let mut current = path;
    loop {
        if ergonote_store::tree::read_node(current).is_some_and(|node| node.kind == NodeKind::Notebook) {
            return Some(current.to_path_buf());
        }
        current = current.parent()?;
    }
}

/// The path as the window may show it: redacted to `<notebooks>/relative/path` when
/// `redact` is on (for screenshots, tests and documentation, so no local path leaks).
fn shown_in(root: &Path, p: &Path, redact: bool) -> String {
    if !redact {
        return shown(p);
    }
    match p.strip_prefix(root) {
        Ok(rel) if rel.as_os_str().is_empty() => "<notebooks folder>".to_string(),
        Ok(rel) => format!("<notebooks>/{}", rel.to_string_lossy().replace('\\', "/")),
        Err(_) => "<outside>".to_string(),
    }
}

/// Where the Tags Summary looks for tagged notes (issue #166), widest last.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TagScope {
    Page,
    Section,
    Notebook,
    All,
}

impl TagScope {
    const ALL: [TagScope; 4] = [TagScope::Page, TagScope::Section, TagScope::Notebook, TagScope::All];

    fn label(self) -> &'static str {
        match self {
            TagScope::Page => crate::tl_id!("ui.tags_scope_this_page"),
            TagScope::Section => crate::tl_id!("ui.tags_scope_this_section"),
            TagScope::Notebook => crate::tl_id!("ui.tags_scope_this_notebook"),
            TagScope::All => crate::tl_id!("ui.tags_scope_all_notebooks"),
        }
    }

    fn id(self) -> &'static str {
        match self {
            TagScope::Page => "page",
            TagScope::Section => "section",
            TagScope::Notebook => "notebook",
            TagScope::All => "all",
        }
    }
}

/// The Tags Summary pane, OneNote's Find Tags: the tagged notes in a scope, grouped by tag,
/// each one a way back to its line. It lists what the pages said when it last looked; Refresh
/// looks again, as OneNote's does.
struct TagsPane {
    scope: TagScope,
    /// Only the To Do items not yet ticked.
    unchecked: bool,
    found: Vec<ergonote_store::tags::Tagged>,
}

/// The Page Templates pane (issue #173): every template, by category, each one a new page in
/// the section; and a way to keep the open page as a template of its own.
struct TemplatesPane {
    templates: Vec<ergonote_store::page_templates::Template>,
    /// What the open page will be called as a template.
    name: String,
}

/// The section a page is in: the nearest folder above it that is a section.
fn section_for(path: &Path) -> Option<PathBuf> {
    let mut current = path.parent()?;
    loop {
        match ergonote_store::tree::read_node(current).map(|node| node.kind) {
            Some(NodeKind::Section) => return Some(current.to_path_buf()),
            Some(NodeKind::Page) => {}
            _ => return None,
        }
        current = current.parent()?;
    }
}

/// What the Format Painter holds: the formatting it picked up, where it picked it up (so the
/// selection it was picked up from is not painted with itself), and whether it keeps painting.
#[derive(Clone, Debug)]
struct FormatPaint {
    style: ergonote_formats::Span,
    from: (usize, (usize, usize)),
    sticky: bool,
}

/// Pending text wrap returned to an asynchronous platform IME (Android) so fast follow-up
/// keystrokes arriving against the pre-wrap buffer generation can be wrapped without losing input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImeRewrite {
    pub block: usize,
    pub prefix: String,
    pub suffix: String,
    pub open: String,
    pub close: String,
}

#[derive(Clone, Debug, Default)]
struct PendingFormat {
    block: usize,
    at: usize,
    size: Option<u8>,
    family: Option<String>,
    bold: bool,
    italic: bool,
    strike: bool,
    underline: bool,
    underline_color: Option<String>,
    text_color: Option<String>,
    highlight_color: Option<String>,
    script: Option<ergonote_formats::markdown::Script>,
}

struct Editor {
    dir: PathBuf,
    format: PageFormat,
    title: String,
    title_buf: String,
    /// The page has a title of its own; `title` is only the fallback name when it has not.
    titled: bool,
    body: String,
    /// What the page's file holds, as last read or written, so a save that would not change it
    /// is not made: a save rewrites the file, which on a synced folder can take a second.
    saved_body: String,
    /// The text an autosave is writing in the background, until it is written.
    saving: Option<String>,
    dirty_since: Option<Instant>,
    blocks: Vec<ergonote_formats::Block>,
    last_saved: Option<String>,
    attachments: Vec<(String, u64)>,
    /// Where the caret is in the page, and the text of that block while it is written in.
    caret: Option<pageedit::Caret>,
    /// Formatting chosen at a collapsed caret, before any text has been inserted.
    pending_format: Option<PendingFormat>,
    restore_format_selection: Option<(usize, usize, usize)>,
    /// False when the page holds markup the block editor would drop: it is then read here
    /// and changed in its source.
    writable: bool,
    /// Which way the whole page reads, if the writer said so. A line with no letters of its
    /// own follows it.
    page_dir: Option<ergonote_model::Dir>,
    /// The boxes the page's blocks are arranged in, as the page's own file keeps them.
    boxes: Vec<ergonote_model::Container>,
    /// How wide the columns of this page's tables are drawn, from the same place.
    columns: ergonote_model::tablecols::Widths,
    /// Which paragraphs of this page are stretched to fill the column, from the same place.
    /// The words themselves are never touched; see `ergonote_model::justify`.
    justified: ergonote_model::justify::Justified,
    /// A selection dragged across more than one block. A caret belongs to one block, so a
    /// selection that leaves it is kept here; `None` is the ordinary case.
    across: Option<pageedit::Across>,
    /// The paper this page is drawn on and the colour behind it, as the page's own file
    /// keeps them. `None` means the page says nothing and the window's own choice stands.
    paper: Option<ribbon::RuleLines>,
    colour: Option<ergonote_model::NodeColor>,
    /// The colour of this page's rule and grid lines, when it was given one (issue #288).
    rule_colour: Option<ergonote_model::NodeColor>,
    /// The sheet the page is written for; `None` is Auto (issue #269).
    paper_size: Option<ergonote_model::paper::Paper>,
    /// What the page looked like before each change, so a change can be taken back.
    history: crate::history::History,
    /// The ink files in the page's folder. The folder is what decides: the page's own
    /// text is kept in step with this, never the other way round.
    ink_files: Vec<String>,
    /// Where the page's area starts on screen, reported so a script can find a box.
    origin: egui::Pos2,
    /// Where the page's origin sits in the panned canvas, from the last frame it was drawn. The
    /// paper's lines are drawn through it, so they stay with the page and its boxes (issue #79).
    origin_in_canvas: Option<egui::Vec2>,
    /// Whether the boxes are written into the page's text. A page nobody arranged stays plain
    /// text, one column; moving, resizing or adding a box arranges it, and from then on the
    /// boxes are elements around the blocks.
    arranged: bool,
    /// How far down the page the containers reached when last drawn, in page points.
    content_bottom: Option<f32>,
    /// How wide each box was drawn last frame, in page points (`containers::Moved::shown_widths`).
    shown_widths: Vec<f32>,
}

impl Editor {
    /// `count` blocks were just put into the page at `at`. The box that holds the block before
    /// them counts them too (the first box, for blocks put in at the top), so the boxes still
    /// add up to the page's blocks and `container::fit` keeps the arrangement instead of
    /// folding it into one box (issue #72). Call it before [`Editor::rebuild_body`].
    fn count_inserted_in_boxes(&mut self, at: usize, count: usize) {
        let counted: usize = self.boxes.iter().map(|c| c.blocks).sum();
        if count == 0 || self.boxes.is_empty() || counted + count != self.blocks.len() {
            return;
        }
        let owner = if at == 0 { Some(0) } else { ergonote_model::container::place(&self.boxes, at - 1).map(|(i, _)| i) };
        if let Some(i) = owner {
            self.boxes[i].blocks += count;
        }
    }

    /// The block at `at` was just taken out of the page: its box counts one fewer.
    fn count_removed_from_boxes(&mut self, at: usize) {
        let counted: usize = self.boxes.iter().map(|c| c.blocks).sum();
        if counted != self.blocks.len() + 1 {
            return;
        }
        if let Some((i, _)) = ergonote_model::container::place(&self.boxes, at) {
            self.boxes[i].blocks -= 1;
        }
    }

    /// [`Editor::count_inserted_in_boxes`], and justified paragraphs and table widths after the
    /// new blocks move down with their blocks. Returns whether the widths and the justified
    /// paragraphs changed, for [`ErgoNoteApp::save_block_marks`].
    fn blocks_inserted(&mut self, at: usize, count: usize) -> (bool, bool) {
        self.count_inserted_in_boxes(at, count);
        let (mut cols, mut justified) = (false, false);
        for offset in 0..count {
            cols |= self.columns.insert_block(at + offset);
            justified |= self.justified.insert_block(at + offset);
        }
        (cols, justified)
    }

    /// Writes the page's text from its blocks, with the boxes around them when it is arranged.
    /// The page as it is written to its file. A box with nothing in it, last in reading order
    /// (a click on open paper adds its line at the end), is only where the cursor was put, not
    /// something on the page, so it is left out.
    fn body_to_save(&self) -> String {
        if !self.arranged || self.boxes.len() < 2 || self.boxes.iter().map(|c| c.blocks).sum::<usize>() != self.blocks.len() {
            return self.body.clone();
        }
        let last = (0..self.boxes.len()).max_by_key(|i| ergonote_model::container::blocks_of(&self.boxes, *i).end).unwrap_or(0);
        let range = ergonote_model::container::blocks_of(&self.boxes, last);
        let blank = |b: &ergonote_formats::Block| crate::preview::is_blank(b) && !matches!(b, ergonote_formats::Block::Item { task: Some(_), .. });
        if range.end != self.blocks.len() || range.is_empty() || !self.blocks[range.clone()].iter().all(blank) {
            return self.body.clone();
        }
        let mut boxes = self.boxes.clone();
        boxes.remove(last);
        ergonote_formats::boxes::write(self.format, &self.blocks[..range.start], Some(&boxes))
    }

    fn rebuild_body(&mut self) {
        let arrangement = (self.arranged && !self.boxes.is_empty()).then_some(self.boxes.as_slice());
        self.body = ergonote_formats::boxes::write(self.format, &self.blocks, arrangement);
    }

    fn apply_pending_format(&mut self, before: Option<&pageedit::Caret>) -> Option<ImeRewrite> {
        let pending = self.pending_format.take()?;
        let (block, at) = (pending.block, pending.at);
        let (Some(before), Some(caret)) = (before, self.caret.as_mut()) else {
            return None;
        };
        if before.block != block || caret.block != block || before.selection != (at, at) || caret.cell.is_some() {
            return None;
        }
        let offset = before.text.char_indices().nth(at).map_or(before.text.len(), |(offset, _)| offset);
        let (prefix, suffix) = before.text.split_at(offset);
        let inserted = caret.text.strip_prefix(prefix).and_then(|text| text.strip_suffix(suffix));
        let Some(inserted) = inserted.filter(|text| !text.is_empty()) else {
            self.pending_format = Some(pending);
            return None;
        };
        let end = at + inserted.chars().count();
        if caret.selection != (end, end) {
            return None;
        }
        if inserted.chars().all(char::is_whitespace) {
            let mut kept = pending;
            kept.at = end;
            self.pending_format = Some(kept);
            return None;
        }
        let trimmed_start = inserted.trim_start_matches(char::is_whitespace);
        let lead = &inserted[..inserted.len() - trimmed_start.len()];
        let core = trimmed_start.trim_end_matches(char::is_whitespace);
        let trail = &trimmed_start[core.len()..];
        let lead_chars = lead.chars().count();
        let core_chars = core.chars().count();
        let trail_chars = trail.chars().count();
        let wrap_start = at + lead_chars;
        let wrap_end = wrap_start + core_chars;

        let mut marker = "ErgoNotePendingTextMarker".to_string();
        while pending.family.as_ref().is_some_and(|family| family.contains(&marker)) {
            marker.push('X');
        }
        let pending_snapshot = pending.clone();
        let span = ergonote_formats::markdown::Span {
            text: marker.clone(),
            font_size: pending.size,
            font_family: pending.family,
            bold: pending.bold,
            italic: pending.italic,
            strike: pending.strike,
            underline: pending.underline,
            underline_color: pending.underline_color,
            text_color: pending.text_color,
            highlight_color: pending.highlight_color,
            script: pending.script,
            ..Default::default()
        };
        let template = ergonote_formats::render::spans_to_markdown(&[span]);
        let Some((open, close)) = template.split_once(&marker) else {
            log::error!("Could not serialize pending formatting");
            return None;
        };
        let (text, selection) = pageedit::wrap_selection_with(&caret.text, (wrap_start, wrap_end), open, close);
        if trail_chars > 0 {
            let new_at = selection.1 + close.chars().count() + trail_chars;
            caret.text = text;
            caret.selection = (new_at, new_at);
            caret.focus = true;
            caret.place = Some(new_at);
            self.pending_format = Some(PendingFormat { block, at: new_at, ..pending_snapshot });
            if let Some(target) = self.blocks.get_mut(block) {
                caret.write(target);
            }
            return None;
        }
        let rewrite_prefix = format!("{prefix}{lead}");
        caret.text = text;
        caret.selection = (selection.1, selection.1);
        caret.focus = true;
        caret.place = Some(selection.1);
        if let Some(target) = self.blocks.get_mut(block) {
            caret.write(target);
        }
        Some(ImeRewrite { block, prefix: rewrite_prefix, suffix: suffix.to_string(), open: open.to_string(), close: close.to_string() })
    }
}

/// The page's own name, drawn on the sheet at the top of the page.
///
/// It used to sit in a strip above the page, beside the breadcrumb and the Saved line,
/// which made it look like a field about the page rather than the first thing on it. In
/// OneNote the name is part of the page, and the owner asked for the same here on
/// 2026-10-19. So it is drawn inside the scroll area and inside the readable column: it
/// scrolls with the words, it lines up with them, and the page colour and rule lines run
/// behind it as they do behind everything else on the sheet.
///
/// It is drawn before the sheet's origin is taken, so ink keeps the coordinates it already
/// has and a drawing made beside a paragraph stays beside it.
///
/// A title that reads right to left is drawn by the bidi painter rather than the text box:
/// egui lays the letters of a word out the right way and then puts the next word to the
/// right of it, so every Arabic page showed its own name with the words backwards. Clicking
/// it hands the line to the text box, which is still where it is edited.
fn page_title(ui: &mut Ui, ed: &mut Editor, tokens: &Tokens, title_id: egui::Id, rename: &mut Option<String>, zoom: f32) {
    let align = crate::pagetext::halign(&ed.title_buf, ed.page_dir);
    let width = ui.available_width();
    let rtl = crate::bidi::paints(&ed.title_buf, ed.page_dir);
    let editing = ui.memory(|m| m.has_focus(title_id)) || !rtl;
    let r = if editing {
        // A box that wraps (issue #316): a single-line box scrolls sideways, so a long title
        // was cut off at the page's edge on a phone. The title is still one line of text:
        // Enter finishes it rather than breaking it, and a line break pasted in becomes a space.
        let finish = ui.memory(|m| m.has_focus(title_id)) && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
        let out = egui::TextEdit::multiline(&mut ed.title_buf)
            .id(title_id)
            .frame(egui::Frame::NONE)
            .font(egui::FontId::proportional(26.0 * zoom))
            .horizontal_align(align)
            .desired_width(width)
            .desired_rows(1)
            .return_key(None)
            .show(ui);
        // A text value is not an accessible name. Preserve TextEdit's role, value and
        // selection while naming the field for screen readers, including an empty title.
        ui.ctx().accesskit_node_builder(out.response.id, |node| node.set_label(crate::tl_id!("ui.title_2")));
        if ed.title_buf.contains(['\n', '\r']) {
            ed.title_buf = one_line(&ed.title_buf);
        }
        if finish {
            out.response.surrender_focus();
        }
        if out.response.has_focus()
            && crate::input_language::is_rtl()
            && let Some(range) = out.cursor_range
        {
            let caret = out.galley.pos_from_cursor(range.primary).translate(out.galley_pos.to_vec2());
            crate::pageedit::paint_rtl_input_marker(ui, caret);
        }
        out.response
    } else {
        let pieces = crate::bidi::plain(&ed.title_buf, 26.0 * zoom, tokens);
        let (drawn, size) = crate::bidi::runs(ui, &ed.title_buf, &pieces, 26.0 * zoom, width, ed.page_dir);
        let (rect, resp) = ui.allocate_exact_size(vec2(width, size.y.max(34.0)), egui::Sense::click());
        crate::bidi::paint(ui, rect.min, &drawn);
        if resp.clicked() {
            ui.memory_mut(|m| m.request_focus(title_id));
        }
        let name = ed.title_buf.clone();
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextInput, true, &name));
        egui::AtomLayoutResponse::empty(resp.on_hover_cursor(egui::CursorIcon::Text))
    };
    if r.lost_focus() {
        let typed = ed.title_buf.trim();
        if typed.is_empty() {
            // Clearing the title of a page that had one makes it untitled, as the blank title
            // area shows; it used to be dropped, and the old title came back on reopen (#132).
            if ed.titled {
                *rename = Some(String::new());
            }
        } else if !ed.titled || typed != ed.title {
            *rename = Some(typed.to_string());
        }
    }
    // The rule under the name, as OneNote draws one. The date the page carries sits under
    // it already, as the first block of the page.
    let y = ui.cursor().min.y + 2.0;
    let (left, right) = (ui.max_rect().min.x, ui.max_rect().max.x);
    ui.painter().hline(left..=right, y, egui::Stroke::new(1.0, tokens.faint));
    ui.add_space(8.0);
}

use pageedit::{TABLE_MAX_COLUMNS, TABLE_MAX_ROWS};

/// The rows and columns typed into Insert Table, when both are whole numbers within the limits.
/// One value typed into the Paragraph Spacing dialog: a whole number of points up to the most a
/// page may ask for. `Ok(None)` for an empty field, which keeps what each paragraph has.
fn spacing_points(text: &str) -> Result<Option<u16>, ()> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    text.parse::<u16>().ok().filter(|v| *v <= ergonote_model::justify::MAX_POINTS).map(Some).ok_or(())
}

fn table_size(rows: &str, columns: &str) -> Option<(usize, usize)> {
    let r = rows.trim().parse::<usize>().ok().filter(|r| (1..=TABLE_MAX_ROWS).contains(r))?;
    let c = columns.trim().parse::<usize>().ok().filter(|c| (1..=TABLE_MAX_COLUMNS).contains(c))?;
    Some((r, c))
}

/// A title as one line: the line breaks a paste brought in become single spaces.
fn one_line(text: &str) -> String {
    text.split(['\n', '\r']).map(str::trim).filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" ")
}

/// What to do with a file the user dropped on a page.
/// What one Undo took back: the drawing, the page's words and boxes, or both at once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Undone {
    Ink,
    Page,
    Both,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FileChoice {
    /// Copy it into the page's folder, so the notebook carries it. OneNote: "Attach File".
    Copy,
    /// Leave it where it is and write a link. OneNote: "Insert a link to the file".
    Link,
    /// Copy it in and insert one rendered image for each printed page.
    Printout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TtsState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum TtsSpeed {
    Slow,
    #[default]
    Normal,
    Fast,
}

#[allow(dead_code)]
impl TtsSpeed {
    const ALL: [TtsSpeed; 3] = [TtsSpeed::Slow, TtsSpeed::Normal, TtsSpeed::Fast];

    fn label(self) -> &'static str {
        match self {
            TtsSpeed::Slow => "0,75×",
            TtsSpeed::Normal => "1×",
            TtsSpeed::Fast => "1,25×",
        }
    }

    fn words_per_second(self) -> f32 {
        match self {
            TtsSpeed::Slow => 1.85,
            TtsSpeed::Normal => 2.45,
            TtsSpeed::Fast => 3.15,
        }
    }

    fn edge_rate(self) -> &'static str {
        match self {
            TtsSpeed::Slow => "-25%",
            TtsSpeed::Normal => "-5%",
            TtsSpeed::Fast => "+22%",
        }
    }

    fn piper_length_scale(self) -> &'static str {
        match self {
            TtsSpeed::Slow => "1.30",
            TtsSpeed::Normal => "1.05",
            TtsSpeed::Fast => "0.82",
        }
    }

    fn spd_rate(self) -> &'static str {
        match self {
            TtsSpeed::Slow => "-25",
            TtsSpeed::Normal => "0",
            TtsSpeed::Fast => "25",
        }
    }

    fn espeak_wpm(self) -> &'static str {
        match self {
            TtsSpeed::Slow => "125",
            TtsSpeed::Normal => "165",
            TtsSpeed::Fast => "210",
        }
    }

    fn win_rate(self) -> i32 {
        match self {
            TtsSpeed::Slow => -2,
            TtsSpeed::Normal => 0,
            TtsSpeed::Fast => 3,
        }
    }

    fn say_wpm(self) -> &'static str {
        match self {
            TtsSpeed::Slow => "135",
            TtsSpeed::Normal => "175",
            TtsSpeed::Fast => "220",
        }
    }
}

#[derive(Clone, Debug)]
struct TtsWordCue {
    word: String,
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    char_start: usize,
    start_sec: f32,
    #[cfg_attr(target_os = "android", allow(dead_code))]
    end_sec: f32,
    block: Option<usize>,
    caret_chars: (usize, usize),
    drawn_bytes: (usize, usize),
}

#[derive(Clone, Debug, Default)]
struct TtsSync {
    preparing: bool,
    playback_started_at: Option<Instant>,
    elapsed_before_pause: f32,
    cues: Vec<TtsWordCue>,
    direct_word_index: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DictationState {
    Idle,
    Listening,
}

struct OcrDone {
    block: usize,
    speak: bool,
    result: Result<crate::ocr::OcrDocument, String>,
}

enum Dialog {
    /// A dropped file: copy it in, or link to it where it is.
    FileOptions {
        dir: PathBuf,
        files: Vec<PathBuf>,
    },
    NewNotebook {
        name: String,
        format: Option<PageFormat>,
    },
    Add {
        parent: PathBuf,
        parent_kind: NodeKind,
        title: String,
        kind: NodeKind,
        format: Option<PageFormat>,
    },
    Rename {
        path: PathBuf,
        title: String,
    },
    /// Move or Copy (issue #167): where a page, section or section group goes.
    MoveOrCopy {
        path: PathBuf,
        title: String,
        kind: NodeKind,
        dest: Option<PathBuf>,
    },
    /// Insert > Link, or editing the link the caret is in (issue #163): the words to show
    /// and the address, put into the line `block` (or its table cell) in place of the
    /// characters `range` of the caret's text.
    Link {
        block: usize,
        cell: Option<(usize, usize)>,
        range: (usize, usize),
        text: String,
        address: String,
        editing: bool,
        first: bool,
    },
    /// Insert > Table > Insert Table...: a table of any size up to the limits, typed in.
    InsertTable {
        rows: String,
        columns: String,
        first: bool,
    },
    /// Home > Alignment and direction > Paragraph Spacing Options (issue #275): the room before
    /// and after the paragraphs that were chosen, and the least height of their lines, in
    /// points. A field left empty keeps what each paragraph has, which is how a mixed value shows.
    ParagraphSpacing {
        blocks: Vec<usize>,
        before: String,
        after: String,
        line: String,
        first: bool,
    },
    /// A file in the page's own folder, about to be deleted.
    DeleteFile {
        page: PathBuf,
        name: String,
    },
    Delete {
        path: PathBuf,
        title: String,
        count: usize,
    },
    DeleteNotebook {
        path: PathBuf,
        title: String,
    },
    DeleteTable {
        block: usize,
    },
    CropPicture {
        block: usize,
        src: String,
        bytes: Vec<u8>,
        dims: (u32, u32),
        rect: [f32; 4],
    },
    OcrResult {
        block: usize,
        doc: crate::ocr::OcrDocument,
        text: String,
    },
    ClearTelemetry {
        path: PathBuf,
    },
    ResetPreferences,
    /// History > Recent Edits (issue #279).
    RecentEdits {
        range: RecentRange,
        /// Only the notebook the open page is in, rather than every open notebook.
        this_notebook: bool,
        filter: String,
        items: Vec<ergonote_store::recent::Edited>,
    },
    PageVersions {
        page: PathBuf,
        versions: Vec<ergonote_store::page::Version>,
        /// A saved version opened for reading, before any Restore (issue #278).
        preview: Option<Box<crate::versions::Preview>>,
    },
    RecycleBin {
        notebook: PathBuf,
        items: Vec<ergonote_store::recycle::Recycled>,
    },
    PurgeRecycleItem {
        notebook: PathBuf,
        id: String,
        title: String,
    },
    Symbols,
    /// The first-use wizard: one window with three steps. Step 0 is the local logging level,
    /// step 1 the page format saved as the default, and step 2 whether to add the Welcome notebook
    /// in that format.
    FirstUseWizard {
        step: u8,
        level: String,
        format: PageFormat,
        add_demo: bool,
        error: Option<String>,
    },
    /// A problem told to the user in a box.    /// A problem told to the user in a box. Ticking "do not alert me again" stops the box for
    /// this `key` for the rest of the session; the status line still says what happened.
    Alert {
        key: String,
        title: String,
        message: String,
        suppress: bool,
        suppress_forever: bool,
    },
    MissingAttachmentIcon {
        page: PathBuf,
        name: String,
    },
    OpenAttachmentWarning {
        path: PathBuf,
    },
    #[allow(dead_code)]
    UnreferencedAttachment {
        page: PathBuf,
        name: String,
    },
    BackupRecovery {
        feedback: Option<String>,
    },
    OrthographyCheck {
        analyzed_text: String,
        target_blocks: Vec<(usize, usize, String)>,
        issues: Vec<(usize, crate::spellcheck::GrammarIssue)>,
        online_checked: bool,
    },
    SupportCreator,
    ReportBug,
    ReleaseNotes,
}

struct ImportJob {
    rx: Receiver<ImportMessage>,
    keep_alive: Option<Sender<ImportMessage>>,
    cancel: Arc<AtomicBool>,
    source: PathBuf,
    destination: PathBuf,
    current: String,
    index: usize,
    total: usize,
}

enum ImportMessage {
    Found(usize),
    Destination(PathBuf),
    Section { index: usize, total: usize, label: String },
    Finished(Result<String, String>),
}

enum Act {
    Toggle(PathBuf),
    Select(PathBuf),
    Add(PathBuf, NodeKind),
    /// A page in this section at this place in the order, from the line between two pages.
    AddPageAt(PathBuf, u32),
    /// The same, when the two pages either side of the line are numbered next to each other:
    /// the section is renumbered ten apart first, then the page goes halfway after the first one.
    RenumberAndAddPage(PathBuf, u32),
    NewPage(PathBuf),
    /// Add a child of this kind, named in the menu rather than chosen in the dialog.
    AddKind(PathBuf, NodeKind, NodeKind),
    /// A notebook, from the menu on the empty space.
    NewNotebook,
    /// Close a notebook opened from outside the notebooks folder.
    CloseNotebook(PathBuf),
    /// Show the notebooks folder in the file manager.
    RevealRoot,
    /// Ask the system for the folder to work from.
    ChooseRoot,
    Rename(PathBuf, String),
    /// Open Move or Copy for a page, section or section group.
    MoveOrCopy(PathBuf),
    MakeSubpage(PathBuf),
    PromoteSubpage(PathBuf),
    MovePageUp(PathBuf),
    MovePageDown(PathBuf),
    PageVersions(PathBuf),
    Delete(PathBuf, String, usize),
    Color(PathBuf, Option<NodeColor>),
    /// Show the node's folder in the operating system's file manager.
    Reveal(PathBuf),
    /// Put a link to this page on the clipboard, to paste into another page.
    CopyLink(PathBuf),
}

/// A file the UI's host should deliver through its download facility.
/// The desktop host keeps the normal file instead; browser hosts opt in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileDownload {
    pub name: String,
    pub mime: &'static str,
    pub bytes: Vec<u8>,
}

/// Non-blocking action queued when an Android native document picker completes.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
enum AndroidPickKind {
    Picture(PathBuf),
    BoardPhoto(PathBuf),
    Printout(PathBuf),
    File(PathBuf),
    ImportZip,
}

/// Sanitizes a page title into a portable export file stem.
fn export_safe_title(raw: &str) -> String {
    let trimmed = if raw.trim().is_empty() { "Cours" } else { raw.trim() };
    let safe: String = trimmed.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
    let clean = safe.trim_matches('-');
    if clean.is_empty() { "Cours".to_string() } else { clean.to_string() }
}

fn char_to_utf16(text: &str, char_idx: usize) -> usize {
    text.chars().take(char_idx).map(char::len_utf16).sum()
}

fn utf16_to_char(text: &str, utf16_idx: usize) -> usize {
    let mut u16_pos = 0usize;
    let mut chars = 0usize;
    for ch in text.chars() {
        let len = ch.len_utf16();
        if u16_pos + len > utf16_idx {
            break;
        }
        u16_pos += len;
        chars += 1;
    }
    chars
}

pub struct ErgoNoteApp {
    session: Session,
    /// Writes autosaves off the window's thread (see [`crate::saver`]).
    saver: crate::saver::Saver,
    open: HashSet<PathBuf>,
    selected: Option<PathBuf>,
    /// The pages visited, for Back and Forward.
    nav: NavHistory,
    /// Which Quick Access buttons are shown, and where.
    qat: quickaccess::Config,
    /// When the pen last changed the page, to tell which Undo should take back.
    last_ink_change: Option<Instant>,
    /// What each Undo took back, newest last, so Redo puts it back the same way.
    undone: Vec<Undone>,
    /// Two columns in the side panel (organisers, then pages) instead of one tree.
    split_panes: bool,
    /// Show a horizontal bar of section tabs (Français, Maths...) below the ribbon.
    section_tabs_bar: bool,
    /// Lock PDF / image containers in background against accidental drag/resize.
    lock_images: bool,
    /// Enable wider letter and line spacing for dyslexic readers.
    dys_spaced: bool,
    /// Show the horizontal reading ruler following the pointer across the page.
    reading_ruler: bool,
    /// Last pointer Y coordinate inside the page viewport for the reading ruler.
    reading_ruler_y: Option<f32>,
    /// Current visible sheet and total sheets on the active page: `(current, total)`.
    page_sheets: (usize, usize),
    /// Extra blank sheets added manually via `+ Feuille` on the current page.
    extra_sheets: usize,
    /// Sheet number (1-based) to scroll to on the next frame.
    scroll_to_sheet: Option<usize>,
    /// Queued text to read aloud for web/host wrappers.
    pending_speech: Option<String>,
    /// Queued speech playback command ("pause", "resume", "stop") for web/host wrappers.
    pending_speech_cmd: Option<&'static str>,
    tts_state: TtsState,
    tts_speed: TtsSpeed,
    tts_last_text: String,
    tts_words: Vec<TtsWordCue>,
    tts_sync: std::sync::Arc<std::sync::Mutex<TtsSync>>,
    tts_paused_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    tts_pid: std::sync::Arc<std::sync::atomic::AtomicU32>,
    tts_done: std::sync::Arc<std::sync::atomic::AtomicBool>,
    tts_stop_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    word_preview_pid: std::sync::Arc<std::sync::atomic::AtomicU32>,
    word_preview_seq: u64,
    spell_lang: crate::spellcheck::SpellLang,
    word_prediction_enabled: bool,
    prediction_selected: Option<usize>,
    prediction_caret_pos: Option<(usize, egui::Rect)>,
    prediction_cache: Option<(usize, String, usize, crate::spellcheck::SpellLang, u64, Option<crate::spellcheck::WordAnalysis>)>,
    dictation_state: DictationState,
    dictation_pid: std::sync::Arc<std::sync::atomic::AtomicU32>,
    dictation_stop_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    dictation_done: std::sync::Arc<std::sync::atomic::AtomicBool>,
    dictation_result: std::sync::Arc<std::sync::Mutex<Option<Result<String, String>>>>,
    /// Whether the organisers column is showing, which the three-bar button toggles.
    organisers: bool,
    /// Whether the window is kept above the others.
    always_on_top: bool,
    /// Set when that has just changed and the window manager has not been told yet.
    window_level_pending: bool,
    /// True when the window has no frame of the system's own (the app asked for none), so
    /// the band across the top draws the window's buttons and moves and resizes it.
    frameless: bool,
    editor: Option<Editor>,
    dialog: Option<Dialog>,
    import_job: Option<ImportJob>,
    ocr_job: Option<Receiver<OcrDone>>,
    selected_image_block: Option<usize>,
    last_selected_image: Option<usize>,
    status: String,
    status_is_error: bool,
    /// The host keeps notebook files only for this session, rather than on durable storage.
    session_only_storage: bool,
    search: String,
    searched: String,
    hits: Vec<Hit>,
    /// The Word-style search pane on the right, showing the pages that match `search`.
    search_open: bool,
    /// The Tags Summary, while it is open.
    tags_pane: Option<TagsPane>,
    /// Home > More tags > Customize Tags, while it is open (issue #259).
    custom_tags: Option<crate::customtags::Dialog>,
    templates_pane: Option<TemplatesPane>,
    cursor: usize,
    tokens: Tokens,
    mode_pref: ModePref,
    ui_language: String,
    /// The size the whole window is drawn at, in percent of the system's own scale, and
    /// whether it still has to be handed to egui.
    ui_scale: u16,
    ui_scale_pending: bool,
    /// What File > Check for updates is doing. The update runs on a thread of its own and
    /// writes here; the window reads it each frame and closes when the update is ready to
    /// be put in place.
    update: std::sync::Arc<std::sync::Mutex<crate::update::Status>>,
    update_closing: bool,
    effective_mode: Mode,
    accent: Accent,
    applied_theme: Option<(ModePref, Accent)>,
    loaders_installed: bool,
    redact_paths: bool,
    control: Option<Receiver<ControlRequest>>,
    control_server: Option<ControlServer>,
    event_frames: std::collections::VecDeque<Vec<egui::Event>>,
    show_source: bool,
    /// The code view: which of the page's files is picked, and its text while it is read
    /// or edited.
    code: crate::codeview::State,
    /// Which ribbon tab is open.
    ribbon_tab: String,
    /// What the pen is reporting, where the platform reports anything.
    stylus: crate::stylus::Stylus,
    /// Where the page's words were drawn last frame, which is where its ink goes.
    page_rect: Option<egui::Rect>,
    /// The canvas translation in screen points. It is UI state, so it never changes page data.
    canvas_pan: egui::Vec2,
    /// Last middle-button position while panning the canvas.
    canvas_drag: Option<egui::Pos2>,
    /// Words typed while a pen was in hand, held for the caret's line until it has the keyboard
    /// again (issue #9), and how many frames they have waited.
    typed_while_inking: Vec<egui::Event>,
    typed_wait: u8,
    /// Collect All was asked for: the page gathers its boxes and ink when it is next drawn.
    collect_pending: bool,
    /// The page's sideways scroll bar and its thumb, when one was drawn last frame (issue #188).
    page_hbar: Option<(egui::Rect, egui::Rect)>,
    /// The page ScrollArea's vertical bar hit strip from last frame, kept out of the ink layer.
    page_scroll_strip: Option<egui::Rect>,
    /// True while Insert Space is in hand: the next drag on the page adds or takes away room.
    space_tool: bool,
    /// True while Insert Text Box is in hand: the next click on the page places a floating text box.
    textbox_tool: bool,
    /// Last periodic 1-minute autosave check.
    last_minute_autosave: Instant,
    /// What the pointer does on the page (the Draw tab).
    pointer: ribbon::Pointer,
    /// The tools on the rack, as the owner has arranged them, and which is in hand. Both
    /// are remembered between runs.
    rack: Vec<ergonote_ink::rack::Nib>,
    nib: usize,
    /// How readily the pen answers pressure, 0 to 100. 50 leaves the readings as the pen
    /// gave them; above that a light hand lays down more ink.
    sensitivity: u8,
    /// The drawing open in this page, if it has one.
    pad: Option<crate::inkpad::Pad>,
    /// Present only when the host supplies a download facility.
    file_downloads: Option<Vec<FileDownload>>,
    /// Lines or squares behind the page.
    rules: ribbon::RuleLines,
    /// Where GridCraft is, when it is installed: a table can be handed to it. Looked for
    /// once, when the window opens.
    gridcraft: Option<std::path::PathBuf>,
    /// The page title line is hidden (OneNote: View, Hide Page Title).
    hide_title: bool,
    /// Boxes snap to the page's grid when moved or resized (View, Snap to grid; issue #79).
    snap_to_grid: bool,
    /// The Format Painter's formatting, waiting for the next words picked (issue #165).
    painter: Option<FormatPaint>,
    /// The ribbon is folded away to its tab names (Ctrl+F1; issue #136). Kept in the settings.
    ribbon_collapsed: bool,
    /// While folded, a tab was clicked: its commands are shown over the page until one is used,
    /// the page is clicked or Escape is pressed.
    ribbon_peek: bool,
    /// Where the band with the tab names ends, which is where a shown-over ribbon starts.
    band_bottom: f32,
    /// The page's zoom. The ribbon and the rest of the window keep their own size.
    zoom_percent: u32,
    /// Recalculate the page zoom from the reading area's width on the next frame.
    fit_page_width: bool,
    /// The theme chooser is open as a window (the ribbon's Theme button).
    theme_window: bool,
    /// Text the ribbon asked to put on the clipboard this frame.
    pending_copy: Option<String>,
    /// Matching HTML flavor for the same copied item, when one is available.
    pending_copy_html: Option<String>,
    keyboard_paste_requested: bool,
    /// The File screen, when it is open, and which of its pages.
    backstage: Option<&'static str>,
    /// File > Save a Copy as it is being filled in (issue #301).
    copy_choice: backstage::CopyChoice,
    /// Which pages the user has read, kept beside their settings (issue #291). Loaded the
    /// first time it is needed.
    read_state: Option<ergonote_store::read_state::ReadState>,
    /// The pages that are unread now, and when that was last worked out.
    unread: std::collections::HashSet<PathBuf>,
    unread_checked: Option<Instant>,
    /// The page fills the window: the notebooks are hidden until it is turned off.
    full_page: bool,
    /// The window is phone-width, so it shows the notebooks or the page, not both
    /// (`compact.rs`, issue #12). Worked out from the window's width every frame.
    compact: bool,
    /// Automatically switch to the ultra-simplified mobile UI whenever the viewport is compact
    /// (phone-width), or force it regardless of window width.
    simple_mobile_on_narrow: bool,
    force_simple_mobile: bool,
    /// User-enabled ultra-simple tile mode (accessible from Mise en page > Mode simple on PC too).
    simple_mode: bool,
    /// Current folder/subject shown in the smartphone portrait tile browser (`None` = root tiles).
    mobile_browse_dir: Option<PathBuf>,
    /// Pending Android SAF file/image picker request (MIME type) waiting for the Java shell.
    android_pick_request: Option<String>,
    /// Action to run once Android's document picker returns a cached copy of the chosen file.
    android_pick_pending: Option<AndroidPickKind>,
    /// The compact layout turned Full page on itself, so a wider window turns it off again.
    compact_full_page: bool,
    /// A page was opened since the last frame: the compact layout shows it.
    page_opened: bool,
    /// Put the keyboard in the search box on the next frame.
    focus_search: bool,
    /// The page the window closed on has been opened again (once per run).
    restored: bool,
    /// Recovery warnings that were already shown during this run.
    icon_recovery_reported: HashSet<String>,
    /// Suppress further missing-icon warnings during this run.
    icon_recovery_suppressed: bool,
    /// Alerts the user asked not to see again this session, by key.
    muted_alerts: HashSet<String>,
    /// Alerts waiting for the open one to close.
    queued_alerts: VecDeque<Dialog>,
    /// The clipboard the app reads and writes. The system's, unless the app was built with another.
    clipboard: Box<dyn crate::clipboard::Board>,
    open_attachment_warning_suppressed: bool,
    welcome_notebook_setup: Option<Box<dyn FnMut(&Path, &str) -> Result<PathBuf, String>>>,
    update_dismissed_version: Option<String>,
    url_opener: Option<Box<dyn Fn(&str) + Send + Sync>>,
    android_update_check_requested: bool,
}

impl ErgoNoteApp {
    /// Replaces the clipboard the app uses. Made at startup by the caller, so the app never
    /// reaches a platform's clipboard by itself.
    pub fn with_clipboard(mut self, board: Box<dyn crate::clipboard::Board>) -> ErgoNoteApp {
        self.clipboard = board;
        self
    }

    /// Queues a native keyboard paste captured before egui processes the shortcut.
    pub fn queue_keyboard_paste(&mut self) {
        log::debug!("queued native keyboard paste");
        self.keyboard_paste_requested = true;
    }

    pub fn new(session: Session) -> ErgoNoteApp {
        // The installed fonts are read in the background, so the window opens at once.
        #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
        crate::fonts::start_catalog();
        let mode_pref = session.settings.get("theme_mode").and_then(|v| ModePref::parse(&v)).unwrap_or(ModePref::Light);
        let ui_language = session.settings.get("ui_language").map(|value| crate::i18n::normalize_preference(&value)).unwrap_or_else(|| "fr".into());
        crate::i18n::set_current(&ui_language);
        let accent = session.settings.get("theme_accent").and_then(|v| Accent::parse(&v)).unwrap_or(Accent::Default);
        let show_source = false;
        let session_rules = session.settings.get("rule_lines").and_then(|v| ribbon::RuleLines::parse(&v)).unwrap_or_default();
        let session_hide_title = session.settings.get("hide_title").map(|v| v.trim() == "true").unwrap_or(false);
        let session_snap = session.settings.get("snap_to_grid").map(|v| v.trim() == "true").unwrap_or(false);
        let session_ribbon_collapsed = session.settings.get("ribbon_collapsed").map(|v| v.trim() == "true").unwrap_or(false);
        // Two columns in the side panel, the way OneNote separates the notebooks from the
        // pages. This is the product's default navigation layout; the one-tree view remains
        // available as an explicit preference.
        let session_split = session.settings.get("split_panes").map(|v| v.trim() == "true").unwrap_or(true);
        // Owner decision, 2026-11-20: the subject tabs bar ("Onglets matières") is the
        // default, because a row of subjects is easier to read than the notebook tree.
        // An explicit saved choice still wins.
        let session_section_tabs = session.settings.get("section_tabs_bar").map(|v| v.trim() == "true").unwrap_or(true);
        let session_simple_mode = session.settings.get("simple_mode").map(|v| v.trim() == "true").unwrap_or(false);
        let session_lock_images = session.settings.get("lock_images").map(|v| v.trim() == "true").unwrap_or(false);
        let session_dys_spaced = session.settings.get("dys_spaced").map(|v| v.trim() == "true").unwrap_or(false);
        let session_reading_ruler = session.settings.get("reading_ruler").map(|v| v.trim() == "true").unwrap_or(false);
        let session_word_prediction = session.settings.get("word_prediction").map(|v| v.trim() != "false").unwrap_or(true);
        // The pen rack as the owner last arranged it, and which tool was in hand.
        let rack = ergonote_ink::rack::parse(&session.settings.get(RACK_KEY).unwrap_or_default());
        let nib = session.settings.get(NIB_KEY).and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(0).min(rack.len() - 1);
        let sensitivity = session.settings.get(SENSITIVITY_KEY).and_then(|v| v.trim().parse::<u8>().ok()).unwrap_or(DEFAULT_SENSITIVITY).min(100);
        let qat = quickaccess::Config::load(|k| session.settings.get(k));
        // `--first-use` clears the key to an empty value, which counts as not seen.
        let privacy_welcome = session.settings.get("privacy_welcome_seen").is_none_or(|v| v.trim().is_empty());
        let telemetry_level = session.settings.get("telemetry_level").unwrap_or_else(|| default_telemetry_level(cfg!(debug_assertions)).to_string());
        log::info!("privacy welcome modal state: show={} telemetry_level={:?}", privacy_welcome, telemetry_level);
        let wizard_format = session.settings.global_format().0;
        let ui_scale = session
            .settings
            .get(UI_SCALE_KEY)
            .and_then(|v| v.trim().parse::<u16>().ok())
            .filter(|percent| backstage::UI_SCALES.contains(percent))
            .unwrap_or(100);
        let wizard_level = session.settings.get("telemetry_level").unwrap_or_else(|| "debug".to_string());
        let spell_lang = crate::spellcheck::SpellLang::detect_from_os(&ui_language);
        let update_dismissed_version = session.settings.get("update_dismissed_version").map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
        ErgoNoteApp {
            session,
            saver: crate::saver::Saver::default(),
            nav: NavHistory::default(),
            qat,
            last_ink_change: None,
            undone: Vec::new(),
            open: HashSet::new(),
            selected: None,
            split_panes: session_split || session_section_tabs,
            section_tabs_bar: session_section_tabs,
            lock_images: session_lock_images,
            dys_spaced: session_dys_spaced,
            reading_ruler: session_reading_ruler,
            reading_ruler_y: None,
            page_sheets: (1, 1),
            extra_sheets: 0,
            scroll_to_sheet: None,
            pending_speech: None,
            pending_speech_cmd: None,
            tts_state: TtsState::Stopped,
            tts_speed: TtsSpeed::Normal,
            tts_last_text: String::new(),
            tts_words: Vec::new(),
            tts_sync: std::sync::Arc::new(std::sync::Mutex::new(TtsSync::default())),
            tts_paused_flag: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            tts_pid: std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0)),
            tts_done: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            tts_stop_flag: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            word_preview_pid: std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0)),
            word_preview_seq: 0,
            spell_lang,
            word_prediction_enabled: session_word_prediction,
            prediction_selected: None,
            prediction_caret_pos: None,
            prediction_cache: None,
            dictation_state: DictationState::Idle,
            dictation_pid: std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0)),
            dictation_stop_flag: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            dictation_done: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            dictation_result: std::sync::Arc::new(std::sync::Mutex::new(None)),
            organisers: !session_section_tabs,
            always_on_top: false,
            window_level_pending: false,
            frameless: false,
            editor: None,
            dialog: if privacy_welcome {
                Some(Dialog::FirstUseWizard { step: 0, level: wizard_level, format: wizard_format, add_demo: true, error: None })
            } else {
                None
            },
            import_job: None,
            ocr_job: None,
            selected_image_block: None,
            last_selected_image: None,
            status: crate::tl_id!("ui.ready_2").into(),
            status_is_error: false,
            session_only_storage: false,
            search: String::new(),
            searched: String::new(),
            hits: Vec::new(),
            search_open: false,
            tags_pane: None,
            custom_tags: None,
            templates_pane: None,
            cursor: 0,
            tokens: Theme::DEFAULT.tokens(),
            mode_pref,
            ui_language,
            ui_scale,
            ui_scale_pending: ui_scale != 100,
            update: Default::default(),
            update_closing: false,
            effective_mode: if mode_pref == ModePref::Dark { Mode::Dark } else { Mode::Light },
            accent,
            applied_theme: None,
            loaders_installed: false,
            redact_paths: false,
            control: None,
            control_server: None,
            event_frames: std::collections::VecDeque::new(),
            show_source,
            code: crate::codeview::State::default(),
            ribbon_tab: "Home".to_string(),
            stylus: crate::stylus::Stylus::default(),
            page_rect: None,
            canvas_pan: egui::Vec2::ZERO,
            collect_pending: false,
            page_hbar: None,
            page_scroll_strip: None,
            space_tool: false,
            textbox_tool: false,
            last_minute_autosave: Instant::now(),
            canvas_drag: None,
            typed_while_inking: Vec::new(),
            typed_wait: 0,
            pointer: ribbon::Pointer::Write,
            rack,
            nib,
            sensitivity,
            pad: None,
            file_downloads: None,
            rules: session_rules,
            gridcraft: crate::gridcraft::installed(),
            hide_title: session_hide_title,
            snap_to_grid: session_snap,
            painter: None,
            ribbon_collapsed: session_ribbon_collapsed,
            ribbon_peek: false,
            band_bottom: 0.0,
            zoom_percent: 100,
            fit_page_width: false,
            theme_window: false,
            pending_copy: None,
            pending_copy_html: None,
            keyboard_paste_requested: false,
            backstage: None,
            copy_choice: backstage::CopyChoice::default(),
            read_state: None,
            unread: std::collections::HashSet::new(),
            unread_checked: None,
            full_page: false,
            compact: false,
            simple_mobile_on_narrow: false,
            force_simple_mobile: false,
            simple_mode: session_simple_mode,
            mobile_browse_dir: None,
            android_pick_request: None,
            android_pick_pending: None,
            compact_full_page: false,
            page_opened: false,
            focus_search: false,
            restored: false,
            icon_recovery_reported: HashSet::new(),
            icon_recovery_suppressed: false,
            muted_alerts: HashSet::new(),
            queued_alerts: VecDeque::new(),
            clipboard: Box::new(crate::clipboard::System),
            open_attachment_warning_suppressed: false,
            welcome_notebook_setup: None,
            update_dismissed_version,
            url_opener: None,
            android_update_check_requested: false,
        }
    }

    /// Installs an external URL opener provided by the platform shell (for example Android).
    pub fn with_url_opener(mut self, opener: impl Fn(&str) + Send + Sync + 'static) -> Self {
        self.url_opener = Some(Box::new(opener));
        self
    }

    /// Runs a quiet one-shot check for a newer ErgoNote release when a desktop window opens.
    /// Never writes to disk, never shows errors while offline, and stays uncalled in tests.
    pub fn check_updates_on_startup(&self, ctx: &Context) {
        crate::update::start_silent(ctx.clone(), self.update.clone());
    }

    /// Applies a raw `releases/latest` JSON answer (e.g. fetched by Android Java shell).
    pub fn apply_release_json(&self, json: &str, os: &str) {
        let Some(next) = crate::update::check_release_json(json, crate::backstage::VERSION, os, false) else {
            return;
        };
        if let Ok(mut guard) = self.update.lock() {
            *guard = next;
        }
    }

    /// Consumes a pending Android native update-check request triggered from File > About/Options.
    pub fn take_android_update_check_request(&mut self) -> bool {
        std::mem::take(&mut self.android_update_check_requested)
    }

    fn available_update(&self) -> Option<(String, String, String)> {
        self.update.lock().ok().and_then(|guard| match &*guard {
            crate::update::Status::Available { version, download_url, release_page } => {
                Some((version.clone(), download_url.clone(), release_page.clone()))
            }
            _ => None,
        })
    }

    fn dismiss_update_banner(&mut self, version: &str) {
        self.update_dismissed_version = Some(version.to_owned());
        let _ = self.session.settings.set("update_dismissed_version", version);
    }

    fn open_external_url(&self, ctx: &Context, url: &str) {
        let _ = ctx;
        if let Some(opener) = &self.url_opener {
            opener(url);
            return;
        }
        #[cfg(target_arch = "wasm32")]
        {
            ctx.open_url(egui::OpenUrl::new_tab(url));
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = open_url_in_system(url);
        }
    }

    /// Installs the application-owned action behind the Welcome notebook step of the first-use
    /// wizard. The callback gets the notebooks folder and the chosen format id, and returns the
    /// installed notebook folder. The wizard opens once: on the first launch after installation, or
    /// after the preferences are reset. It does not reopen for later versions.
    pub fn with_welcome_notebook_setup(mut self, setup: impl FnMut(&Path, &str) -> Result<PathBuf, String> + 'static) -> Self {
        self.welcome_notebook_setup = Some(Box::new(setup));
        self
    }

    /// The first-use wizard, starting at the logging step with the current settings.
    fn first_use_wizard(&self) -> Dialog {
        Dialog::FirstUseWizard {
            step: 0,
            level: self.session.settings.get("telemetry_level").unwrap_or_else(|| "debug".to_string()),
            format: self.session.settings.global_format().0,
            add_demo: true,
            error: None,
        }
    }

    /// Applies the wizard's answers: the logging level, the default page format, and, when chosen,
    /// the Welcome notebook in that format. The wizard is marked seen only after every answer is saved,
    /// so a failed install shows the wizard again on the next launch.
    fn finish_first_use_wizard(&mut self, level: &str, format: PageFormat, add_demo: bool) -> Result<(), String> {
        self.session.settings.set("telemetry_level", level).map_err(|error| error.to_string())?;
        self.session.settings.set_default_format(format).map_err(|error| error.to_string())?;
        if add_demo {
            self.save_now();
            if self.is_dirty() {
                return Err("the current page could not be saved before replacing the Welcome notebook".to_string());
            }
            let installed = {
                let setup = self.welcome_notebook_setup.as_mut().ok_or_else(|| "the bundled Welcome notebook setup is unavailable".to_string())?;
                setup(self.session.root(), format.id())?
            };
            self.editor = None;
            self.pad = None;
            self.session.refresh();
            self.status_is_error = false;
            self.open_path(&installed.join("00-start/welcome").to_string_lossy());
            if self.status_is_error {
                return Err(self.status.clone());
            }
        }
        self.session.settings.set("privacy_welcome_seen", "true").map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Opt in to host-delivered files for explicit exports. Ordinary saves never
    /// request a download. The host must drain these outputs after every frame.
    pub fn enable_file_downloads(&mut self) {
        self.file_downloads.get_or_insert_with(Vec::new);
    }

    /// Takes each explicit file export once, for a host such as the web wrapper.
    pub fn take_file_downloads(&mut self) -> Vec<FileDownload> {
        self.file_downloads.as_mut().map(std::mem::take).unwrap_or_default()
    }

    /// Takes a pending read-aloud text request once, for a host such as the web wrapper.
    pub fn take_speech_request(&mut self) -> Option<String> {
        self.pending_speech.take()
    }

    /// Takes a pending speech control command ("pause", "resume", "stop") once, for web wrappers.
    pub fn take_speech_cmd(&mut self) -> Option<&'static str> {
        self.pending_speech_cmd.take()
    }

    /// Locale tag and speaking rate multiplier for host speech engines (Android / web).
    pub fn speech_locale_and_rate(&self) -> (&'static str, f32) {
        let rate = match self.tts_speed {
            TtsSpeed::Slow => 0.75,
            TtsSpeed::Normal => 1.0,
            TtsSpeed::Fast => 1.25,
        };
        (self.spell_lang.tts_locale(), rate)
    }

    /// Marks the active host-driven read-aloud utterance finished so word highlighting stops.
    pub fn mark_tts_done(&self) {
        self.tts_done.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Takes a pending Android native file/image picker MIME request once, for the Java shell.
    pub fn take_android_pick_request(&mut self) -> Option<String> {
        self.android_pick_request.take()
    }

    /// Asks the Java shell to open the camera and insert the shot as a picture in the page.
    pub fn request_android_camera(&mut self) {
        let Some(dir) = self.editor.as_ref().map(|e| e.dir.clone()) else {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return;
        };
        self.android_pick_pending = Some(AndroidPickKind::Picture(dir));
        self.android_pick_request = Some("camera/photo".to_string());
    }

    /// Finishes an Android native picker started by `take_android_pick_request`.
    pub fn complete_android_pick(&mut self, file: Option<PathBuf>) {
        let Some(kind) = self.android_pick_pending.take() else { return };
        let Some(file) = file else { return };
        match kind {
            AndroidPickKind::Picture(dir) | AndroidPickKind::File(dir) => {
                self.add_file(&dir, &file, FileChoice::Copy);
            }
            AndroidPickKind::BoardPhoto(dir) => {
                if let Some(format) = self.editor.as_ref().map(|e| e.format) {
                    self.attach_board_photo_file(&dir, format, &file);
                }
            }
            AndroidPickKind::Printout(dir) => {
                self.add_file(&dir, &file, FileChoice::Printout);
            }
            AndroidPickKind::ImportZip => {
                let root = self.session.root().to_path_buf();
                match extract_notebooks_zip(&file, &root) {
                    Ok(count) => {
                        self.session.refresh();
                        self.expand_all();
                        self.backstage = None;
                        self.say(format!("Récupération réussie ({count} fichiers restaurés) !"));
                    }
                    Err(err) => {
                        self.status = format!("Impossible de récupérer la sauvegarde : {err}");
                        self.status_is_error = true;
                    }
                }
            }
        }
    }

    /// Updates the status bar after an Android native Save-As picker finishes or is cancelled.
    pub fn notify_android_save_result(&mut self, ok: bool, name: &str) {
        if ok {
            let label = if name.trim().is_empty() { "fichier" } else { name };
            self.say(format!("Fichier enregistré : {label}"));
        } else {
            self.say("Enregistrement annulé.");
        }
    }

    /// Receives recognized speech text asynchronously from Android's `RecognizerIntent`.
    pub fn finish_android_dictation(&mut self, text: String) {
        self.dictation_state = DictationState::Idle;
        let clean = text.trim();
        if clean.is_empty() {
            self.say("Aucune parole détectée.");
        } else {
            self.insert_dictated_text(clean);
            self.say(format!("Dictée ajoutée : « {} »", clean));
        }
    }

    /// Current active paragraph text and UTF-16 selection `(start, end)` for Android's native IME
    /// proxy, enabling hardware/Gboard autocorrect and spellcheck suggestions.
    pub fn ime_paragraph_state(&self) -> Option<(usize, String, usize, usize)> {
        if self.show_source
            || self.dialog.is_some()
            || self.backstage.is_some()
            || self.compact_list_fills()
            || self.pointer != ribbon::Pointer::Write
        {
            return None;
        }
        let ed = self.editor.as_ref()?;
        let c = ed.caret.as_ref()?;
        if c.cell.is_some() || matches!(ed.blocks.get(c.block), Some(ergonote_formats::Block::Code { .. } | ergonote_formats::Block::MathBlock(_))) {
            return None;
        }
        let (primary_char, secondary_char) = c.selection;
        let sel_start_utf16 = char_to_utf16(&c.text, secondary_char);
        let sel_end_utf16 = char_to_utf16(&c.text, primary_char);
        Some((c.block, c.text.clone(), sel_start_utf16, sel_end_utf16))
    }

    /// True while the caret sits in a table cell in write mode so Android keeps the soft
    /// keyboard open instead of closing and reopening it when the viewport resizes.
    pub fn ime_table_cell_active(&self) -> bool {
        if self.show_source
            || self.dialog.is_some()
            || self.backstage.is_some()
            || self.compact_list_fills()
            || self.pointer != ribbon::Pointer::Write
        {
            return false;
        }
        self.editor.as_ref().and_then(|ed| ed.caret.as_ref()).is_some_and(|c| c.cell.is_some())
    }

    fn sanitize_ime_paragraph(before_text: &str, mut text: String, sel: (usize, usize)) -> (String, (usize, usize)) {
        let mut sel = pageedit::snap_caret_out_of_markers(&text, sel);
        let before_chars: Vec<char> = before_text.chars().collect();
        let after_chars: Vec<char> = text.chars().collect();
        if after_chars.len() < before_chars.len() && sel.0 == sel.1 {
            let at = sel.0.min(after_chars.len());
            let byte_at = text.char_indices().nth(at).map_or(text.len(), |(b, _)| b);
            for (pat, extra_check) in [("<u></u>", false), ("******", false), ("****", false), ("***", true)] {
                if extra_check && !before_text.contains("****") && !before_text.contains("**") {
                    continue;
                }
                let min_start = byte_at.saturating_sub(pat.len());
                if let Some(rel) = text.get(min_start..).and_then(|s| s.find(pat)) {
                    let start = min_start + rel;
                    if start <= byte_at && byte_at <= start + pat.len() {
                        let removed_char = text.get(..start).map_or(0, |s| s.chars().count());
                        text.replace_range(start..start + pat.len(), "");
                        return (text, (removed_char, removed_char));
                    }
                }
            }
            // Protect closing `**` against accidental 1-star backspace at the end of a bold run (`**mot**|` -> `**mo**`).
            if after_chars.len() + 1 == before_chars.len() {
                let prefix_len = before_chars.iter().zip(&after_chars).take_while(|(a, b)| a == b).count();
                let before_pieces = crate::pagetext::inline_pieces(before_text);
                if before_chars.get(prefix_len) == Some(&'*')
                    && prefix_len == at
                    && let Some((c_start, c_end)) = before_pieces.iter().enumerate().find_map(|(idx, (range, style))| {
                        if *style != crate::pagetext::Style::Marker {
                            return None;
                        }
                        let cs = before_text.get(..range.start)?.chars().count();
                        let ce = before_text.get(..range.end)?.chars().count();
                        let len = ce.saturating_sub(cs);
                        if len >= 2 && cs <= prefix_len && prefix_len < ce {
                            let prev_formatted = idx > 0
                                && matches!(
                                    before_pieces.get(idx - 1).map(|(_, s)| *s),
                                    Some(crate::pagetext::Style::Bold | crate::pagetext::Style::Italic | crate::pagetext::Style::BoldItalic)
                                );
                            if prev_formatted && cs > 0 { Some((cs, ce)) } else { None }
                        } else {
                            None
                        }
                    })
                {
                    let prev_char_idx = c_start - 1;
                    if before_chars.get(prev_char_idx).is_some_and(|ch| *ch != '*' && !ch.is_whitespace()) {
                        let mut fixed: String = before_chars[..prev_char_idx].iter().collect();
                        fixed.extend(before_chars[c_start..c_end].iter());
                        fixed.extend(before_chars[c_end..].iter());
                        let new_at = c_start - 1 + (c_end - c_start);
                        return (fixed, (new_at, new_at));
                    }
                }
            }
        }
        for pat in ["***", "**", "*"] {
            let needle = format!("{pat} ");
            let mut search_from = 0usize;
            while let Some(rel) = text.get(search_from..).and_then(|s| s.find(&needle)) {
                let i = search_from + rel;
                let prev_ok = i == 0 || text.get(..i).and_then(|s| s.chars().next_back()).is_some_and(char::is_whitespace);
                let after_space = i + needle.len();
                let rest = text.get(after_space..).unwrap_or("");
                let next_ok = rest.chars().next().is_some_and(|c| !c.is_whitespace() && c != '*');
                let has_close = next_ok && rest.find(pat).is_some_and(|c| c > 0 && !rest[..c].contains('\n'));
                if prev_ok && has_close {
                    text.replace_range(i..after_space, &format!(" {pat}"));
                    sel = pageedit::snap_caret_out_of_markers(&text, sel);
                    return (text, sel);
                }
                search_from = after_space;
            }
        }
        (text, sel)
    }

    /// Applies an edit coming back from Android's `InputConnection` onto the active paragraph.
    pub fn apply_ime_paragraph(
        &mut self,
        ctx: &Context,
        text: String,
        sel_start_utf16: usize,
        sel_end_utf16: usize,
    ) -> Option<(usize, Option<ImeRewrite>)> {
        let ed = self.editor.as_mut()?;
        let before_caret = ed.caret.clone();
        let (block, text_changed) = {
            let c = ed.caret.as_mut()?;
            if c.cell.is_some() {
                return None;
            }
            let secondary_char = utf16_to_char(&text, sel_start_utf16);
            let primary_char = utf16_to_char(&text, sel_end_utf16);
            let had_pending = ed.pending_format.is_some();
            let (clean_text, (clean_primary, clean_secondary)) = if had_pending {
                (text, (primary_char, secondary_char))
            } else {
                Self::sanitize_ime_paragraph(&c.text, text, (primary_char, secondary_char))
            };
            let text_changed = c.text != clean_text;
            c.text = clean_text;
            c.selection = (clean_primary, clean_secondary);
            (c.block, text_changed)
        };
        let rewrite = if text_changed && ed.pending_format.is_some() {
            ed.apply_pending_format(before_caret.as_ref())
        } else {
            if let Some(pending) = ed.pending_format.as_ref()
                && ed.caret.as_ref().is_none_or(|caret| caret.block != pending.block || caret.selection != (pending.at, pending.at))
            {
                ed.pending_format = None;
            }
            None
        };
        let (final_primary, final_secondary) = ed.caret.as_ref().map_or((0, 0), |c| c.selection);
        ctx.data_mut(|d| d.insert_temp(pageedit::restore_selection_id(), (block, final_primary, final_secondary)));
        if text_changed {
            if rewrite.is_none()
                && let (Some(c), Some(b)) = (ed.caret.as_mut(), ed.blocks.get_mut(block))
            {
                c.write(b);
            }
            ed.rebuild_body();
            Self::mark_dirty_from_blocks(ed);
            Self::remember_step(ed);
        }
        Some((block, rewrite))
    }

    /// Handles Android's IME Enter key directly so splitting formatted or empty lines never
    /// depends on synthetic egui `TextEdit` focus or drops `**` delimiters across the break.
    pub fn apply_ime_enter(&mut self, ctx: &Context) -> bool {
        if self.ime_paragraph_state().is_none() {
            return false;
        }
        let Some(ed) = self.editor.as_mut() else {
            return false;
        };
        let Some(block) = ed.caret.as_ref().map(|c| c.block) else {
            return false;
        };
        ed.pending_format = None;
        Self::remember_step(ed);
        let before_len = ed.blocks.len();
        let changed = pageedit::split_block(&mut ed.blocks, &mut ed.caret, &mut ed.columns);
        if ed.blocks.len() > before_len {
            let added = ed.blocks.len() - before_len;
            ed.count_inserted_in_boxes(block + 1, added);
            for offset in 0..added {
                ed.justified.insert_block(block + 1 + offset);
            }
        }
        if let Some(caret) = ed.caret.as_ref() {
            ctx.data_mut(|d| d.insert_temp(pageedit::restore_selection_id(), (caret.block, caret.selection.0, caret.selection.1)));
        }
        if changed.blocks || changed.caret {
            ed.rebuild_body();
            Self::mark_dirty_from_blocks(ed);
            Self::remember_step(ed);
        }
        true
    }

    /// The session, for tests and for tools that drive the window.
    pub fn session_mut(&mut self) -> &mut Session {
        &mut self.session
    }

    /// Writes the open page now, instead of waiting for the autosave.
    pub fn save_page_now(&mut self) {
        self.save_now();
    }

    /// Opens the page that was open when the window last closed, and the branches above it,
    /// the way OneNote comes back where you left it. Runs once, on the first frame.
    fn restore_last_page(&mut self) {
        if self.restored {
            return;
        }
        self.restored = true;
        let Some(saved) = self.session.settings.get("last_page") else { return };
        let path = self.session.root().join(saved.trim().replace('/', std::path::MAIN_SEPARATOR_STR));
        if !path.is_dir() {
            return;
        }
        // Open every container above it, so the page is visible in the tree.
        let mut at = path.parent();
        while let Some(dir) = at {
            if !dir.starts_with(self.session.root()) || dir == self.session.root() {
                break;
            }
            self.open.insert(dir.to_path_buf());
            at = dir.parent();
        }
        self.open_page(&path);
    }

    /// Remembers which page is open, for the next time the window starts.
    fn remember_last_page(&mut self, dir: &Path) {
        let Ok(rel) = dir.strip_prefix(self.session.root()) else { return };
        let value = rel.to_string_lossy().replace('\\', "/");
        let _ = self.session.settings.set("last_page", &value);
    }

    /// Marks a host whose notebook files disappear with the current browser tab.
    /// Native callers keep the default durable-file captions.
    pub fn with_session_only_storage(mut self) -> Self {
        self.session_only_storage = true;
        self
    }

    /// Enables the streamlined phone-friendly UI on narrow viewports (or unconditionally when
    /// `force` is true, e.g. `?mobile=1` in the web demo).
    pub fn with_simple_mobile(mut self, force: bool) -> Self {
        self.simple_mobile_on_narrow = true;
        self.force_simple_mobile = force;
        self
    }

    fn is_simple_mobile(&self) -> bool {
        self.simple_mode || self.force_simple_mobile || (self.simple_mobile_on_narrow && self.compact)
    }

    /// Whether the page source is shown beside the page. Off by default: the page is what
    /// the user writes on, and the source is there when they ask for it.
    pub fn show_source(&self) -> bool {
        self.show_source
    }

    pub fn set_show_source(&mut self, on: bool) {
        // Closing the view after the page file was typed in: the page is read again, so
        // the live view, the title and everything in the head come from what is on disk.
        if !on && self.code.page_edited {
            self.code.page_edited = false;
            if let Some(dir) = self.editor.as_ref().map(|e| e.dir.clone()) {
                self.open_page(&dir);
            }
        }
        self.show_source = on;
    }

    /// Chooses a theme (light or dark, and an accent) and saves the choice.
    pub fn set_theme(&mut self, theme: Theme) {
        self.mode_pref = if theme.mode.is_dark() { ModePref::Dark } else { ModePref::Light };
        self.effective_mode = theme.mode;
        self.accent = theme.accent;
        self.save_theme();
    }

    /// Opens a File/backstage page for deterministic UI captures and integration harnesses.
    pub fn open_backstage_page(&mut self, page: &'static str) {
        self.backstage = Some(page);
    }

    /// Shows a deterministic progress state for documentation screenshots.
    pub fn show_demo_import_progress(&mut self, index: usize, total: usize, current: &str) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.import_job = Some(ImportJob {
            rx,
            keep_alive: Some(tx),
            cancel: Arc::new(AtomicBool::new(false)),
            source: PathBuf::from("demo-source"),
            destination: PathBuf::from("demo-destination"),
            current: current.to_string(),
            index,
            total,
        });
    }

    /// Follows the system light or dark setting, with the given accent, and saves the choice.
    pub fn follow_system_theme(&mut self, accent: Accent) {
        self.mode_pref = ModePref::System;
        self.accent = accent;
        self.save_theme();
    }

    /// The theme in use (for the system mode, the mode the system last asked for).
    pub fn theme(&self) -> Theme {
        Theme { mode: self.effective_mode, accent: self.accent }
    }

    pub fn mode_pref(&self) -> ModePref {
        self.mode_pref
    }

    fn save_theme(&mut self) {
        let a = self.session.settings.set("theme_mode", self.mode_pref.id());
        let b = self.session.settings.set("theme_accent", self.accent.id());
        if let Err(e) = a.and(b) {
            self.status = crate::i18n::format_message_id("ui.the_theme_could_not_be_saved_error", &[("error", &e.to_string())]);
            self.status_is_error = true;
        }
    }

    fn set_ui_language(&mut self, language: String) {
        if let Err(error) = self.session.settings.set("ui_language", &language) {
            let error = error.to_string();
            let message = crate::i18n::format_message_id("ui.could_not_save_the_interface_language_error", &[("error", &error)]);
            self.complain(&message);
        } else {
            self.ui_language = language;
            crate::i18n::set_current(&self.ui_language);
            self.status = crate::tl_id!("ui.ready_2").to_string();
            self.status_is_error = false;
        }
    }

    /// The size the whole window is drawn at, in percent of the system's own scale.
    pub fn ui_scale(&self) -> u16 {
        self.ui_scale
    }

    /// Draws the whole window larger or smaller, for a screen where the system's own scale
    /// leaves the ribbon and the menus too small to read, and remembers the choice.
    pub fn set_ui_scale(&mut self, percent: u16) {
        if let Err(error) = self.session.settings.set(UI_SCALE_KEY, &percent.to_string()) {
            let error = error.to_string();
            let message = crate::i18n::format_message_id("ui.could_not_save_the_interface_size_error", &[("error", &error)]);
            self.complain(&message);
            return;
        }
        self.ui_scale = percent;
        self.ui_scale_pending = true;
    }

    /// Applies the chosen theme to egui when it changed, and refreshes the tokens.
    fn apply_theme(&mut self, ctx: &Context) {
        let key = (self.mode_pref, self.accent);
        if self.applied_theme != Some(key) {
            ctx.set_visuals_of(egui::Theme::Light, Theme { mode: Mode::Light, accent: self.accent }.visuals());
            ctx.set_visuals_of(egui::Theme::Dark, Theme { mode: Mode::Dark, accent: self.accent }.visuals());
            ctx.set_theme(match self.mode_pref {
                ModePref::Light => egui::ThemePreference::Light,
                ModePref::Dark => egui::ThemePreference::Dark,
                ModePref::System => egui::ThemePreference::System,
            });
            self.applied_theme = Some(key);
        }
        self.effective_mode = if ctx.theme() == egui::Theme::Dark { Mode::Dark } else { Mode::Light };
        self.tokens = self.theme().tokens();
        // Keep every modal surface opaque and on the same themed panel as the rest of the app.
        // Individual dialogs may opt out of the native title bar, but they all inherit this
        // surface and border treatment.
        for theme in [egui::Theme::Light, egui::Theme::Dark] {
            ctx.style_mut_of(theme, |style| {
                style.visuals.window_fill = self.tokens.panel;
                style.visuals.window_stroke = egui::Stroke::new(1.0, self.tokens.faint);
            });
        }
    }

    /// Shows paths as `<notebooks>/...` instead of real local paths. Used by tests and
    /// when taking screenshots for documentation.
    pub fn set_redact_paths(&mut self, on: bool) {
        self.redact_paths = on;
    }

    fn shown(&self, p: &Path) -> String {
        shown_in(self.session.root(), p, self.redact_paths)
    }

    /// Opens `path` (absolute, or relative to the notebooks folder) and expands the tree down
    /// to it. A notebook, section group or section opens on its first page.
    pub fn open_path(&mut self, path: &str) {
        // An explicit command-line/control request wins over the page saved by the
        // previous session. Otherwise the first frame's restore pass would replace the
        // requested page with a stale page from another notebook.
        self.restored = true;
        // A notebook or section opens on its first page, and a notebook kept elsewhere is
        // opened first: the engine decides, so the command line and agents get the same page.
        if let Some(Output::Path(dir)) = self.run("open.page", &params(&[("path", path)])) {
            self.expand_to(&dir);
            self.open_page(&dir);
        }
    }

    /// Opens every level above `dir`, so the page is visible in the tree.
    fn expand_to(&mut self, dir: &Path) {
        let mut parent = dir.parent();
        while let Some(d) = parent {
            if d == self.session.root() {
                break;
            }
            self.open.insert(d.to_path_buf());
            parent = d.parent();
        }
    }

    /// Back to the page before this one.
    fn go_back(&mut self) {
        match self.nav.back(|p| p.exists()) {
            Some(dir) => {
                self.expand_to(&dir);
                self.open_page_inner(&dir);
            }
            None => self.say(crate::tl_id!("ui.there_is_no_earlier_page")),
        }
    }

    /// Forward to the page after this one.
    fn go_forward(&mut self) {
        match self.nav.forward(|p| p.exists()) {
            Some(dir) => {
                self.expand_to(&dir);
                self.open_page_inner(&dir);
            }
            None => self.say(crate::tl_id!("ui.there_is_no_later_page")),
        }
    }

    fn can_undo(&self) -> bool {
        self.editor.as_ref().is_some_and(|e| e.writable && e.history.can_undo()) || self.pad.as_ref().is_some_and(crate::inkpad::Pad::can_undo)
    }

    fn can_redo(&self) -> bool {
        self.editor.as_ref().is_some_and(|e| e.writable && e.history.can_redo()) || self.pad.as_ref().is_some_and(crate::inkpad::Pad::can_redo)
    }

    /// Undo for the Quick Access Toolbar: the page's words and boxes (the page history in
    /// `history.rs`), or the last stroke when the pen was used after anything else on the page,
    /// or when the page has nothing left to take back but the drawing has.
    fn undo(&mut self) {
        let edited = self.editor.as_ref().and_then(|e| e.history.when());
        // The ink's own last change if it keeps one, else when ink last changed at all.
        let inked = self.pad.as_ref().and_then(crate::inkpad::Pad::newest_change).or(self.last_ink_change);
        let ink_newer = match (inked, edited) {
            (Some(i), Some(e)) => i > e,
            (Some(_), None) => true,
            _ => false,
        };
        // One press that changed both (Delete on boxes and ink picked up together) is one step.
        let together = matches!((inked, edited), (Some(i), Some(e)) if i.max(e).duration_since(i.min(e)) < Duration::from_millis(300));
        let ink_can = self.pad.as_ref().is_some_and(crate::inkpad::Pad::can_undo);
        let page_can = self.editor.as_ref().is_some_and(|e| e.writable && e.history.can_undo());
        let what = if ink_can && page_can && together {
            self.ink_undo();
            self.step_back(true);
            Undone::Both
        } else if ink_can && (ink_newer || !page_can) {
            self.ink_undo();
            Undone::Ink
        } else {
            self.step_back(true);
            Undone::Page
        };
        self.undone.push(what);
    }

    /// Puts back what Undo last took back, from the ink, the page or both, the way it was taken
    /// (issue #155). When the record of which no longer fits what either can put back, the page
    /// goes first, then the ink.
    fn redo(&mut self) {
        let ink_can = self.pad.as_ref().is_some_and(crate::inkpad::Pad::can_redo);
        let page_can = self.editor.as_ref().is_some_and(|e| e.writable && e.history.can_redo());
        match self.undone.pop() {
            Some(Undone::Both) if ink_can && page_can => {
                self.ink_redo();
                self.step_back(false);
            }
            Some(Undone::Ink) if ink_can => self.ink_redo(),
            Some(Undone::Page) if page_can => self.step_back(false),
            _ if !page_can && ink_can => {
                self.undone.clear();
                self.ink_redo();
            }
            _ => {
                self.undone.clear();
                self.step_back(false);
            }
        }
    }

    /// OneNote's automatic switching (issue #9): the window follows what the hand is holding.
    /// A pen that comes down on the page while the pointer is the caret picks up the pen last in
    /// hand, colour and thickness and all, and draws with it from that touch on; typing while a
    /// pen or another ink tool is in hand puts it down and the keys go to the page as words. A
    /// mouse, and a finger on a touch screen, change nothing: only a contact that reports a
    /// force is a pen, so this works where the platform passes the pen's pressure on (see
    /// `stylus.rs`), and elsewhere the tools are chosen by hand as before.
    fn switch_by_device(&mut self, ctx: &Context) {
        if self.dialog.is_some() || self.backstage.is_some() || !self.editor.as_ref().is_some_and(|ed| ed.writable) {
            return;
        }
        if self.pointer == ribbon::Pointer::Write {
            // Android reports touch pressure for finger taps as well as styluses, so never
            // auto-switch away from typing mode on Android when the user taps the page.
            #[cfg(not(target_os = "android"))]
            if self.stylus.landed().is_some_and(|at| self.page_rect.is_some_and(|page| page.contains(at))) {
                self.ribbon_act(ribbon::Act::InkPen(self.nib));
                crate::inkpad::claim_press(ctx);
            }
            return;
        }
        let typed = ctx.input(|i| {
            !i.modifiers.command
                && !i.modifiers.ctrl
                && !i.modifiers.alt
                && i.events.iter().any(|e| matches!(e, egui::Event::Text(t) if !t.trim().is_empty()))
        });
        if !typed || ctx.egui_wants_keyboard_input() {
            return;
        }
        self.ribbon_act(ribbon::Act::InkPointer(ribbon::Pointer::Write));
        // The words go back to the line the caret was in, where it was: the line's text box
        // was not drawn while the pen was in hand, so it has to take the keyboard again before
        // it can read them. They are held until it has (`replay_typing`).
        let Some(ed) = self.editor.as_mut() else { return };
        // A selection across lines left from before the pen would keep every line as finished
        // words, with no text box to type into.
        ed.across = None;
        let Some(caret) = ed.caret.as_mut() else { return };
        caret.focus = true;
        caret.place = Some(caret.selection.1);
        self.typed_while_inking = ctx.input_mut(|i| {
            let (text, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut i.events).into_iter().partition(|e| matches!(e, egui::Event::Text(_)));
            i.events = rest;
            text
        });
        self.typed_wait = 0;
        ctx.request_repaint();
    }

    /// Gives the page the words that were typed with a pen in hand (see `switch_by_device`)
    /// once the caret's line has the keyboard again, a frame or two after the switch.
    fn replay_typing(&mut self, ctx: &Context) {
        if self.typed_while_inking.is_empty() {
            return;
        }
        let ready = self.editor.as_ref().and_then(|ed| ed.caret.as_ref()).is_some_and(|c| !c.focus) && ctx.memory(|m| m.focused().is_some());
        if ready {
            let typed = std::mem::take(&mut self.typed_while_inking);
            ctx.input_mut(|i| i.events.extend(typed));
        } else if self.typed_wait > 4 {
            // The caret never came back (the page was closed, say): the words have nowhere to go.
            self.typed_while_inking.clear();
        } else {
            self.typed_wait += 1;
            ctx.request_repaint();
        }
    }
    /// Keys for the pens, so a tablet's express keys or a keyboard can change tools without
    /// reaching for the ribbon (issue #44): Alt+1 to Alt+9 pick up that pen on the Draw tab's
    /// rack, and Alt+0 puts the pens down and goes back to typing. On a Mac, where Option and a
    /// digit also type a character, the character is dropped so it does not land in the note.
    fn pen_keys(&mut self, ctx: &Context) {
        const DIGITS: [Key; 10] = [Key::Num0, Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9];
        let Some(n) = ctx.input_mut(|i| DIGITS.iter().position(|key| i.consume_key(Modifiers::ALT, *key))) else { return };
        ctx.input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Text(_))));
        if n == 0 {
            self.ribbon_act(ribbon::Act::InkPointer(ribbon::Pointer::Write));
            self.say(crate::tl_id!("ui.pen_keys_typing"));
        } else if n <= self.rack.len() {
            self.ribbon_act(ribbon::Act::InkPen(n - 1));
            self.say(crate::i18n::format_message_id("ui.pen_keys_picked", &[("number", &n.to_string())]));
        }
    }

    /// The keys and buttons that move through the pages that were opened. Undo and Redo keys
    /// are the page history's own, taken below.
    fn nav_keys(&mut self, ctx: &Context) {
        if ctx.input_mut(|i| i.consume_key(Modifiers::ALT, Key::ArrowLeft)) || ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Extra1)) {
            self.go_back();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::ALT, Key::ArrowRight)) || ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Extra2)) {
            self.go_forward();
        }
        // OneNote's Ctrl+Page Up and Ctrl+Page Down: the page above or below in the page list,
        // from the page as well as from the list (issue #162).
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::PageUp)) {
            self.step_page(false);
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::PageDown)) {
            self.step_page(true);
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::F1)) {
            self.toggle_ribbon();
        }
    }

    /// The pages of the section in the order the page list shows them: a page, then its
    /// subpages when it is expanded. A collapsed page's subpages are not on the list, so they
    /// are not stepped through either.
    fn listed_pages(&self) -> Vec<PathBuf> {
        fn walk(nodes: &[Node], open: &HashSet<PathBuf>, out: &mut Vec<PathBuf>) {
            for n in nodes.iter().filter(|n| n.info.kind == NodeKind::Page) {
                out.push(n.path.clone());
                if open.contains(&n.path) {
                    walk(&n.children, open, out);
                }
            }
        }
        let mut out = Vec::new();
        if let Some(section) = self.current_section(self.session.tree()) {
            walk(&section.children, &self.open, &mut out);
        }
        out
    }

    /// Opens the page after the open one in the page list, or the one before it. At either end
    /// it stays where it is, as OneNote does, and says so. The page being left is saved first.
    fn step_page(&mut self, forward: bool) {
        let pages = self.listed_pages();
        let here = self.editor.as_ref().map(|e| e.dir.clone()).or_else(|| self.selected.clone());
        let at = here.as_ref().and_then(|h| pages.iter().position(|p| p == h));
        let next = match at {
            Some(i) if forward => pages.get(i + 1),
            Some(i) => i.checked_sub(1).and_then(|i| pages.get(i)),
            // No page of this section open: the first or last of its list.
            None if forward => pages.first(),
            None => pages.last(),
        };
        match next.cloned() {
            Some(page) => {
                self.selected = Some(page.clone());
                self.open_page(&page);
            }
            None => self.say(if forward { crate::tl_id!("ui.this_is_the_last_page") } else { crate::tl_id!("ui.this_is_the_first_page") }),
        }
    }

    /// Folds the ribbon away to its tab names, or puts it back, and keeps the choice for the
    /// next start (Ctrl+F1, as in Office; issue #136). The page gets the ribbon's height.
    pub fn toggle_ribbon(&mut self) {
        self.ribbon_collapsed = !self.ribbon_collapsed;
        self.ribbon_peek = false;
        let _ = self.session.settings.set("ribbon_collapsed", if self.ribbon_collapsed { "true" } else { "false" });
    }

    /// Whether the ribbon shows only its tab names: the user folded it, or the window is
    /// phone-width, where its groups would leave no room for the page (issue #12).
    fn ribbon_folded(&self) -> bool {
        self.ribbon_collapsed || self.compact
    }

    /// Works out the phone-width layout for this frame (`compact.rs`). An opened page fills a
    /// compact window; a window that grows wide again shows the notebooks beside the page.
    fn compact_layout(&mut self, ctx: &Context) {
        let compact = crate::compact::is_compact(ctx.content_rect().width());
        if compact != self.compact {
            self.compact = compact;
            if !compact && self.compact_full_page {
                self.full_page = false;
            }
            self.compact_full_page = false;
        }
        if std::mem::take(&mut self.page_opened)
            && (compact || self.force_simple_mobile || self.simple_mode)
            && self.editor.is_some()
            && !self.full_page
        {
            self.full_page = true;
            self.compact_full_page = true;
        }
    }

    /// Whether the notebooks list fills a phone-width window this frame, with no page beside it.
    fn compact_list_fills(&self) -> bool {
        (self.compact || self.force_simple_mobile || self.simple_mode) && !(self.full_page && self.editor.is_some())
    }

    /// Android's Back button: closes the topmost thing over the page and says whether it did.
    /// A dialog closes as its Cancel would (the first-use wizard goes back a step instead); a
    /// shown-over ribbon or the format painter get Escape, as from the keyboard; the File screen,
    /// the Tags Summary and the page templates close; a page filling the window gives way to the notebooks. With
    /// nothing left to close it does nothing, and Android keeps the app open.
    pub fn system_back(&mut self) -> bool {
        let escape = |frames: &mut std::collections::VecDeque<Vec<egui::Event>>| {
            let key =
                |pressed| egui::Event::Key { key: Key::Escape, physical_key: Some(Key::Escape), pressed, repeat: false, modifiers: Modifiers::NONE };
            frames.push_back(vec![key(true), key(false)]);
        };
        if let Some(dialog) = &self.dialog {
            // Not every dialog reads Escape (a name box keeps it, for one), so Back cancels the
            // dialog itself. The wizard reads Escape as its own Back button.
            if matches!(dialog, Dialog::FirstUseWizard { .. }) {
                escape(&mut self.event_frames);
            } else {
                self.dialog = None;
            }
            return true;
        }
        if self.search_open || !self.search.is_empty() {
            self.dismiss_search();
            return true;
        }
        if self.ribbon_peek || self.painter.is_some() {
            escape(&mut self.event_frames);
            return true;
        }
        if self.backstage.is_some() {
            self.backstage = None;
            return true;
        }
        if self.tags_pane.is_some() && self.compact {
            self.tags_pane = None;
            return true;
        }
        if self.templates_pane.is_some() && self.compact {
            self.templates_pane = None;
            return true;
        }
        if self.is_simple_mobile() && self.mobile_go_up() {
            return true;
        }
        if self.full_page && self.editor.is_some() {
            self.full_page = false;
            self.compact_full_page = false;
            return true;
        }
        false
    }

    /// Where the soft keyboard's text goes while a block of the page has the caret, or `None`.
    /// The page editor reads keys itself rather than through a `TextEdit`, so a platform with an
    /// on-screen keyboard asks for it with this (the Android shell does).
    pub fn keyboard_caret_rect(&self) -> Option<egui::Rect> {
        let ed = self.editor.as_ref()?;
        ed.caret.as_ref()?;
        if self.show_source || self.dialog.is_some() || self.compact_list_fills() {
            return None;
        }
        self.page_rect
    }

    /// The Quick Access Toolbar, drawn in whatever row it is given.
    fn quick_access(&mut self, ui: &mut Ui, tokens: Tokens) {
        let enabled =
            quickaccess::Enabled { back: self.nav.can_back(), forward: self.nav.can_forward(), undo: self.can_undo(), redo: self.can_redo() };
        let (pressed, changed) = quickaccess::show(ui, &mut self.qat, enabled, &tokens);
        if changed {
            self.qat.save(|k, v| {
                let _ = self.session.settings.set(k, v);
            });
        }
        match pressed {
            Some(quickaccess::Command::Back) => self.go_back(),
            Some(quickaccess::Command::Forward) => self.go_forward(),
            Some(quickaccess::Command::Undo) => self.undo(),
            Some(quickaccess::Command::Redo) => self.redo(),
            None => {}
        }
    }

    /// Opens every level of the tree.
    pub fn expand_all(&mut self) {
        self.open = self.collect_branches().into_iter().collect();
    }

    /// Whether the page being edited has unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.editor.as_ref().is_some_and(|e| e.dirty_since.is_some() || e.saving.is_some())
    }

    /// Where the page's words were drawn last frame. Its ink is measured from the top
    /// left of this, so a test that writes on the page aims here.
    pub fn page_rect(&self) -> Option<egui::Rect> {
        self.page_rect
    }

    /// The page's zoom, in percent, as the View tab shows it.
    pub fn zoom_percent(&self) -> u32 {
        self.zoom_percent
    }

    /// The text in the editor, if a page is open.
    pub fn body(&self) -> Option<&str> {
        self.editor.as_ref().map(|e| e.body.as_str())
    }

    /// The title of the open page, which the window shows above it.
    pub fn title(&self) -> Option<&str> {
        self.editor.as_ref().map(|e| e.title.as_str())
    }

    /// Replaces the text in the editor, as typing would, and marks the page changed.
    pub fn set_body(&mut self, body: &str) {
        if let Some(ed) = self.editor.as_mut() {
            ed.body = body.to_string();
            Self::mark_dirty(ed);
        }
    }

    /// Saves the open page now, if it has changes.
    pub fn save(&mut self) {
        self.save_now();
    }

    /// The last status line.
    pub fn status(&self) -> &str {
        &self.status
    }

    fn say(&mut self, msg: impl Into<String>) {
        self.status = msg.into();
        self.status_is_error = false;
    }

    /// Says something went wrong, in our own words rather than the engine's.
    /// Tells the user about a problem. The status line always says it. A box says it too, unless
    /// the user has asked not to see alerts with this `key` again this session; a box that is
    /// already open is not replaced, the next alert waits its turn in a queue.
    pub fn alert(&mut self, key: &str, title: &str, message: &str) {
        self.status = message.to_string();
        self.status_is_error = true;
        if self.muted_alerts.contains(key) {
            return;
        }
        if self.session.settings.get(&format!("{key}_never")).as_deref() == Some("true") {
            return;
        }
        let showing = matches!(&self.dialog, Some(Dialog::Alert { key: k, .. }) if k == key);
        let queued = self.queued_alerts.iter().any(|d| matches!(d, Dialog::Alert { key: k, .. } if k == key));
        if showing || queued {
            return;
        }
        let dialog =
            Dialog::Alert { key: key.to_string(), title: title.to_string(), message: message.to_string(), suppress: false, suppress_forever: false };
        if self.dialog.is_none() {
            self.dialog = Some(dialog);
        } else {
            self.queued_alerts.push_back(dialog);
        }
    }

    fn complain(&mut self, msg: &str) {
        self.status = msg.to_string();
        self.status_is_error = true;
    }

    fn fail(&mut self, e: &EngineError) {
        self.status = crate::i18n::format_message_id("ui.could_not_complete_the_action_error", &[("error", &e.to_string())]);
        self.status_is_error = true;
    }

    fn run(&mut self, id: &str, p: &Params) -> Option<Output> {
        // A command may read or write the page an autosave is still writing.
        self.take_saves(true);
        match self.session.run(id, p) {
            Ok(o) => Some(o),
            Err(e) => {
                self.fail(&e);
                None
            }
        }
    }

    fn reparent_page(&mut self, path: &Path, command: &str) {
        self.save_now();
        if self.is_dirty() {
            return;
        }
        let editor_path = self.editor.as_ref().map(|editor| editor.dir.clone());
        let Some(Output::Path(new_path)) = self.run(command, &params(&[("path", &path_str(path))])) else { return };
        self.nav.relocate(path, &new_path);
        let remap = |candidate: &Path| candidate.strip_prefix(path).map_or_else(|_| candidate.to_path_buf(), |suffix| new_path.join(suffix));
        self.open = self.open.iter().map(|branch| remap(branch)).collect();
        self.selected = self.selected.as_deref().map(remap);
        if let Some(editor_path) = editor_path.filter(|editor_path| editor_path.starts_with(path)) {
            let relocated = remap(&editor_path);
            self.open_page_inner(&relocated);
        }
    }

    /// Opens Move or Copy for a page, section or section group (issue #167).
    pub fn open_move_or_copy(&mut self, path: &Path) {
        let Some(info) =
            ergonote_store::tree::read_node(path).filter(|n| matches!(n.kind, NodeKind::Page | NodeKind::Section | NodeKind::SectionGroup))
        else {
            return;
        };
        self.dialog = Some(Dialog::MoveOrCopy { path: path.to_path_buf(), title: info.title, kind: info.kind, dest: None });
    }

    /// The places a node of `kind` can be moved or copied to, as (path, title, depth), in tree
    /// order: sections for a page, notebooks and section groups for a section or group. Never
    /// the node itself or anything inside it.
    fn move_destinations(&self, path: &Path, kind: NodeKind) -> Vec<(PathBuf, String, usize)> {
        fn walk(nodes: &[Node], path: &Path, kind: NodeKind, depth: usize, out: &mut Vec<(PathBuf, String, usize)>) {
            for n in nodes.iter().filter(|n| n.info.kind != NodeKind::Page) {
                if n.path.starts_with(path) {
                    continue;
                }
                let fits = match kind {
                    NodeKind::Page => n.info.kind == NodeKind::Section,
                    _ => matches!(n.info.kind, NodeKind::Notebook | NodeKind::SectionGroup),
                };
                // A notebook or group is listed even where it does not fit, as the heading of
                // what is inside it.
                out.push((n.path.clone(), n.info.title.clone(), depth));
                if !fits && n.info.kind == NodeKind::Section {
                    out.pop();
                }
                walk(&n.children, path, kind, depth + 1, out);
            }
        }
        let mut out = Vec::new();
        walk(self.session.tree(), path, kind, 0, &mut out);
        out
    }

    /// Moves or copies `path` into `dest`, then keeps the window on the same page: a moved page
    /// that was open is opened again where it went.
    fn move_or_copy(&mut self, path: &Path, dest: &Path, copy: bool) {
        self.save_now();
        if self.is_dirty() {
            return;
        }
        let command = if copy { "node.copy" } else { "node.move" };
        let editor_path = self.editor.as_ref().map(|editor| editor.dir.clone());
        let Some(Output::Path(new_path)) = self.run(command, &params(&[("path", &path_str(path)), ("to", &path_str(dest))])) else { return };
        self.open.insert(dest.to_path_buf());
        if copy {
            self.say(crate::tl_id!("ui.copied"));
            return;
        }
        self.nav.relocate(path, &new_path);
        let remap = |candidate: &Path| candidate.strip_prefix(path).map_or_else(|_| candidate.to_path_buf(), |suffix| new_path.join(suffix));
        self.open = self.open.iter().map(|branch| remap(branch)).collect();
        self.selected = self.selected.as_deref().map(remap);
        if let Some(editor_path) = editor_path.filter(|editor_path| editor_path.starts_with(path)) {
            let relocated = remap(&editor_path);
            self.open_page_inner(&relocated);
        }
        self.say(crate::tl_id!("ui.moved"));
    }

    fn create_page_in(&mut self, parent: &Path) {
        if let Some(Output::Path(page)) = self.run("node.create", &params(&[("parent", &path_str(parent)), ("kind", "page")])) {
            self.open_page(&page);
            self.say(crate::tl_id!("ui.new_page"));
        }
    }

    /// The user's read state, loaded from beside their settings the first time (issue #291).
    fn read_state(&mut self) -> &mut ergonote_store::read_state::ReadState {
        let file = self.session.settings.path().parent().unwrap_or(Path::new(".")).join("read-state.tsv");
        self.read_state.get_or_insert_with(|| ergonote_store::read_state::ReadState::load(&file))
    }

    /// Records the page at `dir` as read as it stands. `opened` is true when the user opened
    /// it, which also clears a mark they put on it by hand; leaving or saving a page keeps
    /// that mark.
    fn note_read(&mut self, dir: &Path, opened: bool) {
        let Some(id) = ergonote_store::tree::read_node(dir).filter(|n| n.kind == NodeKind::Page).map(|n| n.id) else { return };
        let stamp = ergonote_store::read_state::page_stamp(dir);
        let state = self.read_state();
        if !opened && state.is_marked_unread(&id) {
            return;
        }
        state.mark_read(&id, stamp.as_deref());
        if let Err(e) = state.save() {
            log::warn!("could not keep the read state: {e}");
        }
        self.unread.remove(dir);
    }

    /// Every page in the open notebooks, in the order the page list shows them.
    fn pages_in_order(&self) -> Vec<(PathBuf, String)> {
        fn walk(node: &Node, out: &mut Vec<(PathBuf, String)>) {
            if node.info.kind == NodeKind::Page {
                out.push((node.path.clone(), node.info.id.clone()));
            }
            for child in &node.children {
                walk(child, out);
            }
        }
        let mut out = Vec::new();
        for notebook in self.session.tree() {
            walk(notebook, &mut out);
        }
        out
    }

    /// Works out again which pages are unread: at most every half minute unless `now`.
    fn refresh_unread(&mut self, now: bool) {
        if !now && self.unread_checked.is_some_and(|at| at.elapsed() < Duration::from_secs(30)) {
            return;
        }
        self.unread_checked = Some(Instant::now());
        let here = self.editor.as_ref().map(|e| e.dir.clone());
        let pages = self.pages_in_order();
        let state = self.read_state();
        let mut unread = std::collections::HashSet::new();
        for (dir, id) in pages {
            // The page in front of the user is being read.
            if here.as_deref() == Some(dir.as_path()) && !state.is_marked_unread(&id) {
                continue;
            }
            if state.is_unread(&id, ergonote_store::read_state::page_stamp(&dir).as_deref()) {
                unread.insert(dir);
            }
        }
        self.unread = unread;
    }

    /// The unread pages, from the notebooks folder down, for `ui.inspect`.
    fn unread_list(&self) -> Vec<String> {
        let mut unread: Vec<String> =
            self.unread.iter().map(|p| p.strip_prefix(self.session.root()).unwrap_or(p).to_string_lossy().replace('\\', "/")).collect();
        unread.sort();
        unread
    }

    /// History > Next Unread: the next unread page after the open one, in page-list order,
    /// round to the start (issue #291).
    fn next_unread(&mut self) {
        self.refresh_unread(true);
        let pages = self.pages_in_order();
        let here = self.editor.as_ref().map(|e| e.dir.clone());
        let start = here.as_ref().and_then(|h| pages.iter().position(|(p, _)| p == h)).map_or(0, |i| i + 1);
        let next = pages.iter().cycle().skip(start).take(pages.len()).find(|(p, _)| self.unread.contains(p) && Some(p) != here.as_ref());
        match next.map(|(p, _)| p.clone()) {
            Some(page) => self.open_path(&path_str(&page)),
            None => self.say(crate::tl_id!("ui.there_are_no_unread_pages")),
        }
    }

    /// History > Mark as Read / Mark as Unread on the open page (issue #291).
    fn toggle_read(&mut self) {
        let Some(dir) = self.editor.as_ref().map(|e| e.dir.clone()) else {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return;
        };
        let Some(id) = ergonote_store::tree::read_node(&dir).map(|n| n.id) else { return };
        if self.read_state().is_marked_unread(&id) {
            self.note_read(&dir, true);
            self.say(crate::tl_id!("ui.marked_as_read"));
        } else {
            let state = self.read_state();
            state.mark_unread(&id);
            if let Err(e) = state.save() {
                log::warn!("could not keep the read state: {e}");
            }
            self.unread.insert(dir);
            self.say(crate::tl_id!("ui.marked_as_unread"));
        }
    }
    /// Every notebook that is open: those in the notebooks folder and those opened from
    /// elsewhere.
    fn open_notebooks(&self) -> Vec<ergonote_store::tree::Node> {
        let mut notebooks = ergonote_store::tree::load_notebooks(self.session.root());
        for dir in self.session.opened_elsewhere() {
            if let Some(nb) = ergonote_store::tree::load_tree(dir)
                && !notebooks.iter().any(|n| n.path == nb.path)
            {
                notebooks.push(nb);
            }
        }
        notebooks
    }

    /// The pages changed in `range`, in every open notebook or the open page's own.
    fn recent_edits(&self, range: RecentRange, this_notebook: bool) -> Vec<ergonote_store::recent::Edited> {
        let mut notebooks = self.open_notebooks();
        if this_notebook && let Some(here) = self.editor.as_ref().and_then(|e| notebook_for(&e.dir)) {
            notebooks.retain(|n| n.path == here);
        }
        ergonote_store::recent::edited(&notebooks, range.since(ergonote_model::time::now_secs(), ergonote_model::time::local_offset()))
    }

    fn show_recent_edits(&mut self, range: RecentRange, this_notebook: bool) {
        let items = self.recent_edits(range, this_notebook);
        self.dialog = Some(Dialog::RecentEdits { range, this_notebook, filter: String::new(), items });
    }

    fn show_page_versions(&mut self, page: &Path) {
        self.save_now();
        if self.is_dirty() {
            return;
        }
        let p = params(&[("path", &path_str(page))]);
        if let Some(Output::Versions(versions)) = self.run("page.versions", &p) {
            self.dialog = Some(Dialog::PageVersions { page: page.to_path_buf(), versions, preview: None });
        }
    }

    fn can_move_page(path: &Path, up: bool) -> bool {
        let Some(parent) = path.parent() else { return false };
        let siblings: Vec<_> =
            ergonote_store::tree::child_dirs(parent).into_iter().filter(|sibling| ergonote_store::tree::read_node(sibling).is_some()).collect();
        let Some(index) = siblings.iter().position(|sibling| sibling == path) else { return false };
        let adjacent = if up { index.checked_sub(1) } else { index.checked_add(1).filter(|next| *next < siblings.len()) };
        adjacent
            .and_then(|adjacent| siblings.get(adjacent))
            .and_then(|sibling| ergonote_store::tree::read_node(sibling))
            .is_some_and(|info| info.kind == NodeKind::Page)
    }

    fn can_make_subpage(path: &Path) -> bool {
        let Some(parent) = path.parent() else { return false };
        let siblings: Vec<_> =
            ergonote_store::tree::child_dirs(parent).into_iter().filter(|sibling| ergonote_store::tree::read_node(sibling).is_some()).collect();
        let Some(index) = siblings.iter().position(|sibling| sibling == path) else { return false };
        index
            .checked_sub(1)
            .and_then(|previous| siblings.get(previous))
            .and_then(|previous| ergonote_store::tree::read_node(previous))
            .is_some_and(|info| info.kind == NodeKind::Page)
    }

    fn can_promote_subpage(path: &Path) -> bool {
        path.parent().and_then(ergonote_store::tree::read_node).is_some_and(|info| info.kind == NodeKind::Page)
    }

    fn hierarchy_keys(&mut self, ctx: &Context) {
        // Typing in the page (its title or a paragraph) is not a reason to ignore page
        // commands: OneNote's Ctrl+N and Ctrl+Alt+M work from the middle of a sentence (issue
        // #254). Any other text box keeps the keys: the search box, a rename, the source view.
        let typing_elsewhere = ctx.egui_wants_keyboard_input() && !self.page_text_has_focus(ctx);
        if typing_elsewhere || self.dialog.is_some() || self.backstage.is_some() {
            return;
        }
        let Some(page) = self.selected.clone().filter(|path| ergonote_store::tree::read_node(path).is_some_and(|info| info.kind == NodeKind::Page))
        else {
            return;
        };
        if ctx.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::N)) {
            if let Some(parent) = page.parent() {
                self.create_page_in(parent);
            }
            return;
        }
        let is_macos = cfg!(target_os = "macos");
        let mods = Modifiers { ctrl: !is_macos, alt: true, mac_cmd: is_macos, command: true, ..Modifiers::NONE };
        // OneNote's Ctrl+Alt+M: Move or Copy the page.
        if ctx.input_mut(|input| input.consume_key(mods, Key::M)) {
            self.open_move_or_copy(&page);
            return;
        }
        if Self::can_make_subpage(&page) && ctx.input_mut(|input| input.consume_key(mods, Key::CloseBracket)) {
            self.reparent_page(&page, "node.demote");
        } else if Self::can_promote_subpage(&page) && ctx.input_mut(|input| input.consume_key(mods, Key::OpenBracket)) {
            self.reparent_page(&page, "node.promote");
        }
    }

    /// Whether the keyboard is with one of the open page's own text boxes, its title or a block,
    /// rather than with a box outside the page such as search or a rename in the tree.
    fn page_text_has_focus(&self, ctx: &Context) -> bool {
        let (Some(page), Some(_)) = (self.page_rect, self.editor.as_ref()) else { return false };
        if self.show_source {
            return false;
        }
        ctx.memory(|m| m.focused()).and_then(|id| ctx.read_response(id)).is_some_and(|focused| page.contains(focused.rect.center()))
    }

    // ---------- page open, save ----------

    fn open_page(&mut self, dir: &Path) {
        if let Some(notebook) = notebook_for(dir) {
            self.remember_recent_notebook(&notebook);
        }
        self.nav.visit(dir);
        self.open_page_inner(dir);
    }

    /// Opens a page without counting it as a visit: what Back and Forward do.
    fn open_page_inner(&mut self, dir: &Path) {
        self.dismiss_search();
        self.page_opened = true;
        self.page_sheets = (1, 1);
        // Sheets a writer added by hand belong to their page: the count is kept in the settings
        // under the page's own path, so leaving the page and coming back, or restarting, does
        // not throw the blank sheet away (seen on 2026-11-19).
        self.extra_sheets = self.saved_extra_sheets(dir);
        self.scroll_to_sheet = None;
        self.save_now();
        // The page being left was read as it now stands, own edits included (issue #291).
        if let Some(left) = self.editor.as_ref().map(|e| e.dir.clone()).filter(|d| d != dir) {
            self.note_read(&left, false);
        }
        self.code = crate::codeview::State::default();
        if let Some(Output::Page(page)) = self.run("page.load", &params(&[("path", &path_str(dir))])) {
            let (blocks, in_the_text) = ergonote_formats::boxes::read(page.format, &page.body);
            let audit = ergonote_formats::boxes::span_audit(&page.body, &blocks);
            log::debug!("page read span audit: page={:?} raw_span_tags={} styled_spans={}", dir, audit.raw_tags, audit.styled);
            // What the editor will show and edit: any block whose edit text still carries markup
            // is logged, with its index and the text, so a raw tag on screen is traceable.
            for (index, block) in blocks.iter().enumerate() {
                let shown = crate::pageedit::caret_text(block);
                if shown.contains("<span") && audit.raw_tags > 0 {
                    log::warn!("raw markup in edit text: page={:?} block={index} shown={shown:?}", dir);
                } else if shown.contains("<span") || shown.contains("**") {
                    log::debug!("inline edit markers in caret text: page={:?} block={index} shown={shown:?}", dir);
                }
            }
            let older = ergonote_store::page::containers(dir).unwrap_or_default();
            let arranged = in_the_text.is_some() || !older.is_empty();
            let mut boxes = in_the_text.unwrap_or(older);
            log::debug!(
                "page loaded for layout: page={:?} format={:?} blocks={} arranged={} stored_containers={:?}",
                dir,
                page.format,
                blocks.len(),
                arranged,
                boxes
            );
            // A never-arranged blank page still gets one empty writing box. A
            // page whose last box was deliberately deleted remains boxless so
            // its open paper can create a new box on the next click.
            if blocks.is_empty() && !arranged && boxes.is_empty() {
                boxes.push(ergonote_model::Container::new(0.0, 0.0, 360.0, 0));
                log::debug!(
                    "default page block created: page={:?} block_count={} container=(x=0,y=0,width=360,blocks=0) empty_block_min_height=22",
                    dir,
                    blocks.len()
                );
            }
            // The page as it was opened is the first step of its history: undo goes back
            // this far and no further.
            let first = (page.body.clone(), blocks.clone(), boxes.clone());
            // The paper this page is drawn on and the colour behind it are the page's own.
            let appearance = ergonote_store::page::appearance(dir)
                .map(|(p, c)| (p.and_then(|p| ribbon::RuleLines::parse(&p)), c.and_then(|c| ergonote_model::NodeColor::parse(&c))))
                .unwrap_or((None, None));
            let attachments = ergonote_store::assets::list(dir);
            let page_body = page.body.clone();
            let writable = pageedit::round_trips(page.format, &page.body);
            // A page with no title of its own starts with a blank title area, not with
            // "Untitled page" sitting there as if it had been typed. `title` keeps the
            // display fallback, so losing focus on an empty box is correctly seen as "no
            // change" rather than a rename to the fallback word itself.
            let title_buf = if page.has_title { page.title.clone() } else { String::new() };
            self.editor = Some(Editor {
                dir: dir.to_path_buf(),
                format: page.format,
                title_buf,
                titled: page.has_title,
                title: page.title,
                saved_body: page.body.clone(),
                saving: None,
                body: page.body,
                dirty_since: None,
                blocks,
                last_saved: None,
                attachments: attachments.clone(),
                caret: None,
                pending_format: None,
                restore_format_selection: None,
                writable,
                page_dir: page.dir,
                boxes,
                arranged,
                content_bottom: None,
                shown_widths: Vec::new(),
                columns: ergonote_store::page::table_columns(dir).unwrap_or_default(),
                justified: ergonote_store::page::justified(dir).unwrap_or_default(),
                across: None,
                ink_files: ergonote_store::page::ink_files(dir),
                paper: appearance.0,
                colour: appearance.1,
                rule_colour: ergonote_store::page::rule_colour(dir).ok().flatten().and_then(|c| ergonote_model::NodeColor::parse(&c)),
                paper_size: ergonote_store::page::paper_size(dir).ok().flatten(),
                history: crate::history::History::new(crate::history::Step::new(first.0, first.1, first.2, None, crate::history::Kind::Edit)),
                origin: egui::Pos2::ZERO,
                origin_in_canvas: None,
            });
            if !self.icon_recovery_suppressed
                && let Some(name) = first_missing_attachment_in_source(&page_body, dir).or_else(|| first_legacy_attachment_in_source(&page_body))
            {
                let key = format!("{}::{name}", dir.display());
                log::warn!("attachment recovery scan found missing icon: page={:?} attachment={name:?}", dir);
                // Do not let page recovery replace the first-use privacy dialog. The normal
                // frame pass below will offer recovery after the user answers that dialog.
                if self.dialog.is_none() && self.icon_recovery_reported.insert(key) {
                    self.dialog = Some(Dialog::MissingAttachmentIcon { page: dir.to_path_buf(), name });
                }
            } else {
                log::debug!("attachment recovery scan found no missing icons: page={:?} attachments={}", dir, attachments.len());
            }
            self.pad = Some(crate::inkpad::Pad::of_page(dir));
            // Another page, another history: nothing undone on the last one is redone here.
            self.undone.clear();
            self.absorb_old_sheets(dir);
            self.sync_ink_reference();
            self.selected = Some(dir.to_path_buf());
            self.cursor = 0;
            self.remember_last_page(dir);
            // Opened, so read (issue #291).
            self.note_read(dir, true);
            self.say(crate::tl_id!("ui.opened"));
        }
    }

    /// A page written by an earlier version holds its drawing as a picture pointing at
    /// `assets/drawing-1.svg`. Nothing draws those now, so the page showed a broken
    /// picture. Their strokes are moved onto the page's own ink and the picture is taken
    /// out of the words; the old file is left in the folder untouched.
    fn absorb_old_sheets(&mut self, dir: &Path) {
        use ergonote_formats::markdown::Block;
        let olds: Vec<String> = self
            .editor
            .as_ref()
            .map(|e| {
                e.blocks
                    .iter()
                    .filter_map(|b| match b {
                        Block::Image { src, .. } if crate::inkpad::is_old_sheet(src) => Some(src.clone()),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        if olds.is_empty() {
            return;
        }
        let mut moved = 0;
        if let Some(pad) = self.pad.as_mut() {
            for src in &olds {
                moved += pad.absorb(dir, src);
            }
            if pad.dirty && pad.save().is_err() {
                self.complain(crate::tl_id!("ui.cannot_write_this_page_s_ink"));
                return;
            }
        }
        self.tell_kept_ink();
        if let Some(ed) = self.editor.as_mut() {
            ed.blocks.retain(|b| !matches!(b, Block::Image { src, .. } if crate::inkpad::is_old_sheet(src)));
            ed.rebuild_body();
            Self::mark_dirty_from_blocks(ed);
        }
        self.sync_ink_reference();
        // The old file has been read and its strokes are in `ink.svg`, so leaving it
        // behind only puts a dead 200 KB picture in the page's folder for the owner to
        // wonder about. It goes, now that what was in it is kept somewhere better.
        let mut left = 0;
        for src in &olds {
            let file = dir.join(src.trim().replace('/', std::path::MAIN_SEPARATOR_STR));
            if std::fs::remove_file(&file).is_err() {
                left += 1;
            }
        }
        let message = if left > 0 {
            crate::i18n::format_message_id(
                "ui.moved_moved_strokes_onto_the_page_left_old_drawing_f_89f765dcef",
                &[("moved", &moved.to_string()), ("left", &left.to_string())],
            )
        } else {
            crate::i18n::format_message_id("ui.moved_moved_strokes_from_this_page_s_old_drawing_ont_11ecd930d5", &[("moved", &moved.to_string())])
        };
        self.say(&message);
    }

    /// Writes the page now and returns once it is on disk, after any autosave still being
    /// written: what comes next (another page, closing, a command) may read the file.
    fn save_now(&mut self) {
        self.take_saves(true);
        let Some(ed) = self.editor.as_mut() else { return };
        if ed.dirty_since.is_none() {
            return;
        }
        let (dir, body) = (ed.dir.clone(), ed.body_to_save());
        // Nothing to write: the file already says this.
        if body == ed.saved_body {
            ed.dirty_since = None;
            return;
        }
        match self.session.run("page.save", &params(&[("path", &path_str(&dir)), ("body", &body)])) {
            Ok(Output::Text(ts)) => {
                self.note_read(&dir, false);
                if let Some(ed) = self.editor.as_mut() {
                    ed.dirty_since = None;
                    ed.last_saved = Some(ts);
                    ed.saved_body = body;
                }
                self.say(self.saved_caption(None));
            }
            Ok(_) => {}
            Err(e) => self.fail(&e),
        }
    }

    /// The autosave: hands the page to the saver and carries on, so the window is not held up
    /// while a slow folder writes it.
    fn save_in_background(&mut self) {
        let Some(ed) = self.editor.as_mut() else { return };
        if ed.dirty_since.is_none() {
            return;
        }
        let body = ed.body_to_save();
        ed.dirty_since = None;
        if body == ed.saved_body || ed.saving.as_ref() == Some(&body) {
            return;
        }
        self.saver.submit(&ed.dir, &body);
        ed.saving = Some(body);
    }

    /// Takes in the autosaves that have finished, waiting for all of them if `wait`. A save that
    /// failed says why and leaves the page unsaved, so the autosave tries again.
    fn take_saves(&mut self, wait: bool) {
        if !self.saver.busy() {
            return;
        }
        let done = if wait { self.saver.wait() } else { self.saver.finished() };
        for saved in done {
            match saved.result {
                Ok(ts) => {
                    // Saving a page is not a change someone else made: it stays read.
                    self.note_read(&saved.dir, false);
                    let here = self.editor.as_mut().filter(|ed| ed.dir == saved.dir);
                    if let Some(ed) = here {
                        ed.last_saved = Some(ts);
                        if ed.saving.as_ref() == Some(&saved.body) {
                            ed.saving = None;
                        }
                        ed.saved_body = saved.body;
                    }
                    self.say(self.saved_caption(None));
                }
                Err(e) => {
                    if let Some(ed) = self.editor.as_mut().filter(|ed| ed.dir == saved.dir) {
                        ed.saving = None;
                        ed.dirty_since.get_or_insert_with(Instant::now);
                    }
                    self.status = crate::i18n::format_message_id("ui.could_not_complete_the_action_error", &[("error", &e)]);
                    self.status_is_error = true;
                }
            }
        }
    }

    fn repair_attachment_reference(&mut self, page: &Path, name: &str) -> Result<bool, String> {
        let Some(ed) = self.editor.as_mut() else { return Ok(false) };
        if ed.dir != page {
            return Err("the page changed while the recovery dialog was open".into());
        }
        let link = format!("assets/{name}");
        let html_href = format!("href=\"{link}\"");
        let markdown_href = format!("]({link})");
        let references = ed.body.matches(&html_href).count() + ed.body.matches(&markdown_href).count();
        log::debug!("attachment recovery reference scan: page={:?} attachment={name:?} references={references}", page);
        let old = match ed.format {
            PageFormat::Html => format!("<p><a href=\"{link}\">{name}</a></p>"),
            PageFormat::Cmgfm => format!("[{name}]({link})"),
        };
        if references == 0 && !ed.body.contains(&old) {
            let icon_link = attachment_icon_link(page, name);
            let replacement = replace_attachment_icon(ed.format, &snippets::attachment(ed.format, &link, name), name, &icon_link).trim().to_string();
            ed.body.push('\n');
            ed.body.push_str(&replacement);
            Self::mark_dirty(ed);
            log::debug!("attachment recovery appended missing reference: page={:?} attachment={name:?}", page);
            return Ok(true);
        }
        let icon_link = attachment_icon_link(page, name);
        let replacement = replace_attachment_icon(ed.format, &snippets::attachment(ed.format, &link, name), name, &icon_link).trim().to_string();
        let mut body = ed.body.replace(&old, &replacement);
        if references > 1 {
            let mut kept = false;
            body = body
                .lines()
                .filter(|line| {
                    let card = match ed.format {
                        PageFormat::Html => line.contains(&html_href) && line.contains("<img"),
                        PageFormat::Cmgfm => line.contains(&markdown_href) && line.contains("!["),
                    };
                    if card {
                        if kept {
                            return false;
                        }
                        kept = true;
                    }
                    true
                })
                .collect::<Vec<_>>()
                .join("\n");
            log::warn!("attachment recovery removed duplicate references: page={:?} attachment={name:?} references={references}", page);
        }
        // Migrate an older generated icon in place when the platform now has a native icon.
        #[cfg(target_os = "windows")]
        {
            body = body.replace(&format!("assets/{name}.svg"), &icon_link).replace(&format!("assets/{name}.png"), &icon_link);
        }
        if body == ed.body {
            return Ok(false);
        }
        ed.body = body;
        Self::mark_dirty(ed);
        Ok(true)
    }

    /// The page text changed: remember to save it, and read the blocks from it again.
    fn mark_dirty(ed: &mut Editor) {
        ed.dirty_since = Some(Instant::now());
        let (blocks, in_the_text) = ergonote_formats::boxes::read(ed.format, &ed.body);
        ed.blocks = blocks;
        ed.arranged = in_the_text.is_some();
        ed.boxes = in_the_text.unwrap_or_default();
        ed.caret = None;
    }

    /// The blocks changed (the caret is in the page): remember to save, and keep the blocks
    /// as they are. Reading them back from the text would throw away an empty new block and
    /// move the caret out from under the user.
    fn mark_dirty_from_blocks(ed: &mut Editor) {
        // A change that leaves the file as it is, or as it is being written (a click on open
        // paper, which only places the cursor), is not a change to save.
        let file_will_hold = ed.saving.as_ref().unwrap_or(&ed.saved_body);
        ed.dirty_since = (ed.body_to_save() != *file_will_hold).then(Instant::now);
    }

    /// Puts a printout's blocks where the writer is: into the box that holds the caret, after
    /// the caret's line, the way OneNote puts it where you are writing (issue #67). With no
    /// caret in a box it gets a box of its own below the others.
    fn insert_printout_blocks(&mut self, text: &str) {
        let Some(ed) = self.editor.as_mut() else { return };
        Self::remember_step(ed);
        let Some((index, at)) =
            ed.caret.as_ref().and_then(|c| ergonote_model::container::place(&ed.boxes, c.block).map(|(index, _)| (index, c.block + 1)))
        else {
            self.insert_in_own_container(text);
            return;
        };
        let (added, _) = ergonote_formats::boxes::read(ed.format, text);
        if added.is_empty() {
            return;
        }
        let count = added.len();
        let (mut cols_moved, mut justified_moved) = (false, false);
        for (offset, block) in added.into_iter().enumerate() {
            ed.blocks.insert(at + offset, block);
            cols_moved |= ed.columns.insert_block(at + offset);
            justified_moved |= ed.justified.insert_block(at + offset);
        }
        ed.boxes[index].blocks += count;
        // The pages come out as wide as the sheet, as they do in a box of their own: a narrow box
        // (one started with a click) is widened for them rather than the pages shrunk to fit it.
        let sheet = ed.boxes.first().map_or(360.0, |c| c.width);
        ed.boxes[index].width = ed.boxes[index].width.max(sheet);
        ed.arranged = true;
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
        Self::remember_edit(ed);
        let path = path_str(&ed.dir);
        let cols = cols_moved.then(|| ergonote_model::tablecols::write(&ed.columns));
        let justified = justified_moved.then(|| ergonote_model::justify::write(&ed.justified));
        if let Some(line) = cols {
            self.run("page.columns", &params(&[("path", &path), ("widths", &line)]));
        }
        if let Some(line) = justified {
            self.run("page.justify", &params(&[("path", &path), ("blocks", &line)]));
        }
    }

    /// Adds the blocks of `text` to the page in a container of their own, below what is
    /// already on the page, so each printout can be moved and sized apart from the others.
    fn insert_in_own_container(&mut self, text: &str) {
        let Some(ed) = self.editor.as_mut() else { return };
        let below = ed.content_bottom;
        let (added, _) = ergonote_formats::boxes::read(ed.format, text);
        if added.is_empty() {
            return;
        }
        let width = ed.boxes.first().map_or(360.0, |c| c.width);
        let before = ed.blocks.len();
        ed.blocks.extend(added.iter().cloned());
        let below = below.unwrap_or_else(|| ed.boxes.iter().map(|c| c.y + 60.0).fold(0.0, f32::max));
        ed.boxes = ergonote_model::container::append(&ed.boxes, before, added.len(), width, below, 12.0);
        ed.arranged = true;
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
        Self::remember_step(ed);
    }

    fn insert(&mut self, text: &str) {
        let cursor = self.cursor;
        if let Some(ed) = self.editor.as_mut() {
            let (body, next) = snippets::insert_at(&ed.body, cursor, text);
            ed.body = body;
            Self::mark_dirty(ed);
            self.cursor = next;
        }
    }

    /// Puts the blocks of a snippet (a file copied in or linked) after the caret's line, in the
    /// caret's box, or on the end of the page. It works on the blocks rather than splicing the
    /// page's text at `cursor`, which put the snippet wherever that old text position pointed
    /// (the top of the page, in a box of its own) and read every box back from the text
    /// (issue #72).
    fn insert_snippet_blocks(&mut self, text: &str) {
        let Some(ed) = self.editor.as_mut() else { return };
        let (added, _) = ergonote_formats::boxes::read(ed.format, text);
        if added.is_empty() {
            return;
        }
        let at = ed.caret.as_ref().map_or(ed.blocks.len(), |c| c.block + 1).min(ed.blocks.len());
        let count = added.len();
        for (offset, block) in added.into_iter().enumerate() {
            ed.blocks.insert(at + offset, block);
        }
        let marks = ed.blocks_inserted(at, count);
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
        Self::remember_step(ed);
        self.save_block_marks(marks);
    }

    /// A dropped file asks what to do with it, the way OneNote asks when you insert one.
    fn attach_dropped(&mut self, ctx: &Context) {
        let files: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if files.is_empty() {
            return;
        }
        let Some(dir) = self.editor.as_ref().map(|e| e.dir.clone()) else {
            self.status = crate::tl_id!("ui.open_a_page_first_then_drop_files_onto_it").into();
            self.status_is_error = true;
            return;
        };
        let mut remaining = Vec::new();
        for file in files {
            if printout::supports(&file) {
                self.add_file(&dir, &file, FileChoice::Printout);
            } else if file
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg"))
            {
                self.add_file(&dir, &file, FileChoice::Copy);
            } else {
                remaining.push(file);
            }
        }
        if !remaining.is_empty() {
            self.dialog = Some(Dialog::FileOptions { dir, files: remaining });
        }
    }

    /// Carries out one of the choices of the file dialog.
    fn add_file(&mut self, dir: &Path, file: &Path, how: FileChoice) {
        let Some(format) = self.editor.as_ref().map(|e| e.format) else { return };
        let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        match how {
            FileChoice::Copy => {
                log::debug!("file attach requested: name={name:?} page={:?}", dir);
                match self.run("page.attach", &params(&[("path", &path_str(dir)), ("source", &path_str(file))])) {
                    Some(Output::Text(link)) => {
                        log::debug!("file attach copied: name={name:?} link={link:?}");
                        let text = snippets::attachment(format, &link, &name);
                        self.insert_snippet_blocks(&text);
                        self.say(crate::i18n::format_message_id("ui.name_is_now_a_file_in_this_page_s_folder", &[("name", &name)]));
                    }
                    Some(_) => self.complain(crate::tl_id!("ui.the_file_was_not_attached_the_attach_command_returne_beaffe0fd0")),
                    None => {
                        log::error!("file attach failed: name={name:?} page={:?} status={:?}", dir, self.status);
                        self.complain(&crate::i18n::format_message_id("ui.could_not_attach_name_to_this_page", &[("name", &name)]));
                    }
                }
            }
            FileChoice::Link => {
                let text = snippets::file_link(format, &path_str(file), &name);
                self.insert_snippet_blocks(&text);
                self.say(crate::i18n::format_message_id("ui.linked_name_where_it_is_the_file_was_not_copied", &[("name", &name)]));
            }
            FileChoice::Printout => self.insert_printout(dir, file, format),
        }
        if let Some(ed) = self.editor.as_mut() {
            ed.attachments = ergonote_store::assets::list(&ed.dir);
        }
    }

    fn insert_printout(&mut self, dir: &Path, file: &Path, format: PageFormat) {
        let rendered = match printout::render(file) {
            Ok(rendered) => rendered,
            Err(error) => {
                self.complain(&error);
                return;
            }
        };
        let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let Some(Output::Text(link)) = self.run("page.attach", &params(&[("path", &path_str(dir)), ("source", &path_str(file))])) else {
            self.complain(&crate::i18n::format_message_id("ui.could_not_attach_name_before_rendering_its_pages", &[("name", &name)]));
            return;
        };
        let Some(base) = link.strip_prefix("assets/").and_then(|link| Path::new(link).file_name()).map(|name| name.to_string_lossy().into_owned())
        else {
            self.complain(crate::tl_id!("ui.the_attachment_returned_an_invalid_page_link"));
            return;
        };
        let assets = dir.join("assets");
        let mut generated = Vec::with_capacity(rendered.pages().len());
        for (index, page) in rendered.pages().iter().enumerate() {
            let target = assets.join(format!("{base}.printout-{:03}.png", index + 1));
            if let Err(error) = std::fs::copy(page, &target) {
                for path in &generated {
                    let _ = std::fs::remove_file(path);
                }
                self.complain(&crate::i18n::format_message_id(
                    "ui.could_not_store_printout_page_page_error",
                    &[("page", &(index + 1).to_string()), ("error", &error.to_string())],
                ));
                return;
            }
            generated.push(target);
        }
        let mut text = String::new();
        for (index, _) in generated.iter().enumerate() {
            let image = format!("assets/{base}.printout-{:03}.png", index + 1);
            text.push_str(&snippets::printout_page(format, &image, index + 1));
        }
        self.insert_printout_blocks(&text);
        self.say(crate::i18n::format_message_id(
            "ui.inserted_name_and_count_printout_pages",
            &[("name", &name), ("count", &generated.len().to_string())],
        ));
    }

    // ---------- tree ----------

    fn collect_branches(&self) -> Vec<PathBuf> {
        let mut v = Vec::new();
        for n in self.session.tree() {
            n.branches(&mut v);
        }
        v
    }

    fn apply(&mut self, act: Act) {
        match act {
            Act::Toggle(p) => {
                if !self.open.remove(&p) {
                    self.open.insert(p);
                }
            }
            Act::Select(p) => {
                let is_page = ergonote_store::tree::read_node(&p).map(|i| i.kind == NodeKind::Page).unwrap_or(false);
                if is_page {
                    self.open_page(&p);
                } else {
                    if !self.open.remove(&p) {
                        self.open.insert(p.clone());
                    }
                    // Selecting a container must also move the editor. Leaving the old
                    // page open here made the navigator and page column describe one
                    // location while the editor showed another.
                    if let Some(page) = first_page(self.session.tree(), &p) {
                        self.open_page(&page);
                        // A phone-width window keeps the list up, so the next level can be
                        // picked; only a page chosen for itself takes the window.
                        self.page_opened = false;
                    } else {
                        self.selected = Some(p);
                    }
                }
            }
            Act::Add(parent, parent_kind) => {
                let kind = parent_kind.may_contain().first().copied().unwrap_or(NodeKind::Page);
                let kind = if parent_kind == NodeKind::Notebook || parent_kind == NodeKind::SectionGroup { NodeKind::Section } else { kind };
                self.dialog = Some(Dialog::Add { parent, parent_kind, title: String::new(), kind, format: None });
            }
            Act::NewNotebook => self.dialog = Some(Dialog::NewNotebook { name: String::new(), format: None }),
            Act::CloseNotebook(path) => {
                self.save_now();
                let was_showing = self.editor.as_ref().is_some_and(|ed| ed.dir.starts_with(&path));
                if matches!(self.run("notebook.close", &params(&[("path", &path_str(&path))])), Some(Output::Text(ref result)) if result == "closed")
                {
                    self.open.retain(|p| !p.starts_with(&path));
                    if self.selected.as_ref().is_some_and(|p| p.starts_with(&path)) {
                        self.selected = self.session.tree().first().map(|n| n.path.clone());
                    }
                    if was_showing {
                        self.editor = None;
                        self.pad = None;
                        self.nav = NavHistory::default();
                    }
                    self.say(crate::tl_id!("ui.notebook_closed_its_files_are_still_on_disk"));
                }
            }
            Act::RevealRoot => {
                let root = self.session.root().to_path_buf();
                self.reveal(&root);
            }
            Act::ChooseRoot => self.choose_notebooks_folder(),
            Act::AddKind(parent, parent_kind, kind) => {
                self.dialog = Some(Dialog::Add { parent, parent_kind, title: String::new(), kind, format: None });
            }
            Act::AddPageAt(section, order) => {
                let path = path_str(&section);
                let order = order.to_string();
                // No title: the page is created genuinely untitled, like a new OneNote page.
                // "Untitled page" is only ever the display fallback, never real content.
                if let Some(Output::Path(dir)) = self.run("node.create", &params(&[("parent", &path), ("kind", "page"), ("order", &order)])) {
                    self.open_page(&dir);
                    self.say(crate::tl_id!("ui.new_page"));
                }
            }
            Act::RenumberAndAddPage(section, before) => {
                // Where the page before the line sits among the pages now, before renumbering.
                let position = ergonote_store::tree::child_dirs(&section)
                    .into_iter()
                    .filter_map(|c| ergonote_store::tree::read_node(&c))
                    .filter(|n| n.kind == NodeKind::Page)
                    .position(|n| n.order == before);
                let path = path_str(&section);
                if let (Some(position), Some(_)) = (position, self.run("node.renumber", &params(&[("path", &path)]))) {
                    // Renumbered ten apart in the same order, so the line is halfway after that page.
                    let order = ((position as u32 + 1) * 10 + 5).to_string();
                    if let Some(Output::Path(dir)) = self.run("node.create", &params(&[("parent", &path), ("kind", "page"), ("order", &order)])) {
                        self.open_page(&dir);
                        self.say(crate::tl_id!("ui.new_page"));
                    }
                }
            }
            Act::NewPage(parent) => self.create_page_in(&parent),
            Act::Rename(path, title) => self.dialog = Some(Dialog::Rename { path, title }),
            Act::MoveOrCopy(path) => self.open_move_or_copy(&path),
            Act::MakeSubpage(path) => self.reparent_page(&path, "node.demote"),
            Act::PromoteSubpage(path) => self.reparent_page(&path, "node.promote"),
            Act::MovePageUp(path) => {
                if let Some(Output::Path(path)) = self.run("node.move_up", &params(&[("path", &path_str(&path))])) {
                    self.selected = Some(path.clone());
                    self.session.refresh();
                }
            }
            Act::MovePageDown(path) => {
                if let Some(Output::Path(path)) = self.run("node.move_down", &params(&[("path", &path_str(&path))])) {
                    self.selected = Some(path.clone());
                    self.session.refresh();
                }
            }
            Act::PageVersions(path) => self.show_page_versions(&path),
            Act::Delete(path, title, count) => self.dialog = Some(Dialog::Delete { path, title, count }),
            Act::Color(path, color) => {
                let value = color.map_or("none", NodeColor::id);
                if self.run("node.color", &params(&[("path", &path_str(&path)), ("color", value)])).is_some() {
                    self.say(match color {
                        Some(c) => {
                            let colour = crate::tl!(c.label());
                            crate::i18n::format_message_id("ui.colour_set_to_colour", &[("colour", &colour.to_lowercase())])
                        }
                        None => crate::tl_id!("ui.colour_removed").to_string(),
                    });
                }
            }
            Act::CopyLink(path) => self.copy_link_to(&path),
            Act::Reveal(path) => match open_folder(&path) {
                Ok(()) => self.say(crate::i18n::format_message_id("ui.opened_path", &[("path", &self.shown(&path))])),
                Err(e) => {
                    self.status = crate::i18n::format_message_id("ui.the_folder_could_not_be_opened_error", &[("error", &e.to_string())]);
                    self.status_is_error = true;
                }
            },
        }
    }

    fn draw_node(&self, ui: &mut Ui, node: &Node, depth: usize, acts: &mut Vec<Act>) {
        self.draw_row(ui, node, depth, acts, true);
    }

    /// One row of the tree. With `pages` false the pages are left out, which is what the
    /// organisers column shows: notebooks, section groups and sections only.
    fn draw_row(&self, ui: &mut Ui, node: &Node, depth: usize, acts: &mut Vec<Act>, pages: bool) {
        let has_kids = if pages { !node.children.is_empty() } else { node.children.iter().any(|c| c.info.kind != NodeKind::Page) };
        let open = self.open.contains(&node.path);
        let selected = self.selected.as_deref() == Some(node.path.as_path());
        let width = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(vec2(width, ROW_HEIGHT), Sense::click());
        let painter = ui.painter_at(rect);
        let t = &self.tokens;
        if selected {
            painter.rect_filled(rect.shrink2(vec2(2.0, 1.0)), 6.0, t.row_selected);
        } else if resp.hovered() {
            painter.rect_filled(rect.shrink2(vec2(2.0, 1.0)), 6.0, t.row_hover);
        }
        let x0 = rect.min.x + depth as f32 * INDENT + 2.0;
        let cy = rect.center().y;
        let chevron_rect = egui::Rect::from_min_size(egui::pos2(x0, rect.min.y), vec2(16.0, ROW_HEIGHT));
        if has_kids {
            let c = chevron_rect.center();
            let stroke = egui::Stroke::new(1.5, t.muted);
            let pts = if open {
                [egui::pos2(c.x - 3.5, c.y - 1.5), egui::pos2(c.x, c.y + 2.5), egui::pos2(c.x + 3.5, c.y - 1.5)]
            } else {
                [egui::pos2(c.x - 1.5, c.y - 3.5), egui::pos2(c.x + 2.5, c.y), egui::pos2(c.x - 1.5, c.y + 3.5)]
            };
            painter.line_segment([pts[0], pts[1]], stroke);
            painter.line_segment([pts[1], pts[2]], stroke);
        }
        let (icon, default_color) = match node.info.kind {
            NodeKind::Notebook => (Icon::Notebook, t.notebook),
            NodeKind::SectionGroup => (if open && has_kids { Icon::SectionGroupOpen } else { Icon::SectionGroup }, t.group),
            NodeKind::Section => (Icon::Section, t.section),
            NodeKind::Page => (if has_kids && !open { Icon::PageStack } else { Icon::Page }, t.page),
        };
        // The colour the user gave this notebook or section, kept readable on the panel.
        let color = match node.info.color {
            Some(c) => {
                let (r, g, b) = c.rgb(self.effective_mode.is_dark());
                crate::theme::readable_on(egui::Color32::from_rgb(r, g, b), t.panel, 3.0, self.effective_mode.is_dark())
            }
            None => default_color,
        };
        let icon_rect = egui::Rect::from_min_size(egui::pos2(x0 + 18.0, cy - 8.5), vec2(17.0, 17.0));
        icons::paint(&painter, icon_rect, icon, color);
        // A page changed since the user last read it carries a dot, as unread mail does (#291).
        let unread = node.info.kind == NodeKind::Page && self.unread.contains(&node.path);
        if unread {
            painter.circle_filled(icon_rect.right_top() + vec2(-1.0, 2.0), 3.5, t.accent);
        }
        let text_pos = egui::pos2(x0 + 40.0, cy);
        let text_color = if selected { t.strong } else { ui.style().visuals.text_color() };
        // Pages keep room for the add-page "+" at the end of the row. Organiser rows do not,
        // so notebook, group and section names run almost to the pane border.
        let trailing = if node.info.kind == NodeKind::Page { 28.0 } else { 8.0 };
        let room = (rect.max.x - trailing - text_pos.x).max(0.0);
        // The format label ("html", "md") is the less important of the two: it is shown only
        // when the whole name fits beside it, and the name is never cut to make room for it.
        // Hide technical format labels ("html", "md") for student/DYS clarity.
        let label: Option<&str> = None;
        let label_width = label.map(|l| text_width(ui, l, 10.5) + ROW_LABEL_GAP);
        // A page with no name of its own shows the catalog's own wording ("Page sans titre"),
        // not the English fallback the store keeps for code (seen on 2026-11-19).
        let row_title: &str = if node.info.kind == NodeKind::Page && node.info.title == ergonote_store::tree::UNTITLED_PAGE {
            crate::tl_id!("ui.untitled_page")
        } else {
            &node.info.title
        };
        let (show_label, name_room) = row_name_room(text_width(ui, row_title, 14.0), label_width, room);
        let name = fit_to_width(ui, row_title, 14.0, name_room);
        let text_clip = egui::Rect::from_min_max(rect.min, egui::pos2(text_pos.x + name_room + 1.0, rect.max.y));
        // The tree is on screen all the time, so this is the most visible place the words
        // of an Arabic name would come out backwards. Each row's name decides its own
        // direction; the row keeps its place and its clip either way.
        let clipped = painter.with_clip_rect(text_clip);
        crate::bidi::line_at(ui, &clipped, text_pos, Align2::LEFT_CENTER, &name, 14.0, text_color, None);
        let plus_rect = egui::Rect::from_min_size(egui::pos2(rect.max.x - 24.0, rect.min.y + 2.0), vec2(20.0, ROW_HEIGHT - 4.0));
        if show_label && let Some(l) = label {
            painter.text(egui::pos2(rect.max.x - trailing, cy), Align2::RIGHT_CENTER, l, egui::FontId::proportional(10.5), t.muted);
        }
        if resp.hovered() {
            painter.text(plus_rect.center(), Align2::CENTER_CENTER, "+", egui::FontId::proportional(16.0), t.strong);
        }
        let enabled = ui.is_enabled();
        let spoken = if unread { crate::i18n::format_message_id("ui.title_unread", &[("title", row_title)]) } else { row_title.to_string() };
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &spoken));
        let resp = resp.on_hover_text(self.shown(&node.path));
        if resp.clicked() {
            let pos = resp.interact_pointer_pos().unwrap_or_default();
            if has_kids && chevron_rect.expand2(vec2(2.0, 0.0)).contains(pos) {
                acts.push(Act::Toggle(node.path.clone()));
            } else if plus_rect.contains(pos) {
                acts.push(Act::Add(node.path.clone(), node.info.kind));
            } else {
                acts.push(Act::Select(node.path.clone()));
            }
        }
        resp.context_menu(|ui| {
            let new_page_label = if cfg!(target_os = "macos") { crate::tl_id!("ui.new_page_cmd_n") } else { crate::tl_id!("ui.new_page_ctrl_n") };
            if node.info.kind == NodeKind::Page
                && let Some(parent) = node.path.parent()
                && floating::menu_row(ui, Some(floating::Glyph::Icon(Icon::Page)), new_page_label, t).clicked()
            {
                acts.push(Act::NewPage(parent.to_path_buf()));
                ui.close();
            }
            // What this node may hold, named in full rather than left to the dialog: a
            // notebook and a group take sections and groups, a section takes pages, and a
            // page takes sub-pages.
            for kind in node.info.kind.may_contain() {
                let label = match kind {
                    NodeKind::SectionGroup => "New section group...",
                    NodeKind::Section => "New section...",
                    NodeKind::Page if node.info.kind == NodeKind::Page => crate::tl_id!("ui.add_a_sub_page"),
                    NodeKind::Page => crate::tl_id!("ui.add_page"),
                    NodeKind::Notebook => "New notebook...",
                };
                let icon = match kind {
                    NodeKind::SectionGroup => Icon::SectionGroup,
                    NodeKind::Section => Icon::Section,
                    NodeKind::Page => Icon::Page,
                    NodeKind::Notebook => Icon::Notebook,
                };
                if floating::menu_row(ui, Some(floating::Glyph::Icon(icon)), label, t).clicked() {
                    acts.push(Act::AddKind(node.path.clone(), node.info.kind, *kind));
                    ui.close();
                }
            }
            if node.info.kind == NodeKind::Page
                && (Self::can_make_subpage(&node.path)
                    || Self::can_promote_subpage(&node.path)
                    || Self::can_move_page(&node.path, true)
                    || Self::can_move_page(&node.path, false))
            {
                floating::menu_separator(ui, t);
                if Self::can_make_subpage(&node.path)
                    && floating::menu_row(ui, Some(floating::Glyph::Mark(crate::ribbon_icons::Mark::IndentMore)), crate::tl_id!("ui.make_subpage"), t)
                        .clicked()
                {
                    acts.push(Act::MakeSubpage(node.path.clone()));
                    ui.close();
                }
                if Self::can_promote_subpage(&node.path)
                    && floating::menu_row(
                        ui,
                        Some(floating::Glyph::Mark(crate::ribbon_icons::Mark::IndentLess)),
                        crate::tl_id!("ui.promote_subpage"),
                        t,
                    )
                    .clicked()
                {
                    acts.push(Act::PromoteSubpage(node.path.clone()));
                    ui.close();
                }
                if Self::can_move_page(&node.path, true)
                    && floating::menu_row(ui, Some(floating::Glyph::Icon(Icon::MoveUp)), crate::tl_id!("ui.move_up"), t).clicked()
                {
                    acts.push(Act::MovePageUp(node.path.clone()));
                    ui.close();
                }
                if Self::can_move_page(&node.path, false)
                    && floating::menu_row(ui, Some(floating::Glyph::Icon(Icon::MoveDown)), crate::tl_id!("ui.move_down"), t).clicked()
                {
                    acts.push(Act::MovePageDown(node.path.clone()));
                    ui.close();
                }
            }
            floating::menu_separator(ui, t);
            if floating::menu_row(ui, Some(floating::Glyph::Icon(Icon::Page)), crate::tl_id!("ui.rename_ellipsis"), t).clicked() {
                acts.push(Act::Rename(node.path.clone(), node.info.title.clone()));
                ui.close();
            }
            if matches!(node.info.kind, NodeKind::Page | NodeKind::Section | NodeKind::SectionGroup)
                && floating::menu_row(ui, Some(floating::Glyph::Icon(Icon::MoveDown)), crate::tl_id!("ui.move_or_copy_ellipsis"), t).clicked()
            {
                acts.push(Act::MoveOrCopy(node.path.clone()));
                ui.close();
            }
            if node.info.kind == NodeKind::Notebook
                && floating::menu_row(ui, Some(floating::Glyph::Icon(Icon::Notebook)), crate::tl_id!("ui.close_notebook"), t).clicked()
            {
                acts.push(Act::CloseNotebook(node.path.clone()));
                ui.close();
            }
            if has_kids
                && floating::menu_row(
                    ui,
                    Some(floating::Glyph::Icon(if open { Icon::Minus } else { Icon::Plus })),
                    if node.info.kind == NodeKind::Page {
                        if open { crate::tl_id!("ui.collapse_subpages") } else { crate::tl_id!("ui.expand_subpages") }
                    } else if open {
                        crate::tl_id!("ui.collapse")
                    } else {
                        crate::tl_id!("ui.expand")
                    },
                    t,
                )
                .clicked()
            {
                acts.push(Act::Toggle(node.path.clone()));
                ui.close();
            }
            // Pages take the colour of their section, the way OneNote colours tabs.
            if node.info.kind != NodeKind::Page {
                draw_node_color_menu(ui, &node.path, node.info.color, self.effective_mode.is_dark(), acts);
            }
            if floating::menu_row(ui, Some(floating::Glyph::Icon(Icon::Link)), crate::tl_id!("ui.copy_link_to_page"), t)
                .on_hover_text(crate::tl_id!("ui.a_link_to_paste_into_another_page"))
                .clicked()
            {
                acts.push(Act::CopyLink(node.path.clone()));
                ui.close();
            }
            if floating::menu_row(ui, Some(floating::Glyph::Icon(Icon::Folder)), crate::tl_id!("ui.open_folder"), t)
                .on_hover_text(self.shown(&node.path))
                .clicked()
            {
                acts.push(Act::Reveal(node.path.clone()));
                ui.close();
            }
            if node.info.kind == NodeKind::Page
                && floating::menu_row(ui, Some(floating::Glyph::Icon(Icon::PageStack)), crate::tl_id!("ui.page_versions_2"), t).clicked()
            {
                acts.push(Act::PageVersions(node.path.clone()));
                ui.close();
            }
            floating::menu_separator(ui, t);
            if floating::menu_row(ui, Some(floating::Glyph::Mark(crate::ribbon_icons::Mark::Delete)), crate::tl_id!("ui.delete_ellipsis"), t)
                .clicked()
            {
                acts.push(Act::Delete(node.path.clone(), node.info.title.clone(), node.count()));
                ui.close();
            }
        });
        if has_kids && open {
            let kids: Vec<&Node> = node.children.iter().filter(|c| pages || c.info.kind != NodeKind::Page).collect();
            if pages {
                // Sub-pages get the same gaps as top-level pages: one before each, one after the
                // last. A gap that is dropped here becomes a child of this page.
                let mut last: Option<u32> = None;
                for c in kids {
                    self.page_gap(ui, &node.path, last, Some(c.info.order), acts);
                    self.draw_row(ui, c, depth + 1, acts, pages);
                    last = Some(c.info.order);
                }
                self.page_gap(ui, &node.path, last, None, acts);
            } else {
                for c in kids {
                    self.draw_row(ui, c, depth + 1, acts, pages);
                }
            }
        }
    }

    /// The first section anywhere in the tree, for a window that has nothing selected.
    fn first_section_of(nodes: &[Node]) -> Option<&Node> {
        for n in nodes {
            if n.info.kind == NodeKind::Section {
                return Some(n);
            }
            if let Some(found) = Self::first_section_of(&n.children) {
                return Some(found);
            }
        }
        None
    }

    /// The section whose pages the pages column shows: the selected node if it is one, else
    /// the section the selected page sits in, else the first section there is.
    fn current_section<'a>(&self, tree: &'a [Node]) -> Option<&'a Node> {
        let selected = self.selected.clone();
        fn walk<'a>(nodes: &'a [Node], want: &Option<PathBuf>, best: &mut Option<&'a Node>, inside: Option<&'a Node>) {
            for n in nodes {
                let section = if n.info.kind == NodeKind::Section { Some(n) } else { inside };
                if Some(&n.path) == want.as_ref() {
                    *best = match n.info.kind {
                        NodeKind::Section => Some(n),
                        NodeKind::Page => section,
                        _ => *best,
                    };
                }
                walk(&n.children, want, best, section);
            }
        }
        let mut best = None;
        walk(tree, &selected, &mut best, None);
        best.or_else(|| Self::first_section_of(tree))
    }

    // ---------- panels ----------

    fn top_bar(&mut self, ui: &mut Ui) {
        self.title_band(ui, true);
        if self.is_simple_mobile() {
            self.ribbon_bar(ui);
        } else {
            self.ribbon_bar(ui);
            // Below the ribbon, as OneNote can put it.
            if self.qat.below && !self.qat.hidden {
                let tokens = self.tokens;
                egui::Panel::top("quick-access").show(ui, |ui| {
                    ui.horizontal(|ui| self.quick_access(ui, tokens));
                });
            }
            self.section_tabs_panel(ui);
        }
    }

    fn section_tabs_panel(&mut self, ui: &mut Ui) {
        if !self.section_tabs_bar || self.is_simple_mobile() || self.backstage.is_some() {
            return;
        }
        struct SectionTabItem {
            path: PathBuf,
            title: String,
            colour: Option<NodeColor>,
            first_page: Option<PathBuf>,
        }
        fn collect_sections(nodes: &[Node], out: &mut Vec<SectionTabItem>) {
            for n in nodes {
                if n.info.kind == NodeKind::Section {
                    let first_page = n.children.iter().find(|c| c.info.kind == NodeKind::Page).map(|c| c.path.clone());
                    out.push(SectionTabItem { path: n.path.clone(), title: n.info.title.clone(), colour: n.info.color, first_page });
                } else {
                    collect_sections(&n.children, out);
                }
            }
        }
        let mut tabs = Vec::new();
        collect_sections(self.session.tree(), &mut tabs);
        if tabs.is_empty() {
            return;
        }
        let active_section = self.current_section(self.session.tree()).map(|n| n.path.clone());
        let tokens = self.tokens;
        let dark = tokens.editor.r() < 128;
        let mut clicked_section: Option<(PathBuf, Option<PathBuf>)> = None;
        let mut add_section_parent: Option<(PathBuf, NodeKind)> = None;
        let mut tab_acts: Vec<Act> = Vec::new();
        let new_target = self.new_section_parent();
        egui::Panel::top("section-tabs")
            .frame(egui::Frame::new().fill(tokens.band).inner_margin(egui::Margin { left: 8, right: 8, top: 4, bottom: 0 }))
            .show_separator_line(false)
            .show(ui, |ui| {
                egui::ScrollArea::horizontal().id_salt("section-tabs-scroll").show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
                        for tab in &tabs {
                            let is_active = active_section.as_ref() == Some(&tab.path);
                            let accent = tab
                                .colour
                                .map(|c| {
                                    let (r, g, b) = c.swatch_rgb(dark);
                                    Color32::from_rgb(r, g, b)
                                })
                                .unwrap_or(tokens.accent);
                            let font = egui::FontId::proportional(13.5);
                            let text_col = if is_active { tokens.strong } else { tokens.on_band().text };
                            let galley = ui.painter().layout_no_wrap(tab.title.clone(), font.clone(), text_col);
                            let width = (galley.size().x + 26.0).clamp(72.0, 220.0);
                            let (rect, resp) = ui.allocate_exact_size(vec2(width, 28.0), Sense::click());
                            let painter = ui.painter_at(rect);
                            let fill = if is_active {
                                tokens.editor
                            } else if resp.hovered() {
                                crate::theme::mix(tokens.band, accent, 0.35)
                            } else {
                                crate::theme::mix(tokens.band, accent, 0.18)
                            };
                            painter.rect_filled(rect, egui::CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 }, fill);
                            // Signature OneNote coloured strip along the top edge of the section tab.
                            let stripe = egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, rect.min.y + 3.5));
                            painter.rect_filled(stripe, egui::CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 }, accent);
                            painter.text(rect.center() + vec2(0.0, 1.5), Align2::CENTER_CENTER, &tab.title, font, text_col);
                            let label = tab.title.clone();
                            resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, true, is_active, label.clone()));
                            resp.context_menu(|ui| {
                                draw_node_color_menu(ui, &tab.path, tab.colour, dark, &mut tab_acts);
                            });
                            if resp.on_hover_text(&tab.title).clicked() {
                                clicked_section = Some((tab.path.clone(), tab.first_page.clone()));
                            }
                        }
                        if let Some((parent, kind)) = new_target.clone() {
                            let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
                            let painter = ui.painter_at(rect);
                            if resp.hovered() {
                                painter.rect_filled(rect, 5.0, tokens.on_band().panel);
                            }
                            painter.text(rect.center(), Align2::CENTER_CENTER, "+", egui::FontId::proportional(16.0), tokens.on_band().text);
                            if resp.on_hover_text(crate::tl_id!("ui.new_section_button")).clicked() {
                                add_section_parent = Some((parent, kind));
                            }
                        }
                    });
                });
            });
        for a in tab_acts {
            self.apply(a);
        }
        if let Some((sec_path, first_page)) = clicked_section {
            let mut at = sec_path.parent();
            while let Some(dir) = at {
                if !dir.starts_with(self.session.root()) || dir == self.session.root() {
                    break;
                }
                self.open.insert(dir.to_path_buf());
                at = dir.parent();
            }
            self.open.insert(sec_path.clone());
            self.selected = Some(sec_path.clone());
            let already_in_section = self.editor.as_ref().is_some_and(|e| e.dir.starts_with(&sec_path));
            if !already_in_section && let Some(page) = first_page {
                self.open_page(&page);
            }
        }
        if let Some((parent, kind)) = add_section_parent {
            self.apply(Act::AddKind(parent, kind, NodeKind::Section));
        }
    }

    /// Says that the window has no frame of the system's own, so this window draws its buttons
    /// and takes care of moving and resizing it. The program calls this when it asked for no frame.
    pub fn set_frameless(&mut self, on: bool) {
        self.frameless = on;
    }

    /// The coloured band across the top of the window, as in Office: the Quick Access Toolbar,
    /// the page's title, search and the window's buttons in one row, and under it the tab
    /// names. The File screen shows the first row only.
    fn title_band(&mut self, ui: &mut Ui, with_tabs: bool) {
        if self.is_simple_mobile() {
            self.mobile_title_band(ui);
            return;
        }
        let t = self.tokens;
        let (tab, source, in_table, has_image) = (self.ribbon_tab.clone(), self.show_source, self.caret_in_table(), self.page_has_image());
        let title = self.editor.as_ref().map(|e| e.title.trim().to_owned()).filter(|s| !s.is_empty());
        let mut acts = Vec::new();
        let collapsed = self.ribbon_folded();
        let band = egui::Panel::top("band").frame(egui::Frame::new().fill(t.band)).show_separator_line(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
            // One row: logo, quick access, tab names, then the controls (issue: save room).
            let tabs = with_tabs.then_some((tab.as_str(), source, in_table, has_image, collapsed));
            acts.extend(self.title_row(ui, title.as_deref(), tabs));
        });
        self.band_bottom = band.response.rect.max.y;
        for a in acts {
            self.ribbon_act(a);
        }
        self.update_banner(ui);
    }

    /// The band's first row. Anything on it that is not a control moves the window.
    fn title_row(&mut self, ui: &mut Ui, title: Option<&str>, tabs: Option<(&str, bool, bool, bool, bool)>) -> Vec<ribbon::Act> {
        const H: f32 = 32.0;
        let mut acts = Vec::new();
        let on = self.tokens.on_band();
        let ctx = ui.ctx().clone();
        let row = egui::Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), H));
        // Added first, so the controls drawn over it win the pointer: only bare band is a handle.
        let handle = ui.interact(row, ui.id().with("title-handle"), Sense::click_and_drag());
        if self.frameless {
            if handle.drag_started_by(egui::PointerButton::Primary) {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if handle.double_clicked() {
                let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
            }
        }
        // The choice between the system's title bar and this one, kept in the settings.
        handle.context_menu(|ui| {
            let label = if self.frameless { crate::tl_id!("ui.use_the_system_title_bar") } else { crate::tl_id!("ui.use_ergonote_s_own_title_bar") };
            if ui.button(label).clicked() {
                self.frameless = !self.frameless;
                ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(!self.frameless));
                let _ = self.session.settings.set("window_frame", if self.frameless { "own" } else { "system" });
                ui.close();
            }
        });
        let mut left = ui.new_child(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::left_to_right(egui::Align::Center)));
        left.add_space(8.0);
        left.add(egui::Image::from_bytes("bytes://ergonote-logo", crate::LOGO_PNG).fit_to_exact_size(vec2(22.0, 22.0)));
        left.add_space(6.0);
        if self.qat.hidden || !self.qat.below {
            self.quick_access(&mut left, on);
        }
        if let Some((tab, source, in_table, has_image, collapsed)) = tabs {
            left.add_space(6.0);
            let t = self.tokens;
            ribbon::tab_names(&mut left, tab, source, in_table, has_image, collapsed, &t, &mut acts);
        }
        let mut right = ui.new_child(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::right_to_left(egui::Align::Center)));
        if self.frameless {
            self.window_buttons(&mut right, &on);
        }
        right.add_space(6.0);
        if let Some((_, _, _, _, collapsed)) = tabs {
            let t = self.tokens;
            ribbon::fold_button(&mut right, collapsed, &t, &mut acts);
            right.add_space(4.0);
        }
        if let Some((ver, ..)) = self.available_update() {
            if Self::update_badge(&mut right, on, &ver) {
                self.update_dismissed_version = None;
                let _ = self.session.settings.set("update_dismissed_version", "");
            }
            right.add_space(4.0);
        }
        if Self::version_badge(&mut right, on) {
            self.dialog = Some(Dialog::ReleaseNotes);
        }
        right.add_space(4.0);
        if Self::coffee_badge(&mut right, on) {
            self.dialog = Some(Dialog::SupportCreator);
        }
        right.add_space(4.0);
        if Self::bug_badge(&mut right, on, self.compact) {
            self.dialog = Some(Dialog::ReportBug);
        }
        right.add_space(4.0);
        self.theme_menu(&mut right, on);
        right.add_space(4.0);
        self.language_menu(&mut right, on);
        right.add_space(10.0);
        if !self.compact {
            if self.discord_link(&mut right, on) {
                self.dialog = Some(Dialog::BackupRecovery { feedback: None });
            }
            right.add_space(10.0);
        }
        // The page's name, centred in what the two sides leave; left out when it does not fit.
        let text = title.map_or_else(|| "ErgoNote".to_owned(), |t| format!("ErgoNote — {t}"));
        let free = (left.min_rect().right() + 12.0)..=(right.min_rect().left() - 12.0);
        let half = text.chars().count() as f32 * 3.6;
        if !self.compact && free.start() + half <= row.center().x && row.center().x + half <= *free.end() {
            let band = ui.painter_at(row);
            crate::bidi::line_at(ui, &band, row.center(), Align2::CENTER_CENTER, &text, 13.5, on.text, None);
        }
        ui.advance_cursor_after_rect(row);
        acts
    }

    /// Clickable version badge in the top-right corner opening the release-notes popup.
    fn version_badge(ui: &mut Ui, on: Tokens) -> bool {
        // Year dropped to save room: 2026.11.24 shows as 11.24.
        let v = crate::backstage::VERSION;
        let label = match v.split_once('.') {
            Some((year, rest)) if year.len() == 4 && year.chars().all(|c| c.is_ascii_digit()) => rest.to_owned(),
            _ => format!("v{v}"),
        };
        let font = egui::FontId::proportional(11.5);
        let galley = ui.painter().layout_no_wrap(label.clone(), font.clone(), on.text);
        let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 12.0, 20.0), Sense::click());
        let bg = if resp.hovered() { crate::theme::mix(on.panel, on.text, 0.14) } else { on.panel };
        ui.painter().rect_filled(rect, 5.0, bg);
        ui.painter().rect_stroke(rect, 5.0, egui::Stroke::new(1.0, on.container_border), egui::StrokeKind::Inside);
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, &label, font, on.text);
        resp.on_hover_text(format!("ErgoNote v{} — Voir les nouveautés", crate::backstage::VERSION)).clicked()
    }

    /// Warm coffee button in the header band opening the creator-support dialog.
    fn coffee_badge(ui: &mut Ui, on: Tokens) -> bool {
        let warm = Color32::from_rgb(180, 95, 20);
        let warm_bg = Color32::from_rgb(255, 244, 214);
        let label = "Offrir un café";
        let font = egui::FontId::proportional(11.5);
        let galley = ui.painter().layout_no_wrap(label.to_owned(), font.clone(), warm);
        let (rect, resp) = ui.allocate_exact_size(vec2(28.0, 21.0), Sense::click());
        let _ = &galley;
        let bg = if resp.hovered() { Color32::from_rgb(255, 232, 179) } else { warm_bg };
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 6.0, bg);
        painter.rect_stroke(rect, 6.0, egui::Stroke::new(1.0, crate::theme::mix(warm, on.container_border, 0.35)), egui::StrokeKind::Inside);
        let ico_r = egui::Rect::from_center_size(rect.center(), vec2(14.0, 14.0));
        icons::paint(&painter, ico_r, Icon::Coffee, warm);
        let _ = (label, font);
        resp.on_hover_text("Offrir un café pour soutenir ErgoNote (Ko-fi)").clicked()
    }

    /// Discreet button in the top-right bar opening the bug-report / feedback dialog.
    fn bug_badge(ui: &mut Ui, on: Tokens, compact: bool) -> bool {
        let label = if compact { "Bug" } else { "Signaler un bug" };
        let font = egui::FontId::proportional(11.5);
        let galley = ui.painter().layout_no_wrap(label.to_owned(), font.clone(), on.text);
        let _ = (&galley, &font, label);
        let (rect, resp) = ui.allocate_exact_size(vec2(28.0, 21.0), Sense::click());
        let bg = if resp.hovered() { crate::theme::mix(on.panel, on.text, 0.14) } else { on.panel };
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 6.0, bg);
        painter.rect_stroke(rect, 6.0, egui::Stroke::new(1.0, on.container_border), egui::StrokeKind::Inside);
        let ico_r = egui::Rect::from_center_size(rect.center(), vec2(13.0, 13.0));
        icons::paint(&painter, ico_r, Icon::Help, on.text);
        resp.on_hover_text("Signaler un bug ou proposer une amélioration (formulaire sans compte)").clicked()
    }

    /// Discreet green pill shown next to the version badge when a newer release is available on GitHub.
    fn update_badge(ui: &mut Ui, on: Tokens, version: &str) -> bool {
        let short = version.strip_suffix(".0").unwrap_or(version);
        let label = format!("Maj v{short}");
        let font = egui::FontId::proportional(11.5);
        let fg = Color32::WHITE;
        let galley = ui.painter().layout_no_wrap(label.clone(), font.clone(), fg);
        let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 14.0, 21.0), Sense::click());
        let bg = if resp.hovered() { Color32::from_rgb(21, 128, 61) } else { Color32::from_rgb(22, 163, 74) };
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 6.0, bg);
        painter.rect_stroke(rect, 6.0, egui::Stroke::new(1.0, crate::theme::mix(bg, on.text, 0.25)), egui::StrokeKind::Inside);
        painter.text(rect.center(), Align2::CENTER_CENTER, &label, font, fg);
        resp.on_hover_text(format!("Nouvelle version ErgoNote v{version} disponible — Afficher / Télécharger")).clicked()
    }

    /// Non-destructive top banner displayed when a newer release exists on GitHub.
    fn update_banner(&mut self, ui: &mut Ui) {
        let Some((version, download_url, release_page)) = self.available_update() else {
            return;
        };
        if self.update_dismissed_version.as_deref() == Some(version.as_str()) {
            return;
        }
        let tokens = self.tokens;
        let mut download_clicked = false;
        let mut notes_clicked = false;
        let mut later_clicked = false;
        let banner = egui::Panel::top("update-banner")
            .frame(
                egui::Frame::new()
                    .fill(crate::theme::mix(tokens.panel, tokens.accent, 0.10))
                    .inner_margin(egui::Margin::symmetric(10, 5))
                    .stroke(egui::Stroke::new(1.0, crate::theme::mix(tokens.accent, tokens.container_border, 0.45))),
            )
            .show_separator_line(false)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(8.0, 4.0);
                    ui.label(
                        egui::RichText::new(format!("Nouvelle version d'ErgoNote disponible : v{version} (vos classeurs restent intacts)."))
                            .size(12.5)
                            .strong()
                            .color(tokens.strong),
                    );
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new(format!("Télécharger v{version}")).size(12.0).strong().color(Color32::WHITE))
                                .fill(tokens.accent)
                                .corner_radius(6.0),
                        )
                        .clicked()
                    {
                        download_clicked = true;
                    }
                    if ui.add(egui::Button::new(egui::RichText::new("Notes").size(12.0).color(tokens.text)).corner_radius(6.0)).clicked() {
                        notes_clicked = true;
                    }
                    if ui.add(egui::Button::new(egui::RichText::new("Plus tard").size(12.0).color(tokens.muted)).corner_radius(6.0)).clicked() {
                        later_clicked = true;
                    }
                });
            });
        self.band_bottom = self.band_bottom.max(banner.response.rect.max.y);
        if download_clicked {
            self.open_external_url(ui.ctx(), &download_url);
            self.say("Téléchargement ouvert dans votre navigateur.");
        } else if notes_clicked {
            self.open_external_url(ui.ctx(), &release_page);
            self.say("Notes de version ouvertes dans votre navigateur.");
        } else if later_clicked {
            self.dismiss_update_banner(&version);
        }
    }

    /// Student backup & recovery button in the header band (cross-platform).
    fn discord_link(&self, ui: &mut Ui, on: Tokens) -> bool {
        let dirty = self.is_dirty() || self.pad.as_ref().is_some_and(|p| p.dirty);
        let label = if dirty { crate::tl_id!("ui.saving") } else { crate::tl_id!("ui.backup_recovery") };
        let (rect, resp) = ui.allocate_exact_size(vec2(28.0, 25.0), Sense::click());
        let bg = if resp.hovered() { crate::theme::mix(on.row_hover, on.text, 0.16) } else { on.row_hover };
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 12.0, bg);
        painter.rect_stroke(rect, 12.0, egui::Stroke::new(1.0, crate::theme::mix(on.text, on.panel, 0.45)), egui::StrokeKind::Inside);
        // Crisp vector floppy / backup icon on the left (works on every OS without emoji fonts).
        let icon_rect = egui::Rect::from_center_size(rect.center(), vec2(13.0, 13.0));
        Self::paint_floppy(&painter, icon_rect, on.text, on.panel);
        let _ = label;
        let tip = format!(
            "Cliquez pour sauvegarder ou récupérer vos classeurs (.zip) sur n'importe quel appareil.\nDossier actuel : {}",
            self.session.root().display()
        );
        resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(tip).clicked()
    }

    /// A floppy disk, the mark every Office user knows for "save", drawn rather than typed so
    /// it looks the same on every system. `ink` draws the outline and the shutter; the label
    /// below is quietened towards `surface`.
    fn paint_floppy(painter: &egui::Painter, rect: egui::Rect, ink: Color32, surface: Color32) {
        let unit = rect.width() / 13.0;
        painter.rect_stroke(rect, 2.0 * unit, egui::Stroke::new((1.4 * unit).max(1.0), ink), egui::StrokeKind::Inside);
        painter.rect_filled(egui::Rect::from_min_size(rect.min + vec2(3.0 * unit, 1.0 * unit), vec2(7.0 * unit, 4.0 * unit)), 1.0 * unit, ink);
        painter.rect_filled(
            egui::Rect::from_min_size(rect.min + vec2(3.0 * unit, 7.5 * unit), vec2(7.0 * unit, 4.5 * unit)),
            1.0 * unit,
            crate::theme::mix(ink, surface, 0.35),
        );
    }

    /// Minimise, maximise or restore, and close: the buttons the system would have drawn.
    fn window_buttons(&self, ui: &mut Ui, on: &Tokens) {
        let ctx = ui.ctx().clone();
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        for kind in ["Close", "Maximize", "Minimize"] {
            let (rect, resp) = ui.allocate_exact_size(vec2(46.0, 30.0), Sense::click());
            let painter = ui.painter_at(rect);
            if resp.hovered() {
                let fill = if kind == "Close" { Color32::from_rgb(196, 43, 28) } else { on.row_hover };
                painter.rect_filled(rect, 0.0, fill);
            }
            let c = rect.center();
            let stroke = egui::Stroke::new(1.0, on.text);
            match kind {
                "Close" => {
                    painter.line_segment([c + vec2(-5.0, -5.0), c + vec2(5.0, 5.0)], stroke);
                    painter.line_segment([c + vec2(-5.0, 5.0), c + vec2(5.0, -5.0)], stroke);
                }
                "Maximize" if maximized => {
                    painter.rect_stroke(egui::Rect::from_min_size(c + vec2(-5.0, -2.0), vec2(7.0, 7.0)), 0.0, stroke, egui::StrokeKind::Inside);
                    painter.line_segment([c + vec2(-2.0, -5.0), c + vec2(5.0, -5.0)], stroke);
                    painter.line_segment([c + vec2(5.0, -5.0), c + vec2(5.0, 2.0)], stroke);
                }
                "Maximize" => {
                    painter.rect_stroke(egui::Rect::from_center_size(c, vec2(10.0, 10.0)), 0.0, stroke, egui::StrokeKind::Inside);
                }
                _ => {
                    painter.line_segment([c + vec2(-5.0, 4.0), c + vec2(5.0, 4.0)], stroke);
                }
            }
            let label = if kind == crate::tl_id!("ui.maximize") && maximized { crate::tl_id!("ui.restore") } else { kind };
            let enabled = ui.is_enabled();
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
            if resp.on_hover_text(label).clicked() {
                ctx.send_viewport_cmd(match kind {
                    "Close" => egui::ViewportCommand::Close,
                    "Maximize" => egui::ViewportCommand::Maximized(!maximized),
                    _ => egui::ViewportCommand::Minimized(true),
                });
            }
        }
    }

    /// With no frame of the system's own, the edges and corners of the window are ours: the
    /// pointer shows the resize arrow there, and a press starts the resize.
    fn frame_edges(&self, ctx: &Context) {
        if !self.frameless || ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
            return;
        }
        let r = ctx.content_rect();
        // A line round the window, so its edge shows against whatever is behind it.
        ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("window-edge"))).rect_stroke(
            r,
            0.0,
            egui::Stroke::new(1.0, self.tokens.band),
            egui::StrokeKind::Inside,
        );
        let Some(pos) = ctx.pointer_latest_pos() else { return };
        const E: f32 = 5.0;
        let (west, east, north, south) = (pos.x - r.min.x < E, r.max.x - pos.x < E, pos.y - r.min.y < E, r.max.y - pos.y < E);
        use egui::{CursorIcon as C, viewport::ResizeDirection as D};
        let (dir, cursor) = match (west, east, north, south) {
            (true, _, true, _) => (D::NorthWest, C::ResizeNorthWest),
            (_, true, true, _) => (D::NorthEast, C::ResizeNorthEast),
            (true, _, _, true) => (D::SouthWest, C::ResizeSouthWest),
            (_, true, _, true) => (D::SouthEast, C::ResizeSouthEast),
            (true, ..) => (D::West, C::ResizeWest),
            (_, true, ..) => (D::East, C::ResizeEast),
            (_, _, true, _) => (D::North, C::ResizeNorth),
            (_, _, _, true) => (D::South, C::ResizeSouth),
            _ => return,
        };
        ctx.set_cursor_icon(cursor);
        if ctx.input(|i| i.pointer.primary_pressed()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
        }
    }

    /// The File screen: notebooks and what is done to a whole notebook.
    fn backstage_screen(&mut self, ui: &mut Ui) {
        let page = self.backstage.unwrap_or("Info");
        // Save a Copy counts from the page that is open (issue #301).
        let mut copy = std::mem::take(&mut self.copy_choice);
        copy.here = self.editor.as_ref().map(|e| e.dir.clone());
        let tokens = self.tokens;
        let folder = self.shown(self.session.root());
        let recent = self.recent_notebooks();
        let (root, redact) = (self.session.root().to_path_buf(), self.redact_paths);
        let show_path = move |p: &Path| shown_in(&root, p, redact);
        let ui_language = self.ui_language.clone();
        let languages = crate::i18n::available_languages();
        let mut acts = Vec::new();
        egui::CentralPanel::default().show(ui, |ui| {
            let telemetry_root = self.session.settings.path().parent().unwrap_or(std::path::Path::new(".")).join("telemetry");
            let telemetry_level = self.session.settings.get("telemetry_level").unwrap_or_else(|| "none".into());
            let session_id = self.session.settings.get("telemetry_session_id");
            let update = self.update.lock().map(|s| s.clone()).unwrap_or_default();
            acts = backstage::show(
                ui,
                self.session.tree(),
                &recent,
                &folder,
                page,
                &show_path,
                &telemetry_root,
                &telemetry_level,
                session_id.as_deref(),
                &ui_language,
                &languages,
                self.ui_scale,
                &update,
                &mut copy,
                &tokens,
            );
        });
        for a in acts {
            match a {
                backstage::Act::Close => self.backstage = None,
                backstage::Act::Page(name) => self.backstage = Some(name),
                backstage::Act::NewNotebook => {
                    self.backstage = None;
                    self.dialog = Some(Dialog::NewNotebook { name: String::new(), format: None });
                }
                backstage::Act::OpenFolder(p) => self.reveal(&p),
                backstage::Act::ChooseFolder => self.choose_notebooks_folder(),
                backstage::Act::ImportNotebookFolder => self.import_notebook_folder(),
                backstage::Act::ImportFile => self.import_file(),
                backstage::Act::ImportCloud => {
                    self.status = crate::tl_id!("ui.cloud_import_is_not_implemented_yet").to_string();
                    self.status_is_error = false;
                }
                backstage::Act::OpenNotebooksFolder => {
                    let root = self.session.root().to_path_buf();
                    self.reveal(&root);
                }
                backstage::Act::Open(p) => {
                    self.open_notebook_from_path(&p);
                }
                backstage::Act::OpenRecent(p) => {
                    self.open_notebook_from_path(&p);
                }
                backstage::Act::RemoveRecent(p) => self.remove_recent_notebook(&p),
                backstage::Act::DeleteRecent(p, title) => self.dialog = Some(Dialog::DeleteNotebook { path: p, title }),
                backstage::Act::SetTelemetryLevel(level) => {
                    let _ = self.session.settings.set("telemetry_level", &level);
                }
                backstage::Act::SetUiLanguage(language) => {
                    self.set_ui_language(language);
                }
                backstage::Act::SetUiScale(percent) => self.set_ui_scale(percent),
                backstage::Act::OpenTelemetryFolder(path) => {
                    // The folder is made on first use: it does not exist until a session has
                    // written to it, and Explorer opens Documents for a folder it cannot find.
                    match std::fs::create_dir_all(&path) {
                        Ok(()) => self.reveal(&path),
                        Err(e) => {
                            self.status =
                                crate::i18n::format_message_id("ui.the_diagnostics_folder_could_not_be_created_error", &[("error", &e.to_string())]);
                            self.status_is_error = true;
                        }
                    }
                }
                backstage::Act::ClearTelemetry(path) => self.dialog = Some(Dialog::ClearTelemetry { path }),
                backstage::Act::CheckForUpdates => {
                    if cfg!(target_os = "android") {
                        self.android_update_check_requested = true;
                    }
                    crate::update::start(ui.ctx().clone(), self.update.clone());
                    self.say("Recherche d'une nouvelle version d'ErgoNote…");
                }
                backstage::Act::OpenUpdateDownload(url) => {
                    self.open_external_url(ui.ctx(), &url);
                    self.say("Lien de téléchargement ouvert dans votre navigateur.");
                }
                backstage::Act::OpenReleases => {
                    self.open_external_url(ui.ctx(), backstage::LATEST_RELEASE);
                    self.say("Page des versions ouverte dans votre navigateur.");
                }
                backstage::Act::ResetPreferences => self.dialog = Some(Dialog::ResetPreferences),
                backstage::Act::ExportTelemetry(root) => {
                    if let Some(session) = std::fs::read_dir(&root)
                        .ok()
                        .and_then(|r| r.filter_map(Result::ok).find(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false)))
                        && let Some(destination) = rfd::FileDialog::new()
                            .set_title(crate::tl_id!("ui.export_telemetry_zip"))
                            .set_file_name(format!("ergonote-{}-telemetry.zip", session.file_name().to_string_lossy()))
                            .save_file()
                    {
                        let _ = crate::telemetry_export::export(&session.path(), &destination);
                    }
                }
                backstage::Act::ChooseCopyFolder => {
                    if let Some(folder) = pick_copy_folder() {
                        copy.folder = folder.to_string_lossy().into_owned();
                    }
                }
                backstage::Act::SaveCopy { node, folder } => {
                    copy.result = Some(ergonote_store::ops::save_copy(&node, &folder).map_err(|e| e.to_string()));
                    if let Some(Ok(at)) = &copy.result {
                        self.say(crate::i18n::format_message_id("ui.saved_a_copy_in_path", &[("path", &self.shown(at))]));
                    }
                }
                backstage::Act::PrintPreviewPdf => {
                    self.handle_print_preview_pdf();
                }
                backstage::Act::PrintDirect => {
                    self.handle_print_direct();
                }
                backstage::Act::PrintBrowser => {
                    self.handle_print_browser();
                }
                backstage::Act::SharePdf => {
                    self.handle_share_pdf();
                }
                backstage::Act::ShareEmail => {
                    self.handle_share_email();
                }
                backstage::Act::ShareCopyText => {
                    self.handle_share_copy_text();
                }
                backstage::Act::ExportPagePdf => {
                    self.handle_export_page_pdf();
                }
                backstage::Act::ExportPageDoc => {
                    self.handle_export_page_doc();
                }
                backstage::Act::ExportPageHtml => {
                    self.handle_export_page_html();
                }
                backstage::Act::ExportNotebookZip => {
                    let _ = self.handle_export_notebook_zip();
                }
                backstage::Act::ImportZip => {
                    let _ = self.handle_import_zip();
                }
                backstage::Act::NotBuilt(name) => {
                    self.status =
                        crate::i18n::format_message_id("ui.name_is_not_built_yet_see_the_roadmap_for_what_is_coming", &[("name", crate::tl!(name))]);
                    self.status_is_error = false;
                }
            }
        }
        self.copy_choice = copy;
    }

    fn build_printable_page_html(&mut self) -> Result<(PathBuf, String, String), String> {
        self.save_now();
        let ed = self.editor.as_ref().ok_or_else(|| "Ouvrez d'abord une page pour effectuer cette action.".to_string())?;
        let page_dir = ed.dir.clone();
        let raw_title = if ed.title.trim().is_empty() { "Cours".to_string() } else { ed.title.trim().to_string() };
        let safe_title = export_safe_title(&raw_title);
        let arrangement = (ed.arranged && !ed.boxes.is_empty()).then_some(ed.boxes.as_slice());
        let html_body = ergonote_formats::boxes::write(ergonote_model::PageFormat::Html, &ed.blocks, arrangement)
            .replace("<p><img src=\"ink.svg\" alt=\"Ink\"></p>", "")
            .replace("<img src=\"ink.svg\" alt=\"Ink\">", "");
        let ink_overlay = if page_dir.join("ink.svg").is_file() {
            "<img src=\"ink.svg\" style=\"position:absolute;left:0;top:0;pointer-events:none;max-width:none;z-index:20;\" alt=\"\" />"
        } else {
            ""
        };
        let base_href = file_base_href(&page_dir);
        let full_html = format!(
            "<!doctype html>\n<html lang=\"fr\">\n<head>\n<meta charset=\"utf-8\">\n<base href=\"{base}\">\n<title>{title}</title>\n<style>\n\
            @page {{ size: A4 portrait; margin: 10mm; }}\n\
            * {{ box-sizing: border-box; }}\n\
            body {{ font-family: Ubuntu, 'DejaVu Sans', system-ui, -apple-system, sans-serif; font-size: 14.5px; line-height: 1.25; color: #000000; background: #ffffff; margin: 0; padding: 4px 8px; }}\n\
            h1.page-title {{ font-size: 20px; font-weight: 400; margin: 0 0 8px 0; border-bottom: 1px solid #d8d8e0; padding-bottom: 4px; color: #555566; }}\n\
            .nc-canvas {{ position: relative; min-height: 980px; width: 100%; }}\n\
            .nc-box {{ padding: 14px 6px 6px 6px; background: transparent; color: #000000; }}\n\
            .nc-box h1 {{ font-size: 28px; font-weight: 400; line-height: 1.15; margin: 6px 0 2px 0; color: #000000; }}\n\
            .nc-box h2 {{ font-size: 22px; font-weight: 400; line-height: 1.15; margin: 6px 0 2px 0; color: #000000; }}\n\
            .nc-box h3 {{ font-size: 18px; font-weight: 400; line-height: 1.15; margin: 6px 0 2px 0; color: #000000; }}\n\
            .nc-box h4, .nc-box h5, .nc-box h6 {{ font-size: 16px; font-weight: 400; line-height: 1.15; margin: 6px 0 2px 0; color: #000000; }}\n\
            .nc-box p {{ font-size: 14.5px; line-height: 1.25; margin: 0 0 4px 0; color: #000000; }}\n\
            .nc-box ul, .nc-box ol {{ font-size: 14.5px; line-height: 1.25; margin: 0 0 2px 0; padding-left: 22px; color: #000000; }}\n\
            .nc-box li {{ font-size: 14.5px; line-height: 1.25; margin: 0 0 2px 0; color: #000000; }}\n\
            .nc-box p:has(> img) {{ margin: 0 0 4px 0; line-height: 0; }}\n\
            table {{ border-collapse: collapse; margin: 4px 0 6px 0; font-size: 14.5px; }}\n\
            th, td {{ border: 1px solid #666; padding: 4px 8px; }}\n\
            img {{ display: block; max-width: 100%; }}\n\
            </style>\n</head>\n<body>\n<h1 class=\"page-title\">{title}</h1>\n<div class=\"nc-canvas\">\n{body}\n{ink}\n</div>\n</body>\n</html>\n",
            base = ergonote_formats::html::escape(&base_href),
            title = ergonote_formats::html::escape(&raw_title),
            body = html_body,
            ink = ink_overlay
        );
        let html_path = page_dir.join("_impression.html");
        std::fs::write(&html_path, &full_html).map_err(|e| e.to_string())?;
        Ok((html_path, safe_title, full_html))
    }

    fn generate_page_pdf(&mut self, dest_pdf: &Path) -> Result<(), String> {
        let (html_path, _, _) = self.build_printable_page_html()?;
        let _ = std::fs::remove_file(dest_pdf);
        let pdf_arg = format!("--print-to-pdf={}", dest_pdf.display());
        let browsers: &[&str] = if cfg!(target_os = "windows") {
            &[
                "msedge",
                r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
                r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
                "chrome",
                r"C:\Program Files\Google\Chrome\Application\chrome.exe",
                r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
            ]
        } else if cfg!(target_os = "macos") {
            &[
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
                "/Applications/Chromium.app/Contents/MacOS/Chromium",
                "google-chrome",
                "chromium",
            ]
        } else {
            &["google-chrome", "chromium", "chromium-browser", "microsoft-edge"]
        };
        for browser in browsers {
            let status = std::process::Command::new(browser)
                .args(["--headless", "--disable-gpu", "--no-pdf-header-footer", &pdf_arg])
                .arg(&html_path)
                .status();
            if status.is_ok_and(|s| s.success()) && dest_pdf.is_file() {
                return Ok(());
            }
        }
        Err("Impossible de générer le PDF automatiquement.".to_string())
    }

    fn handle_print_preview_pdf(&mut self) {
        match self.build_printable_page_html() {
            Ok((html_path, safe_title, _)) => {
                let pdf_path = html_path.with_file_name(format!("{safe_title}.pdf"));
                if self.generate_page_pdf(&pdf_path).is_ok() {
                    let _ = open_file_for_viewer(&pdf_path);
                    self.backstage = None;
                    self.say(format!("Aperçu PDF ouvert : {}", pdf_path.display()));
                } else {
                    let _ = open_file_for_viewer(&html_path);
                    self.backstage = None;
                    self.say("Page ouverte dans le navigateur pour impression.".to_string());
                }
            }
            Err(e) => {
                self.status = e;
                self.status_is_error = true;
            }
        }
    }

    fn handle_print_direct(&mut self) {
        match self.build_printable_page_html() {
            Ok((html_path, safe_title, _)) => {
                let pdf_path = html_path.with_file_name(format!("{safe_title}.pdf"));
                if self.generate_page_pdf(&pdf_path).is_ok() {
                    let sent = !cfg!(target_os = "windows") && std::process::Command::new("lp").arg(&pdf_path).status().is_ok_and(|s| s.success());
                    self.backstage = None;
                    if sent {
                        self.say("Page envoyée à l'imprimante par défaut.".to_string());
                    } else {
                        let _ = open_file_for_viewer(&pdf_path);
                        self.say("Aperçu PDF ouvert (Ctrl+P pour imprimer).".to_string());
                    }
                } else {
                    let _ = open_file_for_viewer(&html_path);
                    self.backstage = None;
                    self.say("Page ouverte dans le navigateur pour impression.".to_string());
                }
            }
            Err(e) => {
                self.status = e;
                self.status_is_error = true;
            }
        }
    }

    fn handle_print_browser(&mut self) {
        match self.build_printable_page_html() {
            Ok((html_path, _, _)) => {
                let _ = open_file_for_viewer(&html_path);
                self.backstage = None;
                self.say("Version imprimable ouverte dans le navigateur.".to_string());
            }
            Err(e) => {
                self.status = e;
                self.status_is_error = true;
            }
        }
    }

    fn push_file_download(&mut self, name: String, mime: &'static str, bytes: Vec<u8>) -> bool {
        if let Some(downloads) = self.file_downloads.as_mut() {
            downloads.push(FileDownload { name, mime, bytes });
            true
        } else {
            false
        }
    }

    fn handle_share_pdf(&mut self) {
        if self.file_downloads.is_some() {
            match self.build_printable_page_html() {
                Ok((_, safe_title, full_html)) => {
                    self.push_file_download(format!("{safe_title}.html"), "application/x-ergonote-share-html", full_html.into_bytes());
                    self.backstage = None;
                    self.say("Ouverture du menu de partage…".to_string());
                }
                Err(e) => {
                    self.status = e;
                    self.status_is_error = true;
                }
            }
            return;
        }
        let share_dir = self.session.root().parent().unwrap_or(self.session.root()).join("Partages");
        let _ = std::fs::create_dir_all(&share_dir);
        match self.build_printable_page_html() {
            Ok((_, safe_title, _)) => {
                let pdf_path = share_dir.join(format!("{safe_title}.pdf"));
                if self.generate_page_pdf(&pdf_path).is_ok() {
                    self.reveal(&share_dir);
                    self.backstage = None;
                    self.say(format!("PDF prêt à partager sur Pronote / ENT / E-mail : {}", pdf_path.display()));
                } else {
                    self.reveal(&share_dir);
                }
            }
            Err(e) => {
                self.status = e;
                self.status_is_error = true;
            }
        }
    }

    fn handle_share_email(&mut self) {
        if self.file_downloads.is_some() {
            if let Some(ed) = self.editor.as_ref() {
                let safe_title = export_safe_title(&ed.title);
                let body = ergonote_formats::render::blocks_to_markdown(&ed.blocks);
                self.push_file_download(safe_title, "application/x-ergonote-share-text", body.into_bytes());
                self.backstage = None;
                self.say("Ouverture du menu de partage…".to_string());
            } else {
                self.status = "Ouvrez d'abord une page à partager.".to_string();
                self.status_is_error = true;
            }
            return;
        }
        let share_dir = self.session.root().parent().unwrap_or(self.session.root()).join("Partages");
        let _ = std::fs::create_dir_all(&share_dir);
        match self.build_printable_page_html() {
            Ok((_, safe_title, _)) => {
                let pdf_path = share_dir.join(format!("{safe_title}.pdf"));
                let _ = self.generate_page_pdf(&pdf_path);
                let mailto = format!(
                    "mailto:?subject=Cours%20:%20{}&body=Bonjour,%0A%0AVoici%20le%20cours%20en%20PDF%20:%20{}",
                    safe_title,
                    pdf_path.display()
                );
                let _ = open_url_in_system(&mailto);
                self.backstage = None;
                self.say(format!("Messagerie ouverte (fichier prêt : {})", pdf_path.display()));
            }
            Err(e) => {
                self.status = e;
                self.status_is_error = true;
            }
        }
    }

    fn handle_share_copy_text(&mut self) {
        if let Some(ed) = self.editor.as_ref() {
            let text = ergonote_formats::render::blocks_to_markdown(&ed.blocks);
            self.pending_copy = Some(text);
            self.backstage = None;
            self.say("Tout le texte de la page a été copié dans le presse-papiers.".to_string());
        } else {
            self.status = "Ouvrez d'abord une page.".to_string();
            self.status_is_error = true;
        }
    }

    fn handle_export_page_pdf(&mut self) {
        match self.build_printable_page_html() {
            Ok((html_path, safe_title, full_html)) => {
                if let Some(downloads) = self.file_downloads.as_mut() {
                    downloads.push(FileDownload {
                        name: format!("{safe_title}.pdf"),
                        mime: "application/x-ergonote-print-pdf",
                        bytes: full_html.into_bytes(),
                    });
                    self.backstage = None;
                    self.say("Ouverture de l'enregistrement / impression PDF…".to_string());
                    return;
                }
                if let Some(dest) = rfd::FileDialog::new()
                    .set_title("Exporter la page en PDF")
                    .set_file_name(format!("{safe_title}.pdf"))
                    .add_filter("Document PDF", &["pdf"])
                    .save_file()
                {
                    if self.generate_page_pdf(&dest).is_ok() {
                        self.backstage = None;
                        self.say(format!("Page exportée en PDF : {}", dest.display()));
                    } else {
                        let _ = open_file_for_viewer(&html_path);
                        self.backstage = None;
                        self.say("Navigateur automatique absent : page ouverte dans le navigateur (Ctrl+P -> Enregistrer en PDF).".to_string());
                    }
                }
            }
            Err(e) => {
                self.status = e;
                self.status_is_error = true;
            }
        }
    }

    fn handle_export_page_doc(&mut self) {
        match self.build_printable_page_html() {
            Ok((html_path, safe_title, full_html)) => {
                if let Some(downloads) = self.file_downloads.as_mut() {
                    let name = format!("{safe_title}.doc");
                    downloads.push(FileDownload { name: name.clone(), mime: "application/msword", bytes: full_html.into_bytes() });
                    self.backstage = None;
                    self.say(format!("Enregistrement du document Word : {name}"));
                    return;
                }
                if let Some(dest) = rfd::FileDialog::new()
                    .set_title("Exporter pour Word / LibreOffice")
                    .set_file_name(format!("{safe_title}.doc"))
                    .add_filter("Document Word / LibreOffice", &["doc", "odt", "html"])
                    .save_file()
                {
                    let ext = dest.extension().and_then(|e| e.to_str()).unwrap_or("doc").to_ascii_lowercase();
                    if ext == "odt"
                        && let Some(out_dir) = dest.parent()
                    {
                        let _ = std::process::Command::new("libreoffice")
                            .args(["--headless", "--convert-to", "odt", "--outdir"])
                            .arg(out_dir)
                            .arg(&html_path)
                            .status();
                        let converted = out_dir.join("_impression.odt");
                        if converted.is_file() {
                            let _ = std::fs::rename(&converted, &dest);
                        }
                    } else {
                        let _ = std::fs::write(&dest, &full_html);
                    }
                    self.backstage = None;
                    self.say(format!("Document exporté : {}", dest.display()));
                }
            }
            Err(e) => {
                self.status = e;
                self.status_is_error = true;
            }
        }
    }

    fn handle_export_page_html(&mut self) {
        match self.build_printable_page_html() {
            Ok((_, safe_title, full_html)) => {
                if let Some(downloads) = self.file_downloads.as_mut() {
                    let name = format!("{safe_title}.html");
                    downloads.push(FileDownload { name: name.clone(), mime: "text/html", bytes: full_html.into_bytes() });
                    self.backstage = None;
                    self.say(format!("Enregistrement de la page HTML : {name}"));
                    return;
                }
                if let Some(dest) = rfd::FileDialog::new()
                    .set_title("Exporter la page en HTML")
                    .set_file_name(format!("{safe_title}.html"))
                    .add_filter("Page Web HTML", &["html"])
                    .save_file()
                    && std::fs::write(&dest, &full_html).is_ok()
                {
                    self.backstage = None;
                    self.say(format!("Page Web exportée : {}", dest.display()));
                }
            }
            Err(e) => {
                self.status = e;
                self.status_is_error = true;
            }
        }
    }

    fn handle_export_notebook_zip(&mut self) -> Option<String> {
        self.save_now();
        let root = self.session.root().to_path_buf();
        if self.file_downloads.is_some() {
            self.backstage = None;
            let mut bytes = Vec::new();
            let result = write_notebooks_zip_to_writer(&root, std::io::Cursor::new(&mut bytes));
            return match result {
                Ok(count) => {
                    if let Some(downloads) = self.file_downloads.as_mut() {
                        downloads.push(FileDownload { name: "Mon-Cartable-Sauvegarde.zip".to_owned(), mime: "application/zip", bytes });
                    }
                    let msg = format!("Sauvegarde téléchargée ({count} fichiers) : Mon-Cartable-Sauvegarde.zip");
                    self.say(msg.clone());
                    Some(msg)
                }
                Err(err) => {
                    self.status = format!("Impossible de créer la sauvegarde : {err}");
                    self.status_is_error = true;
                    Some(self.status.clone())
                }
            };
        }
        if let Some(dest) = rfd::FileDialog::new()
            .set_title("Sauvegarder tout le cartable (.zip)")
            .set_file_name("Mon-Cartable-Sauvegarde.zip")
            .add_filter("Archive ZIP", &["zip"])
            .save_file()
        {
            self.backstage = None;
            match write_notebooks_zip(&root, &dest) {
                Ok(count) => {
                    let msg = format!("Sauvegarde réussie ({count} fichiers) : {}", dest.display());
                    self.say(msg.clone());
                    return Some(msg);
                }
                Err(err) => {
                    self.status = format!("Impossible de créer la sauvegarde : {err}");
                    self.status_is_error = true;
                    return Some(self.status.clone());
                }
            }
        }
        None
    }

    fn handle_import_zip(&mut self) -> Option<String> {
        self.save_now();
        #[cfg(target_os = "android")]
        {
            self.android_pick_pending = Some(AndroidPickKind::ImportZip);
            self.android_pick_request = Some("application/zip".to_string());
            self.backstage = None;
            return None;
        }
        #[cfg(not(target_os = "android"))]
        {
            let root = self.session.root().to_path_buf();
            if let Some(src) = rfd::FileDialog::new().set_title("Récupérer une sauvegarde (.zip)").add_filter("Archive ZIP", &["zip"]).pick_file() {
                match extract_notebooks_zip(&src, &root) {
                    Ok(count) => {
                        self.session.refresh();
                        self.expand_all();
                        self.backstage = None;
                        let msg = format!("Récupération réussie ({count} fichiers restaurés) !");
                        self.say(msg.clone());
                        return Some(msg);
                    }
                    Err(err) => {
                        self.status = format!("Impossible de récupérer la sauvegarde : {err}");
                        self.status_is_error = true;
                        return Some(self.status.clone());
                    }
                }
            }
            None
        }
    }

    fn recent_notebooks(&self) -> Vec<Node> {
        let mut paths = Vec::new();
        if let Some(path) = self.editor.as_ref().and_then(|editor| notebook_for(&editor.dir)) {
            paths.push(path);
        }
        paths.extend(self.session.opened_elsewhere().iter().rev().cloned());
        if let Some(saved) = self.session.settings.get("recent_notebooks")
            && let Ok(saved) = serde_json::from_str::<Vec<String>>(&saved)
        {
            paths.extend(saved.into_iter().map(PathBuf::from));
        }
        let mut recent = Vec::new();
        let mut seen = HashSet::new();
        for path in paths {
            let path = std::fs::canonicalize(&path).unwrap_or(path);
            if !seen.insert(path.clone()) {
                continue;
            }
            if let Some(notebook) = ergonote_store::tree::load_tree(&path)
                && notebook.info.kind == NodeKind::Notebook
            {
                recent.push(notebook);
                if recent.len() == 12 {
                    break;
                }
            }
        }
        recent
    }

    fn remember_recent_notebook(&mut self, path: &Path) {
        let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let mut recent =
            self.session.settings.get("recent_notebooks").and_then(|saved| serde_json::from_str::<Vec<String>>(&saved).ok()).unwrap_or_default();
        let path_text = path.to_string_lossy().into_owned();
        recent.retain(|saved| std::fs::canonicalize(saved).unwrap_or_else(|_| PathBuf::from(saved)) != path);
        recent.insert(0, path_text);
        recent.truncate(12);
        if let Ok(serialized) = serde_json::to_string(&recent) {
            let _ = self.session.settings.set("recent_notebooks", &serialized);
        }
    }

    fn remove_recent_notebook(&mut self, path: &Path) {
        let Some(saved) = self.session.settings.get("recent_notebooks") else { return };
        let Ok(mut recent) = serde_json::from_str::<Vec<String>>(&saved) else { return };
        recent.retain(|entry| std::fs::canonicalize(entry).unwrap_or_else(|_| PathBuf::from(entry)) != path);
        if let Ok(serialized) = serde_json::to_string(&recent) {
            let _ = self.session.settings.set("recent_notebooks", &serialized);
        }
        self.say(crate::tl_id!("ui.removed_notebook_from_the_known_notebooks_list"));
    }

    fn open_notebook_from_path(&mut self, path: &Path) {
        self.save_now();
        let path_text = path_str(path);
        if let Some(Output::Path(opened)) = self.run("notebook.open", &params(&[("path", &path_text)])) {
            self.dismiss_search();
            self.remember_recent_notebook(&opened);
            self.open.insert(opened.clone());
            self.selected = Some(opened.clone());
            self.backstage = None;
            if let Some(page) = first_page(self.session.tree(), &opened) {
                self.open_page(&page);
            } else {
                self.editor = None;
                self.pad = None;
                self.say(crate::i18n::format_message_id("ui.opened_path_it_has_no_pages_yet", &[("path", &self.shown(&opened))]));
            }
        }
    }

    /// Asks the system for a folder and opens the notebooks in it, keeping everything that
    /// is already open. A folder that is itself a notebook opens as one; a folder that
    /// holds notebooks opens all of them. Nothing is closed and nothing is moved.
    fn choose_notebooks_folder(&mut self) {
        let Some(picked) =
            rfd::FileDialog::new().set_title(crate::tl_id!("ui.choose_a_notebook_a_folder_of_notebooks_or_an_empty_9de57f8f1f")).pick_folder()
        else {
            return;
        };
        self.open_notebooks_from_folder(picked);
    }

    /// Handles the folder returned by the native chooser, using the same notebook operations
    /// for a notebook itself, a parent containing notebooks, and an empty folder.
    fn open_notebooks_from_folder(&mut self, picked: PathBuf) {
        let is_notebook = ergonote_store::tree::read_node(&picked).is_some_and(|i| i.kind == NodeKind::Notebook);
        let wanted: Vec<std::path::PathBuf> =
            if is_notebook { vec![picked.clone()] } else { ergonote_store::tree::load_notebooks(&picked).into_iter().map(|n| n.path).collect() };
        if wanted.is_empty() {
            self.open_notebook_from_path(&picked);
            return;
        }
        let mut opened_count = 0;
        let mut first = None;
        for path in wanted {
            let p = path_str(&path);
            if let Some(Output::Path(notebook_path)) = self.run("notebook.open", &params(&[("path", &p)])) {
                self.remember_recent_notebook(&notebook_path);
                opened_count += 1;
                first.get_or_insert(path.clone());
                self.open.insert(notebook_path);
            }
        }
        if let Some(path) = first {
            self.open_notebook_from_path(&path);
        }
        self.backstage = None;
        if !self.status_is_error {
            self.say(crate::i18n::format_count_id("ui.n_notebook_opened_n_notebooks_opened", opened_count));
        }
    }

    fn import_notebook_folder(&mut self) {
        let mut dialog = rfd::FileDialog::new().set_title(crate::tl_id!("ui.choose_a_onenote_notebook_folder"));
        if let Some(backup) = ergonote_import::discover::default_source() {
            dialog = dialog.set_directory(backup);
        }
        let Some(source) = dialog.pick_folder() else { return };
        self.import_officeimo_source(source);
    }

    fn import_file(&mut self) {
        let mut dialog = rfd::FileDialog::new().set_title(crate::tl_id!("ui.choose_a_file_to_import"));
        if let Some(backup) = ergonote_import::discover::default_source() {
            dialog = dialog.set_directory(backup);
        }
        let Some(source) = dialog.pick_file() else { return };
        self.import_officeimo_source(source);
    }

    fn import_officeimo_source(&mut self, source: PathBuf) {
        let format = self
            .session
            .settings
            .get("page_format")
            .and_then(|v| ergonote_model::PageFormat::parse(&v).ok())
            .unwrap_or(ergonote_model::PageFormat::Html);
        let bridge_format = match format {
            ergonote_model::PageFormat::Html => ergonote_import::officeimo::Format::Html,
            ergonote_model::PageFormat::Cmgfm => ergonote_import::officeimo::Format::Cmgfm,
        };
        // All source types produce a new notebook under this parent. Progress reports the
        // actual chosen path, so the view never guesses names or collision suffixes.
        let destination = self.session.root().to_path_buf();
        log::info!("OfficeIMO import requested: source={} destination={}", source.display(), destination.display());
        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let source_for_worker = source.clone();
        let output_for_worker = destination.clone();
        std::thread::spawn(move || {
            let mut progress = |event| {
                let message = match event {
                    ergonote_import::officeimo::Progress::Found { total } => ImportMessage::Found(total),
                    ergonote_import::officeimo::Progress::Destination { path } => ImportMessage::Destination(path),
                    ergonote_import::officeimo::Progress::Section { index, total, label } => {
                        ImportMessage::Section { index, total, label: label.to_string() }
                    }
                };
                let _ = tx.send(message);
            };
            let result =
                ergonote_import::officeimo::run_with_progress(&source_for_worker, &output_for_worker, bridge_format, &worker_cancel, &mut progress);
            let _ = tx.send(ImportMessage::Finished(result));
        });
        self.import_job = Some(ImportJob {
            rx,
            keep_alive: None,
            cancel,
            source,
            destination,
            current: crate::tl_id!("ui.starting_officeimo").into(),
            index: 0,
            total: 0,
        });
        self.status = crate::tl_id!("ui.importing").into();
        self.status_is_error = false;
    }

    fn poll_import(&mut self, ctx: &Context) {
        let Some(job) = self.import_job.as_mut() else { return };
        let mut finished = None;
        loop {
            match job.rx.try_recv() {
                Ok(ImportMessage::Found(total)) => job.total = total,
                Ok(ImportMessage::Section { index, total, label }) => {
                    job.index = index;
                    job.total = total;
                    job.current = label;
                }
                Ok(ImportMessage::Destination(path)) => job.destination = path,
                Ok(ImportMessage::Finished(result)) => {
                    finished = Some(result);
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    finished = Some(Err(crate::tl_id!("ui.officeimo_import_worker_stopped_unexpectedly").into()));
                    break;
                }
            }
        }
        if let Some(result) = finished {
            let Some(job) = self.import_job.take() else { return };
            match result {
                Ok(message) => {
                    let _ = self.run("tree.refresh", &Params::new());
                    log::info!("OfficeIMO import succeeded: source={} destination={}", job.source.display(), job.destination.display());
                    self.status_is_error = false;
                    self.open_notebook_from_path(&job.destination);
                    if !self.status_is_error {
                        self.say(crate::tl!(&message).to_string());
                    }
                }
                Err(error) => {
                    log::error!("OfficeIMO import failed: source={} destination={} error={error}", job.source.display(), job.destination.display());
                    self.status = crate::i18n::format_message_id("ui.could_not_import_the_notebook_error", &[("error", &error)]);
                    self.status_is_error = true;
                }
            }
        } else if job.keep_alive.is_none() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }

    fn import_progress(&mut self, ctx: &Context) {
        let Some(job) = self.import_job.as_mut() else { return };
        let mut cancel = false;
        egui::Window::new(crate::tl_id!("ui.importing_onenote"))
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .frame(modal_frame(&self.tokens))
            .show(ctx, |ui| {
                ui.set_min_width(420.0);
                modal_title(ui, &self.tokens, crate::tl_id!("ui.importing_onenote"));
                ui.label(egui::RichText::new(crate::tl_id!("ui.you_can_continue_working_while_the_import_runs")).color(self.tokens.muted));
                ui.add_space(12.0);
                ui.label(egui::RichText::new(crate::i18n::pseudo_dynamic(&job.current)).strong());
                let fraction = if job.total == 0 { 0.0 } else { job.index as f32 / job.total as f32 };
                ui.add(egui::ProgressBar::new(fraction).fill(self.tokens.accent).show_percentage());
                if job.total > 0 {
                    let index = job.index.to_string();
                    let total = job.total.to_string();
                    ui.label(crate::i18n::format_message_id("ui.index_of_total_sections", &[("index", &index), ("total", &total)]));
                } else {
                    ui.label(crate::tl_id!("ui.discovering_sections"));
                }
                ui.add_space(14.0);
                ui.separator();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if modal_button(ui, &self.tokens, crate::tl_id!("ui.cancel_import"), true).clicked() {
                        cancel = true;
                    }
                });
            });
        if cancel {
            job.cancel.store(true, Ordering::Relaxed);
            job.current = "Cancelling…".into();
        }
    }

    fn reveal(&mut self, path: &Path) {
        match open_folder(path) {
            Ok(()) => self.say(crate::i18n::format_message_id("ui.opened_path", &[("path", &self.shown(path))])),
            Err(e) => {
                self.status = crate::i18n::format_message_id("ui.the_folder_could_not_be_opened_error", &[("error", &e.to_string())]);
                self.status_is_error = true;
            }
        }
    }

    /// Follows a link written in the open page. Another page of the notebooks opens in the
    /// window; a notebook, section group or section opens on its first page; a file opens in
    /// the program the system has for it; a web address opens in the browser. A relative
    /// path can never reach outside the notebooks folder (`ergonote_store::links`).
    /// Puts a link to a page on the clipboard, as OneNote's Copy Link to Page does.
    ///
    /// The link names the page from the notebooks folder down
    /// (`ergonote:/book/section/page`), not relative to anything, because what a relative
    /// link would be relative to is the page it is pasted into, and nobody knows that when
    /// the link is copied. Pasted into a page it reads as an ordinary link and opens the
    /// page it names.
    pub fn copy_link_to(&mut self, dir: &Path) {
        let mut roots = vec![self.session.root().to_path_buf()];
        roots.extend(self.session.opened_elsewhere().iter().cloned());
        let Some(uri) = ergonote_store::links::page_uri(&roots, dir) else {
            self.complain(crate::tl_id!("ui.that_page_is_not_in_a_notebook_that_is_open"));
            return;
        };
        let title = ergonote_store::tree::read_node(dir).map(|n| n.title).unwrap_or_else(|| "Page".to_string());
        // Plain text keeps the title and address readable; the matching HTML flavor is an
        // anchor for clients that accept rich content.
        self.pending_copy = Some(format!("{title} ({uri})"));
        self.pending_copy_html = Some(crate::clipboard::html_link(&title, &uri));
        self.say(crate::i18n::format_message_id("ui.link_to_title_copied", &[("title", &title)]));
    }

    pub fn follow_link(&mut self, ctx: &Context, href: &str) {
        use ergonote_store::links::{Link, resolve_in};
        let Some(page) = self.editor.as_ref().map(|e| e.dir.clone()) else { return };
        // Every folder a notebook is open from, not just the notebooks folder: a notebook
        // opened from elsewhere is as much inside as one that lives there, and judging it
        // by the notebooks folder alone made every link in it dead.
        let mut roots = vec![self.session.root().to_path_buf()];
        roots.extend(self.session.opened_elsewhere().iter().cloned());
        match resolve_in(&roots, &page, href) {
            Link::Web(url) => {
                ctx.open_url(egui::OpenUrl::new_tab(url.clone()));
                self.say(crate::i18n::format_message_id("ui.opened_url_in_the_browser", &[("url", &url)]));
            }
            Link::Node(dir) => {
                if ergonote_store::tree::read_node(&dir).is_some_and(|n| n.kind == NodeKind::Page) {
                    self.open_path(&path_str(&dir));
                } else {
                    self.open.insert(dir.clone());
                    self.selected = Some(dir.clone());
                    match first_page_under(self.session.tree(), &dir) {
                        Some(first) => self.open_path(&path_str(&first)),
                        None => self.say(crate::i18n::format_message_id("ui.opened_path_it_has_no_pages_yet", &[("path", &self.shown(&dir))])),
                    }
                }
            }
            Link::File(file) => {
                if self.open_attachment_warning_suppressed {
                    match open_file_for_viewer(&file) {
                        Ok(()) => self.say(crate::i18n::format_message_id("ui.opening_path", &[("path", &self.shown(&file))])),
                        Err(e) => {
                            self.complain(&crate::i18n::format_message_id("ui.the_file_could_not_be_opened_error", &[("error", &e.to_string())]))
                        }
                    }
                } else {
                    self.dialog = Some(Dialog::OpenAttachmentWarning { path: file });
                }
            }
            Link::Missing(p) => {
                self.status = crate::i18n::format_message_id(
                    "ui.nothing_at_path_the_link_points_to_a_page_or_file_th_18d71ccd21",
                    &[("path", &self.shown(&p))],
                );
                self.status_is_error = true;
            }
            Link::Outside => {
                self.status = crate::tl_id!("ui.that_link_points_outside_the_notebooks_folder_so_it_3a37bd9b91").to_string();
                self.status_is_error = true;
            }
        }
    }

    /// The ribbon under the title row: tabs and their groups.
    fn ribbon_bar(&mut self, ui: &mut Ui) {
        if self.is_simple_mobile() {
            self.mobile_simple_ribbon(ui);
            return;
        }
        let (tab, source, tokens) = (self.ribbon_tab.clone(), self.show_source, self.tokens);
        let font_selection = self.font_selection(ui.ctx());
        let family = match &font_selection.family {
            pageedit::FontValue::Value(Some(name)) => Some(name.clone()),
            pageedit::FontValue::Value(None) => Some(crate::tl_id!("ui.default").to_string()),
            pageedit::FontValue::Mixed => Some(crate::tl_id!("ui.mixed").to_string()),
            // With no caret in the page the box shows the default font, as the size box shows its base size.
            pageedit::FontValue::Unavailable => Some(crate::tl_id!("ui.default").to_string()),
        };
        let size = match font_selection.size {
            pageedit::FontValue::Value(Some(size)) => Some(size.to_string()),
            // The page's base size is 11 points, so the box shows that number, not the word Default.
            pageedit::FontValue::Value(None) => Some("11".to_owned()),
            pageedit::FontValue::Mixed => Some(crate::tl_id!("ui.mixed").to_string()),
            // With no caret in the page the box shows the base size, as OneNote does.
            pageedit::FontValue::Unavailable => Some("11".to_owned()),
        };
        let view = ribbon::View {
            split_panes: self.split_panes,
            section_tabs_bar: self.section_tabs_bar,
            simple_mode: self.simple_mode,
            lock_images: self.lock_images,
            dys_spaced: self.dys_spaced,
            reading_ruler: self.reading_ruler,
            word_prediction: self.word_prediction_enabled,
            dictating: self.dictation_state == DictationState::Listening,
            always_on_top: self.always_on_top,
            full_page: self.full_page,
            rules: self.rules,
            hide_title: self.hide_title,
            snap: self.snap_to_grid,
            zoom_percent: self.zoom_percent,
            pointer: self.pointer,
            rack: &self.rack,
            nib: self.nib,
            in_table: self.caret_in_table(),
            has_image: self.page_has_image(),
            ocr_busy: self.ocr_job.is_some(),
            space_tool: self.space_tool,
            textbox_tool: self.textbox_tool,
            table_borders: self.caret_table_has_borders(),
            table_align: self.caret_cells_align(),
            table_shade: self.caret_cells_shade(),
            gridcraft: self.gridcraft.is_some(),
            page_colour: self.editor.as_ref().and_then(|e| e.colour),
            rule_colour: self.editor.as_ref().and_then(|e| e.rule_colour),
            paper_size: self.editor.as_ref().and_then(|e| e.paper_size),
            can_undo: self.editor.as_ref().is_some_and(|e| e.history.can_undo()) || self.pad.as_ref().is_some_and(crate::inkpad::Pad::can_undo),
            can_redo: self.editor.as_ref().is_some_and(|e| e.history.can_redo()) || self.pad.as_ref().is_some_and(crate::inkpad::Pad::can_redo),
            can_ink_redo: self.pad.as_ref().is_some_and(crate::inkpad::Pad::can_redo),
            sensitivity: self.sensitivity,
            pressure_available: self.stylus.sample().pen,
            has_page: self.editor.is_some(),
            page_marked_unread: self.editor.as_ref().is_some_and(|e| self.unread.contains(&e.dir)),
            painting: self.painter.is_some(),
            style: self.caret_style(ui.ctx()),
            font_family: family.as_deref(),
            font_size: size.as_deref(),
            marks: font_selection.marks,
            align: self.caret_align(),
        };
        let format = self.editor.as_ref().map(|e| e.format);
        let mut acts = Vec::new();
        if !self.ribbon_folded() {
            egui::Panel::top("ribbon")
                .exact_size(ribbon::GROUP_H + 8.0)
                .frame(egui::Frame::new().fill(tokens.panel).inner_margin(egui::Margin::symmetric(4, 0)))
                .show_separator_line(false)
                .show(ui, |ui| {
                    acts = ribbon::show(ui, &tab, source, &view, format, &tokens);
                });
        } else if self.ribbon_peek {
            // Folded, with a tab clicked: the tab's commands over the top of the page, as Office
            // shows them, so the page does not jump down and back. Using one, clicking anywhere
            // else or Escape folds it away again.
            let screen = ui.ctx().content_rect();
            let area = egui::Area::new(egui::Id::new("ribbon-peek"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(screen.min.x, self.band_bottom))
                .show(ui.ctx(), |ui| {
                    egui::Frame::new()
                        .fill(tokens.panel)
                        .inner_margin(egui::Margin::symmetric(4, 0))
                        .stroke(egui::Stroke::new(1.0, tokens.container_border))
                        .show(ui, |ui| {
                            ui.set_width(screen.width() - 8.0);
                            ui.set_height(ribbon::GROUP_H + 8.0);
                            acts = ribbon::show(ui, &tab, source, &view, format, &tokens);
                        });
                })
                .response;
            let pointer = ui.ctx().pointer_latest_pos();
            // A gallery the shown ribbon opened (Format Background, say) is over the page too, and
            // a click in it is still a click in the ribbon.
            let in_ribbon = |p: egui::Pos2| area.rect.contains(p) || ui.ctx().layer_id_at(p).is_some_and(|l| l.order == egui::Order::Foreground);
            let clicked_away = ui.input(|i| i.pointer.any_pressed()) && pointer.is_some_and(|p| !in_ribbon(p) && p.y > self.band_bottom);
            let used = acts.iter().any(|a| !matches!(a, ribbon::Act::Tab(_)));
            if used || clicked_away || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.ribbon_peek = false;
            }
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

    fn font_selection(&self, ctx: &Context) -> pageedit::FontSelection {
        use ergonote_formats::Block;
        let Some(ed) = self.editor.as_ref() else { return pageedit::FontSelection::default() };
        if let Some(index) = crate::containers::selected(ctx, &ed.dir) {
            let range = ergonote_model::container::blocks_of(&ed.boxes, index);
            let Some(blocks) = ed.blocks.get(range) else { return pageedit::FontSelection::default() };
            let spans = blocks.iter().flat_map(|block| match block {
                Block::Heading { spans, .. } | Block::Paragraph(spans) | Block::Quote(spans) | Block::Item { spans, .. } => spans.as_slice(),
                _ => &[],
            });
            return pageedit::FontSelection::from_spans(spans);
        }
        if let Some(across) = ed.across.filter(|a| !a.inside_one_block()) {
            let mut result: Option<pageedit::FontSelection> = None;
            for index in across.blocks() {
                let Some(block) = ed.blocks.get(index).filter(|b| pageedit::is_one_line_of_words(b)) else {
                    return pageedit::FontSelection::default();
                };
                let text = pageedit::block_text(block);
                let Some((start, end)) = across.covering(index, text.len()) else { continue };
                let (Some(left), Some(right)) = (text.get(..start), text.get(..end)) else { return pageedit::FontSelection::default() };
                let Some(selection) = pageedit::selection_fonts(&text, (left.chars().count(), right.chars().count())) else {
                    return pageedit::FontSelection::default();
                };
                if let Some(current) = result.as_mut() {
                    current.merge(selection);
                } else {
                    result = Some(selection);
                }
            }
            return result.unwrap_or_default();
        }
        let Some(caret) = ed.caret.as_ref() else { return pageedit::FontSelection::default() };
        let mut result = ed.blocks.get(caret.block).and_then(|block| caret.fonts(block)).unwrap_or_default();
        if caret.selection.0 == caret.selection.1
            && let Some(pending) = ed.pending_format.as_ref().filter(|p| p.block == caret.block && p.at == caret.selection.0)
        {
            if let Some(family) = &pending.family {
                result.family = pageedit::FontValue::Value(Some(family.clone()));
            }
            if let Some(size) = pending.size {
                result.size = pageedit::FontValue::Value(Some(size));
            }
            // A mark clicked with nothing selected is put on what is typed next: the button
            // shows it pressed until then (issue #255).
            let m = &mut result.marks;
            m.bold |= pending.bold;
            m.italic |= pending.italic;
            m.strike |= pending.strike;
            m.underline |= pending.underline;
            m.subscript |= pending.script == Some(ergonote_formats::markdown::Script::Subscript);
            m.superscript |= pending.script == Some(ergonote_formats::markdown::Script::Superscript);
            m.highlight |= pending.highlight_color.is_some();
        }
        result
    }

    /// What a ribbon button does while a container is selected: selecting a box selects everything in
    /// it, so Cut, Copy, the marks and the styles take all of it. True when the act was one of those.
    fn act_on_box(&mut self, ctx: &Context, at: usize, act: &ribbon::Act) -> bool {
        use ergonote_formats::markdown::Block;
        let wanted = matches!(
            act,
            ribbon::Act::Clipboard(_)
                | ribbon::Act::Mark(_)
                | ribbon::Act::UnderlineColor(_)
                | ribbon::Act::TextColor(_)
                | ribbon::Act::HighlightColor(_)
                | ribbon::Act::FontSize(_)
                | ribbon::Act::FontFamily(_)
                | ribbon::Act::Block(_)
                | ribbon::Act::ClearFormatting
                | ribbon::Act::Direction(_)
                | ribbon::Act::Indent(_)
                | ribbon::Act::Tag(_)
                | ribbon::Act::Justify(_)
                | ribbon::Act::Align(_)
        );
        let Some(ed) = self.editor.as_mut().filter(|_| wanted) else { return false };
        let range = ergonote_model::container::blocks_of(&ed.boxes, at);
        if range.end > ed.blocks.len() || range.is_empty() {
            return false;
        }
        let words = |b: &Block| !matches!(b, Block::Code { .. } | Block::MathBlock(_)) && pageedit::is_one_line_of_words(b);
        match act {
            ribbon::Act::Clipboard(which) => {
                crate::containers::ask_clipboard(ctx, *which == "cut");
                return true;
            }
            ribbon::Act::FontSize(_)
            | ribbon::Act::FontFamily(_)
            | ribbon::Act::UnderlineColor(_)
            | ribbon::Act::TextColor(_)
            | ribbon::Act::HighlightColor(_) => {
                let format = match act {
                    ribbon::Act::FontSize(size) if ribbon::FONT_SIZES.contains(size) => pageedit::InlineFormat::Size(*size),
                    ribbon::Act::FontFamily(family) if ergonote_formats::markdown::valid_font_family(family) => {
                        pageedit::InlineFormat::Family(family.clone())
                    }
                    ribbon::Act::UnderlineColor(color) => pageedit::InlineFormat::UnderlineColor(color.map(|c| c.id().to_string())),
                    ribbon::Act::TextColor(color) => pageedit::InlineFormat::TextColor(color.map(|c| c.id().to_string())),
                    ribbon::Act::HighlightColor(color) => pageedit::InlineFormat::HighlightColor(color.map(|c| c.id().to_string())),
                    _ => {
                        log::warn!("ribbon requested an invalid font size or family");
                        self.status = crate::tl_id!("ui.inline_mark_whole_line").into();
                        self.status_is_error = false;
                        return true;
                    }
                };
                if !ed.writable {
                    self.status = crate::tl_id!("ui.inline_mark_whole_line").into();
                    self.status_is_error = false;
                    return true;
                }
                Self::remember_step(ed);
                let mut changed = false;
                for block in ed.blocks.get_mut(range).unwrap_or_default() {
                    if let Block::Heading { spans, .. } | Block::Paragraph(spans) | Block::Quote(spans) | Block::Item { spans, .. } = block {
                        changed |= pageedit::format_span_range(spans, 0..usize::MAX, format.clone());
                    }
                }
                if changed {
                    ed.pending_format = None;
                    ed.rebuild_body();
                    Self::mark_dirty_from_blocks(ed);
                    let step =
                        crate::history::Step::new(ed.body.clone(), ed.blocks.clone(), ed.boxes.clone(), ed.caret.clone(), crate::history::Kind::Edit);
                    ed.history.record(step);
                }
                self.status.clear();
                self.status_is_error = false;
                return true;
            }
            ribbon::Act::Mark(mark) => {
                let new_mark = matches!(*mark, "underline" | "subscript" | "superscript");
                if new_mark && !ed.writable {
                    self.status = crate::tl_id!("ui.read_only").into();
                    self.status_is_error = false;
                    return true;
                }
                if new_mark {
                    Self::remember_step(ed);
                }
                if !pageedit::toggle_mark(&mut ed.blocks, range, mark) {
                    return true;
                }
                if new_mark {
                    ed.pending_format = None;
                    ed.rebuild_body();
                    Self::mark_dirty_from_blocks(ed);
                    let step =
                        crate::history::Step::new(ed.body.clone(), ed.blocks.clone(), ed.boxes.clone(), ed.caret.clone(), crate::history::Kind::Edit);
                    ed.history.record(step);
                    return true;
                }
            }
            ribbon::Act::Direction(dir) => {
                // Words only: a mark in a diagram or an equation would be part of its source.
                for i in range {
                    if ed.blocks.get(i).is_some_and(words) {
                        pageedit::set_direction(&mut ed.blocks, i, *dir);
                    }
                }
            }
            ribbon::Act::Indent(deeper) => {
                for i in range {
                    pageedit::indent(&mut ed.blocks, i, *deeper);
                }
            }
            ribbon::Act::Tag(tag) => {
                // All of them have the tag already: the same button takes it off every one.
                let has = |b: &Block| pageedit::tag_of(b).0 == Some(*tag);
                // A to-do line carries no other tag, so it is left out of every tag but its own.
                let can = |b: &Block| words(b) && (*tag == ergonote_model::Tag::ToDo || !matches!(b, Block::Item { task: Some(_), .. }));
                let all = range.clone().filter_map(|i| ed.blocks.get(i)).filter(|b| can(b)).all(has);
                for i in range {
                    if ed.blocks.get(i).is_some_and(|b| can(b) && (all || !has(b))) {
                        pageedit::set_tag(&mut ed.blocks, i, *tag);
                    }
                }
            }
            ribbon::Act::Align(align) => {
                let blocks: Vec<usize> = range.collect();
                self.align_blocks(&blocks, *align);
                return true;
            }
            ribbon::Act::Justify(strength) => {
                let paragraphs: Vec<usize> = range.filter(|i| matches!(ed.blocks.get(*i), Some(Block::Paragraph(_)))).collect();
                if paragraphs.is_empty() {
                    return true;
                }
                let all = paragraphs.iter().all(|i| ed.justified.strength_of(*i) == Some(*strength));
                let want = (!all).then_some(*strength);
                let mut marked = ed.justified.clone();
                for i in paragraphs {
                    marked.set(i, want);
                }
                self.write_justified(marked, want);
                return true;
            }
            ribbon::Act::Block(kind) => {
                // All of them are that kind already: the same button turns them back into text.
                let all = range.clone().all(|i| ed.blocks.get(i).is_some_and(|b| pageedit::block_kind(b) == *kind));
                for i in range {
                    if all || ed.blocks.get(i).is_some_and(|b| pageedit::block_kind(b) != *kind) {
                        pageedit::set_kind(&mut ed.blocks, i, kind);
                    }
                }
            }
            _ => {
                for i in range {
                    if let Some(b) = ed.blocks.get_mut(i).filter(|b| words(b)) {
                        pageedit::clear_marks(b);
                    }
                }
            }
        }
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
        true
    }

    /// Carries out one thing the ribbon asked for.
    fn ribbon_act(&mut self, act: ribbon::Act) {
        match act {
            ribbon::Act::File => self.backstage = Some("Info"),
            ribbon::Act::NextUnread => self.next_unread(),
            ribbon::Act::ToggleRead => self.toggle_read(),
            ribbon::Act::RecentEdits => self.show_recent_edits(RecentRange::Week, false),
            ribbon::Act::PageVersions => {
                self.save_now();
                if let Some(page) = self.editor.as_ref().map(|e| e.dir.clone()) {
                    let p = params(&[("path", &path_str(&page))]);
                    if let Some(Output::Versions(versions)) = self.run("page.versions", &p) {
                        self.dialog = Some(Dialog::PageVersions { page, versions, preview: None });
                    }
                }
            }
            ribbon::Act::RecycleBin => {
                if let Some(notebook) = self.editor.as_ref().and_then(|e| notebook_for(&e.dir)) {
                    let p = params(&[("notebook", &path_str(&notebook))]);
                    if let Some(Output::Recycled(items)) = self.run("recycle.list", &p) {
                        self.dialog = Some(Dialog::RecycleBin { notebook, items });
                    }
                }
            }
            ribbon::Act::Symbols => self.dialog = Some(Dialog::Symbols),
            ribbon::Act::Equation(tex) => self.insert_in_page(&format!("${tex}$")),
            ribbon::Act::Tab(name) => {
                // Folded, a tab shows its commands over the page; the same tab again hides them.
                if self.ribbon_folded() {
                    self.ribbon_peek = !(self.ribbon_peek && self.ribbon_tab == name);
                }
                self.ribbon_tab = name;
            }
            ribbon::Act::ToggleRibbon => self.toggle_ribbon(),
            ribbon::Act::Mark(mark) => self.apply_mark(mark),
            ribbon::Act::FontSize(size) => self.apply_font_size(size),
            ribbon::Act::FontFamily(family) => self.apply_font_family(&family),
            ribbon::Act::Block(kind) => self.apply_block_kind(kind),
            ribbon::Act::Insert(what) => match what {
                "space" => {
                    let Some(ed) = self.editor.as_mut() else { return };
                    // What was written before is kept as it stood, so Undo takes back only the space.
                    Self::remember_step(ed);
                    let at = ed.caret.as_ref().map(|c| c.block + 1).unwrap_or(ed.blocks.len()).min(ed.blocks.len());
                    ed.blocks.insert(at, ergonote_formats::Block::Paragraph(Vec::new()));
                    let marks = ed.blocks_inserted(at, 1);
                    ed.rebuild_body();
                    ed.caret = Some(pageedit::Caret {
                        block: at,
                        text: String::new(),
                        focus: true,
                        selection: (0, 0),
                        place: None,
                        cell: None,
                        span: None,
                        inline: None,
                    });
                    Self::mark_dirty_from_blocks(ed);
                    Self::remember_edit(ed);
                    self.save_block_marks(marks);
                }
                "link" => self.open_link_dialog(),
                "equation" => self.insert_in_page("$E = mc^2$"),
                "date" | "time" | "datetime" => {
                    let now = ergonote_model::time::iso_now();
                    let stamp = match what {
                        "time" => ergonote_model::time::clock_from_iso(&now),
                        "datetime" => ergonote_model::time::long_from_iso(&now),
                        _ => ergonote_model::time::date_from_iso(&now),
                    };
                    self.insert_in_page(&stamp.unwrap_or(now));
                }
                other => self.insert_block(other),
            },
            ribbon::Act::AddPrintout => {
                let Some(dir) = self.editor.as_ref().map(|e| e.dir.clone()) else {
                    self.say(crate::tl_id!("ui.open_a_page_first"));
                    return;
                };
                #[cfg(target_os = "android")]
                {
                    self.android_pick_pending = Some(AndroidPickKind::Printout(dir));
                    self.android_pick_request = Some("application/pdf".to_string());
                }
                #[cfg(not(target_os = "android"))]
                {
                    let picker = rfd::FileDialog::new().set_title(crate::tl_id!("ui.choose_a_pdf_or_office_document_to_print_out")).add_filter(
                        crate::tl_id!("ui.pdf_and_office_documents"),
                        &["pdf", "doc", "docx", "ppt", "pptx", "xls", "xlsx", "odt", "odp", "ods", "rtf"],
                    );
                    if let Some(file) = picker.pick_file() {
                        self.add_file(&dir, &file, FileChoice::Printout);
                    }
                }
            }
            ribbon::Act::AddFile | ribbon::Act::AddPicture => {
                let Some(dir) = self.editor.as_ref().map(|e| e.dir.clone()) else {
                    self.say(crate::tl_id!("ui.open_a_page_first"));
                    return;
                };
                let is_picture = matches!(act, ribbon::Act::AddPicture);
                #[cfg(target_os = "android")]
                {
                    self.android_pick_pending = Some(if is_picture { AndroidPickKind::Picture(dir) } else { AndroidPickKind::File(dir) });
                    self.android_pick_request = Some((if is_picture { "image/*" } else { "*/*" }).to_string());
                }
                #[cfg(not(target_os = "android"))]
                {
                    let mut picker = rfd::FileDialog::new().set_title(if is_picture {
                        crate::tl_id!("ui.choose_a_picture")
                    } else {
                        crate::tl_id!("ui.choose_a_file_to_attach")
                    });
                    if is_picture {
                        picker = picker.add_filter(crate::tl_id!("ui.pictures"), &["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "jxl"]);
                    }
                    if let Some(file) = picker.pick_file() {
                        self.dialog = Some(Dialog::FileOptions { dir, files: vec![file] });
                    }
                }
            }
            ribbon::Act::ToggleSource => self.set_show_source(!self.show_source),
            ribbon::Act::ToggleFullPage => self.full_page = !self.full_page,
            ribbon::Act::OpenThemes => self.theme_window = !self.theme_window,
            ribbon::Act::RuleLines(r) => {
                // The paper is the page's own, as OneNote's rule lines are. The window's
                // own setting stays as what a page with none of its own is drawn on.
                self.rules = r;
                let _ = self.session.settings.set("rule_lines", r.id());
                if self.editor.is_some() {
                    self.set_page_appearance(Some(r), None);
                }
                self.say(match r {
                    ribbon::RuleLines::None => crate::tl_id!("ui.no_lines_behind_the_page").to_string(),
                    other => crate::i18n::format_message_id("ui.paper_behind_the_page", &[("paper", crate::tl!(other.label()))]),
                });
            }
            ribbon::Act::ToggleAlwaysOnTop => {
                self.always_on_top = !self.always_on_top;
                // Asked for on the next frame, where the context is at hand.
                self.window_level_pending = true;
                let _ = self.session.settings.set("always_on_top", if self.always_on_top { "true" } else { "false" });
                self.say(if self.always_on_top {
                    crate::tl_id!("ui.this_window_stays_on_top")
                } else {
                    crate::tl_id!("ui.this_window_behaves_like_the_others")
                });
            }
            ribbon::Act::TogglePanes => {
                self.split_panes = !self.split_panes;
                self.organisers = true;
                let _ = self.session.settings.set("split_panes", if self.split_panes { "true" } else { "false" });
                self.say(if self.split_panes {
                    crate::tl_id!("ui.notebooks_and_pages_in_their_own_columns")
                } else {
                    crate::tl_id!("ui.one_tree_of_notebooks")
                });
            }
            ribbon::Act::ToggleSectionTabs => {
                self.section_tabs_bar = !self.section_tabs_bar;
                let _ = self.session.settings.set("section_tabs_bar", if self.section_tabs_bar { "true" } else { "false" });
                if self.section_tabs_bar {
                    // Hide the left notebooks/sections column so horizontal section tabs free up width.
                    self.split_panes = true;
                    self.organisers = false;
                    let _ = self.session.settings.set("split_panes", "true");
                } else {
                    self.organisers = true;
                }
                self.say(if self.section_tabs_bar {
                    "Barre d'onglets des matières affichée et panneau bloc-notes masqué."
                } else {
                    "Barre d'onglets des matières masquée."
                });
            }
            ribbon::Act::ToggleSimpleMode => {
                self.simple_mode = !self.simple_mode;
                let _ = self.session.settings.set("simple_mode", if self.simple_mode { "true" } else { "false" });
                if self.simple_mode {
                    self.full_page = false;
                    self.compact_full_page = false;
                }
                self.say(if self.simple_mode { "Mode simple activé (interface en tuiles)." } else { "Mode normal réactivé." });
            }
            ribbon::Act::ToggleLockImages => {
                self.lock_images = !self.lock_images;
                let _ = self.session.settings.set("lock_images", if self.lock_images { "true" } else { "false" });
                self.say(if self.lock_images { "PDF et images verrouillés en arrière-plan." } else { "PDF et images déverrouillés." });
            }
            ribbon::Act::ToggleDysSpacing => {
                self.dys_spaced = !self.dys_spaced;
                let _ = self.session.settings.set("dys_spaced", if self.dys_spaced { "true" } else { "false" });
                self.say(if self.dys_spaced {
                    "Texte aéré TND activé (espacement élargi des lettres et lignes)."
                } else {
                    "Texte aéré TND désactivé."
                });
            }
            ribbon::Act::ToggleReadingRuler => {
                self.reading_ruler = !self.reading_ruler;
                let _ = self.session.settings.set("reading_ruler", if self.reading_ruler { "true" } else { "false" });
                self.say(if self.reading_ruler { "Règle de lecture TND activée." } else { "Règle de lecture TND désactivée." });
            }
            ribbon::Act::ToggleWordPrediction => {
                self.word_prediction_enabled = !self.word_prediction_enabled;
                self.prediction_selected = None;
                self.prediction_cache = None;
                let _ = self.session.settings.set("word_prediction", if self.word_prediction_enabled { "true" } else { "false" });
                self.say(if self.word_prediction_enabled {
                    "Prédiction de mots et correction orthographique activées."
                } else {
                    "Prédiction de mots désactivée (mode dictée)."
                });
            }
            ribbon::Act::CheckOrthography => self.run_orthography_check(),
            ribbon::Act::SupportCreator => self.dialog = Some(Dialog::SupportCreator),
            ribbon::Act::ReportBug => self.dialog = Some(Dialog::ReportBug),
            ribbon::Act::ReadAloud => self.read_page_aloud(),
            ribbon::Act::VoiceDictation => self.toggle_voice_dictation(),
            ribbon::Act::SelectAll => self.select_all_smart(None),
            ribbon::Act::InsertSchoolTable(kind) => self.insert_school_table(kind),
            ribbon::Act::ToggleTitle => {
                self.hide_title = !self.hide_title;
                let _ = self.session.settings.set("hide_title", if self.hide_title { "true" } else { "false" });
            }
            ribbon::Act::ToggleSnap => {
                self.snap_to_grid = !self.snap_to_grid;
                let _ = self.session.settings.set("snap_to_grid", if self.snap_to_grid { "true" } else { "false" });
            }
            ribbon::Act::Zoom(step) => {
                self.zoom_percent = match step {
                    1 => (self.zoom_percent + 10).min(250),
                    -1 => self.zoom_percent.saturating_sub(10).max(50),
                    _ => 100,
                };
                self.fit_page_width = false;
            }
            ribbon::Act::ZoomTo(percent) => {
                self.zoom_percent = percent.clamp(50, 250);
                self.fit_page_width = false;
            }
            ribbon::Act::PageWidth => self.fit_page_width = true,
            ribbon::Act::FormatPainter(sticky) => self.pick_up_formatting(sticky),
            ribbon::Act::PageTemplates => {
                let name = self.templates_pane.as_ref().map(|p| p.name.clone()).unwrap_or_default();
                self.templates_pane = Some(TemplatesPane { templates: Vec::new(), name });
                self.refresh_templates();
            }
            ribbon::Act::InsertSpaceTool => {
                if self.editor.is_none() {
                    self.say(crate::tl_id!("ui.open_a_page_first"));
                    return;
                }
                self.space_tool = !self.space_tool;
                if self.space_tool {
                    self.pointer = ribbon::Pointer::Write;
                    self.say(crate::tl_id!("ui.insert_space_how"));
                }
            }
            ribbon::Act::FindTags => {
                // OneNote looks in the section first; with no page open there is no section.
                let scope = self.tags_pane.as_ref().map_or(if self.editor.is_some() { TagScope::Section } else { TagScope::All }, |p| p.scope);
                let unchecked = self.tags_pane.as_ref().is_some_and(|p| p.unchecked);
                self.tags_pane = Some(TagsPane { scope, unchecked, found: Vec::new() });
                self.refresh_tags();
            }
            ribbon::Act::CustomizeTags => {
                self.custom_tags = Some(crate::customtags::Dialog::new(ergonote_model::tag::custom()));
            }
            ribbon::Act::CollectAll => {
                if self.editor.as_ref().is_none_or(|ed| !ed.writable) {
                    self.say(crate::tl_id!("ui.open_a_page_first"));
                    return;
                }
                self.collect_pending = true;
                // The gathered page starts at its top left, so it is in view.
                self.canvas_pan = egui::Vec2::ZERO;
            }
            ribbon::Act::NotBuilt(what) => {
                self.status =
                    crate::i18n::format_message_id("ui.name_is_not_built_yet_see_the_roadmap_for_what_is_coming", &[("name", crate::tl!(what))]);
                self.status_is_error = false;
            }
            ribbon::Act::Tag(tag) => self.apply_tag(tag),
            ribbon::Act::Clipboard(which) => self.clipboard(which),
            ribbon::Act::Paste => self.paste_here(),
            ribbon::Act::Indent(deeper) => self.indent_block(deeper),
            ribbon::Act::ClearFormatting => self.clear_formatting(),
            ribbon::Act::Direction(d) => self.apply_direction(d),
            ribbon::Act::Justify(strength) => self.apply_justify(strength),
            ribbon::Act::Align(align) => self.apply_align(align),
            ribbon::Act::PageDirection(d) => self.apply_page_direction(d),
            ribbon::Act::Embedding(d) => self.apply_embedding(d),
            ribbon::Act::InkPointer(p) => self.pointer = p,
            ribbon::Act::InkPen(i) => {
                self.nib = i.min(self.rack.len().saturating_sub(1));
                self.pointer = ribbon::Pointer::Draw;
                self.remember_rack();
            }
            ribbon::Act::InkRack(edit) => self.edit_rack(edit),
            ribbon::Act::Table(what) => self.edit_table(what),
            ribbon::Act::DeleteTable => self.delete_table(),
            ribbon::Act::ConfirmDeleteTable(block) => self.request_delete_table(block),
            ribbon::Act::TableSort(descending) => self.sort_table(descending),
            ribbon::Act::TableSelect(what) => self.select_in_table(what),
            ribbon::Act::TableBorders(on) => self.set_table_borders(on),
            ribbon::Act::TableAlign(align) => self.align_table_cells(align),
            ribbon::Act::TableShade(shade) => self.shade_table_cells(shade),
            ribbon::Act::RuleColour(c) => {
                let Some(ed) = self.editor.as_mut() else {
                    self.say(crate::tl_id!("ui.open_a_page_first"));
                    return;
                };
                ed.rule_colour = c;
                let path = path_str(&ed.dir);
                self.run("page.paper", &params(&[("path", &path), ("rules", c.map(ergonote_model::NodeColor::id).unwrap_or("none"))]));
                self.say(match c {
                    Some(c) => crate::i18n::format_message_id("ui.the_lines_are_colour", &[("colour", &crate::tl!(c.label()).to_lowercase())]),
                    None => crate::tl_id!("ui.the_lines_are_the_theme_s_own_colour").to_string(),
                });
            }
            ribbon::Act::PageColour(c) => {
                self.set_page_appearance(None, Some(c));
                self.say(match c {
                    Some(c) => {
                        let colour = crate::tl!(c.label());
                        crate::i18n::format_message_id("ui.the_page_is_colour", &[("colour", &colour.to_lowercase())])
                    }
                    None => crate::tl_id!("ui.the_page_is_the_theme_s_own_colour").to_string(),
                });
            }
            ribbon::Act::PaperSize(paper) => self.set_paper_size(paper.as_deref().and_then(ergonote_model::paper::Paper::parse)),
            ribbon::Act::Undo => self.undo(),
            ribbon::Act::Redo => self.step_back(false),
            ribbon::Act::ToGridCraft => self.hand_table_to_gridcraft(),
            ribbon::Act::FromGridCraft => self.read_table_from_gridcraft(),
            ribbon::Act::NewGridCraft => self.new_gridcraft(),
            ribbon::Act::InsertTable(rows, columns) => self.insert_table(rows, columns),
            ribbon::Act::InsertTableDialog => {
                if self.editor.is_none() {
                    self.say(crate::tl_id!("ui.open_a_page_first"));
                } else {
                    self.dialog = Some(Dialog::InsertTable { rows: "3".into(), columns: "3".into(), first: true });
                }
            }
            ribbon::Act::ParagraphSpacing => self.open_paragraph_spacing(),
            ribbon::Act::InkSensitivity(v) => {
                self.sensitivity = v.min(100);
                let _ = self.session.settings.set(SENSITIVITY_KEY, &self.sensitivity.to_string());
            }
            ribbon::Act::InkUndo => self.ink_undo(),
            ribbon::Act::InkRedo => self.ink_redo(),
            ribbon::Act::InkInsert => self.ink_insert(),
            ribbon::Act::InkExportPlain => self.ink_export_plain(),
            ribbon::Act::BoardPhoto => self.insert_board_photo(),
            ribbon::Act::RecenterStudent => {
                if self.editor.as_ref().is_none_or(|ed| !ed.writable) {
                    self.say(crate::tl_id!("ui.open_a_page_first"));
                    return;
                }
                self.collect_pending = true;
                self.canvas_pan = egui::Vec2::ZERO;
                self.zoom_percent = 100;
                self.fit_page_width = false;
            }
            ribbon::Act::ToggleDysMode => {
                self.set_paper_size(Some(ergonote_model::paper::Paper::new(ergonote_model::paper::Size::A4)));
                let current = self.editor.as_ref().and_then(|e| e.colour);
                let next = match current {
                    None => Some(NodeColor::Yellow),
                    Some(NodeColor::Yellow) => Some(NodeColor::Blue),
                    Some(NodeColor::Blue) => Some(NodeColor::Green),
                    _ => None,
                };
                self.dys_spaced = next.is_some();
                let _ = self.session.settings.set("dys_spaced", if self.dys_spaced { "true" } else { "false" });
                self.ribbon_act(ribbon::Act::PageColour(next));
            }
            ribbon::Act::InsertTextBox => self.toggle_or_insert_textbox(),
            ribbon::Act::ScreenClip => self.insert_screen_clip(),
            ribbon::Act::UnderlineColor(color) => self.apply_underline_color(color),
            ribbon::Act::TextColor(color) => self.apply_text_color(color),
            ribbon::Act::HighlightColor(color) => self.apply_highlight_color(color),
            ribbon::Act::SavePictureAs(src) => self.save_picture_as(&src),
            ribbon::Act::TurnPicture(src, how) => self.turn_picture(&src, how),
            ribbon::Act::CropPicture => self.open_crop_picture(),
            ribbon::Act::TurnSelectedPicture(how) => {
                if let Some((_, src)) = self.active_image_target() {
                    self.turn_picture(&src, how);
                } else {
                    self.say("Sélectionnez d'abord une image sur la page.");
                }
            }
            ribbon::Act::EnhancePicture(mode) => self.enhance_picture(mode),
            ribbon::Act::OcrPicture { speak } => self.start_ocr(speak),
            ribbon::Act::DeleteSelectedPicture => self.delete_selected_picture(),
        }
    }

    fn toggle_or_insert_textbox(&mut self) {
        if self.editor.as_ref().is_none_or(|ed| !ed.writable) {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return;
        }
        if !self.textbox_tool {
            self.textbox_tool = true;
            self.space_tool = false;
            self.pointer = ribbon::Pointer::Write;
            self.say(crate::tl_id!("ui.student_textbox_prompt"));
        } else {
            self.textbox_tool = false;
            let y = self.editor.as_ref().and_then(|ed| ed.content_bottom).unwrap_or(60.0);
            self.place_textbox_at(320.0, (y - 40.0).max(24.0));
        }
    }

    fn place_textbox_at(&mut self, x: f32, y: f32) {
        let Some(ed) = self.editor.as_mut() else { return };
        Self::remember_step(ed);
        let starter = crate::tl_id!("ui.student_textbox_label").replace('\n', " ");
        let starter_len = starter.chars().count();
        let block_idx = ed.blocks.len();
        ed.blocks.push(ergonote_formats::Block::Paragraph(vec![ergonote_formats::Span { text: starter.clone(), ..Default::default() }]));
        ed.boxes.push(ergonote_model::Container { x: x.max(0.0), y: y.max(0.0), width: 260.0, blocks: 1 });
        ed.caret = Some(pageedit::Caret {
            block: block_idx,
            text: starter,
            focus: true,
            selection: (0, starter_len),
            place: None,
            cell: None,
            span: None,
            inline: None,
        });
        ed.restore_format_selection = Some((block_idx, 0, starter_len));
        ed.arranged = true;
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
        Self::remember_edit(ed);
        self.say(crate::tl_id!("ui.student_textbox_placed"));
    }

    fn apply_underline_color(&mut self, color: Option<NodeColor>) {
        let col_opt = color.map(|c| c.id().to_string());
        let pending_col = col_opt.clone();
        self.apply_span_format(pageedit::InlineFormat::UnderlineColor(col_opt), move |pending| {
            pending.underline = pending_col.is_some();
            pending.underline_color = pending_col;
        });
    }

    fn apply_text_color(&mut self, color: Option<NodeColor>) {
        let col_opt = color.map(|c| c.id().to_string());
        let pending_col = col_opt.clone();
        self.apply_span_format(pageedit::InlineFormat::TextColor(col_opt), move |pending| {
            pending.text_color = pending_col;
        });
    }

    fn apply_highlight_color(&mut self, color: Option<NodeColor>) {
        let col_opt = color.map(|c| c.id().to_string());
        let pending_col = col_opt.clone();
        self.apply_span_format(pageedit::InlineFormat::HighlightColor(col_opt), move |pending| {
            pending.highlight_color = pending_col;
        });
    }

    fn apply_span_format(&mut self, format: pageedit::InlineFormat, apply_pending: impl FnOnce(&mut PendingFormat)) {
        if self.editor.as_ref().and_then(|ed| ed.across).is_some_and(|selection| !selection.inside_one_block()) {
            self.apply_across_format(format);
            return;
        }
        let Some(ed) = self.editor.as_mut() else { return };
        Self::place_caret_for_format(ed);
        let Some(caret) = ed.caret.as_ref() else {
            self.status = crate::tl_id!("ui.click_in_the_page_first_then_use_the_ribbon").into();
            self.status_is_error = false;
            return;
        };
        let len = caret.text.chars().count();
        let (lo, hi) = (caret.selection.0.min(caret.selection.1), caret.selection.0.max(caret.selection.1));
        if ed.writable && ed.across.is_none() && caret.span.is_none() && lo < hi && (lo != 0 || hi != len || caret.cell.is_some()) {
            self.apply_partial_inline_format(format);
            return;
        }
        if ed.writable && ed.across.is_none() && caret.cell.is_none() && len == 0 {
            let pending = ed.pending_format.get_or_insert_with(|| PendingFormat { block: caret.block, at: lo, ..Default::default() });
            if pending.block != caret.block || pending.at != lo {
                *pending = PendingFormat { block: caret.block, at: lo, ..Default::default() };
            }
            apply_pending(pending);
            if let Some(caret) = ed.caret.as_mut() {
                caret.focus = true;
                caret.place = Some(lo);
            }
            self.status.clear();
            self.status_is_error = false;
            return;
        }
        if !ed.writable || ed.across.is_some() || caret.cell.is_some() || len == 0 {
            self.status = crate::tl_id!("ui.inline_mark_whole_line").into();
            self.status_is_error = false;
            return;
        }
        let block = caret.block;
        Self::remember_step(ed);
        let spans = match ed.blocks.get_mut(block) {
            Some(
                ergonote_formats::Block::Heading { spans, .. }
                | ergonote_formats::Block::Paragraph(spans)
                | ergonote_formats::Block::Quote(spans)
                | ergonote_formats::Block::Item { spans, .. },
            ) => spans,
            _ => {
                self.status = crate::tl_id!("ui.inline_mark_whole_line").into();
                self.status_is_error = false;
                return;
            }
        };
        if !pageedit::format_span_range(spans, 0..usize::MAX, format) {
            return;
        }
        ed.pending_format = None;
        if let (Some(caret), Some(block)) = (ed.caret.as_mut(), ed.blocks.get(block)) {
            caret.text = pageedit::caret_text(block);
            caret.inline = None;
            caret.selection = (0, caret.text.chars().count());
            caret.focus = true;
            ed.restore_format_selection = Some((caret.block, caret.selection.0, caret.selection.1));
        }
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
        let step = crate::history::Step::new(ed.body.clone(), ed.blocks.clone(), ed.boxes.clone(), ed.caret.clone(), crate::history::Kind::Edit);
        ed.history.record(step);
        self.status.clear();
        self.status_is_error = false;
    }

    /// Captures a board photo from webcam (if available) or opens the picture picker directly,
    /// inserts the photo on the left of the page, and creates a text container right beside it.
    fn insert_board_photo(&mut self) {
        let Some((dir, format)) = self.editor.as_ref().map(|e| (e.dir.clone(), e.format)) else {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return;
        };
        #[cfg(target_os = "android")]
        {
            let _ = format;
            self.android_pick_pending = Some(AndroidPickKind::BoardPhoto(dir));
            self.android_pick_request = Some("image/*".to_string());
        }
        #[cfg(not(target_os = "android"))]
        {
            let captured = Self::try_capture_webcam();
            let chosen = captured.or_else(|| {
                rfd::FileDialog::new()
                    .set_title(crate::tl_id!("ui.choose_a_picture"))
                    .add_filter(crate::tl_id!("ui.pictures"), &["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "jxl"])
                    .pick_file()
            });
            let Some(file) = chosen else { return };
            self.attach_board_photo_file(&dir, format, &file);
        }
    }

    fn attach_board_photo_file(&mut self, dir: &Path, format: PageFormat, file: &Path) {
        let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "tableau.jpg".to_string());
        let Some(Output::Text(link)) = self.run("page.attach", &params(&[("path", &path_str(dir)), ("source", &path_str(file))])) else {
            self.complain(&crate::i18n::format_message_id("ui.could_not_attach_name_to_this_page", &[("name", &name)]));
            return;
        };
        let snippet = snippets::attachment(format, &link, &name);
        let Some(ed) = self.editor.as_mut() else { return };
        let (added, _) = ergonote_formats::boxes::read(ed.format, &snippet);
        if added.is_empty() {
            return;
        }
        let below = ed.content_bottom.unwrap_or_else(|| ed.boxes.iter().map(|c| c.y + 60.0).fold(0.0, f32::max));
        let before = ed.blocks.len();
        let photo_blocks = added.len();
        ed.blocks.extend(added);
        ed.boxes = ergonote_model::container::append(&ed.boxes, before, photo_blocks, 360.0, below, 12.0);
        // Place a companion text container right beside the board photo (x = 380.0, same y) so the student can type immediately.
        let note_block = ed.blocks.len();
        ed.blocks.push(ergonote_formats::Block::Paragraph(Vec::new()));
        let photo_y = ed.boxes.last().map_or(below, |c| c.y);
        ed.boxes.push(ergonote_model::Container { x: 380.0, y: photo_y, width: 340.0, blocks: 1 });
        ed.caret = Some(pageedit::Caret {
            block: note_block,
            text: String::new(),
            focus: true,
            selection: (0, 0),
            place: None,
            cell: None,
            span: None,
            inline: None,
        });
        ed.arranged = true;
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
        Self::remember_step(ed);
        ed.attachments = ergonote_store::assets::list(&ed.dir);
        self.say(crate::i18n::format_message_id("ui.name_is_now_a_file_in_this_page_s_folder", &[("name", &name)]));
    }

    #[cfg(not(target_os = "android"))]
    fn try_capture_webcam() -> Option<PathBuf> {
        #[cfg(target_os = "linux")]
        {
            if !Path::new("/dev/video0").exists() {
                return None;
            }
            let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
            let target = std::env::temp_dir().join(format!("tableau-{stamp}.jpg"));
            let location = format!("location={}", target.display());
            let ok = std::process::Command::new("gst-launch-1.0")
                .args(["-q", "v4l2src", "num-buffers=1", "!", "videoconvert", "!", "jpegenc", "!", "filesink", &location])
                .status()
                .is_ok_and(|s| s.success());
            if ok && target.is_file() {
                return Some(target);
            }
            None
        }
        #[cfg(not(target_os = "linux"))]
        {
            None
        }
    }

    fn insert_screen_clip(&mut self) {
        let Some(dir) = self.editor.as_ref().map(|e| e.dir.clone()) else {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return;
        };
        #[cfg(target_os = "linux")]
        {
            let Ok(dur) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) else { return };
            let target = std::env::temp_dir().join(format!("capture-{}.png", dur.as_secs()));
            let ok = std::process::Command::new("gnome-screenshot").args(["-a", "-f"]).arg(&target).status().is_ok_and(|s| s.success());
            if ok && target.is_file() {
                self.add_file(&dir, &target, FileChoice::Copy);
                return;
            }
        }
        if let Some(file) = rfd::FileDialog::new()
            .set_title(crate::tl_id!("ui.choose_a_picture"))
            .add_filter(crate::tl_id!("ui.pictures"), &["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "jxl"])
            .pick_file()
        {
            self.add_file(&dir, &file, FileChoice::Copy);
        }
    }

    /// Adds, removes, moves or changes a tool on the rack, and remembers the result. The
    /// rules about what may not happen - the last tool cannot be taken off, a tool cannot
    /// be moved off either end - live in `ergonote_ink::rack`, so they hold wherever the
    /// rack is edited from.
    fn edit_rack(&mut self, edit: ribbon::RackEdit) {
        use ergonote_ink::rack;
        match edit {
            ribbon::RackEdit::Add(tool) => {
                if self.rack.len() >= rack::MAX_NIBS {
                    self.say(crate::tl_id!("ui.there_is_no_room_on_the_rack_for_another_tool"));
                    return;
                }
                self.rack.push(rack::new_nib(tool));
                self.nib = self.rack.len() - 1;
                self.pointer = ribbon::Pointer::Draw;
                let tool = crate::tl!(tool.label());
                self.say(crate::i18n::format_message_id("ui.added_a_tool", &[("tool", &tool.to_lowercase())]));
            }
            ribbon::RackEdit::Remove(i) => {
                if !rack::remove(&mut self.rack, i) {
                    self.say(crate::tl_id!("ui.the_last_tool_stays_on_the_rack"));
                    return;
                }
                if self.nib >= self.rack.len() {
                    self.nib = self.rack.len() - 1;
                } else if self.nib > i {
                    self.nib -= 1;
                }
                self.say(crate::tl_id!("ui.took_it_off_the_rack"));
            }
            ribbon::RackEdit::Move(i, left) => {
                let to = rack::shift(&mut self.rack, i, left);
                if self.nib == i {
                    self.nib = to;
                } else if self.nib == to {
                    self.nib = i;
                }
            }
            ribbon::RackEdit::Set(i, nib) => {
                if let Some(slot) = self.rack.get_mut(i) {
                    *slot = nib;
                    self.nib = i;
                    if !matches!(self.pointer, ribbon::Pointer::Shape(_)) {
                        self.pointer = ribbon::Pointer::Draw;
                    }
                }
            }
        }
        self.remember_rack();
    }

    /// Puts whatever is on the clipboard into the page: a picture becomes a file beside
    /// the page and a line that shows it, anything else goes in as words.
    ///
    /// egui hands a window only `Paste(String)`, so a picture copied from a screen
    /// clipper never arrived and Ctrl+V did nothing at all. The clipboard is read here
    /// directly instead.
    fn paste_here(&mut self) {
        let Some(dir) = self.editor.as_ref().map(|e| e.dir.clone()) else {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return;
        };
        if let Some(picture) = self.clipboard.picture() {
            self.paste_picture(&dir, picture);
            return;
        }
        // Browsers, Word and other editors offer a formatted clipboard flavour as well
        // as plain text. Parse it through the same HTML-to-block path as an HTML page, so
        // links, emphasis, lists and tables survive a paste into ErgoNote.
        if let Some(html) = self.clipboard.html() {
            let new = pageedit::pasted_blocks(pageedit::pasted(&html));
            if self.paste_blocks(new) {
                self.say(crate::tl_id!("ui.pasted_formatted_content"));
                return;
            }
        }
        match self.clipboard.text() {
            Some(text) => {
                let new = pageedit::blocks_to_paste(&text);
                if !self.paste_blocks(new) {
                    self.insert_in_page(&text);
                }
                self.say(crate::tl_id!("ui.pasted"));
            }
            None => self.say(crate::tl_id!("ui.there_is_nothing_on_the_clipboard_to_paste")),
        }
    }

    /// Pastes `new` blocks at the caret, or after the last block. False, with nothing done,
    /// when there is nothing to paste or the page cannot be written.
    fn paste_blocks(&mut self, new: Vec<ergonote_formats::Block>) -> bool {
        let Some(ed) = self.editor.as_mut().filter(|ed| ed.writable) else { return false };
        if new.is_empty() {
            return false;
        }
        let at = ed.caret.as_ref().map_or(ed.blocks.len().saturating_sub(1), |c| c.block);
        if ed.blocks.is_empty() {
            ed.blocks.push(ergonote_formats::markdown::Block::Paragraph(Vec::new()));
        }
        let before = ed.blocks.len();
        pageedit::paste_at(&mut ed.blocks, &mut ed.caret, at, new);
        // The paste fills the caret's line and puts the rest after it, all in the caret's box.
        let marks = ed.blocks_inserted(at + 1, ed.blocks.len().saturating_sub(before));
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
        self.save_block_marks(marks);
        true
    }

    /// Writes table widths and justified paragraphs that moved with inserted blocks, as
    /// [`Editor::blocks_inserted`] reported them, into the page's own file.
    fn save_block_marks(&mut self, (cols, justified): (bool, bool)) {
        let Some(ed) = self.editor.as_ref() else { return };
        let path = path_str(&ed.dir);
        let cols = cols.then(|| ergonote_model::tablecols::write(&ed.columns));
        let justified = justified.then(|| ergonote_model::justify::write(&ed.justified));
        if let Some(line) = cols {
            self.run("page.columns", &params(&[("path", &path), ("widths", &line)]));
        }
        if let Some(line) = justified {
            self.run("page.justify", &params(&[("path", &path), ("blocks", &line)]));
        }
    }

    fn paste_picture(&mut self, dir: &std::path::Path, picture: crate::clipboard::Picture) {
        let name = crate::clipboard::picture_name();
        let (w, h) = (picture.width, picture.height);
        log::debug!("clipboard image paste received: width={w} height={h} bytes={}", picture.png.len());
        match ergonote_store::assets::attach_bytes(dir, &name, &picture.png) {
            Ok(link) => {
                log::debug!("clipboard image paste attached: name={name:?} link={link:?}");
                if let Some(ed) = self.editor.as_mut() {
                    let at = ed.caret.as_ref().map_or(ed.blocks.len(), |c| c.block + 1).min(ed.blocks.len());
                    ed.blocks.insert(
                        at,
                        ergonote_formats::markdown::Block::Image { alt: "Pasted picture".to_string(), src: link, width: None, height: None },
                    );
                    let marks = ed.blocks_inserted(at, 1);
                    ed.rebuild_body();
                    Self::mark_dirty_from_blocks(ed);
                    ed.attachments = ergonote_store::assets::list(dir);
                    self.save_block_marks(marks);
                }
                self.say(crate::i18n::format_message_id(
                    "ui.pasted_a_picture_width_by_height",
                    &[("width", &w.to_string()), ("height", &h.to_string())],
                ));
            }
            Err(e) => self.complain(&crate::i18n::format_message_id("ui.cannot_save_the_pasted_picture_error", &[("error", &e.to_string())])),
        }
    }

    /// Keeps the page's words honest about its ink: a page with ink names `ink.svg`, a
    /// page without it does not. Without this the source pane showed a page of text while
    /// the page itself was covered in handwriting, and another program opening the folder
    /// would never have known the drawing was there.
    fn sync_ink_reference(&mut self) {
        let Some(ed) = self.editor.as_mut() else { return };
        // The folder is asked again: the pen has just written or deleted a file.
        ed.ink_files = ergonote_store::page::ink_files(&ed.dir);
        if Self::name_ink(ed) {
            Self::mark_dirty_from_blocks(ed);
        }
    }

    /// The page names every ink file in its folder, and names no ink file that is gone.
    ///
    /// The folder decides. A drawing is a file beside the page file; the line in the page
    /// that names it is how the page's words and what is drawn on it agree, and how
    /// another program opening the page sees the handwriting. Before this, the line could
    /// be lost - deleting the box that held it took it out - and nothing put it back until
    /// the pen next moved, so a page could say nothing about ink that was still there.
    ///
    /// Returns true when the page's text had to be changed.
    fn name_ink(ed: &mut Editor) -> bool {
        use ergonote_formats::markdown::Block;
        let files = ed.ink_files.clone();
        let mut changed = false;
        // A line that names an ink file that is no longer in the folder goes.
        // One at a time, from the end, so each box that held one counts one fewer.
        let gone: Vec<usize> = ed
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| match b {
                Block::Image { src, alt, .. } => crate::inkpad::is_ink_line(src, alt) && !files.iter().any(|f| f == src.trim()),
                _ => false,
            })
            .map(|(i, _)| i)
            .collect();
        for &at in gone.iter().rev() {
            ed.blocks.remove(at);
            ed.count_removed_from_boxes(at);
            changed = true;
        }
        // An ink file the page does not name is adopted: it is drawn, so it is named. The line
        // goes on the end of the page, so it is counted in the last box.
        for file in &files {
            if !ed.blocks.iter().any(|b| matches!(b, Block::Image { src, .. } if src.trim() == file)) {
                ed.blocks.push(Block::Image { alt: "Ink".to_string(), src: file.clone(), width: None, height: None });
                let at = ed.blocks.len() - 1;
                ed.count_inserted_in_boxes(at, 1);
                changed = true;
            }
        }
        if changed {
            ed.rebuild_body();
        }
        changed
    }

    /// Writes the rack and which tool is in hand into the settings, so the next run opens
    /// with the pens the owner arranged.
    fn remember_rack(&mut self) {
        let line = ergonote_ink::rack::write(&self.rack);
        let _ = self.session.settings.set(RACK_KEY, &line);
        let _ = self.session.settings.set(NIB_KEY, &self.nib.to_string());
    }

    /// The pen as the page's ink layer wants it: what it is, and what the pointer is for.
    fn pen(&self) -> crate::inkpad::Pen {
        let nib =
            self.rack.get(self.nib).or_else(|| self.rack.first()).copied().unwrap_or_else(|| ergonote_ink::rack::new_nib(ergonote_ink::Tool::Pen));
        crate::inkpad::Pen {
            pointer: self.pointer,
            tool: nib.tool,
            colour: nib.colour,
            // Keep the chosen nib width at the neutral setting, but let the top of the
            // control reach the same four-times full-width range as the pressure canvas.
            width: nib.width() * Self::sensitivity_width(self.sensitivity),
            gain: self.gain(),
        }
    }

    /// The pressure canvas uses a 4x full-width range at sensitivity 100. Keep 50 neutral
    /// so the rack's selected thickness remains the familiar baseline, and grow only the
    /// maximum width above it. Existing strokes retain the width stored when they were made.
    fn sensitivity_width(sensitivity: u8) -> f32 {
        if sensitivity <= 50 { 1.0 } else { 1.0 + 3.0 * (f32::from(sensitivity - 50) / 50.0) }
    }

    /// The sensitivity as the curve a stroke is drawn with: the middle of the control is
    /// 1, which leaves the pen's readings exactly as they came.
    fn gain(&self) -> f32 {
        let share = f32::from(self.sensitivity.min(100)) / 50.0;
        if share <= 1.0 {
            ergonote_ink::MIN_GAIN + (1.0 - ergonote_ink::MIN_GAIN) * share
        } else {
            1.0 + (ergonote_ink::MAX_GAIN - 1.0) * (share - 1.0)
        }
    }

    /// Puts back the change to the drawing that undo last took back.
    fn ink_redo(&mut self) {
        let Some(pad) = self.pad.as_mut() else {
            self.say(crate::tl_id!("ui.nothing_to_redo"));
            return;
        };
        if !pad.redo() {
            self.say(crate::tl_id!("ui.nothing_to_redo"));
            return;
        }
        let strokes = pad.strokes();
        match pad.save() {
            Ok(()) => self.say(crate::i18n::format_message_id("ui.drawing_saved_count_strokes", &[("count", &strokes.to_string())])),
            Err(e) => self.complain(&crate::i18n::format_message_id("ui.cannot_write_the_drawing_error", &[("error", &e.to_string())])),
        }
        self.sync_ink_reference();
    }

    /// Takes the last stroke back out of the drawing on this page.
    fn ink_undo(&mut self) {
        let Some(pad) = self.pad.as_mut() else {
            self.say(crate::tl_id!("ui.there_is_no_drawing_on_this_page"));
            return;
        };
        if !pad.undo() {
            self.say(crate::tl_id!("ui.there_is_no_stroke_to_take_back"));
            return;
        }
        let left = pad.strokes();
        match pad.save() {
            Ok(()) => self.say(crate::i18n::format_message_id("ui.took_the_last_stroke_back_count_left", &[("count", &left.to_string())])),
            Err(e) => self.complain(&crate::i18n::format_message_id("ui.cannot_write_the_drawing_error", &[("error", &e.to_string())])),
        }
        self.tell_kept_ink();
        // The page stops naming ink it no longer has.
        self.sync_ink_reference();
    }

    /// Whether the caret is in a table, which is when the Table tab is offered.
    /// The Styles kind of the block the caret is in (or that a selected box is all of), for the ribbon to highlight; "" with neither.
    fn caret_style(&self, ctx: &Context) -> &'static str {
        let Some(ed) = self.editor.as_ref() else { return "" };
        if let Some(block) = ed.caret.as_ref().and_then(|c| ed.blocks.get(c.block)) {
            return pageedit::block_kind(block);
        }
        // A selected box has everything in it selected: its style is the one they all share.
        let range = crate::containers::selected(ctx, &ed.dir).map(|i| ergonote_model::container::blocks_of(&ed.boxes, i)).unwrap_or_default();
        let mut kinds = range.filter_map(|i| ed.blocks.get(i)).map(pageedit::block_kind);
        match kinds.next() {
            Some(first) if kinds.all(|k| k == first) => first,
            _ => "",
        }
    }

    fn caret_in_table(&self) -> bool {
        self.editor
            .as_ref()
            .and_then(|e| e.caret.as_ref().map(|c| (e, c)))
            .is_some_and(|(e, c)| c.cell.is_some() && matches!(e.blocks.get(c.block), Some(ergonote_formats::markdown::Block::Table { .. })))
    }

    /// Whether the table the caret is in is drawn with lines round its cells. A table has
    /// them until its own borders are hidden, so with the caret outside any table this reads
    /// as true.
    fn caret_table_has_borders(&self) -> bool {
        let Some(ed) = self.editor.as_ref() else { return true };
        let Some(c) = ed.caret.as_ref().filter(|c| c.cell.is_some()) else { return true };
        !ed.columns.borders_hidden(c.block)
    }

    /// The cells Table > Align acts on: every picked cell, or the caret's own. With the
    /// table's block.
    fn caret_cells(&self) -> Option<(usize, Vec<(usize, usize)>)> {
        let ed = self.editor.as_ref()?;
        let c = ed.caret.as_ref()?;
        let here = c.cell?;
        if !matches!(ed.blocks.get(c.block), Some(ergonote_formats::markdown::Block::Table { .. })) {
            return None;
        }
        let cells = match pageedit::cell_range(c) {
            Some((a, b)) => (a.0..=b.0).flat_map(|r| (a.1..=b.1).map(move |col| (r, col))).collect(),
            None => vec![here],
        };
        Some((c.block, cells))
    }

    /// How the cells Table > Align would act on are aligned now, when they all agree. A cell
    /// given no alignment of its own reads as aligned left, which is how it is drawn.
    fn caret_cells_align(&self) -> Option<ergonote_model::tablecols::Align> {
        use ergonote_model::tablecols::Align;
        let ed = self.editor.as_ref()?;
        let (at, cells) = self.caret_cells()?;
        let mut aligns = cells.iter().map(|cell| ed.columns.align(at, *cell).unwrap_or(Align::Left));
        let first = aligns.next()?;
        aligns.all(|a| a == first).then_some(first)
    }

    /// The colour behind the cells Table > Shading would act on, when they all agree.
    fn caret_cells_shade(&self) -> Option<Option<ergonote_model::NodeColor>> {
        let ed = self.editor.as_ref()?;
        let (at, cells) = self.caret_cells()?;
        let mut shades = cells.iter().map(|cell| ed.columns.shade(at, *cell));
        let first = shades.next()?;
        shades.all(|s| s == first).then_some(first)
    }

    /// Table > Shading: a colour behind the caret's cell, or behind every picked cell, and no
    /// other; No Colour takes it away. Kept with the page beside the cells' alignment, and
    /// noted on the page's history step as the borders and the alignment are (issue #169).
    fn shade_table_cells(&mut self, shade: Option<ergonote_model::NodeColor>) {
        let Some((at, cells)) = self.caret_cells() else {
            self.say(crate::tl_id!("ui.click_in_the_table_first"));
            return;
        };
        let Some(ed) = self.editor.as_mut() else { return };
        if !ed.writable || !ed.columns.set_shade(at, &cells, shade) {
            return;
        }
        Self::remember_edit(ed);
        self.save_block_marks((true, false));
    }

    /// Table > Align Left, Center or Align Right: the words in the caret's cell, or in every
    /// cell picked, and nothing else; the columns keep their widths and the table its place.
    /// Kept with the page, like the widths and the borders, and one step of Undo (issue #170).
    fn align_table_cells(&mut self, align: ergonote_model::tablecols::Align) {
        let Some((at, cells)) = self.caret_cells() else {
            self.say(crate::tl_id!("ui.click_in_the_table_first"));
            return;
        };
        let Some(ed) = self.editor.as_mut() else { return };
        if !ed.writable {
            return;
        }
        // Left is how a cell is drawn when it was given nothing, so it is kept as nothing.
        let keep = (align != ergonote_model::tablecols::Align::Left).then_some(align);
        if !ed.columns.set_align(at, &cells, keep) {
            return;
        }
        Self::remember_edit(ed);
        self.save_block_marks((true, false));
    }

    /// Table > Hide Borders or Show Borders: the table the caret is in, and no other. The
    /// choice is kept with the page, like the widths of the table's columns, so it is there
    /// when the page is opened again (issue #161).
    fn set_table_borders(&mut self, on: bool) {
        let Some(ed) = self.editor.as_mut() else { return };
        if !ed.writable {
            return;
        }
        let Some(at) = ed.caret.as_ref().filter(|c| c.cell.is_some()).map(|c| c.block) else {
            self.say(crate::tl_id!("ui.click_in_the_table_first"));
            return;
        };
        if !matches!(ed.blocks.get(at), Some(ergonote_formats::markdown::Block::Table { .. })) || !ed.columns.set_borders(at, !on) {
            return;
        }
        // Noted on the step the page is at, which is how a dragged column is noted too: an
        // undo back to here keeps the choice.
        Self::remember_edit(ed);
        self.save_block_marks((true, false));
    }

    /// Writes a file of the open page that was typed in in the code view.
    ///
    /// Only the files the code view lets anybody type in reach here: a drawing's SVG. The
    /// page file's body is not one of them; it is the editor's own text and is saved the
    /// way it always was.
    fn write_page_file(&mut self, name: &str, text: &str) {
        let Some(dir) = self.editor.as_ref().map(|e| e.dir.clone()) else { return };
        let is_page_file = self.editor.as_ref().is_some_and(|e| e.format.page_file() == name);
        let path = dir.join(name.replace('/', std::path::MAIN_SEPARATOR_STR));
        if !path.starts_with(&dir) {
            self.complain(crate::tl_id!("ui.that_file_is_not_in_this_page_s_folder"));
            return;
        }
        // The page file may be the one an autosave is still writing.
        self.take_saves(true);
        if let Err(e) = ergonote_store::page::write_atomic(&path, text) {
            self.complain(&crate::i18n::format_message_id("ui.cannot_write_name_error", &[("name", name), ("error", &e.to_string())]));
            return;
        }
        if is_page_file {
            // The page itself was typed in. Nothing the window remembered about it is
            // true any more, and the autosave must not write its old copy over this one;
            // the page is read again when the view closes.
            if let Some(ed) = self.editor.as_mut() {
                ed.dirty_since = None;
                ed.body = ergonote_store::page::load_page(&dir).map(|p| p.body).unwrap_or_else(|_| ed.body.clone());
                let (blocks, _) = ergonote_formats::boxes::read(ed.format, &ed.body);
                let audit = ergonote_formats::boxes::span_audit(&ed.body, &blocks);
                log::debug!("code view commit span audit: page={:?} raw_span_tags={} styled_spans={}", dir, audit.raw_tags, audit.styled);
            }
            self.code.page_edited = true;
            return;
        }
        // A drawing that was typed in is read again, so what the pen does next is done to
        // the file as it now stands rather than to the one the window remembered.
        if ergonote_ink::svg::is_ink(text) {
            self.pad = Some(crate::inkpad::Pad::of_page(&dir));
        }
        self.sync_ink_reference();
    }

    /// The paper the open page is drawn on: its own, or the window's choice when it has
    /// none of its own.
    fn paper_now(&self) -> ribbon::RuleLines {
        self.editor.as_ref().and_then(|e| e.paper).unwrap_or(self.rules)
    }

    /// Gives the open page its own paper, its own colour, or both, and writes them into
    /// the page's file. `None` for either leaves that one as it is.
    fn set_page_appearance(&mut self, paper: Option<ribbon::RuleLines>, colour: Option<Option<ergonote_model::NodeColor>>) {
        let Some(ed) = self.editor.as_mut() else {
            self.say("Open a page first");
            return;
        };
        if let Some(p) = paper {
            ed.paper = Some(p);
        }
        if let Some(c) = colour {
            ed.colour = c;
        }
        let (path, paper, colour) = (path_str(&ed.dir), ed.paper, ed.colour);
        let pairs: Vec<(&str, &str)> = vec![
            ("path", path.as_str()),
            ("paper", paper.map(ribbon::RuleLines::id).unwrap_or("none")),
            ("color", colour.map(ergonote_model::NodeColor::id).unwrap_or("none")),
        ];
        self.run("page.paper", &params(&pairs));
    }

    /// Gives the open page its sheet, or takes it back to Auto, and writes it into the page's
    /// file (issue #269). The words are not touched: only how the page is laid out to show it.
    fn set_paper_size(&mut self, paper: Option<ergonote_model::paper::Paper>) {
        let Some(ed) = self.editor.as_mut() else {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return;
        };
        if ed.paper_size == paper {
            return;
        }
        let was_size = ed.paper_size.map(|p| (std::mem::discriminant(&p.size), p.landscape));
        ed.paper_size = paper;
        let path = path_str(&ed.dir);
        let line = paper.map_or_else(|| "auto".to_string(), ergonote_model::paper::Paper::write);
        self.run("page.paper_size", &params(&[("path", &path), ("size", &line)]));
        // Said when the sheet or its turn changes, not at every step of a dragged margin.
        let now_size = paper.map(|p| (std::mem::discriminant(&p.size), p.landscape));
        if now_size != was_size || paper.is_none() {
            self.say(match paper {
                Some(p) => crate::i18n::format_message_id(
                    "ui.the_page_is_written_on_size_turn",
                    &[
                        ("size", crate::tl!(p.size.label())),
                        ("turn", &(if p.landscape { crate::tl_id!("ui.landscape") } else { crate::tl_id!("ui.portrait") }).to_lowercase()),
                    ],
                ),
                None => crate::tl_id!("ui.the_page_is_auto_it_grows_with_what_is_on_it").to_string(),
            });
        }
    }

    /// Takes the page back to how it was before the last change, or forward again.
    ///
    /// The whole page is put back: its text, its blocks, the boxes they sit in and where
    /// the caret was. Ink is not part of this; a stroke is taken back with Undo Stroke on
    /// the Draw tab, because the drawing is its own file beside the page.
    fn step_back(&mut self, back: bool) {
        let mut nothing = false;
        let mut cols_moved = false;
        let mut layout_moved = false;
        let mut restored_lock: Option<bool> = None;
        if let Some(ed) = self.editor.as_mut() {
            if !ed.writable {
                nothing = true;
            } else {
                let step = if back { ed.history.undo().cloned() } else { ed.history.redo().cloned() };
                match step {
                    Some(step) => {
                        ed.body = step.body;
                        ed.blocks = step.blocks;
                        ed.boxes = step.boxes;
                        ed.caret = step.caret.map(|c| pageedit::Caret { focus: true, ..c });
                        // A selection across blocks is kept in byte offsets into the text as
                        // it was; the restored text can differ (a format adds its markup), so
                        // the selection is let go rather than left pointing at the wrong words.
                        ed.across = None;
                        ed.pending_format = None;
                        ed.dirty_since = Some(Instant::now());
                        // The restored text says whether the page was arranged.
                        ed.arranged = ergonote_formats::boxes::has_boxes(ed.format, &ed.body);
                        // The table widths as they were then, written back when they differ.
                        if let Some(columns) = step.columns
                            && columns != ed.columns
                        {
                            ed.columns = columns;
                            cols_moved = true;
                        }
                        // And which paragraphs were justified or aligned (issue #258).
                        if let Some(justified) = step.justified
                            && justified != ed.justified
                        {
                            ed.justified = justified;
                            layout_moved = true;
                        }
                        if let Some(locked) = step.lock_images {
                            restored_lock = Some(locked);
                        }
                    }
                    None => nothing = true,
                }
            }
        }
        if let Some(locked) = restored_lock
            && locked != self.lock_images
        {
            self.lock_images = locked;
            let _ = self.session.settings.set("lock_images", if locked { "true" } else { "false" });
        }
        if cols_moved || layout_moved {
            self.save_block_marks((cols_moved, layout_moved));
        }
        if nothing {
            self.say(if back { crate::tl_id!("ui.nothing_to_undo") } else { crate::tl_id!("ui.nothing_to_redo") });
        }
    }

    /// Remembers the page as it stands now, so the change that just happened can be taken
    /// back. Called once a frame, after whatever changed has been applied.
    ///
    /// Letters going into the block the caret was already in fold into the step before
    /// them, so Ctrl+Z takes back a run of writing rather than one character.
    fn remember_step(ed: &mut Editor) {
        Self::remember(ed, false);
    }

    /// Remembers a change made from the ribbon rather than by writing, such as Insert Space
    /// or a row put into a table, as a step of its own. It never folds into the writing
    /// before it, even when the caret stays in the same block (issue #153).
    fn remember_edit(ed: &mut Editor) {
        Self::remember(ed, true);
    }

    fn remember(ed: &mut Editor, edit: bool) {
        let same = ed.history.here().is_some_and(|s| s.body == ed.body && s.blocks == ed.blocks && s.boxes == ed.boxes);
        if same || {
            // Only the cursor's place on open paper came or went: the page is the one remembered.
            let saved = ed.body_to_save();
            saved != ed.body && ed.history.here().is_some_and(|s| s.body == saved)
        } {
            // Nothing really changed, or the page was just put back by undo itself. A column
            // dragged since is still noted, so an undo back to here keeps it (issue #154).
            if let Some(step) = ed.history.here_mut() {
                step.columns = Some(ed.columns.clone());
                step.justified = Some(ed.justified.clone());
            }
            return;
        }
        let here = ed.caret.as_ref().map(|c| c.block);
        let was = ed.history.here().and_then(|s| s.caret.as_ref().map(|c| c.block));
        let shape = ed.history.here().is_some_and(|s| s.blocks.len() == ed.blocks.len());
        let kind = match (shape, here, was) {
            (true, Some(a), Some(b)) if a == b && !edit => crate::history::Kind::Typing(a),
            _ => crate::history::Kind::Edit,
        };
        let mut step = crate::history::Step::new(ed.body.clone(), ed.blocks.clone(), ed.boxes.clone(), ed.caret.clone(), kind);
        step.columns = Some(ed.columns.clone());
        step.justified = Some(ed.justified.clone());
        ed.history.record(step);
    }

    /// Puts the spreadsheet back into the table it came from.
    ///
    /// The file is the one Convert wrote for this table, beside the page. Nothing watches
    /// it: a page would be a strange thing to write in if its tables changed under the
    /// writer's hands, so the way back is a button, as the way out is.
    fn read_table_from_gridcraft(&mut self) {
        let Some(ed) = self.editor.as_mut() else { return };
        let Some(at) = ed.caret.as_ref().map(|c| c.block) else {
            self.say(crate::tl_id!("ui.click_in_the_table_first"));
            return;
        };
        if !matches!(ed.blocks.get(at), Some(ergonote_formats::markdown::Block::Table { .. })) {
            self.say(crate::tl_id!("ui.click_in_a_table_first"));
            return;
        }
        let file = ed.dir.join(format!("table-{}.csv", at + 1));
        let Ok(text) = std::fs::read_to_string(&file) else {
            let name = file.file_name().unwrap_or_default().to_string_lossy().into_owned();
            self.complain(&crate::i18n::format_message_id("ui.no_name_beside_this_page_hand_the_table_over_first", &[("name", &name)]));
            return;
        };
        let Some((table, rows, columns)) = table_from_gridcraft_csv(&text) else {
            self.complain(crate::tl_id!("ui.that_file_has_nothing_in_it_to_put_back"));
            return;
        };
        if let Some(block) = ed.blocks.get_mut(at) {
            *block = table;
        }
        ed.caret = None;
        ed.body = pageedit::to_body(ed.format, &ed.blocks);
        Self::mark_dirty_from_blocks(ed);
        self.say(crate::i18n::format_message_id(
            "ui.read_back_rows_rows_and_columns_columns",
            &[("rows", &rows.to_string()), ("columns", &columns.to_string())],
        ));
    }

    /// Hands the table the caret is in to GridCraft, which is where a spreadsheet
    /// belongs: OneNote's Table tab ends with Convert to Excel Spreadsheet, and this is
    /// the same door with the open spreadsheet behind it.
    fn hand_table_to_gridcraft(&mut self) {
        let Some(exe) = self.gridcraft.clone() else {
            self.say(crate::tl_id!("ui.gridcraft_is_not_installed"));
            return;
        };
        let Some(ed) = self.editor.as_ref() else { return };
        let Some(at) = ed.caret.as_ref().map(|c| c.block) else { return };
        let Some(block) = ed.blocks.get(at) else { return };
        let name = format!("table-{}", at + 1);
        match crate::gridcraft::hand_over(&exe, &ed.dir, &name, block) {
            Ok(file) => {
                let name = file.file_name().unwrap_or_default().to_string_lossy().to_string();
                let said = crate::i18n::format_message_id("ui.name_is_open_in_gridcraft", &[("name", &name)]);
                self.say(&said);
            }
            Err(e) => self.complain(&crate::i18n::format_message_id("ui.cannot_open_the_table_in_gridcraft_error", &[("error", &e.to_string())])),
        }
    }

    /// A table of the picked size, where the caret is.
    fn insert_table(&mut self, rows: usize, columns: usize) {
        let Some(ed) = self.editor.as_mut() else { return };
        let at = ed.caret.as_ref().map(|c| c.block + 1).unwrap_or(ed.blocks.len()).min(ed.blocks.len());
        let before = ed.blocks.len();
        pageedit::insert_table(&mut ed.blocks, at, rows, columns);
        let marks = ed.blocks_inserted(at, ed.blocks.len().saturating_sub(before));
        ed.rebuild_body();
        ed.caret = Some(pageedit::Caret {
            block: at.min(ed.blocks.len() - 1),
            text: String::new(),
            focus: true,
            selection: (0, 0),
            place: None,
            cell: Some((0, 0)),
            span: None,
            inline: None,
        });
        Self::mark_dirty_from_blocks(ed);
        self.save_block_marks(marks);
    }

    fn insert_school_table(&mut self, kind: &str) {
        let Some(ed) = self.editor.as_mut() else {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return;
        };
        if !ed.writable {
            return;
        }
        let (md, initial_cell) = match kind {
            "conversion" => (
                "| km | hm | dam | m | dm | cm | mm |\n| --- | --- | --- | --- | --- | --- | --- |\n|  |  |  |  |  |  |  |\n|  |  |  |  |  |  |  |\n|  |  |  |  |  |  |  |\n",
                (1, 0),
            ),
            "numeration" => (
                "| C. mille | D. mille | U. mille | Centaines | Dizaines | Unités | , | Dixièmes | Centièmes | Millièmes |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n|  |  |  |  |  |  | , |  |  |  |\n|  |  |  |  |  |  | , |  |  |  |\n|  |  |  |  |  |  | , |  |  |  |\n",
                (1, 0),
            ),
            "operation" => (
                "| Op. | Milliers (M) | Centaines (C) | Dizaines (D) | Unités (U) |\n| --- | --- | --- | --- | --- |\n| Retenues |  |  |  |  |\n| Nombre 1 |  |  |  |  |\n| + / - / × |  |  |  |  |\n| = Résultat |  |  |  |  |\n",
                (2, 1),
            ),
            _ => ("| Grandeur A | Valeur 1 | Valeur 2 | Valeur 3 |\n| --- | --- | --- | --- |\n| Grandeur B (× …) |  |  |  |\n", (1, 0)),
        };
        let Some(table_block) = ergonote_formats::markdown::parse(md).into_iter().next() else {
            return;
        };
        let at = ed.caret.as_ref().map(|c| c.block + 1).unwrap_or(ed.blocks.len()).min(ed.blocks.len());
        let before = ed.blocks.len();
        ed.blocks.insert(at, table_block);
        let marks = ed.blocks_inserted(at, ed.blocks.len().saturating_sub(before));
        ed.rebuild_body();
        ed.caret = Some(pageedit::Caret {
            block: at.min(ed.blocks.len() - 1),
            text: String::new(),
            focus: true,
            selection: (0, 0),
            place: None,
            cell: Some(initial_cell),
            span: None,
            inline: None,
        });
        Self::mark_dirty_from_blocks(ed);
        self.save_block_marks(marks);
        self.say("Gabarit scolaire TND inséré dans la page.");
    }

    /// An empty spreadsheet beside the page, opened in GridCraft and linked from the page
    /// so it can be found again.
    fn new_gridcraft(&mut self) {
        let Some(exe) = self.gridcraft.clone() else {
            self.say(crate::tl_id!("ui.gridcraft_is_not_installed"));
            return;
        };
        let Some(ed) = self.editor.as_ref() else { return };
        let dir = ed.dir.clone();
        let mut name = "spreadsheet.csv".to_string();
        for n in 2..100 {
            if !dir.join(&name).exists() {
                break;
            }
            name = format!("spreadsheet-{n}.csv");
        }
        let file = dir.join(&name);
        if let Err(e) = std::fs::write(&file, "") {
            self.complain(&crate::i18n::format_message_id("ui.cannot_write_name_error", &[("name", &name), ("error", &e.to_string())]));
            return;
        }
        match std::process::Command::new(exe).arg(&file).spawn() {
            Ok(_) => {
                self.insert_in_page(&format!("[{name}]({name})"));
                let said = crate::i18n::format_message_id("ui.name_is_open_in_gridcraft", &[("name", &name)]);
                self.say(&said);
            }
            Err(e) => self.complain(&crate::i18n::format_message_id("ui.cannot_open_gridcraft_error", &[("error", &e.to_string())])),
        }
    }

    /// Picks a block of cells from the Table tab's Select buttons, as a drag across them
    /// would.
    fn select_in_table(&mut self, what: pageedit::TableSpan) {
        let Some(ed) = self.editor.as_mut() else { return };
        let Some((at, cell)) = ed.caret.as_ref().and_then(|c| c.cell.map(|cell| (c.block, cell))) else {
            return;
        };
        let Some(block) = ed.blocks.get(at) else { return };
        let Some((from, to)) = pageedit::span_of(block, cell, what) else { return };
        if let Some(caret) = ed.caret.as_mut() {
            caret.cell = Some(from);
            caret.span = Some(to);
            caret.text = pageedit::cell_text(block, from).unwrap_or_default();
            caret.focus = false;
        }
    }

    /// Changes the shape of the table the caret is in, from the Table tab.
    ///
    /// A block of cells picked across several rows or columns takes them all: Delete Rows
    /// with three rows picked takes the three, the way it does in OneNote.
    /// Table > Sort: the table's rows by the caret's column, or only the picked rows when a
    /// block of cells running over more than one row is picked (issue #171). One step of
    /// the page's history, so Undo puts the rows back in their order.
    fn sort_table(&mut self, descending: bool) {
        let Some(ed) = self.editor.as_mut() else { return };
        let Some((at, cell)) = ed.caret.as_ref().and_then(|c| c.cell.map(|cell| (c.block, cell))) else {
            self.say(crate::tl_id!("ui.click_in_the_table_first"));
            return;
        };
        let picked = ed.caret.as_ref().and_then(pageedit::cell_range).filter(|(a, b)| a.0 != b.0).map(|(a, b)| (a.0, b.0));
        Self::remember_step(ed);
        let Some(block) = ed.blocks.get_mut(at) else { return };
        let Some(order) = pageedit::sort_table_rows(block, cell.1, picked, descending) else {
            self.say(crate::tl_id!("ui.the_rows_are_already_in_that_order"));
            return;
        };
        // The caret stays in the same place in the table, and shows what is there now.
        let text = pageedit::cell_text(block, cell).unwrap_or_default();
        ed.rebuild_body();
        if let Some(caret) = ed.caret.as_mut() {
            caret.text = text;
            caret.selection = (0, 0);
            caret.inline = None;
        }
        // An aligned cell goes where its row went (issue #170).
        let cols_moved = ed.columns.reorder_rows(at, &order);
        Self::mark_dirty_from_blocks(ed);
        Self::remember_edit(ed);
        if cols_moved {
            self.save_block_marks((true, false));
        }
    }

    /// Table > Delete Table (issue #297): takes the table the caret is in out of the page, as one
    /// Undo step. The caret goes to the block that takes its place (the one after it, or the
    /// one before when it was last); a page left with nothing gets an empty line to type on.
    /// Table widths, cell marks and justified paragraphs after it move up with their blocks.
    fn request_delete_table(&mut self, block: Option<usize>) {
        use ergonote_formats::markdown::Block;
        let Some(ed) = self.editor.as_ref() else { return };
        let at = block.or_else(|| ed.caret.as_ref().filter(|c| c.cell.is_some()).map(|c| c.block));
        let Some(at) = at.filter(|&at| matches!(ed.blocks.get(at), Some(Block::Table { .. }))) else {
            self.say(crate::tl_id!("ui.put_the_caret_in_a_table_first"));
            return;
        };
        self.dialog = Some(Dialog::DeleteTable { block: at });
    }

    fn delete_table(&mut self) {
        use ergonote_formats::markdown::Block;
        let Some(ed) = self.editor.as_ref() else { return };
        let at = ed.caret.as_ref().filter(|c| c.cell.is_some()).map(|c| c.block);
        let Some(at) = at.filter(|&at| matches!(ed.blocks.get(at), Some(Block::Table { .. }))) else {
            self.say(crate::tl_id!("ui.put_the_caret_in_a_table_first"));
            return;
        };
        self.delete_table_at(at);
    }

    fn delete_table_at(&mut self, at: usize) {
        use ergonote_formats::markdown::Block;
        let Some(ed) = self.editor.as_mut() else { return };
        if !matches!(ed.blocks.get(at), Some(Block::Table { .. })) {
            self.say(crate::tl_id!("ui.put_the_caret_in_a_table_first"));
            return;
        }
        Self::remember_step(ed);
        ed.caret = None;
        ed.blocks.remove(at);
        ed.count_removed_from_boxes(at);
        let cols = ed.columns.remove_blocks(at..at + 1);
        let justified = ed.justified.remove_blocks(at..at + 1);
        if ed.blocks.is_empty() {
            ed.blocks.push(Block::Paragraph(Vec::new()));
            ed.count_inserted_in_boxes(0, 1);
        }
        pageedit::renumber(&mut ed.blocks);
        ed.rebuild_body();
        let to = at.min(ed.blocks.len() - 1);
        let text = pageedit::text_for_caret(&ed.blocks, to);
        ed.caret = Some(pageedit::Caret { block: to, text, focus: true, selection: (0, 0), place: Some(0), cell: None, span: None, inline: None });
        Self::mark_dirty_from_blocks(ed);
        Self::remember_edit(ed);
        self.save_block_marks((cols, justified));
        self.say(crate::tl_id!("ui.deleted_the_table"));
    }

    fn edit_table(&mut self, what: pageedit::TableEdit) {
        let Some(ed) = self.editor.as_mut() else { return };
        let Some((at, cell)) = ed.caret.as_ref().and_then(|c| c.cell.map(|cell| (c.block, cell))) else {
            return;
        };
        // Deleting works from the top-left of what is picked, over and over: each one
        // after the first falls into the place the one before it left. A picked block
        // that takes in the heading row starts under it, so picking the whole table and
        // asking for its rows empties the body and keeps the heading.
        let picked = ed.caret.as_ref().and_then(pageedit::cell_range);
        let (cell, times) = match (what, picked) {
            (pageedit::TableEdit::DeleteRow, Some((a, b))) => {
                let start = a.0.max(1);
                ((start, a.1), (b.0 + 1).saturating_sub(start))
            }
            (pageedit::TableEdit::DeleteColumn, Some((a, b))) => (a, b.1 - a.1 + 1),
            _ => (cell, 1),
        };
        // What was written before is kept as it stood, so Undo takes back only the table change.
        Self::remember_step(ed);
        let Some(block) = ed.blocks.get_mut(at) else { return };
        let mut last = None;
        let mut done = 0;
        for _ in 0..times {
            match pageedit::edit_table(block, cell, what) {
                Some(to) => {
                    last = Some(to);
                    done += 1;
                }
                None => break,
            }
        }
        let Some(to) = last else {
            // The rules said no: a table keeps its heading row and at least one column.
            self.say(crate::tl_id!("ui.a_table_keeps_its_heading_row_and_at_least_one_column"));
            return;
        };
        let text = pageedit::cell_text(block, to).unwrap_or_default();
        ed.rebuild_body();
        ed.caret = Some(pageedit::Caret { block: at, text, focus: true, selection: (0, 0), place: None, cell: Some(to), span: None, inline: None });
        // A width the page keeps for this table stays with the column it was dragged for, so a
        // column put in or taken out does not hand its neighbours the wrong widths (issue #154).
        let cols_moved = match what {
            pageedit::TableEdit::InsertColumn(left) => ed.columns.insert_column(at, if left { cell.1 } else { cell.1 + 1 }, pageedit::COLUMN_W),
            pageedit::TableEdit::DeleteColumn => (0..done).fold(false, |moved, _| ed.columns.remove_column(at, cell.1) || moved),
            // An aligned cell stays with its row too (issue #170).
            pageedit::TableEdit::InsertRow(above) => ed.columns.insert_row(at, if above { cell.0 } else { cell.0 + 1 }),
            pageedit::TableEdit::DeleteRow => (0..done).fold(false, |moved, _| ed.columns.remove_row(at, cell.0) || moved),
        };
        Self::mark_dirty_from_blocks(ed);
        Self::remember_edit(ed);
        self.save_block_marks((cols_moved, false));
    }

    /// The sizes typed into an open Insert Table dialog, as a person would type them. For tests
    /// and automation; does nothing when the dialog is not open.
    pub fn set_insert_table_size(&mut self, rows: &str, columns: &str) {
        if let Some(Dialog::InsertTable { rows: r, columns: c, .. }) = self.dialog.as_mut() {
            *r = rows.to_string();
            *c = columns.to_string();
        }
    }

    /// The open page's blocks, as the editor holds them.
    pub fn editor_blocks(&self) -> Vec<ergonote_formats::Block> {
        self.editor.as_ref().map(|e| e.blocks.clone()).unwrap_or_default()
    }

    /// Picks up a pen, which is all it takes: the whole page is the ink's, so there is no
    /// sheet to put anywhere first.
    fn ink_insert(&mut self) {
        if self.editor.is_none() {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return;
        }
        self.pointer = ribbon::Pointer::Draw;
        self.ribbon_tab = "Draw".to_string();
        self.say(crate::tl_id!("ui.write_anywhere_on_the_page"));
    }

    /// Says where the page's ink file was kept, when writing the ink kept it first because it
    /// held more than could be read back as strokes (issue #157).
    fn tell_kept_ink(&mut self) {
        let Some(kept) = self.pad.as_mut().and_then(|pad| pad.kept_original.take()) else { return };
        let shown = self.shown(&kept);
        log::warn!("the page's ink file held more than its strokes; the original was kept: {kept:?}");
        self.say(crate::i18n::format_message_id("ui.kept_the_original_drawing_file_path", &[("path", &shown)]));
    }

    /// Writes the drawing again with no pen readings in it, for sending somewhere else.
    fn ink_export_plain(&mut self) {
        let Some(pad) = self.pad.as_ref() else {
            self.say(crate::tl_id!("ui.there_is_no_drawing_on_this_page"));
            return;
        };
        let saved = pad.save_plain().and_then(|path| {
            if let Some(downloads) = self.file_downloads.as_mut() {
                let bytes = ergonote_store::vfs::read(&path)?;
                let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_else(|| "ink-picture.svg".to_owned());
                downloads.push(FileDownload { name, mime: "image/svg+xml", bytes });
            }
            Ok(path)
        });
        match saved {
            Ok(path) => {
                let shown = if self.file_downloads.is_some() {
                    path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_else(|| self.shown(&path))
                } else {
                    self.shown(&path)
                };
                self.say(crate::i18n::format_message_id("ui.saved_the_drawing_as_a_picture_path", &[("path", &shown)]));
            }
            Err(e) => self.complain(&crate::i18n::format_message_id("ui.cannot_write_the_picture_error", &[("error", &e.to_string())])),
        }
    }

    /// Exports the original image file at `src` via the native save file dialog (or browser download).
    /// Rotates or flips a picture on the page (issue #273). The turned picture is written as a
    /// new file beside the page and every place the page shows the old one points at it; the
    /// original file stays, so Undo puts the page back on it. A picture given a size of its own
    /// keeps it, with width and height swapped by a quarter turn.
    fn turn_picture(&mut self, src: &str, how: crate::turn::Turn) {
        use ergonote_formats::markdown::Block;
        let Some(ed) = self.editor.as_ref() else {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return;
        };
        if !ed.writable {
            self.complain(crate::tl_id!("ui.this_picture_cannot_be_turned_here"));
            return;
        }
        let dir = ed.dir.clone();
        let (_, _, bytes) = match crate::imagemenu::validate_and_read_image(&dir, src) {
            Ok(read) => read,
            Err(e) => {
                self.complain(&crate::i18n::format_message_id("ui.cannot_read_the_picture_error", &[("error", &e.to_string())]));
                return;
            }
        };
        let (turned, ext) = match crate::turn::turn(&bytes, how) {
            Ok(turned) => turned,
            Err(e) => {
                log::debug!("picture turn failed for {src:?}: {e}");
                self.complain(crate::tl_id!("ui.this_picture_cannot_be_turned_here"));
                return;
            }
        };
        let link = match ergonote_store::assets::attach_bytes(&dir, &crate::turn::turned_name(src, how, ext), &turned) {
            Ok(link) => link,
            Err(e) => {
                self.complain(&crate::i18n::format_message_id("ui.cannot_save_the_pasted_picture_error", &[("error", &e.to_string())]));
                return;
            }
        };
        let Some(ed) = self.editor.as_mut() else { return };
        // The page as it stood is kept, so Undo takes back only the turn.
        Self::remember_step(ed);
        let mut found = false;
        for block in &mut ed.blocks {
            match block {
                Block::Image { src: s, width, height, .. } if s == src => {
                    s.clone_from(&link);
                    if how.swaps_sides() {
                        std::mem::swap(width, height);
                    }
                    found = true;
                }
                Block::Heading { spans, .. } | Block::Paragraph(spans) | Block::Quote(spans) | Block::Item { spans, .. } => {
                    for span in spans.iter_mut().filter(|s| s.image.as_deref() == Some(src)) {
                        span.image = Some(link.clone());
                        found = true;
                    }
                }
                _ => {}
            }
        }
        if !found {
            // Nothing pointed at it after all: the copy is not left lying in the folder.
            let _ = ergonote_store::assets::remove(&dir, link.trim_start_matches("assets/"));
            return;
        }
        if let Some(caret) = ed.caret.as_mut()
            && let Some(block) = ed.blocks.get(caret.block)
        {
            caret.text = pageedit::caret_text(block);
            caret.inline = None;
        }
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
        Self::remember_edit(ed);
        ed.attachments = ergonote_store::assets::list(&dir);
        let inked = self.pad.as_ref().is_some_and(|p| !p.is_empty());
        self.say(match (how.swaps_sides(), inked) {
            (true, false) => crate::tl_id!("ui.rotated_the_picture"),
            (false, false) => crate::tl_id!("ui.flipped_the_picture"),
            // Ink is its own layer over the whole page: it is not turned with the picture.
            (true, true) => crate::tl_id!("ui.rotated_the_picture_ink_on_the_page_stays_where_it_is"),
            (false, true) => crate::tl_id!("ui.flipped_the_picture_ink_on_the_page_stays_where_it_is"),
        });
    }

    fn is_user_image_block(block: &ergonote_formats::markdown::Block) -> Option<&str> {
        match block {
            ergonote_formats::markdown::Block::Image { src, alt, .. }
                if !crate::inkpad::is_ink_line(src, alt) && !crate::inkpad::is_page_ink(src) =>
            {
                Some(src.as_str())
            }
            _ => None,
        }
    }

    fn page_has_image(&self) -> bool {
        self.editor.as_ref().is_some_and(|ed| ed.blocks.iter().any(|b| Self::is_user_image_block(b).is_some()))
    }

    fn active_image_target(&self) -> Option<(usize, String)> {
        let ed = self.editor.as_ref()?;
        if let Some(sel) = self.selected_image_block
            && let Some(block) = ed.blocks.get(sel)
            && let Some(src) = Self::is_user_image_block(block)
        {
            return Some((sel, src.to_string()));
        }
        ed.blocks.iter().enumerate().find_map(|(i, b)| Self::is_user_image_block(b).map(|src| (i, src.to_string())))
    }

    fn replace_picture_asset(&mut self, block_idx: usize, src: &str, edited: &[u8], tag: &str, ext: &str, crop_ratios: Option<(f32, f32)>) -> bool {
        use ergonote_formats::markdown::Block;
        let Some(dir) = self.editor.as_ref().filter(|e| e.writable).map(|e| e.dir.clone()) else {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return false;
        };
        let filename = crate::turn::edited_name(src, tag, ext);
        let link = match ergonote_store::assets::attach_bytes(&dir, &filename, edited) {
            Ok(link) => link,
            Err(e) => {
                self.complain(&crate::i18n::format_message_id("ui.cannot_save_the_pasted_picture_error", &[("error", &e.to_string())]));
                return false;
            }
        };
        let Some(ed) = self.editor.as_mut() else { return false };
        Self::remember_step(ed);
        let mut changed = false;
        if let Some(Block::Image { src: s, width, height, .. }) = ed.blocks.get_mut(block_idx)
            && s == src
        {
            s.clone_from(&link);
            if let Some((rx, ry)) = crop_ratios
                && let (Some(w), Some(h)) = (*width, *height)
            {
                *width = Some((w * rx).max(24.0));
                *height = Some((h * ry).max(24.0));
            }
            changed = true;
        }
        if !changed {
            let _ = ergonote_store::assets::remove(&dir, link.trim_start_matches("assets/"));
            return false;
        }
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
        Self::remember_edit(ed);
        ed.attachments = ergonote_store::assets::list(&dir);
        true
    }

    fn open_crop_picture(&mut self) {
        let Some(ed) = self.editor.as_ref() else {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return;
        };
        if !ed.writable {
            self.complain("Cette image ne peut pas être rognée ici.");
            return;
        }
        let Some((block, src)) = self.active_image_target() else {
            self.say("Aucune image à rogner sur cette page.");
            return;
        };
        let (_, _, bytes) = match crate::imagemenu::validate_and_read_image(&ed.dir, &src) {
            Ok(read) => read,
            Err(e) => {
                self.complain(&crate::i18n::format_message_id("ui.cannot_read_the_picture_error", &[("error", &e.to_string())]));
                return;
            }
        };
        let Ok(dims) = image::load_from_memory(&bytes).map(|im| (im.width(), im.height())) else {
            self.complain("Ce format d'image ne peut pas être rogné ici.");
            return;
        };
        self.dialog = Some(Dialog::CropPicture { block, src, bytes, dims, rect: [0.0, 0.0, 1.0, 1.0] });
    }

    fn apply_crop_picture(&mut self, block: usize, src: &str, bytes: &[u8], rect: [f32; 4]) {
        match crate::turn::crop(bytes, rect) {
            Ok((out, ext, _dims)) => {
                let rx = (rect[2] - rect[0]).abs().clamp(0.05, 1.0);
                let ry = (rect[3] - rect[1]).abs().clamp(0.05, 1.0);
                if self.replace_picture_asset(block, src, &out, "cropped", ext, Some((rx, ry))) {
                    self.say("Image rognée.");
                }
            }
            Err(e) => self.complain(&e),
        }
    }

    fn enhance_picture(&mut self, mode: crate::turn::Enhance) {
        let Some(ed) = self.editor.as_ref() else {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return;
        };
        if !ed.writable {
            self.complain("Cette image ne peut pas être modifiée ici.");
            return;
        }
        let Some((block, src)) = self.active_image_target() else {
            self.say("Aucune image sur cette page.");
            return;
        };
        let (_, _, bytes) = match crate::imagemenu::validate_and_read_image(&ed.dir, &src) {
            Ok(read) => read,
            Err(e) => {
                self.complain(&crate::i18n::format_message_id("ui.cannot_read_the_picture_error", &[("error", &e.to_string())]));
                return;
            }
        };
        match crate::turn::enhance(&bytes, mode) {
            Ok((out, ext)) => {
                if self.replace_picture_asset(block, &src, &out, mode.tag(), ext, None) {
                    self.say(match mode {
                        crate::turn::Enhance::ScanBw => "Scan noir et blanc amélioré.",
                        crate::turn::Enhance::Contrast => "Contraste renforcé.",
                    });
                }
            }
            Err(e) => self.complain(&e),
        }
    }

    fn start_ocr(&mut self, speak: bool) {
        if self.ocr_job.is_some() {
            self.say("Une extraction de texte est déjà en cours.");
            return;
        }
        let Some(ed) = self.editor.as_ref() else {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return;
        };
        let dir = ed.dir.clone();
        let Some((block, src)) = self.active_image_target() else {
            self.say("Aucune image ou PDF à analyser sur cette page.");
            return;
        };
        let (_, _, bytes) = match crate::imagemenu::validate_and_read_image(&dir, &src) {
            Ok(read) => read,
            Err(e) => {
                self.complain(&crate::i18n::format_message_id("ui.cannot_read_the_picture_error", &[("error", &e.to_string())]));
                return;
            }
        };
        let (tx, rx) = std::sync::mpsc::channel();
        self.ocr_job = Some(rx);
        self.say("Extraction intelligente en cours...");
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            let result = crate::ocr::extract_document(&dir, &src, &bytes);
            let _ = tx.send(OcrDone { block, speak, result });
        });
        #[cfg(target_arch = "wasm32")]
        {
            let _ = tx.send(OcrDone { block, speak, result: crate::ocr::extract_document(&dir, &src, &bytes) });
        }
    }

    fn poll_ocr(&mut self, ctx: &Context) {
        let Some(rx) = self.ocr_job.as_ref() else { return };
        match rx.try_recv() {
            Ok(done) => {
                self.ocr_job = None;
                match done.result {
                    Ok(doc) => {
                        if done.speak {
                            self.speak_text_with_speed(&doc.plain_text);
                        } else {
                            let text = doc.plain_text.clone();
                            self.dialog = Some(Dialog::OcrResult { block: done.block, doc, text });
                            self.say("Texte extrait.");
                        }
                    }
                    Err(e) => self.complain(&e),
                }
            }
            Err(TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(80));
            }
            Err(TryRecvError::Disconnected) => {
                self.ocr_job = None;
                self.complain("L'analyse OCR s'est arrêtée de manière inattendue.");
            }
        }
    }

    fn reconstruct_page_from_ocr(&mut self, block_idx: usize, doc: &crate::ocr::OcrDocument, keep_image_bg: bool) {
        use ergonote_formats::markdown::{Block, Span};
        use ergonote_model::container;
        if doc.blocks.is_empty() {
            self.say("Aucune zone de texte à recréer.");
            return;
        }
        let prev_locked = self.lock_images;
        let Some(ed) = self.editor.as_mut() else { return };
        Self::remember_step(ed);
        if let Some(step) = ed.history.here_mut() {
            step.lock_images = Some(prev_locked);
        }

        let explicit_img_w = ed.blocks.get(block_idx).and_then(|b| match b {
            Block::Image { width: Some(w), .. } if w.is_finite() => Some(*w),
            _ => None,
        });
        let (origin_x, origin_y, box_w) = container::place(&ed.boxes, block_idx)
            .and_then(|(owner, _)| ed.boxes.get(owner))
            .map(|c| (c.x, c.y, c.width.max(420.0)))
            .unwrap_or((0.0, 0.0, 700.0));
        let target_w = explicit_img_w.unwrap_or(box_w).max(420.0);
        let target_h = (target_w * doc.aspect_ratio).max(240.0);

        let mut cols_changed = false;
        let mut just_changed = false;
        if keep_image_bg
            && block_idx < ed.blocks.len()
            && let Some(orig_src) = Self::is_user_image_block(&ed.blocks[block_idx]).map(str::to_string)
        {
            let dir = ed.dir.clone();
            let regions: Vec<(f32, f32, f32, f32)> = doc.blocks.iter().map(|b| (b.x, b.y, b.width, b.height)).collect();
            if let Ok((_, _, orig_bytes)) = crate::imagemenu::validate_and_read_image(&dir, &orig_src)
                && let Ok(cleaned) = crate::turn::erase_text_regions(&orig_bytes, &regions)
                && let Ok(bg_link) = ergonote_store::assets::attach_bytes(&dir, &crate::turn::edited_name(&orig_src, "bg", "png"), &cleaned)
            {
                if let Some(Block::Image { src, width, height, .. }) = ed.blocks.get_mut(block_idx) {
                    *src = bg_link;
                    *width = Some(target_w);
                    *height = Some(target_h);
                }
                ed.attachments = ergonote_store::assets::list(&dir);
            }
            if ed.boxes.is_empty() && !ed.blocks.is_empty() {
                ed.boxes.push(ergonote_model::Container::new(0.0, 0.0, target_w, ed.blocks.len()));
            }
        } else if !keep_image_bg && block_idx < ed.blocks.len() && Self::is_user_image_block(&ed.blocks[block_idx]).is_some() {
            ed.blocks.remove(block_idx);
            cols_changed |= ed.columns.remove_blocks(block_idx..block_idx + 1);
            just_changed |= ed.justified.remove_blocks(block_idx..block_idx + 1);
            if let Some((owner, _)) = container::place(&ed.boxes, block_idx) {
                if ed.boxes[owner].blocks <= 1 {
                    ed.boxes.remove(owner);
                } else {
                    ed.boxes[owner].blocks -= 1;
                }
            }
        } else if ed.boxes.is_empty() && !ed.blocks.is_empty() {
            ed.boxes.push(ergonote_model::Container::new(0.0, 0.0, target_w, ed.blocks.len()));
        }

        let bold_leadin_spans = |text: &str| -> Vec<Span> {
            if let Some(colon) = text.find(" : ")
                && text[..colon].chars().count() <= 55
            {
                let cut = colon + 2;
                let mut spans = vec![Span { text: text[..cut].to_string(), bold: true, ..Default::default() }];
                if !text[cut..].is_empty() {
                    spans.push(Span { text: text[cut..].to_string(), ..Default::default() });
                }
                return spans;
            }
            vec![Span { text: text.to_string(), ..Default::default() }]
        };
        fn parse_ordered_prefix(line: &str) -> Option<(usize, &str)> {
            let (num_str, rest) = line.split_once(". ")?;
            if num_str.is_empty() || num_str.len() > 2 || !num_str.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let n = num_str.parse::<usize>().ok()?;
            (n > 0).then_some((n, rest))
        }

        let first_new_block = ed.blocks.len();
        let single_container = !keep_image_bg && doc.is_single_column();
        let mut total_single_count = 0_usize;
        let mut placed_rects: Vec<(f32, f32, f32, f32)> = Vec::new();
        for blk in &doc.blocks {
            if blk.lines.is_empty() {
                continue;
            }
            let is_heading = blk.font_scale >= 1.32 && blk.lines.len() <= 2;
            let mut new_blocks = Vec::with_capacity(blk.lines.len());
            if is_heading {
                let level = if blk.font_scale >= 1.7 { 1 } else { 2 };
                new_blocks.push(Block::Heading { level, spans: vec![Span { text: blk.lines.join(" "), ..Default::default() }] });
            } else {
                for line in &blk.lines {
                    if let Some(rest) = line.strip_prefix("  - ") {
                        new_blocks.push(Block::Item { ordered: false, number: 0, task: None, depth: 1, spans: bold_leadin_spans(rest) });
                    } else if let Some(rest) = line.strip_prefix("- ") {
                        new_blocks.push(Block::Item { ordered: false, number: 0, task: None, depth: 0, spans: bold_leadin_spans(rest) });
                    } else if let Some((number, rest)) = parse_ordered_prefix(line) {
                        new_blocks.push(Block::Item { ordered: true, number, task: None, depth: 0, spans: bold_leadin_spans(rest) });
                    } else {
                        new_blocks.push(Block::Paragraph(bold_leadin_spans(line)));
                    }
                }
            }
            let count = new_blocks.len();
            let start_at = ed.blocks.len();
            ed.blocks.extend(new_blocks);
            for offset in 0..count {
                cols_changed |= ed.columns.insert_block(start_at + offset);
                just_changed |= ed.justified.insert_block(start_at + offset);
            }
            if single_container {
                total_single_count += count;
            } else {
                // Align overlaid boxes with the background image rectangle (containers have 6px pad and 14px handle).
                let pad_off = if keep_image_bg { 6.0 } else { 0.0 };
                let handle_off = if keep_image_bg { 14.0 } else { 0.0 };
                let box_x = if blk.x.is_finite() { (origin_x + blk.x * target_w - pad_off).max(0.0) } else { origin_x.max(0.0) };
                let mut box_y = if blk.y.is_finite() { (origin_y + blk.y * target_h - handle_off).max(0.0) } else { origin_y.max(0.0) };
                let raw_w = if blk.width.is_finite() { blk.width * target_w + if keep_image_bg { 18.0 } else { 0.0 } } else { 240.0 };
                let box_w = raw_w.clamp(140.0_f32.min(target_w), target_w.max(140.0));
                let est_h = (blk.height.max(0.02) * target_h).max(count as f32 * 20.0);
                for &(px, py, pw, ph) in &placed_rects {
                    if box_x < px + pw && box_x + box_w > px && box_y >= py && box_y < py + ph {
                        box_y = py + ph + 4.0;
                    }
                }
                placed_rects.push((box_x, box_y, box_w, est_h));
                ed.boxes.push(ergonote_model::Container::new(box_x, box_y, box_w, count));
            }
        }
        if single_container && total_single_count > 0 {
            ed.boxes.push(ergonote_model::Container::new(origin_x.max(0.0), origin_y.max(0.0), target_w, total_single_count));
        }

        ed.arranged = true;
        pageedit::renumber(&mut ed.blocks);
        ed.rebuild_body();
        if let Some(first) = ed.blocks.get(first_new_block) {
            let caret_text = pageedit::caret_text(first);
            ed.caret = Some(pageedit::Caret {
                block: first_new_block,
                text: caret_text,
                focus: true,
                selection: (0, 0),
                place: Some(0),
                cell: None,
                span: None,
                inline: None,
            });
        }
        Self::mark_dirty_from_blocks(ed);
        let new_locked = if keep_image_bg { true } else { self.lock_images };
        if keep_image_bg && !self.lock_images {
            self.lock_images = true;
            let _ = self.session.settings.set("lock_images", "true");
        }
        Self::remember_edit(ed);
        if let Some(step) = ed.history.here_mut() {
            step.lock_images = Some(new_locked);
        }
        self.selected_image_block = None;
        self.last_selected_image = None;
        self.save_block_marks((cols_changed, just_changed));
        self.say(format!("{} zone(s) modifiable(s) créée(s) ({}).", doc.blocks.len(), doc.source_label));
    }

    fn insert_ocr_text_below(&mut self, block_idx: usize, text: &str) {
        use ergonote_formats::markdown::{Block, Span};
        let Some(ed) = self.editor.as_mut() else { return };
        let lines: Vec<Block> = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(|l| Block::Paragraph(vec![Span { text: l.to_string(), ..Default::default() }]))
            .collect();
        if lines.is_empty() {
            return;
        }
        Self::remember_step(ed);
        let at = (block_idx + 1).min(ed.blocks.len());
        let count = lines.len();
        for (offset, b) in lines.into_iter().enumerate() {
            ed.blocks.insert(at + offset, b);
        }
        let marks = ed.blocks_inserted(at, count);
        pageedit::renumber(&mut ed.blocks);
        ed.rebuild_body();
        let caret_text = pageedit::text_for_caret(&ed.blocks, at);
        ed.caret = Some(pageedit::Caret {
            block: at,
            text: caret_text,
            focus: true,
            selection: (0, 0),
            place: Some(0),
            cell: None,
            span: None,
            inline: None,
        });
        Self::mark_dirty_from_blocks(ed);
        Self::remember_edit(ed);
        self.save_block_marks(marks);
        self.say("Texte inséré sous l'image.");
    }

    fn delete_selected_picture(&mut self) {
        use ergonote_formats::markdown::Block;
        let Some((at, _src)) = self.active_image_target() else {
            self.say("Aucune image sélectionnée sur cette page.");
            return;
        };
        let Some(ed) = self.editor.as_mut() else { return };
        if Self::is_user_image_block(&ed.blocks[at]).is_none() {
            return;
        }
        Self::remember_step(ed);
        ed.caret = None;
        ed.blocks.remove(at);
        ed.count_removed_from_boxes(at);
        let cols = ed.columns.remove_blocks(at..at + 1);
        let justified = ed.justified.remove_blocks(at..at + 1);
        if ed.blocks.is_empty() {
            ed.blocks.push(Block::Paragraph(Vec::new()));
            ed.count_inserted_in_boxes(0, 1);
        }
        pageedit::renumber(&mut ed.blocks);
        ed.rebuild_body();
        let to = at.min(ed.blocks.len() - 1);
        let text = pageedit::text_for_caret(&ed.blocks, to);
        ed.caret = Some(pageedit::Caret { block: to, text, focus: true, selection: (0, 0), place: Some(0), cell: None, span: None, inline: None });
        self.selected_image_block = None;
        self.last_selected_image = None;
        Self::mark_dirty_from_blocks(ed);
        Self::remember_edit(ed);
        self.save_block_marks((cols, justified));
        self.say("Image supprimée.");
    }

    fn save_picture_as(&mut self, src: &str) {
        let Some(page_dir) = self.editor.as_ref().map(|e| e.dir.clone()) else {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return;
        };
        let (_source_path, default_name, bytes) = match crate::imagemenu::validate_and_read_image(&page_dir, src) {
            Ok(res) => res,
            Err(e) => {
                self.complain(&crate::i18n::format_message_id("ui.cannot_read_the_picture_error", &[("error", &e.to_string())]));
                return;
            }
        };

        if let Some(downloads) = self.file_downloads.as_mut() {
            let mime = match std::path::Path::new(&default_name).extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
                Some("png") => "image/png",
                Some("jpg" | "jpeg") => "image/jpeg",
                Some("gif") => "image/gif",
                Some("webp") => "image/webp",
                Some("bmp") => "image/bmp",
                Some("svg") => "image/svg+xml",
                _ => "application/octet-stream",
            };
            downloads.push(FileDownload { name: default_name.clone(), mime, bytes });
            self.say(crate::i18n::format_message_id("ui.saved_picture_as_path", &[("path", &default_name)]));
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let ext = std::path::Path::new(&default_name).extension().and_then(|e| e.to_str()).unwrap_or("");
            let mut picker = rfd::FileDialog::new().set_title(crate::tl_id!("ui.save_picture_as")).set_file_name(&default_name);
            if !ext.is_empty() && !["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "jxl"].contains(&ext) {
                picker = picker.add_filter(ext, &[ext]);
            }
            picker = picker.add_filter(crate::tl_id!("ui.pictures"), &["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "jxl"]);

            let Some(destination) = picker.save_file() else {
                return;
            };

            match std::fs::write(&destination, &bytes) {
                Ok(()) => {
                    let shown = self.shown(&destination);
                    self.say(crate::i18n::format_message_id("ui.saved_picture_as_path", &[("path", &shown)]));
                }
                Err(e) => {
                    self.complain(&crate::i18n::format_message_id("ui.cannot_write_the_picture_error", &[("error", &e.to_string())]));
                }
            }
        }
    }

    /// Puts marks around the selection in the block the caret is in.
    fn apply_mark(&mut self, mark: &str) {
        if self.editor.as_ref().and_then(|ed| ed.across).is_some_and(|selection| !selection.inside_one_block()) {
            let format = match mark {
                "bold" => pageedit::InlineFormat::Bold,
                "italic" => pageedit::InlineFormat::Italic,
                "strike" => pageedit::InlineFormat::Strike,
                "underline" => pageedit::InlineFormat::Underline,
                "subscript" => pageedit::InlineFormat::Subscript,
                "superscript" => pageedit::InlineFormat::Superscript,
                "code" => pageedit::InlineFormat::Code,
                _ => return,
            };
            self.apply_across_format(format);
            return;
        }
        if let Some(ed) = self.editor.as_mut() {
            Self::place_caret_for_format(ed);
        }
        if matches!(mark, "underline" | "subscript" | "superscript") {
            self.apply_new_mark(mark);
            return;
        }
        let collapsed_text_caret =
            self.editor.as_ref().and_then(|ed| ed.caret.as_ref()).is_some_and(|caret| caret.cell.is_none() && caret.selection.0 == caret.selection.1);
        if collapsed_text_caret && matches!(mark, "bold" | "italic" | "strike") {
            self.apply_new_mark(mark);
            return;
        }
        let partial = self.editor.as_ref().and_then(|ed| ed.caret.as_ref()).is_some_and(|caret| {
            let (lo, hi) = (caret.selection.0.min(caret.selection.1), caret.selection.0.max(caret.selection.1));
            lo < hi && (lo != 0 || hi != caret.text.chars().count() || (caret.cell.is_some() && caret.span.is_none()))
        });
        if partial {
            let format = match mark {
                "bold" => Some(pageedit::InlineFormat::Bold),
                "italic" => Some(pageedit::InlineFormat::Italic),
                "strike" => Some(pageedit::InlineFormat::Strike),
                "code" => Some(pageedit::InlineFormat::Code),
                _ => None,
            };
            if let Some(format) = format {
                self.apply_partial_inline_format(format);
                return;
            }
        }
        let Some(ed) = self.editor.as_mut() else { return };
        let Some(caret) = ed.caret.as_mut() else {
            self.status = crate::tl_id!("ui.click_in_the_page_first_then_use_the_ribbon").into();
            self.status_is_error = false;
            return;
        };
        let marks = match mark {
            "bold" => "**",
            "italic" => "*",
            "strike" => "~~",
            _ => "`",
        };
        let len = caret.text.chars().count();
        let (lo, hi) = (caret.selection.0.min(caret.selection.1), caret.selection.0.max(caret.selection.1));
        // All of the line's words: marked on the spans, so words that are partly marked already come out
        // right. A part of them is wrapped in the text, where the caret's own selection lives.
        if len > 0 && lo == 0 && hi == len && pageedit::toggle_mark(&mut ed.blocks, caret.block..caret.block + 1, mark) {
            if let Some(b) = ed.blocks.get(caret.block) {
                caret.text = pageedit::caret_text(b);
                caret.inline = None;
                caret.selection = (0, caret.text.chars().count());
            }
            ed.rebuild_body();
            Self::mark_dirty_from_blocks(ed);
            return;
        }
        let (text, _) = pageedit::wrap_selection(&caret.text, caret.selection, marks);
        caret.text = text;
        if let Some(b) = ed.blocks.get_mut(caret.block) {
            caret.write(b);
        }
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
    }

    /// When a collapsed caret sits right before closing delimiters for an already-active mark
    /// (common on smartphones without arrow keys), step past them and report any sibling marks
    /// that had to close alongside the target delimiter.
    fn step_out_of_closing_delimiters(
        text: &str,
        lo: usize,
        mark: &str,
        span: &ergonote_formats::markdown::Span,
    ) -> Option<(usize, (bool, bool, bool, bool))> {
        let active = match mark {
            "bold" => span.bold,
            "italic" => span.italic,
            "strike" => span.strike,
            "underline" => span.underline,
            _ => false,
        };
        if !active {
            return None;
        }
        let byte_at = text.char_indices().nth(lo).map_or(text.len(), |(b, _)| b);
        let mut rest = text.get(byte_at..)?;
        let mut consumed_chars = 0usize;
        let mut has_bold = span.bold;
        let mut has_italic = span.italic;
        let mut has_strike = span.strike;
        let mut has_underline = span.underline;
        let mut target_exited = false;
        while !target_exited {
            if has_bold && has_italic && rest.starts_with("***") {
                rest = rest.get(3..)?;
                consumed_chars += 3;
                has_bold = false;
                has_italic = false;
                target_exited = matches!(mark, "bold" | "italic");
            } else if has_bold && rest.starts_with("**") {
                rest = rest.get(2..)?;
                consumed_chars += 2;
                has_bold = false;
                target_exited = mark == "bold";
            } else if has_italic && rest.starts_with('*') {
                rest = rest.get(1..)?;
                consumed_chars += 1;
                has_italic = false;
                target_exited = mark == "italic";
            } else if has_strike && rest.starts_with("~~") {
                rest = rest.get(2..)?;
                consumed_chars += 2;
                has_strike = false;
                target_exited = mark == "strike";
            } else if has_underline && rest.starts_with("</u>") {
                rest = rest.get(4..)?;
                consumed_chars += 4;
                has_underline = false;
                target_exited = mark == "underline";
            } else {
                break;
            }
        }
        if !target_exited {
            return None;
        }
        let keep_bold = mark != "bold" && span.bold && !has_bold;
        let keep_italic = mark != "italic" && span.italic && !has_italic;
        let keep_strike = mark != "strike" && span.strike && !has_strike;
        let keep_underline = mark != "underline" && span.underline && !has_underline;
        Some((lo + consumed_chars, (keep_bold, keep_italic, keep_strike, keep_underline)))
    }

    fn apply_new_mark(&mut self, mark: &str) {
        let Some(ed) = self.editor.as_mut() else { return };
        Self::place_caret_for_format(ed);
        let Some(caret) = ed.caret.as_ref() else {
            self.status = crate::tl_id!("ui.click_in_the_page_first_then_use_the_ribbon").into();
            self.status_is_error = false;
            return;
        };
        let len = caret.text.chars().count();
        let (lo, hi) = (caret.selection.0.min(caret.selection.1), caret.selection.0.max(caret.selection.1));
        if ed.writable
            && ed.across.is_none()
            && caret.cell.is_none()
            && lo == hi
            && matches!(
                ed.blocks.get(caret.block),
                Some(
                    ergonote_formats::Block::Heading { .. }
                        | ergonote_formats::Block::Paragraph(_)
                        | ergonote_formats::Block::Quote(_)
                        | ergonote_formats::Block::Item { .. }
                )
            )
        {
            let mut marker = "ErgoNoteCaretProbe".to_string();
            while caret.text.contains(&marker) {
                marker.push('X');
            }
            let (probe, _) = pageedit::insert_at_caret(&caret.text, (lo, lo), &marker);
            let inherited = ed
                .blocks
                .get(caret.block)
                .map(|block| caret.parse(block, &probe))
                .unwrap_or_default()
                .into_iter()
                .find(|span| span.text.contains(&marker));
            if let Some(span) = inherited.as_ref()
                && !span.code
                && !span.math
                && span.image.is_none()
                && let Some((new_at, (keep_bold, keep_italic, keep_strike, keep_underline))) =
                    Self::step_out_of_closing_delimiters(&caret.text, lo, mark, span)
            {
                let block = caret.block;
                if let Some(caret) = ed.caret.as_mut() {
                    caret.selection = (new_at, new_at);
                    caret.focus = true;
                    caret.place = Some(new_at);
                }
                ed.restore_format_selection = Some((block, new_at, new_at));
                ed.pending_format = (keep_bold || keep_italic || keep_strike || keep_underline).then_some(PendingFormat {
                    block,
                    at: new_at,
                    bold: keep_bold,
                    italic: keep_italic,
                    strike: keep_strike,
                    underline: keep_underline,
                    ..Default::default()
                });
                self.status.clear();
                self.status_is_error = false;
                return;
            }
            let unsafe_context = inherited.is_none_or(|span| {
                span.code
                    || span.math
                    || span.image.is_some()
                    || (mark == "bold" && span.bold)
                    || (mark == "italic" && span.italic)
                    || (mark == "strike" && span.strike)
                    || (mark == "underline" && span.underline)
                    || (matches!(mark, "subscript" | "superscript") && span.script.is_some())
            });
            if unsafe_context {
                self.status = crate::tl_id!("ui.inline_mark_whole_line").into();
                self.status_is_error = false;
                return;
            }
            let pending = ed.pending_format.get_or_insert_with(|| PendingFormat { block: caret.block, at: lo, ..Default::default() });
            if pending.block != caret.block || pending.at != lo {
                *pending = PendingFormat { block: caret.block, at: lo, ..Default::default() };
            }
            match mark {
                "bold" => pending.bold = !pending.bold,
                "italic" => pending.italic = !pending.italic,
                "strike" => pending.strike = !pending.strike,
                "underline" => pending.underline = !pending.underline,
                "subscript" => {
                    let script = ergonote_formats::markdown::Script::Subscript;
                    pending.script = (pending.script != Some(script)).then_some(script);
                }
                "superscript" => {
                    let script = ergonote_formats::markdown::Script::Superscript;
                    pending.script = (pending.script != Some(script)).then_some(script);
                }
                _ => {}
            }
            if let Some(caret) = ed.caret.as_mut() {
                caret.focus = true;
                caret.place = Some(lo);
            }
            self.status.clear();
            self.status_is_error = false;
            return;
        }
        if ed.writable && ed.across.is_none() && caret.span.is_none() && lo < hi && (lo != 0 || hi != len || caret.cell.is_some()) {
            let format = match mark {
                "underline" => pageedit::InlineFormat::Underline,
                "subscript" => pageedit::InlineFormat::Subscript,
                "superscript" => pageedit::InlineFormat::Superscript,
                _ => return,
            };
            self.apply_partial_inline_format(format);
            return;
        }
        if !ed.writable || ed.across.is_some() || caret.cell.is_some() || len == 0 || lo != 0 || hi != len {
            self.status = crate::tl_id!("ui.inline_mark_whole_line").into();
            self.status_is_error = false;
            return;
        }
        let block = caret.block;
        Self::remember_step(ed);
        if !pageedit::toggle_mark(&mut ed.blocks, block..block + 1, mark) {
            self.status = crate::tl_id!("ui.inline_mark_whole_line").into();
            self.status_is_error = false;
            return;
        }
        ed.pending_format = None;
        if let (Some(caret), Some(block)) = (ed.caret.as_mut(), ed.blocks.get(block)) {
            caret.text = pageedit::caret_text(block);
            caret.inline = None;
            caret.selection = (0, caret.text.chars().count());
            caret.focus = true;
            ed.restore_format_selection = Some((caret.block, caret.selection.0, caret.selection.1));
        }
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
        let step = crate::history::Step::new(ed.body.clone(), ed.blocks.clone(), ed.boxes.clone(), ed.caret.clone(), crate::history::Kind::Edit);
        ed.history.record(step);
        self.status.clear();
        self.status_is_error = false;
    }

    /// Format Painter: picks up the formatting at the caret, or of the words picked, to give
    /// to the next words picked (issue #165). A second click puts the painter down again; a
    /// double-click (`sticky`) keeps it painting until Escape.
    fn pick_up_formatting(&mut self, sticky: bool) {
        if let Some(held) = self.painter.as_mut() {
            if sticky {
                held.sticky = true;
                self.say(crate::tl_id!("ui.format_painter_keeps_painting_until_escape"));
            } else {
                self.painter = None;
                self.say(crate::tl_id!("ui.format_painter_put_down"));
            }
            return;
        }
        let Some(ed) = self.editor.as_ref().filter(|ed| ed.writable) else {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return;
        };
        let picked = ed.caret.as_ref().and_then(|caret| Some((caret.block, caret.selection, caret.style(ed.blocks.get(caret.block)?)?)));
        let Some((block, selection, style)) = picked else {
            self.say(crate::tl_id!("ui.click_in_the_words_whose_formatting_to_copy"));
            return;
        };
        self.painter = Some(FormatPaint { style, from: (block, selection), sticky });
        self.say(crate::tl_id!("ui.pick_the_words_to_give_that_formatting"));
    }

    /// Puts the Format Painter's formatting on words just picked with the pointer: on the
    /// release that ends the pick, as in Office. Escape puts the painter down. Called once a
    /// frame after the page is drawn.
    /// Escape puts the laser pointer down (issue #289): it is for a moment of pointing, and
    /// the caret comes back afterwards.
    /// The Eyedropper (issue #290): a colour taken from the page goes to the tool it was
    /// started from, and Escape puts it down with that tool in hand, unchanged.
    fn eyedropper(&mut self, ctx: &Context) {
        let ribbon::Pointer::Eyedropper(tool) = self.pointer else { return };
        if let Some((i, colour)) = crate::inkpad::take_eyedropped(ctx)
            && let Some(nib) = self.rack.get(i).copied()
        {
            self.edit_rack(ribbon::RackEdit::Set(i, ergonote_ink::rack::Nib::new(nib.tool, colour, nib.tenths)));
            self.say(crate::i18n::format_message_id("ui.the_pen_takes_colour", &[("colour", &colour.hex())]));
            return;
        }
        if self.dialog.is_none() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.nib = tool.min(self.rack.len().saturating_sub(1));
            self.pointer = ribbon::Pointer::Draw;
            self.say(crate::tl_id!("ui.eyedropper_put_down_the_pen_is_as_it_was"));
        }
    }

    fn laser_keys(&mut self, ctx: &Context) {
        if self.pointer == ribbon::Pointer::Laser && self.dialog.is_none() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.pointer = ribbon::Pointer::Write;
            crate::inkpad::clear_laser(ctx);
            self.say(crate::tl_id!("ui.laser_pointer_put_down"));
        }
    }

    fn paint_formatting(&mut self, ctx: &Context) {
        if self.painter.is_none() {
            return;
        }
        if self.dialog.is_none() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.painter = None;
            self.say(crate::tl_id!("ui.format_painter_put_down"));
            return;
        }
        if !ctx.input(|i| i.pointer.primary_released()) {
            return;
        }
        let Some(held) = self.painter.clone() else { return };
        let Some(ed) = self.editor.as_mut() else { return };
        let Some(caret) = ed.caret.as_ref() else { return };
        let (lo, hi) = (caret.selection.0.min(caret.selection.1), caret.selection.0.max(caret.selection.1));
        // A click without a pick, or the words it was picked up from: nothing to paint yet.
        if lo == hi || (caret.block, caret.selection) == held.from {
            return;
        }
        let Some(block) = ed.blocks.get(caret.block) else { return };
        let Some(caret) = ed.caret.as_mut() else { return };
        let Some((text, selection)) = caret.paint(block, &held.style) else {
            self.say(crate::tl_id!("ui.inline_mark_whole_line"));
            return;
        };
        if text != caret.text {
            Self::remember_step(ed);
            let Some(caret) = ed.caret.as_mut() else { return };
            caret.text = text;
            caret.selection = selection;
            caret.place = Some(selection.0);
            caret.focus = true;
            if let Some(block) = ed.blocks.get_mut(caret.block) {
                caret.write(block);
            }
            ed.pending_format = None;
            ed.rebuild_body();
            Self::mark_dirty_from_blocks(ed);
            Self::remember_edit(ed);
        }
        if !held.sticky {
            self.painter = None;
        }
    }

    fn apply_partial_inline_format(&mut self, format: pageedit::InlineFormat) {
        let Some(ed) = self.editor.as_mut() else { return };
        let Some(caret) = ed.caret.as_ref() else { return };
        let valid_target = match ed.blocks.get(caret.block) {
            Some(
                ergonote_formats::Block::Heading { .. }
                | ergonote_formats::Block::Paragraph(_)
                | ergonote_formats::Block::Quote(_)
                | ergonote_formats::Block::Item { .. },
            ) => caret.cell.is_none(),
            Some(ergonote_formats::Block::Table { .. }) => caret.cell.is_some() && caret.span.is_none(),
            _ => false,
        };
        if !ed.writable || ed.across.is_some() || !valid_target {
            self.status = crate::tl_id!("ui.inline_mark_whole_line").into();
            self.status_is_error = false;
            return;
        }
        let Some(caret) = ed.caret.as_mut() else { return };
        let Some(block) = ed.blocks.get(caret.block) else { return };
        let Some((text, selection)) = caret.format(block, format) else {
            self.status = crate::tl_id!("ui.inline_mark_whole_line").into();
            self.status_is_error = false;
            return;
        };
        if text == caret.text {
            return;
        }
        Self::remember_step(ed);
        let Some(caret) = ed.caret.as_mut() else { return };
        caret.text = text;
        caret.selection = selection;
        caret.focus = true;
        ed.restore_format_selection = Some((caret.block, selection.0, selection.1));
        if let Some(block) = ed.blocks.get_mut(caret.block) {
            caret.write(block);
        }
        ed.pending_format = None;
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
        let step = crate::history::Step::new(ed.body.clone(), ed.blocks.clone(), ed.boxes.clone(), ed.caret.clone(), crate::history::Kind::Edit);
        ed.history.record(step);
        self.status.clear();
        self.status_is_error = false;
    }

    /// An inline format over a selection that runs across blocks: every covered block's
    /// covered words take it, and the selection is kept with its ends remapped to the new
    /// text, so the highlight, Copy and Delete still cover the same words.
    fn apply_across_format(&mut self, format: pageedit::InlineFormat) {
        let Some(ed) = self.editor.as_mut() else { return };
        let Some(open) = ed.across.filter(|a| !a.inside_one_block()) else { return };
        if !ed.writable {
            self.status = crate::tl_id!("ui.inline_mark_whole_line").into();
            self.status_is_error = false;
            return;
        }
        Self::remember_step(ed);
        match pageedit::format_across(&mut ed.blocks, open, format) {
            Some(remapped) => {
                ed.across = Some(remapped);
                ed.pending_format = None;
                ed.rebuild_body();
                Self::mark_dirty_from_blocks(ed);
                // The caret's block may be one of the formatted ones: its editing text has
                // to say what the block says now, or the next keystroke writes the old words
                // back over the new formatting.
                if let Some(at) = ed.caret.as_ref().filter(|c| c.cell.is_none()).map(|c| c.block)
                    && let Some(block) = ed.blocks.get(at)
                    && matches!(
                        block,
                        ergonote_formats::Block::Heading { .. }
                            | ergonote_formats::Block::Paragraph(_)
                            | ergonote_formats::Block::Quote(_)
                            | ergonote_formats::Block::Item { .. }
                    )
                    && let Some(caret) = ed.caret.as_mut()
                {
                    caret.text = pageedit::caret_text(block);
                    caret.inline = None;
                    caret.selection = (0, caret.text.chars().count());
                }
                let step =
                    crate::history::Step::new(ed.body.clone(), ed.blocks.clone(), ed.boxes.clone(), ed.caret.clone(), crate::history::Kind::Edit);
                ed.history.record(step);
                self.status.clear();
                self.status_is_error = false;
            }
            None => {
                self.status = crate::tl_id!("ui.inline_mark_whole_line").into();
                self.status_is_error = false;
            }
        }
    }

    /// With no caret in the page, a font or size choice goes to the end of the last writable
    /// block, so it applies to the next text typed there, as OneNote does.
    fn place_caret_for_format(ed: &mut Editor) {
        if ed.caret.is_some() {
            return;
        }
        let Some(index) = ed.blocks.iter().rposition(pageedit::is_writable) else { return };
        let text = pageedit::caret_text(&ed.blocks[index]);
        let end = text.chars().count();
        ed.caret =
            Some(pageedit::Caret { block: index, text, focus: true, selection: (end, end), place: Some(end), cell: None, span: None, inline: None });
    }

    fn apply_font_family(&mut self, family: &str) {
        if !ergonote_formats::markdown::valid_font_family(family) {
            log::warn!("ribbon requested an invalid font family");
            return;
        }
        if self.editor.as_ref().and_then(|ed| ed.across).is_some_and(|selection| !selection.inside_one_block()) {
            self.apply_across_format(pageedit::InlineFormat::Family(family.to_owned()));
            return;
        }
        let Some(ed) = self.editor.as_mut() else { return };
        Self::place_caret_for_format(ed);
        let Some(caret) = ed.caret.as_ref() else {
            self.status = crate::tl_id!("ui.click_in_the_page_first_then_use_the_ribbon").into();
            self.status_is_error = false;
            return;
        };
        let len = caret.text.chars().count();
        let (lo, hi) = (caret.selection.0.min(caret.selection.1), caret.selection.0.max(caret.selection.1));
        if ed.writable
            && ed.across.is_none()
            && caret.cell.is_none()
            && lo == hi
            && matches!(
                ed.blocks.get(caret.block),
                Some(
                    ergonote_formats::Block::Heading { .. }
                        | ergonote_formats::Block::Paragraph(_)
                        | ergonote_formats::Block::Quote(_)
                        | ergonote_formats::Block::Item { .. }
                )
            )
        {
            let pending = ed.pending_format.get_or_insert_with(|| PendingFormat { block: caret.block, at: lo, ..Default::default() });
            if pending.block != caret.block || pending.at != lo {
                *pending = PendingFormat { block: caret.block, at: lo, ..Default::default() };
            }
            pending.family = Some(family.to_owned());
            if let Some(caret) = ed.caret.as_mut() {
                caret.focus = true;
                caret.place = Some(lo);
            }
            self.status.clear();
            self.status_is_error = false;
            return;
        }
        if ed.writable && ed.across.is_none() && caret.span.is_none() && lo < hi && (lo != 0 || hi != len || caret.cell.is_some()) {
            self.apply_partial_inline_format(pageedit::InlineFormat::Family(family.to_owned()));
            return;
        }
        if !ed.writable || ed.across.is_some() || caret.cell.is_some() || len == 0 || lo != 0 || hi != len {
            self.status = crate::tl_id!("ui.inline_mark_whole_line").into();
            self.status_is_error = false;
            return;
        }
        let block = caret.block;
        Self::remember_step(ed);
        let spans = match ed.blocks.get_mut(block) {
            Some(
                ergonote_formats::Block::Heading { spans, .. }
                | ergonote_formats::Block::Paragraph(spans)
                | ergonote_formats::Block::Quote(spans)
                | ergonote_formats::Block::Item { spans, .. },
            ) => spans,
            _ => {
                self.status = crate::tl_id!("ui.inline_mark_whole_line").into();
                self.status_is_error = false;
                return;
            }
        };
        if !pageedit::format_span_range(spans, 0..usize::MAX, pageedit::InlineFormat::Family(family.to_owned())) {
            return;
        }
        ed.pending_format = None;
        if let (Some(caret), Some(block)) = (ed.caret.as_mut(), ed.blocks.get(block)) {
            caret.text = pageedit::caret_text(block);
            caret.inline = None;
            caret.selection = (0, caret.text.chars().count());
            caret.focus = true;
            ed.restore_format_selection = Some((caret.block, caret.selection.0, caret.selection.1));
        }
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
        let step = crate::history::Step::new(ed.body.clone(), ed.blocks.clone(), ed.boxes.clone(), ed.caret.clone(), crate::history::Kind::Edit);
        ed.history.record(step);
        self.status.clear();
        self.status_is_error = false;
    }

    fn apply_font_size(&mut self, size: u8) {
        if !ribbon::FONT_SIZES.contains(&size) {
            log::warn!("ribbon requested unsupported font size {size} pt");
            return;
        }
        if self.editor.as_ref().and_then(|ed| ed.across).is_some_and(|selection| !selection.inside_one_block()) {
            self.apply_across_format(pageedit::InlineFormat::Size(size));
            return;
        }
        let Some(ed) = self.editor.as_mut() else { return };
        Self::place_caret_for_format(ed);
        let Some(caret) = ed.caret.as_ref() else {
            self.status = crate::tl_id!("ui.click_in_the_page_first_then_use_the_ribbon").into();
            self.status_is_error = false;
            return;
        };
        if caret.selection.0 == caret.selection.1 {
            if caret.cell.is_some()
                || !matches!(
                    ed.blocks.get(caret.block),
                    Some(
                        ergonote_formats::Block::Heading { .. }
                            | ergonote_formats::Block::Paragraph(_)
                            | ergonote_formats::Block::Quote(_)
                            | ergonote_formats::Block::Item { .. }
                    )
                )
            {
                self.status = crate::tl_id!("ui.select_text_before_changing_font_size").into();
                self.status_is_error = false;
                return;
            }
            let pending = ed.pending_format.get_or_insert_with(|| PendingFormat { block: caret.block, at: caret.selection.0, ..Default::default() });
            if pending.block != caret.block || pending.at != caret.selection.0 {
                *pending = PendingFormat { block: caret.block, at: caret.selection.0, ..Default::default() };
            }
            pending.size = Some(size);
            if let Some(caret) = ed.caret.as_mut() {
                caret.focus = true;
                caret.place = Some(caret.selection.0);
            }
            self.status.clear();
            self.status_is_error = false;
            return;
        }
        let len = caret.text.chars().count();
        let (lo, hi) = (caret.selection.0.min(caret.selection.1), caret.selection.0.max(caret.selection.1));
        if caret.span.is_none() && lo < hi && (lo != 0 || hi != len || caret.cell.is_some()) {
            self.apply_partial_inline_format(pageedit::InlineFormat::Size(size));
            return;
        }
        ed.pending_format = None;
        Self::remember_step(ed);
        let Some(caret) = ed.caret.as_mut() else { return };
        let len = caret.text.chars().count();
        let (lo, hi) = (caret.selection.0.min(caret.selection.1), caret.selection.0.max(caret.selection.1));
        let spans = match ed.blocks.get_mut(caret.block) {
            Some(
                ergonote_formats::Block::Heading { spans, .. }
                | ergonote_formats::Block::Paragraph(spans)
                | ergonote_formats::Block::Quote(spans)
                | ergonote_formats::Block::Item { spans, .. },
            ) if lo == 0 && hi == len => Some(spans),
            _ => None,
        };
        if let Some(spans) = spans {
            if !pageedit::format_span_range(spans, 0..usize::MAX, pageedit::InlineFormat::Size(size)) {
                return;
            }
            if let Some(block) = ed.blocks.get(caret.block) {
                caret.text = pageedit::caret_text(block);
                caret.inline = None;
                caret.selection = (0, caret.text.chars().count());
                caret.focus = true;
                ed.restore_format_selection = Some((caret.block, caret.selection.0, caret.selection.1));
            }
        } else {
            let open = format!("<span style=\"font-size: {size}pt\">");
            let (text, selection) = pageedit::wrap_selection_with(&caret.text, caret.selection, &open, "</span>");
            caret.text = text;
            caret.selection = selection;
            if let Some(block) = ed.blocks.get_mut(caret.block) {
                caret.write(block);
            }
        }
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
        if !ed.history.here().is_some_and(|s| s.body == ed.body && s.blocks == ed.blocks && s.boxes == ed.boxes) {
            let step = crate::history::Step::new(ed.body.clone(), ed.blocks.clone(), ed.boxes.clone(), ed.caret.clone(), crate::history::Kind::Edit);
            ed.history.record(step);
        }
    }

    /// The keys Windows users expect for paragraph direction. Alt+Shift remains available
    /// to Windows for switching the active input language.
    fn direction_keys(&mut self, ctx: &Context) {
        use ergonote_model::Dir;
        let ctrl_shift = Modifiers { ctrl: true, shift: true, command: true, ..Modifiers::NONE };
        let pressed = |m: Modifiers, k: Key| ctx.input_mut(|i| i.consume_key(m, k));
        if pressed(ctrl_shift, Key::R) {
            self.apply_direction(Some(Dir::Rtl));
        }
        if pressed(ctrl_shift, Key::L) {
            self.apply_direction(Some(Dir::Ltr));
        }
        // Align Left and Align Right, as in Word and OneNote (issue #258). Ctrl+E, which Word
        // gives to Center, is Search here as it is in OneNote, so Center is on the menu only.
        if self.editor.as_ref().is_some_and(|e| e.caret.is_some() || e.across.is_some()) {
            if pressed(Modifiers::COMMAND, Key::L) {
                self.apply_align(ergonote_model::justify::Align::Left);
            }
            if pressed(Modifiers::COMMAND, Key::R) {
                self.apply_align(ergonote_model::justify::Align::Right);
            }
        }
    }

    /// Which way the line the caret is in reads. `None` lets its own words decide.
    fn apply_direction(&mut self, dir: Option<ergonote_model::Dir>) {
        let Some(ed) = self.editor.as_mut() else { return };
        let Some(at) = ed.caret.as_ref().map(|c| c.block) else {
            self.status = crate::tl_id!("ui.click_in_the_page_first_then_use_the_ribbon").into();
            self.status_is_error = false;
            return;
        };
        if !pageedit::set_direction(&mut ed.blocks, at, dir) {
            self.say(crate::tl_id!("ui.a_direction_goes_on_a_paragraph_of_text"));
            return;
        }
        ed.rebuild_body();
        if let Some(b) = ed.blocks.get(at) {
            let text = pageedit::caret_text(b);
            if let Some(c) = ed.caret.as_mut() {
                c.text = text;
            }
        }
        Self::mark_dirty_from_blocks(ed);
        self.say(match dir {
            Some(d) if d.is_rtl() => crate::tl_id!("ui.this_paragraph_reads_right_to_left"),
            Some(_) => crate::tl_id!("ui.this_paragraph_reads_left_to_right"),
            None => crate::tl_id!("ui.this_paragraph_reads_whichever_way_its_words_do"),
        });
    }

    /// Fill the column with the paragraph the caret is in, or stop filling it.
    ///
    /// Which paragraphs are justified is the page's own, like the arrangement of its boxes
    /// and the width of its table columns, so it goes into the page's file and not into the
    /// window. The words are not touched: the stretching happens when the paragraph is
    /// drawn, so the file another program reads is the sentences the writer typed.
    fn apply_justify(&mut self, strength: ergonote_model::justify::Strength) {
        let Some(ed) = self.editor.as_ref() else {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return;
        };
        let Some(at) = ed.caret.as_ref().map(|c| c.block) else {
            self.status = crate::tl_id!("ui.click_in_the_page_first_then_use_the_ribbon").into();
            self.status_is_error = false;
            return;
        };
        if !matches!(ed.blocks.get(at), Some(ergonote_formats::Block::Paragraph(_))) {
            self.say(crate::tl_id!("ui.justification_goes_on_a_paragraph_of_text"));
            return;
        }
        // Asking for the strength a paragraph already has turns justification off, which
        // is how a menu of three gives a way back to a ragged edge without a fourth entry.
        let want = (ed.justified.strength_of(at) != Some(strength)).then_some(strength);
        let mut marked = ed.justified.clone();
        marked.set(at, want);
        self.write_justified(marked, want);
    }

    /// The alignment of the paragraph or heading the caret is in, or that every paragraph and
    /// heading in a multi-block selection shares.
    fn caret_align(&self) -> Option<ergonote_model::justify::Align> {
        use ergonote_formats::Block;
        let ed = self.editor.as_ref()?;
        let blocks: Vec<usize> = match (ed.across.filter(|a| !a.inside_one_block()), ed.caret.as_ref()) {
            (Some(across), _) => across.blocks().collect(),
            (None, Some(caret)) => vec![caret.block],
            (None, None) => return None,
        };
        let text: Vec<usize> =
            blocks.into_iter().filter(|i| matches!(ed.blocks.get(*i), Some(Block::Paragraph(_) | Block::Heading { .. }))).collect();
        let first = ed.justified.align_of(*text.first()?)?;
        text.iter().all(|i| ed.justified.align_of(*i) == Some(first)).then_some(first)
    }

    /// Sets the paragraphs the caret or the selection is in against `align` (issue #258).
    fn apply_align(&mut self, align: ergonote_model::justify::Align) {
        let Some(ed) = self.editor.as_ref() else {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return;
        };
        let blocks: Vec<usize> = match (ed.across.filter(|a| !a.inside_one_block()), ed.caret.as_ref()) {
            (Some(across), _) => across.blocks().collect(),
            (None, Some(caret)) => vec![caret.block],
            (None, None) => {
                self.status = crate::tl_id!("ui.click_in_the_page_first_then_use_the_ribbon").into();
                self.status_is_error = false;
                return;
            }
        };
        self.align_blocks(&blocks, align);
    }

    /// Aligns the paragraphs and headings among `blocks`, or, when every one of them already
    /// has that alignment, gives them back the alignment their words have. One undo step, and
    /// written into the page's own file with the rest of its paragraph layout.
    fn align_blocks(&mut self, blocks: &[usize], align: ergonote_model::justify::Align) {
        use ergonote_formats::Block;
        let Some(ed) = self.editor.as_mut() else { return };
        let text: Vec<usize> =
            blocks.iter().copied().filter(|i| matches!(ed.blocks.get(*i), Some(Block::Paragraph(_) | Block::Heading { .. }))).collect();
        if text.is_empty() {
            self.say(crate::tl_id!("ui.alignment_goes_on_a_paragraph_or_a_heading"));
            return;
        }
        let all = text.iter().all(|i| ed.justified.align_of(*i) == Some(align));
        let mut marked = ed.justified.clone();
        for i in &text {
            marked.set_mark(*i, (!all).then_some(ergonote_model::justify::Mark::Align(align)));
        }
        if marked == ed.justified {
            return;
        }
        // The step before, with the layout it had, and the step after with the new one: an
        // alignment changes no words, so the usual step taken after a change would not see it.
        Self::remember_step(ed);
        if let Some(here) = ed.history.here_mut() {
            here.justified = Some(ed.justified.clone());
        }
        ed.justified = marked;
        let mut step = crate::history::Step::new(ed.body.clone(), ed.blocks.clone(), ed.boxes.clone(), ed.caret.clone(), crate::history::Kind::Edit);
        step.columns = Some(ed.columns.clone());
        step.justified = Some(ed.justified.clone());
        ed.history.record(step);
        if let Some(c) = ed.caret.as_mut() {
            c.focus = true;
        }
        self.save_block_marks((false, true));
        self.say(match (all, align) {
            (true, _) => crate::tl_id!("ui.the_paragraph_follows_its_words_again"),
            (false, ergonote_model::justify::Align::Left) => crate::tl_id!("ui.aligned_left"),
            (false, ergonote_model::justify::Align::Center) => crate::tl_id!("ui.centred"),
            (false, ergonote_model::justify::Align::Right) => crate::tl_id!("ui.aligned_right"),
        });
    }

    /// Opens the Paragraph Spacing dialog for the paragraphs the caret or the selection is in,
    /// showing the value they share, or leaving a field empty where they differ (issue #275).
    fn open_paragraph_spacing(&mut self) {
        use ergonote_formats::Block;
        let Some(ed) = self.editor.as_ref() else {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return;
        };
        let blocks: Vec<usize> = match (ed.across.filter(|a| !a.inside_one_block()), ed.caret.as_ref()) {
            (Some(across), _) => across.blocks().collect(),
            (None, Some(caret)) => vec![caret.block],
            (None, None) => {
                self.status = crate::tl_id!("ui.click_in_the_page_first_then_use_the_ribbon").into();
                self.status_is_error = false;
                return;
            }
        };
        let text: Vec<usize> = blocks
            .into_iter()
            .filter(|i| matches!(ed.blocks.get(*i), Some(Block::Paragraph(_) | Block::Heading { .. } | Block::Item { .. } | Block::Quote(_))))
            .collect();
        if text.is_empty() {
            self.say(crate::tl_id!("ui.spacing_goes_on_text"));
            return;
        }
        let each: Vec<ergonote_model::justify::Spacing> = text.iter().map(|i| ed.justified.spacing_of(*i).unwrap_or_default()).collect();
        let shared = |part: fn(&ergonote_model::justify::Spacing) -> u16| -> String {
            let first = each.first().map(part).unwrap_or(0);
            if each.iter().map(part).all(|v| v == first) { first.to_string() } else { String::new() }
        };
        self.dialog = Some(Dialog::ParagraphSpacing {
            before: shared(|s| s.before),
            after: shared(|s| s.after),
            line: shared(|s| s.line),
            blocks: text,
            first: true,
        });
    }

    /// Gives the paragraphs among `blocks` the room in `change` (before, after, line height), where
    /// a part left as `None` keeps what each already has. One undo step, written into the page's own
    /// file with the rest of its paragraph layout (issue #275).
    fn apply_spacing(&mut self, blocks: &[usize], change: [Option<u16>; 3]) {
        use ergonote_model::justify::Spacing;
        let Some(ed) = self.editor.as_mut() else { return };
        let mut marked = ed.justified.clone();
        for i in blocks {
            let have = marked.spacing_of(*i).unwrap_or_default();
            let want =
                Spacing { before: change[0].unwrap_or(have.before), after: change[1].unwrap_or(have.after), line: change[2].unwrap_or(have.line) };
            marked.set_spacing(*i, Some(want));
        }
        if marked == ed.justified {
            return;
        }
        // The step before, with the layout it had, and the step after with the new one: spacing
        // changes no words, so the usual step taken after a change would not see it.
        Self::remember_step(ed);
        if let Some(here) = ed.history.here_mut() {
            here.justified = Some(ed.justified.clone());
        }
        ed.justified = marked;
        let mut step = crate::history::Step::new(ed.body.clone(), ed.blocks.clone(), ed.boxes.clone(), ed.caret.clone(), crate::history::Kind::Edit);
        step.columns = Some(ed.columns.clone());
        step.justified = Some(ed.justified.clone());
        ed.history.record(step);
        if let Some(c) = ed.caret.as_mut() {
            c.focus = true;
        }
        self.save_block_marks((false, true));
        self.say(crate::tl_id!("ui.paragraph_spacing_set"));
    }

    /// What is typed into the Paragraph Spacing dialog, for tests and automation: the three
    /// values, as a person would type them. Does nothing when the dialog is not open.
    pub fn set_paragraph_spacing_values(&mut self, before: &str, after: &str, line: &str) {
        if let Some(Dialog::ParagraphSpacing { before: b, after: a, line: l, .. }) = self.dialog.as_mut() {
            *b = before.to_string();
            *a = after.to_string();
            *l = line.to_string();
        }
    }

    /// Writes which paragraphs are justified into the page's own file, and says what `want` now is.
    fn write_justified(&mut self, marked: ergonote_model::justify::Justified, want: Option<ergonote_model::justify::Strength>) {
        let Some(ed) = self.editor.as_ref() else { return };
        let path = path_str(&ed.dir);
        let line = ergonote_model::justify::write(&marked);
        self.save_now();
        if self.run("page.justify", &params(&[("path", &path), ("blocks", &line)])).is_some() {
            if let Some(ed) = self.editor.as_mut() {
                ed.justified = marked;
            }
            self.say(match want {
                Some(ergonote_model::justify::Strength::Low) => crate::tl_id!("ui.this_paragraph_fills_the_column_stretched_lightly"),
                Some(ergonote_model::justify::Strength::Medium) => crate::tl_id!("ui.this_paragraph_fills_the_column"),
                Some(ergonote_model::justify::Strength::High) => crate::tl_id!("ui.this_paragraph_fills_the_column_stretched_hard"),
                None => crate::tl_id!("ui.this_paragraph_is_ragged_again"),
            });
        }
    }

    /// Which way the whole page reads. It is written into the page's own file.
    fn apply_page_direction(&mut self, dir: Option<ergonote_model::Dir>) {
        let Some(ed) = self.editor.as_ref() else {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return;
        };
        let path = path_str(&ed.dir);
        let want = dir.map(|d| d.id()).unwrap_or("auto");
        self.save_now();
        if self.run("page.direction", &params(&[("path", &path), ("dir", want)])).is_some() {
            if let Some(ed) = self.editor.as_mut() {
                ed.page_dir = dir;
            }
            self.say(match dir {
                Some(d) if d.is_rtl() => crate::tl_id!("ui.this_page_reads_right_to_left"),
                Some(_) => crate::tl_id!("ui.this_page_reads_left_to_right"),
                None => crate::tl_id!("ui.every_line_of_this_page_decides_for_itself"),
            });
        }
    }

    /// Which way the selection, or what is typed next, reads: an isolated run.
    fn apply_embedding(&mut self, dir: ergonote_model::Dir) {
        let Some(ed) = self.editor.as_mut() else { return };
        let Some(caret) = ed.caret.as_mut() else {
            self.status = crate::tl_id!("ui.click_in_the_page_first_then_use_the_ribbon").into();
            self.status_is_error = false;
            return;
        };
        let (text, selection) = pageedit::isolate(&caret.text, caret.selection, dir);
        caret.text = text;
        caret.selection = selection;
        caret.place = Some(selection.1);
        if let Some(b) = ed.blocks.get_mut(caret.block) {
            caret.write(b);
        }
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
        self.say(if dir.is_rtl() {
            crate::tl_id!("ui.this_run_reads_right_to_left_a_mixed_line_is_still_d_d2a74de074")
        } else {
            crate::tl_id!("ui.this_run_reads_left_to_right_a_mixed_line_is_still_d_af62d34d2f")
        });
    }

    /// Makes the block the caret is in a heading, a list item, a quote or ordinary text.
    fn apply_block_kind(&mut self, kind: &str) {
        use ergonote_formats::markdown::Block;
        let Some(ed) = self.editor.as_mut() else { return };
        if !ed.writable {
            self.status = crate::tl_id!("ui.click_in_the_page_first_then_use_the_ribbon").into();
            self.status_is_error = false;
            return;
        }
        Self::remember_step(ed);
        let at = if let Some(caret) = ed.caret.as_ref() {
            caret.block
        } else {
            let idx = ed
                .blocks
                .iter()
                .position(|b| matches!(b, Block::Paragraph(_) | Block::Heading { .. } | Block::Quote(_) | Block::Item { .. }))
                .unwrap_or_else(|| {
                    let pos = ed.blocks.len();
                    ed.blocks.push(Block::Paragraph(Vec::new()));
                    let _ = ed.blocks_inserted(pos, 1);
                    pos
                });
            let text = ed.blocks.get(idx).map(pageedit::caret_text).unwrap_or_default();
            let len = text.len();
            ed.caret =
                Some(pageedit::Caret { block: idx, text, focus: true, selection: (len, len), place: None, cell: None, span: None, inline: None });
            idx
        };
        if !pageedit::set_kind(&mut ed.blocks, at, kind) {
            return;
        }
        ed.rebuild_body();
        if let Some(b) = ed.blocks.get(at) {
            let text = pageedit::caret_text(b);
            if let Some(c) = ed.caret.as_mut() {
                c.text = text;
                c.focus = true;
            }
        }
        Self::mark_dirty_from_blocks(ed);
        Self::remember_edit(ed);
    }

    /// Puts a tag on the line the caret is in, or takes it off again.
    fn apply_tag(&mut self, tag: ergonote_model::Tag) {
        let Some(ed) = self.editor.as_mut() else { return };
        let Some(at) = ed.caret.as_ref().map(|c| c.block) else {
            self.status = crate::tl_id!("ui.click_in_the_page_first_then_use_the_ribbon").into();
            self.status_is_error = false;
            return;
        };
        if !pageedit::set_tag(&mut ed.blocks, at, tag) {
            self.say(crate::tl_id!("ui.a_tag_goes_on_a_line_of_text"));
            return;
        }
        ed.rebuild_body();
        if let Some(b) = ed.blocks.get(at) {
            let text = pageedit::caret_text(b);
            if let Some(c) = ed.caret.as_mut() {
                c.text = text;
            }
        }
        Self::mark_dirty_from_blocks(ed);
        self.say(crate::i18n::format_message_id("ui.tag_tag", &[("tag", crate::tl!(tag.label()))]));
    }

    /// Cut or copy what is selected in the block the caret is in.
    /// Ctrl+C, Ctrl+X, Ctrl+V and Ctrl+A for the words on a page.
    ///
    /// egui's own text box answers these for a paragraph it is drawing, and it is left to
    /// do so. Nothing answered them anywhere else, and that is a lot of places: not in a
    /// right-to-left paragraph, which ErgoNote draws itself; not for a selection that
    /// spans paragraphs, which no text box owns; and not when a box is picked or nothing
    /// has the keyboard at all. So Ctrl+C did nothing in Arabic, Ctrl+A selected nothing,
    /// and a box that had been copied could not be pasted into an empty one. The owner
    /// reported all three.
    ///
    /// The keys are read here and sent to the same `clipboard` and `paste_here` the ribbon
    /// buttons use, so there is one path and not two. Nothing is done when egui's text box
    /// is holding the keyboard, or both would act and the words would be pasted twice.
    fn clipboard_keys(&mut self, ctx: &egui::Context) {
        let (copy, cut, paste, all) = ctx.input(|i| {
            let hit = |want: egui::Key| {
                // `key_pressed` is the platform-normalized view.  Some native
                // backends do not retain the raw key event in `events` after a
                // focused text widget has considered it, while the normalized
                // edge remains available for the rest of the frame.  Keep the
                // raw-event check as a fallback for the headless harness and
                // older egui backends.
                (i.key_pressed(want) && (i.modifiers.command || i.modifiers.ctrl))
                    || i.events.iter().any(|e| {
                        matches!(e, egui::Event::Key { key, pressed: true, modifiers, .. } if *key == want && (modifiers.command || modifiers.ctrl))
                    })
                    || i.raw.events.iter().any(|e| {
                        matches!(e, egui::Event::Key { key, pressed: true, modifiers, .. } if *key == want && (modifiers.command || modifiers.ctrl))
                    })
            };
            (hit(egui::Key::C), hit(egui::Key::X), hit(egui::Key::V), hit(egui::Key::A))
        });
        if !(copy || cut || paste || all) {
            return;
        }
        let ours = self.words_are_ours();
        if copy && ours {
            self.clipboard("copy");
        }
        if cut && ours {
            self.clipboard("cut");
        }
        // A screenshot has no text for egui's text widget to consume. Route every Ctrl+V
        // through the same paste path; it reads the image clipboard exactly once. Probing
        // with has_picture() first was racy with VirtualBox clipboard synchronisation and
        // could make the actual image read miss the clipboard handoff.
        if paste {
            // Consume the shortcut before the text widgets draw. Otherwise an LTR TextEdit
            // handles Ctrl+V itself while the page menu goes through `paste_here`, giving the
            // two entry points different behavior.
            ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::V) || i.consume_key(Modifiers::CTRL, Key::V));
            let picture = self.clipboard.picture();
            if let (Some(picture), Some(dir)) = (picture, self.editor.as_ref().map(|e| e.dir.clone())) {
                self.paste_picture(&dir, picture);
            } else {
                self.paste_here();
            }
        }
        if all {
            let search_focused = ctx.memory(|m| m.has_focus(Id::new("page-search")));
            let title_focused = ctx.memory(|m| m.has_focus(Id::new("page-title-box")));
            if !search_focused && !title_focused && !self.show_source && self.dialog.is_none() && self.backstage.is_none() {
                let in_page_editor = self.editor.as_ref().is_some_and(|e| e.caret.is_some() || e.across.is_some());
                if in_page_editor || ctx.memory(|m| m.focused().is_none()) {
                    self.select_all_smart(Some(ctx));
                }
            }
        }
    }

    /// True when the words the caret is in are drawn by ErgoNote rather than by an egui
    /// text box, so the keys above are ours to answer.
    fn words_are_ours(&self) -> bool {
        let Some(ed) = self.editor.as_ref() else { return false };
        // A selection across paragraphs belongs to no text box at all.
        if ed.across.is_some_and(|a| !a.inside_one_block()) {
            return true;
        }
        let Some(caret) = ed.caret.as_ref() else { return false };
        // The same question `show_blocks` asks when it decides which painter draws the
        // block: a line with right-to-left in it is ours, and code and maths never are.
        pageedit::needs_our_caret(&caret.text)
            && !matches!(
                ed.blocks.get(caret.block),
                Some(ergonote_formats::markdown::Block::Code { .. }) | Some(ergonote_formats::markdown::Block::MathBlock(_))
            )
    }

    /// Ctrl+A / Tout sélectionner à 2 niveaux (comme OneNote) :
    /// 1er appui : tout le paragraphe courant ; 2e appui (ou hors paragraphe) : toute la page.
    fn select_all_smart(&mut self, ctx: Option<&Context>) {
        let Some(ed) = self.editor.as_mut() else { return };
        if let Some(caret) = ed.caret.as_mut() {
            let end = caret.text.chars().count();
            let (lo, hi) = (caret.selection.0.min(caret.selection.1), caret.selection.0.max(caret.selection.1));
            if end > 0 && (lo > 0 || hi < end || ed.blocks.len() <= 1) {
                caret.selection = (end, 0);
                caret.place = None;
                ed.across = None;
                if let Some(c) = ctx {
                    c.data_mut(|d| d.insert_temp(pageedit::restore_selection_id(), (caret.block, end, 0)));
                } else {
                    ed.restore_format_selection = Some((caret.block, end, 0));
                }
                self.say(crate::tl_id!("ui.paragraph_selected"));
                return;
            }
        }
        self.select_all_page(ctx);
    }

    fn select_all_page(&mut self, ctx: Option<&Context>) {
        let Some(ed) = self.editor.as_mut() else { return };
        if ed.blocks.is_empty() {
            return;
        }
        if let Some(c) = ctx {
            c.input_mut(|i| {
                let _ = i.consume_key(Modifiers::COMMAND, Key::A);
                let _ = i.consume_key(Modifiers::CTRL, Key::A);
            });
        }
        if ed.blocks.len() == 1 {
            let block = 0;
            let text = pageedit::caret_text(&ed.blocks[0]);
            let end = text.chars().count();
            ed.across = None;
            ed.caret = Some(pageedit::Caret { block, text, focus: true, selection: (end, 0), place: None, cell: None, span: None, inline: None });
            ed.restore_format_selection = Some((0, end, 0));
        } else {
            let last = ed.blocks.len() - 1;
            let last_len = pageedit::block_text(&ed.blocks[last]).len();
            ed.caret = None;
            ed.pending_format = None;
            ed.across = Some(pageedit::Across { from: (0, 0), to: (last, last_len) });
        }
        self.say("Toute la page est sélectionnée.");
    }

    fn clipboard(&mut self, which: &str) {
        let Some(ed) = self.editor.as_mut() else { return };
        // A selection that runs across blocks is not any one block's, so it is taken from
        // the blocks themselves. Cut takes the words out and joins what is left.
        if let Some(open) = ed.across.filter(|a| !a.inside_one_block()) {
            let picked = pageedit::span_text(&ed.blocks, open);
            if picked.is_empty() {
                self.say(crate::tl_id!("ui.select_some_words_first"));
                return;
            }
            let Some(copied) = pageedit::copied_span(&ed.blocks, open) else { return };
            self.pending_copy = Some(ergonote_formats::render::blocks_to_text(&copied));
            self.pending_copy_html = Some(ergonote_formats::render::blocks_to_html(&copied));
            if which == "cut" {
                let (block, at) = pageedit::delete_span(&mut ed.blocks, open);
                let text = ed.blocks.get(block).map(pageedit::caret_text).unwrap_or_default();
                ed.caret =
                    Some(pageedit::Caret { block, text, focus: true, selection: (at, at), place: Some(at), cell: None, span: None, inline: None });
                ed.across = None;
                ed.rebuild_body();
                Self::mark_dirty_from_blocks(ed);
            }
            self.say(if which == crate::tl_id!("ui.cut_2") { crate::tl_id!("ui.cut") } else { crate::tl_id!("ui.copied") });
            return;
        }
        let Some(caret) = ed.caret.as_mut() else {
            self.status = crate::tl_id!("ui.click_in_the_page_first").into();
            self.status_is_error = false;
            return;
        };
        let (a, b) = (caret.selection.0.min(caret.selection.1), caret.selection.0.max(caret.selection.1));
        let picked: String = caret.text.chars().skip(a).take(b.saturating_sub(a)).collect();
        if picked.is_empty() {
            self.status = crate::tl_id!("ui.select_some_words_first").into();
            self.status_is_error = false;
            return;
        }
        let Some(copied) = ed.blocks.get(caret.block).and_then(|block| caret.copied_blocks(block)) else { return };
        self.pending_copy = Some(ergonote_formats::render::blocks_to_text(&copied));
        self.pending_copy_html = Some(ergonote_formats::render::blocks_to_html(&copied));
        if which == "cut" {
            let (text, at) = pageedit::insert_at_caret(&caret.text, caret.selection, "");
            caret.text = text;
            caret.selection = (at, at);
            caret.place = Some(at);
            caret.focus = true;
            if let Some(b) = ed.blocks.get_mut(caret.block) {
                caret.write(b);
            }
            ed.rebuild_body();
            Self::mark_dirty_from_blocks(ed);
        }
        self.say(if which == crate::tl_id!("ui.cut_2") { crate::tl_id!("ui.cut") } else { crate::tl_id!("ui.copied") });
    }

    /// More or less indent on the list item the caret is in.
    fn indent_block(&mut self, deeper: bool) {
        let Some(ed) = self.editor.as_mut() else { return };
        let Some(at) = ed.caret.as_ref().map(|c| c.block) else {
            self.status = crate::tl_id!("ui.click_in_the_page_first").into();
            self.status_is_error = false;
            return;
        };
        if !pageedit::indent(&mut ed.blocks, at, deeper) {
            self.say(crate::tl_id!("ui.only_a_list_item_can_be_indented"));
            return;
        }
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
    }

    /// Takes the marks off the selection: bold, italic, strikethrough, code.
    fn clear_formatting(&mut self) {
        let Some(ed) = self.editor.as_mut() else { return };
        let Some(caret) = ed.caret.as_mut() else { return };
        if let Some(b) = ed.blocks.get_mut(caret.block) {
            caret.clear_marks(b);
        }
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
    }

    /// Puts a whole block (a table, a diagram) after the caret.
    fn insert_block(&mut self, what: &str) {
        let Some(ed) = self.editor.as_mut() else { return };
        let at = ed.caret.as_ref().map(|c| c.block + 1).unwrap_or(ed.blocks.len()).min(ed.blocks.len());
        if !pageedit::insert_block(&mut ed.blocks, at, what) {
            self.say(crate::tl_id!("ui.that_is_not_something_a_page_can_hold"));
            return;
        }
        let marks = ed.blocks_inserted(at, 1);
        ed.rebuild_body();
        ed.caret = ed.blocks.get(at).map(|b| pageedit::Caret {
            block: at,
            text: pageedit::caret_text(b),
            focus: true,
            selection: (0, 0),
            place: None,
            cell: None,
            span: None,
            inline: None,
        });
        Self::mark_dirty_from_blocks(ed);
        self.save_block_marks(marks);
    }

    /// Puts a snippet where the caret is, or at the end of the page.
    /// Opens the Link dialog, as OneNote's Insert > Link and Ctrl+K do (issue #163). With the
    /// caret in a link it edits that link; with words selected they are the words to show;
    /// otherwise the link goes in at the caret.
    pub fn open_link_dialog(&mut self) {
        let Some(ed) = self.editor.as_mut().filter(|ed| ed.writable) else {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return;
        };
        if ed.across.is_some_and(|a| !a.inside_one_block()) {
            self.say(crate::tl_id!("ui.select_words_in_one_line_for_a_link"));
            return;
        }
        Self::place_caret_for_format(ed);
        let Some(caret) = ed.caret.as_ref() else {
            self.say(crate::tl_id!("ui.click_in_the_page_first_then_use_the_ribbon"));
            return;
        };
        let (lo, hi) = (caret.selection.0.min(caret.selection.1), caret.selection.0.max(caret.selection.1));
        let (range, text, address, editing) = match pageedit::link_at(&caret.text, lo, hi) {
            Some((start, end, words, address)) => ((start, end), words, address, true),
            None => ((lo, hi), caret.text.chars().skip(lo).take(hi - lo).collect(), String::new(), false),
        };
        self.dialog = Some(Dialog::Link { block: caret.block, cell: caret.cell, range, text, address, editing, first: true });
    }

    /// Puts `replacement` in place of the characters `range` of the caret's line, when the
    /// caret is still in the line the Link dialog was opened for. One step of the history.
    fn replace_in_caret_line(&mut self, block: usize, cell: Option<(usize, usize)>, range: (usize, usize), replacement: &str) {
        let Some(ed) = self.editor.as_mut() else { return };
        if !ed.caret.as_ref().is_some_and(|c| c.block == block && c.cell == cell) {
            self.say(crate::tl_id!("ui.the_line_changed_put_the_link_in_again"));
            return;
        }
        Self::remember_step(ed);
        let Some(caret) = ed.caret.as_mut() else { return };
        let chars: Vec<char> = caret.text.chars().collect();
        let (a, b) = (range.0.min(chars.len()), range.1.min(chars.len()));
        let mut text: String = chars[..a].iter().collect();
        text.push_str(replacement);
        let at = text.chars().count();
        text.extend(&chars[b..]);
        caret.text = text;
        caret.selection = (at, at);
        caret.place = Some(at);
        caret.focus = true;
        // Read again from the words, so the new link is a link and not text that looks like one.
        caret.inline = None;
        if let Some(b) = ed.blocks.get_mut(block) {
            caret.write(b);
        }
        ed.rebuild_body();
        Self::mark_dirty_from_blocks(ed);
        Self::remember_edit(ed);
    }

    fn insert_in_page(&mut self, text: &str) {
        let Some(ed) = self.editor.as_mut() else { return };
        // An insert is an edit: it is recorded for undo before the page changes.
        if ed.caret.is_some() {
            Self::remember_step(ed);
        }
        match ed.caret.as_mut() {
            Some(caret) => {
                // Where the caret is, not at the end of the block.
                let (new_text, at) = pageedit::insert_at_caret(&caret.text, caret.selection, text.trim_matches('\n'));
                caret.text = new_text;
                caret.selection = (at, at);
                caret.place = Some(at);
                caret.focus = true;
                if let Some(b) = ed.blocks.get_mut(caret.block) {
                    caret.write(b);
                }
                ed.rebuild_body();
                Self::mark_dirty_from_blocks(ed);
            }
            None => {
                let cursor = ed.body.chars().count();
                self.cursor = cursor;
                self.insert(text);
            }
        }
    }

    /// The theme switcher: 14 swatches (light and dark, seven accents each) and a switch to
    /// follow the system setting.
    fn theme_menu(&mut self, ui: &mut Ui, tokens: Tokens) {
        let current = self.theme();
        let mut pick: Option<(Mode, Accent)> = None;
        let mut follow = self.mode_pref == ModePref::System;
        let follow_before = follow;
        // A sun or a moon in the theme's accent colour, instead of a line of text.
        let (rect, resp) = ui.allocate_exact_size(vec2(28.0, 24.0), Sense::click());
        let painter = ui.painter_at(rect);
        if resp.hovered() {
            painter.rect_filled(rect, 4.0, tokens.row_hover);
        }
        let icon_rect = egui::Rect::from_center_size(rect.center(), vec2(19.0, 19.0));
        if current.mode.is_dark() {
            icons::paint_moon(&painter, icon_rect, tokens.accent, if resp.hovered() { tokens.row_hover } else { tokens.panel });
        } else {
            icons::paint(&painter, icon_rect, Icon::Sun, tokens.accent);
        }
        let theme_label = current.label();
        let label = crate::i18n::format_message_id("ui.theme_theme_2", &[("theme", &theme_label)]);
        let enabled = ui.is_enabled();
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &label));
        let theme_label = current.label();
        let tip = crate::i18n::format_message_id("ui.theme_theme_click_to_change_it", &[("theme", &theme_label)]);
        let resp = resp.on_hover_text(tip);
        egui::Popup::menu(&resp).show(|ui| {
            theme_grid(ui, current, &mut pick, &mut follow);
        });
        // The same chooser, as a window, for the ribbon's Theme button.
        if self.theme_window {
            let mut open = true;
            egui::Window::new(crate::tl_id!("ui.theme"))
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .anchor(Align2::RIGHT_TOP, vec2(-12.0, 120.0))
                .show(ui.ctx(), |ui| {
                    theme_grid(ui, current, &mut pick, &mut follow);
                });
            if !open || pick.is_some() {
                self.theme_window = false;
            }
        }
        if let Some((mode, accent)) = pick {
            // A swatch names a mode, so clicking one means that mode, even while the app was
            // following the system. Otherwise picking "Dark Default" on a light desktop did
            // nothing visible. The switch below is the way back to following the system.
            self.set_theme(Theme { mode, accent });
        } else if follow != follow_before {
            if follow {
                self.follow_system_theme(current.accent);
            } else {
                self.set_theme(current);
            }
        }
    }

    /// A compact language picker beside the header's theme control.
    fn language_menu(&mut self, ui: &mut Ui, tokens: Tokens) {
        let languages = crate::i18n::available_languages();
        let active_code = if self.ui_language == crate::i18n::AUTO { crate::i18n::current_code() } else { self.ui_language.clone() };
        let active_name =
            languages.iter().find(|language| language.code == active_code).map_or_else(|| active_code.clone(), |language| language.name.clone());
        let badge = if active_code == "en-XA" {
            "XA".to_string()
        } else if active_code == "ar-XB" {
            "XB".to_string()
        } else {
            active_code.split('-').next().unwrap_or("en").to_ascii_uppercase()
        };
        let label = crate::i18n::format_message_id("ui.interface_language_language", &[("language", &active_name)]);
        let (rect, resp) = ui.allocate_exact_size(vec2(52.0, 24.0), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(rect, 4.0, tokens.row_hover);
        }
        let flag_rect = egui::Rect::from_min_size(rect.left_top() + vec2(5.0, 5.0), vec2(20.0, 14.0));
        crate::language_flag::paint_at(ui, flag_rect, &active_code);
        ui.painter().text(rect.left_center() + vec2(36.0, 0.0), Align2::CENTER_CENTER, badge, egui::FontId::proportional(10.5), tokens.text);
        let enabled = ui.is_enabled();
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &label));
        let tip = crate::i18n::format_message_id("ui.language_click_to_change_it", &[("language", &active_name)]);
        let resp = resp.on_hover_text(tip);
        let mut picked = None;
        egui::Popup::menu(&resp).style(crate::theme::picker_popup_style(tokens)).show(|ui| {
            let automatic = crate::tl_id!("ui.automatic_system_language");
            if ui.selectable_label(self.ui_language == crate::i18n::AUTO, automatic).clicked() {
                picked = Some(crate::i18n::AUTO.to_string());
            }
            ui.separator();
            for language in &languages {
                let mut clicked = false;
                ui.horizontal(|ui| {
                    crate::language_flag::paint(ui, &language.code);
                    clicked = ui.selectable_label(self.ui_language == language.code, &language.name).clicked();
                });
                if clicked {
                    picked = Some(language.code.clone());
                }
            }
        });
        if let Some(language) = picked {
            self.set_ui_language(language);
        }
    }

    /// Puts the search pane on the right and the keyboard in its box.
    fn open_search(&mut self) {
        self.search_open = true;
        self.focus_search = true;
    }

    fn dismiss_search(&mut self) {
        self.search.clear();
        self.searched.clear();
        self.hits.clear();
        self.search_open = false;
    }

    fn open_search_result(&mut self, path: &Path) {
        self.expand_to(path);
        self.open_page(path);
    }

    /// Looks again for the tagged notes in the Tags Summary's scope.
    fn refresh_tags(&mut self) {
        let Some(pane) = self.tags_pane.as_ref() else { return };
        let page = self.editor.as_ref().map(|ed| ed.dir.clone());
        let scope = match pane.scope {
            TagScope::Page => page.clone(),
            TagScope::Section => page.as_deref().and_then(section_for),
            TagScope::Notebook => page.as_deref().and_then(notebook_for),
            TagScope::All => None,
        };
        let mut p: Vec<(&str, String)> = Vec::new();
        if let Some(scope) = scope {
            p.push(("scope", path_str(&scope)));
        } else if pane.scope != TagScope::All {
            // Nothing open to look round: there is nothing in this page or section to list.
            if let Some(pane) = self.tags_pane.as_mut() {
                pane.found.clear();
            }
            return;
        }
        if pane.scope == TagScope::Page {
            p.push(("page", "true".into()));
        }
        if pane.unchecked {
            p.push(("unchecked", "true".into()));
        }
        let pairs: Vec<(&str, &str)> = p.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let found = match self.run("tags.find", &params(&pairs)) {
            Some(Output::Tagged(found)) => found,
            _ => Vec::new(),
        };
        if let Some(pane) = self.tags_pane.as_mut() {
            pane.found = found;
        }
    }

    /// The Tags Summary pane, on the right of the page as OneNote has it.
    /// Reads the templates again: the built-in ones and those kept in the notebooks folder.
    fn refresh_templates(&mut self) {
        let found = ergonote_store::page_templates::list(self.session.root());
        if let Some(pane) = self.templates_pane.as_mut() {
            pane.templates = found;
        }
    }

    /// A new page in the open section, started from the template, and opened.
    fn page_from_template(&mut self, id: &str, name: &str) {
        let Some(section) = self.current_section(self.session.tree()).map(|n| n.path.clone()) else {
            self.say(crate::tl_id!("ui.open_a_section_first_for_a_template"));
            return;
        };
        self.save_now();
        if let Some(Output::Path(page)) = self.run("page.from_template", &params(&[("parent", &path_str(&section)), ("template", id)])) {
            self.open_page(&page);
            self.say(crate::i18n::format_message_id("ui.page_from_template", &[("name", name)]));
        }
    }

    /// Keeps the open page as a template called `name`.
    fn save_page_as_template(&mut self, name: &str) {
        let Some(page) = self.editor.as_ref().map(|ed| ed.dir.clone()) else {
            self.say(crate::tl_id!("ui.open_a_page_first"));
            return;
        };
        self.save_now();
        if let Some(Output::Path(_)) = self.run("page.save_template", &params(&[("path", &path_str(&page)), ("name", name)])) {
            self.say(crate::i18n::format_message_id("ui.saved_as_template", &[("name", name)]));
            if let Some(pane) = self.templates_pane.as_mut() {
                pane.name.clear();
            }
            self.refresh_templates();
        }
    }

    fn central(&mut self, ui: &mut Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            if self.editor.is_none() {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() * 0.22);
                    ui.add(egui::Image::from_bytes("bytes://ergonote-logo", crate::LOGO_PNG).fit_to_exact_size(vec2(128.0, 128.0)));
                    ui.add_space(8.0);
                    ui.heading(crate::tl_id!("ui.ergonote"));
                    ui.label(crate::tl_id!("ui.notes_as_plain_folders_open_a_page_or_create_a_notebook_to_start"));
                    ui.add_space(4.0);
                    let folder = self.shown(self.session.root());
                    let message = crate::i18n::format_message_id("ui.notebooks_folder_folder", &[("folder", &folder)]);
                    ui.label(egui::RichText::new(message).small().color(self.tokens.muted));
                });
                return;
            }
            self.editor_ui(ui);
        });
    }

    fn wheel_canvas_pan_delta(delta: egui::Vec2, modifiers: Modifiers, canvas_pan_enabled: bool) -> egui::Vec2 {
        if modifiers.ctrl {
            return egui::Vec2::ZERO;
        }
        if modifiers.shift {
            return egui::vec2(delta.y, 0.0);
        }
        if canvas_pan_enabled { delta } else { egui::Vec2::ZERO }
    }

    fn editor_ui(&mut self, ui: &mut Ui) {
        if !self.icon_recovery_suppressed
            && self.dialog.is_none()
            && let Some(ed) = self.editor.as_ref()
            && let Some(name) = first_missing_attachment_in_source(&ed.body, &ed.dir).or_else(|| first_legacy_attachment_in_source(&ed.body))
        {
            let key = format!("{}::{name}", ed.dir.display());
            if self.icon_recovery_reported.insert(key) {
                log::warn!("attachment icon is missing: page={:?} attachment={name:?}", ed.dir);
                self.dialog = Some(Dialog::MissingAttachmentIcon { page: ed.dir.clone(), name });
            }
        }
        let page_input_rect = self.page_rect;
        let wheel_canvas_pan_enabled = self.pointer != ribbon::Pointer::Write;
        if self.editor.is_some() && !self.show_source {
            // Draw > Panning Hand: the main button drags the page as the middle one always does
            // (issue #270), but not from the scroll bar, which keeps its own drag.
            let hand = self.pointer == ribbon::Pointer::Pan;
            let bar = self.page_scroll_strip;
            let (pointer, middle_pressed, middle_down, wheel) = ui.input(|i| {
                let pointer = page_input_rect.and_then(|rect| i.pointer.interact_pos().filter(|p| rect.contains(*p)));
                let wheel = if page_input_rect.is_some_and(|rect| i.pointer.hover_pos().is_some_and(|p| rect.contains(p))) {
                    i.events.iter().fold(egui::Vec2::ZERO, |sum, event| match event {
                        egui::Event::MouseWheel { delta, modifiers, .. } => {
                            sum + Self::wheel_canvas_pan_delta(*delta, *modifiers, wheel_canvas_pan_enabled)
                        }
                        _ => sum,
                    })
                } else {
                    egui::Vec2::ZERO
                };
                let pointer = pointer.filter(|p| !(hand && bar.is_some_and(|b| b.contains(*p))));
                let pressed = i.pointer.button_pressed(egui::PointerButton::Middle) || (hand && i.pointer.primary_pressed());
                let down = i.pointer.button_down(egui::PointerButton::Middle) || (hand && i.pointer.primary_down());
                (pointer, pressed, down, wheel)
            });
            if wheel_canvas_pan_enabled {
                self.canvas_pan += wheel;
                self.canvas_pan.x = self.canvas_pan.x.clamp(-1.0e6, 1.0e6);
                self.canvas_pan.y = self.canvas_pan.y.clamp(-1.0e6, 1.0e6);
            }
            if middle_pressed {
                self.canvas_drag = pointer;
            } else if middle_down {
                if let (Some(last), Some(now)) = (self.canvas_drag, pointer) {
                    self.canvas_pan += now - last;
                    self.canvas_drag = Some(now);
                }
            } else {
                self.canvas_drag = None;
            }
        } else {
            self.canvas_drag = None;
        }
        let wheel_zoom =
            ui.input(|i| page_input_rect.and_then(|rect| i.pointer.hover_pos().filter(|p| rect.contains(*p))).map_or(1.0, |_| i.zoom_delta()));
        if wheel_zoom.is_finite() && (wheel_zoom - 1.0).abs() > 0.001 {
            #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            {
                self.zoom_percent = (self.zoom_percent as f32 * wheel_zoom).round().clamp(50.0, 250.0) as u32;
            }
            self.fit_page_width = false;
        }
        if self.fit_page_width {
            let width = ui.available_width();
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let percent = (width / 780.0 * 100.0).round().clamp(50.0, 250.0) as u32;
            self.zoom_percent = percent;
            self.fit_page_width = false;
        }
        #[allow(clippy::cast_precision_loss)]
        let zoom_scale = self.zoom_percent as f32 / 100.0;
        let tokens = self.tokens;
        let mut rename: Option<String> = None;
        let cursor = self.cursor;
        let mut changed = false;
        let show_source = self.show_source;
        let full_page = self.full_page;
        let (hide_title, rules) = (self.hide_title, self.paper_now());
        // The grid a moved or resized box snaps to: the paper's own lines, or 12 points on plain
        // paper. None while snapping is off (issue #79).
        let snap = self.snap_to_grid.then(|| if rules.on() { rules.step() } else { PLAIN_SNAP_STEP });
        // The wash behind this page, if it carries a colour: a pale tint of it over the
        // page's own background, so the words keep their contrast whatever is chosen.
        let page_colour = self.editor.as_ref().and_then(|e| e.colour).map(|c| {
            let dark = self.theme().mode == crate::theme::Mode::Dark;
            let (r, g, b) = c.rgb(dark);
            crate::theme::mix(self.tokens.editor, egui::Color32::from_rgb(r, g, b), 0.16)
        });
        // What the title area is showing, so the page's words do not say it again.
        let shown_title = self.editor.as_ref().map(|e| e.title.clone()).unwrap_or_default();
        let (mut toggle_source, mut toggle_full_page, mut focus_search) = (false, false, false);
        let mut from_blocks = false;
        let mut boxes_moved = false;
        let mut cols_moved = false;
        let mut justified_moved = false;
        // A file other than the page written in the code view, waiting to go to disk.
        let mut wrote_file: Option<(String, String)> = None;
        let mut file_action: Option<crate::codeview::FileAction> = None;
        let mut code = std::mem::take(&mut self.code);
        let mut save_cols: Option<(String, String)> = None;
        let mut save_justified: Option<(String, String)> = None;
        // A link clicked in the page this frame, followed once the page's borrow is over.
        let mut follow: Option<String> = None;
        // The drawing on this page is lent to the block that holds it, and written back
        // once the page's borrow is over.
        let (pen, pressure) = (self.pen(), self.stylus.sample().pressure);
        let mut pad = self.pad.take();
        let mut ink_changed = false;
        let mut page_rect = None;
        let mut hbar = crate::hscroll::Shown::default();
        // What the page's right-click menu asked for, run once the page's borrow is over.
        let mut page_acts: Vec<ribbon::Act> = Vec::new();
        let extra_sheets = self.extra_sheets;
        let mut scroll_to_sheet = self.scroll_to_sheet.take();
        let mut page_total_sheets: usize = 1;
        let mut page_sheet_h_px: f32 = 1122.0;
        let collect_now = std::mem::take(&mut self.collect_pending);
        // Insert Space: in hand this frame, and whether it was used or put down.
        let space_on = self.space_tool && self.pointer == ribbon::Pointer::Write;
        let mut space_over = false;
        let textbox_on = self.textbox_tool && self.pointer == ribbon::Pointer::Write;
        let mut textbox_placed: Option<(f32, f32)> = None;
        let mut textbox_cancelled = false;
        let simple_mobile = self.is_simple_mobile();
        {
            let Some(ed) = self.editor.as_mut() else { return };
            let missing_fonts = crate::fonts::ensure_page_fonts(ui.ctx(), &crate::fonts::page_families(&ed.blocks));
            if !missing_fonts.is_empty() {
                let message = crate::i18n::format_message_id("ui.fonts_unavailable", &[("fonts", &missing_fonts.join(", "))]);
                ui.label(egui::RichText::new(message).color(tokens.muted));
            }
            // One id for the title, whether it is drawn as words or as a text box.
            let title_id = egui::Id::new("page-title-box");
            if !simple_mobile || ui.input(|i| !i.raw.hovered_files.is_empty()) {
                ui.horizontal_wrapped(|ui| {
                    // File details live in the status bar. Keep only compact page controls
                    // here, with a drop instruction while a file is actually over the window.
                    if ui.input(|i| !i.raw.hovered_files.is_empty()) {
                        ui.label(egui::RichText::new(crate::tl_id!("ui.drop_a_file_onto_the_page_to_add_it")).small().color(tokens.muted));
                    }
                    if !simple_mobile {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // The page controls, in OneNote's order: full page, search, source.
                            let full = icon_toggle(
                                ui,
                                if full_page { Icon::ExitFullPage } else { Icon::FullPage },
                                full_page,
                                &tokens,
                                crate::tl_id!("ui.full_page"),
                                if full_page {
                                    crate::tl_id!("ui.show_the_notebooks_again")
                                } else {
                                    crate::tl_id!("ui.give_the_page_the_whole_window")
                                },
                            );
                            if full.clicked() {
                                toggle_full_page = true;
                            }
                            if icon_toggle(ui, Icon::Search, false, &tokens, crate::tl_id!("ui.search"), crate::tl_id!("ui.search_all_pages"))
                                .clicked()
                            {
                                focus_search = true;
                            }
                        });
                    }
                });
                ui.separator();
            }
            let height = ui.available_height();
            // How far the page's content reaches sideways on screen this frame (issue #188).
            let mut page_span: Option<(f32, f32)> = None;
            // The page itself. Without the source beside it the page is a readable column,
            // the way a page looks in OneNote or Word, and a click puts the caret in it.
            let mut page = |ui: &mut Ui,
                            ed: &mut Editor,
                            wide: bool,
                            _changed: &mut bool,
                            from_blocks: &mut bool,
                            _toggle: &mut bool,
                            boxes_moved: &mut bool,
                            cols_moved: &mut bool,
                            justified_moved: &mut bool,
                            follow: &mut Option<String>,
                            rename: &mut Option<String>| {
                let raw_viewport = ui.available_rect_before_wrap().shrink2(vec2(0.0, 1.0));
                let visual_sheet = crate::paper::sheet_for(ed.paper_size, zoom_scale);
                let sheet_h_pts = (visual_sheet.size.y / zoom_scale.max(0.01)).max(200.0);
                let ink_bottom_pts = pad.as_ref().and_then(|p| p.bounds()).map(|[_, _, _, y1]| y1).unwrap_or(0.0);
                let content_bottom_pts = ed.content_bottom.unwrap_or(0.0).max(ink_bottom_pts);
                let auto_sheets = crate::paper::sheet_count(content_bottom_pts, sheet_h_pts);
                let total_sheets = auto_sheets.max(1 + extra_sheets);
                let total_sheets_px = total_sheets as f32 * visual_sheet.size.y;
                page_total_sheets = total_sheets;
                page_sheet_h_px = visual_sheet.size.y;
                let viewport = raw_viewport.with_max_y(raw_viewport.max.y.max(raw_viewport.min.y + total_sheets_px + 48.0));
                let content_rect = viewport.translate(self.canvas_pan);
                let available_width = content_rect.width();
                let sheet = ed.paper_size.filter(|_| wide).map(|p| crate::paper::Sheet::of(p, zoom_scale));
                let page_column = sheet.map_or(780.0 * zoom_scale, crate::paper::Sheet::column);
                let margin = if wide { ((available_width - page_column) / 2.0).max(8.0) } else { 0.0 };
                let top_pad = sheet.map_or(4, |s| s.margins[0].round().clamp(4.0, 127.0) as i8);
                let column_w = if available_width >= page_column { page_column } else { (available_width - 16.0).max(1.0) };
                let (sheet_left, sheet_top, sheet_draw_w) = if let Some(sheet) = sheet {
                    (
                        content_rect.min.x + f32::from(margin as i8) - sheet.margins[3],
                        content_rect.min.y + f32::from(top_pad) - sheet.margins[0],
                        sheet.size.x,
                    )
                } else {
                    let pad_x = 12.0_f32.min((f32::from(margin as i8) - 2.0).max(0.0));
                    (content_rect.min.x + f32::from(margin as i8) - pad_x, content_rect.min.y + f32::from(top_pad) - 2.0, column_w + pad_x * 2.0)
                };
                if wide && let Some(wash) = page_colour {
                    ui.painter_at(viewport).rect_filled(viewport, 0.0, wash);
                } else if wide {
                    // The sheet is the theme's page: white in a light theme, dark in a dark
                    // one, so the writing on it keeps the theme's own text colour.
                    let sheets_rect = egui::Rect::from_min_size(egui::pos2(sheet_left, sheet_top), vec2(sheet_draw_w, total_sheets_px));
                    ui.painter_at(viewport).rect_filled(sheets_rect, 2.0, tokens.editor);
                }
                if wide && rules.on() {
                    let lines = crate::theme::rule_lines_in(&tokens, ed.rule_colour);
                    crate::ribbon::paint_paper_canvas(
                        &ui.painter_at(viewport),
                        viewport,
                        rules,
                        zoom_scale,
                        // Through the page's origin, so a box on the grid sits on a line. A page
                        // not yet drawn as boxes counts from the canvas's corner.
                        content_rect.min + ed.origin_in_canvas.unwrap_or_default(),
                        lines.0,
                        lines.1,
                    );
                }
                ui.scope_builder(egui::UiBuilder::new().id_salt("page-canvas-content").max_rect(content_rect), |ui| {
                    // Which way lines fall when they say nothing themselves: the page's own
                    // direction, if the writer gave it one.
                    let page_dir = ed.page_dir;
                    if wide {
                        crate::paper::paint_sheets(
                            &ui.painter().with_clip_rect(viewport),
                            egui::pos2(sheet_left, sheet_top),
                            sheet_draw_w,
                            visual_sheet,
                            total_sheets,
                            sheet.is_some(),
                            &tokens,
                        );
                    }
                    egui::Frame::NONE.inner_margin(egui::Margin { left: margin as i8, right: margin as i8, top: top_pad, bottom: 8 }).show(ui, |ui| {
                        let column = if available_width >= page_column { page_column } else { (available_width - 16.0).max(1.0) };
                        ui.set_min_width(column);
                        ui.set_width(column);
                        // The page's own name, at the top of the sheet. Before the sheet's
                        // origin is taken below, so the ink layer keeps its own coordinates
                        // and a drawing stays beside the words it was made next to.
                        if !hide_title {
                            page_title(ui, ed, &tokens, title_id, rename, zoom_scale);
                        }
                        if wide && ed.writable {
                            // The caret lives in the page: every block is drawn the way it
                            // will be read, and the one being written in is a text box in
                            // that block's own type. The markup is never shown here. The
                            // blocks sit in containers, which is where they are placed.
                            let top = ui.cursor().min;
                            if let Some(pad) = pad.as_mut() {
                                // Ink sits above page content, but below foreground controls such as
                                // pen palettes. A foreground ink layer registered after the ribbon
                                // paints across its menus when a stroke is already in progress.
                                let layer = egui::LayerId::new(egui::Order::Middle, ui.id().with("ink-overlay"));
                                // The viewport is one view tall from the top of the page, which a
                                // scrolled page has left behind: the ink reaches down to the
                                // bottom of what the window shows.
                                let ink_area = viewport.with_max_y(viewport.max.y.max(ui.clip_rect().max.y));
                                let hit = self
                                    .page_scroll_strip
                                    .filter(|bar| bar.min.x > ink_area.min.x)
                                    .map_or(ink_area, |bar| ink_area.with_max_x(ink_area.max.x.min(bar.min.x)));
                                let mut ink_ui = ui.new_child(egui::UiBuilder::new().layer_id(layer).max_rect(ink_area));
                                ink_changed |= crate::inkpad::show(&mut ink_ui, ink_area, hit, top, pad, pen, pressure, &tokens, zoom_scale);
                            }
                            // A press lets go of whatever was selected: the drag that may
                            // follow it builds a new one. Done here, once a frame, rather
                            // than inside the blocks, where a container drawn later would
                            // clear what an earlier one had just set.
                            let ink_owns_canvas = pen.pointer != ribbon::Pointer::Write;
                            let pressed = ui.input(|i| i.pointer.primary_pressed().then(|| i.pointer.interact_pos()).flatten());
                            let press_in_page = pressed.is_some_and(|pos| {
                                viewport.contains(pos) && ui.ctx().layer_id_at(pos).is_none_or(|layer| layer.order < egui::Order::Foreground)
                            });
                            if press_in_page && !ink_owns_canvas {
                                ed.across = None;
                            }
                            // A page without a saved arrangement is one responsive sheet of
                            // text. Keep its default box aligned with the available page width.
                            if !ed.arranged && ed.boxes.len() == 1 {
                                ed.boxes[0].width = crate::containers::responsive_default_width(ui.available_width(), zoom_scale);
                            }
                            if let Some(selection) = ed.restore_format_selection.take() {
                                ui.data_mut(|data| data.insert_temp(pageedit::restore_selection_id(), selection));
                            }
                            let before_caret = ed.pending_format.as_ref().and_then(|_| ed.caret.clone());
                            let mut moved = if ink_owns_canvas {
                                crate::containers::show_ink_view(
                                    ui,
                                    &ed.blocks,
                                    &mut ed.boxes,
                                    &ed.dir,
                                    &tokens,
                                    page_dir,
                                    &ed.justified,
                                    &ed.columns,
                                    if hide_title { "" } else { &shown_title },
                                    zoom_scale,
                                )
                            } else {
                                crate::containers::show(
                                    ui,
                                    &mut ed.blocks,
                                    &mut ed.boxes,
                                    &mut ed.caret,
                                    &ed.dir,
                                    &tokens,
                                    page_dir,
                                    &mut ed.columns,
                                    &mut ed.justified,
                                    &mut ed.across,
                                    if hide_title { "" } else { &shown_title },
                                    zoom_scale,
                                    snap,
                                )
                            };
                            if moved.changed.blocks && ui.input(|input| input.events.iter().any(|event| matches!(event, egui::Event::Text(_)))) {
                                let _ = ed.apply_pending_format(before_caret.as_ref());
                            } else if let Some(pending) = ed.pending_format.as_ref()
                                && ed.caret.as_ref().is_none_or(|caret| caret.block != pending.block || caret.selection != (pending.at, pending.at))
                            {
                                ed.pending_format = None;
                            }
                            page_acts.append(&mut moved.acts);
                            // Collect All (issue #23): the boxes and the strokes of ink, each as the
                            // rectangle it covers, gathered so no empty stretch of the page between
                            // them is wider than a small gap. Nothing moves past anything else.
                            if collect_now {
                                let strokes = pad.as_ref().map(crate::inkpad::Pad::stroke_bounds).unwrap_or_default();
                                let items: Vec<egui::Rect> = moved
                                    .frames
                                    .iter()
                                    .copied()
                                    .chain(strokes.iter().map(|[x0, y0, x1, y1]| egui::Rect::from_min_max(egui::pos2(*x0, *y0), egui::pos2(*x1, *y1))))
                                    .collect();
                                let by = crate::collect::gather(&items, COLLECT_GAP);
                                for (i, d) in by.iter().take(moved.frames.len()).enumerate() {
                                    if let Some(b) = ed.boxes.get_mut(i)
                                        && *d != egui::Vec2::ZERO
                                    {
                                        *b = ergonote_model::container::Container::new(b.x + d.x, b.y + d.y, b.width, b.blocks);
                                        moved.boxes = true;
                                    }
                                }
                                let ink: Vec<(f32, f32)> = by.iter().skip(moved.frames.len()).map(|d| (d.x, d.y)).collect();
                                if let Some(pad) = pad.as_mut() {
                                    ink_changed |= pad.move_each(&ink);
                                }
                            }
                            // The page's column, its boxes and its ink: what the sideways bar
                            // has to reach.
                            {
                                let left = moved.origin.x;
                                let mut span = (left, (left + moved.right * zoom_scale).max(left + ui.min_rect().width()));
                                if let Some([x0, _, x1, _]) = pad.as_ref().and_then(|p| p.bounds()) {
                                    span = (span.0.min(top.x + x0 * zoom_scale), span.1.max(top.x + x1 * zoom_scale));
                                }
                                page_span = Some(span);
                            }
                            // Insert Space (issue #164): a drag down the page pushes everything that
                            // starts below where it began, boxes and ink together, down by as far as
                            // it went; a drag up pulls it back up. One drag, then the tool is put down.
                            if space_on {
                                let visible = ui.clip_rect().intersect(viewport.with_max_y(viewport.max.y.max(ui.clip_rect().max.y)));
                                if let Some((from, by)) = insert_space_overlay(ui.ctx(), visible, &tokens) {
                                    let z = zoom_scale.max(0.01);
                                    let (at, by) = ((from - moved.origin.y) / z, by / z);
                                    let below: Vec<usize> = (0..ed.boxes.len()).filter(|i| ed.boxes.get(*i).is_some_and(|b| b.y >= at)).collect();
                                    // Room taken away never pulls anything above the top of the page.
                                    let highest = below.iter().filter_map(|i| ed.boxes.get(*i)).map(|b| b.y).fold(f32::INFINITY, f32::min);
                                    let by = if by < 0.0 && highest.is_finite() { by.max(-highest) } else { by };
                                    for i in &below {
                                        if let Some(b) = ed.boxes.get_mut(*i) {
                                            *b = ergonote_model::container::Container::new(b.x, b.y + by, b.width, b.blocks);
                                        }
                                    }
                                    if !below.is_empty() && by != 0.0 {
                                        moved.boxes = true;
                                    }
                                    if let Some(pad) = pad.as_mut() {
                                        ink_changed |= pad.shift_below(at, by);
                                    }
                                    space_over = true;
                                }
                                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                    space_over = true;
                                }
                            }
                            if textbox_on {
                                let visible = ui.clip_rect().intersect(viewport.with_max_y(viewport.max.y.max(ui.clip_rect().max.y)));
                                if let Some(pos) = insert_textbox_overlay(ui.ctx(), visible, &tokens, zoom_scale) {
                                    let z = zoom_scale.max(0.01);
                                    textbox_placed = Some(((pos.x - moved.origin.x) / z, (pos.y - moved.origin.y) / z));
                                }
                                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                    textbox_cancelled = true;
                                }
                            }
                            // The selection box takes ink as well as boxes, as in OneNote, and what
                            // it took moves together: boxes dragged take the ink with them, ink
                            // dragged takes the boxes.
                            if let Some(pad) = pad.as_mut()
                                && !ink_owns_canvas
                            {
                                if moved.selection_started {
                                    pad.let_go();
                                }
                                if let Some(shown) = moved.selection_box {
                                    let to_ink = |p: egui::Pos2| ((p.x - top.x) / zoom_scale.max(0.01), (p.y - top.y) / zoom_scale.max(0.01));
                                    let (a, b) = (to_ink(shown.min), to_ink(shown.max));
                                    pad.hold_touching((a.0, a.1, b.0, b.1));
                                }
                                if let Some((dx, dy)) = moved.selected_shift {
                                    pad.move_held(dx, dy);
                                    ink_changed |= pad.holding() > 0;
                                }
                                if let Some((dx, dy)) = pad.take_shift() {
                                    for i in crate::containers::selected_boxes(ui.ctx(), &ed.dir) {
                                        if let Some(c) = ed.boxes.get_mut(i) {
                                            *c = ergonote_model::container::Container::new(c.x + dx, c.y + dy, c.width, c.blocks);
                                            moved.boxes = true;
                                        }
                                    }
                                }
                                // A press anywhere else on the page lets go of the ink, unless it
                                // went on something still selected.
                                let on_held = pressed.is_some_and(|p| crate::inkpad::held_area(ui.ctx()).is_some_and(|r| r.contains(p)));
                                let ctrl_press = ui.input(|i| {
                                    i.events.iter().any(|e| {
                                        matches!(e, egui::Event::PointerButton { pressed: true, modifiers, .. } if modifiers.command || modifiers.ctrl)
                                    })
                                });
                                if press_in_page
                                    && !on_held
                                    && !ctrl_press
                                    && moved.selection_box.is_none()
                                    && pad.holding() > 0
                                    && crate::containers::selected_boxes(ui.ctx(), &ed.dir).is_empty()
                                {
                                    pad.let_go();
                                }
                            }
                            if let Some((html, text)) = moved.copy.take()
                                && self.clipboard.set_html(&html, &text).is_err()
                            {
                                ui.ctx().copy_text(text);
                            }
                            // The ink of a page sits over its words, anywhere on it, the
                            // way a pen on paper does. It goes on after them so a press
                            // with a pen in hand reaches the ink and not the caret.
                            {
                                // The sheet, not the words: ink goes anywhere on the page,
                                // and the page is a sheet of paper whether or not anything
                                // is written that far down it. Measuring the ink's area
                                // from where the text happens to end meant the pen stopped
                                // at the last line, which is not what a sheet of paper
                                // does. The room is claimed so the page can be scrolled
                                // into it and so what is drawn there is never cut off.
                                let used = ui.cursor().min.y - top.y;
                                let want = total_sheets_px.max(crate::inkpad::SHEET_H).max(used + 120.0);
                                if want > used {
                                    ui.allocate_space(vec2(ui.available_width(), want - used));
                                }
                                if let Some(target) = scroll_to_sheet.take() {
                                    let target_y = sheet_top + (target.saturating_sub(1) as f32) * visual_sheet.size.y;
                                    ui.scroll_to_rect(
                                        egui::Rect::from_min_size(egui::pos2(sheet_left, target_y), vec2(10.0, 48.0)),
                                        Some(egui::Align::TOP),
                                    );
                                }
                                let body = viewport;
                                page_rect = Some(body);
                                // The page's own right-click menu. No widget is laid
                                // over the page for it: a widget that senses clicks would
                                // take the primary click away from the words, the ink and
                                // the caret, and one that does not sense clicks cannot
                                // carry a context menu at all. So the secondary click is
                                // watched for directly and the menu drawn where it fell.
                                crate::imagemenu::show(ui, body, &mut page_acts, &tokens);
                                let in_table = ed
                                    .caret
                                    .as_ref()
                                    .is_some_and(|c| c.cell.is_some() && matches!(ed.blocks.get(c.block), Some(ergonote_formats::markdown::Block::Table { .. })));
                                crate::pagemenu::show(ui, body, in_table, &mut page_acts, &tokens);
                                self.selected_image_block =
                                    crate::containers::selected_object(ui.ctx(), &ed.dir).and_then(|(i, kind)| (kind == "image").then_some(i));
                            }
                            if moved.changed.blocks {
                                ed.rebuild_body();
                                *from_blocks = true;
                            }
                            ed.content_bottom = Some(moved.bottom);
                            ed.shown_widths = std::mem::take(&mut moved.shown_widths);
                            if moved.boxes {
                                *boxes_moved = true;
                            }
                            if moved.changed.columns {
                                *cols_moved = true;
                            }
                            // Blocks went in or out before a justified paragraph: its mark
                            // moved with it, and is written into the page's file like the widths.
                            if moved.justified {
                                *justified_moved = true;
                            }
                            ed.origin = moved.origin;
                            ed.origin_in_canvas = Some(moved.origin - content_rect.min);
                            if moved.changed.link.is_some() {
                                *follow = moved.changed.link;
                            }
                            if let Some(table_block) = moved.changed.delete_table {
                                page_acts.push(ribbon::Act::ConfirmDeleteTable(Some(table_block)));
                            }
                        } else if wide {
                            // A page with markup the block model cannot hold: writing it back
                            // would drop that markup, so it is read here and changed in its
                            // source. See docs/gaps.md.
                            if let Some(href) = preview::show_zoomed(ui, &ed.blocks, &ed.dir, &tokens, page_dir, &ed.justified, zoom_scale) {
                                *follow = Some(href);
                            }
                            ui.add_space(14.0);
                            ui.label(
                                egui::RichText::new(crate::tl_id!("ui.this_page_holds_markup_ergonote_does_not_edit_yet_s_bc3b3b6a92"))
                                    .small()
                                    .color(tokens.muted),
                            );
                        } else if let Some(href) = preview::show_zoomed(ui, &ed.blocks, &ed.dir, &tokens, page_dir, &ed.justified, zoom_scale) {
                            *follow = Some(href);
                        }
                    });
                });
            };
            if show_source {
                // The code view: the page's files on the left, the one picked on the
                // right. The page itself is not drawn while it is open, so there is only
                // ever one editable copy of the page on screen.
                let named = pageedit::links_in(&ed.blocks);
                let entries = crate::codeview::files(&ed.dir, ed.format.page_file(), &named);
                let out = crate::codeview::show(ui, &mut code, &entries, &ed.dir, &tokens, height);
                wrote_file = out.wrote;
                file_action = out.file_action;
            } else {
                let ink_owns_canvas = self.pointer != ribbon::Pointer::Write;
                let touch_nav = pageedit::touch_navigation(ui.ctx());
                let dragging_handle = pageedit::is_dragging_selection_handle(ui.ctx());
                let scroll = egui::containers::scroll_area::ScrollSource {
                    drag: if ink_owns_canvas || dragging_handle {
                        egui::containers::scroll_area::DragScroll::Never
                    } else if touch_nav {
                        egui::containers::scroll_area::DragScroll::Always
                    } else {
                        egui::containers::scroll_area::DragScroll::Never
                    },
                    ..egui::containers::scroll_area::ScrollSource::default()
                };
                if touch_nav && !ink_owns_canvas && !dragging_handle && ui.input(|i| i.pointer.is_decidedly_dragging()) {
                    pageedit::cancel_center_on_caret(ui.ctx());
                }
                let page_outer = ui.available_rect_before_wrap();
                if touch_nav && !ink_owns_canvas && !dragging_handle {
                    pageedit::update_touch_gesture(ui.ctx(), page_outer);
                }
                let shown = egui::ScrollArea::vertical().id_salt("page-main").auto_shrink([false, false]).scroll_source(scroll).show(ui, |ui| {
                    // On a touch screen, one finger scrolls in browse/text mode and two fingers
                    // scroll even while an ink tool is active.
                    if let Some(touch) = ui.input(|i| i.multi_touch()) {
                        ui.scroll_with_delta(vec2(0.0, touch.translation_delta.y));
                    }
                    page(
                        ui,
                        ed,
                        true,
                        &mut changed,
                        &mut from_blocks,
                        &mut toggle_source,
                        &mut boxes_moved,
                        &mut cols_moved,
                        &mut justified_moved,
                        &mut follow,
                        &mut rename,
                    );
                });
                let scroll_style = ui.spacing().scroll;
                let scroll_edge = page_outer.max.x.min(ui.clip_rect().max.x);
                let max_cross = scroll_edge - scroll_style.bar_outer_margin;
                self.page_scroll_strip = (scroll.scroll_bar && shown.content_size.y > shown.inner_rect.height() + 0.5)
                    .then(|| egui::Rect::from_x_y_ranges((max_cross - scroll_style.bar_width)..=max_cross, shown.inner_rect.y_range()));
                if page_total_sheets > 0 && page_sheet_h_px > 1.0 {
                    let current_sheet = (((shown.state.offset().y + shown.inner_rect.height() * 0.35) / page_sheet_h_px).floor() as usize + 1)
                        .clamp(1, page_total_sheets);
                    self.page_sheets = (current_sheet, page_total_sheets);
                }
                if self.reading_ruler {
                    let view = shown.inner_rect;
                    if let Some(pos) = ui.ctx().pointer_latest_pos().filter(|p| view.contains(*p)) {
                        self.reading_ruler_y = Some(pos.y);
                    }
                    let cy =
                        self.reading_ruler_y.unwrap_or_else(|| view.center().y).clamp(view.min.y + 16.0, (view.max.y - 16.0).max(view.min.y + 16.0));
                    let band_h = (32.0 * zoom_scale).clamp(24.0, 56.0);
                    let band = egui::Rect::from_min_max(
                        egui::pos2(view.min.x, (cy - band_h * 0.5).max(view.min.y)),
                        egui::pos2(view.max.x, (cy + band_h * 0.5).min(view.max.y)),
                    );
                    let dark = tokens.editor.r() < 128;
                    let fill =
                        if dark { Color32::from_rgba_unmultiplied(250, 204, 21, 34) } else { Color32::from_rgba_unmultiplied(250, 204, 21, 56) };
                    let line_col = tokens.accent.linear_multiply(0.65);
                    let painter = ui.painter_at(view);
                    painter.rect_filled(band, 0.0, fill);
                    painter.hline(band.x_range(), band.min.y, egui::Stroke::new(1.5, line_col));
                    painter.hline(band.x_range(), band.max.y, egui::Stroke::new(1.5, line_col));
                }
                // Sideways the page is a canvas: when what is on it reaches past the view's
                // sides, a bar along the bottom shows it and moves the canvas (issue #188).
                if let Some(span) = page_span {
                    let view = shown.inner_rect;
                    let vertical = shown.content_size.y > view.height() + 0.5;
                    let reserve = if vertical { ui.spacing().scroll.bar_width + ui.spacing().scroll.bar_outer_margin } else { 0.0 };
                    hbar = crate::hscroll::show(ui.ctx(), egui::Id::new("page-hscroll"), view, span, reserve);
                }
            }
            if changed {
                Self::mark_dirty(ed);
            } else if from_blocks {
                Self::mark_dirty_from_blocks(ed);
            }
            // Before the step is remembered, so the page in the history is one that
            // names its ink: a box deleted with the ink line in it is repaired at once
            // rather than at the next open.
            if (changed || from_blocks) && Self::name_ink(ed) {
                from_blocks = true;
            }
            if changed || from_blocks || boxes_moved {
                Self::remember_step(ed);
            }
            // A container was dragged or resized: the arrangement is the page's own, so it
            // is written into the page's file rather than kept in the window.
            if boxes_moved {
                ed.arranged = true;
                ed.rebuild_body();
                Self::mark_dirty_from_blocks(ed);
            }
            // A column was dragged wider or narrower. Widths are layout, like the boxes,
            // so they go into the page's own file beside them.
            if cols_moved {
                let path = path_str(&ed.dir);
                let line = ergonote_model::tablecols::write(&ed.columns);
                save_cols = Some((path, line));
            }
            if justified_moved {
                let path = path_str(&ed.dir);
                let line = ergonote_model::justify::write(&ed.justified);
                save_justified = Some((path, line));
            }
        }
        self.cursor = cursor;
        if let Some((path, line)) = save_cols {
            self.run("page.columns", &params(&[("path", &path), ("widths", &line)]));
        }
        if let Some((path, line)) = save_justified {
            self.run("page.justify", &params(&[("path", &path), ("blocks", &line)]));
        }
        self.code = code;
        if let Some((name, text)) = wrote_file {
            self.write_page_file(&name, &text);
        }
        if let Some(action) = file_action {
            match action {
                crate::codeview::FileAction::CopyPath(path) => {
                    ui.ctx().copy_text(path);
                    self.say(crate::tl_id!("ui.full_path_copied"));
                }
                crate::codeview::FileAction::OpenFolder(path) => match open_folder(&path) {
                    Ok(()) => self.say(crate::tl_id!("ui.opened_the_containing_folder")),
                    Err(e) => {
                        self.complain(&crate::i18n::format_message_id("ui.could_not_open_the_containing_folder_error", &[("error", &e.to_string())]))
                    }
                },
                crate::codeview::FileAction::OpenShell(path) => {
                    if self.open_attachment_warning_suppressed {
                        match open_file_for_viewer(&path) {
                            Ok(()) => self.say(crate::tl_id!("ui.opened_using_the_associated_application")),
                            Err(e) => {
                                self.complain(&crate::i18n::format_message_id("ui.could_not_open_the_file_error", &[("error", &e.to_string())]))
                            }
                        }
                    } else {
                        self.dialog = Some(Dialog::OpenAttachmentWarning { path });
                    }
                }
                crate::codeview::FileAction::Delete(name) => {
                    log::debug!("source view delete action received: file={name:?}");
                    let page = self.editor.as_ref().map(|e| e.dir.clone());
                    if let Some(page) = page {
                        self.dialog = Some(Dialog::DeleteFile { page, name });
                    }
                }
                crate::codeview::FileAction::AddFile => {
                    if let Some(dir) = self.editor.as_ref().map(|ed| ed.dir.clone())
                        && let Some(file) = rfd::FileDialog::new().set_title(crate::tl_id!("ui.add_file_to_this_page")).pick_file()
                    {
                        self.dialog = Some(Dialog::FileOptions { dir, files: vec![file] });
                    }
                }
            }
        }
        if toggle_source {
            self.set_show_source(!show_source);
        }
        if toggle_full_page {
            self.full_page = !full_page;
        }
        if focus_search {
            self.open_search();
        }
        // The drawing comes back from the page, and a stroke laid down this frame is
        // written at once: ink is its own file, so it does not wait for the page to save.
        self.pad = pad;
        if page_rect.is_some() {
            self.page_rect = page_rect;
        }
        self.page_hbar = hbar.track.zip(hbar.handle);
        if hbar.pan != 0.0 && hbar.pan.is_finite() {
            self.canvas_pan.x = (self.canvas_pan.x + hbar.pan).clamp(-1.0e6, 1.0e6);
            ui.ctx().request_repaint();
        }
        if space_over {
            self.space_tool = false;
        }
        if textbox_cancelled {
            self.textbox_tool = false;
        }
        if let Some((tx, ty)) = textbox_placed {
            self.textbox_tool = false;
            self.place_textbox_at(tx, ty);
        }
        for act in page_acts {
            self.ribbon_act(act);
        }
        if ink_changed {
            self.last_ink_change = Some(Instant::now());
        }
        // A change to the ink is finished once no button is down: what it was before becomes a
        // step undo can go back to.
        if let Some(pad) = self.pad.as_mut()
            && !ui.input(|i| i.pointer.any_down())
        {
            pad.settle();
        }
        if ink_changed && let Some(pad) = self.pad.as_mut() {
            let strokes = pad.strokes();
            match pad.save() {
                Ok(()) => self.say(crate::i18n::format_message_id("ui.drawing_saved_count_strokes", &[("count", &strokes.to_string())])),
                Err(e) => self.complain(&crate::i18n::format_message_id("ui.cannot_write_the_drawing_error", &[("error", &e.to_string())])),
            }
            self.tell_kept_ink();
            self.sync_ink_reference();
        }
        if let Some(href) = follow {
            self.follow_link(ui.ctx(), &href);
        }
        if let Some(t) = rename
            && let Some(dir) = self.editor.as_ref().map(|e| e.dir.clone())
            && self.run("node.title", &params(&[("path", &path_str(&dir)), ("title", &t)])).is_some()
        {
            if let Some(ed) = self.editor.as_mut() {
                ed.titled = !t.is_empty();
                ed.title = if t.is_empty() { ergonote_store::tree::UNTITLED_PAGE.to_string() } else { t };
            }
            self.say(crate::tl_id!("ui.title_saved"));
        }
    }

    fn saved_caption(&self, saved_at: Option<&str>) -> String {
        if self.session_only_storage {
            crate::tl_id!("ui.kept_in_this_tab").into()
        } else {
            saved_at
                .map(|time| {
                    let shown = ergonote_model::time::clock_from_iso(time).unwrap_or_else(|| time.to_owned());
                    crate::i18n::format_message_id("ui.saved_time", &[("time", &shown)])
                })
                .unwrap_or_else(|| crate::tl_id!("ui.saved").into())
        }
    }

    fn saved_extra_sheets(&self, dir: &Path) -> usize {
        let key = format!("extra_sheets.{}", dir.display());
        self.session.settings.get(&key).and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(0)
    }

    fn remember_extra_sheets_for(&mut self, dir: &Path, count: usize) {
        let key = format!("extra_sheets.{}", dir.display());
        if let Err(err) = self.session.settings.set(&key, &count.to_string()) {
            log::warn!("could not save extra_sheets for {}: {err}", dir.display());
        }
    }

    fn remember_extra_sheets(&mut self) {
        if let Some(dir) = self.editor.as_ref().map(|ed| ed.dir.clone()) {
            self.remember_extra_sheets_for(&dir, self.extra_sheets);
        }
    }

    fn page_save_state(&self, ed: &Editor) -> String {
        if ed.dirty_since.is_some() || ed.saving.is_some() {
            crate::tl_id!("ui.editing").into()
        } else {
            self.saved_caption(ed.last_saved.as_deref())
        }
    }

    fn storage_notice(&self, ui: &mut Ui) {
        if self.session_only_storage {
            egui::Panel::top("session-storage-notice").show(ui, |ui| {
                ui.label(egui::RichText::new(crate::tl_id!("ui.session_only_storage_notice")).color(self.tokens.warn));
            });
        }
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        if self.is_simple_mobile() {
            let in_page = !self.compact_list_fills() && self.editor.is_some();
            if self.status_is_error && !self.status.trim().is_empty() {
                egui::Panel::bottom("status").show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Label::new(egui::RichText::new(crate::i18n::pseudo_dynamic(&self.status)).small().color(self.tokens.warn))
                                .truncate(),
                        );
                    });
                });
            } else if in_page && let Some(ed) = self.editor.as_ref() {
                let saving = ed.dirty_since.is_some() || ed.saving.is_some();
                let caption = if saving { "Enregistrement…" } else { "✓ Enregistré" };
                egui::Panel::bottom("status").show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(caption).small().color(self.tokens.muted));
                    });
                });
            }
            return;
        }
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                let color = if self.status_is_error { self.tokens.warn } else { self.tokens.muted };
                ui.label(egui::RichText::new(crate::i18n::pseudo_dynamic(&self.status)).color(color));
                let editor_state = self.editor.as_ref().map(|ed| self.page_save_state(ed));
                if let Some(state) = editor_state {
                    ui.separator();
                    ui.label(egui::RichText::new(state).small().color(self.tokens.muted));
                    ui.separator();
                    let (cur_sheet, total_sheets) = self.page_sheets;
                    if ui.add_enabled(cur_sheet > 1, egui::Button::new("<").small()).on_hover_text("Feuille précédente").clicked() {
                        self.scroll_to_sheet = Some(cur_sheet - 1);
                        ui.ctx().request_repaint();
                    }
                    ui.label(egui::RichText::new(format!("Feuille {cur_sheet} / {total_sheets}")).small().strong().color(self.tokens.text));
                    if ui.add_enabled(cur_sheet < total_sheets, egui::Button::new(">").small()).on_hover_text("Feuille suivante").clicked() {
                        self.scroll_to_sheet = Some(cur_sheet + 1);
                        ui.ctx().request_repaint();
                    }
                    if ui.small_button("+ Feuille").on_hover_text("Ajouter une feuille supplémentaire en dessous").clicked() {
                        self.extra_sheets = total_sheets;
                        self.remember_extra_sheets();
                        self.scroll_to_sheet = Some(total_sheets + 1);
                        ui.ctx().request_repaint();
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.status_bar_zoom(ui);
                });
            });
        });
    }

    /// Word-style zoom bar in the bottom-right corner of the desktop status bar.
    fn status_bar_zoom(&mut self, ui: &mut Ui) {
        let t = self.tokens;
        let enabled = ui.is_enabled();
        ui.spacing_mut().item_spacing.x = 3.0;

        // 1. Farthest right: percentage button (resets to 100%).
        // i18n-exempt: percentage is a numeric zoom value, not translated copy.
        let pct_text = format!("{}%", self.zoom_percent);
        let (pct_rect, pct_resp) = ui.allocate_exact_size(vec2(46.0, 20.0), Sense::click());
        let pct_painter = ui.painter_at(pct_rect);
        if pct_resp.hovered() {
            pct_painter.rect_filled(pct_rect, 4.0, t.row_hover);
        }
        pct_painter.text(pct_rect.center(), Align2::CENTER_CENTER, pct_text, egui::FontId::proportional(11.5), t.text);
        pct_resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, crate::tl_id!("ui.zoom_to_100")));
        if pct_resp.on_hover_text(crate::tl_id!("ui.show_the_page_at_100")).clicked() {
            self.ribbon_act(ribbon::Act::Zoom(0));
        }

        // 2. Zoom In (+).
        let (in_rect, in_resp) = ui.allocate_exact_size(vec2(22.0, 20.0), Sense::click());
        let in_painter = ui.painter_at(in_rect);
        if in_resp.hovered() {
            in_painter.rect_filled(in_rect, 4.0, t.row_hover);
        }
        let ic = in_rect.center();
        let stroke = egui::Stroke::new(1.5, t.text);
        in_painter.line_segment([egui::pos2(ic.x - 4.0, ic.y), egui::pos2(ic.x + 4.0, ic.y)], stroke);
        in_painter.line_segment([egui::pos2(ic.x, ic.y - 4.0), egui::pos2(ic.x, ic.y + 4.0)], stroke);
        in_resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, crate::tl_id!("ui.zoom_in")));
        if in_resp.on_hover_text(crate::tl_id!("ui.make_the_page_larger")).clicked() {
            self.ribbon_act(ribbon::Act::Zoom(1));
        }

        // 3. Word-style slider (50%..250% with 100% centered).
        let (slider_rect, slider_resp) = ui.allocate_exact_size(vec2(104.0, 20.0), Sense::click_and_drag());
        let s_painter = ui.painter_at(slider_rect);
        let track_left = slider_rect.min.x + 7.0;
        let track_right = slider_rect.max.x - 7.0;
        let track_w = (track_right - track_left).max(1.0);
        let cy = slider_rect.center().y;
        let share = zoom_slider_share(self.zoom_percent);
        let knob_x = track_left + track_w * share;
        let center_x = track_left + track_w * 0.5;
        s_painter.line_segment([egui::pos2(track_left, cy), egui::pos2(track_right, cy)], egui::Stroke::new(2.0, t.container_border));
        s_painter.line_segment([egui::pos2(track_left, cy), egui::pos2(knob_x, cy)], egui::Stroke::new(2.0, t.accent));
        s_painter.line_segment([egui::pos2(center_x, cy - 4.0), egui::pos2(center_x, cy + 4.0)], egui::Stroke::new(1.2, t.muted));
        let knob_r = if slider_resp.hovered() || slider_resp.dragged() { 6.0 } else { 5.0 };
        s_painter.circle(egui::pos2(knob_x, cy), knob_r, t.editor, egui::Stroke::new(1.6, t.accent));
        if (slider_resp.clicked() || slider_resp.dragged())
            && let Some(pos) = slider_resp.interact_pointer_pos()
        {
            let picked = zoom_slider_percent((pos.x - track_left) / track_w);
            if picked != self.zoom_percent {
                self.ribbon_act(ribbon::Act::ZoomTo(picked));
            }
        }
        let zoom_val = f64::from(self.zoom_percent);
        slider_resp.widget_info(move || egui::WidgetInfo::slider(enabled, zoom_val, crate::tl_id!("ui.zoom")));
        slider_resp.on_hover_text(format!("{}: {}%", crate::tl_id!("ui.zoom"), self.zoom_percent));

        // 4. Zoom Out (-).
        let (out_rect, out_resp) = ui.allocate_exact_size(vec2(22.0, 20.0), Sense::click());
        let out_painter = ui.painter_at(out_rect);
        if out_resp.hovered() {
            out_painter.rect_filled(out_rect, 4.0, t.row_hover);
        }
        let oc = out_rect.center();
        out_painter.line_segment([egui::pos2(oc.x - 4.0, oc.y), egui::pos2(oc.x + 4.0, oc.y)], stroke);
        out_resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, crate::tl_id!("ui.zoom_out")));
        if out_resp.on_hover_text(crate::tl_id!("ui.make_the_page_smaller")).clicked() {
            self.ribbon_act(ribbon::Act::Zoom(-1));
        }

        // 5. Leftmost of the cluster: Page Width toggle.
        let (pw_rect, pw_resp) = ui.allocate_exact_size(vec2(24.0, 20.0), Sense::click());
        let pw_painter = ui.painter_at(pw_rect);
        let fit_on = self.fit_page_width;
        if fit_on {
            pw_painter.rect_filled(pw_rect, 4.0, t.row_selected);
            pw_painter.rect_stroke(pw_rect, 4.0, egui::Stroke::new(1.0, t.accent), egui::StrokeKind::Inside);
        } else if pw_resp.hovered() {
            pw_painter.rect_filled(pw_rect, 4.0, t.row_hover);
        }
        icons::paint_ribbon(
            ui,
            egui::Rect::from_center_size(pw_rect.center(), vec2(15.0, 15.0)),
            icons::Icon::PageWidth,
            if fit_on { t.accent } else { t.text },
        );
        pw_resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, fit_on, crate::tl_id!("ui.page_width")));
        if pw_resp.on_hover_text(crate::tl_id!("ui.fit_the_page_to_the_available_width")).clicked() {
            self.ribbon_act(ribbon::Act::PageWidth);
        }
    }

    /// Draws the whole window. Call once per frame.
    /// Fills the strips the platform's own bars lie over (a phone's status bar and navigation
    /// bar, reported as egui's safe area) with the title band's colour, so the system's white
    /// icons read on it in a light theme as well as a dark one. Everything else is laid out
    /// inside the safe area. On a desktop there is none, and nothing is drawn.
    fn paint_system_bars(&self, ctx: &Context) {
        let strips = self.system_bar_strips(ctx);
        if strips.is_empty() {
            return;
        }
        // On the background layer, which egui always paints, clipped to the whole window rather
        // than to the safe area as a layer painter is by default. Nothing else is drawn under
        // the bars: every panel and window is inside the safe area.
        let painter = egui::Painter::new(ctx.clone(), egui::LayerId::background(), ctx.viewport_rect());
        for (strip, colour) in strips {
            painter.rect_filled(strip, 0.0, colour);
        }
    }

    /// The strips [`Self::paint_system_bars`] fills, with their colour: the parts of the window
    /// outside egui's safe area. None on a desktop.
    pub fn system_bar_strips(&self, ctx: &Context) -> Vec<(egui::Rect, egui::Color32)> {
        let (outer, inner) = (ctx.viewport_rect(), ctx.content_rect());
        let strips = [
            egui::Rect::from_min_max(outer.min, egui::pos2(outer.max.x, inner.min.y)),
            egui::Rect::from_min_max(egui::pos2(outer.min.x, inner.max.y), outer.max),
            egui::Rect::from_min_max(egui::pos2(outer.min.x, inner.min.y), egui::pos2(inner.min.x, inner.max.y)),
            egui::Rect::from_min_max(egui::pos2(inner.max.x, inner.min.y), egui::pos2(outer.max.x, inner.max.y)),
        ];
        strips.into_iter().filter(|r| r.is_positive()).map(|r| (r, self.tokens.band)).collect()
    }

    pub fn show(&mut self, ui: &mut Ui) {
        // egui's root `Ui` covers the whole window, the parts under a phone's status and
        // navigation bars included. Everything is laid out inside the safe area instead
        // (issue #315); on a desktop the two are the same.
        let ctx = ui.ctx().clone();
        let content = ctx.content_rect();
        if content != ctx.viewport_rect() {
            self.paint_system_bars(&ctx);
            let room = ui.max_rect().intersect(content);
            ui.scope_builder(egui::UiBuilder::new().id_salt("safe-area").max_rect(room), |ui| self.show_in(ui));
        } else {
            self.show_in(ui);
        }
    }

    fn show_in(&mut self, ui: &mut Ui) {
        crate::i18n::set_current(&self.ui_language);
        let ctx = ui.ctx().clone();
        let ctx = &ctx;
        self.poll_dictation();
        if self.tts_done.swap(false, std::sync::atomic::Ordering::SeqCst) {
            self.tts_state = TtsState::Stopped;
            if let Ok(mut sync) = self.tts_sync.lock() {
                sync.preparing = false;
                sync.playback_started_at = None;
                sync.direct_word_index = None;
            }
        }
        if self.tts_state == TtsState::Playing {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        let tts_active_highlight = match self.tts_sync_snapshot() {
            (false, Some(idx), cues) => cues.get(idx).and_then(|cue| cue.block.map(|b| (b, cue.caret_chars, cue.drawn_bytes))),
            _ => None,
        };
        ctx.data_mut(|d| {
            d.insert_temp(Id::new("ergonote-lock-images"), self.lock_images);
            d.insert_temp(Id::new("ergonote-dys-spaced"), self.dys_spaced);
            d.insert_temp(pageedit::spell_lang_id(), self.spell_lang);
            d.insert_temp(pageedit::word_prediction_enabled_id(), self.word_prediction_enabled);
            d.remove::<(usize, egui::Rect)>(pageedit::caret_screen_pos_id());
            if let Some(hl) = tts_active_highlight {
                d.insert_temp(pageedit::tts_highlight_id(), hl);
            } else {
                d.remove::<(usize, (usize, usize), (usize, usize))>(pageedit::tts_highlight_id());
            }
        });
        // Search owns Escape before text widgets and the page can consume it.
        if self.dialog.is_none() && !self.search.is_empty() && ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
            self.dismiss_search();
            ctx.memory_mut(|memory| memory.surrender_focus(Id::new("page-search")));
        }
        if !self.loaders_installed {
            crate::pictures::install(ctx);
            self.loaders_installed = true;
        }
        // Arabic and the other right-to-left scripts have no glyphs in egui's own faces.
        crate::fonts::install_once(ctx);
        // Pen pressure, where the platform reports it. Nothing draws with it yet; it is
        // read here so `ui.inspect` can say whether a pen reaches this window at all.
        ctx.input(|i| self.stylus.update(&i.events));
        // An update is ready to be put in place: save the page and close, so the script that
        // waits for this window can replace the program and start the new one.
        if !self.update_closing && self.update.lock().is_ok_and(|s| matches!(*s, crate::update::Status::Restart { .. })) {
            self.update_closing = true;
            self.save();
            log::info!("update: closing so the new version can be put in place");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.apply_theme(ctx);
        // Only when the size was chosen or read at start: egui's own Ctrl+Plus and Ctrl+Minus
        // change the same factor, and are left alone in between.
        if self.ui_scale_pending {
            ctx.set_zoom_factor(f32::from(self.ui_scale) / 100.0);
            self.ui_scale_pending = false;
        }
        if self.window_level_pending {
            let level = if self.always_on_top { egui::viewport::WindowLevel::AlwaysOnTop } else { egui::viewport::WindowLevel::Normal };
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
            self.window_level_pending = false;
        }
        if let Some(text) = self.pending_copy.take() {
            if let Some(html) = self.pending_copy_html.take() {
                if self.clipboard.set_html(&html, &text).is_err() {
                    ctx.copy_text(text);
                }
            } else {
                ctx.copy_text(text);
            }
        }
        if self.keyboard_paste_requested {
            self.keyboard_paste_requested = false;
            log::debug!("queued keyboard paste dispatched to paste_here");
            self.paste_here();
        }
        // A button whose file dialog Android or Web cannot show says so (filepicker.android.rs).
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        if let Some(notice) = crate::filepicker::take_notice() {
            self.say(notice);
        }
        self.clipboard_keys(ctx);
        self.poll_control(ctx);
        self.poll_import(ctx);
        self.poll_ocr(ctx);
        if self.selected_image_block != self.last_selected_image {
            if self.selected_image_block.is_some() {
                self.ribbon_tab = ribbon::IMAGE_TAB.to_string();
            }
            self.last_selected_image = self.selected_image_block;
        }
        if !self.event_frames.is_empty() {
            ctx.request_repaint();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::S)) {
            if let Some(ed) = self.editor.as_mut() {
                ed.dirty_since.get_or_insert_with(Instant::now);
            }
            self.save_now();
        }
        // Taken before any widget sees them: a text box has an undo of its own that
        // knows only about the letters in that one box, and the page's undo has to be the
        // one that answers, or Ctrl+Z would mean two different things on one page.
        let ctrl_shift = Modifiers { ctrl: true, shift: true, command: true, ..Modifiers::NONE };
        // Not while a dialog is open: it owns the keyboard, and the page behind it must not
        // change unseen (issue #262: Ctrl+Z undid and saved an edit behind Insert symbol). A
        // text box in the dialog keeps its own undo.
        let page_keys = self.dialog.is_none();
        if page_keys && ctx.input_mut(|i| i.consume_key(ctrl_shift, Key::Z) || i.consume_key(Modifiers::COMMAND, Key::Y)) {
            // The ink's as well as the page's, whichever Undo took back.
            self.redo();
        } else if page_keys && ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Z)) {
            // The ink's changes as well as the page's: whichever was last, or both when one
            // press made them together.
            self.undo();
        }
        // Ctrl+F, and OneNote's Ctrl+E, go to the search box. Taken before the page sees
        // them: with the caret in the page, the key went nowhere and the words typed to find
        // something were written into the note (issue #134). The code view has a Find of its
        // own for the file it shows, so it keeps Ctrl+F while it is open.
        // OneNote's Ctrl+K: the Link dialog for the words picked or the link the caret is in.
        if !self.show_source && self.dialog.is_none() && self.editor.is_some() && ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::K)) {
            self.open_link_dialog();
        }
        if !self.show_source && ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::F) || i.consume_key(Modifiers::COMMAND, Key::E)) {
            if let Some(caret) = self.editor.as_mut().and_then(|ed| ed.caret.as_mut()) {
                caret.focus = false;
            }
            self.focus_search = true;
        }
        self.nav_keys(ctx);
        self.replay_typing(ctx);
        self.switch_by_device(ctx);
        self.pen_keys(ctx);
        self.direction_keys(ctx);
        self.hierarchy_keys(ctx);
        self.restore_last_page();
        self.attach_dropped(ctx);
        // The File screen takes the whole window while it is open, as it does in Office.
        if self.backstage.is_some() {
            self.title_band(ui, false);
            self.storage_notice(ui);
            self.backstage_screen(ui);
            self.dialogs(ctx);
            self.tts_overlay(ctx);
            self.dictation_overlay(ctx);
            self.import_progress(ctx);
            self.frame_edges(ctx);
            return;
        }
        self.compact_layout(ctx);
        self.top_bar(ui);
        self.storage_notice(ui);
        self.status_bar(ui);
        self.side_panel(ui);
        self.tags_panel(ui);
        self.templates_panel(ui);
        self.search_panel(ui);
        self.prediction_keys(ctx);
        // In a phone-width window a pane that fills it leaves no room for the page.
        if !self.compact_list_fills() && !(self.compact && (self.tags_pane.is_some() || self.templates_pane.is_some() || self.search_open)) {
            self.central(ui);
        }
        self.paint_formatting(ctx);
        self.refresh_unread(false);
        self.laser_keys(ctx);
        self.eyedropper(ctx);
        self.dialogs(ctx);
        self.show_word_prediction_popup(ctx);
        self.tts_overlay(ctx);
        self.dictation_overlay(ctx);
        self.import_progress(ctx);
        self.frame_edges(ctx);
        // Closing the window writes the page first, and waits for an autosave still being
        // written: what was typed in the moment before closing is not lost.
        if ctx.input(|i| i.viewport().close_requested()) {
            self.save_now();
        }
        self.take_saves(false);
        if self.saver.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        if let Some(since) = self.editor.as_ref().and_then(|e| e.dirty_since) {
            // Read once: read twice, the second could be past the wait, and the subtraction
            // below panicked (seen on a slow CI runner).
            let waited = since.elapsed();
            if waited >= AUTOSAVE_AFTER {
                self.save_in_background();
            } else {
                ctx.request_repaint_after(AUTOSAVE_AFTER.saturating_sub(waited));
            }
        }
        let minute_waited = self.last_minute_autosave.elapsed();
        if minute_waited >= MINUTE_AUTOSAVE_INTERVAL {
            self.last_minute_autosave = Instant::now();
            self.save_in_background();
            if let Some(pad) = self.pad.as_mut()
                && pad.dirty
            {
                let _ = pad.save();
            }
            if self.editor.is_some() {
                self.say(crate::tl_id!("ui.student_autosave_minute"));
            }
            ctx.request_repaint_after(MINUTE_AUTOSAVE_INTERVAL);
        } else {
            ctx.request_repaint_after(MINUTE_AUTOSAVE_INTERVAL.saturating_sub(minute_waited));
        }
    }
}

fn table_from_gridcraft_csv(text: &str) -> Option<(ergonote_formats::markdown::Block, usize, usize)> {
    let table = ergonote_formats::csv::to_table(text)?;
    let (rows, columns) = pageedit::table_size(&table).unwrap_or((0, 0));
    Some((table, rows, columns))
}

#[cfg(all(any(target_os = "linux", target_os = "macos"), not(test)))]
fn transcribe_audio_file_cmd(wav: &str, locale: &str, lang_code: &str) -> Result<String, String> {
    if !std::path::Path::new(wav).is_file() {
        return Err("Aucun enregistrement audio capturé.".to_string());
    }
    // 1) Try offline whisper-cli / whisper.
    for (bin, args) in [
        ("whisper-cli", vec!["-l", lang_code, "-nt", "--no-prints", "-f", wav]),
        ("whisper", vec![wav, "--language", lang_code, "--output_format", "txt", "--output_dir", "/tmp"]),
    ] {
        if let Ok(out) = std::process::Command::new(bin).args(&args).output()
            && out.status.success()
        {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !stdout.is_empty() {
                return Ok(stdout);
            }
            let txt_path = "/tmp/ergonote_dictation.txt";
            if let Ok(txt) = std::fs::read_to_string(txt_path)
                && !txt.trim().is_empty()
            {
                let _ = std::fs::remove_file(txt_path);
                return Ok(txt.trim().to_string());
            }
        }
    }
    // 2) Try offline vosk-transcriber.
    if let Ok(out) = std::process::Command::new("vosk-transcriber").args(["-l", lang_code, "-i", wav]).output()
        && out.status.success()
    {
        let txt = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !txt.is_empty() {
            return Ok(txt);
        }
    }
    // 3) Fallback to Python3 SpeechRecognition if installed.
    let py = "import sys, speech_recognition as sr\nr = sr.Recognizer()\nwith sr.AudioFile(sys.argv[1]) as src:\n    audio = r.record(src)\ntry:\n    print(r.recognize_google(audio, language=sys.argv[2]))\nexcept Exception as e:\n    sys.stderr.write(str(e)); sys.exit(2)\n";
    if let Ok(out) = std::process::Command::new("python3").args(["-c", py, wav, locale]).output()
        && out.status.success()
    {
        let txt = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !txt.is_empty() {
            return Ok(txt);
        }
    }
    Err("Audio capturé, mais aucun moteur vocal trouvé (installez vosk-transcriber, whisper-cli ou python3-speechrecognition).".to_string())
}

/// Formats a directory path as a cross-platform `file://` base URL (supports both Linux `/...` and Windows `C:\...`).
fn file_base_href(dir: &Path) -> String {
    let raw = dir.to_string_lossy().replace('\\', "/");
    let trimmed = raw.trim_end_matches('/');
    if trimmed.starts_with('/') { format!("file://{trimmed}/") } else { format!("file:///{trimmed}/") }
}

fn open_url_in_system(url: &str) -> std::io::Result<()> {
    if cfg!(target_os = "windows") {
        std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn().map(|_| ())
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn().map(|_| ())
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn().map(|_| ())
    }
}

/// Cross-platform pure-Rust ZIP backup of the entire notebooks root directory.
fn write_notebooks_zip(root: &Path, dest: &Path) -> Result<usize, String> {
    let file = ergonote_store::vfs::File::create(dest).map_err(|e| e.to_string())?;
    write_notebooks_zip_to_writer(root, file)
}

fn write_notebooks_zip_to_writer<W: std::io::Write + std::io::Seek>(root: &Path, sink: W) -> Result<usize, String> {
    use std::io::Write;
    let mut writer = zip::ZipWriter::new(sink);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut count = 0usize;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = ergonote_store::vfs::read_dir(&dir).map_err(|e| e.to_string())?;
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str == "_impression.html" || name_str.ends_with(".tmp") {
                continue;
            }
            if ergonote_store::vfs::is_dir(&path) {
                stack.push(path);
            } else if ergonote_store::vfs::is_file(&path) {
                let rel = path.strip_prefix(root).map_err(|e| e.to_string())?;
                let zip_name = rel.to_string_lossy().replace('\\', "/");
                writer.start_file(&zip_name, options).map_err(|e| e.to_string())?;
                let bytes = ergonote_store::vfs::read(&path).map_err(|e| e.to_string())?;
                writer.write_all(&bytes).map_err(|e| e.to_string())?;
                count += 1;
            }
        }
    }
    writer.finish().map_err(|e| e.to_string())?;
    Ok(count)
}

/// Cross-platform pure-Rust ZIP extraction into the notebooks root directory.
fn extract_notebooks_zip(src: &Path, dest_root: &Path) -> Result<usize, String> {
    let file = ergonote_store::vfs::File::open(src).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    ergonote_store::vfs::create_dir_all(dest_root).map_err(|e| e.to_string())?;
    let mut count = 0usize;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        let out_path = dest_root.join(rel);
        if entry.is_dir() {
            ergonote_store::vfs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = out_path.parent() {
                ergonote_store::vfs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut out_file = ergonote_store::vfs::File::create(&out_path).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut out_file).map_err(|e| e.to_string())?;
            count += 1;
        }
    }
    Ok(count)
}

/// One swatch of the theme switcher: the surface colour, ringed and dotted in the accent.
fn theme_swatch(ui: &mut Ui, theme: Theme, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
    let t = theme.tokens();
    let painter = ui.painter_at(rect);
    let c = rect.center();
    painter.circle_filled(c, 10.5, t.panel);
    painter.circle_stroke(c, 10.5, egui::Stroke::new(if selected { 3.0 } else { 1.5 }, t.accent));
    painter.circle_filled(c, 4.5, t.accent);
    if resp.hovered() {
        painter.circle_stroke(c, 12.0, egui::Stroke::new(1.0, ui.style().visuals.text_color()));
    }
    let theme_label = theme.label();
    let label = crate::i18n::format_message_id("ui.theme_theme", &[("theme", &theme_label)]);
    let enabled = ui.is_enabled();
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, enabled, selected, &label));
    resp.on_hover_text(theme_label)
}

fn format_picker(ui: &mut Ui, format: &mut Option<PageFormat>) {
    let selected = format.map(PageFormat::label).unwrap_or(crate::tl_id!("ui.inherit_notebook_section_or_global_setting"));
    egui::ComboBox::from_label(crate::tl_id!("ui.page_format")).selected_text(selected).show_ui(ui, |ui| {
        ui.selectable_value(format, None, crate::tl_id!("ui.inherit_notebook_section_or_global_setting"));
        for f in PageFormat::ALL {
            ui.selectable_value(format, Some(f), f.label());
        }
    });
}

#[allow(dead_code)]
fn _unused(_: Color32) {}

/// A small square button that draws one icon, with an on state for the ones that toggle.
fn icon_toggle(ui: &mut Ui, icon: Icon, on: bool, t: &Tokens, label: &str, tip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 22.0), Sense::click());
    let painter = ui.painter_at(rect);
    if on || resp.hovered() {
        painter.rect_filled(rect, 4.0, if on { t.row_selected } else { t.row_hover });
    }
    let color = if on { t.accent } else { t.muted };
    icons::paint(&painter, egui::Rect::from_center_size(rect.center(), vec2(17.0, 17.0)), icon, color);
    let enabled = ui.is_enabled();
    let name = label.to_string();
    resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, on, name.clone()));
    resp.on_hover_text(tip)
}

/// Submenu offering all named section/notebook colours in a 4x4 grid plus "No colour".
fn draw_node_color_menu(ui: &mut Ui, path: &Path, current: Option<NodeColor>, dark: bool, acts: &mut Vec<Act>) {
    ui.menu_button(crate::tl_id!("ui.colour"), |ui| {
        egui::Grid::new("node-colours").spacing([4.0, 4.0]).show(ui, |ui| {
            for (i, c) in NodeColor::ALL.into_iter().enumerate() {
                if color_swatch(ui, c, dark, current == Some(c)).clicked() {
                    acts.push(Act::Color(path.to_path_buf(), Some(c)));
                    ui.close();
                }
                if i % 4 == 3 {
                    ui.end_row();
                }
            }
        });
        ui.add_space(2.0);
        if ui.button(crate::tl_id!("ui.no_colour")).clicked() {
            acts.push(Act::Color(path.to_path_buf(), None));
            ui.close();
        }
    });
}

/// One colour of the node colour picker: a filled circle, ringed when it is the current one.
fn color_swatch(ui: &mut Ui, color: NodeColor, dark: bool, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
    let (r, g, b) = color.swatch_rgb(dark);
    let fill = Color32::from_rgb(r, g, b);
    let painter = ui.painter_at(rect);
    painter.circle_filled(rect.center(), 8.0, fill);
    painter.circle_stroke(rect.center(), 8.0, egui::Stroke::new(0.8, Color32::from_gray(if dark { 200 } else { 110 }).linear_multiply(0.35)));
    if selected {
        painter.circle_stroke(rect.center(), 10.0, egui::Stroke::new(2.0, ui.style().visuals.text_color()));
    }
    let enabled = ui.is_enabled();
    let label = crate::tl!(color.label());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, enabled, selected, label));
    resp.on_hover_text(label)
}

/// Shows a folder in the operating system's file manager. An optional extra: everything the
/// app itself does works without it, and a failure is reported, never fatal.
fn open_folder(dir: &Path) -> std::io::Result<()> {
    // Explorer opens the user's Documents folder when it is given a path that does not exist, so
    // a missing folder is an error here, not a window on the wrong place.
    if !dir.is_dir() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, format!("{} does not exist", dir.display())));
    }
    let program = if cfg!(target_os = "windows") {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    // `explorer` returns a non-zero status even when it worked, so the status is not checked.
    std::process::Command::new(program).arg(dir).spawn().map(|_| ())
}

/// Opens a linked attachment with the operating system's associated application. Windows may
/// show its own warning for files marked as downloaded; that prompt is intentionally preserved.
pub(crate) fn open_file_for_viewer(file: &Path) -> std::io::Result<()> {
    if cfg!(target_os = "windows") {
        std::process::Command::new("cmd").args(["/C", "start", "", &file.to_string_lossy()]).spawn().map(|_| ())
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(file).spawn().map(|_| ())
    } else {
        std::process::Command::new("xdg-open").arg(file).spawn().map(|_| ())
    }
}

/// Saves a screenshot to `path`, or returns it as base64 PNG in the reply.
fn encode_screenshot(image: &egui::ColorImage, path: Option<&Path>) -> Result<Value, String> {
    let [w, h] = image.size;
    let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
    let img = image::RgbaImage::from_raw(w as u32, h as u32, rgba).ok_or("the screenshot has the wrong size")?;
    match path {
        Some(p) => {
            img.save(p).map_err(|e| format!("cannot save {}: {e}", p.display()))?;
            Ok(json!({"path": p.to_string_lossy(), "width": w, "height": h}))
        }
        None => {
            let mut png = std::io::Cursor::new(Vec::new());
            img.write_to(&mut png, image::ImageFormat::Png).map_err(|e| e.to_string())?;
            Ok(json!({"png": base64(png.get_ref()), "width": w, "height": h}))
        }
    }
}

/// The first page inside a notebook or section, at any depth.
fn first_page(tree: &[Node], root: &Path) -> Option<PathBuf> {
    fn walk(n: &Node) -> Option<PathBuf> {
        if n.info.kind == NodeKind::Page {
            return Some(n.path.clone());
        }
        n.children.iter().find_map(walk)
    }
    tree.iter().find(|n| n.path == root).and_then(walk)
}

/// The first page inside the node at `at`, wherever in the tree that node is.
fn first_page_under(tree: &[Node], at: &Path) -> Option<PathBuf> {
    fn find<'a>(n: &'a Node, at: &Path) -> Option<&'a Node> {
        if n.path == at {
            return Some(n);
        }
        n.children.iter().find_map(|c| find(c, at))
    }
    let node = tree.iter().find_map(|n| find(n, at))?;
    first_page(std::slice::from_ref(node), &node.path)
}

/// The 14 swatches and the switch that follows the system, shared by the sun or moon button
/// and the ribbon's Theme button.
fn theme_grid(ui: &mut Ui, current: Theme, pick: &mut Option<(Mode, Accent)>, follow: &mut bool) {
    egui::Grid::new("theme-grid").spacing([6.0, 6.0]).show(ui, |ui| {
        for mode in [Mode::Light, Mode::Dark] {
            ui.label(crate::tl!(mode.label()));
            for accent in Accent::ALL {
                let theme = Theme { mode, accent };
                let selected = theme == current;
                if theme_swatch(ui, theme, selected).clicked() {
                    *pick = Some((mode, accent));
                }
            }
            ui.end_row();
        }
    });
    ui.add_space(4.0);
    let accent = crate::tl!(current.accent.label());
    let label = crate::i18n::format_message_id("ui.accent_accent", &[("accent", accent)]);
    ui.label(egui::RichText::new(label).small());
    ui.checkbox(follow, crate::tl_id!("ui.follow_the_system_light_or_dark_setting"));
}

/// File > Save a Copy's folder chooser (issue #301). A browser has no folder dialog, so the web
/// build chooses nothing and the folder is typed instead. Kept here, below the other choosers,
/// and gated in its own right: the web build's rules rewrite the dialogs above by where they sit.
#[cfg(not(target_arch = "wasm32"))]
fn pick_copy_folder() -> Option<PathBuf> {
    rfd::FileDialog::new().set_title(crate::tl_id!("ui.choose_the_folder_for_the_copy")).pick_folder()
}

#[cfg(target_arch = "wasm32")]
fn pick_copy_folder() -> Option<PathBuf> {
    None
}

mod control;
mod dialogs;
mod mobile;
mod sidebar;
mod tts_dictation;

#[cfg(test)]
#[path = "app/tests.rs"]
mod app_tests;
