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
/// # The model, and what it promises
///
/// > A line is its sequence of UAX#29 extended grapheme clusters, after painty's sanitization. Each
/// > cluster occupies exactly `unicode-width`'s width of that cluster **measured in isolation**. A
/// > byte offset's column is one plus the cells of the whole clusters before it. A span widens
/// > outward to cluster boundaries. Cross-cluster width adjustments — `unicode-width`'s
/// > string-level script ligatures — are **rejected by specification**, because no
/// > cursor-addressable terminal can implement them: a grid device must have a definite cursor
/// > position between any two characters it receives, and the first cluster has closed and may
/// > already have been reported past before the second arrives.
///
/// **What that promises.** painty places markers in the logical cell grid of a grapheme-aware
/// fixed-width terminal. A marker covers exactly the cells of every cluster its span touches, and
/// on a terminal that advances the cursor per grapheme cluster the marker row aligns with the
/// source row cell for cell.
///
/// **Four things it does not promise**, each a real limit rather than a gap to close later:
///
/// 1. **Agreement with `unicode-width`'s whole-string width.** Its cross-cluster script ligatures
///    describe shaped text, not a cell grid. [`width`](Self::width) is the sum of the clusters'
///    widths and is not obliged to equal `str::width` of the same line — on current data six
///    families differ, and they are pinned at both values in the tests.
/// 2. **Visual alignment under bidi reordering.** Columns are logical. A terminal that reorders an
///    RTL run moves glyphs after painty has written both rows; no column model can place a caret
///    under a visually reordered glyph. This is where every grid diagnostic tool stands.
/// 3. **Font shaping and ligation.** A font may draw `لا`, or `=>`, as one glyph across the cells
///    the grid allotted. Underlining half of such a pair marks that half's cell — deliberately: the
///    two code points are two addressable source positions, and collapsing them would erase a
///    distinction the caller's span made.
/// 4. **Legacy per-codepoint cell counts.** On wcwidth-era terminals a ZWJ emoji sequence occupies
///    more cells than its cluster width, so markers after one sit left of the glyphs there. painty
///    matches where terminals are converging, not where they have been.
///
/// Two consequences the model settles rather than leaves open: a zero-width cluster occupies no
/// cell, and a marker for it is the never-empty caret on the following unit's cell; an empty span
/// at or inside a cluster anchors to the whole of that cluster.
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

  /// Writes the line as a terminal is to draw it: tabs expanded to spaces, control characters
  /// replaced by a visible stand-in.
  ///
  /// Walks the same units [`column_at`](Self::column_at) does rather than re-deriving the tab
  /// arithmetic, so the two cannot drift: an underline is placed under the text this writes only
  /// because both are the same walk.
  ///
  /// Source text is caller-supplied and an ESC in it is an instruction to the terminal, so it does
  /// not reach one: `\x1b[38;5;196m` in a file would otherwise colour the rest of the frame. Each
  /// C0 character is written as its Control Pictures glyph — ESC reads as `␛` — DEL as `␡`, and C1,
  /// which has none, as the replacement character. A substitution and not a deletion, because a
  /// reader has to be able to see that something was there, and every stand-in is one cell so
  /// nothing placed against this line moves. It happens here rather than in the renderer because
  /// here is where the same walk measures it.
  ///
  /// Writes the whole line, however wide it is. A caller drawing its own excerpt owns the size of
  /// what it asked for; [`Terminal`](super::Terminal) bounds what IT draws instead — see
  /// [`Terminal::max_source_bytes`](super::Terminal::max_source_bytes).
  pub fn write_expanded(&self, out: &mut impl fmt::Write) -> fmt::Result {
    self.write_expanded_upto(out, self.line.text().len())
  }

  /// Everything the renderer needs about one line and every mark on it, in a single walk that
  /// stops at `budget`.
  ///
  /// # Why this is one function and not three
  ///
  /// The obvious shape — ask [`columns_for`](Self::columns_for) for the exact span columns, then
  /// clip them to a separately computed stop — does work proportional to the WHOLE LINE for a span
  /// that is not going to be drawn at all. A span starting past the ceiling walked twenty million
  /// units to place one caret under an elision mark. So the budget is applied to the walk rather
  /// than to its result.
  ///
  /// Four things come out together because they have to agree. The stop is not the ceiling — a unit
  /// is drawn whole or not at all, and a tab can be 256 cells — the elision mark sits at
  /// `visible + 1`, and the marker row is placed against that same column. Two callers each
  /// deciding where the row ended would put the caret past the row.
  ///
  /// # Why it is one walk and not one per mark
  ///
  /// A line is DRAWN once, so it is WALKED once. Placing k marks by calling this k times would
  /// segment the window k times, and — the sharper half — would take the row's stop from whichever
  /// call the caller happened to keep. That the k answers agree today is a property of this
  /// implementation rather than of the type: the stop depends only on the budget, and a later edit
  /// that made it depend on the span would put the carets against a row nobody drew. One call
  /// produces one [`Row`] and every mark is placed against it, so there is nothing to agree about.
  ///
  /// The marks come in and their columns go out **on the same values**, for the same reason. A
  /// slice of spans in and a slice of columns out would have to be lined up by index, and an index
  /// that has to line up is a rule the type is not making.
  ///
  /// # The stop is a BYTE OFFSET, and that is the point
  ///
  /// [`drawn_end`](Row::drawn_end) is what the writer replays to, rather than a cell count it
  /// re-derives a stop from. Handing it a cell count is what let zero-width clusters through: a run
  /// of U+200B advances no cells, so a cell check never trips, and the writer walked and emitted the
  /// whole line while the geometry thought it had stopped. Two walks with different stopping rules
  /// is how that class recurs, so there is one rule and it produces one number.
  ///
  /// Inside the window the answer is [`columns_for`](Self::columns_for)'s, exactly; the tests hold
  /// the two together over the corpus so that bounding the walk cannot quietly change the geometry.
  pub(crate) fn place_marks<T>(&self, marks: &mut [Mark<T>], budget: Budget) -> Row {
    // Clamped here rather than where a mark was built, so a mark carries no line of its own and
    // cannot have been measured against a different one.
    for mark in marks.iter_mut() {
      mark.within = self.clamp(mark.covered.start())..self.clamp(mark.covered.end());
      mark.first = None;
      mark.last = None;
      mark.anchor = None;
    }

    // Two budgets, and the second is the one that matters. Cells bound the LAYOUT — where the
    // elision mark goes — and are blind to a unit that occupies none: a run of U+200B or of
    // combining marks advances zero cells for as many bytes as the caller cares to supply, so a
    // cell check alone never trips. Bytes bound the WORK, and every input that has cost anything
    // over five rounds of review had first to be supplied as bytes.
    //
    // The byte budget is spent by SLICING rather than by checking, because a check runs after the
    // segmenter has already been over the cluster it rejects. One eight-megabyte grapheme cluster
    // is one unit, so the check fired once — having read all eight megabytes to get there, against
    // a budget of sixty-four kilobytes.
    let (within, whole) = self.capped(budget.bytes);
    let mut drawn = 0;
    let mut drawn_end = 0;
    // Nothing past the cap will be drawn, so a cap that cut the line is an elision already.
    let mut elided = !whole;
    for unit in Units::new(within, self.tab_width) {
      // A subtraction against the cell budget rather than `drawn + unit.cells > cells`, which
      // overflows for an unbounded caller. `drawn` never passes it, so this cannot.
      //
      // The last unit of a CUT slice is dropped, because the cluster it came from may continue past
      // the cap and half a cluster is not something a terminal can draw. Conservative in the one
      // direction that is safe: a cluster that happened to end exactly at the cap is elided a unit
      // early, on a row that is being elided anyway.
      if budget.cells - drawn < unit.cells || (!whole && unit.end == within.len()) {
        elided = true;
        break;
      }
      for mark in marks.iter_mut() {
        let (start, end) = (mark.within.start, mark.within.end);
        // `columns_for`'s predicate, so the two cannot answer differently inside the window.
        let touches = unit.start < end || (unit.start <= start && start < unit.end);
        if touches && unit.end > start {
          mark.first.get_or_insert(unit.column);
          mark.last = Some(unit.column + unit.cells);
        }
        // What `column_at` would answer for `start`, carried along rather than fetched by a second
        // walk when the span turns out to touch nothing.
        if mark.anchor.is_none() && start < unit.end {
          mark.anchor = Some(unit.column);
        }
      }
      drawn += unit.cells;
      drawn_end = unit.end;
    }

    let stop = drawn + 1;
    for mark in marks.iter_mut() {
      // A span running past the drawn text reaches the elision mark, so it is drawn over it: that
      // is what tells a reader the span continues out there. Without this the underline would stop
      // at the last drawn cell and claim the span ended with the row.
      if elided && mark.within.end > drawn_end {
        mark.first.get_or_insert(stop);
        mark.last = Some(stop + 1);
      }
      mark.columns = match (mark.first, mark.last) {
        (Some(from), Some(to)) => from..to,
        _ => {
          let at = mark.anchor.unwrap_or(stop);
          at..at
        }
      };
    }

    Row { drawn_end, elided }
  }

  /// The same walk as [`write_expanded`](Self::write_expanded), replaying to a byte offset.
  ///
  /// Takes [`Row::drawn_end`] — the offset the geometry walk already stopped at — rather than a
  /// budget it would have to re-derive a stop from. There is one stopping rule and this is not it;
  /// this only replays the decision, so the row drawn and the row measured cannot disagree about
  /// where it ended whatever the budget was denominated in.
  ///
  /// Replayed over the SLICE and not over the line with a check inside the loop, for the reason
  /// [`place_marks`](Self::place_marks) slices: a check sees a cluster only after the segmenter has
  /// read it, and the cluster starting at a stop can be the whole rest of the file. `drawn_end` is
  /// a cluster boundary of the line, so the slice segments to the same units the geometry measured.
  pub(crate) fn write_expanded_upto(
    &self,
    out: &mut impl fmt::Write,
    byte_end: usize,
  ) -> fmt::Result {
    let text = self.upto(byte_end);
    for unit in Units::new(text, self.tab_width) {
      let cluster = &text[unit.start..unit.end];
      // Before the control arm, and that ORDER is the tab's whole exception. `control_picture` has
      // a picture for a tab like every other C0 character, so reaching it first would draw one `␉`
      // where `unit.cells` cells were counted — a marker misplaced by the width of a stop. A tab is
      // a device unit here and a glyph nowhere else, so it is spent here and the lookup stays
      // unconditional for everyone who has no stop to spend it against.
      if cluster == "\t" {
        for _ in 0..unit.cells {
          out.write_char(' ')?;
        }
      } else if holds_a_control(cluster) {
        for character in cluster.chars() {
          out.write_char(control_picture(character).unwrap_or(character))?;
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

  /// The line's text up to a line-relative offset, moved back to a character boundary.
  ///
  /// Total rather than slicing whatever it is handed: a `&str` cannot be cut inside a character,
  /// and a renderer that panics on an offset is a diagnostic lost.
  fn upto(&self, byte_end: usize) -> &'a str {
    let text = self.line.text();
    let mut end = byte_end.min(text.len());
    while !text.is_char_boundary(end) {
      end -= 1;
    }
    &text[..end]
  }

  /// The most of this line that `bytes` allows to be looked at, and whether that is all of it.
  ///
  /// The budget is denominated in bytes of the line, so it is spent by handing the segmenter fewer
  /// of them. A cap that falls inside a character moves back to its boundary; a cap that falls
  /// inside a grapheme CLUSTER cannot be seen from here, which is why the caller drops the last
  /// unit of a cut slice.
  fn capped(&self, bytes: u64) -> (&'a str, bool) {
    let text = self.line.text();
    let cap = usize::try_from(bytes).unwrap_or(usize::MAX);
    (self.upto(cap), cap >= text.len())
  }

  /// The placement units of this line, left to right.
  fn units(&self) -> Units<'a> {
    Units::new(self.line.text(), self.tab_width)
  }
}

/// What one excerpt may spend.
///
/// # Two budgets, because one resource was the wrong denomination
///
/// Four rounds of review each bounded the resource the previous round's defect had consumed, and
/// each was bypassed through one it had not: rows were materialised, then streamed, then streamed
/// without limit, then limited in cells — and a zero-width cluster walks past a cell limit without
/// touching it.
///
/// `cells` is a **layout rule**. It decides how wide a row is drawn and where the elision mark
/// goes. It is not a bound on anything, because a unit can cost bytes and time while costing no
/// cells at all.
///
/// `bytes` is the **resource bound**, and it is denominated in the one thing an adversary must
/// spend to reach any of the others. Tabs, long lines, off-window spans, zero-width runs and
/// combining sequences all have to be supplied as source bytes first, so a bound here is not one
/// more patch against the class that was just found — it is upstream of the class.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Budget {
  /// How many cells a row may occupy before it is cut and marked.
  pub(crate) cells: u64,
  /// How many bytes of the line may be examined at all.
  pub(crate) bytes: u64,
}

