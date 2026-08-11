//! The seam between the two styles, and nothing more than the seam.
//!
//! # This was derived, not designed
//!
//! Every method below exists because [`Rustc`](super::rustc::Rustc) and
//! [`Miette`](super::miette::Miette) were both written out in full first and then differed there.
//! Each carries the divergence that forced it. A method whose two implementations agreed is not a
//! method: the margin's slots, the source text, the cell arithmetic, the elision mark and the
//! whole of [`Plan`](super::render::Plan) came out identical and stayed in the renderer.
//!
//! That is the design's rule for this phase, and it is worth restating why. A trait written from
//! three prose descriptions of three renderers would have fifteen methods, most of them
//! unconsumed, and would be wrong in the places prose is vague — which is exactly where a
//! renderer is not.
//!
//! # It is crate-private, and that is a decision
//!
//! Phase 3 is HTML, and it is the FOURTH example — the one the design says is most likely to
//! collapse a trait fitted to terminals. So every method below was checked against it before this
//! was frozen, and the answer is that **most of them do not survive**:
//!
//! | method | can HTML answer it? |
//! |---|---|
//! | [`header`](Presentation::header) | yes |
//! | [`open_block`](Presentation::open_block) | yes, ignoring `gutter` |
//! | [`close_block`](Presentation::close_block) | yes, ignoring `gutter` |
//! | [`help`](Presentation::help) | yes, ignoring `gutter` |
//! | [`opens_in_margin`](Presentation::opens_in_margin) | yes, but vacuously — see below |
//! | [`line_field`](Presentation::line_field) | **no** |
//! | [`elision_row`](Presentation::elision_row) | **no** |
//! | [`bracket`](Presentation::bracket) | **no** |
//! | [`whole`](Presentation::whole) | **no** |
//! | [`opens`](Presentation::opens) | **no** |
//! | [`closes`](Presentation::closes) | **no** |
//!
//! The six failures have ONE shape, which is what makes this a finding rather than a list.
//! Everything that positions or draws a mark is denominated in **display cells** and in **`char`**:
//! `whole` takes a `Range<u64>` of cells, `opens` and `closes` take a cell, `bracket` returns a
//! character to stand in a cell, and `line_field` and `elision_row` pad to a cell width. HTML has
//! no cell. Its mark is an element wrapped around the span's BYTES, its bracket is a border or a
//! pseudo-element rather than a glyph, and its alignment is the browser's rather than something
//! painted one column at a time. An HTML style could return `Some('│')` and pad with `&nbsp;`, but
//! that is emulating a terminal in a medium that has real boxes.
//!
//! §4 of the design already draws that line — resolution reports *character* columns and the
//! terminal converts to *display* columns — and this trait sits **below** the conversion, because
//! [`Terminal::render`](super::Terminal::render) has already turned every span into cells before a
//! style is called. So `Presentation` is a terminal trait by construction, and the honest reading
//! of the two that pass on a technicality is worth stating too: three of the four ignore `gutter`,
//! and `opens_in_margin` survives only because HTML's answer is a constant — its inputs are
//! whether a CELL budget cut the row and whether the source is indented, and a browser never cuts
//! a row.
//!
//! **So Phase 3 is a peer of [`Terminal`](super::Terminal), not an implementor of this.** What the
//! two outputs share is [`Plan`](super::render::Plan) — which lines are drawn, where each span's
//! ends fall, which bracket runs where, what is elided — and a plan is stated in lines and byte
//! spans, which both media have. If a medium-independent version of this trait is ever wanted, the
//! change it needs is already visible: hand a style a `RegionLine`, which carries the line and the
//! byte span it covers, instead of a `Range<u64>` of cells, and a semantic bracket value instead of
//! a `char`. That is a bigger claim than two terminal styles can support, which is why it is not
//! being made here.
//!
//! What a caller gets instead of a trait is the choice —
//! [`Terminal::like_rustc`](super::Terminal::like_rustc) and
//! [`Terminal::like_miette`](super::Terminal::like_miette) — which is the whole of what the design
//! asked for, and which can grow into a public trait later without breaking anything.

