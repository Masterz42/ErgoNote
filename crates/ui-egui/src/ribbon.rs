//! The ribbon: a tab strip and, under it, the groups of controls for the chosen tab, the way
//! Office and OneNote arrange them. It follows the shape the sibling Craft apps use (a tab
//! strip, groups of buttons, a label under each group) without sharing their code.
//!
//! Every control here asks for an [`Act`]; the window carries it out. Nothing in this module
//! touches the notebook, so the whole ribbon is tested by asking what the buttons return.

use crate::floating::{self, Glyph};
use crate::icons::{self, Icon};
use crate::ribbon_icons::{self, Mark};
use crate::theme::Tokens;
use egui::{Align2, Color32, Rect, Sense, Ui, vec2};
use ergonote_ink::rack::{self, Nib};
use ergonote_ink::{Rgb, Tool};
use ergonote_model::{PageFormat, Tag};

/// The tabs, in order. "Source" only appears while the page source is shown, the way Office
/// shows a tab for what is selected.
///
/// Owner decision, 2026-11-27: the Draw tab was folded into Home > Insérer, so there is one
/// main tab alongside contextual tabs (`Table`, `Image`, `Source`).
pub const TABS: [&str; 1] = ["Home"];
pub const SOURCE_TAB: &str = "Source";
pub const FONT_SIZES: [u8; 16] = [8, 9, 10, 11, 12, 14, 16, 18, 20, 22, 24, 26, 28, 36, 48, 72];
/// Shown only while the caret is in a table, the way Office shows a tab for the thing in
/// hand. It goes when the caret leaves, and the ribbon falls back to Home.
pub const TABLE_TAB: &str = "Table";
/// Shown when an image / PDF printout is on the page (selected or locked in background).
pub const IMAGE_TAB: &str = "Image";

/// What a ribbon control asks the window to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    /// The File tab: open the File screen.
    File,
    /// Show the saved versions of the open page.
    PageVersions,
    /// History > Next Unread (issue #291).
    NextUnread,
    /// History > Mark as Read, or as unread when it is read (issue #291).
    ToggleRead,
    /// History > Recent Edits: the pages changed lately, to go to one (issue #279).
    RecentEdits,
    /// Open the active notebook's recycle bin.
    RecycleBin,
    /// Open the built-in symbol picker.
    Symbols,
    /// Insert an embedded vector sticker or school chemistry diagram (`stickers.rs`).
    InsertSticker(&'static str),
    /// Put marks around the selection, or at the caret: bold, italic, strikethrough, code.
    Mark(&'static str),
    /// Apply a point size to the selected text.
    FontSize(u8),
    /// Apply an installed font family to the selected text.
    FontFamily(String),
    /// Make the block under the caret this kind: `p`, `h1`, `h2`, `h3`, `quote`, `bullet`,
    /// `number`, `todo`.
    Block(&'static str),
    /// Put a snippet in the page: a table, a link, an equation, a diagram, the date.
    Insert(&'static str),
    /// Put this TeX in the page as an equation: the one picked from the Equation gallery.
    Equation(&'static str),
    /// Add a file to the page (opens the same question a dropped file asks).
    AddFile,
    /// Pick a document and insert it as a printout.
    AddPrintout,
    /// Add a picture to the page.
    AddPicture,
    /// Save picture as... exports an image file from the page.
    SavePictureAs(String),
    /// Rotate or flip the picture with this source (issue #273).
    TurnPicture(String, crate::turn::Turn),
    /// Open interactive crop mode directly on the selected or active picture on the page.
    CropPicture,
    /// Apply the current inline crop selection onto the image asset.
    ApplyCrop,
    /// Reset the inline crop rectangle back to the full image.
    ResetCrop,
    /// Cancel inline image cropping without modifying the image.
    CancelCrop,
    /// Rotate or flip the selected or active picture on the page from the Image ribbon tab.
    TurnSelectedPicture(crate::turn::Turn),
    /// Enhance contrast / convert a scan into clean greyscale on the selected or active picture.
    EnhancePicture(crate::turn::Enhance),
    /// Run pure-Rust OCR on the active image (`speak`: read aloud right away vs text dialog).
    OcrPicture {
        speak: bool,
    },
    /// Delete the selected or active picture from the page.
    DeleteSelectedPicture,
    ToggleSource,
    ToggleFullPage,
    /// Show or hide the horizontal section tabs bar (Français, Maths...) below the ribbon.
    ToggleSectionTabs,
    /// Toggle ultra-simple tile mode (portrait/kid-friendly interface).
    ToggleSimpleMode,
    /// Lock or unlock PDF / images in the page background so they are not moved by accident.
    ToggleLockImages,
    /// Toggle wider letter and line spacing for dyslexic readers.
    ToggleDysSpacing,
    /// Toggle the visual horizontal reading ruler following the pointer.
    ToggleReadingRuler,
    /// Enable or disable word prediction and spellcheck underline (useful to disable during dictations).
    ToggleWordPrediction,
    /// Scan the selection or open page for spelling, homophone and syntax issues.
    CheckOrthography,
    /// Open the support / "Buy me a coffee" creator dialog.
    SupportCreator,
    /// Open the bug report / user feedback dialog.
    ReportBug,
    /// Read the selected text or page aloud via TTS.
    ReadAloud,
    /// Start or stop voice-to-text dictation at the caret.
    VoiceDictation,
    /// Select all text intelligently (paragraph first, then entire page).
    SelectAll,
    /// Insert a pre-formatted school table ("conversion", "numeration", "proportion", "operation").
    InsertSchoolTable(&'static str),
    OpenThemes,
    /// Lines or squares behind the page, as on paper.
    RuleLines(RuleLines),
    /// Show or hide the page's title line.
    ToggleTitle,
    /// Snap boxes to the page's grid when they are moved or resized, or stop.
    ToggleSnap,
    /// Format Painter: pick up the formatting at the caret for the next words picked. True keeps
    /// painting until Escape (a double-click, as in Office).
    FormatPainter(bool),
    /// Fold the ribbon away to its tab names, or put it back (Ctrl+F1).
    ToggleRibbon,
    /// Sort the table's rows by the caret's column: true for descending (issue #171).
    TableSort(bool),
    /// One tree, or two columns: the organisers and then the pages, as OneNote has it.
    TogglePanes,
    /// Keep this window above the others, or stop.
    ToggleAlwaysOnTop,
    /// Larger (1), smaller (-1) or back to 100% (0).
    Zoom(i8),
    /// Set the page zoom to this percentage.
    ZoomTo(u32),
    /// Fit the page width to the available reading area.
    PageWidth,
    /// Cut or copy what is selected.
    Clipboard(&'static str),
    /// Put the clipboard into the page: a picture as a file beside it, anything else as
    /// words. Its own act rather than `Clipboard("paste")`, because a picture is not a
    /// clipboard operation on the text at all.
    Paste,
    /// More or less indent on a list item.
    Indent(bool),
    /// Which way the line the caret is in reads. `None` lets the words decide again.
    Direction(Option<ergonote_model::Dir>),
    /// Stretch the paragraph the caret is in to fill the column, at this strength, or stop
    /// when it is already at that strength. An Arabic paragraph is stretched by kashida, a
    /// Latin one by its spaces.
    Justify(ergonote_model::justify::Strength),
    /// Set the caret's paragraphs against the left, the middle or the right (issue #258), or
    /// give them back their own alignment when they already have that one.
    Align(ergonote_model::justify::Align),
    /// Which way the whole page reads. `None` lets every line decide for itself.
    PageDirection(Option<ergonote_model::Dir>),
    /// Which way the selection, or what is typed next, reads: an isolated run inside the line.
    Embedding(ergonote_model::Dir),
    /// Take the marks off the selection.
    ClearFormatting,
    /// Put a tag on the line the caret is in, or take it off again.
    Tag(Tag),
    /// Take up the Insert Space tool, or put it down (issue #164).
    InsertSpaceTool,
    /// Open the Tags Summary: the tagged notes, and a way to each of them (issue #166).
    FindTags,
    /// Home > More tags > Customize Tags: the dialog where a person defines their own tags.
    CustomizeTags,
    /// Gather what has drifted apart on the page, boxes and ink, back together (issue #23).
    CollectAll,
    /// Open the Page Templates pane: new pages from templates, and this page kept as one
    /// (issue #173).
    PageTemplates,
    /// Something OneNote has that ErgoNote does not: say so plainly.
    NotBuilt(&'static str),
    /// Change the shape of the table the caret is in.
    Table(crate::pageedit::TableEdit),
    /// Table > Delete Table: the whole table the caret is in.
    DeleteTable,
    /// Prompt for confirmation before deleting a table (from the right-click menu or the
    /// table's corner delete button; `None` deletes the table the caret is in).
    ConfirmDeleteTable(Option<usize>),
    /// Pick a block of cells of the table the caret is in.
    TableSelect(crate::pageedit::TableSpan),
    /// Draw the lines round the cells of a table, or stop drawing them.
    TableBorders(bool),
    /// Align the words in the caret's cell, or in every picked cell (issue #170).
    TableAlign(ergonote_model::tablecols::Align),
    /// A colour behind the caret's cell, or behind every picked cell; `None` takes it away
    /// (issue #169).
    TableShade(Option<ergonote_model::NodeColor>),
    /// The colour behind the open page. `None` is the theme's own paper.
    PageColour(Option<ergonote_model::NodeColor>),
    /// View > Paper Size: the sheet the page is written for, as the page file writes it
    /// ([`ergonote_model::paper::Paper::write`]), or `None` for Auto (issue #269).
    PaperSize(Option<String>),
    /// Colour the open page's rule and grid lines, or `None` for the theme's own (issue #288).
    RuleColour(Option<ergonote_model::NodeColor>),
    /// Take back the last change to the page, or put it back.
    Undo,
    Redo,
    /// Hand the table the caret is in to GridCraft.
    ToGridCraft,
    /// Read the spreadsheet back into the table it came from.
    FromGridCraft,
    /// Start a spreadsheet beside the page and open it in GridCraft.
    NewGridCraft,
    /// A table of this many rows by this many columns, from the size picker.
    InsertTable(usize, usize),
    /// Insert > Table > Insert Table...: the dialog for a size typed in.
    InsertTableDialog,
    /// Home > Alignment and direction > Paragraph Spacing Options: the dialog for the room
    /// before and after the paragraphs the caret or the selection is in, and their line height.
    ParagraphSpacing,
    /// What the pointer does on the page: write, draw or rub out.
    InkPointer(Pointer),
    /// The pen at this place on the rack was picked up.
    InkPen(usize),
    /// The rack itself changed: a tool added, taken off, moved or altered.
    InkRack(RackEdit),
    /// How readily the pen answers pressure, 0 to 100 along the wedge.
    InkSensitivity(u8),
    /// Take the last stroke back.
    InkUndo,
    /// Put back the stroke Undo stroke last took back.
    InkRedo,
    /// Clear every ink stroke on the page.
    InkClear,
    /// Put a drawing on the page at the caret.
    InkInsert,
    /// Save the drawing beside the page as a plain picture, with no pen readings in it.
    InkExportPlain,
    /// Capture or pick a classroom board photo and create a side-by-side text container.
    BoardPhoto,
    /// Gather drifted boxes/ink, reset pan and reset zoom to 100%.
    RecenterStudent,
    /// Toggle DYS visual comfort pack (A4 sheet, soft cream page, blue ruled lines, grid snap).
    ToggleDysMode,
    /// Activate or insert a new floating text box on the page.
    InsertTextBox,
    /// Capture an area of the screen and insert it into the page.
    ScreenClip,
    /// Underline the selected text (or current line) in a chosen color (`None` clears underlining).
    UnderlineColor(Option<ergonote_model::NodeColor>),
    /// Change foreground text colour (`None` restores default theme text colour).
    TextColor(Option<ergonote_model::NodeColor>),
    /// Change text highlight background colour (`None` clears highlighting).
    HighlightColor(Option<ergonote_model::NodeColor>),
    /// The user picked another tab.
    Tab(String),
}

/// What the pointer does on a page. The Draw tab sets it; everywhere else it is `Write`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pointer {
    /// The caret: the ordinary page, where a click puts the caret and a drag selects.
    #[default]
    Write,
    /// Lay down ink with the pen that is chosen.
    Draw,
    /// Rub out whatever the pointer passes over.
    Erase,
    /// Pick ink up: click a stroke to hold it, then move, stretch or delete it.
    Pick,
    /// Pick ink up by drawing a loop round it.
    Lasso,
    /// Draw a regular shape: the drag gives the two corners it is drawn between.
    Shape(ergonote_ink::shapes::Shape),
    /// Draw > Panning Hand: a drag with the main button (or one finger) moves the page, and
    /// nothing on it is drawn, picked or written in.
    Pan,
    /// Draw > Add > Laser Pointer (issue #289): a red dot follows the pointer and a drag leaves
    /// a short trail that fades, to point at something while presenting. Nothing is drawn on
    /// the page, nothing is saved and there is nothing to undo.
    Laser,
    /// The Eyedropper of the rack's tool at this place (issue #290): the colour of the ink
    /// under the pointer is shown beside it, and a click gives that tool that colour.
    Eyedropper(usize),
}

impl Pointer {
    pub fn id(self) -> &'static str {
        match self {
            Pointer::Write => "write",
            Pointer::Draw => "draw",
            Pointer::Erase => "erase",
            Pointer::Pick => "select",
            Pointer::Lasso => "lasso",
            Pointer::Shape(s) => s.id(),
            Pointer::Pan => "pan",
            Pointer::Laser => "laser",
            Pointer::Eyedropper(_) => "eyedropper",
        }
    }
}

/// What the user asked to do to the rack itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RackEdit {
    /// Put a new tool of this kind on the end.
    Add(Tool),
    /// Take the one at this place off.
    Remove(usize),
    /// Move it one place left (true) or right.
    Move(usize, bool),
    /// Change the one at this place to this.
    Set(usize, Nib),
}

/// What is drawn behind the page: nothing, ruled lines, or squares. OneNote calls this
/// Rule Lines, and keeps it per page; here it is how the pages are viewed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RuleLines {
    #[default]
    None,
    /// Ruled paper, in the four spacings OneNote offers.
    Narrow,
    College,
    Standard,
    Wide,
    /// Squared paper, in four sizes.
    GridSmall,
    GridMedium,
    GridLarge,
    GridHuge,
    /// ErgoNote's own two: the same paper with every fifth line drawn stronger, which is
    /// easier to count down and across than a plain grid.
    CraftLines,
    CraftGrid,
}

impl RuleLines {
    /// Every choice, in the order the menu shows them: the ruled ones, then the squares.
    pub const RULED: [RuleLines; 4] = [RuleLines::Narrow, RuleLines::College, RuleLines::Standard, RuleLines::Wide];
    pub const GRIDS: [RuleLines; 4] = [RuleLines::GridSmall, RuleLines::GridMedium, RuleLines::GridLarge, RuleLines::GridHuge];
    /// ErgoNote's own, offered under the other two.
    pub const CRAFT: [RuleLines; 2] = [RuleLines::CraftLines, RuleLines::CraftGrid];

    pub fn id(self) -> &'static str {
        match self {
            RuleLines::None => "none",
            RuleLines::Narrow => "narrow",
            RuleLines::College => "college",
            RuleLines::Standard => "standard",
            RuleLines::Wide => "wide",
            RuleLines::GridSmall => "grid-small",
            RuleLines::GridMedium => "grid-medium",
            RuleLines::GridLarge => "grid-large",
            RuleLines::GridHuge => "grid-huge",
            RuleLines::CraftLines => "lines",
            RuleLines::CraftGrid => "grid",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RuleLines::None => "None",
            RuleLines::Narrow => "Narrow ruled",
            RuleLines::College => "College ruled",
            RuleLines::Standard => "Standard ruled",
            RuleLines::Wide => "Wide ruled",
            RuleLines::GridSmall => "Small squares",
            RuleLines::GridMedium => "Medium squares",
            RuleLines::GridLarge => "Large squares",
            RuleLines::GridHuge => "Very large squares",
            RuleLines::CraftLines => "ErgoNote ruled",
            RuleLines::CraftGrid => "ErgoNote squares",
        }
    }

    pub fn translated_label(self) -> &'static str {
        crate::tl!(self.label())
    }

    /// The name under a thumbnail, where the shelf above it already says what kind of paper
    /// this is. The full `label` stays the button's name, so it is still what a screen
    /// reader says and what the ribbon's button shows.
    pub fn short(self) -> &'static str {
        match self {
            RuleLines::None => "None",
            RuleLines::Narrow => "Narrow",
            RuleLines::College => "College",
            RuleLines::Standard => "Standard",
            RuleLines::Wide => "Wide",
            RuleLines::GridSmall => "Small",
            RuleLines::GridMedium => "Medium",
            RuleLines::GridLarge => "Large",
            RuleLines::GridHuge => "Very large",
            RuleLines::CraftLines => "Ruled",
            RuleLines::CraftGrid => "Squares",
        }
    }

    pub fn translated_short(self) -> &'static str {
        crate::tl!(self.short())
    }

    pub fn translated_mid(self) -> &'static str {
        match self {
            RuleLines::None => crate::tl_id!("ui.none"),
            RuleLines::Narrow => crate::tl_id!("ui.narrow_ruled"),
            RuleLines::College => crate::tl_id!("ui.college_ruled"),
            RuleLines::Standard => crate::tl_id!("ui.standard_ruled"),
            RuleLines::Wide => crate::tl_id!("ui.wide_ruled"),
            RuleLines::GridSmall => crate::tl_id!("ui.small_squares"),
            RuleLines::GridMedium => crate::tl_id!("ui.medium_squares"),
            RuleLines::GridLarge => crate::tl_id!("ui.large_squares"),
            RuleLines::GridHuge => crate::tl_id!("ui.very_large_squares"),
            RuleLines::CraftLines => crate::tl_id!("ui.ergonote_ruled"),
            RuleLines::CraftGrid => crate::tl_id!("ui.ergonote_squares"),
        }
    }

    /// How far apart the lines are, in points. The ruled spacings follow the paper the
    /// names come from; the squares are the same four steps without the wide one.
    pub fn step(self) -> f32 {
        match self {
            RuleLines::None => 0.0,
            RuleLines::Narrow => 16.0,
            RuleLines::College => 20.0,
            RuleLines::Standard => 24.0,
            RuleLines::Wide => 32.0,
            RuleLines::GridSmall => 12.0,
            RuleLines::GridMedium => 18.0,
            RuleLines::GridLarge => 24.0,
            RuleLines::GridHuge => 32.0,
            RuleLines::CraftLines | RuleLines::CraftGrid => 24.0,
        }
    }

    /// True for the squared papers, which draw the upright lines as well.
    pub fn squares(self) -> bool {
        matches!(self, RuleLines::GridSmall | RuleLines::GridMedium | RuleLines::GridLarge | RuleLines::GridHuge | RuleLines::CraftGrid)
    }

    /// True where every fifth line is drawn stronger: ErgoNote's own two papers.
    pub fn fives(self) -> bool {
        matches!(self, RuleLines::CraftLines | RuleLines::CraftGrid)
    }

    pub fn on(self) -> bool {
        self != RuleLines::None
    }

    pub fn parse(text: &str) -> Option<RuleLines> {
        let want = text.trim();
        if want == "none" {
            return Some(RuleLines::None);
        }
        // "lines" and "grid" are ErgoNote's own two, and the ids the first version wrote.
        RuleLines::RULED.into_iter().chain(RuleLines::GRIDS).chain(RuleLines::CRAFT).find(|r| r.id() == want)
    }
}

