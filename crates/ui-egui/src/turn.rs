//! Rotate and Flip for a picture on the page (issue #273).
//!
//! A picture is turned by writing a turned copy beside the page and pointing the page at it,
//! not by changing the file the page named: the original stays in the page's `assets` folder,
//! so Undo has the very file it pointed at before, and a picture shared with another page or
//! linked from elsewhere is never changed under it. Neither page format has a way to say
//! "drawn turned" that the other could read, and a turned file reads the same everywhere.

use std::io::Cursor;

/// One of the four turns the picture menu offers, as OneNote and Word have them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Turn {
    /// A quarter turn clockwise.
    Right,
    /// A quarter turn anticlockwise.
    Left,
    /// Mirrored left to right.
    FlipHorizontal,
    /// Mirrored top to bottom.
    FlipVertical,
}

impl Turn {
    /// True for a quarter turn, which swaps the picture's width and height.
    pub fn swaps_sides(self) -> bool {
        matches!(self, Turn::Right | Turn::Left)
    }

    /// What the turned copy's name says it is.
    fn tag(self) -> &'static str {
        match self {
            Turn::Right | Turn::Left => "rotated",
            Turn::FlipHorizontal | Turn::FlipVertical => "flipped",
        }
    }
}

/// Visual enhancement presets for photos and scanned worksheets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Enhance {
    /// Clean greyscale with lifted brightness & contrast for dark photocopies.
    ScanBw,
    /// Stronger contrast while keeping colours.
    Contrast,
}

impl Enhance {
    pub fn tag(self) -> &'static str {
        match self {
            Enhance::ScanBw => "scan",
            Enhance::Contrast => "contrast",
        }
    }
}

fn encode_like(picture: &image::DynamicImage, format: image::ImageFormat) -> Result<(Vec<u8>, &'static str), String> {
    let mut out = Cursor::new(Vec::new());
    if format == image::ImageFormat::Jpeg {
        let rgb = picture.to_rgb8();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 92);
        encoder.encode_image(&rgb).map_err(|e| e.to_string())?;
        return Ok((out.into_inner(), "jpg"));
    }
    picture.write_to(&mut out, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok((out.into_inner(), "png"))
}

/// Crops `bytes` to the normalized `[x0, y0, x1, y1]` rectangle (`0.0..=1.0`), enforcing at
/// least a 4x4 pixel region, and returns the encoded image bytes, extension, and pixel size.
pub fn crop(bytes: &[u8], rect_norm: [f32; 4]) -> Result<(Vec<u8>, &'static str, (u32, u32)), String> {
    let format = image::guess_format(bytes).map_err(|e| e.to_string())?;
    let picture = image::load_from_memory_with_format(bytes, format).map_err(|e| e.to_string())?;
    let (pw, ph) = (picture.width(), picture.height());
    if pw < 4 || ph < 4 {
        return Err("L'image est trop petite pour être rognée.".to_string());
    }
    let x0 = rect_norm[0].clamp(0.0, 1.0);
    let y0 = rect_norm[1].clamp(0.0, 1.0);
    let x1 = rect_norm[2].clamp(0.0, 1.0);
    let y1 = rect_norm[3].clamp(0.0, 1.0);
    let (ax, bx) = (x0.min(x1), x0.max(x1));
    let (ay, by) = (y0.min(y1), y0.max(y1));
    let mut x = ((ax * pw as f32).round() as u32).min(pw.saturating_sub(4));
    let mut y = ((ay * ph as f32).round() as u32).min(ph.saturating_sub(4));
    let right = ((bx * pw as f32).round() as u32).clamp(x + 4, pw);
    let bottom = ((by * ph as f32).round() as u32).clamp(y + 4, ph);
    if right <= x || bottom <= y {
        x = 0;
        y = 0;
    }
    let w = (right - x).clamp(4, pw - x);
    let h = (bottom - y).clamp(4, ph - y);
    let cropped = picture.crop_imm(x, y, w, h);
    let (out, ext) = encode_like(&cropped, format)?;
    Ok((out, ext, (w, h)))
}

/// Enhances contrast or converts a dark scan into a cleaner greyscale copy.
pub fn enhance(bytes: &[u8], mode: Enhance) -> Result<(Vec<u8>, &'static str), String> {
    let format = image::guess_format(bytes).map_err(|e| e.to_string())?;
    let picture = image::load_from_memory_with_format(bytes, format).map_err(|e| e.to_string())?;
    let enhanced = match mode {
        Enhance::ScanBw => picture.grayscale().adjust_contrast(35.0).brighten(12),
        Enhance::Contrast => picture.adjust_contrast(22.0),
    };
    encode_like(&enhanced, format)
}

