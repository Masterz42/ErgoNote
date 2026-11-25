//! Intelligent layout-aware PDF & image text extraction:
//! 1. Exact native PDF Unicode + layout via `pdftotext -bbox-layout` (Poppler) on desktop, with
//!    `hayro` (`hayro_interpret::Device`) fallback when `pdftotext` is unavailable.
//! 2. PaddleOCR PP-OCRv5 Latin (`oar-ocr` / ONNX Runtime) on desktop for accurate French accents
//!    on scanned PDFs and photos.
//! 3. Desktop `tesseract` (`fra.traineddata`) and pure-Rust `ocrs` + `rten` fallbacks with spatial
//!    blocks and French accent restoration.

use std::path::{Path, PathBuf};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;

/// Hugging Face download URL for the `ocrs` text detection model.
pub const DETECTION_URL: &str = "https://huggingface.co/robertknight/ocrs/resolve/main/text-detection-ssfbcj81.rten";
/// Hugging Face download URL for the `ocrs` text recognition model.
pub const RECOGNITION_URL: &str = "https://huggingface.co/robertknight/ocrs/resolve/main/text-rec-checkpoint-s52qdbqt.rten";
/// Fast French Tesseract model downloaded once into the cache folder on desktop when needed.
pub const TESS_FRA_URL: &str = "https://github.com/tesseract-ocr/tessdata_fast/raw/main/fra.traineddata";
/// PaddleOCR PP-OCRv3 ONNX text detector (compact, 2.4 MB).
pub const PADDLE_DET_URL: &str = "https://huggingface.co/monkt/paddleocr-onnx/resolve/main/detection/v3/det.onnx";
/// PaddleOCR PP-OCRv5 Latin/French ONNX text recognizer (full French accents).
pub const PADDLE_LATIN_REC_URL: &str = "https://huggingface.co/monkt/paddleocr-onnx/resolve/main/languages/latin/rec.onnx";
/// Character dictionary matching `PADDLE_LATIN_REC_URL`.
pub const PADDLE_LATIN_DICT_URL: &str = "https://huggingface.co/monkt/paddleocr-onnx/resolve/main/languages/latin/dict.txt";

const DETECTION_FILE: &str = "text-detection-ssfbcj81.rten";
const RECOGNITION_FILE: &str = "text-rec-checkpoint-s52qdbqt.rten";
const TESS_FRA_FILE: &str = "fra.traineddata";
const PADDLE_DET_FILE: &str = "ppocr_v3_det.onnx";
const PADDLE_LATIN_REC_FILE: &str = "ppocr_v5_latin_rec.onnx";
const PADDLE_LATIN_DICT_FILE: &str = "ppocr_v5_latin_dict.txt";

/// A positioned editable text zone extracted from a PDF page or scanned image.
#[derive(Clone, Debug, PartialEq)]
pub struct OcrBlock {
    /// Normalized position (`0.0..=1.0`) from left edge of the source page/image.
    pub x: f32,
    /// Normalized position (`0.0..=1.0`) from top edge of the source page/image.
    pub y: f32,
    /// Normalized width (`0.05..=1.0`).
    pub width: f32,
    /// Normalized height (`0.02..=1.0`).
    pub height: f32,
    /// Font size ratio relative to body text (`1.0` = body, `>= 1.35` = heading).
    pub font_scale: f32,
    /// Clean lines of text in this zone.
    pub lines: Vec<String>,
}

/// Complete structured extraction result ready to reconstruct an editable ErgoNote page.
#[derive(Clone, Debug, PartialEq)]
pub struct OcrDocument {
    pub source_label: &'static str,
    pub aspect_ratio: f32,
    pub blocks: Vec<OcrBlock>,
    pub plain_text: String,
}

impl OcrDocument {
    /// Returns true when no two detected blocks sit side-by-side horizontally on the page.
    pub fn is_single_column(&self) -> bool {
        for (i, a) in self.blocks.iter().enumerate() {
            for b in self.blocks.iter().skip(i + 1) {
                let overlap_y = (a.y + a.height).min(b.y + b.height) - a.y.max(b.y);
                let disjoint_x = a.x + a.width <= b.x + 0.01 || b.x + b.width <= a.x + 0.01;
                if overlap_y > 0.005 && disjoint_x {
                    return false;
                }
            }
        }
        true
    }
}

#[derive(Clone, Debug)]
struct PositionedLine {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    font_size: f32,
    text: String,
}

/// User-writable cache directory where downloaded `.rten` / `.traineddata` OCR models live.
pub fn cache_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    if let Ok(local) = std::env::var("LOCALAPPDATA")
        && !local.is_empty()
    {
        return PathBuf::from(local).join("ErgoNote").join("ocr");
    }
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME")
        && !xdg.is_empty()
    {
        return PathBuf::from(xdg).join("ergonote").join("ocr");
    }
    if let Ok(home) = std::env::var("HOME")
        && !home.is_empty()
    {
        return PathBuf::from(home).join(".cache").join("ergonote").join("ocr");
    }
    std::env::temp_dir().join("ergonote-ocr")
}

/// Maps a printout image reference (`assets/<doc.pdf>.printout-001.png`) back to `(pdf_path, 0-based page)`.
pub fn resolve_pdf_printout(page_dir: &Path, image_src: &str) -> Option<(PathBuf, usize)> {
    let rel = image_src.strip_prefix("assets/")?;
    let (base, rest) = rel.rsplit_once(".printout-")?;
    let num_str = rest.strip_suffix(".png")?;
    let page_one_based: usize = num_str.parse().ok()?;
    if page_one_based == 0 || !base.to_ascii_lowercase().ends_with(".pdf") {
        return None;
    }
    if base.contains("..") || base.contains('\\') || base.starts_with('/') {
        return None;
    }
    let pdf_path = page_dir.join("assets").join(base);
    notecraft_store::vfs::is_file(&pdf_path).then_some((pdf_path, page_one_based - 1))
}

#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos", target_os = "android"))]
fn download_if_missing(url: &str, dest: &Path, min_bytes: u64) -> Result<(), String> {
    use std::io::Read;
    if dest.exists() && std::fs::metadata(dest).is_ok_and(|m| m.len() > min_bytes) {
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Impossible de créer le dossier cache OCR : {e}"))?;
    }
    let response = ureq::get(url).call().map_err(|e| format!("Téléchargement du modèle OCR impossible : {e}"))?;
    let mut reader = response.into_body().into_reader();
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).map_err(|e| format!("Lecture du modèle OCR échouée : {e}"))?;
    if (bytes.len() as u64) <= min_bytes {
        return Err("Le modèle OCR téléchargé est vide ou incomplet.".to_string());
    }
    let part = dest.with_extension("part");
    std::fs::write(&part, &bytes).map_err(|e| format!("Écriture du modèle OCR impossible : {e}"))?;
    std::fs::rename(&part, dest).map_err(|e| format!("Enregistrement du modèle OCR impossible : {e}"))?;
    Ok(())
}

