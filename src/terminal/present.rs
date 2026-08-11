//! The seam between the styles, and nothing more than the seam.
//!
//! # This was derived, not designed
//!
//! Every method below exists because two implementations were written out in full first and then
//! differed there. Each carries the divergence that forced it. A method whose implementations
//! agreed is not a method: the margin's slots, the source text, the cell arithmetic and the whole
//! of [`Plan`](super::render::Plan) came out identical and stayed in the renderer.
//!
//! That is the design's rule for this phase, and it is worth restating why. A trait written from
//! prose descriptions of four renderers would have twenty methods, most of them unconsumed, and
//! would be wrong in the places prose is vague — which is exactly where a renderer is not.
//!
//! ## What the third and fourth implementations did to it
//!
//! Eleven methods were derived from [`Rustc`](super::rustc::Rustc) and
//! [`Miette`](super::miette::Miette). Two is not evidence that an abstraction is right — two can
//! only fail to show it is wrong — so [`Ariadne`](super::ariadne::Ariadne) and
//! [`Codespan`](super::codespan::Codespan) were added, one at a time, each written complete before
//! anything here was touched.
//!
//! `Ariadne` moved it in four places, and every one is a sentence the first two had no way to say:
//!
//! - [`margin`](Presentation::margin), a method that did not exist. Where a multi-line span opens
//!   is said ON the source row, by a run that leaves the bracket's column, crosses every column to
//!   its right and ends in an arrow at the line. Both earlier styles leave the region between the
//!   margin and the text a constant blank, so the renderer owned it. It cannot: its width and its
//!   contents are a style's.
//! - [`Part::Gap`], a variant. The row standing for elided lines used to be drawn by asking for
//!   [`Part::Runs`], because the first two answered the two identically.
//! - `first` on [`open_block`](Presentation::open_block). Two styles frame each BLOCK; that one
//!   frames the whole RENDER, so a second input is introduced inside the frame rather than opening
//!   one of its own.
//! - [`close_render`](Presentation::close_render), a method, and [`Drawn`] at it and at
//!   [`help`](Presentation::help). A style that frames each block closes it before the help; one
//!   that frames the render has to close it after, because the help is a row inside.
//!
//! `Codespan` moved it **nowhere**. It is designed independently of the first two, it says
//! something different from `Rustc` in eight of the thirteen methods, and the one capability it
//! reaches for — [`Frame::columns`], so a corner's run can pass behind the bars it crosses — had
//! arrived with `Ariadne`. That is the first evidence in this phase that the trait grew to the
//! right size rather than to the size of the style that grew it.
//!
//! `annotate-snippets` was on the same list and is **not here**: its default character set is
//! `Rustc`'s glyph for glyph, being the crate rustc's own emitter was extracted into. The evidence
//! is tabulated in [`rustc`](super::rustc), and the rule is this phase's own — what has no
//! divergence behind it does not get a file.
//!
//! # It is crate-private, and that is a decision
//!
//! Phase 3 is HTML, and it is the example the design says is most likely to collapse a trait
//! fitted to terminals. Every method was checked against it before the first two styles were
//! frozen, and the answer was that most did not survive. **Re-run against all thirteen, the
//! verdict did not move — it hardened:**
//!
//! | method | can HTML answer it? |
//! |---|---|
//! | [`header`](Presentation::header) | yes |
//! | [`open_block`](Presentation::open_block) | yes, ignoring `gutter`; `first` is meaningful |
//! | [`close_block`](Presentation::close_block) | yes, ignoring `gutter` |
//! | [`help`](Presentation::help) | yes, ignoring `gutter` |
//! | [`close_render`](Presentation::close_render) | yes |
//! | [`opens_in_margin`](Presentation::opens_in_margin) | yes, but vacuously — see below |
//! | [`line_field`](Presentation::line_field) | **no** |
//! | [`margin`](Presentation::margin) | **no** |
//! | [`elision_row`](Presentation::elision_row) | **no** |
//! | [`bracket`](Presentation::bracket) | **no** |
//! | [`whole`](Presentation::whole) | **no** |
//! | [`opens`](Presentation::opens) | **no** |
//! | [`closes`](Presentation::closes) | **no** |
//!
//! Seven of thirteen now, where it was six of eleven, and the failures still have ONE shape — which
//! is what makes this a finding rather than a list. Everything that positions or draws a mark is
//! denominated in **display cells** and in **`char`**: `whole` takes a `Range<u64>` of cells,
//! `opens` and `closes` take a cell, `bracket` returns a character to stand in a cell, `margin`
//! takes a connector COLUMN and writes a run of characters across the columns right of it, and
//! `line_field` and `elision_row` pad to a cell width. HTML has no cell. Its mark is an element
//! wrapped around the span's BYTES, its bracket is a border or a pseudo-element rather than a
//! glyph, and its alignment is the browser's rather than something painted one column at a time.
//! An HTML style could return `Some('│')` and pad with `&nbsp;`, but that is emulating a terminal
//! in a medium that has real boxes.
//!
//! **The two new methods fell on opposite sides, and that is the useful part.** `margin` is the
//! most cell-denominated method in the trait — it exists precisely to let a style draw ACROSS the
//! columns — while `close_render` is pure document structure and an HTML style would answer it by
//! closing an element. So the split the earlier verdict guessed at is now visible as a line through
//! the middle of the trait: **six structure hooks** (`header`, `open_block`, `close_block`, `help`,
//! `close_render`, `opens_in_margin`) that name what a region IS, and **seven geometry hooks** that
//! name where a glyph GOES. Three implementations added since that verdict landed on both sides of
//! it and did not blur it.
//!
//! §4 of the design already draws the same line — resolution reports *character* columns and the
//! terminal converts to *display* columns — and this trait sits **below** the conversion, because
//! [`Terminal::render`](super::Terminal::render) has already turned every span into cells before a
//! style is called. So `Presentation` is a terminal trait by construction, and the honest reading
//! of the ones that pass on a technicality is worth stating too: four of the six ignore `gutter`,
//! and `opens_in_margin` survives only because HTML's answer is a constant — its inputs are
//! whether a CELL budget cut the row and whether the source is indented, and a browser never cuts
//! a row.
//!
//! **So Phase 3 is a peer of [`Terminal`](super::Terminal), not an implementor of this** — the
//! same conclusion as before, now with four implementations behind it instead of two. What the
//! outputs share is [`Plan`](super::render::Plan) — which lines are drawn, where each span's ends
//! fall, which bracket runs where, what is elided — and a plan is stated in lines and byte spans,
//! which every medium has. If a medium-independent version of this trait is ever wanted, the change
//! it needs is still what it was, and the third style added one more to the list: hand a style a
//! `RegionLine`, which carries the line and the byte span it covers, instead of a `Range<u64>` of
//! cells; a semantic bracket value instead of a `char`; and, for `margin`, the SET of brackets a
//! row shows instead of a run to be painted over cells.
//!
//! What a caller gets instead of a trait is the choice —
//! [`Terminal::like_rustc`](super::Terminal::like_rustc),
//! [`like_miette`](super::Terminal::like_miette),
//! [`like_ariadne`](super::Terminal::like_ariadne) and
//! [`like_codespan`](super::Terminal::like_codespan) — which is the whole of what the design asked
//! for, and which can grow into a public trait later without breaking anything.

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
  /// The row standing for lines left out, which the bracket runs through without being able to
  /// show them.
  ///
  /// Added by [`Ariadne`](super::ariadne::Ariadne), which is the first style to answer it
  /// differently from [`Runs`](Self::Runs): its elision row breaks the WALL into a dotted bar and
  /// breaks every connector beside it the same way, so a reader sees at a glance that the rows
  /// under the gap are not adjacent. The two styles that came before draw their running bar and
  /// cannot tell the two rows apart — which is why the renderer used to ask for `Runs` here and
  /// why the distinction had to be made before a third style could state it.
  Gap,
}

