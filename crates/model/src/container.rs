//! Containers: the level between a page and its paragraphs.
//!
//! A page in OneNote is not one column of text. It is a sheet with boxes on it, each holding
//! paragraphs, pictures and tables, each placed where the writer put it. ErgoNote calls
//! those containers, and this is what one is: a place on the page, a width, and how many of
//! the page's blocks belong to it.
//!
//! **Why only a count.** The blocks themselves stay where they are, in the page's own HTML or
//! Markdown, in reading order. A container says "the next three blocks are mine". So a page
//! read by anything else, or edited in another program, is still the same document in the
//! same order; the boxes are a layer over it, kept in the page's metadata. If the counts and
//! the blocks ever disagree, [`fit`] puts everything in one container rather than losing a
//! word.

use crate::NodeColor;

/// One box on the page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Container {
    /// Points from the left edge of the page.
    pub x: f32,
    /// Points from the top of the page.
    pub y: f32,
    /// How wide the box is, in points.
    pub width: f32,
    /// How many of the page's blocks, in order, are in this box.
    pub blocks: usize,
    /// Whether the box always draws a visible border even when not hovered.
    pub framed: bool,
    /// Optional custom border colour (`None` uses the theme's border colour).
    pub border_color: Option<NodeColor>,
    /// Border stroke thickness in points (`1..=3`).
    pub border_width: u8,
    /// Optional custom background fill colour (`None` uses the page paper).
    pub fill_color: Option<NodeColor>,
}

/// The narrowest a container may be, so one cannot be dragged into nothing.
pub const MIN_WIDTH: f32 = 120.0;

impl Container {
    pub fn new(x: f32, y: f32, width: f32, blocks: usize) -> Container {
        Container {
            x: x.max(0.0),
            y: y.max(0.0),
            width: width.max(MIN_WIDTH),
            blocks,
            framed: false,
            border_color: None,
            border_width: 1,
            fill_color: None,
        }
    }

    pub fn with_frame(mut self, framed: bool) -> Self {
        self.framed = framed;
        self
    }

    pub fn has_style(&self) -> bool {
        self.framed || self.border_color.is_some() || self.fill_color.is_some() || self.border_width > 1
    }

    pub fn with_geometry(mut self, x: f32, y: f32, width: f32) -> Self {
        self.x = x.max(0.0);
        self.y = y.max(0.0);
        self.width = width.max(MIN_WIDTH);
        self
    }
}

/// Reads the line a page keeps its containers on: `x,y,width,blocks` per box (with optional
/// `key=value` style tokens), separated by semicolons. Anything that does not parse is skipped
/// rather than guessed at.
pub fn parse(text: &str) -> Vec<Container> {
    text.split(';')
        .filter_map(|part| {
            let mut fields = part.split(',').map(str::trim);
            let x: f32 = fields.next()?.parse().ok()?;
            let y: f32 = fields.next()?.parse().ok()?;
            let width: f32 = fields.next()?.parse().ok()?;
            let blocks: usize = fields.next()?.parse().ok()?;
            if !x.is_finite() || !y.is_finite() || !width.is_finite() {
                return None;
            }
            let mut c = Container::new(x, y, width, blocks);
            for tok in fields {
                let (key, val) = tok.split_once('=')?;
                match key.trim() {
                    "frame" => match val.trim() {
                        "1" => c.framed = true,
                        "0" => c.framed = false,
                        _ => return None,
                    },
                    "border" => c.border_color = Some(NodeColor::parse(val)?),
                    "bw" => {
                        let bw: u8 = val.trim().parse().ok()?;
                        if !(1..=3).contains(&bw) {
                            return None;
                        }
                        c.border_width = bw;
                    }
                    "fill" => c.fill_color = Some(NodeColor::parse(val)?),
                    _ => return None,
                }
            }
            Some(c)
        })
        .collect()
}