/// Ensures both `ocrs` models are cached locally on disk, downloading them on first use.
#[cfg(not(target_arch = "wasm32"))]
pub fn ensure_models() -> Result<(PathBuf, PathBuf), String> {
    let dir = cache_dir();
    let det = dir.join(DETECTION_FILE);
    let rec = dir.join(RECOGNITION_FILE);
    #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos", target_os = "android"))]
    {
        download_if_missing(DETECTION_URL, &det, 1024)?;
        download_if_missing(RECOGNITION_URL, &rec, 1024)?;
        Ok((det, rec))
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos", target_os = "android")))]
    {
        if det.exists() && rec.exists() {
            Ok((det, rec))
        } else {
            Err("Le téléchargement automatique des modèles OCR n'est pas disponible sur cette plateforme.".to_string())
        }
    }
}

/// Extracts exact Unicode text and spatial blocks directly from a vector/text PDF page using
/// `hayro_interpret`. Returns `Ok(None)` for scanned PDFs without embedded glyphs so OCR runs.
#[cfg(not(target_arch = "wasm32"))]
pub fn extract_pdf_page_layout(pdf_bytes: &[u8], page_index: usize) -> Result<Option<OcrDocument>, String> {
    use hayro::hayro_interpret::font::Glyph;
    use hayro::hayro_interpret::{
        BlendMode, ClipPath, Context, Device, GlyphDrawMode, InterpreterSettings, PageExt, Paint, PathDrawMode, SoftMask, interpret_page,
    };
    use hayro::hayro_syntax::Pdf;
    use kurbo::{Affine, BezPath, Point, Rect, Shape as _};

    #[derive(Clone)]
    struct PdfGlyph {
        ch: char,
        x0: f32,
        x1: f32,
        y: f32,
        font_size: f32,
    }

    struct Collector {
        glyphs: Vec<PdfGlyph>,
    }

    impl Device<'_> for Collector {
        fn set_soft_mask(&mut self, _: Option<SoftMask<'_>>) {}
        fn set_blend_mode(&mut self, _: BlendMode) {}
        fn draw_path(&mut self, _: &BezPath, _: Affine, _: &Paint<'_>, _: &PathDrawMode) {}
        fn push_clip_path(&mut self, _: &ClipPath) {}
        fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'_>>, _: BlendMode) {}
        fn draw_glyph(&mut self, glyph: &Glyph<'_>, transform: Affine, glyph_transform: Affine, _: &Paint<'_>, _: &GlyphDrawMode) {
            let Some(ch) = glyph.as_unicode() else { return };
            if ch.is_control() {
                return;
            }
            let combined = transform * glyph_transform;
            let pos = combined * Point::ZERO;
            let c = combined.as_coeffs();
            let raw_scale = c[1].hypot(c[3]).max(c[0].hypot(c[2])) as f32;
            if !pos.x.is_finite() || !pos.y.is_finite() || !raw_scale.is_finite() {
                return;
            }
            match glyph {
                Glyph::Outline(outline) => {
                    let font_size = (raw_scale * 1000.0).max(6.0);
                    let mut path = outline.outline();
                    path.apply_affine(combined);
                    let bbox = path.bounding_box();
                    let (x0, x1) = if bbox.width() > 0.0 && bbox.x0.is_finite() && bbox.x1.is_finite() {
                        (bbox.x0 as f32, bbox.x1 as f32)
                    } else {
                        let px = pos.x as f32;
                        (px, px + font_size * 0.45)
                    };
                    self.glyphs.push(PdfGlyph { ch, x0, x1, y: pos.y as f32, font_size });
                }
                Glyph::Type3(_) => {
                    let font_size = raw_scale.max(6.0);
                    let px = pos.x as f32;
                    self.glyphs.push(PdfGlyph { ch, x0: px, x1: px + font_size * 0.55, y: pos.y as f32, font_size });
                }
            }
        }
        fn draw_image(&mut self, _: hayro::hayro_interpret::Image<'_, '_>, _: Affine) {}
        fn pop_clip_path(&mut self) {}
        fn pop_transparency_group(&mut self) {}
    }

    let pdf = Pdf::new(Arc::new(pdf_bytes.to_vec())).map_err(|e| format!("PDF illisible : {e:?}"))?;
    let Some(page) = pdf.pages().get(page_index) else {
        return Err(format!("Page PDF {} introuvable.", page_index + 1));
    };
    let (pw, ph) = page.render_dimensions();
    if pw <= 1.0 || ph <= 1.0 {
        return Ok(None);
    }
    let settings = InterpreterSettings::default();
    let mut state = Context::new(page.initial_transform(true), Rect::new(0.0, 0.0, f64::from(pw), f64::from(ph)), page.xref(), settings);
    let mut collector = Collector { glyphs: Vec::new() };
    interpret_page(page, &mut state, &mut collector);

    let mut glyphs: Vec<PdfGlyph> = collector.glyphs.into_iter().filter(|g| !g.ch.is_whitespace()).collect();
    if glyphs.is_empty() {
        return Ok(None);
    }
    glyphs.sort_by(|a, b| a.y.total_cmp(&b.y).then_with(|| a.x0.total_cmp(&b.x0)));

    let mut raw_lines: Vec<Vec<PdfGlyph>> = Vec::new();
    for g in glyphs {
        if let Some(last_line) = raw_lines.last_mut()
            && let Some(first) = last_line.first()
            && (g.y - first.y).abs() <= first.font_size.max(g.font_size) * 0.45
        {
            last_line.push(g);
        } else {
            raw_lines.push(vec![g]);
        }
    }

    let mut lines: Vec<PositionedLine> = Vec::with_capacity(raw_lines.len());
    for mut line_glyphs in raw_lines {
        line_glyphs.sort_by(|a, b| a.x0.total_cmp(&b.x0));
        let mut text = String::new();
        let mut prev_end_x = 0.0_f32;
        let mut first = true;
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        let mut font_sum = 0.0_f32;
        for g in &line_glyphs {
            if !first && (g.x0 - prev_end_x) > g.font_size * 0.22 && !text.ends_with(' ') {
                text.push(' ');
            }
            text.push(g.ch);
            first = false;
            prev_end_x = g.x1;
            min_x = min_x.min(g.x0);
            max_x = max_x.max(g.x1);
            min_y = min_y.min(g.y - g.font_size * 0.85);
            max_y = max_y.max(g.y + g.font_size * 0.2);
            font_sum += g.font_size;
        }
        let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if !cleaned.is_empty() {
            lines.push(PositionedLine {
                x0: min_x.max(0.0),
                y0: min_y.max(0.0),
                x1: max_x.min(pw),
                y1: max_y.min(ph),
                font_size: font_sum / (line_glyphs.len().max(1) as f32),
                text: cleaned,
            });
        }
    }
    if lines.is_empty() {
        return Ok(None);
    }
    let blocks = cluster_lines_into_blocks(lines, pw, ph);
    let plain_text = blocks.iter().map(|b| b.lines.join("\n")).collect::<Vec<_>>().join("\n\n");
    Ok(Some(OcrDocument { source_label: "PDF natif (texte exact)", aspect_ratio: safe_aspect_ratio(pw, ph), blocks, plain_text }))
}

fn safe_aspect_ratio(w: f32, h: f32) -> f32 {
    if w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0 { (h / w).clamp(0.3, 3.0) } else { 1.414 }
}

fn median_font_size(lines: &[PositionedLine]) -> f32 {
    let mut sizes: Vec<f32> = lines.iter().map(|l| l.font_size).filter(|s| s.is_finite() && *s > 0.0).collect();
    if sizes.is_empty() {
        return 14.0;
    }
    sizes.sort_by(f32::total_cmp);
    sizes[(sizes.len() - 1) / 2]
}

fn make_ocr_block(x0: f32, y0: f32, x1: f32, y1: f32, max_font: f32, body_size: f32, lines: Vec<String>, page_w: f32, page_h: f32) -> OcrBlock {
    let x = (x0 / page_w).clamp(0.0, 0.92);
    let y = (y0 / page_h).clamp(0.0, 0.96);
    // Keep min <= max and avoid inflating narrow boxes so erase_text_regions and column detection stay accurate.
    let max_w = (1.0 - x).max(0.02);
    let min_w = 0.02_f32.min(max_w);
    let w = ((x1 - x0) / page_w).clamp(min_w, max_w);
    let max_h = (1.0 - y).max(0.02);
    let min_h = 0.02_f32.min(max_h);
    let h = ((y1 - y0) / page_h).clamp(min_h, max_h);
    let font_scale = if body_size > 0.0 && max_font.is_finite() { (max_font / body_size).clamp(0.7, 3.0) } else { 1.0 };
    OcrBlock { x, y, width: w, height: h, font_scale, lines }
}

fn cluster_lines_into_blocks(lines: Vec<PositionedLine>, page_w: f32, page_h: f32) -> Vec<OcrBlock> {
    if !page_w.is_finite() || !page_h.is_finite() || page_w <= 0.0 || page_h <= 0.0 {
        return Vec::new();
    }
    let mut lines: Vec<PositionedLine> = lines
        .into_iter()
        .filter(|l| {
            l.x0.is_finite() && l.y0.is_finite() && l.x1.is_finite() && l.y1.is_finite() && l.font_size.is_finite() && !l.text.trim().is_empty()
        })
        .collect();
    if lines.is_empty() {
        return Vec::new();
    }
    let body_size = median_font_size(&lines);
    lines.sort_by(|a, b| a.y0.total_cmp(&b.y0).then_with(|| a.x0.total_cmp(&b.x0)));

    struct Group {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        last_line_x0: f32,
        last_line_y1: f32,
        max_font: f32,
        lines: Vec<String>,
    }

    let mut groups: Vec<Group> = Vec::new();
    for line in lines {
        let line_h = (line.y1 - line.y0).max(line.font_size).max(8.0);
        let is_heading = (line.font_size / body_size) >= 1.32;
        let match_idx = groups.iter().rposition(|g| {
            let g_heading = (g.max_font / body_size) >= 1.32;
            let vertical_gap = line.y0 - g.last_line_y1;
            let same_col = (line.x0 - g.last_line_x0).abs() <= page_w * 0.09
                || (line.x0 >= g.x0 - page_w * 0.05 && line.x0 <= g.x1 + page_w * 0.03 && vertical_gap <= line_h * 1.4);
            let close_vertically = vertical_gap >= -line_h * 0.4 && vertical_gap <= line_h * 1.85;
            g_heading == is_heading && same_col && close_vertically
        });

        if let Some(idx) = match_idx {
            let g = &mut groups[idx];
            g.x0 = g.x0.min(line.x0);
            g.y0 = g.y0.min(line.y0);
            g.x1 = g.x1.max(line.x1);
            g.y1 = g.y1.max(line.y1);
            g.last_line_x0 = line.x0;
            g.last_line_y1 = line.y1;
            g.max_font = g.max_font.max(line.font_size);
            g.lines.push(line.text);
        } else {
            groups.push(Group {
                x0: line.x0,
                y0: line.y0,
                x1: line.x1,
                y1: line.y1,
                last_line_x0: line.x0,
                last_line_y1: line.y1,
                max_font: line.font_size,
                lines: vec![line.text],
            });
        }
    }

    groups.sort_by(|a, b| a.y0.total_cmp(&b.y0).then_with(|| a.x0.total_cmp(&b.x0)));
    groups.into_iter().map(|g| make_ocr_block(g.x0, g.y0, g.x1, g.y1, g.max_font, body_size, g.lines, page_w, page_h)).collect()
}

fn xml_attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    for quote in ['"', '\''] {
        let needle = format!("{name}={quote}");
        if let Some(start) = tag.find(&needle) {
            let rest = &tag[start + needle.len()..];
            if let Some(end) = rest.find(quote) {
                return Some(&rest[..end]);
            }
        }
    }
    None
}

fn unescape_xml(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        if let Some(semi) = after.find(';') {
            let entity = &after[..semi];
            let decoded = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ if entity.starts_with("#x") || entity.starts_with("#X") => u32::from_str_radix(&entity[2..], 16).ok().and_then(char::from_u32),
                _ if entity.starts_with('#') => entity[1..].parse::<u32>().ok().and_then(char::from_u32),
                _ => None,
            };
            if let Some(ch) = decoded {
                out.push(ch);
                rest = &after[semi + 1..];
                continue;
            }
        }
        out.push('&');
        rest = after;
    }
    out.push_str(rest);
    out
}