/// Turns the picture in `bytes`. Returns the turned picture's bytes and the extension it is
/// written with: a JPEG stays a JPEG (a photo would grow several times over as a PNG), and
/// everything else becomes a PNG, which loses nothing. A picture that cannot be read (an SVG,
/// a JPEG XL, a damaged file) is an error, and the page is left as it was.
pub fn turn(bytes: &[u8], how: Turn) -> Result<(Vec<u8>, &'static str), String> {
    let format = image::guess_format(bytes).map_err(|e| e.to_string())?;
    let picture = image::load_from_memory_with_format(bytes, format).map_err(|e| e.to_string())?;
    let turned = match how {
        Turn::Right => picture.rotate90(),
        Turn::Left => picture.rotate270(),
        Turn::FlipHorizontal => picture.fliph(),
        Turn::FlipVertical => picture.flipv(),
    };
    let mut out = Cursor::new(Vec::new());
    if format == image::ImageFormat::Jpeg {
        let rgb = turned.to_rgb8();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 92);
        encoder.encode_image(&rgb).map_err(|e| e.to_string())?;
        return Ok((out.into_inner(), "jpg"));
    }
    turned.write_to(&mut out, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok((out.into_inner(), "png"))
}

/// The name the turned copy is offered under: `photo.png` becomes `photo-rotated.png`. A
/// copy of a copy keeps one tag, so turning a picture four times does not grow its name four
/// times; the store adds a number when the name is taken.
pub fn turned_name(src: &str, how: Turn, ext: &str) -> String {
    edited_name(src, how.tag(), ext)
}

/// Builds a sibling asset filename carrying at most one edit suffix (`rotated`, `flipped`,
/// `cropped`, `scan`, `contrast`) so repeated edits keep concise filenames.
pub fn edited_name(src: &str, tag: &str, ext: &str) -> String {
    let file = src.rsplit('/').next().unwrap_or(src);
    let stem = file.rsplit_once('.').map_or(file, |(s, _)| s);
    let mut base = stem;
    for suffix in ["-rotated", "-flipped", "-cropped", "-scan", "-contrast", "-bg"] {
        if let Some(cut) = base.find(suffix) {
            base = &base[..cut];
        }
    }
    let base = if base.is_empty() { "picture" } else { base };
    format!("{base}-{tag}.{ext}")
}