/// One span to be marked on a line, whatever the caller needs to remember about it, and the display
/// columns [`LineCells::place_marks`] gave it.
///
/// One value carries the question and the answer. Handing the walk a slice of spans and taking a
/// slice of columns back would leave the two to be paired by index, and every marker row on the
/// line would then be one silent off-by-one away from pointing at the wrong text.
#[derive(Debug, Clone)]
pub(crate) struct Mark<T> {
  /// The bytes to mark, as the caller knows them: absolute, and not yet clamped to any line.
  covered: Span,
  /// Those bytes as the walk compares them — line-relative and clamped in. Set by
  /// [`LineCells::place_marks`], which is the only thing that knows which line this is against.
  within: core::ops::Range<usize>,
  /// Where the mark's first and last drawn units sat, while the walk is running.
  first: Option<u64>,
  last: Option<u64>,
  /// What [`LineCells::column_at`] would answer for the start, for a mark that touches no drawn
  /// unit at all.
  anchor: Option<u64>,
  /// What the walk settled on. Empty at column zero — a column no line has — until it has run.
  columns: core::ops::Range<u64>,
  payload: T,
}

impl<T> Mark<T> {
  /// A mark over `covered`, not yet placed.
  #[inline]
  pub(crate) const fn new(covered: Span, payload: T) -> Self {
    Self {
      covered,
      within: 0..0,
      first: None,
      last: None,
      anchor: None,
      columns: 0..0,
      payload,
    }
  }