/// Writes them back out the same way. Whole points: a page is not a drawing program, and
/// the shorter line is the one a person can read in the file.
pub fn write(containers: &[Container]) -> String {
    containers
        .iter()
        .map(|c| {
            let mut out = format!("{},{},{},{}", c.x.round() as i64, c.y.round() as i64, c.width.round() as i64, c.blocks);
            if c.framed {
                out.push_str(",frame=1");
            }
            if let Some(bc) = c.border_color {
                out.push_str(",border=");
                out.push_str(bc.id());
            }
            if c.border_width > 1 {
                out.push_str(&format!(",bw={}", c.border_width.clamp(1, 3)));
            }
            if let Some(fc) = c.fill_color {
                out.push_str(",fill=");
                out.push_str(fc.id());
            }
            out
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// The containers to use for a page with `blocks` blocks in it.
///
/// This is where honesty about a count-based layer lives. If the containers account for
/// exactly the blocks there are, they are used as they are. If they do not, because the page
/// was edited somewhere else, everything goes into one container and nothing is lost.
pub fn fit(containers: &[Container], blocks: usize, width: f32) -> Vec<Container> {
    // An empty page has no box until the writer clicks on its paper. Keeping a
    // zero-block placeholder here makes that placeholder claim the click before
    // the page-level hit target can create the first real box.
    if blocks == 0 {
        return containers.to_vec();
    }
    let counted: usize = containers.iter().map(|c| c.blocks).sum();
    if !containers.is_empty() && counted == blocks {
        return containers.to_vec();
    }
    vec![Container::new(0.0, 0.0, width, blocks)]
}

/// Which container a block belongs to, and which block of that container it is.
pub fn place(containers: &[Container], block: usize) -> Option<(usize, usize)> {
    let mut first = 0;
    for (i, c) in containers.iter().enumerate() {
        if block < first + c.blocks {
            return Some((i, block - first));
        }
        first += c.blocks;
    }
    None
}

/// The range of blocks in container `at`.
pub fn blocks_of(containers: &[Container], at: usize) -> std::ops::Range<usize> {
    let first: usize = containers.iter().take(at).map(|c| c.blocks).sum();
    let count = containers.get(at).map(|c| c.blocks).unwrap_or(0);
    first..first + count
}

/// Adds a container of its own for `added` new blocks that were appended to a page that had
/// `blocks_before` blocks. It goes at the left edge, `gap` below `below` (how far down the
/// existing containers reach), or at the top of an empty page.
pub fn append(containers: &[Container], blocks_before: usize, added: usize, width: f32, below: f32, gap: f32) -> Vec<Container> {
    let mut out = fit(containers, blocks_before, width);
    let y = if out.is_empty() { 0.0 } else { below + gap };
    out.push(Container::new(0.0, y, width, added));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_appended_container_sits_below_the_others_and_counts_every_block() {
        let boxes = vec![Container::new(0.0, 0.0, 300.0, 2), Container::new(400.0, 50.0, 200.0, 1)];
        let out = append(&boxes, 3, 4, 300.0, 180.0, 12.0);
        assert_eq!(out.len(), 3);
        assert_eq!(out[..2], boxes[..]);
        assert_eq!((out[2].x, out[2].y, out[2].blocks), (0.0, 192.0, 4));
        assert_eq!(out.iter().map(|c| c.blocks).sum::<usize>(), 7);
    }

    #[test]
    fn appending_to_an_unarranged_page_keeps_its_words_in_one_container_first() {
        let out = append(&[], 5, 2, 360.0, 0.0, 12.0);
        assert_eq!(out.iter().map(|c| c.blocks).collect::<Vec<_>>(), vec![5, 2]);
        assert!(out[1].y > 0.0);
    }

    #[test]
    fn appending_to_an_empty_page_starts_at_the_top() {
        let out = append(&[], 0, 3, 360.0, 0.0, 12.0);
        assert_eq!(out, vec![Container::new(0.0, 0.0, 360.0, 3)]);
    }

    #[test]
    fn a_line_of_containers_round_trips() {
        let boxes = vec![Container::new(0.0, 0.0, 420.0, 3), Container::new(460.0, 24.0, 300.0, 2)];
        let line = write(&boxes);
        assert_eq!(line, "0,0,420,3; 460,24,300,2");
        assert_eq!(parse(&line), boxes);
    }

    #[test]
    fn nonsense_is_skipped_not_guessed_at() {
        assert!(parse("").is_empty());
        assert!(parse("garbage").is_empty());
        assert!(parse("1,2,3").is_empty(), "four fields or nothing");
        assert!(parse("1,2,3,4,5").is_empty());
        assert!(parse("x,y,z,w").is_empty());
        assert!(parse("1,2,NaN,4").is_empty(), "a box of no width is not a box");
        // One good box among the rubbish is still read.
        assert_eq!(parse("junk; 10,20,300,2").len(), 1);
    }

    #[test]
    fn a_box_has_a_floor_under_its_size() {
        let c = Container::new(-50.0, -10.0, 4.0, 1);
        assert_eq!((c.x, c.y), (0.0, 0.0), "a box stays on the page");
        assert_eq!(c.width, MIN_WIDTH, "and stays wide enough to write in");
    }

    #[test]
    fn containers_that_do_not_match_the_page_are_not_trusted() {
        let two = vec![Container::new(0.0, 0.0, 400.0, 2), Container::new(420.0, 0.0, 200.0, 1)];
        assert_eq!(fit(&two, 3, 700.0), two, "three blocks, three accounted for");
        // The page gained a block somewhere else: everything goes in one box instead.
        let fitted = fit(&two, 4, 700.0);
        assert_eq!(fitted.len(), 1);
        assert_eq!(fitted.first().map(|c| (c.blocks, c.width)), Some((4, 700.0)));
        // No containers at all is the same thing: one box holding the page.
        assert_eq!(fit(&[], 2, 700.0), vec![Container::new(0.0, 0.0, 700.0, 2)]);
    }

    #[test]
    fn an_empty_page_has_no_placeholder_box() {
        assert!(fit(&[], 0, 700.0).is_empty());
        assert_eq!(fit(&[Container::new(0.0, 0.0, 400.0, 0)], 0, 700.0).len(), 1);
    }

    #[test]
    fn a_block_knows_which_box_it_is_in() {
        let boxes = vec![Container::new(0.0, 0.0, 400.0, 2), Container::new(420.0, 0.0, 200.0, 3)];
        assert_eq!(place(&boxes, 0), Some((0, 0)));
        assert_eq!(place(&boxes, 1), Some((0, 1)));
        assert_eq!(place(&boxes, 2), Some((1, 0)));
        assert_eq!(place(&boxes, 4), Some((1, 2)));
        assert_eq!(place(&boxes, 5), None, "there is no sixth block");
        assert_eq!(blocks_of(&boxes, 0), 0..2);
        assert_eq!(blocks_of(&boxes, 1), 2..5);
        assert_eq!(blocks_of(&boxes, 9), 5..5);
    }

    #[test]
    fn styled_containers_round_trip_and_invalid_tokens_are_rejected() {
        let styled = Container {
            border_color: Some(NodeColor::Blue),
            border_width: 2,
            fill_color: Some(NodeColor::Yellow),
            ..Container::new(120.0, 40.0, 280.0, 1).with_frame(true)
        };
        let plain = Container::new(0.0, 0.0, 400.0, 2);
        let written = write(&[plain, styled]);
        assert_eq!(written, "0,0,400,2; 120,40,280,1,frame=1,border=blue,bw=2,fill=yellow");
        assert_eq!(parse(&written), vec![plain, styled]);

        // Unstyled stays short
        assert_eq!(write(&[plain]), "0,0,400,2");

        // Geometry update preserves style
        let moved = styled.with_geometry(50.0, 60.0, 320.0);
        assert_eq!(moved.x, 50.0);
        assert_eq!(moved.y, 60.0);
        assert_eq!(moved.width, 320.0);
        assert!(moved.framed);
        assert_eq!(moved.border_color, Some(NodeColor::Blue));
        assert_eq!(moved.border_width, 2);
        assert_eq!(moved.fill_color, Some(NodeColor::Yellow));

        // Invalid extra tokens are still rejected
        assert!(parse("1,2,3,4,5").is_empty());
        assert!(parse("1,2,3,4,border=invalid").is_empty());
        assert!(parse("1,2,3,4,fill=nope").is_empty());
        assert!(parse("1,2,3,4,bw=9").is_empty());
        assert!(parse("1,2,3,4,frame=yes").is_empty());
    }
}