use core::{fmt, panic::RefUnwindSafe};

use super::{
  paint::Painter,
  render::{Block, Phrase},
};
use crate::{Diagnostic, Role};

/// Which part of a bracket one row shows.
///
/// The renderer decides this from the line — it is arithmetic over the connector's two ends — and
/// the style decides only what to draw for it. Both parts of that split are load-bearing: a style
/// that worked out for itself which line was which could disagree with the plan about where a span
/// ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Part {
  /// The source row the span opens on.
  Opens,
  /// A row between its two ends, and every annotation row under one of them.
  Runs,
  /// The source row it closes on.
  Closes,
}

/// What a style is told about the line a multi-line span opens on.
///
/// Facts, not a recommendation, and each independently true — which costs something the
/// short-circuited conjunction this replaced did not pay. See [`Onset::at_first_nonblank`].
///
/// # Both facts are about the span and its own line, and that is the point
///
/// There was a third — whether the span's opening was the only thing marked on that line — and
/// removing it is what makes `which_cells_are_marked_is_a_function_of_the_span_alone` statable in
/// full. While it was here, adding an unrelated label to a line could change whether a DIFFERENT
/// span's opening cell was marked: a property saying otherwise would have failed, and one written
/// around it would have been the gate weakened to pass.
///
/// Dropping it is only sound because the two that remain are enough to make an unmarked opening
/// **recoverable**, and it was not sound while the second of them said merely that the span's
/// prefix was blank — see [`at_first_nonblank`](Self::at_first_nonblank) for the case that
/// falsified it and for what a row with no marker on it is able to name.
#[derive(Debug, Clone, Copy)]
pub(super) struct Onset {
  drawn: bool,
  at_first_nonblank: bool,
}

impl Onset {
  pub(super) const fn new(drawn: bool, at_first_nonblank: bool) -> Self {
    Self {
      drawn,
      at_first_nonblank,
    }
  }

  /// Whether the row, as it will be drawn, shows the cell the span opens at.
  ///
  /// A row is cut at [`Terminal::max_rendered_width`](super::Terminal::max_rendered_width) cells,
  /// so an opening five thousand spaces into a line is out past the `…`. A style that suppressed
  /// its marker on the strength of a margin glyph would leave nothing at all pointing into the
  /// row.
  pub(super) const fn drawn(&self) -> bool {
    self.drawn
  }

  /// Whether the span begins at the first non-blank of that line — the one cell an unmarked row is
  /// able to name.
  ///
  /// # It used to say only that the prefix was blank, and that names a set rather than a cell
  ///
  /// A style that suppresses a span's start marker leaves the row to say where the span begins,
  /// and a row carrying no marker offers a reader exactly one position: the first cell of the line
  /// that holds something. Every column of the indentation satisfies "nothing but blanks precedes
  /// it", and every one of them decodes to that same cell, so the weaker fact let an opening
  /// inside the indentation be announced as an opening at the text.
  ///
  /// Two of them on one line is where that stopped being recoverable at all — both compacted, both
  /// drew a glyph in the margin, and neither marked a cell, so two spans at different columns
  /// produced identical bytes. `an_opening_the_row_cannot_name_is_marked_rather_than_compacted`
  /// is that case.
  ///
  /// **Bounded here rather than by the caller's `&&`.** This used to be the last term of a
  /// short-circuited conjunction whose previous term was [`drawn`](Self::drawn), and that ordering
  /// was the only thing bounding it: "is this indentation" is a scan over as many bytes as a
  /// caller cares to indent with. Handing a style a set of FACTS means computing them all, so the
  /// scan is now bounded by the same byte budget the geometry walk spends. Nothing observable
  /// changes — an opening the row drew is inside that budget by construction — and the fact is
  /// true on its own instead of true given another one.
  pub(super) const fn at_first_nonblank(&self) -> bool {
    self.at_first_nonblank
  }
}