/// What is standing in one connector column of one row.
///
/// A glyph, the role it is painted in, and **which question it answers** — the part the renderer
/// asked [`bracket`](Presentation::bracket) for when it put this here.
///
/// # The part travels with the glyph rather than beside it
///
/// [`margin`](Presentation::margin) used to take `turns: Option<u64>`, the column a bracket had an
/// end in — *and the rightmost where two did*. That parenthesis is the whole defect: it is a set
/// projected onto one of its members, and a row can have two ends with a column between them that
/// is neither. With ends at depth 1 and depth 3 and a bracket still running at depth 2, the style
/// that draws an end on the source row was told only about depth 3, so the end at depth 1 drew no
/// run at all and the row read as two unconnected corners.
///
/// Two shapes were available for the repair. A second slice of parts, parallel to the columns, was
/// the smaller change and is the one this crate has already rejected once: [`Frame`]'s own
/// documentation says three things read together on every row are one value rather than three
/// parameters, "because a caller pairing them by position is a caller that can pair them wrongly".
/// So the part is attached to the slot it describes, `turns` is gone, and the projection that
/// caused this cannot be written.
///
/// Every slot's part is true of the row it is on: a source row carries the parts the plan computed,
/// a row under one carries [`Part::Runs`] for every bracket still open — that is what the
/// normalisation below the source row MEANS — and an elision row carries [`Part::Gap`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Standing {
  glyph: char,
  role: Role,
  part: Part,
}

