//! The style `miette` uses: a boxed location line, a box-drawing wall, brackets down the margin.
//!
//! Named for the renderer whose shape it follows. It is miette-SHAPED and is not miette: it is not
//! byte-compatible with that crate's output and does not try to be. What it is for is being **far
//! enough from [`Rustc`](super::rustc::Rustc)** that the difference between the two is worth
//! naming — the design picked this pair for exactly that reason, because one indents where the
//! other boxes.

use core::fmt;

use super::{
  paint::Painter,
  present::{Drawn, Frame, Onset, Part, Presentation},
  render::{Block, Phrase, digits},
};
use crate::{Diagnostic, Role, Severity};

/// The boxed presentation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Miette;

/// The glyphs one bracket is drawn with, and the two an underline is.
///
/// A weight rather than a shape, because that is the axis left over. [`Rustc`](super::rustc::Rustc)
/// says which position a diagnostic is ABOUT with a different character — `^` against `-` — and a
/// box-drawing style has no second character to spare: `─` is the underline. So the primary is
/// drawn heavy and a secondary light, which is legible with no colour at all and stays legible
/// with it.
#[derive(Debug, Clone, Copy)]
struct Brackets {
  /// The corner on the source row a multi-line span opens on.
  opens: char,
  /// The bar it runs down.
  runs: char,
  /// The corner on the source row it closes on, which is a tee: the label hangs off it below.
  closes: char,
  /// The corner a label row starts with.
  corner: char,
  /// The horizontal run, used by an underline and by a label row's reach alike.
  rule: char,
  /// Where a single-line label hangs off its own underline.
  tee: char,
}

const HEAVY: Brackets = Brackets {
  opens: '┏',
  runs: '┃',
  closes: '┣',
  corner: '┗',
  rule: '━',
  tee: '┳',
};

const LIGHT: Brackets = Brackets {
  opens: '╭',
  runs: '│',
  closes: '├',
  corner: '╰',
  rule: '─',
  tee: '┬',
};

/// The glyphs a position of this importance is drawn with.
const fn brackets(primary: bool) -> Brackets {
  if primary { HEAVY } else { LIGHT }
}

/// The character that announces a severity, where [`Rustc`](super::rustc::Rustc) writes the word.
///
/// What that costs is searchability: `error` is a word a reader can grep a log for and `×` is not.
/// What it buys is a header one glyph wide, which is what leaves room for the box under it.
const fn glyph(severity: Severity) -> char {
  match severity {
    Severity::Error => '×',
    Severity::Warning => '⚠',
    Severity::Advice => '☞',
  }
}

impl Miette {
  /// `   · ` — the field of a row that says something about the line above it.
  ///
  /// The dot is what says the row is not a line of the file. [`Rustc`](super::rustc::Rustc) leaves
  /// the number field blank and repeats the bar; here the wall changes character instead, so a
  /// reader scanning the left edge sees where the source stops without counting spaces.
  fn annotation_field(&self, paint: &mut Painter<'_>, gutter: u64) -> fmt::Result {
    paint.pad(gutter + 2)?;
    paint.styled(Role::Gutter, "·")?;
    paint.frame_char(' ')
  }
}

impl Presentation for Miette {
  fn header(&self, paint: &mut Painter<'_>, diagnostic: &Diagnostic<'_>) -> fmt::Result {
    let severity = diagnostic.severity();
    paint.styled(Role::Code, diagnostic.code())?;
    paint.frame("\n\n  ")?;
    paint.styled(
      Role::Severity(severity),
      glyph(severity).encode_utf8(&mut [0; 4]),
    )?;
    paint.frame_char(' ')?;
    paint.shown(diagnostic.message())?;
    paint.newline()
  }

