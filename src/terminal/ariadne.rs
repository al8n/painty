//! The style `ariadne` uses: one frame around the whole report, and arrows into the source.
//!
//! Named for the renderer whose shape it follows. It is ariadne-SHAPED and is not ariadne: it is
//! not byte-compatible with that crate's output and does not try to be.
//!
//! # What it was added to find out
//!
//! [`Presentation`] was derived from two implementations, and two of anything cannot show that an
//! abstraction is right — they can only fail to show it is wrong. The design named this style as
//! the furthest of the three from what was already there: *rustc indents, miette boxes, ariadne
//! draws arrows.* It is the arrow that pays: where a multi-line span opens is said **on the source
//! row**, by a run that leaves the bracket's column, crosses every column to its right and ends
//! pointing at the line. Neither style before it writes anything at all between the margin and the
//! text, so that region belonged to the renderer — and it could not stay there.
//!
//! Three other things this style says that the first two had no way to:
//!
//! - its frame belongs to the **render** rather than to a block, so a second input is introduced
//!   inside the frame and the frame closes once, *after* the help;
//! - a row standing for elided lines breaks the connectors beside the wall as well as the wall;
//! - a bracket that never marks a cell still turns a corner on a row of its own, one row below the
//!   source, with the label hanging off it.
//!
//! # It distinguishes the primary by colour alone
//!
//! [`Rustc`](super::rustc::Rustc) says which position a diagnostic is ABOUT with a different
//! character, `^` against `-`; [`Miette`](super::miette::Miette) with a different weight. This one
//! does neither, because ariadne does neither: it gives every label a colour of its own and lets
//! the colour carry it. That is a real cost and it is the style's, not a defect to repair —
//! rendered at [`ColorCapability::None`](super::ColorCapability::None) a primary and a secondary
//! label are drawn with the same glyphs, and the two are told apart by what their labels say.

use core::fmt;

use super::{
  paint::Painter,
  present::{Drawn, Frame, Onset, Part, Presentation},
  render::{Block, Phrase, digits, slot},
};
use crate::{Diagnostic, Role, Severity};

/// The arrowed presentation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Ariadne;

/// The wall, and the bar a bracket runs down.
const BAR: char = '│';
/// The wall of a row standing for lines that were left out, and of the connectors beside it.
const BREAK: char = '┆';
/// The horizontal run: a crossing, an underline and a corner's reach alike.
const RULE: char = '─';
/// What an arrow ends in, and the whole of what a multi-line span points at.
const ARROW: char = '▶';
/// Where a single-line label hangs off its own underline.
const TEE: char = '┬';
/// The corner a label row starts with.
const CORNER: char = '╰';

/// How many cells stand between the last connector column and the source text.
///
/// Two for the arrow's own `─▶`, and one for the blank every style leaves between the margin and
/// the text. It is a constant of this style rather than of the renderer, and that is the point:
/// the styles before it left one blank and nothing else, so nothing above knew the region had a
/// width at all.
const APPROACH: u64 = 3;

/// The word this style announces a severity with.
///
/// Capitalised, and the whole header is `[code] Kind: message` — the code leads, where
/// [`Rustc`](super::rustc::Rustc) puts it inside the severity's own brackets.
const fn kind(severity: Severity) -> &'static str {
  match severity {
    Severity::Error => "Error",
    Severity::Warning => "Warning",
    Severity::Advice => "Advice",
  }
}

impl Ariadne {
  /// `   │` — a row of the frame carrying nothing, which is what separates its sections.
  fn rule(&self, paint: &mut Painter<'_>, gutter: u64) -> fmt::Result {
    paint.pad(gutter + 2)?;
    paint.styled(Role::Gutter, "│")?;
    paint.newline()
  }

  /// `   │ ` — the field of a row that says something about the line above it.
  ///
  /// The wall does not change character here, unlike [`Miette`](super::miette::Miette)'s: what
  /// tells a reader this is not a line of the file is that it has no number, and the frame runs
  /// unbroken from the location line to the bottom rule.
  fn annotation_field(&self, paint: &mut Painter<'_>, gutter: u64) -> fmt::Result {
    paint.pad(gutter + 2)?;
    paint.styled(Role::Gutter, "│")?;
    paint.frame_char(' ')
  }