  /// Returns what the caller attached to it.
  #[inline]
  pub(crate) const fn payload(&self) -> &T {
    &self.payload
  }

  /// Returns the display columns the mark occupies, already inside the drawable window.
  #[inline]
  pub(crate) fn columns(&self) -> core::ops::Range<u64> {
    self.columns.clone()
  }
}

/// Where one line's drawn row stopped, and whether anything was left over.
///
/// One [`Row`] per walk and one walk per line, so every mark on the line is placed against the same
/// stop — see [`LineCells::place_marks`].
pub(crate) struct Row {
  /// The byte offset the walk stopped at: the only thing the writer is told, and the only unit in
  /// which the stop is expressed anywhere. A cell count was here too and nothing production read
  /// it — carrying the stop in two units is how the walk and the row came to disagree.
  pub(crate) drawn_end: usize,
  /// Whether the budget cut the line short.
  pub(crate) elided: bool,
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
/// # Why the string-level rules are rejected, and not merely unimplemented
///
/// `unicode-width` also applies rules **across** cluster boundaries — Arabic lam followed by alef
/// scores 1 for the pair where the clusters score 1 + 1 — and painty does not. That is a decision,
/// with a reason no future review should have to rediscover.
///
/// **No cursor-addressable terminal can implement them.** A grid terminal must have a definite
/// cursor position between any two characters it receives: `CSI 6n` can be issued between the lam
/// and the alef, and the lam's cell can be painted, wrapped past, or addressed before the alef
/// arrives. The lam's cluster *closes* when the alef arrives, because the alef starts a new one.
/// For the pair to occupy one cell the alef would have to advance zero cells into a cell the
/// terminal may already have reported past — cursor regression against the device's own report.
/// Legacy per-codepoint terminals advance 1 + 1 here too. Terminals that visually ligate Arabic do
/// it by glyph substitution *inside* the two cells the grid already allotted, exactly as a font
/// draws `=>` as one glyph across two cells.
///
/// So the cluster boundary is not a convenience. It is the **maximum lookahead a cursor-addressable
/// device can hold without contradicting its own cursor reports**, which is why it is the unit.
///
/// Two further reasons, either sufficient on its own. Adopting the string model would not fix
/// placement, it would *remove* it: a column is a prefix sum, and a non-compositional width
/// function has no fact of the matter about where a string's interior is. And a rule that coalesced
/// clusters when the concatenation measures narrower would infer structure from a width
/// measurement — the exact shape of the two defects above — over a rule set `unicode-width`'s own
/// documentation calls string-only exceptions that "may be tweaked in the future".
///
/// Six families diverge on the Unicode 17 data both crates carry: Arabic lam-alef, Hebrew
/// alef-ZWJ-lamed, Buginese, Lisu, Old Turkic and Tifinagh. That list is a **survey result**, and
/// the suite does not recompute it — which is worth stating exactly, because "the divergence set is
/// pinned" is true of three different things to three different extents:
///
/// * **A change to one of the six**: caught. Each is asserted at both its clustered and its
///   whole-string width, so a rule that is dropped, reshaped, or moved under a shifted boundary
///   fails with the input in hand.
/// * **A seventh family**: *not* caught by those pins, and caught by the corpus only if its input
///   happens to be in it. Enumerating the set instead would mean measuring strings, since
///   `unicode-width` exposes only `UNICODE_VERSION` and two sealed traits — its rules are a private
///   state machine, not a table anything can read — and the pairs alone are the square of the code
///   space, before the Arabic rule's arbitrarily many interposed transparents. Restricting the
///   search to plausible scripts would re-derive the rule list being checked, so finding no seventh
///   family would be a measurement of the restriction rather than evidence about the set.
/// * **The trigger**: pinned instead of the result. A Unicode release is what adds a script
///   ligature rule, and the data version is asserted at a literal, so the update stops the suite
///   and asks for the survey to be run again. The residual gap is a `unicode-width` release that
///   changes a rule on unchanged data — its own documentation reserves that right.
///
/// Khmer coeng is one sample of the same question, kept because it has already moved once: it is
/// listed among those rules and agrees on this data.
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

impl<'a> Units<'a> {
  /// The units of `text`, which is a whole line or as much of one as a budget allows.
  fn new(text: &'a str, tab_width: u64) -> Self {
    Self {
      clusters: text.grapheme_indices(true),
      tab_width,
      column: 1,
    }
  }
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
      // Control characters are measured as they are, not as the stand-ins that will be written in
      // their place, and that is safe only because the two are the same width: the table gives
      // every control character one cell and every stand-in is one cell. A branch here that
      // measured the substitution instead would be unreachable — no input can tell the two apart —
      // so the property is asserted in the tests rather than defended by code no plant can kill.
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

/// The visible stand-in for a control character, or `None` for anything drawable as it is.
///
/// A control character in caller-supplied text is an instruction to the terminal, and a renderer
/// whose whole promise is deciding what the terminal is told cannot forward one unread. ESC is the
/// sharp case — `\x1b[38;5;196m` sitting in a source file would colour the rest of the frame, and
/// would do it under [`ColorCapability::None`](super::ColorCapability::None), which is exactly the
/// guarantee the capability gate exists to make. It is not the only one: U+009B is a single-byte
/// CSI on terminals that honour C1, and a bare newline needs no escape at all to break the frame.
///
/// Shown rather than swallowed. A reader has to be able to tell that something was there, so C0
/// becomes its Control Pictures glyph — ESC reads as `␛` — DEL becomes `␡`, and C1, which has no
/// pictures, becomes the replacement character.
///
/// Every stand-in is one cell and one grapheme cluster, and the width table gives every control
/// character one cell too, so the substitution moves nothing. That is asserted rather than assumed:
/// a marker's column is compared across a line with a control character and one without.
///
/// Tab included, and deliberately so — a `None` for it here would be a defect rather than a
/// refinement. `␉` is the standard picture for U+0009 as much as `␛` is for ESC. What makes a tab
/// different is not the character but one of its callers: SOURCE text expands a tab against a stop
/// instead of drawing it, so [`write_expanded`](LineCells::write_expanded) matches the tab cluster
/// before it consults this and [`Units`] prices it. The exception belongs to the caller that has a
/// stop, not to the lookup that every caller shares.
///
/// Sited here instead, it cost the guarantee above. Text that is not source — a message, a code, an
/// origin, a label, a help line, a caller's own [`fmt::Display`] — has no stop to expand against and
/// reaches the terminal through a sanitizer that asks this and nothing else, so a live C0 cursor
/// movement went into the frame at every capability. A lookup that answers "what is this
/// character's picture" cannot also answer "does this caller expand it".
pub(crate) fn control_picture(character: char) -> Option<char> {
  /// The Control Pictures block, U+2400 upwards, in code point order — so the picture for a C0
  /// character is at its own index. Written out rather than computed as `0x2400 + n`, because that
  /// arithmetic needs a `u32` and the numeric rule places every integer in `src/`; a scalar value
  /// is `char`'s business and not one of painty's numbers. A test walks the table against
  /// `char::from_u32` so the transcription is checked rather than trusted.
  const C0: [char; 32] = [
    '␀', '␁', '␂', '␃', '␄', '␅', '␆', '␇', '␈', '␉', '␊', '␋', '␌', '␍', '␎', '␏', '␐', '␑', '␒',
    '␓', '␔', '␕', '␖', '␗', '␘', '␙', '␚', '␛', '␜', '␝', '␞', '␟',
  ];
  match character {
    '\0'..='\u{1f}' => C0.get(character as usize).copied(),
    '\u{7f}' => Some('\u{2421}'),
    '\u{80}'..='\u{9f}' => Some('\u{fffd}'),
    _ => None,
  }
}

/// Whether any character in `cluster` needs a stand-in.
///
/// Measurement and writing consult this same predicate, which is what keeps the cells counted and
/// the characters written in agreement.
fn holds_a_control(cluster: &str) -> bool {
  cluster.chars().any(|c| control_picture(c).is_some())
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