/// The fixed geometry every row of one block is drawn against: how wide the line-number field is,
/// how many connector columns the input needs, and what is standing in them on this row.
///
/// One value rather than three parameters, because the three are read together on every row and a
/// caller pairing them by position is a caller that can pair them wrongly.
#[derive(Debug, Clone, Copy)]
pub(super) struct Frame<'m> {
  gutter: u64,
  depth: u64,
  margin: &'m [Option<(char, Role)>],
}

impl<'m> Frame<'m> {
  pub(super) const fn new(gutter: u64, depth: u64, margin: &'m [Option<(char, Role)>]) -> Self {
    Self {
      gutter,
      depth,
      margin,
    }
  }

  /// How many cells the line-number field takes.
  pub(super) const fn gutter(&self) -> u64 {
    self.gutter
  }

  /// How many connector columns this input needs, and zero when it has none.
  pub(super) const fn depth(&self) -> u64 {
    self.depth
  }

  /// Every connector column of this row, and the blank that separates them from the source.
  ///
  /// Nothing at all when the input has no bracket, which is what keeps such an input flush against
  /// the gutter.
  pub(super) fn margin(&self, paint: &mut Painter<'_>) -> fmt::Result {
    if self.margin.is_empty() {
      return Ok(());
    }
    paint.margin(self.margin)?;
    paint.frame_char(' ')
  }

  /// The connector columns left of `depth`'s own, without the blank that would follow them.
  ///
  /// Everything from that column rightwards belongs to the corner the style is about to draw.
  pub(super) fn margin_left_of(&self, paint: &mut Painter<'_>, depth: u64) -> fmt::Result {
    let own = super::render::slot(depth)
      .saturating_sub(1)
      .min(self.margin.len());
    paint.margin(&self.margin[..own])
  }

  /// The connector columns worth writing on a row that says nothing after the last of them, or
  /// `None` when that is all of them.
  ///
  /// Trailing blanks are dropped, so an elision row ends where it stops saying anything.
  pub(super) fn occupied(&self) -> Option<&'m [Option<(char, Role)>]> {
    let last = self.margin.iter().rposition(Option::is_some)?;
    Some(&self.margin[..=last])
  }
}

/// How one style draws what the plan worked out.
///
/// Object-safe on purpose. The renderer holds a `&'static dyn Presentation`, so the choice is a
/// value rather than a type parameter: a caller reading `--style` out of a flag keeps one
/// `Terminal` type, and nothing in this crate's public signatures mentions the trait.
///
/// # `Sync + RefUnwindSafe` are the two an `&'static` forwards
///
/// A trait object carries only the auto traits its trait names, and this one sits in a field of
/// [`Terminal`](super::Terminal), which is public. Bounded by `Debug` alone it took `Send`, `Sync`,
/// `UnwindSafe` and `RefUnwindSafe` off every `Terminal` — a compatibility break for a caller that
/// shares one configured renderer between worker threads, and for one that renders inside
/// `catch_unwind`.
///
/// These two are what fix all four rather than the two that name them: `&T` is `Send` when `T` is
/// `Sync`, and `&T` is both `UnwindSafe` and `RefUnwindSafe` when `T` is `RefUnwindSafe`. Adding
/// `Send` or `UnwindSafe` here would constrain implementors to buy nothing, because the renderer
/// never owns a `Presentation` and never moves one. What the pair asks of an implementor is what a
/// presentation already is: an immutable `'static` value with no interior mutability and no thread
/// affinity. The set is pinned where the field is — see the assertion under
/// [`Terminal`](super::Terminal) — so the next field of this kind fails the build instead of a
/// caller's.
pub(super) trait Presentation: fmt::Debug + Sync + RefUnwindSafe {
  /// The severity, the code and the message.
  ///
  /// **Diverges:** `error[code]: message` on one line, against the code on a line of its own and
  /// then a severity GLYPH — `×` — where the other writes the word.
  fn header(&self, paint: &mut Painter<'_>, diagnostic: &Diagnostic<'_>) -> fmt::Result;

  /// Everything before an input's first source row.
  ///
  /// **Diverges:** two rows against one. `-->` names the location and a bar then opens the
  /// excerpt; a boxed style has a box top, and where the excerpt came from is what the top says.
  fn open_block(&self, paint: &mut Painter<'_>, gutter: u64, block: &Block<'_>) -> fmt::Result;

