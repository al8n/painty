//! The style `codespan-reporting` uses: rustc's arrangement drawn in box characters.
//!
//! Named for the renderer whose shape it follows. It is codespan-SHAPED and is not
//! codespan-reporting: it is not byte-compatible with that crate's output and does not try to be.
//!
//! # What it was added to find out, and what the answer was
//!
//! [`Ariadne`](super::ariadne::Ariadne) is the style the design predicted would move the trait, and
//! it did. This one is the control: a renderer designed independently of both of the first two,
//! widely used, and near enough to [`Rustc`](super::rustc::Rustc) that whatever it needs is a
//! question about the SEAM rather than about the layout. It needed **nothing the trait did not
//! already have** — the one capability it reaches for, reading the plan's connector columns back so
//! a run can pass behind them, arrived with the third style.
//!
//! Eight of the thirteen methods say something different from `Rustc` all the same, and two say
//! exactly the same thing: the header is `error[code]: message` in both, and a single-line span is
//! carets under the cells with the label on the same row. That is the honest measure of how close
//! two independently-designed renderers are, and it is why a diagnostic with no position to draw
//! renders identically in the two — see `no_two_styles_draw_the_same_bytes`, which states the rule
//! over the corpus rather than over each case for exactly this reason.
//!
//! # Where it deliberately does not follow the crate it is named for
//!
//! `codespan-reporting` announces a multi-line span in the margin when the span begins **at or
//! before its line's first non-blank** — `*start <= source.len() - source.trim_start().len()`. That
//! is the weaker of the two conditions painty considered and rejected: every column of the
//! indentation satisfies it, and a margin glyph names exactly one cell, so two spans opening at two
//! columns of one indented line are announced identically. painty has a test for that case —
//! `an_opening_the_row_cannot_name_is_marked_rather_than_compacted` — and a style reproducing the
//! weaker rule would fail it. So this style answers
//! [`opens_in_margin`](Presentation::opens_in_margin) exactly as `Rustc` does, and the divergence is
//! recorded here rather than rendered.

use core::fmt;

use super::{
  paint::Painter,
  present::{Drawn, Frame, Onset, Part, Presentation},
  render::{Block, Phrase, digits, slot},
};
use crate::{Diagnostic, Role};

/// The box-drawing presentation of rustc's arrangement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Codespan;

/// The wall beside the line numbers, and the bar a bracket runs down.
const BAR: char = '│';
/// The wall of a row standing for lines that were left out.
const BREAK: char = '·';
/// The horizontal run of a bracket's two corners.
const RULE: char = '─';

/// The character a reader already knows, at both ends of a span drawn on one line.
const fn caret(primary: bool) -> char {
  if primary { '^' } else { '-' }
}

/// What a multi-line span's corner run ends in.
///
/// The same pair of importances as [`caret`], and a different pair of characters: an apostrophe
/// where a caret would sit on a cell of its own is what the crate this style is named for uses, and
/// it reads as the end of a rule rather than as a second underline.
const fn terminator(primary: bool) -> char {
  if primary { '^' } else { '\'' }
}

impl Codespan {
  /// The gutter of a row that says something about the line above it: blank where the number would
  /// be, and the wall.
  fn annotation_field(&self, paint: &mut Painter<'_>, gutter: u64) -> fmt::Result {
    paint.pad(gutter + 1)?;
    paint.styled(Role::Gutter, BAR.encode_utf8(&mut [0; 4]))?;
    paint.frame_char(' ')
  }

