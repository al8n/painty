use core::fmt;

use unicode_segmentation::{GraphemeIndices, UnicodeSegmentation};
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
/// `1` plus the cells of the whole grapheme clusters *before* an offset. Clusters, not characters
/// and not prefixes: a terminal draws a cluster or it draws none of it, so an offset in the middle
/// of one reports where that cluster begins. Which is where a cluster ends is UAX#29's question and
/// is asked of `unicode-segmentation` rather than inferred here — two rounds of review found two
/// different home-grown answers, and both put a marker in the wrong cell.
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
  /// Walks the same units [`column_at`](Self::column_at) does rather than re-deriving the tab
  /// arithmetic, so the two cannot drift: an underline is placed under the text this writes only
  /// because both are the same walk.
  pub fn write_expanded(&self, out: &mut impl fmt::Write) -> fmt::Result {
    let text = self.line.text();
    for unit in self.units() {
      let cluster = &text[unit.start..unit.end];
      if cluster == "\t" {
        for _ in 0..unit.cells {
          out.write_char(' ')?;
        }
      } else {
        out.write_str(cluster)?;
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
      clusters: self.line.text().grapheme_indices(true),
      tab_width: self.tab_width,
      column: 1,
    }
  }
}

/// One grapheme cluster, and the cells a terminal gives it.
///
/// # Where a unit ends is asked, not inferred
///
/// Two rounds of review found two different home-grown answers to that question, and both were
/// wrong in the same way. The first sliced the text up to an offset and measured the prefix, so
/// every interior boundary of a joined emoji measured alike and a span over one component widened
/// into the *following* character's cell. The second fixed a unit's width from its first character
/// and extended while the measured width held — which reads a joiner's zero width as a signal, and
/// so cuts any sequence whose final shaped width differs from its base. A variation selector
/// (`☃` + U+FE0F) is exactly that: one cluster, two cells, and a base that measures one. Every
/// marker after it landed a cell early.
///
/// Both derived a unit's **extent** from **incremental prefix measurement**, which is how neither
/// grapheme segmentation nor display width is defined. A heuristic that needs tuning a third time
/// is not a heuristic that needs tuning. So the extent is asked of the authority for it —
/// `unicode-segmentation`, which implements UAX#29 — for the same reason `syn` and not a line
/// scanner decides what a public item is, and each whole cluster is then measured once.
///
/// The review that found the first made the sharper point about how it survived. The invariant
/// meant to catch it cross-checked the renderer against this very layer, and both were built on
/// prefix slicing, so they agreed while the marker was visibly wrong. **A cross-check between two
/// layers is evidence only if the layers do not share the model being checked.** The numbers that
/// hold this file now are in the tests' `PLACEMENTS` table: cells a terminal paints, written by
/// hand, agreeing with no library.
///
/// # A cluster and a display unit are not defined to be the same thing
///
/// `unicode-width` applies rules over a whole string, so the sum of the clusters' widths is not
/// obliged to equal the width of the line they came from. painty measures per cluster, because a
/// marker has to start and stop at a boundary and a single number for the line cannot be
/// decomposed into the clusters it spans. [`width`](LineCells::width) is therefore that sum. The
/// two agree on every case in the corpus, and the test that says so records that this is an
/// observation about a table version rather than a guarantee.
#[derive(Debug, Clone, Copy)]
struct Unit {
  start: usize,
  end: usize,
  column: u64,
  cells: u64,
}

struct Units<'a> {
  clusters: GraphemeIndices<'a>,
  tab_width: u64,
  column: u64,
}

impl Iterator for Units<'_> {
  type Item = Unit;

  fn next(&mut self) -> Option<Unit> {
    let (start, cluster) = self.clusters.next()?;

    // A tab is the one unit whose width is not a property of its text, so it is the one this file
    // measures itself. UAX#29 puts it in a cluster of its own — a tab is `Control`, and GB4/GB5
    // break on both sides of one — so this arm never sees a tab attached to anything.
    let cells = if cluster == "\t" {
      next_stop(self.column - 1, self.tab_width) + 1 - self.column
    } else {
      width_of(cluster)
    };

    let unit = Unit {
      start,
      end: start + cluster.len(),
      column: self.column,
      cells,
    };
    self.column += cells;
    Some(unit)
  }
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
