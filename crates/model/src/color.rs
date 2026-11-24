//! The colour a notebook, section group or section can carry, the way OneNote colours its
//! notebook covers and section tabs. A fixed, named set: a name survives a theme change and
//! stays readable in both light and dark, which a free hex value would not.

/// The colours a container can be given. `None` means the theme's own colour for that kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeColor {
    Red,
    Coral,
    Orange,
    Amber,
    Yellow,
    Lime,
    Green,
    Teal,
    Cyan,
    Blue,
    Navy,
    Purple,
    Lavender,
    Magenta,
    Pink,
    Gray,
}

impl NodeColor {
    pub const ALL: [NodeColor; 16] = [
        NodeColor::Red,
        NodeColor::Coral,
        NodeColor::Orange,
        NodeColor::Amber,
        NodeColor::Yellow,
        NodeColor::Lime,
        NodeColor::Green,
        NodeColor::Teal,
        NodeColor::Cyan,
        NodeColor::Blue,
        NodeColor::Navy,
        NodeColor::Purple,
        NodeColor::Lavender,
        NodeColor::Magenta,
        NodeColor::Pink,
        NodeColor::Gray,
    ];

    pub fn id(self) -> &'static str {
        match self {
            NodeColor::Red => "red",
            NodeColor::Coral => "coral",
            NodeColor::Orange => "orange",
            NodeColor::Amber => "amber",
            NodeColor::Yellow => "yellow",
            NodeColor::Lime => "lime",
            NodeColor::Green => "green",
            NodeColor::Teal => "teal",
            NodeColor::Cyan => "cyan",
            NodeColor::Blue => "blue",
            NodeColor::Navy => "navy",
            NodeColor::Purple => "purple",
            NodeColor::Lavender => "lavender",
            NodeColor::Magenta => "magenta",
            NodeColor::Pink => "pink",
            NodeColor::Gray => "gray",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            NodeColor::Red => "Red",
            NodeColor::Coral => "Coral",
            NodeColor::Orange => "Orange",
            NodeColor::Amber => "Amber",
            NodeColor::Yellow => "Yellow",
            NodeColor::Lime => "Lime",
            NodeColor::Green => "Green",
            NodeColor::Teal => "Teal",
            NodeColor::Cyan => "Cyan",
            NodeColor::Blue => "Blue",
            NodeColor::Navy => "Navy",
            NodeColor::Purple => "Purple",
            NodeColor::Lavender => "Lavender",
            NodeColor::Magenta => "Magenta",
            NodeColor::Pink => "Pink",
            NodeColor::Gray => "Gray",
        }
    }

    /// Contrast-safe colour on a light or dark surface (used for text, rule lines, and outlines).
    pub fn rgb(self, dark: bool) -> (u8, u8, u8) {
        let (light, night) = match self {
            NodeColor::Red => ((0xC0, 0x39, 0x2B), (0xFF, 0x8A, 0x80)),
            NodeColor::Coral => ((0xB8, 0x45, 0x3B), (0xFF, 0x9E, 0x94)),
            NodeColor::Orange => ((0xC2, 0x5A, 0x11), (0xFF, 0xA8, 0x62)),
            NodeColor::Amber => ((0xB4, 0x69, 0x00), (0xFF, 0xBE, 0x4F)),
            NodeColor::Yellow => ((0xA8, 0x74, 0x00), (0xF2, 0xC9, 0x4C)),
            NodeColor::Lime => ((0x4D, 0x7C, 0x0F), (0xA3, 0xE6, 0x35)),
            NodeColor::Green => ((0x1E, 0x8A, 0x44), (0x6C, 0xD0, 0x8A)),
            NodeColor::Teal => ((0x0F, 0x82, 0x8C), (0x5A, 0xCF, 0xD8)),
            NodeColor::Cyan => ((0x0E, 0x74, 0x90), (0x67, 0xE8, 0xF9)),
            NodeColor::Blue => ((0x1A, 0x5F, 0xD0), (0x8F, 0xB8, 0xFF)),
            NodeColor::Navy => ((0x1E, 0x3A, 0x8A), (0x93, 0xA8, 0xF5)),
            NodeColor::Purple => ((0x7A, 0x45, 0xB8), (0xB0, 0x85, 0xE8)),
            NodeColor::Lavender => ((0x6D, 0x4C, 0xC4), (0xC4, 0xB5, 0xFD)),
            NodeColor::Magenta => ((0xA8, 0x27, 0x7A), (0xF0, 0x8A, 0xCE)),
            NodeColor::Pink => ((0xBE, 0x18, 0x5D), (0xF9, 0xA8, 0xD4)),
            NodeColor::Gray => ((0x5B, 0x64, 0x72), (0xB4, 0xBC, 0xC8)),
        };
        if dark { night } else { light }
    }

    /// Vivid swatch / section tab accent colour so yellows, limes, and cyans look unmistakable.
    pub fn swatch_rgb(self, dark: bool) -> (u8, u8, u8) {
        if dark {
            return self.rgb(true);
        }
        match self {
            NodeColor::Red => (0xDC, 0x26, 0x26),
            NodeColor::Coral => (0xF2, 0x5F, 0x5C),
            NodeColor::Orange => (0xF9, 0x73, 0x16),
            NodeColor::Amber => (0xF5, 0x9E, 0x0B),
            NodeColor::Yellow => (0xFA, 0xCC, 0x15),
            NodeColor::Lime => (0x84, 0xCC, 0x16),
            NodeColor::Green => (0x16, 0xA3, 0x4A),
            NodeColor::Teal => (0x0D, 0x94, 0x88),
            NodeColor::Cyan => (0x06, 0xB6, 0xD4),
            NodeColor::Blue => (0x25, 0x63, 0xEB),
            NodeColor::Navy => (0x1E, 0x40, 0xAF),
            NodeColor::Purple => (0x7C, 0x3A, 0xED),
            NodeColor::Lavender => (0xA7, 0x8B, 0xFA),
            NodeColor::Magenta => (0xC0, 0x26, 0xD3),
            NodeColor::Pink => (0xEC, 0x48, 0x99),
            NodeColor::Gray => (0x64, 0x74, 0x8B),
        }
    }

    /// Reads a stored value. Unknown text is `None`, so a hand-edited file cannot break a tree.
    pub fn parse(text: &str) -> Option<NodeColor> {
        let want = text.trim().to_ascii_lowercase();
        NodeColor::ALL.into_iter().find(|c| c.id() == want)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_and_junk_is_ignored() {
        for c in NodeColor::ALL {
            assert_eq!(NodeColor::parse(c.id()), Some(c));
            assert_eq!(NodeColor::parse(&c.label().to_uppercase()), Some(c));
            assert!(!c.label().is_empty());
        }
        assert_eq!(NodeColor::parse("  Blue  "), Some(NodeColor::Blue));
        assert_eq!(NodeColor::parse("chartreuse"), None);
        assert_eq!(NodeColor::parse(""), None);
        assert_eq!(NodeColor::parse("#ff0000"), None);
    }

    #[test]
    fn every_colour_differs_in_both_modes() {
        for dark in [false, true] {
            let all: Vec<(u8, u8, u8)> = NodeColor::ALL.iter().map(|c| c.rgb(dark)).collect();
            for (i, a) in all.iter().enumerate() {
                for b in all.iter().skip(i + 1) {
                    assert_ne!(a, b, "two colours are the same (dark: {dark})");
                }
            }
        }
        assert_ne!(NodeColor::Red.rgb(false), NodeColor::Red.rgb(true), "the dark variant is lighter");
    }
}