impl Standing {
  pub(super) const fn new(glyph: char, role: Role, part: Part) -> Self {
    Self { glyph, role, part }
  }

  /// What the style put here.
  pub(super) const fn glyph(&self) -> char {
    self.glyph
  }

  /// The style it is painted in.
  pub(super) const fn role(&self) -> Role {
    self.role
  }

  /// Whether a bracket has an END in this column on this row, rather than passing through it.
  ///
  /// The question `turns` used to answer for one column of the row. Asked of a slot, it cannot
  /// collapse two ends into one.
  pub(super) const fn turns(&self) -> bool {
    matches!(self.part, Part::Opens | Part::Closes)
  }
}

/// Whether the render drew a block at all.
///
/// A diagnostic can name no position this renderer is able to draw — [`Location::entire`], or an
/// input index the caller did not supply — and then there is no block, no excerpt and no source.
///
/// Two of the styles never read it: their frame is a block's, so a render with no block has
/// nothing of theirs open in it either way. [`Ariadne`](super::ariadne::Ariadne)'s frame is the
/// RENDER's, and both of the hooks that come after the last block have to know whether that frame
/// exists — its help is a row inside the frame and its closing rule is the frame's bottom. Told
/// only at the second of them, it draws the help against a wall nothing opened; told at neither,
/// it draws the wall and no bottom.
///
/// One value handed to both, rather than the renderer declining to CALL the closing hook: the same
/// fact reaching two hooks by two different mechanisms is the shape that lets them disagree.
///
/// [`Location::entire`]: crate::Location::entire
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Drawn {
  /// At least one block was drawn.
  Blocks,
  /// Nothing but the header, and whatever a style says after it.
  Nothing,
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
  margin: &'m [Option<Standing>],
}

