use core::fmt;

use unicode_width::UnicodeWidthStr;

use crate::{Line, Span};

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
  /// The tab width is caller-supplied and multiplies into every later column, so it is bounded at
  /// both ends rather than trusted. Zero would leave a tab advancing nowhere. The ceiling is
  /// [`max_tab_width`](Self::max_tab_width): an unbounded width overflows the column arithmetic —
  /// `u64::MAX` panicked here — and makes [`write_expanded`](Self::write_expanded) emit that many
  /// spaces, which is a hang rather than a wide tab.
  #[inline]
  #[must_use]
  pub const fn new(line: Line<'a>, tab_width: u64) -> Self {
    Self {
      line,
      tab_width: if tab_width == 0 {
        1
      } else if tab_width > Self::max_tab_width() {
        Self::max_tab_width()
      } else {
        tab_width
      },
    }
  }

  /// The furthest apart tab stops may be placed.
  ///
  /// Not a taste: the width is caller-supplied and multiplies into every column, so it needs a
  /// ceiling for the arithmetic to be total. Wider than any terminal anybody uses, and small
  /// enough that expanding a line of tabs cannot become a hang. A function rather than a
  /// `pub const` for the reason [`default_tab_width`](Self::default_tab_width) is one.
  #[inline]
  #[must_use]
  pub const fn max_tab_width() -> u64 {
    256
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

  /// Returns the 1-based display column where `offset`'s **placement unit** begins.
  ///
  /// Total, and clamping: an offset before the line answers `1`, an offset past its content answers
  /// one past the last cell, and an offset inside a unit answers where that unit starts, because
  /// that is where a terminal draws the whole of it.
  pub fn column_at(&self, offset: usize) -> u64 {
    let offset = self.clamp(offset);
    let mut last = 1;
    for unit in self.units() {
      if offset < unit.end {
        return unit.column;
      }
      last = unit.column + unit.cells;
    }
    last
  }

  /// Returns how many cells the whole line occupies.
  #[inline]
  pub fn width(&self) -> u64 {
    self
      .units()
      .last()
      .map_or(0, |unit| unit.column + unit.cells - 1)
  }

  /// Returns the display columns a span must be marked at, widened to whole units.
  ///
  /// A span reaching into a unit is widened to the whole of it, because a terminal has no way to
  /// draw part of one. Without that, a span over an interior component of a joined emoji has equal
  /// start and end columns, and the marker lands on whatever follows the cluster.
  pub fn columns_for(&self, span: Span) -> core::ops::Range<u64> {
    let start = self.clamp(span.start());
    let end = self.clamp(span.end());
    let mut first = None;
    let mut last = None;
    for unit in self.units() {
      // Half-open, except that a unit exactly at an empty span's offset still anchors it.
      let touches = unit.start < end || (unit.start <= start && start < unit.end);
      if touches && unit.end > start {
        first.get_or_insert(unit.column);
        last = Some(unit.column + unit.cells);
      }
    }
    match (first, last) {
      (Some(from), Some(to)) => from..to,
      _ => {
        let at = self.column_at(start);
        at..at
      }
    }
  }

  /// Returns how many cells the text between two byte offsets occupies, widened to whole units.
  #[inline]
  pub fn cells_between(&self, start: usize, end: usize) -> u64 {
    let columns = self.columns_for(Span::new(start, end));
    columns.end - columns.start
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

  /// An absolute byte offset, clamped into the line and back onto a character boundary.
  fn clamp(&self, offset: usize) -> usize {
    let text = self.line.text();
    let base = self.line.span().start();
    let mut relative = offset.clamp(base, base + text.len()) - base;
    while !text.is_char_boundary(relative) {
      relative -= 1;
    }
    relative
  }

  /// The placement units of this line, left to right.
  fn units(&self) -> Units<'a> {
    Units {
      text: self.line.text(),
      tab_width: self.tab_width,
      at: 0,
      column: 1,
    }
  }
}

/// One run of characters a terminal places together.
///
/// # Why units and not prefix slices
///
/// This measured a display column by slicing the text up to an offset and asking its width. That
/// is a different model from the one a terminal uses, and the two agree only on simple input: for
/// a joined emoji, every interior boundary measures the same width, so a span over one component
/// produced an empty range that widened into the *following* character's cell.
///
/// The review that found it made the sharper point. The invariant meant to catch this cross-checked
/// the renderer against this very layer — and both were built on prefix slicing, so they agreed
/// while the marker was visibly wrong. **A cross-check between two layers is evidence only if the
/// layers do not share the model being checked.** The oracle for these units is therefore written
/// to a different shape in the tests, and deliberately a cruder one.
///
/// A unit is found by asking the width table, not by recognising a pattern: characters are added
/// while doing so does not change the measured width. That makes a joined sequence one unit and a
/// combining mark part of its base's, without this file holding any opinion about which code points
/// join.
#[derive(Debug, Clone, Copy)]
struct Unit {
  start: usize,
  end: usize,
  column: u64,
  cells: u64,
}

struct Units<'a> {
  text: &'a str,
  tab_width: u64,
  at: usize,
  column: u64,
}

impl Iterator for Units<'_> {
  type Item = Unit;

  fn next(&mut self) -> Option<Unit> {
    let start = self.at;
    if start >= self.text.len() {
      return None;
    }

    if self.text[start..].starts_with('\t') {
      let stop = next_stop(self.column - 1, self.tab_width) + 1;
      let unit = Unit {
        start,
        end: start + 1,
        column: self.column,
        cells: stop - self.column,
      };
      self.at = unit.end;
      self.column = stop;
      return Some(unit);
    }

    let mut end = boundary_after(self.text, start);
    let cells = width_of(&self.text[start..end]);
    // Extend while the next character adds nothing, which is what makes a joined sequence and its
    // joiners one unit without this code knowing what a joiner is.
    while end < self.text.len() && !self.text[end..].starts_with('\t') {
      let next = boundary_after(self.text, end);
      if width_of(&self.text[start..next]) != cells {
        break;
      }
      end = next;
    }

    let unit = Unit {
      start,
      end,
      column: self.column,
      cells,
    };
    self.at = end;
    self.column += cells;
    Some(unit)
  }
}

/// The next character boundary strictly after `at`.
fn boundary_after(text: &str, at: usize) -> usize {
  let mut next = at + 1;
  while next < text.len() && !text.is_char_boundary(next) {
    next += 1;
  }
  next.min(text.len())
}

/// The next tab stop at or after `column`, counting from zero.
#[inline]
const fn next_stop(column: u64, tab_width: u64) -> u64 {
  column + (tab_width - column % tab_width)
}

/// The cells `text` occupies, ignoring tabs.
///
/// Measured over the whole slice rather than character by character, because a table lookup per
/// `char` cannot see a sequence: a joined emoji is several code points and one drawn glyph, and
/// summing their individual widths overstates it.
#[inline]
fn width_of(text: &str) -> u64 {
  text.width() as u64
}