/// Fills each normalized `(x, y, width, height)` region with the sampled local background colour
/// so original text disappears while keeping borders, tables, and illustrations intact.
pub fn erase_text_regions(bytes: &[u8], regions: &[(f32, f32, f32, f32)]) -> Result<Vec<u8>, String> {
    let format = image::guess_format(bytes).map_err(|e| e.to_string())?;
    let mut rgba = image::load_from_memory_with_format(bytes, format).map_err(|e| e.to_string())?.to_rgba8();
    let (pw, ph) = (rgba.width(), rgba.height());
    if pw == 0 || ph == 0 {
        return Err("Image vide.".to_string());
    }
    for &(rx, ry, rw, rh) in regions {
        if !rx.is_finite() || !ry.is_finite() || !rw.is_finite() || !rh.is_finite() || rw <= 0.0 || rh <= 0.0 {
            continue;
        }
        let x0 = ((rx * pw as f32).floor() as i32 - 2).clamp(0, pw as i32 - 1) as u32;
        let y0 = ((ry * ph as f32).floor() as i32 - 2).clamp(0, ph as i32 - 1) as u32;
        let x1 = (((rx + rw) * pw as f32).ceil() as i32 + 2).clamp(0, pw as i32) as u32;
        let y1 = (((ry + rh) * ph as f32).ceil() as i32 + 2).clamp(0, ph as i32) as u32;
        if x1 <= x0 || y1 <= y0 {
            continue;
        }
        let mut samples = Vec::new();
        let top_y = y0.saturating_sub(2);
        let bot_y = y1.min(ph - 1);
        for x in x0..x1 {
            samples.push(rgba.get_pixel(x, top_y).0);
            samples.push(rgba.get_pixel(x, bot_y).0);
        }
        let left_x = x0.saturating_sub(2);
        let right_x = x1.min(pw - 1);
        for y in y0..y1 {
            samples.push(rgba.get_pixel(left_x, y).0);
            samples.push(rgba.get_pixel(right_x, y).0);
        }
        let fill = if samples.is_empty() {
            image::Rgba([255, 255, 255, 255])
        } else {
            // Pick the 75th-percentile brightest perimeter pixel so adjacent dark borders don't
            // darken the cleaned paper area.
            samples.sort_by_key(|p| u32::from(p[0]) * 299 + u32::from(p[1]) * 587 + u32::from(p[2]) * 114);
            let pick = samples[(samples.len() * 3 / 4).min(samples.len() - 1)];
            image::Rgba(pick)
        };
        for y in y0..y1 {
            for x in x0..x1 {
                rgba.put_pixel(x, y, fill);
            }
        }
    }
    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(rgba).write_to(&mut out, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 3 by 2 picture whose top-left pixel is red and the rest white.
    fn picture() -> Vec<u8> {
        let mut img = image::RgbaImage::from_pixel(3, 2, image::Rgba([255, 255, 255, 255]));
        img.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        let mut out = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img).write_to(&mut out, image::ImageFormat::Png).expect("png");
        out.into_inner()
    }

    fn read(bytes: &[u8]) -> image::RgbaImage {
        image::load_from_memory(bytes).expect("read").to_rgba8()
    }

    fn red_at(img: &image::RgbaImage) -> (u32, u32) {
        img.enumerate_pixels().find(|(_, _, p)| p.0 == [255, 0, 0, 255]).map(|(x, y, _)| (x, y)).expect("the red pixel")
    }

    #[test]
    fn each_turn_moves_the_corner_where_it_should() {
        let src = picture();
        for (how, size, corner) in [
            (Turn::Right, (2, 3), (1, 0)),
            (Turn::Left, (2, 3), (0, 2)),
            (Turn::FlipHorizontal, (3, 2), (2, 0)),
            (Turn::FlipVertical, (3, 2), (0, 1)),
        ] {
            let (bytes, ext) = turn(&src, how).expect("turned");
            assert_eq!(ext, "png");
            let img = read(&bytes);
            assert_eq!(img.dimensions(), size, "{how:?}");
            assert_eq!(red_at(&img), corner, "{how:?}");
            assert_eq!(how.swaps_sides(), size != (3, 2));
        }
        // Four quarter turns are the picture again.
        let mut bytes = src.clone();
        for _ in 0..4 {
            bytes = turn(&bytes, Turn::Right).expect("turned").0;
        }
        assert_eq!(read(&bytes), read(&src));
    }

    #[test]
    fn a_jpeg_stays_a_jpeg_and_nonsense_is_refused() {
        let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(4, 2, image::Rgb([10, 200, 30])));
        let mut jpeg = Cursor::new(Vec::new());
        img.write_to(&mut jpeg, image::ImageFormat::Jpeg).expect("jpeg");
        let (bytes, ext) = turn(&jpeg.into_inner(), Turn::Left).expect("turned");
        assert_eq!(ext, "jpg");
        assert_eq!(image::guess_format(&bytes).expect("format"), image::ImageFormat::Jpeg);
        assert_eq!(read(&bytes).dimensions(), (2, 4));
        assert!(turn(b"<svg xmlns='http://www.w3.org/2000/svg'/>", Turn::Right).is_err());
    }

    #[test]
    fn the_copy_is_named_after_the_picture_once() {
        assert_eq!(turned_name("assets/photo.png", Turn::Right, "png"), "photo-rotated.png");
        assert_eq!(turned_name("assets/photo-rotated.png", Turn::Left, "png"), "photo-rotated.png");
        assert_eq!(turned_name("assets/photo-rotated-2.jpg", Turn::FlipVertical, "jpg"), "photo-flipped.jpg");
        assert_eq!(turned_name("assets/scan.webp", Turn::FlipHorizontal, "png"), "scan-flipped.png");
        assert_eq!(turned_name("assets/.png", Turn::Right, "png"), "picture-rotated.png");
        assert_eq!(edited_name("assets/scan-cropped.png", Enhance::ScanBw.tag(), "png"), "scan-scan.png");
    }

    #[test]
    fn crop_and_enhance_produce_valid_images() {
        let big = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(20, 10, image::Rgba([40, 80, 120, 255])));
        let mut raw = Cursor::new(Vec::new());
        big.write_to(&mut raw, image::ImageFormat::Png).expect("png");
        let bytes = raw.into_inner();

        let (cropped, ext, dims) = crop(&bytes, [0.25, 0.2, 0.75, 0.8]).expect("crop");
        assert_eq!(ext, "png");
        assert_eq!(dims, (10, 6));
        assert_eq!(read(&cropped).dimensions(), (10, 6));

        let (scan_bytes, scan_ext) = enhance(&bytes, Enhance::ScanBw).expect("scan bw");
        assert_eq!(scan_ext, "png");
        assert_eq!(read(&scan_bytes).dimensions(), (20, 10));

        let (contrast_bytes, contrast_ext) = enhance(&bytes, Enhance::Contrast).expect("contrast");
        assert_eq!(contrast_ext, "png");
        assert_eq!(read(&contrast_bytes).dimensions(), (20, 10));
    }

    #[test]
    fn erase_text_regions_clears_interior_and_keeps_outer_frame() {
        let mut img = image::RgbaImage::from_pixel(40, 40, image::Rgba([255, 255, 255, 255]));
        // Outer red frame at pixel (2, 2) must survive; dark black pixels in center must be erased.
        img.put_pixel(2, 2, image::Rgba([220, 30, 30, 255]));
        for y in 15..25 {
            for x in 15..25 {
                img.put_pixel(x, y, image::Rgba([10, 10, 10, 255]));
            }
        }
        let mut raw = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img).write_to(&mut raw, image::ImageFormat::Png).expect("png");
        let erased = erase_text_regions(&raw.into_inner(), &[(0.375, 0.375, 0.25, 0.25)]).expect("erase");
        let check = read(&erased);
        assert_eq!(check.get_pixel(2, 2).0, [220, 30, 30, 255]);
        assert_eq!(check.get_pixel(20, 20).0, [255, 255, 255, 255]);
    }
}