fn bullet_marker_depth(token: &str) -> Option<usize> {
    match token.trim() {
        "·" | "•" | "▪" | "●" | "-" | "–" | "—" => Some(0),
        "o" | "◦" | "○" => Some(1),
        _ => None,
    }
}

fn starts_list_marker(line: &str) -> bool {
    if line.starts_with("- ") || line.starts_with("  - ") {
        return true;
    }
    let Some((num, rest)) = line.split_once(". ") else {
        return false;
    };
    !num.is_empty() && num.len() <= 2 && num.bytes().all(|b| b.is_ascii_digit()) && !rest.trim().is_empty()
}

fn normalize_inline_bullet(line: &mut PositionedLine) {
    let trimmed = line.text.trim_start();
    for (prefix, depth) in [("· ", 0), ("• ", 0), ("▪ ", 0), ("● ", 0), ("– ", 0), ("— ", 0), ("o ", 1), ("◦ ", 1), ("○ ", 1)] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            let out_prefix = if depth == 0 { "- " } else { "  - " };
            line.text = format!("{out_prefix}{}", rest.trim_start());
            return;
        }
    }
}

/// Attaches standalone bullet blocks (such as `·` or `o` in Poppler XML) onto the list line to
/// their right so they never spawn orphan text boxes, and normalizes inline bullet prefixes.
fn merge_bullet_lines(tagged_lines: Vec<(usize, PositionedLine)>, page_w: f32) -> Vec<(usize, PositionedLine)> {
    let mut standalone = Vec::new();
    let mut content = Vec::new();
    for (bid, mut line) in tagged_lines {
        if let Some(depth) = bullet_marker_depth(&line.text)
            && (line.x1 - line.x0) < page_w * 0.05
        {
            standalone.push((line, depth));
        } else {
            normalize_inline_bullet(&mut line);
            content.push((bid, line));
        }
    }

    let min_bullet_x = standalone.iter().map(|(l, _)| l.x0).reduce(f32::min).unwrap_or(f32::MAX);
    for (bullet, mut depth) in standalone {
        let by = (bullet.y0 + bullet.y1) * 0.5;
        if min_bullet_x.is_finite() && bullet.x0 - min_bullet_x > page_w * 0.035 {
            depth = depth.max(1);
        }
        let mut best_idx = None;
        let mut best_score = f32::MAX;
        for (i, (_, t)) in content.iter().enumerate() {
            if t.x0 <= bullet.x0 - 2.0 {
                continue;
            }
            let ty = (t.y0 + t.y1) * 0.5;
            let dy = (ty - by).abs();
            if dy <= t.font_size.max(bullet.font_size) * 0.75 {
                let score = dy * 4.0 + (t.x0 - bullet.x0).max(0.0);
                if score < best_score {
                    best_score = score;
                    best_idx = Some(i);
                }
            }
        }
        if let Some(idx) = best_idx {
            let target = &mut content[idx].1;
            let prefix = if depth == 0 { "- " } else { "  - " };
            if !target.text.starts_with("- ") && !target.text.starts_with("  - ") {
                target.text = format!("{prefix}{}", target.text);
            }
            target.x0 = target.x0.min(bullet.x0);
            target.y0 = target.y0.min(bullet.y0);
            target.y1 = target.y1.max(bullet.y1);
        } else {
            let prefix = if depth == 0 { "- " } else { "  - " };
            let mut l = bullet;
            l.text = prefix.trim_end().to_string();
            content.push((usize::MAX, l));
        }
    }
    content
}