  /// Whatever closes an input's block.
  ///
  /// **Diverges:** a bare bar against the bottom of the box.
  fn close_block(&self, paint: &mut Painter<'_>, gutter: u64) -> fmt::Result;

  /// What stands before a source row's text: the number, and the wall beside it.
  ///
  /// The renderer writes the margin and the text; only the field is a style's, and the two styles
  /// differ in it by one cell of inset and by which character the wall is.
  ///
  /// **Diverges:** `NN | ` against ` NN │ `.
  fn line_field(&self, paint: &mut Painter<'_>, gutter: u64, number: u64) -> fmt::Result;

  /// The row standing for the lines left out between two excerpts.
  ///
  /// **Diverges:** `...` in the NUMBER's column, which says numbers are missing, against `⋮` in
  /// the WALL's, which says rows are. A style with a wall has somewhere to say it.
  fn elision_row(&self, paint: &mut Painter<'_>, frame: Frame<'_>) -> fmt::Result;

  /// What stands in a bracket's column on one row, and nothing when the bracket draws none there.
  ///
  /// **Diverges:** three answers against two. One style says where a span CLOSES on the source row
  /// itself, so the row under it carries only the label; the other says it on a row of its own
  /// that reaches back into the source, and so has nothing to put here but the running bar.
  fn bracket(&self, part: Part, role: Role, compact: bool) -> Option<char>;

  /// Whether a multi-line span opens in the margin rather than with a row of its own.
  ///
  /// Asked at PLAN time, because [`bracket`](Self::bracket) has to answer for the source row
  /// before any mark under it is drawn, and because the answer needs a measurement of a row that
  /// does not exist yet.
  ///
  /// **Diverges:** conditional against unconditional. One style can only leave the opening cell
  /// unmarked when nothing else on the line would be confused with it and the line's own text
  /// says where the span begins; the other's bracket is in the margin whatever precedes the span,
  /// so it never needs a row and never marks the cell.
  fn opens_in_margin(&self, onset: Onset) -> bool;

  /// A span drawn whole on one line.
  ///
  /// **Diverges:** one row against two. A marker row carrying its own label ends wherever the
  /// label ends; hanging the label off a tee on the row below leaves the underline row free for
  /// every span on the line.
  fn whole(
    &self,
    paint: &mut Painter<'_>,
    frame: Frame<'_>,
    columns: core::ops::Range<u64>,
    phrase: Phrase<'_>,
  ) -> fmt::Result;

  /// Where a multi-line span opens.
  ///
  /// **Diverges:** a corner row reaching from the bracket's column to the opening cell, against
  /// nothing at all — the margin glyph on the source row above has already said it.
  ///
  /// Called for every multi-line opening rather than only for the ones that need a row, because
  /// "which ones need a row" is the question this method and
  /// [`opens_in_margin`](Self::opens_in_margin) are two halves of. A renderer that tested
  /// `compact` here would be holding one style's rule.
  fn opens(
    &self,
    paint: &mut Painter<'_>,
    frame: Frame<'_>,
    column: u64,
    phrase: Phrase<'_>,
  ) -> fmt::Result;

  /// Where it closes, and where its label is said.
  ///
  /// **Diverges, and this is the deepest one.** One style's closing runs back along the row to the
  /// cell the span's last character sits in and marks it; the other turns the corner under the
  /// bracket and never leaves the margin. So a bracketed span marks a cell in one style and no
  /// cell at all in the other, which is the exception
  /// `which_cells_are_marked_is_a_function_of_the_span_alone` has to be stated around.
  fn closes(
    &self,
    paint: &mut Painter<'_>,
    frame: Frame<'_>,
    column: u64,
    phrase: Phrase<'_>,
  ) -> fmt::Result;

  /// What the reader can do about it.
  ///
  /// **Diverges:** `= help:` aligned under the gutter, against `help:` at a fixed indent — the box
  /// has closed by then, so there is no gutter left to align to.
  fn help(&self, paint: &mut Painter<'_>, gutter: u64, help: &str) -> fmt::Result;
}