/// Draws a paper's lines inside `area`. The page draws itself at full size; the gallery in
/// the ribbon draws the same papers at `scale`, so a thumbnail is the paper itself made
/// small and cannot drift from what the page will show.
pub fn paint_paper(painter: &egui::Painter, area: egui::Rect, rules: RuleLines, scale: f32, line: egui::Color32, strong: egui::Color32) {
    if !rules.on() {
        return;
    }
    let step = (rules.step() * scale).max(4.0);
    let fives = rules.fives();
    let colour = |n: usize| if fives && n.is_multiple_of(5) { strong } else { line };
    let mut y = area.min.y + step;
    let mut n = 0;
    while y < area.max.y {
        n += 1;
        painter.line_segment([egui::pos2(area.min.x, y), egui::pos2(area.max.x, y)], egui::Stroke::new(1.0, colour(n)));
        y += step;
    }
    if rules.squares() {
        let mut x = area.min.x + step;
        let mut n = 0;
        while x < area.max.x {
            n += 1;
            painter.line_segment([egui::pos2(x, area.min.y), egui::pos2(x, area.max.y)], egui::Stroke::new(1.0, colour(n)));
            x += step;
        }
    }
}

/// Paints rule lines in a viewport, one of them through `anchor` and the rest a step apart, so
/// they stay put on the page as it pans and a box snapped to the page's grid sits on a line.
/// The window passes the page's origin, which is where box positions are counted from.
pub fn paint_paper_canvas(
    painter: &egui::Painter,
    area: egui::Rect,
    rules: RuleLines,
    scale: f32,
    anchor: egui::Pos2,
    line: egui::Color32,
    strong: egui::Color32,
) {
    if !rules.on() {
        return;
    }
    let step = (rules.step() * scale).max(4.0);
    let index = |screen: f32, anchor: f32| {
        let line = ((screen - anchor) / step).round();
        #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
        {
            line.clamp(i64::MIN as f32, i64::MAX as f32) as i64
        }
    };
    let colour = |n: i64| if rules.fives() && n.rem_euclid(5) == 0 { strong } else { line };

    let mut y = first_grid_line(area.min.y, anchor.y, step);
    while y < area.max.y {
        painter.line_segment([egui::pos2(area.min.x, y), egui::pos2(area.max.x, y)], egui::Stroke::new(1.0, colour(index(y, anchor.y))));
        y += step;
    }
    if rules.squares() {
        let mut x = first_grid_line(area.min.x, anchor.x, step);
        while x < area.max.x {
            painter.line_segment([egui::pos2(x, area.min.y), egui::pos2(x, area.max.y)], egui::Stroke::new(1.0, colour(index(x, anchor.x))));
            x += step;
        }
    }
}

/// The first line at or after `min` on a grid `step` apart that has a line through `anchor`.
fn first_grid_line(min: f32, anchor: f32, step: f32) -> f32 {
    anchor + ((min - anchor) / step).ceil() * step
}

/// What the View tab shows about the window.
#[derive(Clone, Copy, Debug, Default)]
pub struct View<'a> {
    /// True when the side panel is two columns.
    pub split_panes: bool,
    /// True when the horizontal section tabs bar is shown under the ribbon.
    pub section_tabs_bar: bool,
    /// True when ultra-simple tile mode (portrait/kid-friendly interface) is active.
    pub simple_mode: bool,
    /// True when PDF / image blocks are locked in background against accidental drag/resize.
    pub lock_images: bool,
    /// True when DYS wider letter and line spacing is enabled.
    pub dys_spaced: bool,
    /// True when the horizontal visual reading ruler is enabled.
    pub reading_ruler: bool,
    /// True when word prediction and inline spellcheck are enabled.
    pub word_prediction: bool,
    /// True when voice-to-text dictation is currently recording/listening.
    pub dictating: bool,
    /// True when the window is kept above the others.
    pub always_on_top: bool,
    pub full_page: bool,
    pub rules: RuleLines,
    pub hide_title: bool,
    /// Whether boxes snap to the page's grid when they are moved or resized.
    pub snap: bool,
    pub zoom_percent: u32,
    /// What the pointer does on the page.
    pub pointer: Pointer,
    /// The tools on the rack, and which of them is in hand.
    pub rack: &'a [Nib],
    pub nib: usize,
    /// True while the Insert Space tool is in hand.
    pub space_tool: bool,
    /// True while the Insert Text Box placement tool is in hand.
    pub textbox_tool: bool,
    /// True while the caret is in a table, which is when the Table tab is offered.
    pub in_table: bool,
    /// True while an image / PDF printout is on the page (selected or locked in background).
    pub has_image: bool,
    /// True while direct on-image touch/mouse cropping handles are active.
    pub image_cropping: bool,
    /// True while an offline OCR worker is analyzing an image.
    pub ocr_busy: bool,
    /// Whether the table the caret is in is drawn with lines round its cells.
    pub table_borders: bool,
    /// How the words in the caret's cell, or in every picked cell, are aligned; `None` when
    /// the picked cells differ.
    pub table_align: Option<ergonote_model::tablecols::Align>,
    /// The colour behind the caret's cell, or behind every picked cell: `Some(None)` for none,
    /// and `None` when the picked cells differ.
    pub table_shade: Option<Option<ergonote_model::NodeColor>>,
    /// True when GridCraft is installed, which is when a table can be handed to it.
    pub gridcraft: bool,
    /// The colour behind the open page, when it carries one of its own.
    pub page_colour: Option<ergonote_model::NodeColor>,
    /// The colour of the open page's rule and grid lines, `None` for the theme's (issue #288).
    pub rule_colour: Option<ergonote_model::NodeColor>,
    /// The open page's sheet; `None` is Auto (issue #269).
    pub paper_size: Option<ergonote_model::paper::Paper>,
    /// Whether the page has anything to take back, and anything to put back.
    pub can_undo: bool,
    pub can_redo: bool,
    /// Whether the drawing has a stroke that Undo stroke took back, for Draw > Redo stroke.
    pub can_ink_redo: bool,
    /// How readily the pen answers pressure, 0 to 100.
    pub sensitivity: u8,
    /// Whether the current input path has reported a pressure-capable pen.
    pub pressure_available: bool,
    /// A page is open for page-specific History commands.
    pub has_page: bool,
    /// The open page was marked unread by hand (issue #291).
    pub page_marked_unread: bool,
    /// The Format Painter is holding formatting to put on the next words picked.
    pub painting: bool,
    /// The style of the block the caret is in (a [`STYLES`] kind), or "" when there is no caret.
    pub style: &'static str,
    pub font_family: Option<&'a str>,
    /// The marks of the words at the caret or of the whole selection, which Home's mark
    /// buttons show as pressed (issue #255).
    pub marks: crate::pageedit::Marks,
    pub align: Option<ergonote_model::justify::Align>,
    pub font_size: Option<&'a str>,
}

impl View<'_> {
    /// The tool in hand, or the first on the rack if the saved number is out of range.
    pub fn in_hand(&self) -> Nib {
        self.rack.get(self.nib).or_else(|| self.rack.first()).copied().unwrap_or_else(|| rack::new_nib(Tool::Pen))
    }
}

/// How tall the ribbon is: the tab strip, then the groups.
pub const TAB_STRIP_H: f32 = 32.0;
/// The ribbon's own height under the tabs: the buttons, then the group names along the
/// bottom. Enlarged for student & DYS readability.
pub const GROUP_H: f32 = 112.0;
/// How far down a big button its icon, its first line of writing, and its second sit.
const ICON_Y: f32 = 24.0;
const LABEL_Y: f32 = 50.0;
const LINE_H: f32 = 15.5;
/// The narrowest a big button may be, and the room its writing gets either side.
const BIG: f32 = 56.0;
const BIG_PAD: f32 = 14.0;
const SMALL_W: f32 = 32.0;
const SMALL_H: f32 = 25.0;
/// Where the group's name sits above the ribbon's bottom edge.
const GROUP_NAME_Y: f32 = 11.0;

/// Draws the ribbon. `tab` is the chosen tab, `source` whether the page source is shown.
/// Returns every act the user asked for this frame (usually none or one).
pub fn show(ui: &mut Ui, tab: &str, source: bool, view: &View<'_>, format: Option<PageFormat>, t: &Tokens) -> Vec<Act> {
    let mut acts = Vec::new();
    ui.add_space(3.0);
    let chosen = resolve_tab(tab, source, view.in_table, view.has_image);
    // A tab with many groups is wider than a narrow window: the Draw tab alone has eleven,
    // as OneNote's does. Rather than clipping the last of them, the groups scroll sideways.
    // A mouse drag must not scroll: the controls here are clicked and dragged themselves, and a
    // drag that scrolled the ribbon instead would take the pen out of the user's hand. On a
    // touch screen a finger drag is the only way to reach the groups past the edge, so there
    // it scrolls (egui's OnTouch); a control under the finger still takes its own drag.
    // Where an overflow arrow or the wheel asked the ribbon to go, last frame (issue #277).
    let jump_id = egui::Id::new(("ribbon-jump", chosen));
    let jump = ui.data_mut(|d| d.remove_temp::<f32>(jump_id));
    let mut area = egui::ScrollArea::horizontal();
    if let Some(x) = jump {
        area = area.horizontal_scroll_offset(x);
    }
    let out = area
        .id_salt(("ribbon", chosen))
        // The ribbon is an Office-style command band, not a document scroller.  A
        // visible horizontal scrollbar is painted across its bottom edge when a tab
        // is wider than the window, which looks like a stray black artifact.  Keep
        // the scroll area (overflow tabs still scroll) but do not paint that bar.
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .scroll_source(egui::scroll_area::ScrollSource { drag: egui::scroll_area::DragScroll::OnTouch, ..egui::scroll_area::ScrollSource::default() })
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                // Ribbon groups scroll sideways instead of wrapping standard egui buttons into narrow
                // vertical strips when they approach the right edge of the viewport.
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                ui.add_space(4.0);
                match chosen {
                    TABLE_TAB => table_tab(ui, view, t, &mut acts),
                    IMAGE_TAB => image_tab(ui, view, t, &mut acts),
                    SOURCE_TAB => source_tab(ui, format, t, &mut acts),
                    _ => home_tab(ui, view, t, &mut acts),
                }
            });
        });
    if let Some(x) = overflow_arrows(ui, &out, t) {
        ui.data_mut(|d| d.insert_temp(jump_id, x));
        ui.ctx().request_repaint();
    }
    acts
}

/// How much of the ribbon an arrow moves it by: most of what is in view, so a group cut at
/// the edge comes whole into sight and nothing is skipped.
const OVERFLOW_STEP: f32 = 0.7;

/// The arrows at the ends of a ribbon too wide for the window (issue #277). The ribbon
/// scrolls sideways but shows no scroll bar, so nothing said that groups lay past the edge:
/// an arrow appears at each end that has more beyond it, over a fade into the ribbon, and a
/// click (or Enter on it from the keyboard) brings the next stretch into view. The mouse wheel
/// over the ribbon scrolls it sideways as well.
/// Returns where the ribbon should scroll to, when that changed.
fn overflow_arrows<R>(ui: &mut Ui, out: &egui::scroll_area::ScrollAreaOutput<R>, t: &Tokens) -> Option<f32> {
    let view = out.inner_rect;
    let spare = (out.content_size.x - view.width()).max(0.0);
    if spare < 1.0 {
        return None;
    }
    let was = out.state.offset().x;
    let mut offset = was;
    // A vertical wheel over a ribbon that only scrolls sideways moves it sideways.
    let wheel = ui.input(|i| if i.pointer.hover_pos().is_some_and(|p| view.contains(p)) { i.smooth_scroll_delta.y } else { 0.0 });
    if wheel != 0.0 {
        offset -= wheel;
    }
    let side = 22.0;
    let arrow = |ui: &mut Ui, left: bool| -> bool {
        let rect = if left {
            egui::Rect::from_min_size(view.min, vec2(side, view.height()))
        } else {
            egui::Rect::from_min_size(egui::pos2(view.max.x - side, view.min.y), vec2(side, view.height()))
        };
        let id = ui.id().with(("ribbon-overflow", left));
        let resp = ui.interact(rect, id, Sense::click());
        let painter = ui.painter();
        // A fade from the ribbon's colour, so the words under the arrow do not show through.
        let fade = rect.expand2(vec2(10.0, 0.0)).translate(vec2(if left { 10.0 } else { -10.0 }, 0.0));
        let mut mesh = egui::Mesh::default();
        let (solid, clear) = (t.panel, t.panel.gamma_multiply(0.0));
        let (l, r) = if left { (solid, clear) } else { (clear, solid) };
        mesh.colored_vertex(fade.left_top(), l);
        mesh.colored_vertex(fade.right_top(), r);
        mesh.colored_vertex(fade.right_bottom(), r);
        mesh.colored_vertex(fade.left_bottom(), l);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        painter.add(egui::Shape::mesh(mesh));
        if resp.hovered() || resp.has_focus() {
            painter.rect_filled(rect.shrink2(vec2(2.0, 8.0)), 4.0, t.row_hover);
        }
        let c = rect.center();
        let d = if left { -1.0 } else { 1.0 };
        let stroke = egui::Stroke::new(2.0, t.accent);
        painter.line_segment([egui::pos2(c.x - 3.0 * d, c.y - 7.0), egui::pos2(c.x + 3.0 * d, c.y)], stroke);
        painter.line_segment([egui::pos2(c.x + 3.0 * d, c.y), egui::pos2(c.x - 3.0 * d, c.y + 7.0)], stroke);
        let name =
            if left { crate::tl_id!("ui.show_more_of_the_ribbon_to_the_left") } else { crate::tl_id!("ui.show_more_of_the_ribbon_to_the_right") };
        let enabled = ui.is_enabled();
        resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name));
        resp.on_hover_text(name).clicked()
    };
    if offset > 0.5 && arrow(ui, true) {
        offset -= view.width() * OVERFLOW_STEP;
    }
    if offset < spare - 0.5 && arrow(ui, false) {
        offset += view.width() * OVERFLOW_STEP;
    }
    let offset = offset.clamp(0.0, spare);
    ((offset - was).abs() > 0.1).then_some(offset)
}

/// The tab that is really shown: contextual tabs disappear when their context ends.
pub fn resolve_tab(tab: &str, source: bool, in_table: bool, has_image: bool) -> &str {
    if tab == SOURCE_TAB && !source {
        return "Home";
    }
    if tab == TABLE_TAB && !in_table {
        return "Home";
    }
    if tab == IMAGE_TAB && !has_image {
        return "Home";
    }
    if TABS.contains(&tab) || (tab == SOURCE_TAB && source) || (tab == TABLE_TAB && in_table) || (tab == IMAGE_TAB && has_image) {
        tab
    } else {
        "Home"
    }
}

/// Every tab that can be chosen now.
pub fn tabs(source: bool, in_table: bool, has_image: bool) -> Vec<&'static str> {
    let mut v = TABS.to_vec();
    if in_table {
        v.push(TABLE_TAB);
    }
    if has_image {
        v.push(IMAGE_TAB);
    }
    if source {
        v.push(SOURCE_TAB);
    }
    v
}

/// The tab names, drawn on the coloured band at the top of the window, File first, as in
/// Office. The chosen tab is a raised tab in the ribbon's own colour that runs down into the
/// ribbon below it; the others are white writing on the band, lit when the pointer is over
/// one.
pub fn tab_names(ui: &mut Ui, tab: &str, source: bool, in_table: bool, has_image: bool, collapsed: bool, t: &Tokens, acts: &mut Vec<Act>) {
    // Folded, no tab is the raised one: there is no ribbon under it to run into.
    let chosen = if collapsed { "" } else { resolve_tab(tab, source, in_table, has_image) };
    {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(4.0);
        let names = std::iter::once(FILE_TAB).chain(tabs(source, in_table, has_image));
        for name in names {
            let selected = name == chosen;
            let display_name = crate::tl!(name);
            let font = egui::FontId::proportional(14.5);
            let galley = ui.painter().layout_no_wrap(display_name.to_string(), font.clone(), t.text);
            let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 26.0, TAB_STRIP_H), Sense::click());
            let painter = ui.painter_at(rect);
            if selected {
                // Raised: the tab and the ribbon are one surface, so there is no line between.
                let top = egui::CornerRadius { nw: 4, ne: 4, sw: 0, se: 0 };
                painter.rect_filled(egui::Rect::from_min_max(egui::pos2(rect.min.x, rect.min.y + 3.0), rect.max), top, t.panel);
            } else if resp.hovered() {
                painter.rect_filled(rect.shrink2(vec2(1.0, 3.0)), 3.0, t.band_hover);
            }
            let colour = if selected { t.accent } else { t.band_text };
            painter.text(rect.center() + vec2(0.0, 1.0), Align2::CENTER_CENTER, display_name, font, colour);
            let enabled = ui.is_enabled();
            let label = display_name.to_string();
            resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, selected, label.clone()));
            if resp.clicked() {
                acts.push(if name == FILE_TAB { Act::File } else { Act::Tab(name.to_string()) });
            }
        }
    }
}