  /// One end of a multi-line span: the corner that joins its column to the cell it marks.
  ///
  /// The same row [`Rustc`](super::rustc::Rustc) draws, and it crosses the margin differently.
  /// There the run is underscores over everything to the right of its own column, so a bar it
  /// passes is overwritten; here every column that has something standing in it keeps it and the
  /// rule fills only the gaps, which is a run passing BEHIND the connectors it crosses rather than
  /// over them. Both say the same thing — a span still open out there opened later, so this bracket
  /// closes over it — and this one leaves the other bracket readable while saying it.
  fn corner(
    &self,
    paint: &mut Painter<'_>,
    frame: Frame<'_>,
    column: u64,
    phrase: Phrase<'_>,
    closing: bool,
  ) -> fmt::Result {
    self.annotation_field(paint, frame.gutter())?;
    frame.margin_left_of(paint, phrase.depth)?;

    let role = phrase.role();
    let columns = frame.columns();
    let crossed = slot(phrase.depth).min(columns.len());
    paint.styled(
      role,
      if closing { '╰' } else { '╭' }.encode_utf8(&mut [0; 4]),
    )?;
    for standing in &columns[crossed..] {
      match standing {
        Some((glyph, occupant)) => paint.styled(*occupant, glyph.encode_utf8(&mut [0; 4]))?,
        None => paint.styled(role, RULE.encode_utf8(&mut [0; 4]))?,
      }
    }
    paint.styled_with(role, |shown| {
      for _ in 0..column {
        fmt::Write::write_char(shown, RULE)?;
      }
      fmt::Write::write_char(shown, terminator(phrase.primary))
    })?;
    // Said where the span CLOSES, because that is where a reader has seen all of it.
    if closing && let Some(text) = phrase.text {
      paint.frame_char(' ')?;
      paint.styled(role, text)?;
    }
    paint.newline()
  }
}

impl Presentation for Codespan {
  /// `error[code]: message`, which is [`Rustc`](super::rustc::Rustc)'s to the byte.
  ///
  /// Recorded rather than differentiated. Two renderers designed independently arrived at the same
  /// header, and inventing a difference to make this method carry one would be the opposite of what
  /// the styles are here to measure.
  fn header(&self, paint: &mut Painter<'_>, diagnostic: &Diagnostic<'_>) -> fmt::Result {
    let severity = diagnostic.severity();
    paint.styled(Role::Severity(severity), severity.as_str())?;
    paint.styled(Role::Code, "[")?;
    paint.styled(Role::Code, diagnostic.code())?;
    paint.styled(Role::Code, "]")?;
    paint.frame(": ")?;
    paint.shown(diagnostic.message())?;
    paint.newline()
  }

  /// `┌─` in the WALL's own column, where the arrow of the style this follows sits one cell left of
  /// it and points into the location instead.
  ///
  /// Every input gets a snippet of its own, so `first` is nothing to this style.
  fn open_block(
    &self,
    paint: &mut Painter<'_>,
    gutter: u64,
    block: &Block<'_>,
    _first: bool,
  ) -> fmt::Result {
    paint.pad(gutter + 1)?;
    paint.styled(Role::Gutter, "┌─")?;
    paint.frame_char(' ')?;
    if let Some(origin) = block.origin {
      paint.shown(origin)?;
      paint.frame_char(':')?;
    }
    paint.frame_fmt(format_args!("{}:{}\n", block.line, block.column))?;
    self.close_block(paint, gutter)
  }

  fn close_block(&self, paint: &mut Painter<'_>, gutter: u64) -> fmt::Result {
    paint.pad(gutter + 1)?;
    paint.styled(Role::Gutter, BAR.encode_utf8(&mut [0; 4]))?;
    paint.newline()
  }

  fn line_field(&self, paint: &mut Painter<'_>, gutter: u64, number: u64) -> fmt::Result {
    paint.pad(gutter - digits(number))?;
    paint.styled(Role::LineNumber, &number.to_string())?;
    paint.frame_char(' ')?;
    paint.styled(Role::Gutter, BAR.encode_utf8(&mut [0; 4]))?;
    paint.frame_char(' ')
  }

  /// The plan's columns and one blank. Both ends of a multi-line span are drawn on rows of their
  /// own, so nothing this style draws reaches into a source row.
  fn margin(&self, paint: &mut Painter<'_>, frame: Frame<'_>, _turns: Option<u64>) -> fmt::Result {
    frame.margin(paint)
  }