  /// The location line and the rule that opens the block are ONE row here.
  ///
  /// Not a saving: the box has a top, the top has to say something, and where the excerpt came
  /// from is what it says.
  ///
  /// Every input gets a box of its own, so `first` is nothing to this style.
  fn open_block(
    &self,
    paint: &mut Painter<'_>,
    gutter: u64,
    block: &Block<'_>,
    _first: bool,
  ) -> fmt::Result {
    paint.pad(gutter + 2)?;
    paint.styled(Role::Gutter, "╭─[")?;
    if let Some(origin) = block.origin {
      paint.shown(origin)?;
      paint.frame_char(':')?;
    }
    paint.frame_fmt(format_args!("{}:{}", block.line, block.column))?;
    paint.styled(Role::Gutter, "]")?;
    paint.newline()
  }

  fn close_block(&self, paint: &mut Painter<'_>, gutter: u64) -> fmt::Result {
    paint.pad(gutter + 2)?;
    paint.styled(Role::Gutter, "╰────")?;
    paint.newline()
  }

  /// ` NN │ ` — the number, right-aligned, and the box's own wall.
  ///
  /// One cell wider on the left than [`Rustc`](super::rustc::Rustc)'s, because the wall belongs to
  /// the box and the box is inset from the header above it.
  fn line_field(&self, paint: &mut Painter<'_>, gutter: u64, number: u64) -> fmt::Result {
    paint.frame_char(' ')?;
    paint.pad(gutter - digits(number))?;
    paint.styled(Role::LineNumber, &number.to_string())?;
    paint.frame_char(' ')?;
    paint.styled(Role::Gutter, "│")?;
    paint.frame_char(' ')
  }

  /// The plan's columns and one blank. A bracket here opens and closes in its own column, so
  /// nothing this style draws ever leaves the margin on a source row.
  fn margin(&self, paint: &mut Painter<'_>, frame: Frame<'_>) -> fmt::Result {
    frame.margin(paint)
  }

  /// In the WALL's column rather than the number's, which is the other way round from `...`.
  ///
  /// Both are true and they say different things: `...` stands where the numbers would be and
  /// reads as "numbers are missing"; `⋮` stands in the wall and reads as "rows are missing". A
  /// boxed style has a wall to say it in.
  fn elision_row(&self, paint: &mut Painter<'_>, frame: Frame<'_>) -> fmt::Result {
    paint.pad(frame.gutter() + 2)?;
    paint.styled(Role::LineNumber, "⋮")?;
    if let Some(columns) = frame.occupied() {
      paint.frame_char(' ')?;
      paint.margin(columns)?;
    }
    paint.newline()
  }

  fn bracket(&self, part: Part, role: Role, _compact: bool) -> Option<char> {
    let glyphs = brackets(matches!(role, Role::PrimaryLabel));
    Some(match part {
      Part::Opens => glyphs.opens,
      // A gap draws the bar it would have drawn on the rows it stands for. The `⋮` in the wall is
      // where this style says rows are missing, and a bracket dashed alongside it would be saying
      // the same thing twice — see [`Ariadne`](super::ariadne::Ariadne), which says it there
      // instead and so answers this differently.
      Part::Runs | Part::Gap => glyphs.runs,
      Part::Closes => glyphs.closes,
    })
  }

  /// Always. The bracket is in the margin whatever precedes the span, so this style never needs a
  /// row for an opening — and never marks the cell one would have pointed at.
  ///
  /// So it reads neither of [`Onset`]'s facts, and that is the honest measure of how much of the
  /// other style's rule was the other style's: the whole of it.
  fn opens_in_margin(&self, _onset: Onset) -> bool {
    true
  }