/// The ribbon fold control for the end of the single header row.
pub fn fold_button(ui: &mut Ui, collapsed: bool, t: &Tokens, acts: &mut Vec<Act>) {
    if ribbon_fold(ui, collapsed, t).clicked() {
        acts.push(Act::ToggleRibbon);
    }
}

/// The fold control at the end of the tab names: a chevron up to fold the ribbon away, down
/// to put it back.
fn ribbon_fold(ui: &mut Ui, collapsed: bool, t: &Tokens) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(26.0, TAB_STRIP_H - 6.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 3.0, t.band_hover);
    }
    let c = rect.center();
    let stroke = egui::Stroke::new(1.6, t.band_text);
    let points = if collapsed {
        [egui::pos2(c.x - 4.5, c.y - 2.0), egui::pos2(c.x, c.y + 2.5), egui::pos2(c.x + 4.5, c.y - 2.0)]
    } else {
        [egui::pos2(c.x - 4.5, c.y + 2.0), egui::pos2(c.x, c.y - 2.5), egui::pos2(c.x + 4.5, c.y + 2.0)]
    };
    ui.painter().add(egui::Shape::line(points.to_vec(), stroke));
    let label = if collapsed { crate::tl_id!("ui.show_the_ribbon_ctrl_f1") } else { crate::tl_id!("ui.collapse_the_ribbon_ctrl_f1") }.to_string();
    let enabled = ui.is_enabled();
    let name = label.clone();
    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &name));
    resp.on_hover_text(label)
}

/// The name of the first tab, which opens the File screen instead of a ribbon.
pub const FILE_TAB: &str = "File";

/// One group of the ribbon: its controls inside a soft rounded card, and its name underneath.
// i18n-ui-text: title
fn group(ui: &mut Ui, title: &str, t: &Tokens, add: impl FnOnce(&mut Ui)) {
    let display_title = crate::tl!(title);
    let card_shape = ui.painter().add(egui::Shape::Noop);
    let inner = ui
        .scope(|ui| {
            ui.spacing_mut().item_spacing = vec2(5.0, 3.5);
            ui.horizontal_top(|ui| {
                ui.add_space(8.0);
                add(ui);
                ui.add_space(8.0);
            });
        })
        .response
        .rect;
    let card_rect = egui::Rect::from_min_max(egui::pos2(inner.min.x, inner.min.y + 1.0), egui::pos2(inner.max.x, inner.min.y + GROUP_H - 3.0));
    let card_fill = crate::theme::mix(t.panel, t.editor, 0.72);
    let card_border = crate::theme::mix(t.panel, t.text, 0.13);
    ui.painter().set(
        card_shape,
        egui::Shape::Rect(egui::epaint::RectShape::new(
            card_rect,
            egui::CornerRadius::same(10),
            card_fill,
            egui::Stroke::new(1.0, card_border),
            egui::StrokeKind::Inside,
        )),
    );
    let painter = ui.painter();
    painter.text(
        egui::pos2(inner.center().x, inner.min.y + GROUP_H - GROUP_NAME_Y),
        Align2::CENTER_CENTER,
        display_title,
        egui::FontId::proportional(12.0),
        t.muted,
    );
    let named = ui.interact(inner, ui.id().with(("group", title)), Sense::hover());
    let (enabled, label) = (ui.is_enabled(), display_title.to_owned());
    named.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Label, enabled, &label));
    ui.add_space(6.0);
}

/// A big button: an icon with its name under it.
/// A big button: the mark it makes above what it is called, as Office draws one. A label
/// with a newline in it is written on two lines, which is how "Panning Hand" and "Full
/// Page View" fit without making the button wide.
// i18n-ui-text: label,tip
fn big(ui: &mut Ui, icon: Icon, label: &str, tip: &str, t: &Tokens) -> egui::Response {
    big_inner(ui, icon, label, tip, false, false, t)
}

/// The same, with the small chevron Office puts on a button that opens something.
// i18n-ui-text: label,tip
fn big_menu(ui: &mut Ui, icon: Icon, label: &str, tip: &str, t: &Tokens) -> egui::Response {
    big_inner(ui, icon, label, tip, true, false, t)
}

fn pictures_menu(ui: &mut Ui, t: &Tokens, acts: &mut Vec<Act>) {
    let response = big_menu(ui, Icon::InsertFile, crate::tl_id!("ui.student_insert_menu"), crate::tl_id!("ui.student_insert_menu_tip"), t);
    egui::Popup::menu(&response).show(|ui| {
        egui::Frame::popup(ui.style()).corner_radius(8.0).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
            ui.set_min_width(228.0);
            if picture_menu_row(ui, Icon::Picture, crate::tl_id!("ui.student_photo_file"), t, true).clicked() {
                acts.push(Act::AddPicture);
                ui.close();
            }
            if picture_menu_row(ui, Icon::Page, crate::tl_id!("ui.student_pdf_doc"), t, true).clicked() {
                acts.push(Act::AddPrintout);
                ui.close();
            }
            if picture_menu_row(ui, Icon::Attach, crate::tl_id!("ui.student_other_file"), t, true).clicked() {
                acts.push(Act::AddFile);
                ui.close();
            }
        });
    });
}

fn more_insert_menu(ui: &mut Ui, t: &Tokens, acts: &mut Vec<Act>) {
    let response = big_menu(ui, Icon::PageStack, crate::tl_id!("ui.student_more"), crate::tl_id!("ui.student_more_tip"), t);
    egui::Popup::menu(&response).show(|ui| {
        egui::Frame::popup(ui.style()).corner_radius(8.0).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
            ui.set_min_width(244.0);
            if picture_menu_row(ui, Icon::PageStack, crate::tl_id!("ui.student_templates_row"), t, true).clicked() {
                acts.push(Act::PageTemplates);
                ui.close();
            }
            if picture_menu_row(ui, Icon::Symbol, crate::tl_id!("ui.student_symbol_row"), t, true).clicked() {
                acts.push(Act::Symbols);
                ui.close();
            }
            if picture_menu_row(ui, Icon::Equation, crate::tl_id!("ui.student_equation_row"), t, true).clicked() {
                acts.push(Act::Insert("equation"));
                ui.close();
            }
            if picture_menu_row(ui, Icon::Link, crate::tl_id!("ui.link"), t, true).clicked() {
                acts.push(Act::Insert("link"));
                ui.close();
            }
            if picture_menu_row(ui, Icon::Todo, crate::tl_id!("ui.student_todo_row"), t, true).clicked() {
                acts.push(Act::Block("todo"));
                ui.close();
            }
            if menu_row_glyph(ui, Glyph::Mark(Mark::Important), crate::tl_id!("ui.student_important_row"), t, true).clicked() {
                acts.push(Act::Tag(Tag::Important));
                ui.close();
            }
            if menu_row_glyph(ui, Glyph::Mark(Mark::Question), crate::tl_id!("ui.student_question_row"), t, true).clicked() {
                acts.push(Act::Tag(Tag::Question));
                ui.close();
            }
        });
    });
}

// i18n-ui-text: label
fn picture_menu_row(ui: &mut Ui, icon: Icon, label: &str, t: &Tokens, enabled: bool) -> egui::Response {
    menu_row_glyph(ui, Glyph::Icon(icon), label, t, enabled)
}

// i18n-ui-text: label
fn menu_row_glyph(ui: &mut Ui, glyph: Glyph, label: &str, t: &Tokens, enabled: bool) -> egui::Response {
    let label = crate::tl!(label);
    let (rect, response) = ui.allocate_exact_size(vec2(228.0, 36.0), Sense::click());
    if response.hovered() && enabled {
        ui.painter().rect_filled(rect, 6.0, crate::theme::mix(t.panel, t.accent, 0.12));
    }
    let color = if !enabled {
        t.muted
    } else {
        match glyph {
            Glyph::Icon(Icon::Picture) => t.ok,
            Glyph::Mark(mark) => mark.color(t.text, t.accent),
            Glyph::Icon(_) => t.accent,
        }
    };
    let painter = ui.painter_at(rect);
    let icon_rect = Rect::from_center_size(egui::pos2(rect.left() + 20.0, rect.center().y), vec2(20.0, 20.0));
    match glyph {
        Glyph::Icon(icon) => icons::paint_ribbon(ui, icon_rect, icon, color),
        Glyph::Mark(mark) => ribbon_icons::paint(&painter, icon_rect, mark, color),
    }
    painter.text(
        egui::pos2(rect.left() + 43.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(13.5),
        if enabled { t.text } else { t.muted },
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    response
}

/// The small chevron Office puts under anything that opens a menu, and under the tool in
/// use on the rack.
fn chevron(painter: &egui::Painter, at: egui::Pos2, colour: egui::Color32) {
    painter.add(egui::Shape::line(
        vec![egui::pos2(at.x - 3.5, at.y - 1.5), egui::pos2(at.x, at.y + 2.0), egui::pos2(at.x + 3.5, at.y - 1.5)],
        egui::Stroke::new(1.2, colour),
    ));
}

// i18n-ui-text: label,tip
fn big_inner(ui: &mut Ui, icon: Icon, label: &str, tip: &str, menu: bool, on: bool, t: &Tokens) -> egui::Response {
    let label = crate::tl!(label);
    let tip = crate::tl!(tip);
    let lines: Vec<&str> = label.split('\n').collect();
    let font = egui::FontId::proportional(13.0);
    let widest = lines.iter().map(|l| ui.painter().layout_no_wrap((*l).to_string(), font.clone(), t.text).size().x).fold(0.0_f32, f32::max);
    let (rect, resp) = ui.allocate_exact_size(vec2(BIG.max(widest + BIG_PAD), GROUP_H - 20.0), Sense::click());
    let painter = ui.painter_at(rect);
    let colour = if !ui.is_enabled() {
        t.muted
    } else {
        match icon {
            Icon::Picture | Icon::Clip => t.ok,
            Icon::Attach => t.warn,
            Icon::Link => t.link,
            Icon::InsertSpace => t.accent,
            Icon::Calendar | Icon::Clock => t.strong,
            _ => t.accent,
        }
    };
    if on {
        painter.rect_filled(rect, 8.0, crate::theme::mix(t.editor, colour, 0.16));
        painter.rect_stroke(rect, 8.0, egui::Stroke::new(1.8, colour), egui::StrokeKind::Inside);
    } else if resp.hovered() {
        painter.rect_filled(rect, 8.0, t.row_hover);
    }
    let badge = Rect::from_center_size(egui::pos2(rect.center().x, rect.min.y + ICON_Y), vec2(36.0, 34.0));
    painter.rect_filled(badge, 9.0, crate::theme::mix(t.editor, colour, if on { 0.24 } else { 0.13 }));
    icons::paint_ribbon(ui, Rect::from_center_size(egui::pos2(rect.center().x, rect.min.y + ICON_Y), vec2(26.0, 26.0)), icon, colour);
    for (i, line) in lines.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let y = rect.min.y + LABEL_Y + i as f32 * LINE_H;
        painter.text(egui::pos2(rect.center().x, y), Align2::CENTER_CENTER, line, font.clone(), t.text);
    }
    if menu {
        // A chevron under the writing: this one opens something rather than doing it.
        #[allow(clippy::cast_precision_loss)]
        let y = rect.min.y + LABEL_Y + lines.len() as f32 * LINE_H - 1.0;
        chevron(&painter, egui::pos2(rect.center().x, y), t.muted);
    }
    let enabled = ui.is_enabled();
    // The name a test or a screen reader hears: one line, no matter how the label wraps.
    let name = label.replace('\n', " ");
    resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, on, name.clone()));
    resp.on_hover_text(tip)
}

/// A small button: an icon, or a letter for the marks that have one.
/// `big`, with the button shown as the one in use.
// i18n-ui-text: label,tip
fn big_on(ui: &mut Ui, icon: Icon, label: &str, tip: &str, on: bool, t: &Tokens) -> egui::Response {
    big_inner(ui, icon, label, tip, false, on, t)
}

// i18n-ui-text: letter,label,tip
fn small(ui: &mut Ui, icon: Option<Icon>, letter: &str, label: &str, tip: &str, on: bool, t: &Tokens) -> egui::Response {
    let label = crate::tl!(label);
    let tip = crate::tl!(tip);
    let letter = crate::tl!(letter);
    // Wide enough for whatever it says: a letter, or a word like "Copy".
    let width = if icon.is_some() || letter.is_empty() {
        SMALL_W
    } else {
        ui.painter().layout_no_wrap(letter.to_string(), egui::FontId::proportional(14.0), t.text).size().x + 12.0
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(width.max(SMALL_W), SMALL_H), Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect_filled(
        rect,
        6.0,
        if on {
            t.row_selected
        } else if resp.hovered() {
            t.row_hover
        } else {
            t.editor
        },
    );
    painter.rect_stroke(rect, 6.0, egui::Stroke::new(1.0, if on { t.accent } else { t.faint }), egui::StrokeKind::Inside);
    match icon {
        Some(i) => icons::paint_ribbon(ui, Rect::from_center_size(rect.center(), vec2(18.0, 18.0)), i, t.accent),
        None => {
            painter.text(rect.center(), Align2::CENTER_CENTER, letter, egui::FontId::proportional(14.0), t.strong);
        }
    }
    let enabled = ui.is_enabled();
    let name = label.to_string();
    resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, on, name.clone()));
    resp.on_hover_text(tip)
}

fn ribbon_row_sep(ui: &mut Ui, t: &Tokens) {
    let (rect, _) = ui.allocate_exact_size(vec2(2.0, SMALL_H), Sense::hover());
    let x = rect.center().x;
    ui.painter().line_segment([egui::pos2(x, rect.min.y + 4.0), egui::pos2(x, rect.max.y - 4.0)], egui::Stroke::new(1.0, t.faint));
}

fn write_mode_pill(ui: &mut Ui, width: f32, pointer: Pointer, t: &Tokens) -> egui::Response {
    let is_write = pointer == Pointer::Write;
    let (rect, resp) = ui.allocate_exact_size(vec2(width, SMALL_H), Sense::click());
    let enabled = ui.is_enabled();
    let painter = ui.painter_at(rect);

    let (fill, edge, stroke_w, pulse) = if is_write {
        (if resp.hovered() { t.row_hover } else { t.row_selected }, t.accent, 1.3, 0.0_f32)
    } else {
        let time = ui.input(|i| i.time);
        let p = ((time * std::f64::consts::TAU * 0.75).sin() * 0.5 + 0.5) as f32;
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(60));
        let amber = Color32::from_rgb(0xF5, 0x9E, 0x0B);
        let orange = Color32::from_rgb(0xEA, 0x58, 0x0C);
        let base_fill = crate::theme::mix(t.editor, amber, 0.10 + 0.24 * p);
        (if resp.hovered() { crate::theme::mix(t.row_hover, amber, 0.28) } else { base_fill }, orange, 1.2 + 1.0 * p, p)
    };
    painter.rect_filled(rect, 6.0, fill);
    painter.rect_stroke(rect, 6.0, egui::Stroke::new(stroke_w, edge), egui::StrokeKind::Inside);

    // Intuitive mini-keyboard icon on the left.
    let kb_rect = Rect::from_center_size(egui::pos2(rect.min.x + 16.0, rect.center().y), vec2(18.0, 12.0));
    let kb_frame = if is_write { t.accent } else { Color32::from_rgb(0xC2, 0x41, 0x0C) };
    painter.rect_filled(kb_rect, 2.5, crate::theme::mix(t.editor, kb_frame, 0.16));
    painter.rect_stroke(kb_rect, 2.5, egui::Stroke::new(1.2, kb_frame), egui::StrokeKind::Inside);
    for col_i in 0..3 {
        let x = kb_rect.min.x + 3.0 + (col_i as f32) * 4.5;
        painter.rect_filled(Rect::from_min_size(egui::pos2(x, kb_rect.min.y + 2.5), vec2(3.0, 2.2)), 0.5, kb_frame);
        painter.rect_filled(Rect::from_min_size(egui::pos2(x, kb_rect.min.y + 5.7), vec2(3.0, 2.2)), 0.5, kb_frame);
    }
    painter.rect_filled(Rect::from_min_size(egui::pos2(kb_rect.min.x + 4.5, kb_rect.max.y - 3.2), vec2(9.0, 1.8)), 0.6, kb_frame);

    let text_col = if enabled { t.text } else { t.muted };
    painter.text(egui::pos2(rect.min.x + 31.0, rect.center().y), Align2::LEFT_CENTER, "Écrire", egui::FontId::proportional(12.5), text_col);
    if !is_write {
        let dot_col = crate::theme::mix(Color32::from_rgb(0xEA, 0x58, 0x0C), Color32::from_rgb(0xFB, 0xBF, 0x24), pulse);
        painter.circle_filled(egui::pos2(rect.max.x - 10.0, rect.center().y), 3.0 + 1.2 * pulse, dot_col);
    }

    let name = "Écrire".to_string();
    resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, is_write, name.clone()));
    let tip = if is_write { "Mode écriture au clavier actif" } else { "Cliquez ici pour revenir à l'écriture au clavier" };
    resp.on_hover_text(tip)
}

/// The styles OneNote lists in its Styles gallery, and what each one is here.
pub const STYLES: [(&str, &str); 12] = [
    ("Heading 1", "h1"),
    ("Heading 2", "h2"),
    ("Heading 3", "h3"),
    ("Heading 4", "h4"),
    ("Heading 5", "h5"),
    ("Heading 6", "h6"),
    ("Page Title", "title"),
    ("Citation", "citation"),
    ("Quote", "quote"),
    ("Code", "codeblock"),
    ("123|code", "codenumbers"),
    ("Normal", "p"),
];

/// OneNote's tag list. Every one of them works: To Do is a task item, the rest are a
/// marker word at the start of the line (`ergonote_model::Tag`).
pub fn tag_label(tag: Tag) -> String {
    let label = crate::tl!(tag.label());
    match tag.shortcut() {
        Some(keys) => format!("{label} ({keys})"),
        None => label.to_string(),
    }
}

