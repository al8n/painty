use core::fmt;

use unicode_width::UnicodeWidthStr;

use crate::Line;

/// One source line, measured in the cells a fixed-width terminal will give it.
///
/// # Three units, and this is the third
///
/// A **byte offset** indexes the caller's text. A **character column** counts characters, and is
/// what [`Line::column_at`] reports. A **display column** counts the cells a terminal actually
/// paints, and it agrees with neither: a CJK ideograph is one character in two cells, a combining
/// mark is one character in none, and a tab is one character in however many cells the next tab
/// stop is away.
///
/// Resolution stays free of this on purpose — measuring cells needs a Unicode table, and layer 2
/// has no dependencies. So the conversion lives here, behind the `terminal` feature, and it is
/// built **on** layer 2's answers rather than beside them: everything below is a function of the
/// text slices [`Line`] and [`RegionLine`](crate::RegionLine) already hand out, so no line, column
/// or span is ever recomputed here.
///
/// # What a display column is defined to be
///
/// `1` plus the number of cells the text *before* an offset occupies. That definition is what the
/// invariants are written against, and it has one consequence worth naming: a prefix cut through
/// the middle of a grapheme cluster is measured as the prefix, not as the cluster, because a
/// terminal drawing that prefix would do the same.
///
/// ```
/// use painty::{Source, terminal::LineCells};
///
/// let source = Source::new("\tabc");
/// let cells = LineCells::new(source.line(1).unwrap(), 4);
///
/// // The tab occupies whatever reaches the next stop, so `a` is not in column 2.
/// assert_eq!(cells.column_at(0), 1);
/// assert_eq!(cells.column_at(1), 5);
/// assert_eq!(cells.width(), 7);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LineCells<'a> {
  line: Line<'a>,
  tab_width: u64,
}

impl<'a> LineCells<'a> {
  /// Measures `line` against a tab stop every `tab_width` cells.
  ///
  /// A width of zero would leave a tab advancing nowhere and every later column wrong, so it is
  /// raised to one: a tab is at minimum one cell, whatever the caller asked for.
  #[inline]
  #[must_use]
  pub const fn new(line: Line<'a>, tab_width: u64) -> Self {
    Self {
      line,
      tab_width: if tab_width == 0 { 1 } else { tab_width },
    }
  }

  /// The tab width used when a caller has no reason to choose one.
  ///
  /// A function rather than a `pub const`, so that the same function-pointer ascription pins it as
  /// pins everything else: a constant cannot be named by a `fn(..) -> ..` type, and one member is
  /// not worth a second pinning mechanism.
  #[inline]
  #[must_use]
  pub const fn default_tab_width() -> u64 {
    4
  }

  /// Returns the line being measured.
  #[inline]
  pub const fn line(&self) -> Line<'a> {
    self.line
  }

  /// Returns the distance between tab stops, in cells.
  ///
  /// `u64` by rule 1 of [the numeric widths](crate#numeric-widths): a measure in painty's own
  /// rendered geometry, which is what that rule widened to cover.
  #[inline]
  pub const fn tab_width(&self) -> u64 {
    self.tab_width
  }

  /// Returns the 1-based display column at `offset`, a byte offset into the source.
  ///
  /// Total, and clamping on the same terms as [`Line::column_at`]: an offset before the line
  /// answers `1`, an offset past its content answers one past the last cell, and an offset inside
  /// a character is measured as though it were at that character's start.
  pub fn column_at(&self, offset: usize) -> u64 {
    1 + cells(self.prefix(offset), self.tab_width)
  }

  /// Returns how many cells the whole line occupies.
  #[inline]
  pub fn width(&self) -> u64 {
    cells(self.line.text(), self.tab_width)
  }

  /// Returns how many cells the text between two byte offsets occupies.
  ///
  /// Measured as the difference of two columns rather than by measuring the slice alone, because a
  /// tab's width depends on where it starts: the same `\t` is four cells at the head of a line and
  /// one cell three characters in.
  #[inline]
  pub fn cells_between(&self, start: usize, end: usize) -> u64 {
    self.column_at(end).saturating_sub(self.column_at(start))
  }

  /// Writes the line with its tabs expanded to spaces.
  ///
  /// The expansion and the measurement are the same arithmetic, which is what keeps an underline
  /// placed by [`column_at`](Self::column_at) under the text this writes.
  pub fn write_expanded(&self, out: &mut impl fmt::Write) -> fmt::Result {
    let mut column = 0u64;
    for piece in self.line.text().split_inclusive('\t') {
      match piece.strip_suffix('\t') {
        Some(body) => {
          out.write_str(body)?;
          column += width_of(body);
          let stop = next_stop(column, self.tab_width);
          for _ in column..stop {
            out.write_char(' ')?;
          }
          column = stop;
        }
        None => {
          out.write_str(piece)?;
          column += width_of(piece);
        }
      }
    }
    Ok(())
  }

  /// The line's text up to `offset`, clamped into the line and onto a character boundary.
  fn prefix(&self, offset: usize) -> &'a str {
    let text = self.line.text();
    let base = self.line.span().start();
    let mut relative = offset.clamp(base, base + text.len()) - base;
    while !text.is_char_boundary(relative) {
      relative -= 1;
    }
    &text[..relative]
  }
}

/// The next tab stop at or after `column`.
#[inline]
const fn next_stop(column: u64, tab_width: u64) -> u64 {
  column + (tab_width - column % tab_width)
}

/// The cells `text` occupies, ignoring tabs.
///
/// Measured over the whole slice rather than character by character, because a table lookup per
/// `char` cannot see a sequence: a ZWJ emoji is several code points and one drawn glyph, and
/// summing their individual widths overstates it.
#[inline]
fn width_of(text: &str) -> u64 {
  text.width() as u64
}

/// The cells `text` occupies, with tabs advancing to the next stop.
fn cells(text: &str, tab_width: u64) -> u64 {
  let mut column = 0u64;
  for piece in text.split_inclusive('\t') {
    match piece.strip_suffix('\t') {
      Some(body) => {
        column += width_of(body);
        column = next_stop(column, tab_width);
      }
      None => column += width_of(piece),
    }
  }
  column
}