fn append_wrapped_line(prev: &mut String, next: &str) {
    let trimmed = next.trim();
    if trimmed.is_empty() {
        return;
    }
    if prev.ends_with('-') && trimmed.chars().next().is_some_and(|c| c.is_lowercase()) {
        prev.push_str(trimmed);
    } else {
        if !prev.ends_with(' ') {
            prev.push(' ');
        }
        prev.push_str(trimmed);
    }
}

/// Parses Poppler's `pdftotext -bbox-layout` XHTML output into an `OcrDocument`, keeping full
/// Unicode French accents, merged bullets, joined wrapped lines, and Poppler's layout blocks.
#[cfg(any(not(target_arch = "wasm32"), test))]
pub(crate) fn parse_pdftotext_bbox_layout(xml: &str) -> Option<OcrDocument> {
    let mut page_w = 0.0_f32;
    let mut page_h = 0.0_f32;
    let mut block_id = 0_usize;
    let mut current_line: Option<(usize, f32, f32, f32, f32, Vec<String>)> = None;
    let mut tagged_lines: Vec<(usize, PositionedLine)> = Vec::new();

    let mut pos = 0_usize;
    while let Some(rel_lt) = xml[pos..].find('<') {
        let lt = pos + rel_lt;
        let Some(rel_gt) = xml[lt..].find('>') else {
            break;
        };
        let gt = lt + rel_gt;
        let tag = xml[lt + 1..gt].trim();
        pos = gt + 1;

        if tag.starts_with("page ") || tag == "page" {
            page_w = xml_attr(tag, "width").and_then(|v| v.parse::<f32>().ok()).unwrap_or(page_w);
            page_h = xml_attr(tag, "height").and_then(|v| v.parse::<f32>().ok()).unwrap_or(page_h);
        } else if tag.starts_with("block ") || tag == "block" {
            block_id += 1;
        } else if tag.starts_with("line ") || tag == "line" {
            let x0 = xml_attr(tag, "xMin").and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
            let y0 = xml_attr(tag, "yMin").and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
            let x1 = xml_attr(tag, "xMax").and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
            let y1 = xml_attr(tag, "yMax").and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
            if x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite() {
                current_line = Some((block_id, x0, y0, x1, y1, Vec::new()));
            }
        } else if tag == "/line" {
            if let Some((bid, x0, y0, x1, y1, words)) = current_line.take() {
                let joined = words.join(" ");
                let cleaned = joined.trim();
                if !cleaned.is_empty() && x1 > x0 && y1 > y0 {
                    tagged_lines.push((bid, PositionedLine { x0, y0, x1, y1, font_size: (y1 - y0).max(8.0), text: cleaned.to_string() }));
                }
            }
        } else if (tag.starts_with("word ") || tag == "word")
            && let Some(end_rel) = xml[pos..].find("</word>")
        {
            let word_raw = &xml[pos..pos + end_rel];
            pos += end_rel + "</word>".len();
            let word = unescape_xml(word_raw);
            let trimmed = word.trim();
            if !trimmed.is_empty()
                && let Some((_, _, _, _, _, ref mut words)) = current_line
            {
                words.push(trimmed.to_string());
            }
        }
    }

    if tagged_lines.is_empty() || !page_w.is_finite() || !page_h.is_finite() || page_w <= 1.0 || page_h <= 1.0 {
        return None;
    }

    let merged = merge_bullet_lines(tagged_lines, page_w);
    let only_lines: Vec<PositionedLine> = merged.iter().map(|(_, l)| l.clone()).collect();
    let body_size = median_font_size(&only_lines);

    struct BlockAcc {
        bid: usize,
        is_heading: bool,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        last_y1: f32,
        max_font: f32,
        lines: Vec<String>,
    }

    let mut accs: Vec<BlockAcc> = Vec::new();
    for (bid, line) in merged {
        let is_heading = (line.font_size / body_size) >= 1.32;
        if let Some(last) = accs.last_mut()
            && last.bid == bid
            && last.is_heading == is_heading
        {
            let vertical_gap = line.y0 - last.last_y1;
            last.x0 = last.x0.min(line.x0);
            last.y0 = last.y0.min(line.y0);
            last.x1 = last.x1.max(line.x1);
            last.y1 = last.y1.max(line.y1);
            last.last_y1 = line.y1;
            last.max_font = last.max_font.max(line.font_size);
            if !starts_list_marker(&line.text)
                && vertical_gap <= line.font_size * 0.85
                && let Some(prev_line) = last.lines.last_mut()
            {
                append_wrapped_line(prev_line, &line.text);
            } else {
                last.lines.push(line.text);
            }
        } else {
            accs.push(BlockAcc {
                bid,
                is_heading,
                x0: line.x0,
                y0: line.y0,
                x1: line.x1,
                y1: line.y1,
                last_y1: line.y1,
                max_font: line.font_size,
                lines: vec![line.text],
            });
        }
    }

    accs.sort_by(|a, b| a.y0.total_cmp(&b.y0).then_with(|| a.x0.total_cmp(&b.x0)));
    // Merge closely stacked non-heading blocks belonging to the same centered/aligned group
    // (e.g. centered address headers or signature lines that Poppler emitted per line).
    let mut stacked: Vec<BlockAcc> = Vec::with_capacity(accs.len());
    for mut acc in accs {
        if let Some(prev) = stacked.last_mut()
            && !prev.is_heading
            && !acc.is_heading
        {
            let v_gap = acc.y0 - prev.y1;
            let line_tol = prev.max_font.max(acc.max_font) * 0.95;
            let horiz_overlap = acc.x0 <= prev.x1 && prev.x0 <= acc.x1;
            let prev_cx = (prev.x0 + prev.x1) * 0.5;
            let acc_cx = (acc.x0 + acc.x1) * 0.5;
            let same_align = (acc.x0 - prev.x0).abs() <= page_w * 0.10 || (acc_cx - prev_cx).abs() <= page_w * 0.08;
            if v_gap >= -line_tol * 0.4 && v_gap <= line_tol && horiz_overlap && same_align {
                prev.x0 = prev.x0.min(acc.x0);
                prev.y0 = prev.y0.min(acc.y0);
                prev.x1 = prev.x1.max(acc.x1);
                prev.y1 = prev.y1.max(acc.y1);
                prev.last_y1 = prev.last_y1.max(acc.last_y1);
                prev.max_font = prev.max_font.max(acc.max_font);
                prev.lines.extend(std::mem::take(&mut acc.lines));
                continue;
            }
        }
        stacked.push(acc);
    }

    let blocks: Vec<OcrBlock> =
        stacked.into_iter().map(|g| make_ocr_block(g.x0, g.y0, g.x1, g.y1, g.max_font, body_size, g.lines, page_w, page_h)).collect();
    let plain_text = blocks.iter().map(|b| b.lines.join("\n")).collect::<Vec<_>>().join("\n\n");
    Some(OcrDocument { source_label: "PDF natif exact (Poppler)", aspect_ratio: safe_aspect_ratio(page_w, page_h), blocks, plain_text })
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn pdftotext_extract_page(pdf_bytes: &[u8], page_index: usize) -> Option<OcrDocument> {
    let page_num = (page_index + 1).to_string();
    let tmp_pdf = std::env::temp_dir().join(format!("ergonote_pdftotext_{}.pdf", std::process::id()));
    std::fs::write(&tmp_pdf, pdf_bytes).ok()?;
    let output =
        std::process::Command::new("pdftotext").args(["-f", &page_num, "-l", &page_num, "-bbox-layout"]).arg(&tmp_pdf).arg("-").output().ok();
    let _ = std::fs::remove_file(&tmp_pdf);
    let out = output?;
    if !out.status.success() {
        return None;
    }
    let xml = String::from_utf8(out.stdout).ok()?;
    parse_pdftotext_bbox_layout(&xml)
}

/// Restores French diacritics on frequent unaccented school words when an accent-free OCR model
/// such as `ocrs`'s default English-trained model produces unaccented French words.
pub fn restore_french_accents(text: &str) -> String {
    const MAP: &[(&str, &str)] = &[
        ("ecole", "école"),
        ("ecoles", "écoles"),
        ("eleve", "élève"),
        ("eleves", "élèves"),
        ("francais", "français"),
        ("francaise", "française"),
        ("etre", "être"),
        ("etat", "état"),
        ("etape", "étape"),
        ("etude", "étude"),
        ("etudes", "études"),
        ("ecrire", "écrire"),
        ("ecrit", "écrit"),
        ("ecoute", "écoute"),
        ("ecouter", "écouter"),
        ("lecon", "leçon"),
        ("lecons", "leçons"),
        ("reponse", "réponse"),
        ("reponses", "réponses"),
        ("repondre", "répondre"),
        ("resume", "résumé"),
        ("numero", "numéro"),
        ("numeros", "numéros"),
        ("mathematiques", "mathématiques"),
        ("geometrie", "géométrie"),
        ("conjugaison", "conjugaison"),
        ("grammaire", "grammaire"),
        ("orthographe", "orthographe"),
        ("vocabulaire", "vocabulaire"),
        ("theoreme", "théorème"),
        ("probleme", "problème"),
        ("problemes", "problèmes"),
        ("systeme", "système"),
        ("symetrie", "symétrie"),
        ("perimetre", "périmètre"),
        ("premiere", "première"),
        ("deuxieme", "deuxième"),
        ("troisieme", "troisième"),
        ("dernier", "dernier"),
        ("derniere", "dernière"),
        ("apres", "après"),
        ("tres", "très"),
        ("deja", "déjà"),
        ("voila", "voilà"),
        ("methode", "méthode"),
        ("regle", "règle"),
        ("regles", "règles"),
        ("modele", "modèle"),
        ("activite", "activité"),
        ("evaluation", "évaluation"),
        ("completer", "compléter"),
        ("complete", "complète"),
        ("souligner", "souligner"),
        ("entourer", "entourer"),
        ("calculer", "calculer"),
        ("verifier", "vérifier"),
        ("tableau", "tableau"),
        ("differents", "différents"),
        ("differentes", "différentes"),
        ("general", "général"),
        ("generale", "générale"),
        ("resultat", "résultat"),
        ("resultats", "résultats"),
        ("experience", "expérience"),
        ("histoire", "histoire"),
        ("geographie", "géographie"),
        ("francaises", "françaises"),
        ("aout", "août"),
        ("fevrier", "février"),
        ("decembre", "décembre"),
    ];

    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    let flush = |w: &mut String, dest: &mut String| {
        if w.is_empty() {
            return;
        }
        let lower = w.to_lowercase();
        if let Some((_, fixed)) = MAP.iter().find(|(k, _)| *k == lower) {
            if w.chars().all(|c| !c.is_lowercase()) {
                dest.push_str(&fixed.to_uppercase());
            } else if w.chars().next().is_some_and(char::is_uppercase) {
                let mut chars = fixed.chars();
                if let Some(first) = chars.next() {
                    for up in first.to_uppercase() {
                        dest.push(up);
                    }
                    dest.push_str(chars.as_str());
                }
            } else {
                dest.push_str(fixed);
            }
        } else {
            dest.push_str(w);
        }
        w.clear();
    };
    for ch in text.chars() {
        if ch.is_alphabetic() {
            word.push(ch);
        } else {
            flush(&mut word, &mut out);
            out.push(ch);
        }
    }
    flush(&mut word, &mut out);
    out
}

fn parse_tesseract_tsv(tsv: &str, img_w: f32, img_h: f32) -> Vec<OcrBlock> {
    use std::collections::BTreeMap;
    #[derive(Default)]
    struct LineAcc {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        words: Vec<String>,
        has_coords: bool,
    }

    let mut lines_map: BTreeMap<(i32, i32, i32), LineAcc> = BTreeMap::new();
    for (idx, row) in tsv.lines().enumerate() {
        if idx == 0 {
            continue;
        }
        let cols: Vec<&str> = row.split('\t').collect();
        if cols.len() < 12 || cols[0] != "5" {
            continue;
        }
        let word = cols[11].trim();
        if word.is_empty() {
            continue;
        }
        let block_num: i32 = cols[2].parse().unwrap_or(0);
        let par_num: i32 = cols[3].parse().unwrap_or(0);
        let line_num: i32 = cols[4].parse().unwrap_or(0);
        let left: f32 = cols[6].parse().unwrap_or(0.0);
        let top: f32 = cols[7].parse().unwrap_or(0.0);
        let w: f32 = cols[8].parse().unwrap_or(0.0);
        let h: f32 = cols[9].parse().unwrap_or(0.0);
        let conf: f32 = cols[10].parse().unwrap_or(-1.0);
        if conf < 0.0 && word.len() == 1 {
            continue;
        }
        let acc = lines_map.entry((block_num, par_num, line_num)).or_default();
        if !acc.has_coords {
            acc.x0 = left;
            acc.y0 = top;
            acc.x1 = left + w;
            acc.y1 = top + h;
            acc.has_coords = true;
        } else {
            acc.x0 = acc.x0.min(left);
            acc.y0 = acc.y0.min(top);
            acc.x1 = acc.x1.max(left + w);
            acc.y1 = acc.y1.max(top + h);
        }
        acc.words.push(word.to_string());
    }

    let lines: Vec<PositionedLine> = lines_map
        .into_values()
        .filter(|a| !a.words.is_empty())
        .map(|a| PositionedLine { x0: a.x0, y0: a.y0, x1: a.x1, y1: a.y1, font_size: (a.y1 - a.y0).max(10.0), text: a.words.join(" ") })
        .collect();
    cluster_lines_into_blocks(lines, img_w.max(1.0), img_h.max(1.0))
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn tesseract_extract(bytes: &[u8]) -> Option<OcrDocument> {
    let check = std::process::Command::new("tesseract").arg("--version").output().ok()?;
    if !check.status.success() {
        return None;
    }
    let img = image::load_from_memory(bytes).ok()?;
    let (img_w, img_h) = (img.width() as f32, img.height() as f32);
    if img_w < 4.0 || img_h < 4.0 {
        return None;
    }

    // Ensure clean RGB PNG for Tesseract at high contrast if low-res/scan.
    let tmp_png = std::env::temp_dir().join(format!("ergonote_ocr_{}.png", std::process::id()));
    img.to_rgb8().save_with_format(&tmp_png, image::ImageFormat::Png).ok()?;

    let dir = cache_dir();
    let fra_cached = dir.join(TESS_FRA_FILE);
    let _ = download_if_missing(TESS_FRA_URL, &fra_cached, 100_000);

    let output = if fra_cached.is_file() {
        std::process::Command::new("tesseract")
            .arg(&tmp_png)
            .arg("stdout")
            .arg("--tessdata-dir")
            .arg(&dir)
            .args(["-l", "fra", "--psm", "3", "tsv"])
            .output()
            .ok()
    } else {
        std::process::Command::new("tesseract")
            .arg(&tmp_png)
            .arg("stdout")
            .args(["-l", "fra+eng", "--psm", "3", "tsv"])
            .output()
            .ok()
            .or_else(|| std::process::Command::new("tesseract").arg(&tmp_png).arg("stdout").args(["--psm", "3", "tsv"]).output().ok())
    };
    let _ = std::fs::remove_file(&tmp_png);

    let out = output?;
    if !out.status.success() {
        return None;
    }
    let tsv = String::from_utf8_lossy(&out.stdout);
    let blocks = parse_tesseract_tsv(&tsv, img_w, img_h);
    if blocks.is_empty() {
        return None;
    }
    let plain_text = blocks.iter().map(|b| b.lines.join("\n")).collect::<Vec<_>>().join("\n\n");
    Some(OcrDocument { source_label: "Tesseract Français", aspect_ratio: safe_aspect_ratio(img_w, img_h), blocks, plain_text })
}

/// Pure-Rust `ocrs` extraction keeping bounding boxes so layout can still be reconstructed on
/// every platform (Android, Windows without Tesseract, etc.).
#[cfg(not(target_arch = "wasm32"))]
fn ocrs_extract_document(bytes: &[u8]) -> Result<OcrDocument, String> {
    use ocrs::TextItem;
    let rgb = image::load_from_memory(bytes).map_err(|e| format!("Image illisible pour l'OCR : {e}"))?.into_rgb8();
    let (img_w, img_h) = (rgb.width() as f32, rgb.height() as f32);
    let (det_path, rec_path) = ensure_models()?;
    let detection_model = rten::Model::load_file(&det_path).map_err(|e| format!("Chargement détection OCR : {e}"))?;
    let recognition_model = rten::Model::load_file(&rec_path).map_err(|e| format!("Chargement reconnaissance OCR : {e}"))?;
    let engine = ocrs::OcrEngine::new(ocrs::OcrEngineParams {
        detection_model: Some(detection_model),
        recognition_model: Some(recognition_model),
        ..Default::default()
    })
    .map_err(|e| format!("Initialisation OCR : {e}"))?;
    let source = ocrs::ImageSource::from_bytes(rgb.as_raw(), rgb.dimensions()).map_err(|e| format!("Préparation image OCR : {e}"))?;
    let input = engine.prepare_input(source).map_err(|e| format!("Analyse image OCR : {e}"))?;
    let words = engine.detect_words(&input).map_err(|e| format!("Détection texte OCR : {e}"))?;
    let line_rects = engine.find_text_lines(&input, &words);
    let recognized = engine.recognize_text(&input, &line_rects).map_err(|e| format!("Reconnaissance OCR : {e}"))?;

    let mut lines: Vec<PositionedLine> = Vec::new();
    for line in recognized.into_iter().flatten() {
        let raw = line.to_string();
        let cleaned = restore_french_accents(raw.trim());
        if cleaned.is_empty() {
            continue;
        }
        let (mut x0, mut y0, mut x1, mut y1) = (img_w, img_h, 0.0_f32, 0.0_f32);
        for ch in line.chars() {
            x0 = x0.min(ch.rect.left() as f32);
            y0 = y0.min(ch.rect.top() as f32);
            x1 = x1.max(ch.rect.right() as f32);
            y1 = y1.max(ch.rect.bottom() as f32);
        }
        if x1 <= x0 || y1 <= y0 {
            continue;
        }
        lines.push(PositionedLine { x0, y0, x1, y1, font_size: (y1 - y0).max(10.0), text: cleaned });
    }
    if lines.is_empty() {
        return Err("Aucun texte lisible n'a été détecté sur cette image.".to_string());
    }
    let blocks = cluster_lines_into_blocks(lines, img_w.max(1.0), img_h.max(1.0));
    let plain_text = blocks.iter().map(|b| b.lines.join("\n")).collect::<Vec<_>>().join("\n\n");
    Ok(OcrDocument { source_label: "OCR intégré (Rust)", aspect_ratio: safe_aspect_ratio(img_w, img_h), blocks, plain_text })
}

/// PaddleOCR PP-OCRv5 Latin/French extraction via `oar-ocr` (ONNX Runtime) on desktop.
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
fn paddle_oar_extract(bytes: &[u8]) -> Option<OcrDocument> {
    let dir = cache_dir();
    let det_path = dir.join(PADDLE_DET_FILE);
    let rec_path = dir.join(PADDLE_LATIN_REC_FILE);
    let dict_path = dir.join(PADDLE_LATIN_DICT_FILE);
    if let Err(err) = download_if_missing(PADDLE_DET_URL, &det_path, 100_000)
        .and_then(|()| download_if_missing(PADDLE_LATIN_REC_URL, &rec_path, 100_000))
        .and_then(|()| download_if_missing(PADDLE_LATIN_DICT_URL, &dict_path, 100))
    {
        log::warn!("PaddleOCR model download failed, falling back: {err}");
        return None;
    }

    let rgb = image::load_from_memory(bytes).ok()?.into_rgb8();
    let (img_w, img_h) = (rgb.width() as f32, rgb.height() as f32);
    if img_w < 4.0 || img_h < 4.0 {
        return None;
    }
    let engine = match oar_ocr::oarocr::OAROCRBuilder::new(&det_path, &rec_path, &dict_path).build() {
        Ok(engine) => engine,
        Err(err) => {
            log::warn!("PaddleOCR engine init failed, falling back: {err}");
            return None;
        }
    };
    let results = match engine.predict(vec![rgb]) {
        Ok(results) => results,
        Err(err) => {
            log::warn!("PaddleOCR prediction failed, falling back: {err}");
            return None;
        }
    };

    let mut lines = Vec::new();
    for result in results {
        for region in result.text_regions {
            let Some(raw) = region.text.as_deref() else { continue };
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }
            let x0 = region.bounding_box.x_min().max(0.0);
            let y0 = region.bounding_box.y_min().max(0.0);
            let x1 = region.bounding_box.x_max().min(img_w);
            let y1 = region.bounding_box.y_max().min(img_h);
            if !x0.is_finite() || !y0.is_finite() || !x1.is_finite() || !y1.is_finite() || x1 <= x0 || y1 <= y0 {
                continue;
            }
            lines.push(PositionedLine { x0, y0, x1, y1, font_size: (y1 - y0).max(10.0), text: trimmed.to_string() });
        }
    }
    if lines.is_empty() {
        return None;
    }
    let blocks = cluster_lines_into_blocks(lines, img_w, img_h);
    if blocks.is_empty() {
        return None;
    }
    let plain_text = blocks.iter().map(|b| b.lines.join("\n")).collect::<Vec<_>>().join("\n\n");
    Some(OcrDocument { source_label: "PaddleOCR Français (oar-ocr)", aspect_ratio: safe_aspect_ratio(img_w, img_h), blocks, plain_text })
}

/// Intelligent layout-aware extraction for PDF printouts and scanned images:
/// 1. Native PDF Unicode + layout via `pdftotext -bbox-layout` (desktop) or `hayro`.
/// 2. Desktop PaddleOCR PP-OCRv5 Latin (`oar-ocr`) for full French accents on scans/photos.
/// 3. Desktop `tesseract` and pure-Rust `ocrs` fallbacks.
#[cfg(not(target_arch = "wasm32"))]
pub fn extract_document(page_dir: &Path, image_src: &str, image_bytes: &[u8]) -> Result<OcrDocument, String> {
    if let Some((pdf_path, page_index)) = resolve_pdf_printout(page_dir, image_src)
        && let Ok(pdf_bytes) = notecraft_store::vfs::read(&pdf_path)
    {
        #[cfg(not(target_os = "android"))]
        if let Some(doc) = pdftotext_extract_page(&pdf_bytes, page_index) {
            return Ok(doc);
        }
        if let Ok(Some(doc)) = extract_pdf_page_layout(&pdf_bytes, page_index) {
            return Ok(doc);
        }
    }
    #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
    if let Some(doc) = paddle_oar_extract(image_bytes) {
        return Ok(doc);
    }
    #[cfg(not(target_os = "android"))]
    if let Some(doc) = tesseract_extract(image_bytes) {
        return Ok(doc);
    }
    ocrs_extract_document(image_bytes)
}

#[cfg(target_arch = "wasm32")]
pub fn extract_document(_page_dir: &Path, _image_src: &str, _image_bytes: &[u8]) -> Result<OcrDocument, String> {
    Err("L'extraction OCR / PDF n'est pas disponible dans la version navigateur.".to_string())
}

/// Extracts recognized text from raw image bytes (without PDF printout resolution).
#[cfg(not(target_arch = "wasm32"))]
pub fn recognize_bytes(bytes: &[u8]) -> Result<String, String> {
    #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
    if let Some(doc) = paddle_oar_extract(bytes) {
        return Ok(doc.plain_text);
    }
    #[cfg(not(target_os = "android"))]
    if let Some(doc) = tesseract_extract(bytes) {
        return Ok(doc.plain_text);
    }
    ocrs_extract_document(bytes).map(|d| d.plain_text)
}

#[cfg(target_arch = "wasm32")]
pub fn recognize_bytes(_bytes: &[u8]) -> Result<String, String> {
    Err("L'OCR hors-ligne n'est pas disponible dans la version navigateur.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    #[test]
    fn model_urls_and_cache_paths_are_well_formed() {
        assert!(DETECTION_URL.starts_with("https://huggingface.co/robertknight/ocrs/"));
        assert!(RECOGNITION_URL.starts_with("https://huggingface.co/robertknight/ocrs/"));
        assert!(TESS_FRA_URL.ends_with("/fra.traineddata"));
        assert!(PADDLE_DET_URL.starts_with("https://huggingface.co/monkt/paddleocr-onnx/"));
        assert!(PADDLE_LATIN_REC_URL.ends_with("/languages/latin/rec.onnx"));
        assert!(PADDLE_LATIN_DICT_URL.ends_with("/languages/latin/dict.txt"));
        let dir = cache_dir();
        assert!(dir.ends_with("ocr"));
    }

    #[test]
    fn resolves_pdf_printout_asset_names() {
        let tmp = notecraft_store::testing::TempDir::new("ocr-pdf-resolve");
        let assets = tmp.path().join("assets");
        std::fs::create_dir_all(&assets).expect("mkdir");
        std::fs::write(assets.join("controle.pdf"), b"%PDF-1.4").expect("pdf");
        assert_eq!(resolve_pdf_printout(tmp.path(), "assets/controle.pdf.printout-002.png"), Some((assets.join("controle.pdf"), 1)));
        assert_eq!(resolve_pdf_printout(tmp.path(), "assets/photo.png"), None);
    }

    #[test]
    fn clusters_lines_into_separate_columns_and_headings() {
        let lines = vec![
            PositionedLine { x0: 50.0, y0: 40.0, x1: 300.0, y1: 68.0, font_size: 24.0, text: "Titre".into() },
            PositionedLine { x0: 50.0, y0: 120.0, x1: 250.0, y1: 136.0, font_size: 12.0, text: "Colonne A".into() },
            PositionedLine { x0: 420.0, y0: 120.0, x1: 600.0, y1: 136.0, font_size: 12.0, text: "Colonne B".into() },
        ];
        let blocks = cluster_lines_into_blocks(lines, 700.0, 1000.0);
        assert_eq!(blocks.len(), 3);
        assert!(blocks[0].font_scale >= 1.5);
        assert!(blocks[1].x < 0.2);
        assert!(blocks[2].x > 0.5);
    }

    #[test]
    fn restores_common_french_accents() {
        assert_eq!(
            restore_french_accents("Lecon de francais pour les eleves : reponse numero 1"),
            "Leçon de français pour les élèves : réponse numéro 1"
        );
    }

    #[test]
    fn parses_tesseract_tsv_lines_into_blocks() {
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
5\t1\t1\t1\t1\t1\t100\t50\t80\t30\t95\tBonjour\n\
5\t1\t1\t1\t1\t2\t190\t50\t70\t30\t94\télèves\n\
5\t1\t2\t1\t1\t1\t100\t200\t120\t16\t93\tExercice\n";
        let blocks = parse_tesseract_tsv(tsv, 800.0, 1000.0);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].lines, vec!["Bonjour élèves"]);
        assert_eq!(blocks[1].lines, vec!["Exercice"]);
    }

    #[test]
    fn extracts_native_pdf_text_and_layout_via_hayro() {
        let content = "BT /F1 24 Tf 72 720 Td (Grand Titre) Tj ET\nBT /F1 12 Tf 72 600 Td (Paragraphe test) Tj ET";
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_owned(),
            format!("<< /Length {} >>\nstream\n{}\nendstream", content.len(), content),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        ];
        let mut pdf = b"%PDF-1.4\n".to_vec();
        let mut offsets = vec![0usize];
        for (i, obj) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            write!(pdf, "{} 0 obj\n{obj}\nendobj\n", i + 1).expect("obj");
        }
        let xref = pdf.len();
        write!(pdf, "xref\n0 {}\n0000000000 65535 f \n", offsets.len()).expect("xref");
        for off in offsets.iter().skip(1) {
            writeln!(pdf, "{off:010} 00000 n ").expect("entry");
        }
        write!(pdf, "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", offsets.len()).expect("trailer");

        let doc = extract_pdf_page_layout(&pdf, 0).expect("extract").expect("text pdf");
        assert_eq!(doc.source_label, "PDF natif (texte exact)");
        assert_eq!(doc.blocks.len(), 2);
        assert_eq!(doc.blocks[0].lines, vec!["Grand Titre"]);
        assert_eq!(doc.blocks[1].lines, vec!["Paragraphe test"]);
        assert!(doc.blocks[0].y < doc.blocks[1].y, "heading should be above body: {:?}", doc.blocks);
        assert!(doc.blocks[0].font_scale > 1.5, "heading scale should exceed body: {:?}", doc.blocks[0]);
    }

    #[test]
    fn cluster_lines_near_right_edge_and_non_finite_never_panics() {
        let lines = vec![
            PositionedLine { x0: 690.0, y0: 990.0, x1: 700.0, y1: 1000.0, font_size: 10.0, text: "99".into() },
            PositionedLine { x0: f32::NAN, y0: 10.0, x1: 40.0, y1: 20.0, font_size: 12.0, text: "bad".into() },
        ];
        let blocks = cluster_lines_into_blocks(lines, 700.0, 1000.0);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].x.is_finite() && blocks[0].width.is_finite());
        assert!(blocks[0].x + blocks[0].width <= 1.0 + 1e-4, "block exceeds page width: {:?}", blocks[0]);
        assert!(blocks[0].y + blocks[0].height <= 1.0 + 1e-4, "block exceeds page height: {:?}", blocks[0]);
    }

    #[test]
    fn parses_pdftotext_bbox_layout_with_french_accents_and_columns() {
        let xml = r#"<doc>
  <page width="600.0" height="800.0">
    <flow>
      <block xMin="50" yMin="40" xMax="350" yMax="72">
        <line xMin="50" yMin="40" xMax="350" yMax="72">
          <word xMin="50" yMin="40" xMax="120" yMax="72">Élève</word>
          <word xMin="130" yMin="40" xMax="350" yMax="72">Français</word>
        </line>
      </block>
      <block xMin="50" yMin="140" xMax="260" yMax="156">
        <line xMin="50" yMin="140" xMax="260" yMax="156">
          <word xMin="50" yMin="140" xMax="260" yMax="156">L&apos;école &amp; l&apos;été</word>
        </line>
      </block>
      <block xMin="330" yMin="140" xMax="540" yMax="156">
        <line xMin="330" yMin="140" xMax="540" yMax="156">
          <word xMin="330" yMin="140" xMax="540" yMax="156">Réponse n&#176; 2</word>
        </line>
      </block>
    </flow>
  </page>
</doc>"#;
        let doc = parse_pdftotext_bbox_layout(xml).expect("poppler layout");
        assert_eq!(doc.source_label, "PDF natif exact (Poppler)");
        assert_eq!(doc.blocks.len(), 3);
        assert_eq!(doc.blocks[0].lines, vec!["Élève Français"]);
        assert!(doc.blocks[0].font_scale > 1.5);
        assert_eq!(doc.blocks[1].lines, vec!["L'école & l'été"]);
        assert_eq!(doc.blocks[2].lines, vec!["Réponse n° 2"]);
        assert!(doc.blocks[2].x > doc.blocks[1].x);
        assert!(!doc.is_single_column());
    }

    #[test]
    fn merges_standalone_bullets_joins_wrapped_lines_and_detects_single_column() {
        let xml = r#"<doc>
  <page width="595.3" height="841.9">
    <flow>
      <block xMin="36" yMin="43" xMax="495" yMax="118">
        <line xMin="36" yMin="43" xMax="495" yMax="87"><word xMin="36" yMin="43" xMax="200" yMax="87">Cours Magistral</word></line>
        <line xMin="36" yMin="75" xMax="102" yMax="118"><word xMin="36" yMin="75" xMax="102" yMax="118">(RAE)</word></line>
      </block>
      <block xMin="36" yMin="116" xMax="525" yMax="138">
        <line xMin="36" yMin="116" xMax="525" yMax="138"><word xMin="36" yMin="116" xMax="525" yMax="138">Enseignante : Pédopsychiatre</word></line>
      </block>
      <block xMin="36" yMin="139" xMax="252" yMax="172">
        <line xMin="36" yMin="139" xMax="252" yMax="172"><word xMin="36" yMin="139" xMax="252" yMax="172">1. Introduction et Définition</word></line>
      </block>
      <block xMin="53.4" yMin="177.8" xMax="58.5" yMax="188.8">
        <line xMin="53.4" yMin="177.8" xMax="58.5" yMax="188.8"><word xMin="53.4" yMin="177.8" xMax="58.5" yMax="188.8">·</word></line>
      </block>
      <block xMin="71.4" yMin="169.3" xMax="526.8" yMax="210">
        <line xMin="71.4" yMin="169.3" xMax="526.8" yMax="191.6"><word xMin="71.4" yMin="169.3" xMax="526.8" yMax="191.6">Nature du trouble : volonté délibérée</word></line>
        <line xMin="71.4" yMin="185.2" xMax="300" yMax="207.5"><word xMin="71.4" yMin="185.2" xMax="300" yMax="207.5">d'enfreindre l'école.</word></line>
        <line xMin="89.4" yMin="210" xMax="340" yMax="232"><word xMin="89.4" yMin="210" xMax="96" yMax="228">o</word><word xMin="107.4" yMin="210" xMax="340" yMax="232">Incidents en milieu urbain.</word></line>
      </block>
    </flow>
  </page>
</doc>"#;
        let doc = parse_pdftotext_bbox_layout(xml).expect("course handout");
        assert!(doc.is_single_column());
        assert_eq!(doc.blocks[0].lines, vec!["Cours Magistral (RAE)"]);
        assert!(doc.blocks[0].font_scale >= 1.7);
        assert_eq!(doc.blocks[2].lines, vec!["1. Introduction et Définition"]);
        assert!(doc.blocks[2].font_scale >= 1.32);
        assert!(doc.blocks.iter().all(|b| b.lines != vec!["·"] && b.lines != vec!["o"]), "no orphan bullet blocks: {:?}", doc.blocks);
        assert_eq!(doc.blocks[3].lines, vec!["- Nature du trouble : volonté délibérée d'enfreindre l'école.", "  - Incidents en milieu urbain."]);
    }

    #[test]
    fn merges_stacked_centered_header_lines_while_keeping_date_separate() {
        let xml = r#"<doc>
  <page width="595.3" height="841.9">
    <flow>
      <block xMin="110" yMin="90" xMax="250" yMax="104"><line xMin="110" yMin="90" xMax="250" yMax="104"><word xMin="110" yMin="90" xMax="250" yMax="104">MÉDECIN GÉNÉRALISTE</word></line></block>
      <block xMin="125" yMin="106" xMax="235" yMax="120"><line xMin="125" yMin="106" xMax="235" yMax="120"><word xMin="125" yMin="106" xMax="235" yMax="120">12 G Rue Gambetta</word></line></block>
      <block xMin="115" yMin="122" xMax="245" yMax="136"><line xMin="115" yMin="122" xMax="245" yMax="136"><word xMin="115" yMin="122" xMax="245" yMax="136">42400 Saint-Chamond</word></line></block>
      <block xMin="95" yMin="138" xMax="265" yMax="152"><line xMin="95" yMin="138" xMax="265" yMax="152"><word xMin="95" yMin="138" xMax="265" yMax="152">Tel: 04 77 22 31 77</word></line></block>
      <block xMin="445" yMin="180" xMax="530" yMax="194"><line xMin="445" yMin="180" xMax="530" yMax="194"><word xMin="445" yMin="180" xMax="530" yMax="194">Le 09 juil. 2026</word></line></block>
    </flow>
  </page>
</doc>"#;
        let doc = parse_pdftotext_bbox_layout(xml).expect("doctor letter");
        assert_eq!(doc.blocks.len(), 2, "4 stacked header lines should merge into 1 block: {:?}", doc.blocks);
        assert_eq!(doc.blocks[0].lines, vec!["MÉDECIN GÉNÉRALISTE", "12 G Rue Gambetta", "42400 Saint-Chamond", "Tel: 04 77 22 31 77"]);
        assert_eq!(doc.blocks[1].lines, vec!["Le 09 juil. 2026"]);
    }
}