fn home_tab(ui: &mut Ui, view: &View<'_>, t: &Tokens, acts: &mut Vec<Act>) {
    group(ui, crate::tl_id!("ui.styles"), t, |ui| {
        ui.vertical(|ui| {
            ui.add_space(2.0);
            ui.spacing_mut().item_spacing = vec2(4.0, 5.0);
            for (title, kind, col, sz) in
                [("Grand Titre", "h1", t.accent, 14.0), ("Sous-titre", "h2", t.section, 13.0), ("Texte normal", "p", t.text, 12.5)]
            {
                let on = view.style == kind;
                let (rect, resp) = ui.allocate_exact_size(vec2(106.0, SMALL_H), Sense::click());
                let painter = ui.painter_at(rect);
                let bg = if on {
                    crate::theme::mix(t.editor, col, 0.18)
                } else if resp.hovered() {
                    t.row_hover
                } else {
                    t.editor
                };
                painter.rect_filled(rect, 6.0, bg);
                painter.rect_stroke(
                    rect,
                    6.0,
                    egui::Stroke::new(if on { 1.5 } else { 1.0 }, if on { col } else { t.faint }),
                    egui::StrokeKind::Inside,
                );
                let bar_rect = Rect::from_min_size(egui::pos2(rect.min.x + 3.0, rect.min.y + 4.0), vec2(3.5, rect.height() - 8.0));
                painter.rect_filled(bar_rect, 2.0, col);
                painter.text(egui::pos2(rect.min.x + 11.0, rect.center().y), Align2::LEFT_CENTER, title, egui::FontId::proportional(sz), col);
                let enabled = ui.is_enabled();
                let spoken = title.to_string();
                resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, on, &spoken));
                if resp.on_hover_text(title).clicked() {
                    acts.push(Act::Block(kind));
                }
            }
        });
    });
    group(ui, crate::tl_id!("ui.basic_text"), t, |ui| {
        ui.vertical(|ui| {
            ui.add_space(2.0);
            ui.spacing_mut().item_spacing = vec2(4.0, 5.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                // The font name: the same combo box as the size, listing the installed fonts.
                // The font list is read in the background. While it is not ready the list says so,
                // and the window asks to be drawn again until it is.
                let (mut families, families_note): (Vec<String>, Option<String>) = {
                    #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
                    {
                        match crate::fonts::catalog_if_ready() {
                            Some(Ok(catalog)) => (
                                catalog.families().iter().filter(|name| ergonote_formats::markdown::valid_font_family(name)).cloned().collect(),
                                None,
                            ),
                            Some(Err(error)) => {
                                log::warn!("Could not discover installed fonts: {error}");
                                (Vec::new(), Some(crate::tl_id!("ui.font_catalog_unavailable").to_string()))
                            }
                            None => {
                                ui.ctx().request_repaint_after(std::time::Duration::from_millis(150));
                                (Vec::new(), Some(crate::tl_id!("ui.loading_fonts").to_string()))
                            }
                        }
                    }
                    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
                    {
                        (Vec::new(), None)
                    }
                };
                if !families.iter().any(|f| f == crate::fonts::DYS_FONT_NAME) {
                    families.insert(0, crate::fonts::DYS_FONT_NAME.to_string());
                }
                let family_out = crate::combo::show(
                    ui,
                    t.text,
                    crate::combo::Spec {
                        id: ui.id().with("font-family"),
                        box_label: crate::tl_id!("ui.font_family_box"),
                        arrow_label: crate::tl_id!("ui.font_family"),
                        shown: view.font_family.unwrap_or(""),
                        width: 136.0,
                        items: &families,
                        empty_note: families_note.as_deref(),
                        accept: &|typed| ergonote_formats::markdown::valid_font_family(typed),
                    },
                );
                if let Some(name) = family_out.chosen {
                    acts.push(Act::FontFamily(name));
                }
                // OneNote's size control: an editable box showing the size, and a drop-down arrow.
                // The box shows the selected size; with no selection it keeps the last size seen.
                let last_id = ui.id().with("font-size-last");
                if let Some(now) = view.font_size.filter(|s| s.parse::<u8>().is_ok()) {
                    ui.data_mut(|d| d.insert_temp(last_id, now.to_owned()));
                }
                let shown_owned: String =
                    view.font_size.map(str::to_owned).or_else(|| ui.data(|d| d.get_temp::<String>(last_id))).unwrap_or_default();
                let shown = shown_owned.as_str();
                let sizes: Vec<String> = FONT_SIZES.iter().map(|size| size.to_string()).collect();
                let size_out = crate::combo::show(
                    ui,
                    t.text,
                    crate::combo::Spec {
                        id: ui.id().with("font-size"),
                        box_label: crate::tl_id!("ui.font_size_box"),
                        arrow_label: crate::tl_id!("ui.font_size"),
                        shown,
                        width: 34.0,
                        items: &sizes,
                        empty_note: None,
                        accept: &|typed| typed.parse::<u8>().is_ok_and(|size| FONT_SIZES.contains(&size)),
                    },
                );
                let boxes = crate::floating::FontBoxes {
                    family: view.font_family.unwrap_or("").to_owned(),
                    size: shown.to_owned(),
                    families: families.clone(),
                    families_note: families_note.clone(),
                    sizes: sizes.clone(),
                };
                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(crate::floating::FONT_BOXES_KEY), boxes));
                if let Some(size) = size_out.chosen.and_then(|typed| typed.parse::<u8>().ok()) {
                    acts.push(Act::FontSize(size));
                }
                ribbon_row_sep(ui, t);
                // i18n-exempt: superscript mathematical notation glyph.
                if small(ui, None, "x²", crate::tl_id!("ui.superscript"), crate::tl_id!("ui.superscript"), view.marks.superscript, t).clicked() {
                    acts.push(Act::Mark("superscript"));
                }
                // i18n-exempt: subscript mathematical notation glyph.
                if small(ui, None, "x₂", crate::tl_id!("ui.subscript"), crate::tl_id!("ui.subscript"), view.marks.subscript, t).clicked() {
                    acts.push(Act::Mark("subscript"));
                }
                if mark_button(ui, Mark::ClearFormat, crate::tl_id!("ui.clear_formatting_2"), crate::tl_id!("ui.take_the_marks_off"), false, t)
                    .clicked()
                {
                    acts.push(Act::ClearFormatting);
                }
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if mark_button(ui, Mark::Bold, crate::tl_id!("ui.bold"), crate::tl_id!("ui.bold_ctrl_b"), view.marks.bold, t).clicked() {
                    acts.push(Act::Mark("bold"));
                }
                if mark_button(ui, Mark::Italic, crate::tl_id!("ui.italic"), crate::tl_id!("ui.italic_ctrl_i"), view.marks.italic, t).clicked() {
                    acts.push(Act::Mark("italic"));
                }
                if mark_button(ui, Mark::Underline, crate::tl_id!("ui.underline"), crate::tl_id!("ui.underline"), view.marks.underline, t).clicked() {
                    acts.push(Act::Mark("underline"));
                }
                ribbon_row_sep(ui, t);
                text_color_picker_button(ui, t, acts);
                highlight_color_picker_button(ui, t, acts);
                underline_color_picker_button(ui, t, acts);
                ribbon_row_sep(ui, t);
                if small(
                    ui,
                    Some(Icon::AlignLeft),
                    "",
                    crate::tl_id!("ui.align_left_ctrl_l"),
                    crate::tl_id!("ui.align_left_ctrl_l"),
                    view.align == Some(ergonote_model::justify::Align::Left),
                    t,
                )
                .clicked()
                {
                    acts.push(Act::Align(ergonote_model::justify::Align::Left));
                }
                if small(
                    ui,
                    Some(Icon::AlignCenter),
                    "",
                    crate::tl_id!("ui.center"),
                    crate::tl_id!("ui.center"),
                    view.align == Some(ergonote_model::justify::Align::Center),
                    t,
                )
                .clicked()
                {
                    acts.push(Act::Align(ergonote_model::justify::Align::Center));
                }
                if small(
                    ui,
                    Some(Icon::AlignRight),
                    "",
                    crate::tl_id!("ui.align_right_ctrl_r"),
                    crate::tl_id!("ui.align_right_ctrl_r"),
                    view.align == Some(ergonote_model::justify::Align::Right),
                    t,
                )
                .clicked()
                {
                    acts.push(Act::Align(ergonote_model::justify::Align::Right));
                }
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if ribbon_selector_pill(
                    ui,
                    119.0,
                    SMALL_H,
                    PillGlyph::Icon(Icon::ListBullet),
                    None,
                    "Puces",
                    false,
                    view.style == "bullet",
                    crate::tl_id!("ui.a_bullet_list"),
                    t,
                )
                .clicked()
                {
                    acts.push(Act::Block("bullet"));
                }
                if ribbon_selector_pill(
                    ui,
                    119.0,
                    SMALL_H,
                    PillGlyph::Icon(Icon::ListNumber),
                    None,
                    "Numéros",
                    false,
                    view.style == "number",
                    crate::tl_id!("ui.a_numbered_list"),
                    t,
                )
                .clicked()
                {
                    acts.push(Act::Block("number"));
                }
                ribbon_row_sep(ui, t);
                if write_mode_pill(ui, 104.0, view.pointer, t).clicked() {
                    acts.push(Act::InkPointer(Pointer::Write));
                }
            });
        });
    });
    group(ui, crate::tl_id!("ui.student_insert_group"), t, |ui| {
        if big(ui, Icon::Picture, crate::tl_id!("ui.student_photo"), crate::tl_id!("ui.student_photo_tip"), t).clicked() {
            acts.push(Act::BoardPhoto);
        }
        pictures_menu(ui, t, acts);
        table_menu(ui, view, t, acts);
        if big_on(ui, Icon::TextBox, crate::tl_id!("ui.student_textbox_label"), crate::tl_id!("ui.student_textbox_tip"), view.textbox_tool, t)
            .clicked()
        {
            acts.push(Act::InsertTextBox);
        }
        insert_draw_controls(ui, view, t, acts);
        insert_stickers_controls(ui, t, acts);
        more_insert_menu(ui, t, acts);
    });
    group(ui, crate::tl_id!("ui.student_aid_tnd_group"), t, |ui| {
        if big(ui, Icon::ReadAloud, crate::tl_id!("ui.student_read_aloud"), crate::tl_id!("ui.student_read_aloud_tip"), t).clicked() {
            acts.push(Act::ReadAloud);
        }
        if big_on(ui, Icon::Mic, crate::tl_id!("ui.student_dictate"), crate::tl_id!("ui.student_dictate_tip"), view.dictating, t).clicked() {
            acts.push(Act::VoiceDictation);
        }
        if big_on(ui, Icon::Ruler, crate::tl_id!("ui.student_reading_ruler"), crate::tl_id!("ui.student_reading_ruler_tip"), view.reading_ruler, t)
            .clicked()
        {
            acts.push(Act::ToggleReadingRuler);
        }
        if big_on(
            ui,
            Icon::InkToText,
            crate::tl_id!("ui.student_word_prediction"),
            crate::tl_id!("ui.student_word_prediction_tip"),
            view.word_prediction,
            t,
        )
        .clicked()
        {
            acts.push(Act::ToggleWordPrediction);
        }
        if big(ui, Icon::SpellCheck, crate::tl_id!("ui.student_spellcheck"), crate::tl_id!("ui.student_spellcheck_tip"), t).clicked() {
            acts.push(Act::CheckOrthography);
        }
    });
    page_setup_group(ui, view, t, acts);
}

/// View > Paper Size (issue #269): Auto or a sheet, which way it is turned, and its margins.
/// Every change is an act at once, so the page shows it while the menu stays open.
fn paper_size_menu(ui: &mut Ui, now: Option<ergonote_model::paper::Paper>, t: &Tokens, acts: &mut Vec<Act>) {
    use ergonote_model::paper::{MAX_SIDE, MIN_SIDE, Paper, Size};
    ui.set_min_width(250.0);
    ui.label(egui::RichText::new(crate::tl_id!("ui.size")).strong());
    if ui.selectable_label(now.is_none(), crate::tl_id!("ui.auto_the_page_grows_with_what_is_on_it")).clicked() {
        acts.push(Act::PaperSize(None));
    }
    let base = now.unwrap_or_else(|| Paper::new(Size::Letter));
    egui::Grid::new("paper-sizes").num_columns(2).spacing(vec2(6.0, 1.0)).show(ui, |ui| {
        for (n, size) in Size::STANDARD.into_iter().enumerate() {
            let here = now.is_some_and(|p| p.size == size);
            if ui.selectable_label(here, crate::tl!(size.label())).clicked() {
                acts.push(Act::PaperSize(Some(Paper { size, ..base }.write())));
            }
            if n % 2 == 1 {
                ui.end_row();
            }
        }
        let custom = now.is_some_and(|p| matches!(p.size, Size::Custom(..)));
        if ui.selectable_label(custom, crate::tl_id!("ui.custom_size")).clicked() && !custom {
            let (w, h) = base.sheet_mm();
            acts.push(Act::PaperSize(Some(Paper { size: Size::Custom(w, h), landscape: false, ..base }.write())));
        }
    });
    let Some(paper) = now else {
        ui.add_space(2.0);
        ui.label(egui::RichText::new(crate::tl_id!("ui.auto_pages_have_no_edges_choose_a_size_to_see_the_sheet")).color(t.muted).size(11.0));
        return;
    };
    let mut next = paper;
    if let Size::Custom(w, h) = paper.size {
        let (mut w, mut h) = (w, h);
        ui.horizontal(|ui| {
            mm_field(ui, &mut w, MIN_SIDE..=MAX_SIDE, crate::tl_id!("ui.width_mm"));
            mm_field(ui, &mut h, MIN_SIDE..=MAX_SIDE, crate::tl_id!("ui.height_mm"));
        });
        next.size = Size::Custom(w, h);
    } else {
        ui.separator();
        ui.label(egui::RichText::new(crate::tl_id!("ui.orientation")).strong());
        ui.horizontal(|ui| {
            if ui.selectable_label(!paper.landscape, crate::tl_id!("ui.portrait")).clicked() {
                next.landscape = false;
            }
            if ui.selectable_label(paper.landscape, crate::tl_id!("ui.landscape")).clicked() {
                next.landscape = true;
            }
        });
    }
    ui.separator();
    ui.label(egui::RichText::new(crate::tl_id!("ui.margins_mm")).strong());
    let (w, h) = paper.sheet_mm();
    let [mut top, mut right, mut bottom, mut left] = paper.margins;
    egui::Grid::new("paper-margins").num_columns(2).show(ui, |ui| {
        mm_field(ui, &mut top, 0.0..=h / 2.0, crate::tl_id!("ui.top_margin_mm"));
        mm_field(ui, &mut bottom, 0.0..=h / 2.0, crate::tl_id!("ui.bottom_margin_mm"));
        ui.end_row();
        mm_field(ui, &mut left, 0.0..=w / 2.0, crate::tl_id!("ui.left_margin_mm"));
        mm_field(ui, &mut right, 0.0..=w / 2.0, crate::tl_id!("ui.right_margin_mm"));
        ui.end_row();
    });
    next.margins = [top, right, bottom, left];
    if ui.button(crate::tl_id!("ui.restore_default_margins")).clicked() {
        next.margins = Paper::DEFAULT_MARGINS;
    }
    if next != paper {
        acts.push(Act::PaperSize(Some(next.write())));
    }
}

/// A length in millimetres, labelled for a screen reader, that can be dragged or typed.
// i18n-ui-text: label
fn mm_field(ui: &mut Ui, value: &mut f32, range: std::ops::RangeInclusive<f32>, label: &str) {
    ui.horizontal(|ui| {
        let name = ui.label(egui::RichText::new(label).size(11.5));
        ui.add(egui::DragValue::new(value).range(range).speed(0.5).max_decimals(1)).labelled_by(name.id);
    });
}

/// The equations the Equation button offers, each a name and the TeX it puts in the page. They are
/// drawn in the list, so a person picks by looking. Every one of them is something the engine
/// draws in full; a test checks it.
pub const EQUATIONS: &[(&str, &str)] = &[
    ("Fraction", r"\frac{a}{b}"),
    ("Square root", r"\sqrt{x}"),
    ("Root of any degree", r"\sqrt[n]{x}"),
    ("Superscript and subscript", r"x_i^2"),
    ("Brackets", r"\left( \frac{a}{b} \right)"),
    ("Sum", r"\sum_{i=1}^{n} i"),
    ("Integral", r"\int_{a}^{b} f(x)\,dx"),
    ("Limit", r"\lim_{x \to 0} \frac{\sin x}{x}"),
    ("Binomial", r"\binom{n}{k}"),
    ("Accents", r"\hat{x} + \vec{v} + \bar{y}"),
    ("Matrix", r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}"),
    ("Cases", r"f(x) = \begin{cases} x^2 & x \ge 0 \\ -x & x < 0 \end{cases}"),
    ("Aligned equations", r"\begin{aligned} a &= b + c \\ d &= e + f \end{aligned}"),
    ("Quadratic formula", r"x = \frac{-b \pm \sqrt{b^2-4ac}}{2a}"),
    ("Pythagorean theorem", r"a^2 + b^2 = c^2"),
    ("Euler's identity", r"e^{i\pi} + 1 = 0"),
];

/// OneNote's mark for a tag. A custom tag has no mark of its own; it shows its symbol.
pub fn mark_for(tag: Tag) -> Option<Mark> {
    Some(match tag {
        Tag::ToDo => Mark::ToDo,
        Tag::Important => Mark::Important,
        Tag::Question => Mark::Question,
        Tag::Remember => Mark::Remember,
        Tag::Definition => Mark::Definition,
        Tag::Highlight => Mark::Highlighter,
        Tag::Contact => Mark::Contact,
        Tag::Address => Mark::Address,
        Tag::Phone => Mark::Phone,
        Tag::Website => Mark::Website,
        Tag::Idea => Mark::Idea,
        Tag::Critical => Mark::Critical,
        Tag::Custom(_) => return None,
    })
}

/// Draws a tag's mark in `rect`: OneNote's own for a built-in tag, the person's symbol in its
/// colour for one they defined (issue #259).
pub fn paint_tag(painter: &egui::Painter, rect: Rect, tag: Tag, text: Color32, accent: Color32) {
    match (mark_for(tag), tag.custom_look()) {
        (Some(mark), _) => ribbon_icons::paint(painter, rect, mark, mark.color(text, accent)),
        (None, Some((symbol, colour))) => paint_symbol(painter, rect, symbol, colour),
        (None, None) => {}
    }
}

