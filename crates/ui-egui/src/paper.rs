//! The sheet a page is written for (issue #269), drawn behind it: the edges of each sheet down
//! the page, and its margins as a dashed line. The column the words flow in is as wide as the
//! space between the margins. Nothing is clipped: what reaches past a sheet's edge is still
//! drawn and still saved, the way OneNote shows content outside a printed page.

use crate::theme::Tokens;
use egui::{Align2, FontId, Pos2, Rect, Stroke, Vec2, pos2, vec2};
use notecraft_model::paper::{POINTS_PER_MM, Paper, Size};

/// A sheet in screen points at the page's zoom.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sheet {
    pub size: Vec2,
    /// Top, right, bottom, left.
    pub margins: [f32; 4],
}

impl Sheet {
    pub fn of(paper: Paper, zoom: f32) -> Sheet {
        let (w, h) = paper.sheet_mm();
        let scale = POINTS_PER_MM * zoom;
        Sheet { size: vec2(w * scale, h * scale), margins: paper.margins_mm().map(|m| m * scale) }
    }

    /// How wide the words may run: between the left and right margins.
    pub fn column(self) -> f32 {
        (self.size.x - self.margins[1] - self.margins[3]).max(40.0)
    }
}

/// Visual sheet metrics for the page canvas (defaults to A4 when Auto).
pub fn sheet_for(paper: Option<Paper>, zoom: f32) -> Sheet {
    Sheet::of(paper.unwrap_or_else(|| Paper::new(Size::A4)), zoom)
}

/// Number of sheets needed so content reaching `content_bottom_pts` gets a fresh sheet before overflowing.
pub fn sheet_count(content_bottom_pts: f32, sheet_h_pts: f32) -> usize {
    let h = sheet_h_pts.max(200.0);
    if content_bottom_pts + 64.0 <= h { 1 } else { ((content_bottom_pts + 64.0) / h).ceil().max(1.0) as usize }
}

/// Draws `count` full-height sheets from `top_left`, with optional dashed margins,
/// clear page-break separators between sheets, and a `Feuille X / Y` label at the bottom-right.
pub fn paint_sheets(painter: &egui::Painter, top_left: Pos2, width: f32, sheet: Sheet, count: usize, show_margins: bool, t: &Tokens) {
    if sheet.size.y < 4.0 || width < 4.0 || count == 0 {
        return;
    }
    let [mt, mr, mb, ml] = sheet.margins;
    let edge = Stroke::new(1.2, t.container_border);
    let margin = Stroke::new(1.0, t.faint);
    let break_stroke = Stroke::new(2.0, t.accent.gamma_multiply(0.55));
    let badge_bg = crate::theme::mix(t.panel, t.accent, 0.14);
    let label_col = t.muted;
    for idx in 0..count {
        let top = top_left.y + idx as f32 * sheet.size.y;
        let rect = Rect::from_min_size(pos2(top_left.x, top), vec2(width, sheet.size.y));
        painter.rect_stroke(rect, 2.0, edge, egui::StrokeKind::Inside);
        if show_margins {
            let inner = Rect::from_min_max(pos2(rect.min.x + ml, rect.min.y + mt), pos2(rect.max.x - mr, rect.max.y - mb));
            if inner.width() > 1.0 && inner.height() > 1.0 {
                for side in [
                    [inner.left_top(), inner.right_top()],
                    [inner.right_top(), inner.right_bottom()],
                    [inner.right_bottom(), inner.left_bottom()],
                    [inner.left_bottom(), inner.left_top()],
                ] {
                    painter.extend(egui::Shape::dashed_line(&side, margin, 4.0, 4.0));
                }
            }
        }
        if idx > 0 {
            painter.hline(rect.x_range(), top, break_stroke);
            let badge_text = format!("Feuille {} / {}", idx + 1, count);
            let font = FontId::proportional(11.0);
            let galley = painter.layout_no_wrap(badge_text, font, t.accent);
            let pill = Rect::from_center_size(pos2(rect.center().x, top), galley.size() + vec2(16.0, 6.0));
            painter.rect_filled(pill, 8.0, badge_bg);
            painter.rect_stroke(pill, 8.0, Stroke::new(1.0, t.accent.gamma_multiply(0.45)), egui::StrokeKind::Inside);
            painter.galley(pill.min + vec2(8.0, 3.0), galley, t.accent);
        }
        let footer = format!("Feuille {} / {}", idx + 1, count);
        painter.text(rect.right_bottom() - vec2(12.0, 10.0), Align2::RIGHT_BOTTOM, footer, FontId::proportional(11.0), label_col);
    }
}

/// Draws the sheets from `top_left` down to the bottom of `view`: each sheet's edge, and its
/// margins dashed.
pub fn paint(painter: &egui::Painter, view: Rect, top_left: Pos2, sheet: Sheet, t: &Tokens) {
    if sheet.size.y < 4.0 || sheet.size.x < 4.0 {
        return;
    }
    let [mt, mr, mb, ml] = sheet.margins;
    let edge = Stroke::new(1.0, t.muted.gamma_multiply(0.7));
    let margin = Stroke::new(1.0, t.faint);
    let mut top = top_left.y;
    // Sheets above the view are skipped, not drawn; a long page draws only what is in sight.
    if top + sheet.size.y < view.min.y {
        let skip = ((view.min.y - top) / sheet.size.y).floor();
        top += skip * sheet.size.y;
    }
    for _ in 0..400 {
        if top > view.max.y {
            break;
        }
        let rect = Rect::from_min_size(pos2(top_left.x, top), sheet.size);
        // Only lines: the page's colour and ruling stay as they are under the sheet.
        painter.rect_stroke(rect, 0.0, edge, egui::StrokeKind::Inside);
        let inner = Rect::from_min_max(pos2(rect.min.x + ml, rect.min.y + mt), pos2(rect.max.x - mr, rect.max.y - mb));
        if inner.width() > 1.0 && inner.height() > 1.0 {
            for side in [
                [inner.left_top(), inner.right_top()],
                [inner.right_top(), inner.right_bottom()],
                [inner.right_bottom(), inner.left_bottom()],
                [inner.left_bottom(), inner.left_top()],
            ] {
                painter.extend(egui::Shape::dashed_line(&side, margin, 4.0, 4.0));
            }
        }
        top += sheet.size.y;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notecraft_model::paper::Size;

    #[test]
    fn a_sheet_is_the_paper_at_the_page_zoom() {
        let a4 = Sheet::of(Paper { size: Size::A4, landscape: false, margins: [0.0, 20.0, 0.0, 10.0] }, 1.0);
        assert!((a4.size.x - 210.0 * POINTS_PER_MM).abs() < 0.01);
        assert!((a4.column() - 180.0 * POINTS_PER_MM).abs() < 0.01, "the words run between the margins");
        let twice = Sheet::of(Paper { size: Size::A4, landscape: true, margins: [0.0; 4] }, 2.0);
        assert!((twice.size.x - 297.0 * POINTS_PER_MM * 2.0).abs() < 0.01, "turned and zoomed");
    }

    #[test]
    fn sheet_count_adds_sheets_when_content_nears_or_crosses_bottom() {
        let a4 = sheet_for(None, 1.0);
        assert_eq!(sheet_count(100.0, a4.size.y), 1);
        assert_eq!(sheet_count(a4.size.y - 64.0, a4.size.y), 1);
        assert_eq!(sheet_count(a4.size.y - 10.0, a4.size.y), 2);
        assert_eq!(sheet_count(a4.size.y * 2.0 + 10.0, a4.size.y), 3);
    }
}