  /// The connector columns of an annotation row, and the arrow's own width left blank.
  ///
  /// Every row of a block has to reach the source text at the same cell, and this style's source
  /// rows spend [`APPROACH`] cells getting there. A row that used the renderer's single blank
  /// would put its underline two cells left of the thing it points at.
  fn blank_margin(&self, paint: &mut Painter<'_>, frame: Frame<'_>) -> fmt::Result {
    if frame.depth() == 0 {
      return Ok(());
    }
    frame.margin(paint)?;
    paint.pad(APPROACH - 1)
  }

  /// The connector columns of a row that says nothing after the last of them, with the bracket at
  /// `depth` shown as a plain bar.
  ///
  /// The bracket has already turned on the source row above — that is where its corner was drawn —
  /// so what this row says about it is only that it is still running. Everything to the right of
  /// it stands as it is, because a span still open out there is not this one's to redraw, and the
  /// trailing blanks are dropped so the row ends where it stops saying anything.
  fn running_margin(
    &self,
    paint: &mut Painter<'_>,
    frame: Frame<'_>,
    depth: u64,
    role: Role,
  ) -> fmt::Result {
    let columns = frame.columns();
    let own = slot(depth).saturating_sub(1);
    if own >= columns.len() {
      return paint.margin(columns);
    }
    let last = columns.iter().rposition(Option::is_some).unwrap_or(own);
    paint.margin(&columns[..own])?;
    paint.styled(role, BAR.encode_utf8(&mut [0; 4]))?;
    paint.margin(&columns[own + 1..=last.max(own)])
  }
}

impl Presentation for Ariadne {
  fn header(&self, paint: &mut Painter<'_>, diagnostic: &Diagnostic<'_>) -> fmt::Result {
    let severity = diagnostic.severity();
    paint.styled(Role::Code, "[")?;
    paint.styled(Role::Code, diagnostic.code())?;
    paint.styled(Role::Code, "]")?;
    paint.frame_char(' ')?;
    paint.styled(Role::Severity(severity), kind(severity))?;
    paint.frame(": ")?;
    paint.shown(diagnostic.message())?;
    paint.newline()
  }

  /// The top of the frame for the first block, and a tee INTO it for every later one.
  ///
  /// The separator row above a later block is written here rather than by
  /// [`close_block`](Self::close_block), and the two produce the same bytes: ariadne ends a
  /// non-final section with an empty frame row, and the only thing that can follow one is another
  /// section. Saying it on this side means the style never needs to be told which block is last.
  fn open_block(
    &self,
    paint: &mut Painter<'_>,
    gutter: u64,
    block: &Block<'_>,
    first: bool,
  ) -> fmt::Result {
    if !first {
      self.rule(paint, gutter)?;
    }
    paint.pad(gutter + 2)?;
    paint.styled(Role::Gutter, if first { "╭─[ " } else { "├─[ " })?;
    if let Some(origin) = block.origin {
      paint.shown(origin)?;
      paint.frame_char(':')?;
    }
    paint.frame_fmt(format_args!("{}:{}", block.line, block.column))?;
    paint.styled(Role::Gutter, " ]")?;
    paint.newline()?;
    self.rule(paint, gutter)
  }

  /// Nothing. The frame is the render's rather than the block's, so what would close a block here
  /// is written once at the end — see [`close_render`](Self::close_render).
  fn close_block(&self, _paint: &mut Painter<'_>, _gutter: u64) -> fmt::Result {
    Ok(())
  }

  /// ` NN │ ` — the number, right-aligned, and the frame's own wall.
  fn line_field(&self, paint: &mut Painter<'_>, gutter: u64, number: u64) -> fmt::Result {
    paint.frame_char(' ')?;
    paint.pad(gutter - digits(number))?;
    paint.styled(Role::LineNumber, &number.to_string())?;
    paint.frame_char(' ')?;
    paint.styled(Role::Gutter, "│")?;
    paint.frame_char(' ')
  }