/// A custom tag's symbol in its colour; a plain dot when it has none.
pub fn paint_symbol(painter: &egui::Painter, rect: Rect, symbol: &str, colour: ergonote_model::NodeColor) {
    let (r, g, b) = colour.rgb(painter.ctx().global_style().visuals.dark_mode);
    let colour = Color32::from_rgb(r, g, b);
    if symbol.is_empty() {
        painter.circle_filled(rect.center(), rect.height() * 0.3, colour);
    } else {
        painter.text(rect.center(), Align2::CENTER_CENTER, symbol, egui::FontId::proportional(rect.height() * 0.9), colour);
    }
}

/// Compact toggle control with icon, visible label, and separate spoken accessibility name.
// i18n-ui-text: label,tip
fn mark_button(ui: &mut Ui, mark: Mark, label: &str, tip: &str, on: bool, t: &Tokens) -> egui::Response {
    let label = crate::tl!(label);
    let tip = crate::tl!(tip);
    let (rect, resp) = ui.allocate_exact_size(vec2(SMALL_W, SMALL_H), Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect_filled(
        rect,
        6.0,
        if on {
            t.row_selected
        } else if resp.hovered() {
            t.row_hover
        } else {
            t.editor
        },
    );
    painter.rect_stroke(rect, 6.0, egui::Stroke::new(if on { 1.5 } else { 1.0 }, if on { t.accent } else { t.faint }), egui::StrokeKind::Inside);
    ribbon_icons::paint(&painter, Rect::from_center_size(rect.center(), vec2(18.0, 18.0)), mark, mark.color(t.text, t.accent));
    let enabled = ui.is_enabled();
    let name = label.to_string();
    resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, on, name.clone()));
    resp.on_hover_text(tip)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ColorPickerKind {
    Text,
    Highlight,
    Underline,
}

fn color_picker_popup(ui: &mut Ui, id: &str, tip: &str, label_text: &str, kind: ColorPickerKind, t: &Tokens, acts: &mut Vec<Act>) {
    let mem_id = ui.id().with((id, "last-color"));
    let default_color = match kind {
        ColorPickerKind::Highlight => ergonote_model::NodeColor::Yellow,
        ColorPickerKind::Text | ColorPickerKind::Underline => ergonote_model::NodeColor::Red,
    };
    let active_color = ui.data(|d| d.get_temp::<Option<ergonote_model::NodeColor>>(mem_id)).flatten().unwrap_or(default_color);
    let dark = t.editor.r() < 128;
    let swatch_for = |c: ergonote_model::NodeColor| match kind {
        ColorPickerKind::Text => crate::theme::text_swatch_color(c, dark),
        ColorPickerKind::Highlight => crate::theme::highlight_bg(c, dark),
        ColorPickerKind::Underline => {
            let (r, g, b) = c.rgb(false);
            Color32::from_rgb(r, g, b)
        }
    };
    let bar_color = swatch_for(active_color);

    let (rect, resp) = ui.allocate_exact_size(vec2(40.0, SMALL_H), Sense::click());
    let painter = ui.painter_at(rect);
    let bg = if resp.hovered() { t.row_hover } else { t.editor };
    painter.rect_filled(rect, 6.0, bg);
    painter.rect_stroke(rect, 6.0, egui::Stroke::new(1.0, t.faint), egui::StrokeKind::Inside);
    let glyph_pos = egui::pos2(rect.min.x + 13.5, rect.center().y - 3.0);
    if kind == ColorPickerKind::Highlight {
        let mark_rect = Rect::from_center_size(glyph_pos, vec2(20.0, 14.0));
        painter.rect_filled(mark_rect, 2.0, bar_color);
        painter.text(glyph_pos, Align2::CENTER_CENTER, label_text, egui::FontId::proportional(11.5), Color32::from_rgb(30, 32, 36));
    } else {
        painter.text(glyph_pos, Align2::CENTER_CENTER, label_text, egui::FontId::proportional(14.5), t.strong);
    }
    painter.line_segment(
        [egui::pos2(rect.min.x + 4.0, rect.max.y - 4.0), egui::pos2(rect.min.x + 23.0, rect.max.y - 4.0)],
        egui::Stroke::new(3.5, bar_color),
    );
    // Small dropdown chevron on the right.
    let cx = rect.max.x - 7.5;
    let cy = rect.center().y + 1.0;
    painter.add(egui::Shape::line(
        vec![egui::pos2(cx - 3.0, cy - 1.5), egui::pos2(cx, cy + 2.0), egui::pos2(cx + 3.0, cy - 1.5)],
        egui::Stroke::new(1.4, t.muted),
    ));
    let enabled = ui.is_enabled();
    let name = tip.to_string();
    let resp = resp.on_hover_text(tip);
    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &name));
    egui::Popup::menu(&resp).id(ui.id().with(id)).show(|ui| {
        egui::Frame::popup(ui.style()).corner_radius(8.0).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
            ui.label(egui::RichText::new(tip).strong().color(t.text));
            ui.add_space(4.0);
            ui.horizontal(|ui| match kind {
                ColorPickerKind::Text => {
                    if ui.small_button(crate::tl_id!("ui.student_default")).clicked() {
                        ui.data_mut(|d| d.remove::<Option<ergonote_model::NodeColor>>(mem_id));
                        acts.push(Act::TextColor(None));
                        ui.close();
                    }
                }
                ColorPickerKind::Highlight => {
                    if ui.small_button(crate::tl_id!("ui.student_none")).clicked() {
                        ui.data_mut(|d| d.remove::<Option<ergonote_model::NodeColor>>(mem_id));
                        acts.push(Act::HighlightColor(None));
                        ui.close();
                    }
                }
                ColorPickerKind::Underline => {
                    if ui.small_button(crate::tl_id!("ui.student_normal")).clicked() {
                        ui.data_mut(|d| d.remove::<Option<ergonote_model::NodeColor>>(mem_id));
                        acts.push(Act::Mark("underline"));
                        ui.close();
                    }
                    if ui.small_button(crate::tl_id!("ui.student_none")).clicked() {
                        ui.data_mut(|d| d.remove::<Option<ergonote_model::NodeColor>>(mem_id));
                        acts.push(Act::UnderlineColor(None));
                        ui.close();
                    }
                }
            });
            ui.add_space(4.0);
            egui::Grid::new(id).spacing([4.0, 4.0]).show(ui, |ui| {
                for (i, c) in ergonote_model::NodeColor::ALL.into_iter().enumerate() {
                    let swatch = swatch_for(c);
                    let (r_rect, r_resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                    ui.painter().rect_filled(r_rect, 4.0, swatch);
                    ui.painter().rect_stroke(
                        r_rect,
                        4.0,
                        egui::Stroke::new(if r_resp.hovered() { 1.5 } else { 1.0 }, if r_resp.hovered() { t.strong } else { t.faint }),
                        egui::StrokeKind::Inside,
                    );
                    let c_label = crate::tl!(c.label());
                    let c_name = c_label.to_string();
                    r_resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &c_name));
                    if r_resp.on_hover_text(c_label).clicked() {
                        ui.data_mut(|d| d.insert_temp(mem_id, Some(c)));
                        acts.push(match kind {
                            ColorPickerKind::Text => Act::TextColor(Some(c)),
                            ColorPickerKind::Highlight => Act::HighlightColor(Some(c)),
                            ColorPickerKind::Underline => Act::UnderlineColor(Some(c)),
                        });
                        ui.close();
                    }
                    if i % 8 == 7 {
                        ui.end_row();
                    }
                }
            });
        });
    });
}

fn text_color_picker_button(ui: &mut Ui, t: &Tokens, acts: &mut Vec<Act>) {
    color_picker_popup(ui, "text-color-picker", "Couleur du texte", "A", ColorPickerKind::Text, t, acts);
}

fn highlight_color_picker_button(ui: &mut Ui, t: &Tokens, acts: &mut Vec<Act>) {
    color_picker_popup(ui, "highlight-color-picker", "Surligner le texte", "ab", ColorPickerKind::Highlight, t, acts);
}

fn underline_color_picker_button(ui: &mut Ui, t: &Tokens, acts: &mut Vec<Act>) {
    color_picker_popup(ui, "underline-color-picker", "Couleur du soulignage", "S", ColorPickerKind::Underline, t, acts);
}

/// One paper in the gallery: the paper itself, drawn small, with its name under it. Picking
/// a paper is picking a look, so the look is what is shown; the name is there to say it.
fn paper_swatch(ui: &mut Ui, r: RuleLines, here: bool, t: &Tokens) -> egui::Response {
    // Half size. At that scale the closest two papers still differ by two lines, and
    // ErgoNote's own show their stronger fifth line, which is the whole point of them.
    const SCALE: f32 = 0.5;
    const PAPER: egui::Vec2 = egui::vec2(78.0, 66.0);
    const CAPTION: f32 = 15.0;
    let (rect, resp) = ui.allocate_exact_size(PAPER + vec2(0.0, CAPTION), egui::Sense::click());
    let enabled = ui.is_enabled();
    let label = r.translated_label();
    let caption = r.translated_mid();
    resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, here, label));
    let paper = egui::Rect::from_min_size(rect.min, PAPER);
    let painter = ui.painter().with_clip_rect(paper);
    painter.rect_filled(paper, 2.0, t.editor);
    paint_paper(&painter, paper, r, SCALE, t.rule_line, t.rule_line_strong);
    // A page with no lines needs something to look at, so it says so in the middle.
    if !r.on() {
        painter.text(paper.center(), egui::Align2::CENTER_CENTER, crate::tl_id!("ui.plain"), egui::FontId::proportional(12.0), t.muted);
    }
    let (edge, width) = if here {
        (t.accent, 2.0)
    } else if resp.hovered() {
        (t.muted, 1.0)
    } else {
        (t.faint, 1.0)
    };
    ui.painter().rect_stroke(paper, 2.0, egui::Stroke::new(width, edge), egui::StrokeKind::Inside);
    // The name is kept inside the thumbnail's own width, so a long one cannot run into the
    // paper beside it.
    let colour = if here { t.accent } else { t.text };
    let name = egui::pos2(paper.center().x, paper.max.y + 1.0);
    ui.painter().with_clip_rect(rect).text(name, egui::Align2::CENTER_TOP, caption, egui::FontId::proportional(11.0), colour);
    resp.on_hover_text(label)
}

/// One page colour in the Format Background menu: a pale wash of it, as the page is
/// drawn, with a ring round the one the page is using.
fn colour_swatch(ui: &mut Ui, colour: Option<ergonote_model::NodeColor>, here: bool, t: &Tokens) -> egui::Response {
    let side = 22.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(side, side), Sense::click());
    let dark = t.editor.r() < 128;
    let fill = match colour {
        Some(c) => {
            let (r, g, b) = c.rgb(dark);
            crate::theme::mix(t.editor, egui::Color32::from_rgb(r, g, b), 0.16)
        }
        None => t.editor,
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 3.0, fill);
    let (edge, width) = if here { (t.accent, 2.0) } else { (t.faint, 1.0) };
    painter.rect_stroke(rect, 3.0, egui::Stroke::new(width, edge), egui::StrokeKind::Inside);
    if colour.is_none() {
        painter.text(rect.center(), egui::Align2::CENTER_CENTER, "/", egui::FontId::proportional(13.0), t.muted);
    }
    let name = colour.map(ergonote_model::NodeColor::label).unwrap_or(crate::tl_id!("ui.no_colour"));
    let enabled = ui.is_enabled();
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, here, name));
    resp.on_hover_text(name)
}

/// The Rule Line Colour row of the paper gallery (issue #288): Automatic (the theme's own lines)
/// and the named colours, each swatch drawn as a short rule in the colour the lines will be.
fn rule_colour_palette(ui: &mut Ui, view: &View<'_>, t: &Tokens, acts: &mut Vec<Act>) -> bool {
    let mut picked = false;
    ui.label(egui::RichText::new(crate::tl_id!("ui.rule_line_colour")).strong().color(t.text));
    ui.add_enabled_ui(view.has_page, |ui| {
        ui.horizontal(|ui| {
            for colour in std::iter::once(None).chain(ergonote_model::NodeColor::ALL.into_iter().map(Some)) {
                let here = view.rule_colour == colour;
                let side = 22.0;
                let (rect, resp) = ui.allocate_exact_size(vec2(side, side), Sense::click());
                let painter = ui.painter();
                painter.rect_filled(rect, 3.0, t.editor);
                let (_, strong) = crate::theme::rule_lines_in(t, colour);
                for k in 1..4 {
                    #[allow(clippy::cast_precision_loss)]
                    let y = rect.min.y + side * k as f32 / 4.0;
                    painter.hline(rect.min.x + 3.0..=rect.max.x - 3.0, y, egui::Stroke::new(1.5, strong));
                }
                let (edge, width) = if here { (t.accent, 2.0) } else { (t.faint, 1.0) };
                painter.rect_stroke(rect, 3.0, egui::Stroke::new(width, edge), egui::StrokeKind::Inside);
                // Named apart from the page colour swatches beside them: "Blue lines".
                let name = match colour {
                    Some(c) => crate::i18n::format_message_id("ui.colour_lines", &[("colour", crate::tl!(c.label()))]),
                    None => crate::tl_id!("ui.automatic_rule_lines").to_string(),
                };
                let enabled = ui.is_enabled();
                let said = name.clone();
                resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, here, &said));
                if resp.on_hover_text(name).clicked() {
                    acts.push(Act::RuleColour(colour));
                    picked = true;
                }
            }
        });
    });
    picked
}

/// Shared by the direct Page colour menu and the combined Format Background gallery.
fn page_colour_palette(ui: &mut Ui, view: &View<'_>, t: &Tokens, acts: &mut Vec<Act>) -> bool {
    let mut picked = false;
    ui.label(egui::RichText::new(crate::tl_id!("ui.page_colour")).strong().color(t.text));
    ui.horizontal(|ui| {
        if colour_swatch(ui, None, view.page_colour.is_none(), t).clicked() {
            acts.push(Act::PageColour(None));
            picked = true;
        }
        for c in ergonote_model::NodeColor::ALL {
            if colour_swatch(ui, Some(c), view.page_colour == Some(c), t).clicked() {
                acts.push(Act::PageColour(Some(c)));
                picked = true;
            }
        }
    });
    ui.label(egui::RichText::new(crate::tl_id!("ui.the_paper_and_colour_apply_to_this_page_only")).small().color(t.muted));
    picked
}

/// Table > Shading: a colour behind the caret's cell, or behind every picked cell (issue #169).
/// The palette is the named colours a page or section can carry, washed the way the cell will
/// be drawn, with No Colour first; the colour the cells have now is ringed.
fn shading_menu(ui: &mut Ui, view: &View<'_>, t: &Tokens, acts: &mut Vec<Act>) {
    let (rect, resp) = ui.allocate_exact_size(vec2(64.0, GROUP_H - 20.0), Sense::click());
    let painter = ui.painter_at(rect);
    if resp.hovered() {
        painter.rect_filled(rect, 8.0, t.row_hover);
    }
    let active_colour = view.table_shade.flatten();
    let swatch_fill = active_colour.map_or(t.editor, |c| crate::pageedit::cell_shade(t, c));
    let badge = Rect::from_center_size(egui::pos2(rect.center().x, rect.min.y + ICON_Y), vec2(32.0, 30.0));
    painter.rect_filled(badge, 8.0, swatch_fill);
    let badge_stroke = if active_colour.is_some() { t.accent } else { t.faint };
    painter.rect_stroke(badge, 8.0, egui::Stroke::new(1.25, badge_stroke), egui::StrokeKind::Inside);
    if active_colour.is_none() {
        painter.line_segment([badge.min + vec2(9.0, 21.0), badge.max - vec2(9.0, 21.0)], egui::Stroke::new(1.5, t.muted));
    }
    let label = "Couleur";
    let font = egui::FontId::proportional(12.0);
    let text_w = painter.layout_no_wrap(label.to_string(), font.clone(), t.text).size().x;
    let total_w = text_w + 10.0;
    let text_x = rect.center().x - total_w * 0.5;
    painter.text(egui::pos2(text_x, rect.min.y + LABEL_Y), Align2::LEFT_CENTER, label, font, t.text);
    let chev_x = text_x + text_w + 6.0;
    let chev_y = rect.min.y + LABEL_Y + 0.5;
    painter.add(egui::Shape::line(
        vec![egui::pos2(chev_x - 3.0, chev_y - 1.5), egui::pos2(chev_x, chev_y + 1.5), egui::pos2(chev_x + 3.0, chev_y - 1.5)],
        egui::Stroke::new(1.3, t.muted),
    ));
    let enabled = ui.is_enabled();
    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "Couleur"));
    let resp = resp.on_hover_text(crate::tl_id!("ui.student_cell_bg_tip"));

    let open_id = egui::Id::new("ribbon-table-shading");
    let mut open = ui.data(|d| d.get_temp::<bool>(open_id).unwrap_or(false));
    if resp.clicked() {
        open = !open;
    }
    if !open {
        ui.data_mut(|d| d.insert_temp(open_id, open));
        return;
    }
    let mut picked = false;
    let area = egui::Area::new(open_id.with("area"))
        .order(egui::Order::Foreground)
        .fixed_pos(resp.rect.left_bottom())
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.label(egui::RichText::new(crate::tl_id!("ui.student_fill_label")).strong().color(t.text));
                ui.horizontal(|ui| {
                    let now = view.table_shade;
                    if shade_swatch(ui, None, now == Some(None), t).clicked() {
                        acts.push(Act::TableShade(None));
                        picked = true;
                    }
                    for c in ergonote_model::NodeColor::ALL {
                        if shade_swatch(ui, Some(c), now == Some(Some(c)), t).clicked() {
                            acts.push(Act::TableShade(Some(c)));
                            picked = true;
                        }
                    }
                });
            });
        })
        .response;
    let clicked_away =
        ui.input(|i| i.pointer.any_click()) && !area.rect.contains(ui.ctx().pointer_latest_pos().unwrap_or_default()) && !resp.clicked();
    if picked || clicked_away || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        open = false;
    }
    ui.data_mut(|d| d.insert_temp(open_id, open));
}

