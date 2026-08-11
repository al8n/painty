//! The style `rustc` uses: an arrow, an indented gutter, carets under the cells.
//!
//! Named for the renderer whose shape it follows. It is not byte-compatible with `rustc`'s output
//! and does not try to be; what it is is the arrangement a Rust reader already knows, which is why
//! it is the default.

use core::fmt;

use super::{
  paint::Painter,
  present::{Frame, Onset, Part, Presentation},
  render::{Block, Phrase, digits},
};
use crate::{Diagnostic, Role};

/// The default presentation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Rustc;

impl Rustc {
  /// The gutter of a row that says something about the line above it: blank where the number
  /// would be, and the bar.
  fn annotation_field(&self, paint: &mut Painter<'_>, gutter: u64) -> fmt::Result {
    paint.pad(gutter + 1)?;
    paint.styled(Role::Gutter, "|")?;
    paint.frame_char(' ')
  }

  /// One end of a multi-line span: the corner that joins its column to the cell it marks.
  ///
  /// The run of underscores reaches from this span's own connector column to the marker, so it
  /// crosses every column to the right of it. That is not an accident of drawing order: a span
  /// still open out there opened LATER than this one, so what the crossing says is that this
  /// bracket closes over it, and a run broken into pieces to avoid the crossing would stop reading
  /// as one connector at all.
  fn corner(
    &self,
    paint: &mut Painter<'_>,
    frame: Frame<'_>,
    column: u64,
    phrase: Phrase<'_>,
    closing: bool,
  ) -> fmt::Result {
    self.annotation_field(paint, frame.gutter())?;
    // Only what is to the LEFT is written from the margin. Everything from this span's own column
    // rightwards is the corner below.
    frame.margin_left_of(paint, phrase.depth)?;

    let role = phrase.role();
    let marker = marker(phrase.primary);
    let reach = (frame.depth() + column).saturating_sub(phrase.depth);
    paint.styled_with(role, |shown| {
      fmt::Write::write_char(shown, if closing { '|' } else { ' ' })?;
      for _ in 0..reach {
        fmt::Write::write_char(shown, '_')?;
      }
      fmt::Write::write_char(shown, marker)
    })?;
    // Said where the span CLOSES, because that is where a reader has seen all of it.
    if closing && let Some(text) = phrase.text {
      paint.frame_char(' ')?;
      paint.styled(role, text)?;
    }
    paint.newline()
  }
}

/// The character a reader already knows: a caret for the position the diagnostic is about, a dash
/// for one it is only mentioning.
const fn marker(primary: bool) -> char {
  if primary { '^' } else { '-' }
}

impl Presentation for Rustc {
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

  fn open_block(&self, paint: &mut Painter<'_>, gutter: u64, block: &Block<'_>) -> fmt::Result {
    paint.pad(gutter)?;
    paint.frame("--> ")?;
    if let Some(origin) = block.origin {
      paint.shown(origin)?;
      paint.frame_char(':')?;
    }
    paint.frame_fmt(format_args!("{}:{}\n", block.line, block.column))?;
    // One `  |` separator row. Carries no connectors, and deliberately: it is drawn once above an
    // input's first line and once below its last, where nothing is open.
    paint.pad(gutter + 1)?;
    paint.styled(Role::Gutter, "|")?;
    paint.newline()
  }

  fn close_block(&self, paint: &mut Painter<'_>, gutter: u64) -> fmt::Result {
    paint.pad(gutter + 1)?;
    paint.styled(Role::Gutter, "|")?;
    paint.newline()
  }

  fn line_field(&self, paint: &mut Painter<'_>, gutter: u64, number: u64) -> fmt::Result {
    paint.pad(gutter - digits(number))?;
    paint.styled(Role::LineNumber, &number.to_string())?;
    paint.frame_char(' ')?;
    paint.styled(Role::Gutter, "|")?;
    paint.frame_char(' ')
  }

  /// Written where the line NUMBER would be, which is what says that numbers are missing rather
  /// than that a row is.
  fn elision_row(&self, paint: &mut Painter<'_>, frame: Frame<'_>) -> fmt::Result {
    paint.styled(Role::LineNumber, "...")?;
    if let Some(columns) = frame.occupied() {
      paint.pad(frame.gutter())?;
      paint.margin(columns)?;
    }
    paint.newline()
  }

  fn bracket(&self, part: Part, _role: Role, compact: bool) -> Option<char> {
    match part {
      // The span has not opened yet on its own source row: what opens it is either the `/` here or
      // the underscore run on the row below, and those are the same decision.
      Part::Opens => compact.then_some('/'),
      Part::Runs | Part::Closes => Some('|'),
    }
  }

  /// A span opens with a `/` in its column when the row it opens on DRAWS the cell it opens at,
  /// nothing else is drawn under that line, and nothing but blanks precedes it there — so it needs
  /// no corner row and its start cell carries no marker.
  ///
  /// [`Onset::drawn`] is the one a reader would not think to ask for, and it is the one whose
  /// absence made this wrong. Suppressing the marker is only a saving if the cell the marker would
  /// have gone on is on the page: a row is cut at
  /// [`max_rendered_width`](super::Terminal::max_rendered_width) CELLS, so an opening five
  /// thousand spaces into a line is out past the `…` and the compact form leaves the span with a
  /// `/` in the margin and nothing at all pointing into the row.
  fn opens_in_margin(&self, onset: Onset) -> bool {
    onset.alone() && onset.drawn() && onset.blank()
  }

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
    let marker = marker(phrase.primary);
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

  fn help(&self, paint: &mut Painter<'_>, gutter: u64, help: &str) -> fmt::Result {
    paint.pad(gutter + 1)?;
    paint.frame("= ")?;
    paint.styled(Role::Help, "help")?;
    paint.frame(": ")?;
    paint.styled(Role::Help, help)?;
    paint.newline()
  }
}