  /// The arrow, and the run that reaches it.
  ///
  /// A bracket with an end on this row leaves its own column, crosses every column to the right of
  /// it, and finishes with `─▶` against the source. The crossing is not an accident of drawing
  /// order: a span still open out there opened LATER than this one, so what the run says is that
  /// this bracket closes over it.
  ///
  /// Where nothing turns, the arrow's cells are blank and the columns stand as the plan filled
  /// them.
  ///
  /// # Two ends on one row, which is what the contract above has to survive
  ///
  /// The run starts at the **leftmost** end and every column from there is either an end of its
  /// own — drawn as the plan filled it, because a corner and the run passing through it are the
  /// same cell — or a cell of the run. A row can carry ends at depth 1 and depth 3 with a bracket
  /// still running at depth 2, or with depth 2 already closed and empty, and in both the left end
  /// has to reach: `├─╭` and `├─├`, not `├│╭` and `├ ├`.
  ///
  /// It could not, while the renderer said which columns turned by handing over one depth — *the
  /// rightmost*, where two did. That is a set projected onto one of its members, and the member it
  /// kept was the one with nothing to cross. The fact travels per column now; see [`Standing`].
  fn margin(&self, paint: &mut Painter<'_>, frame: Frame<'_>) -> fmt::Result {
    let columns = frame.columns();
    if columns.is_empty() {
      return Ok(());
    }
    let Some(leftmost) = columns
      .iter()
      .position(|column| column.is_some_and(|standing| standing.turns()))
    else {
      paint.margin(columns)?;
      return paint.pad(APPROACH);
    };
    paint.margin(&columns[..leftmost])?;
    // Every column from the leftmost end rightwards belongs to the run, except the cells where
    // another bracket's own end stands — that cell is both, and the end is what a reader needs.
    let role = columns[leftmost].map_or(Role::Gutter, |standing| standing.role());
    for column in &columns[leftmost..] {
      match column {
        Some(standing) if standing.turns() => {
          paint.styled(standing.role(), standing.glyph().encode_utf8(&mut [0; 4]))?;
        }
        _ => paint.styled(role, RULE.encode_utf8(&mut [0; 4]))?,
      }
    }
    paint.styled_with(role, |shown| {
      fmt::Write::write_char(shown, RULE)?;
      fmt::Write::write_char(shown, ARROW)
    })?;
    paint.frame_char(' ')
  }

  /// The wall and every connector beside it, all broken the same way.
  ///
  /// **The one place this style needs [`Part::Gap`].** A row standing for lines that were left out
  /// is not a row of the file, and every other style says so in the wall's column alone while its
  /// brackets run through it unchanged. Here the break is the whole width of the
  /// margin, so a reader following a bracket downwards sees it dashed exactly where the lines it
  /// covers stop being shown.
  fn elision_row(&self, paint: &mut Painter<'_>, frame: Frame<'_>) -> fmt::Result {
    paint.pad(frame.gutter() + 2)?;
    paint.styled(Role::Gutter, BREAK.encode_utf8(&mut [0; 4]))?;
    if let Some(columns) = frame.occupied() {
      paint.frame_char(' ')?;
      paint.margin(columns)?;
    }
    paint.newline()
  }

  fn bracket(&self, part: Part, _role: Role, _compact: bool) -> Option<char> {
    Some(match part {
      Part::Opens => '╭',
      Part::Runs => BAR,
      Part::Closes => '├',
      Part::Gap => BREAK,
    })
  }

  /// Always. The arrow is drawn on the source row whatever precedes the span, so this style never
  /// needs a row for an opening — and never marks the cell one would have pointed at.
  fn opens_in_margin(&self, _onset: Onset) -> bool {
    true
  }

  /// An underline, and the label hanging off it below.
  ///
  /// The same two rows [`Miette`](super::miette::Miette) writes, and drawn in the same light
  /// glyphs — this style has no heavy pair to reach for, because it distinguishes a primary by
  /// colour. Two labels on one line therefore differ in what they say and in what they are painted
  /// with, and not in their shape.
  fn whole(
    &self,
    paint: &mut Painter<'_>,
    frame: Frame<'_>,
    columns: core::ops::Range<u64>,
    phrase: Phrase<'_>,
  ) -> fmt::Result {
    let role = phrase.role();
    let width = columns.end - columns.start;
    // Where the label hangs from, and none when there is nothing to hang.
    let tee = phrase.text.map(|_| width / 2);

    self.annotation_field(paint, frame.gutter())?;
    self.blank_margin(paint, frame)?;
    paint.pad(columns.start - 1)?;
    paint.styled_with(role, |shown| {
      for cell in 0..width {
        let at = if Some(cell) == tee { TEE } else { RULE };
        fmt::Write::write_char(shown, at)?;
      }
      Ok(())
    })?;
    paint.newline()?;

    let (Some(text), Some(hangs)) = (phrase.text, tee) else {
      return Ok(());
    };
    self.annotation_field(paint, frame.gutter())?;
    self.blank_margin(paint, frame)?;
    paint.pad(columns.start - 1 + hangs)?;
    paint.styled_with(role, |shown| {
      fmt::Write::write_char(shown, CORNER)?;
      fmt::Write::write_char(shown, RULE)?;
      fmt::Write::write_char(shown, RULE)
    })?;
    paint.frame_char(' ')?;
    paint.styled(role, text)?;
    paint.newline()
  }