/// One colour in the Shading palette, drawn as a cell shaded with it would be.
fn shade_swatch(ui: &mut Ui, colour: Option<ergonote_model::NodeColor>, here: bool, t: &Tokens) -> egui::Response {
    let side = 22.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(side, side), Sense::click());
    let fill = colour.map_or(t.editor, |c| crate::pageedit::cell_shade(t, c));
    let painter = ui.painter();
    painter.rect_filled(rect, 3.0, fill);
    let (edge, width) = if here { (t.accent, 2.0) } else { (t.faint, 1.0) };
    painter.rect_stroke(rect, 3.0, egui::Stroke::new(width, edge), egui::StrokeKind::Inside);
    if colour.is_none() {
        painter.text(rect.center(), egui::Align2::CENTER_CENTER, "/", egui::FontId::proportional(13.0), t.muted);
    }
    let name = colour.map(ergonote_model::NodeColor::label).unwrap_or(crate::tl_id!("ui.no_colour"));
    let enabled = ui.is_enabled();
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, here, name));
    resp.on_hover_text(name)
}

/// OneNote's Table button: a grid to pick the size from, the way OneNote picks one.
///
/// Running the pointer over the grid lights the cells up to it and names the size, and a
/// click puts a table of that size on the page. OneNote's menu ends with a dialog and a
/// new Excel spreadsheet; the dialog is not built, and the spreadsheet is GridCraft's.
fn table_menu(ui: &mut Ui, view: &View<'_>, t: &Tokens, acts: &mut Vec<Act>) {
    const MOST: usize = 8;
    const SIDE: f32 = 17.0;
    const GAP: f32 = 3.0;

    let resp = big_menu(ui, Icon::Table, crate::tl_id!("ui.table_3"), crate::tl_id!("ui.a_table_pick_its_size"), t);
    let open_id = egui::Id::new("ribbon-insert-table");
    let mut open = ui.data(|d| d.get_temp::<bool>(open_id).unwrap_or(false));
    if resp.clicked() {
        open = !open;
    }
    if !open {
        ui.data_mut(|d| d.insert_temp(open_id, open));
        return;
    }
    let mut picked = false;
    let area = egui::Area::new(open_id.with("area"))
        .order(egui::Order::Foreground)
        .fixed_pos(resp.rect.left_bottom())
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                let across = MOST as f32 * (SIDE + GAP) - GAP;
                // The size is named above the grid, as OneNote names it, so the rect is
                // taken first and written into once the pointer has been read.
                let (naming, _) = ui.allocate_exact_size(vec2(across, 18.0), Sense::hover());
                let (grid, _) = ui.allocate_exact_size(vec2(across, across), Sense::hover());
                // Every square is its own button, named for the size it picks: the grid
                // then answers the keyboard and a screen reader, not only the pointer.
                let mut over = None;
                let mut chose = None;
                let mut cells = Vec::with_capacity(MOST * MOST);
                for r in 0..MOST {
                    for c in 0..MOST {
                        let at = grid.min + vec2(c as f32 * (SIDE + GAP), r as f32 * (SIDE + GAP));
                        let cell = egui::Rect::from_min_size(at, egui::vec2(SIDE, SIDE));
                        let resp = ui.interact(cell, open_id.with((r, c)), Sense::click());
                        let name = crate::i18n::format_message_id(
                            "ui.columns_x_rows_table",
                            &[("columns", &(c + 1).to_string()), ("rows", &(r + 1).to_string())],
                        );
                        let enabled = ui.is_enabled();
                        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &name));
                        if resp.hovered() {
                            over = Some((r, c));
                        }
                        if resp.clicked() {
                            chose = Some((r, c));
                        }
                        cells.push((r, c, cell));
                    }
                }
                let paint = ui.painter();
                for (r, c, cell) in cells {
                    let lit = over.is_some_and(|(hr, hc)| r <= hr && c <= hc);
                    let (fill, line) = if lit { (t.accent.gamma_multiply(0.25), t.accent) } else { (t.editor, t.table_line) };
                    paint.rect_filled(cell, 1.0, fill);
                    paint.rect_stroke(cell, 1.0, egui::Stroke::new(1.0, line), egui::StrokeKind::Inside);
                }
                let title = match over {
                    Some((r, c)) => crate::i18n::format_message_id(
                        "ui.columns_x_rows_table",
                        &[("columns", &(c + 1).to_string()), ("rows", &(r + 1).to_string())],
                    ),
                    None => crate::tl_id!("ui.table_3").to_string(),
                };
                paint.text(naming.left_top(), egui::Align2::LEFT_TOP, title, egui::FontId::proportional(13.0), t.text);
                if let Some((r, c)) = chose {
                    acts.push(Act::InsertTable(r + 1, c + 1));
                    picked = true;
                }
                ui.separator();
                if floating::menu_row(ui, Some(Glyph::Icon(Icon::Table)), crate::tl_id!("ui.insert_table_2"), t).clicked() {
                    acts.push(Act::InsertTableDialog);
                    picked = true;
                }
                ui.separator();
                ui.label(egui::RichText::new(crate::tl_id!("ui.student_school_templates_hdr")).small().strong().color(t.muted));
                for (label, kind) in [
                    ("Tableau de conversion (km -> mm)", "conversion"),
                    ("Tableau de numération (Mille / Unités)", "numeration"),
                    ("Tableau de proportionnalité", "proportion"),
                    ("Pose d'opération en colonnes (C / D / U)", "operation"),
                ] {
                    if floating::menu_row(ui, Some(Glyph::Icon(Icon::Table)), label, t).clicked() {
                        acts.push(Act::InsertSchoolTable(kind));
                        picked = true;
                    }
                }
                ui.separator();
                if view.gridcraft {
                    if floating::menu_row(ui, Some(Glyph::Icon(Icon::Spreadsheet)), crate::tl_id!("ui.new_gridcraft_spreadsheet"), t).clicked() {
                        acts.push(Act::NewGridCraft);
                        picked = true;
                    }
                } else {
                    ui.add_enabled(
                        false,
                        egui::Label::new(egui::RichText::new(crate::tl_id!("ui.new_gridcraft_spreadsheet_not_installed")).color(t.muted)),
                    );
                }
            });
        })
        .response;
    let clicked_away =
        ui.input(|i| i.pointer.any_click()) && !area.rect.contains(ui.ctx().pointer_latest_pos().unwrap_or_default()) && !resp.clicked();
    if picked || clicked_away || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        open = false;
    }
    ui.data_mut(|d| d.insert_temp(open_id, open));
}

/// Compact rule-lines and page background gallery button inside Mise en page.
fn compact_rule_lines_button(ui: &mut Ui, view: &View<'_>, t: &Tokens, acts: &mut Vec<Act>) {
    let open_id = egui::Id::new("ribbon-rule-lines");
    let mut open = ui.data(|d| d.get_temp::<bool>(open_id).unwrap_or(false));
    let active = open || view.rules != RuleLines::None;
    let resp = ribbon_selector_pill(
        ui,
        98.0,
        33.0,
        PillGlyph::Icon(Icon::Ruler),
        None,
        "Lignes",
        true,
        active,
        crate::tl_id!("ui.lines_or_squares_behind_the_page_as_on_paper"),
        t,
    );
    if resp.clicked() {
        open = !open;
    }
    if open {
        let mut picked = false;
        let area = egui::Area::new(open_id.with("area"))
            .order(egui::Order::Foreground)
            .fixed_pos(resp.rect.left_bottom())
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).corner_radius(8.0).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                    ui.spacing_mut().item_spacing = vec2(8.0, 5.0);
                    let shelf = |ui: &mut Ui, title: &str, papers: &[RuleLines], acts: &mut Vec<Act>, picked: &mut bool| {
                        ui.label(egui::RichText::new(title).strong().color(t.text));
                        ui.horizontal(|ui| {
                            for r in papers {
                                if paper_swatch(ui, *r, view.rules == *r, t).clicked() {
                                    acts.push(Act::RuleLines(*r));
                                    *picked = true;
                                }
                            }
                        });
                    };
                    shelf(ui, "No lines", &[RuleLines::None], acts, &mut picked);
                    shelf(ui, "Rule lines", &RuleLines::RULED, acts, &mut picked);
                    shelf(ui, "Grid lines", &RuleLines::GRIDS, acts, &mut picked);
                    shelf(ui, "ErgoNote's own", &RuleLines::CRAFT, acts, &mut picked);
                    ui.add_space(1.0);
                    ui.label(
                        egui::RichText::new(crate::tl_id!("ui.every_fifth_line_is_slightly_stronger_for_easier_counting")).small().color(t.muted),
                    );
                    ui.separator();
                    picked |= page_colour_palette(ui, view, t, acts);
                    ui.separator();
                    picked |= rule_colour_palette(ui, view, t, acts);
                });
            })
            .response;
        let clicked_away =
            ui.input(|i| i.pointer.any_click()) && !area.rect.contains(ui.ctx().pointer_latest_pos().unwrap_or_default()) && !resp.clicked();
        if picked || clicked_away || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            open = false;
        }
    }
    ui.data_mut(|d| d.insert_temp(open_id, open));
}

const DRAW_COLOURS: [(Rgb, &str); 8] = [
    (Rgb(0x20, 0x20, 0x20), "Noir"),
    (Rgb(0xd0, 0x21, 0x21), "Rouge"),
    (Rgb(0x18, 0x55, 0xc4), "Bleu"),
    (Rgb(0x16, 0x7f, 0x3d), "Vert"),
    (Rgb(0xe6, 0x77, 0x00), "Orange"),
    (Rgb(0x70, 0x48, 0xe8), "Violet"),
    (Rgb(0xf5, 0x9f, 0x00), "Jaune"),
    (Rgb(0xd6, 0x33, 0x6c), "Rose"),
];

#[derive(Clone, Copy)]
enum PillGlyph {
    Icon(Icon),
    Eraser,
    ColourDot(Option<ergonote_model::NodeColor>),
}

#[allow(clippy::too_many_arguments)]
fn ribbon_selector_pill(
    ui: &mut Ui,
    width: f32,
    height: f32,
    glyph: PillGlyph,
    dot: Option<Color32>,
    label: &str,
    dropdown: bool,
    on: bool,
    tip: &str,
    t: &Tokens,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    let enabled = ui.is_enabled();
    let painter = ui.painter_at(rect);
    let fill = if on {
        t.row_selected
    } else if resp.hovered() && enabled {
        t.row_hover
    } else {
        t.editor
    };
    let edge = if on { t.accent } else { t.faint };
    let radius = if height >= 30.0 { 7.0 } else { 6.0 };
    painter.rect_filled(rect, radius, fill);
    painter.rect_stroke(rect, radius, egui::Stroke::new(if on { 1.4 } else { 1.0 }, edge), egui::StrokeKind::Inside);

    let g_side = if height >= 30.0 { 17.0 } else { 14.0 };
    let g_cx = if height >= 30.0 { rect.min.x + 15.0 } else { rect.min.x + 12.0 };
    let g_rect = Rect::from_center_size(egui::pos2(g_cx, rect.center().y), vec2(g_side, g_side));
    let icon_col = if enabled { t.accent } else { t.muted };
    match glyph {
        PillGlyph::Icon(icon) => icons::paint_ribbon(ui, g_rect, icon, icon_col),
        PillGlyph::Eraser => {
            let er = Rect::from_center_size(g_rect.center(), vec2(13.0, 9.5));
            let split = er.min.x + er.width() * 0.48;
            painter.rect_filled(Rect::from_min_max(er.min, egui::pos2(split, er.max.y)), 1.8, Color32::from_rgb(0xdf, 0xe1, 0xe5));
            painter.rect_filled(Rect::from_min_max(egui::pos2(split - 1.0, er.min.y), er.max), 1.8, Color32::from_rgb(0xe4, 0xc4, 0xee));
            painter.rect_stroke(er, 1.8, egui::Stroke::new(1.0, Color32::from_rgb(0x8f, 0x94, 0x9d)), egui::StrokeKind::Inside);
        }
        PillGlyph::ColourDot(page_colour) => {
            let center = g_rect.center();
            if let Some(c) = page_colour {
                let dark = t.editor.r() < 128;
                let (r, g, b) = c.rgb(dark);
                painter.circle_filled(center, 6.5, Color32::from_rgb(r, g, b));
                painter.circle_stroke(center, 6.5, egui::Stroke::new(1.0, t.container_border));
            } else {
                painter.circle_filled(center + vec2(-3.3, -3.0), 3.1, Color32::from_rgb(239, 68, 68));
                painter.circle_filled(center + vec2(3.3, -3.0), 3.1, Color32::from_rgb(245, 158, 11));
                painter.circle_filled(center + vec2(-3.3, 3.0), 3.1, Color32::from_rgb(34, 197, 94));
                painter.circle_filled(center + vec2(3.3, 3.0), 3.1, Color32::from_rgb(59, 130, 246));
            }
        }
    }

    let mut text_x = if height >= 30.0 { rect.min.x + 28.0 } else { rect.min.x + 23.0 };
    if let Some(dot_col) = dot {
        let dot_c = egui::pos2(text_x + 4.0, rect.center().y);
        painter.circle_filled(dot_c, 4.2, dot_col);
        painter.circle_stroke(dot_c, 4.2, egui::Stroke::new(0.8, t.text.gamma_multiply(0.45)));
        text_x += 13.0;
    }
    let text_col = if enabled { t.text } else { t.muted };
    let font_sz = if height >= 30.0 { 12.5 } else { 12.0 };
    painter.text(egui::pos2(text_x, rect.center().y), Align2::LEFT_CENTER, label, egui::FontId::proportional(font_sz), text_col);
    if dropdown {
        chevron(&painter, egui::pos2(rect.max.x - 10.0, rect.center().y), if (resp.hovered() && enabled) || on { t.accent } else { t.muted });
    }
    let name = label.to_string();
    resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, on, name.clone()));
    resp.on_hover_text(tip)
}

fn ribbon_colour_swatches(ui: &mut Ui, view: &View<'_>, active_nib: Nib, activate_draw: bool, t: &Tokens, acts: &mut Vec<Act>) -> bool {
    let mut picked = false;
    for chunk in DRAW_COLOURS.chunks(4) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            for (rgb, name) in chunk {
                let here = active_nib.colour == *rgb;
                let (cell, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
                let painter = ui.painter_at(cell);
                let c32 = Color32::from_rgb(rgb.0, rgb.1, rgb.2);
                painter.circle_filled(cell.center(), 8.5, c32);
                if here {
                    painter.circle_stroke(cell.center(), 10.5, egui::Stroke::new(2.0, t.accent));
                } else if resp.hovered() {
                    painter.circle_stroke(cell.center(), 10.5, egui::Stroke::new(1.5, t.muted));
                }
                if resp.on_hover_text(*name).clicked() {
                    acts.push(Act::InkRack(RackEdit::Set(view.nib, Nib::new(active_nib.tool, *rgb, active_nib.tenths))));
                    if activate_draw {
                        acts.push(Act::InkPointer(Pointer::Draw));
                    }
                    picked = true;
                }
            }
        });
    }
    picked
}

fn ribbon_thickness_pills(ui: &mut Ui, view: &View<'_>, active_nib: Nib, activate_draw: bool, t: &Tokens, acts: &mut Vec<Act>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 0.0);
        for (label, tenths) in [("Fin", 15u16), ("Moyen", 30u16), ("Épais", 55u16)] {
            let on = (i32::from(active_nib.tenths) - i32::from(tenths)).abs() <= 8;
            let btn = egui::Button::new(egui::RichText::new(label).size(11.5).color(if on { t.accent } else { t.text }))
                .fill(if on { t.row_selected } else { t.editor })
                .stroke(egui::Stroke::new(1.0, if on { t.accent } else { t.faint }))
                .corner_radius(4.0)
                .min_size(vec2(52.0, 22.0));
            if ui.add(btn).clicked() {
                acts.push(Act::InkRack(RackEdit::Set(view.nib, Nib::new(active_nib.tool, active_nib.colour, tenths))));
                if activate_draw {
                    acts.push(Act::InkPointer(Pointer::Draw));
                }
            }
        }
    });
}

fn draw_pen_menu(ui: &mut Ui, view: &View<'_>, active_nib: Nib, active_col: Color32, t: &Tokens, acts: &mut Vec<Act>) {
    let drawing = view.pointer == Pointer::Draw;
    let response = ribbon_selector_pill(
        ui,
        104.0,
        33.0,
        PillGlyph::Icon(Icon::Pen),
        Some(active_col),
        "Stylo",
        true,
        drawing,
        "Dessiner : choisir le type de stylo, la couleur et l'épaisseur",
        t,
    );
    egui::Popup::menu(&response).show(|ui| {
        egui::Frame::popup(ui.style()).corner_radius(8.0).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
            ui.set_min_width(184.0);
            ui.spacing_mut().item_spacing = vec2(4.0, 5.0);
            ui.label(egui::RichText::new(crate::tl_id!("ui.student_tool")).small().color(t.muted));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                for (tool, lbl) in [(Tool::Pen, "Stylo"), (Tool::Highlighter, "Surligneur"), (Tool::Pencil, "Crayon")] {
                    let on = drawing && active_nib.tool == tool;
                    let btn = egui::Button::new(egui::RichText::new(lbl).size(11.5).color(if on { t.accent } else { t.text }))
                        .fill(if on { t.row_selected } else { t.editor })
                        .stroke(egui::Stroke::new(1.0, if on { t.accent } else { t.faint }))
                        .corner_radius(4.0)
                        .min_size(vec2(54.0, 24.0));
                    if ui.add(btn).clicked() {
                        acts.push(Act::InkRack(RackEdit::Set(view.nib, Nib::new(tool, active_nib.colour, active_nib.tenths))));
                        acts.push(Act::InkPointer(Pointer::Draw));
                    }
                }
            });
            ui.label(egui::RichText::new(crate::tl_id!("ui.colour")).small().color(t.muted));
            ribbon_colour_swatches(ui, view, active_nib, true, t, acts);
            ui.label(egui::RichText::new(crate::tl_id!("ui.thickness")).small().color(t.muted));
            ribbon_thickness_pills(ui, view, active_nib, true, t, acts);
        });
    });
}