impl<'m> Frame<'m> {
  pub(super) const fn new(gutter: u64, depth: u64, margin: &'m [Option<Standing>]) -> Self {
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

  /// What the plan put in each connector column of this row, before any style has drawn it.
  ///
  /// The slots are the plan's — one per multi-line span, assigned so a later span runs to the
  /// right of an earlier one — and what stands in them came from
  /// [`bracket`](Presentation::bracket), so a style reading this back is reading its own answers
  /// in the plan's order. It is here for the styles that draw something ACROSS the margin rather
  /// than in one column of it: a run that crosses the columns to its right has to know which of
  /// them are occupied, and [`margin`](Self::margin) and [`margin_left_of`](Self::margin_left_of)
  /// can only write a prefix of them.
  ///
  /// Read-only, which is what keeps the split intact: a style still cannot move a bracket into
  /// another span's column.
  pub(super) const fn columns(&self) -> &'m [Option<Standing>] {
    self.margin
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
  pub(super) fn occupied(&self) -> Option<&'m [Option<Standing>]> {
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
  ///
  /// `first` says whether this block opens the render. **Diverges, and it is the third style that
  /// forced it:** two of them frame each BLOCK, so every block opens the same way and the flag is
  /// nothing to them; [`Ariadne`](super::ariadne::Ariadne) frames the whole RENDER, and a second
  /// input is introduced *inside* the frame with a tee rather than opening a frame of its own. A
  /// style that could not tell would draw two tops and one bottom.
  fn open_block(
    &self,
    paint: &mut Painter<'_>,
    gutter: u64,
    block: &Block<'_>,
    first: bool,
  ) -> fmt::Result;

  /// Whatever closes an input's block.
  ///
  /// **Diverges:** a bare bar, the bottom of the box, and nothing at all — the style whose frame
  /// belongs to the render closes it in [`close_render`](Self::close_render) instead, because by
  /// then the help has to have been said inside it.
  fn close_block(&self, paint: &mut Painter<'_>, gutter: u64) -> fmt::Result;

  /// What stands before a source row's text: the number, and the wall beside it.
  ///
  /// The renderer writes the text; the field and the margin beside it are a style's, and the
  /// styles differ in the field by one cell of inset and by which character the wall is.
  ///
  /// **Diverges:** `NN | ` against ` NN │ `.
  fn line_field(&self, paint: &mut Painter<'_>, gutter: u64, number: u64) -> fmt::Result;

  /// Everything between the line-number field and a source row's own text.
  ///
  /// Which columns hold a bracket's END rather than a bracket passing through is read off
  /// [`Standing::turns`], one column at a time. It was a single `turns: Option<u64>` for as long as
  /// the only style that asked had one end per row to draw — see [`Standing`] for what that cost
  /// and why the answer now travels with the column it is about.
  ///
  /// **Diverges, and this is the one a trait derived from two styles did not have.** Both of the
  /// first two say where a multi-line span opens either in the span's own column or on a row of
  /// its own, so the region between the margin and the text is a constant blank and belonged to
  /// the renderer. [`Ariadne`](super::ariadne::Ariadne) says it *on the source row itself*, with a
  /// run that leaves the bracket's column, crosses every column to its right, and ends in an arrow
  /// pointing at the line — so the width of that region and what stands in it are a style's, and
  /// the renderer cannot write it.
  fn margin(&self, paint: &mut Painter<'_>, frame: Frame<'_>) -> fmt::Result;

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
  /// has closed by then, so there is no gutter left to align to — against a row *inside* a frame
  /// that has not closed yet, which is the one that has to read [`Drawn`].
  fn help(&self, paint: &mut Painter<'_>, gutter: u64, help: &str, drawn: Drawn) -> fmt::Result;

  /// Whatever closes the whole render, after the help.
  ///
  /// **Diverges:** nothing at all, against a rule that runs back under the gutter. This exists
  /// because it is not the same hook as [`close_block`](Self::close_block): a style that frames
  /// each block closes it before the help, and a style that frames the render has to close it
  /// *after*, since the help is one of the rows inside. One method could not be both, and
  /// reordering the renderer to suit either one moves the other style's help to the wrong side of
  /// its own frame.
  fn close_render(&self, paint: &mut Painter<'_>, gutter: u64, drawn: Drawn) -> fmt::Result;
}