  /// Nothing. The arrow on the source row has already said where the span opens, and it points at
  /// the LINE — so no cell is marked, which is the exception
  /// `which_cells_are_marked_is_a_function_of_the_span_alone` is stated around.
  fn opens(
    &self,
    _paint: &mut Painter<'_>,
    _frame: Frame<'_>,
    _column: u64,
    _phrase: Phrase<'_>,
  ) -> fmt::Result {
    Ok(())
  }

  /// Two rows: the bracket still running, and then the corner it turns with its label.
  ///
  /// The first is what [`Miette`](super::miette::Miette) does without and this style does not: a
  /// closing arrow points into the source row, so turning the corner on the very next row would
  /// put two different statements about one span in adjacent cells. One row of plain bar between
  /// them is what separates "this is where it ends" from "and this is what it is".
  ///
  /// `column` is the cell the indented style would mark, and this one has no use for it.
  fn closes(
    &self,
    paint: &mut Painter<'_>,
    frame: Frame<'_>,
    _column: u64,
    phrase: Phrase<'_>,
  ) -> fmt::Result {
    let role = phrase.role();
    self.annotation_field(paint, frame.gutter())?;
    self.running_margin(paint, frame, phrase.depth, role)?;
    paint.newline()?;

    self.annotation_field(paint, frame.gutter())?;
    frame.margin_left_of(paint, phrase.depth)?;
    // Past the arrow's own cells and one beyond, so the label starts clear of the source text
    // rather than under it.
    let reach = frame.depth().saturating_sub(phrase.depth) + APPROACH + 1;
    paint.styled_with(role, |shown| {
      fmt::Write::write_char(shown, CORNER)?;
      for _ in 0..reach {
        fmt::Write::write_char(shown, RULE)?;
      }
      Ok(())
    })?;
    if let Some(text) = phrase.text {
      paint.frame_char(' ')?;
      paint.styled(role, text)?;
    }
    paint.newline()
  }

  /// Inside the frame, under a separator row of its own — and outside it when there is no frame.
  ///
  /// The other two say it once the frame has closed, so neither of them reads [`Drawn`]. Here the
  /// frame has not closed yet and [`close_render`](Self::close_render) is what ends the report
  /// below this, which is exactly why the empty case has to be told: a diagnostic that names no
  /// position this renderer can draw has no frame around it, and a wall drawn here would belong to
  /// one nothing opened.
  fn help(&self, paint: &mut Painter<'_>, gutter: u64, help: &str, drawn: Drawn) -> fmt::Result {
    match drawn {
      Drawn::Blocks => {
        self.rule(paint, gutter)?;
        self.annotation_field(paint, gutter)?;
      }
      Drawn::Nothing => paint.frame("  ")?,
    }
    paint.styled(Role::Help, "Help")?;
    paint.frame(": ")?;
    paint.styled(Role::Help, help)?;
    paint.newline()
  }

  /// The bottom of the frame, run back under the gutter to the left edge — and nothing at all when
  /// no block opened one.
  fn close_render(&self, paint: &mut Painter<'_>, gutter: u64, drawn: Drawn) -> fmt::Result {
    if drawn == Drawn::Nothing {
      return Ok(());
    }
    paint.styled_with(Role::Gutter, |shown| {
      for _ in 0..gutter + 2 {
        fmt::Write::write_char(shown, RULE)?;
      }
      fmt::Write::write_char(shown, '╯')
    })?;
    paint.newline()
  }
}