fn draw_shapes_menu(ui: &mut Ui, view: &View<'_>, active_nib: Nib, active_col: Color32, t: &Tokens, acts: &mut Vec<Act>) {
    use ergonote_ink::shapes::Shape;
    let in_shapes = matches!(view.pointer, Pointer::Shape(_));
    let response = ribbon_selector_pill(
        ui,
        104.0,
        33.0,
        PillGlyph::Icon(Icon::Shapes),
        Some(active_col),
        "Formes",
        true,
        in_shapes,
        "Tracer une forme géométrique avec la couleur choisie",
        t,
    );
    egui::Popup::menu(&response).show(|ui| {
        egui::Frame::popup(ui.style()).corner_radius(8.0).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            ui.label(egui::RichText::new(crate::tl_id!("ui.student_shapes")).small().color(t.muted));
            let in_hand = |s: Shape| view.pointer == Pointer::Shape(s);
            let rows_of_shapes = [
                [Shape::Rectangle, Shape::Square, Shape::Line],
                [Shape::Circle, Shape::Ellipse, Shape::Arrow],
                [Shape::Triangle, Shape::Diamond, Shape::Star],
            ];
            for row in rows_of_shapes {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                    for s in row {
                        if shape_button(ui, s, in_hand(s), active_col, t).clicked() {
                            acts.push(Act::InkPointer(Pointer::Shape(s)));
                            ui.close();
                        }
                    }
                });
            }
            ui.separator();
            ui.label(egui::RichText::new(crate::tl_id!("ui.student_shape_color_label")).small().color(t.muted));
            ribbon_colour_swatches(ui, view, active_nib, false, t, acts);
            ui.label(egui::RichText::new(crate::tl_id!("ui.thickness")).small().color(t.muted));
            ribbon_thickness_pills(ui, view, active_nib, false, t, acts);
        });
    });
}

fn insert_draw_controls(ui: &mut Ui, view: &View<'_>, t: &Tokens, acts: &mut Vec<Act>) {
    let active_nib = view.in_hand();
    let active_col = Color32::from_rgb(active_nib.colour.0, active_nib.colour.1, active_nib.colour.2);
    ui.vertical(|ui| {
        ui.add_space(6.0);
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            draw_pen_menu(ui, view, active_nib, active_col, t, acts);
            let erase_on = view.pointer == Pointer::Erase;
            if ribbon_selector_pill(
                ui,
                88.0,
                33.0,
                PillGlyph::Eraser,
                None,
                "Gomme",
                false,
                erase_on,
                "Gommer un tracé (re-cliquer pour revenir à l'écriture)",
                t,
            )
            .clicked()
            {
                acts.push(Act::InkPointer(if erase_on { Pointer::Write } else { Pointer::Erase }));
            }
        });
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            draw_shapes_menu(ui, view, active_nib, active_col, t, acts);
            if ribbon_selector_pill(ui, 88.0, 33.0, PillGlyph::Icon(Icon::Undo), None, "Annuler", false, false, "Annuler le dernier tracé", t)
                .clicked()
            {
                acts.push(Act::InkUndo);
            }
        });
    });
}

fn sticker_tile(
    ui: &mut Ui,
    sticker: &'static crate::stickers::Sticker,
    tile: egui::Vec2,
    icon_sz: egui::Vec2,
    show_label: bool,
    t: &Tokens,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(tile, Sense::click());
    let painter = ui.painter_at(rect);
    let bg = if resp.hovered() { t.row_hover } else { t.editor };
    let edge = if resp.hovered() { t.accent } else { t.faint };
    painter.rect_filled(rect, 6.0, bg);
    painter.rect_stroke(rect, 6.0, egui::Stroke::new(1.0, edge), egui::StrokeKind::Inside);
    let img_center_y = if show_label { rect.min.y + 24.0 } else { rect.center().y };
    let img_rect = Rect::from_center_size(egui::pos2(rect.center().x, img_center_y), icon_sz);
    egui::Image::from_bytes(crate::stickers::preview_uri(sticker), sticker.svg.as_bytes()).paint_at(ui, img_rect);
    if show_label {
        painter.text(egui::pos2(rect.center().x, rect.max.y - 10.0), Align2::CENTER_CENTER, sticker.label, egui::FontId::proportional(10.0), t.text);
    }
    let enabled = ui.is_enabled();
    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, sticker.label));
    resp.on_hover_text(sticker.label).clicked()
}

fn sticker_grid_popup(ui: &mut Ui, items: &'static [crate::stickers::Sticker], cols: usize, t: &Tokens, acts: &mut Vec<Act>) {
    ui.spacing_mut().item_spacing = vec2(5.0, 5.0);
    for chunk in items.chunks(cols.max(1)) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(5.0, 5.0);
            for s in chunk {
                if sticker_tile(ui, s, vec2(44.0, 44.0), vec2(30.0, 30.0), false, t) {
                    acts.push(Act::InsertSticker(s.id));
                    ui.close();
                }
            }
        });
    }
}

fn big_sticker_menu_button(ui: &mut Ui, is_labo: bool, label: &str, tip: &str, t: &Tokens) -> egui::Response {
    let font = egui::FontId::proportional(13.0);
    let widest = ui.painter().layout_no_wrap(label.to_owned(), font.clone(), t.text).size().x;
    let (rect, resp) = ui.allocate_exact_size(vec2(BIG.max(widest + BIG_PAD), GROUP_H - 20.0), Sense::click());
    let painter = ui.painter_at(rect);
    if resp.hovered() {
        painter.rect_filled(rect, 8.0, t.row_hover);
    }
    let tint = if is_labo { Color32::from_rgb(0x25, 0x63, 0xEB) } else { Color32::from_rgb(0xD9, 0x77, 0x06) };
    let badge = Rect::from_center_size(egui::pos2(rect.center().x, rect.min.y + ICON_Y), vec2(38.0, 34.0));
    painter.rect_filled(badge, 9.0, crate::theme::mix(t.editor, tint, 0.14));
    let c = badge.center();
    if is_labo {
        // Colorful 2D Erlenmeyer flask vector icon.
        let top = c.y - 10.5;
        let neck = c.y - 3.0;
        let liq_top = c.y + 1.5;
        let bot = c.y + 10.5;
        let outline = vec![
            egui::pos2(c.x - 3.2, top),
            egui::pos2(c.x + 3.2, top),
            egui::pos2(c.x + 3.2, neck),
            egui::pos2(c.x + 10.0, bot),
            egui::pos2(c.x - 10.0, bot),
            egui::pos2(c.x - 3.2, neck),
        ];
        let liquid =
            vec![egui::pos2(c.x - 5.6, liq_top), egui::pos2(c.x + 5.6, liq_top), egui::pos2(c.x + 9.2, bot - 0.8), egui::pos2(c.x - 9.2, bot - 0.8)];
        painter.add(egui::Shape::convex_polygon(outline.clone(), Color32::from_rgba_unmultiplied(0xE0, 0xF2, 0xFE, 210), egui::Stroke::NONE));
        painter.add(egui::Shape::convex_polygon(liquid, Color32::from_rgb(0x38, 0xBD, 0xF8), egui::Stroke::NONE));
        painter.add(egui::Shape::closed_line(outline, egui::Stroke::new(1.5, Color32::from_rgb(0x1E, 0x3A, 0x8A))));
        painter.line_segment([egui::pos2(c.x - 4.5, top), egui::pos2(c.x + 4.5, top)], egui::Stroke::new(1.8, Color32::from_rgb(0x1E, 0x3A, 0x8A)));
        painter.circle_filled(egui::pos2(c.x - 2.2, c.y + 6.2), 1.4, Color32::from_rgb(0xFF, 0xFF, 0xFF));
        painter.circle_filled(egui::pos2(c.x + 2.4, c.y + 4.2), 1.1, Color32::from_rgb(0xFF, 0xFF, 0xFF));
    } else {
        // Warm golden vector smiley icon.
        painter.circle_filled(c, 11.5, Color32::from_rgb(0xFF, 0xD9, 0x3B));
        painter.circle_stroke(c, 11.5, egui::Stroke::new(1.4, Color32::from_rgb(0xB4, 0x53, 0x09)));
        painter.circle_filled(egui::pos2(c.x - 5.5, c.y + 1.5), 2.0, Color32::from_rgba_unmultiplied(0xF9, 0x73, 0x16, 120));
        painter.circle_filled(egui::pos2(c.x + 5.5, c.y + 1.5), 2.0, Color32::from_rgba_unmultiplied(0xF9, 0x73, 0x16, 120));
        painter.circle_filled(egui::pos2(c.x - 3.8, c.y - 2.5), 1.6, Color32::from_rgb(0x3B, 0x2B, 0x09));
        painter.circle_filled(egui::pos2(c.x + 3.8, c.y - 2.5), 1.6, Color32::from_rgb(0x3B, 0x2B, 0x09));
        let smile: Vec<egui::Pos2> = (0..=8)
            .map(|i| {
                let frac = (i as f32) / 8.0;
                let ang = 0.22 * std::f32::consts::PI + frac * 0.56 * std::f32::consts::PI;
                egui::pos2(c.x - 5.8 * ang.cos(), c.y + 0.5 + 5.8 * ang.sin())
            })
            .collect();
        painter.add(egui::Shape::line(smile, egui::Stroke::new(1.7, Color32::from_rgb(0x3B, 0x2B, 0x09))));
    }
    painter.text(egui::pos2(rect.center().x, rect.min.y + LABEL_Y), Align2::CENTER_CENTER, label, font, t.text);
    chevron(&painter, egui::pos2(rect.center().x, rect.min.y + LABEL_Y + LINE_H - 1.0), t.muted);
    let enabled = ui.is_enabled();
    let name = label.to_owned();
    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name.clone()));
    resp.on_hover_text(tip)
}

fn insert_stickers_controls(ui: &mut Ui, t: &Tokens, acts: &mut Vec<Act>) {
    use crate::stickers::{EMOJIS, LABO, LabGroup, SMILEYS, Season, current_season, seasonal_emojis, seasonal_smileys};
    let season = current_season();
    let season_col = match season {
        Season::Printemps => Color32::from_rgb(0xEC, 0x48, 0x99),
        Season::Ete => Color32::from_rgb(0xF5, 0x9E, 0x0B),
        Season::Automne => Color32::from_rgb(0xEA, 0x58, 0x0C),
        Season::Hiver => Color32::from_rgb(0x0E, 0xA5, 0xE9),
    };
    let emojis_btn = big_sticker_menu_button(ui, false, "Émojis", "Insérer un smiley ou un émoji scolaire", t);
    egui::Popup::menu(&emojis_btn).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        egui::Frame::popup(ui.style()).corner_radius(8.0).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
            let tab_id = egui::Id::new("ribbon-emoji-tab");
            let mut active_tab = ui.data(|d| d.get_temp::<u8>(tab_id).unwrap_or(0));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
                for (idx, title) in [(0u8, "Smileys"), (1u8, "Émojis")] {
                    let on = active_tab == idx;
                    let btn = egui::Button::new(egui::RichText::new(title).size(12.0).color(if on { t.accent } else { t.text }))
                        .fill(if on { t.row_selected } else { t.editor })
                        .stroke(egui::Stroke::new(1.0, if on { t.accent } else { t.faint }))
                        .corner_radius(5.0)
                        .min_size(vec2(138.0, 26.0));
                    if ui.add(btn).clicked() {
                        active_tab = idx;
                    }
                }
            });
            ui.data_mut(|d| d.insert_temp(tab_id, active_tab));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                let (dot_r, _) = ui.allocate_exact_size(vec2(10.0, 14.0), Sense::hover());
                ui.painter().circle_filled(dot_r.center(), 4.0, season_col);
                ui.label(egui::RichText::new(format!("Saison : {}", season.label())).small().strong().color(season_col));
            });
            let seasonal = if active_tab == 0 { seasonal_smileys(season) } else { seasonal_emojis(season) };
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(5.0, 5.0);
                for s in seasonal {
                    if sticker_tile(ui, s, vec2(44.0, 44.0), vec2(30.0, 30.0), false, t) {
                        acts.push(Act::InsertSticker(s.id));
                        ui.close();
                    }
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().id_salt(("ribbon-sticker-scroll", active_tab)).max_height(250.0).show(ui, |ui| {
                sticker_grid_popup(ui, if active_tab == 0 { SMILEYS } else { EMOJIS }, 6, t, acts);
            });
        });
    });

    let labo_btn = big_sticker_menu_button(ui, true, "Labo", "Insérer du matériel de chimie (bécher, pipette, fiole...) ou un pictogramme", t);
    egui::Popup::menu(&labo_btn).show(|ui| {
        egui::Frame::popup(ui.style()).corner_radius(8.0).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            for (heading, grp) in [("Verrerie", LabGroup::Verrerie), ("Mesure & chauffage", LabGroup::Mesure), ("Sécurité (SGH)", LabGroup::Securite)]
            {
                ui.label(egui::RichText::new(heading).small().color(t.muted));
                let list: Vec<&'static crate::stickers::Sticker> = LABO.iter().filter(|s| s.lab_group == Some(grp)).collect();
                for chunk in list.chunks(4) {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                        for s in chunk {
                            if sticker_tile(ui, s, vec2(74.0, 62.0), vec2(34.0, 34.0), true, t) {
                                acts.push(Act::InsertSticker(s.id));
                                ui.close();
                            }
                        }
                    });
                }
            }
        });
    });
}

/// One shape in the Shapes gallery: the shape itself, drawn small, with its name beside it.
fn shape_button(ui: &mut Ui, shape: ergonote_ink::shapes::Shape, here: bool, stroke_col: Color32, t: &Tokens) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(86.0, 20.0), Sense::click());
    if here || resp.hovered() {
        ui.painter().rect_filled(rect, 3.0, if here { t.row_selected } else { t.row_hover });
    }
    let box_ = egui::Rect::from_min_size(egui::pos2(rect.min.x + 4.0, rect.min.y + 4.0), vec2(12.0, 12.0));
    let dabs = ergonote_ink::shapes::trace(shape, (box_.min.x, box_.min.y), (box_.max.x, box_.max.y), 1.0);
    let points: Vec<egui::Pos2> = dabs.iter().map(|d| egui::pos2(d.x, d.y)).collect();
    if points.len() >= 2 {
        ui.painter().add(egui::Shape::line(points, egui::Stroke::new(1.6, stroke_col)));
    }
    let at = egui::pos2(rect.min.x + 24.0, rect.center().y);
    let shape_label = crate::tl!(shape.label());
    ui.painter().text(at, egui::Align2::LEFT_CENTER, shape_label, egui::FontId::proportional(11.5), t.text);
    let enabled = ui.is_enabled();
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, here, shape_label));
    resp.on_hover_text(crate::i18n::format_message_id("ui.shape_drag_on_the_page_to_draw_one", &[(crate::tl_id!("ui.shape"), shape_label)]))
}

/// Fixed-width pill button so stacked Table ribbon rows align to the exact pixel.
fn table_grid_btn(ui: &mut Ui, width: f32, icon: Option<Icon>, mark: Option<Mark>, label: &str, tip: &str, on: bool, t: &Tokens) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, SMALL_H), Sense::click());
    let painter = ui.painter_at(rect);
    if on {
        painter.rect_filled(rect, 6.0, t.row_selected);
        painter.rect_stroke(rect, 6.0, egui::Stroke::new(1.0, t.accent), egui::StrokeKind::Inside);
    } else if resp.hovered() {
        painter.rect_filled(rect, 6.0, t.row_hover);
        painter.rect_stroke(rect, 6.0, egui::Stroke::new(1.0, t.faint), egui::StrokeKind::Inside);
    } else {
        painter.rect_filled(rect, 6.0, t.editor);
        painter.rect_stroke(rect, 6.0, egui::Stroke::new(1.0, t.faint), egui::StrokeKind::Inside);
    }
    let glyph_box = Rect::from_center_size(egui::pos2(rect.min.x + 13.0, rect.center().y), vec2(16.0, 16.0));
    if let Some(icon) = icon {
        icons::paint_ribbon(ui, glyph_box, icon, t.accent);
    } else if let Some(mark) = mark {
        let col = mark.color(t.text, t.accent);
        ribbon_icons::paint(&painter, glyph_box, mark, col);
    }
    painter.text(egui::pos2(rect.min.x + 25.0, rect.center().y), Align2::LEFT_CENTER, label, egui::FontId::proportional(12.5), t.text);
    let enabled = ui.is_enabled();
    let name = label.to_owned();
    resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, on, name.clone()));
    resp.on_hover_text(tip)
}

/// Crisp 3-line alignment glyph button (`Left`, `Center`, `Right`), 30 px wide.
fn table_align_btn(ui: &mut Ui, align: ergonote_model::tablecols::Align, on: bool, t: &Tokens) -> egui::Response {
    use ergonote_model::tablecols::Align as A;
    let (rect, resp) = ui.allocate_exact_size(vec2(30.0, SMALL_H), Sense::click());
    let painter = ui.painter_at(rect);
    if on {
        painter.rect_filled(rect, 6.0, t.row_selected);
        painter.rect_stroke(rect, 6.0, egui::Stroke::new(1.0, t.accent), egui::StrokeKind::Inside);
    } else if resp.hovered() {
        painter.rect_filled(rect, 6.0, t.row_hover);
        painter.rect_stroke(rect, 6.0, egui::Stroke::new(1.0, t.faint), egui::StrokeKind::Inside);
    } else {
        painter.rect_filled(rect, 6.0, t.editor);
        painter.rect_stroke(rect, 6.0, egui::Stroke::new(1.0, t.faint), egui::StrokeKind::Inside);
    }
    let stroke = egui::Stroke::new(1.6, if on { t.accent } else { t.text });
    let cx = rect.center().x;
    let cy = rect.center().y;
    let span = 14.0;
    let left_edge = cx - span * 0.5;
    let right_edge = cx + span * 0.5;
    for (dy, len) in [(-4.0_f32, 14.0_f32), (0.0, 9.0), (4.0, 12.0)] {
        let x0 = match align {
            A::Left => left_edge,
            A::Center => cx - len * 0.5,
            A::Right => right_edge - len,
        };
        painter.line_segment([egui::pos2(x0, cy + dy), egui::pos2(x0 + len, cy + dy)], stroke);
    }
    let label = match align {
        A::Left => "Aligner à gauche",
        A::Center => "Centrer",
        A::Right => "Aligner à droite",
    };
    let enabled = ui.is_enabled();
    resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, on, label.to_owned()));
    resp.on_hover_text(label)
}