  /// A break in the WALL, where [`Rustc`](super::rustc::Rustc) writes `...` in the number's column.
  ///
  /// The third answer on this method and the second placement: this style says rows are missing
  /// where the wall would be, as the boxed one does, and says it with a dot rather than a vertical
  /// ellipsis. What it does NOT do is break the connectors beside it — see
  /// [`bracket`](Self::bracket).
  fn elision_row(&self, paint: &mut Painter<'_>, frame: Frame<'_>) -> fmt::Result {
    paint.pad(frame.gutter() + 1)?;
    paint.styled(Role::Gutter, BREAK.encode_utf8(&mut [0; 4]))?;
    if let Some(columns) = frame.occupied() {
      paint.frame_char(' ')?;
      paint.margin(columns)?;
    }
    paint.newline()
  }

  /// The same three answers [`Rustc`](super::rustc::Rustc) gives, drawn in box characters — and the
  /// same answer for a gap as for a run, because the `·` in the wall is where this style says rows
  /// are missing.
  fn bracket(&self, part: Part, _role: Role, compact: bool) -> Option<char> {
    match part {
      Part::Opens => compact.then_some('╭'),
      Part::Runs | Part::Closes | Part::Gap => Some(BAR),
    }
  }

  /// [`Rustc`](super::rustc::Rustc)'s rule, and deliberately not the one the crate this style is
  /// named for uses — see the note at the top of this module.
  fn opens_in_margin(&self, onset: Onset) -> bool {
    onset.drawn() && onset.at_first_nonblank()
  }

  /// Carets under the cells and the label on the same row, which is
  /// [`Rustc`](super::rustc::Rustc)'s to the byte.
  fn whole(
    &self,
    paint: &mut Painter<'_>,
    frame: Frame<'_>,
    columns: core::ops::Range<u64>,
    phrase: Phrase<'_>,
  ) -> fmt::Result {
    self.annotation_field(paint, frame.gutter())?;
    frame.margin(paint)?;
    paint.pad(columns.start - 1)?;

    let role = phrase.role();
    let marker = caret(phrase.primary);
    paint.styled_with(role, |shown| {
      for _ in 0..columns.end - columns.start {
        fmt::Write::write_char(shown, marker)?;
      }
      Ok(())
    })?;
    if let Some(text) = phrase.text {
      paint.frame_char(' ')?;
      paint.styled(role, text)?;
    }
    paint.newline()
  }

  fn opens(
    &self,
    paint: &mut Painter<'_>,
    frame: Frame<'_>,
    column: u64,
    phrase: Phrase<'_>,
  ) -> fmt::Result {
    if phrase.compact {
      return Ok(());
    }
    self.corner(paint, frame, column, phrase, false)
  }

  fn closes(
    &self,
    paint: &mut Painter<'_>,
    frame: Frame<'_>,
    column: u64,
    phrase: Phrase<'_>,
  ) -> fmt::Result {
    self.corner(paint, frame, column, phrase, true)
  }

  /// `= ` and the caller's own words, with no word of this crate's in front of them.
  ///
  /// The bullet is where [`Rustc`](super::rustc::Rustc)'s is and what follows it is not: the crate
  /// this style is named for renders a note as the note, on the reasoning that a producer writing
  /// "consider using a wildcard pattern" has already said what kind of thing it is.
  fn help(&self, paint: &mut Painter<'_>, gutter: u64, help: &str, _drawn: Drawn) -> fmt::Result {
    paint.pad(gutter + 1)?;
    paint.frame("= ")?;
    paint.styled(Role::Help, help)?;
    paint.newline()
  }

  /// Nothing. Every block closed itself with a wall, so there is nothing left open to end.
  fn close_render(&self, _paint: &mut Painter<'_>, _gutter: u64, _drawn: Drawn) -> fmt::Result {
    Ok(())
  }
}