  /// An underline, and the label hanging off it below.
  ///
  /// TWO rows where [`Rustc`](super::rustc::Rustc) writes one, and that is a divergence rather
  /// than a taste. A marker row that also carries its label ends at whatever the label's length
  /// is, so a second label further right on the same line has nowhere to go; hanging the label off
  /// a tee leaves the underline row free for every span on the line. painty does not merge them
  /// yet — that is the row-assignment problem the renderer names as the next one — and this is the
  /// shape that will not have to change when it does.
  fn whole(
    &self,
    paint: &mut Painter<'_>,
    frame: Frame<'_>,
    columns: core::ops::Range<u64>,
    phrase: Phrase<'_>,
  ) -> fmt::Result {
    let glyphs = brackets(phrase.primary);
    let role = phrase.role();
    let width = columns.end - columns.start;
    // Where the label hangs from, and none when there is nothing to hang: a tee with no row under
    // it is a promise the render does not keep.
    let tee = phrase.text.map(|_| width / 2);

    self.annotation_field(paint, frame.gutter())?;
    frame.margin(paint)?;
    paint.pad(columns.start - 1)?;
    paint.styled_with(role, |shown| {
      for cell in 0..width {
        let at = if Some(cell) == tee {
          glyphs.tee
        } else {
          glyphs.rule
        };
        fmt::Write::write_char(shown, at)?;
      }
      Ok(())
    })?;
    paint.newline()?;

    let (Some(text), Some(hangs)) = (phrase.text, tee) else {
      return Ok(());
    };
    self.annotation_field(paint, frame.gutter())?;
    frame.margin(paint)?;
    paint.pad(columns.start - 1 + hangs)?;
    paint.styled_with(role, |shown| {
      fmt::Write::write_char(shown, glyphs.corner)?;
      fmt::Write::write_char(shown, glyphs.rule)?;
      fmt::Write::write_char(shown, glyphs.rule)
    })?;
    paint.frame_char(' ')?;
    paint.styled(role, text)?;
    paint.newline()
  }

  /// Nothing. Where a span opens is said by the corner already drawn in the margin of the source
  /// row above, so there is no row here and no cell marked — which is the whole of what this style
  /// gives up in exchange for never needing one.
  fn opens(
    &self,
    _paint: &mut Painter<'_>,
    _frame: Frame<'_>,
    _column: u64,
    _phrase: Phrase<'_>,
  ) -> fmt::Result {
    Ok(())
  }

  /// It never leaves the margin. [`Rustc`](super::rustc::Rustc)'s closing runs back along the row
  /// to the cell the span's last character sits in and puts a marker there; this one turns the
  /// corner under the bracket and says the label, so the closing end of a multi-line span is
  /// marked on no cell at all.
  ///
  /// The run still crosses every column to the right, for the reason the underscores do: a span
  /// still open out there opened later, so the crossing says this bracket closes over it.
  ///
  /// `column` is the cell the other style would mark, and this style has no use for it. That is
  /// the divergence stated as a signature.
  fn closes(
    &self,
    paint: &mut Painter<'_>,
    frame: Frame<'_>,
    _column: u64,
    phrase: Phrase<'_>,
  ) -> fmt::Result {
    let glyphs = brackets(phrase.primary);
    let role = phrase.role();
    self.annotation_field(paint, frame.gutter())?;
    frame.margin_left_of(paint, phrase.depth)?;

    let reach = frame.depth().saturating_sub(phrase.depth) + 4;
    paint.styled_with(role, |shown| {
      fmt::Write::write_char(shown, glyphs.corner)?;
      for _ in 0..reach {
        fmt::Write::write_char(shown, glyphs.rule)?;
      }
      Ok(())
    })?;
    if let Some(text) = phrase.text {
      paint.frame_char(' ')?;
      paint.styled(role, text)?;
    }
    paint.newline()
  }

  /// At a fixed indent rather than under the gutter: the box has closed by the time this is
  /// written, so there is no gutter left to align to.
  fn help(&self, paint: &mut Painter<'_>, _gutter: u64, help: &str, _drawn: Drawn) -> fmt::Result {
    paint.frame("  ")?;
    paint.styled(Role::Help, "help")?;
    paint.frame(": ")?;
    paint.styled(Role::Help, help)?;
    paint.newline()
  }

  /// Nothing. Every box closed itself, and the help was said outside them all.
  fn close_render(&self, _paint: &mut Painter<'_>, _gutter: u64, _drawn: Drawn) -> fmt::Result {
    Ok(())
  }
}