/// Tall compact button matching `big_mark_compact` for deleting a selected table or image.
fn compact_delete_btn(ui: &mut Ui, tip: &str, widget_label: &str, t: &Tokens) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(68.0, GROUP_H - 20.0), Sense::click());
    let painter = ui.painter_at(rect);
    if resp.hovered() {
        painter.rect_filled(rect, 8.0, t.row_hover);
    }
    let col = Mark::Delete.color(t.text, t.accent);
    let badge = Rect::from_center_size(egui::pos2(rect.center().x, rect.min.y + ICON_Y), vec2(32.0, 30.0));
    painter.rect_filled(badge, 8.0, crate::theme::mix(t.editor, col, 0.14));
    ribbon_icons::paint(&painter, Rect::from_center_size(egui::pos2(rect.center().x, rect.min.y + ICON_Y), vec2(20.0, 20.0)), Mark::Delete, col);
    painter.text(egui::pos2(rect.center().x, rect.min.y + LABEL_Y), Align2::CENTER_CENTER, "Supprimer", egui::FontId::proportional(12.0), t.text);
    let enabled = ui.is_enabled();
    let label = widget_label.to_owned();
    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label.clone()));
    resp.on_hover_text(tip)
}

/// Compact, aligned Table tab shown while the caret is inside a table: row/column edits,
/// appearance (cell shading, borders, alignment) and whole-table deletion with confirmation.
fn table_tab(ui: &mut Ui, view: &View<'_>, t: &Tokens, acts: &mut Vec<Act>) {
    use crate::pageedit::TableEdit as E;
    use ergonote_model::tablecols::Align as A;
    group(ui, crate::tl_id!("ui.student_rows_and_cols_group"), t, |ui| {
        ui.vertical(|ui| {
            ui.add_space(6.0);
            ui.spacing_mut().item_spacing = vec2(5.0, 6.0);
            ui.horizontal(|ui| {
                if table_grid_btn(ui, 90.0, Some(Icon::TableRow), None, "+ Haut", "Insérer une ligne au-dessus", false, t).clicked() {
                    acts.push(Act::Table(E::InsertRow(true)));
                }
                if table_grid_btn(ui, 90.0, Some(Icon::TableRow), None, "+ Bas", "Insérer une ligne en dessous", false, t).clicked() {
                    acts.push(Act::Table(E::InsertRow(false)));
                }
                if table_grid_btn(ui, 90.0, None, Some(Mark::Delete), "Ligne", "Supprimer la ligne", false, t).clicked() {
                    acts.push(Act::Table(E::DeleteRow));
                }
            });
            ui.horizontal(|ui| {
                if table_grid_btn(ui, 90.0, Some(Icon::TableColumn), None, "+ Gauche", "Insérer une colonne à gauche", false, t).clicked() {
                    acts.push(Act::Table(E::InsertColumn(true)));
                }
                if table_grid_btn(ui, 90.0, Some(Icon::TableColumn), None, "+ Droite", "Insérer une colonne à droite", false, t).clicked() {
                    acts.push(Act::Table(E::InsertColumn(false)));
                }
                if table_grid_btn(ui, 90.0, None, Some(Mark::Delete), "Colonne", "Supprimer la colonne", false, t).clicked() {
                    acts.push(Act::Table(E::DeleteColumn));
                }
            });
        });
    });
    group(ui, crate::tl_id!("ui.student_appearance_group"), t, |ui| {
        shading_menu(ui, view, t, acts);
        ui.vertical(|ui| {
            ui.add_space(6.0);
            ui.spacing_mut().item_spacing = vec2(5.0, 6.0);
            let hiding = view.table_borders;
            let tip = if hiding { "Masquer les bordures du tableau" } else { "Afficher les bordures du tableau" };
            if table_grid_btn(ui, 100.0, Some(Icon::Table), None, "Bordures", tip, hiding, t).clicked() {
                acts.push(Act::TableBorders(!hiding));
            }
            ui.horizontal(|ui| {
                for align in [A::Left, A::Center, A::Right] {
                    if table_align_btn(ui, align, view.table_align == Some(align), t).clicked() {
                        acts.push(Act::TableAlign(align));
                    }
                }
            });
        });
    });
    group(ui, crate::tl_id!("ui.student_table_group"), t, |ui| {
        if compact_delete_btn(ui, "Supprimer tout le tableau", "Supprimer le tableau", t).clicked() {
            acts.push(Act::ConfirmDeleteTable(None));
        }
    });
}

/// Contextual Image / PDF tab matching the compact 3-group layout of the Table tab:
/// crop/rotate/scan enhancement, pure-Rust offline OCR, background lock, and deletion.
fn image_tab(ui: &mut Ui, view: &View<'_>, t: &Tokens, acts: &mut Vec<Act>) {
    use crate::turn::{Enhance, Turn};
    group(ui, crate::tl_id!("ui.student_edit_group"), t, |ui| {
        ui.vertical(|ui| {
            ui.add_space(6.0);
            ui.spacing_mut().item_spacing = vec2(5.0, 6.0);
            ui.horizontal(|ui| {
                if view.image_cropping {
                    if table_grid_btn(ui, 108.0, Some(Icon::Picture), None, "\u{2713} Valider", "Appliquer le rognage", true, t).clicked() {
                        acts.push(Act::ApplyCrop);
                    }
                    if table_grid_btn(ui, 96.0, None, None, "R\u{00E9}initialiser", "Revenir au cadrage complet", false, t).clicked() {
                        acts.push(Act::ResetCrop);
                    }
                    if table_grid_btn(ui, 90.0, None, None, "\u{2715} Annuler", "Annuler le rognage (Échap)", false, t).clicked() {
                        acts.push(Act::CancelCrop);
                    }
                } else {
                    if table_grid_btn(ui, 96.0, Some(Icon::Picture), None, "Rogner", "Rogner l'image directement sur la page", false, t).clicked() {
                        acts.push(Act::CropPicture);
                    }
                    if table_grid_btn(ui, 96.0, Some(Icon::Redo), None, "Pivoter \u{21BB}", "Pivoter de 90° à droite", false, t).clicked() {
                        acts.push(Act::TurnSelectedPicture(Turn::Right));
                    }
                    if table_grid_btn(ui, 96.0, Some(Icon::Undo), None, "Pivoter \u{21BA}", "Pivoter de 90° à gauche", false, t).clicked() {
                        acts.push(Act::TurnSelectedPicture(Turn::Left));
                    }
                }
            });
            ui.horizontal(|ui| {
                if table_grid_btn(ui, 96.0, Some(Icon::Page), None, "Scan N&B", "Améliorer un scan ou une photocopie en noir et blanc", false, t)
                    .clicked()
                {
                    acts.push(Act::EnhancePicture(Enhance::ScanBw));
                }
                if table_grid_btn(ui, 96.0, None, Some(Mark::Highlighter), "Contraste +", "Renforcer le contraste de l'image", false, t).clicked() {
                    acts.push(Act::EnhancePicture(Enhance::Contrast));
                }
                if table_grid_btn(ui, 96.0, Some(Icon::Redo), None, "Miroir ⇄", "Retourner horizontalement", false, t).clicked() {
                    acts.push(Act::TurnSelectedPicture(Turn::FlipHorizontal));
                }
            });
        });
    });
    group(ui, crate::tl_id!("ui.student_ocr_group"), t, |ui| {
        let ocr_label = if view.ocr_busy { "OCR en\ncours..." } else { "Extraire\nle texte" };
        if big(ui, Icon::InkToText, ocr_label, crate::tl_id!("ui.student_ocr_extract_tip"), t).clicked() && !view.ocr_busy {
            acts.push(Act::OcrPicture { speak: false });
        }
        if big(ui, Icon::ReadAloud, crate::tl_id!("ui.student_ocr_read"), crate::tl_id!("ui.student_ocr_read_tip"), t).clicked() && !view.ocr_busy {
            acts.push(Act::OcrPicture { speak: true });
        }
    });
    group(ui, crate::tl_id!("ui.student_image_group"), t, |ui| {
        let lock_label = if view.lock_images { "Déverrouiller\nle fond" } else { "Verrouiller\nen fond" };
        let lock_tip = "Verrouiller les PDF et images en fond pour écrire dessus sans les déplacer par erreur";
        if big_on(ui, Icon::Page, lock_label, lock_tip, view.lock_images, t).clicked() {
            acts.push(Act::ToggleLockImages);
        }
        if compact_delete_btn(ui, "Supprimer l'image sélectionnée", "Supprimer l'image", t).clicked() {
            acts.push(Act::DeleteSelectedPicture);
        }
    });
}

/// Page setup and compact view controls placed at the right end of Home.
fn page_setup_group(ui: &mut Ui, view: &View<'_>, t: &Tokens, acts: &mut Vec<Act>) {
    group(ui, crate::tl_id!("ui.page_setup"), t, |ui| {
        if big_on(ui, Icon::Folder, crate::tl_id!("ui.student_section_tabs"), crate::tl_id!("ui.student_section_tabs_tip"), view.section_tabs_bar, t)
            .clicked()
        {
            acts.push(Act::ToggleSectionTabs);
        }
        ui.vertical(|ui| {
            ui.add_space(6.0);
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                compact_rule_lines_button(ui, view, t, acts);
                ui.add_enabled_ui(view.has_page, |ui| {
                    let colour_resp = ribbon_selector_pill(
                        ui,
                        98.0,
                        33.0,
                        PillGlyph::ColourDot(view.page_colour),
                        None,
                        "Couleur",
                        true,
                        false,
                        crate::tl_id!("ui.page_colour"),
                        t,
                    );
                    egui::Popup::menu(&colour_resp).show(|ui| {
                        if page_colour_palette(ui, view, t, acts) {
                            ui.close();
                        }
                    });
                });
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.add_enabled_ui(view.has_page, |ui| {
                    let paper_resp = ribbon_selector_pill(
                        ui,
                        98.0,
                        33.0,
                        PillGlyph::Icon(Icon::Page),
                        None,
                        "Papier",
                        true,
                        false,
                        crate::tl_id!("ui.the_sheet_this_page_is_written_for_its_size_turn_and_margins"),
                        t,
                    );
                    egui::Popup::menu(&paper_resp)
                        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                        .show(|ui| paper_size_menu(ui, view.paper_size, t, acts));
                });
                if ribbon_selector_pill(
                    ui,
                    98.0,
                    33.0,
                    PillGlyph::Icon(Icon::Tiles),
                    None,
                    "Simple",
                    false,
                    view.simple_mode,
                    "Interface ultra simple en tuiles (idéale pour les enfants ou les petits écrans)",
                    t,
                )
                .clicked()
                {
                    acts.push(Act::ToggleSimpleMode);
                }
            });
        });
    });
}

fn source_tab(ui: &mut Ui, format: Option<PageFormat>, t: &Tokens, acts: &mut Vec<Act>) {
    group(ui, crate::tl_id!("ui.source"), t, |ui| {
        if big(ui, Icon::Code, crate::tl_id!("ui.hide"), crate::tl_id!("ui.close_the_source_pane"), t).clicked() {
            acts.push(Act::ToggleSource);
        }
    });
    group(ui, crate::tl_id!("ui.this_page"), t, |ui| {
        let name = format.map(PageFormat::label).unwrap_or("No page");
        ui.vertical(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(name).size(12.0).color(t.text));
            ui.label(egui::RichText::new(crate::tl_id!("ui.the_format_is_set_when_the_page_is_created")).size(10.0).color(t.muted));
        });
    });
}

#[cfg(test)]
mod grid_line_tests {
    use super::first_grid_line;

    /// Issue #79: the paper's lines go through the anchor and every step from it, whichever side
    /// of the viewport's edge the anchor is on, so they move with the page as it pans.
    #[test]
    fn grid_lines_go_through_the_anchor_and_move_with_it() {
        for (min, anchor) in [(100.0, 100.0), (100.0, 107.0), (100.0, 40.0), (100.0, 400.0)] {
            let first = first_grid_line(min, anchor, 18.0);
            assert!(first >= min - 0.001 && first < min + 18.0, "the first line is in the viewport: min={min} anchor={anchor} first={first}");
            let steps = (first - anchor) / 18.0;
            assert!((steps - steps.round()).abs() < 0.001, "it is a whole number of steps from the anchor: {first} vs {anchor}");
        }
        // Panning the page by 3 points moves its lines by the same 3 points, the same way.
        let before = first_grid_line(100.0, 107.0, 18.0);
        let after = first_grid_line(100.0, 110.0, 18.0);
        assert!((after - before - 3.0).abs() < 0.001, "lines follow the page: {before} -> {after}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    fn draw(tab: &str, source: bool) -> Vec<Act> {
        let t = Theme::DEFAULT.tokens();
        let ctx = egui::Context::default();
        let mut acts = Vec::new();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            acts = show(ui, tab, source, &View { zoom_percent: 100, ..View::default() }, Some(PageFormat::Html), &t);
        });
        out.textures_delta.clear();
        acts
    }

    #[test]
    fn the_tabs_are_office_like_and_source_comes_and_goes() {
        // Owner decision, 2026-11-27: Draw was folded into Home > Insérer, leaving Home + contextual tabs.
        assert_eq!(tabs(false, false, false), vec!["Home"]);
        assert_eq!(tabs(true, false, false), vec!["Home", "Source"]);
        assert_eq!(resolve_tab("Insert", false, false, false), "Home");
        assert_eq!(resolve_tab("Draw", false, false, false), "Home", "Draw was folded into Home");
        assert_eq!(resolve_tab("View", false, false, false), "Home", "the View tab was folded into Home");
        assert_eq!(resolve_tab("Source", true, false, false), "Source");
        assert_eq!(resolve_tab("Source", false, false, false), "Home", "the Source tab goes when the source pane does");
        assert_eq!(resolve_tab("Nonsense", false, false, false), "Home");
        assert_eq!(resolve_tab("", true, false, false), "Home");

        // The Table tab goes with the caret: it is offered only while the caret is in a
        // table, and leaving one takes the tab and its commands away.
        assert_eq!(tabs(false, true, false), vec!["Home", "Table"]);
        assert_eq!(resolve_tab("Table", false, true, false), "Table");
        assert_eq!(resolve_tab("Table", false, false, false), "Home", "out of the table, out of the tab");

        // The Image tab appears when an image / PDF is on the page.
        assert_eq!(tabs(false, false, true), vec!["Home", "Image"]);
        assert_eq!(resolve_tab("Image", false, false, true), "Image");
        assert_eq!(resolve_tab("Image", false, false, false), "Home");
    }

    #[test]
    fn every_paper_has_a_name_an_id_and_a_spacing() {
        use std::collections::HashSet;
        let all: Vec<RuleLines> = std::iter::once(RuleLines::None).chain(RuleLines::RULED).chain(RuleLines::GRIDS).chain(RuleLines::CRAFT).collect();
        assert_eq!(all.len(), 11, "none, four ruled, four grids, two of ours");
        let mut ids = HashSet::new();
        for r in &all {
            assert!(ids.insert(r.id()), "two papers share an id: {}", r.id());
            assert!(!r.label().is_empty());
            assert_eq!(RuleLines::parse(r.id()), Some(*r), "{} reads back", r.id());
            assert_eq!(r.on(), *r != RuleLines::None);
            assert!(r.step() > 8.0 || *r == RuleLines::None, "{} has a spacing", r.id());
        }
        // Only the squares draw upright lines, and only ours mark every fifth.
        assert!(RuleLines::GRIDS.iter().all(|r| r.squares()) && RuleLines::RULED.iter().all(|r| !r.squares()));
        assert!(RuleLines::CraftGrid.squares() && !RuleLines::CraftLines.squares());
        assert!(RuleLines::CRAFT.iter().all(|r| r.fives()));
        assert!(RuleLines::RULED.iter().chain(RuleLines::GRIDS.iter()).all(|r| !r.fives()), "OneNote's papers are evenly weighted");
        // The ids the first version wrote are ErgoNote's own two, so a settings file
        // written before the other papers existed keeps the paper it had.
        assert_eq!(RuleLines::parse("lines"), Some(RuleLines::CraftLines));
        assert_eq!(RuleLines::parse("grid"), Some(RuleLines::CraftGrid));
        assert_eq!(RuleLines::parse("nonsense"), None);
    }

    #[test]
    fn every_tab_draws_without_panicking_and_asks_for_nothing_by_itself() {
        for tab in ["Home", "Insert", "Draw", "View", "Source", "junk"] {
            for source in [false, true] {
                assert_eq!(draw(tab, source), Vec::new(), "{tab} asked for something nobody clicked");
            }
        }
    }

    #[test]
    fn image_tab_draws_compactly_without_panicking() {
        let t = Theme::DEFAULT.tokens();
        let ctx = egui::Context::default();
        let mut acts = Vec::new();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            acts = show(ui, IMAGE_TAB, false, &View { has_image: true, zoom_percent: 100, ..View::default() }, Some(PageFormat::Html), &t);
        });
        out.textures_delta.clear();
        assert_eq!(acts, Vec::new());
    }

    #[test]
    fn table_tab_draws_compactly_without_panicking() {
        let t = Theme::DEFAULT.tokens();
        let ctx = egui::Context::default();
        let mut acts = Vec::new();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            acts = show(ui, TABLE_TAB, false, &View { in_table: true, zoom_percent: 100, ..View::default() }, Some(PageFormat::Html), &t);
        });
        out.textures_delta.clear();
        assert_eq!(acts, Vec::new());
    }

    /// The acts are plain values, so the window's side of the ribbon is testable on its own.
    #[test]
    fn a_page_starts_with_no_lines_behind_it() {
        assert_eq!(RuleLines::default(), RuleLines::None);
        assert_eq!(RuleLines::parse("squares"), None);
        assert!(!RuleLines::None.on() && !RuleLines::None.squares() && !RuleLines::None.fives());
    }

    #[test]
    fn the_acts_name_what_they_do() {
        assert_eq!(Act::Mark("bold"), Act::Mark("bold"));
        assert_ne!(Act::Mark("bold"), Act::Mark("italic"));
        assert_ne!(Act::Block("h1"), Act::Insert("h1"));
        assert_eq!(Act::Tab("Insert".into()), Act::Tab("Insert".to_string()));
    }

    /// SUP-009: the font size list is OneNote's whole sizes (the page format stores whole
    /// numbers, so 9.5 and 10.5 wait for a format change).
    #[test]
    fn the_font_size_list_is_onenotes_whole_sizes() {
        assert_eq!(FONT_SIZES.as_slice(), [8u8, 9, 10, 11, 12, 14, 16, 18, 20, 22, 24, 26, 28, 36, 48, 72].as_slice());
    }
}
