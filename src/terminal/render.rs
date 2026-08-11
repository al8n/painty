use core::{cmp::Reverse, fmt};

use std::collections::BinaryHeap;

use super::{
  ColorCapability, LineCells,
  miette::Miette,
  paint::Painter,
  present::{Frame, Onset, Part, Presentation},
  rustc::Rustc,
  width::{Budget, Mark},
};
use crate::{
  Diagnostic, Line, Palette, RegionLine, Role, Source, Span, Theme,
  source::{Opening, Walk},
};

/// One of the caller's inputs: its text, and whatever the caller calls it.
///
/// A [`Location`](crate::Location) carries a `source: u32` that indexes the list the producer was
/// numbering, so a renderer needs that same list to resolve a span. Mapping an index to a *name*
/// stays with the caller — painty is not a source-file manager — which is why the name comes in
/// here rather than being looked up.
#[derive(Debug, Clone, Copy)]
pub struct Input<'a> {
  source: Source<'a>,
  origin: Option<&'a str>,
}

impl<'a> Input<'a> {
  /// An input with no name.
  #[inline]
  #[must_use]
  pub const fn new(source: Source<'a>) -> Self {
    Self {
      source,
      origin: None,
    }
  }

  /// Names the input — a path, a URL, whatever the caller has.
  #[inline]
  #[must_use]
  pub const fn with_origin(mut self, origin: &'a str) -> Self {
    self.origin = Some(origin);
    self
  }

  /// Returns the text.
  #[inline]
  pub const fn source(&self) -> Source<'a> {
    self.source
  }

  /// Returns the caller's name for it, if it gave one.
  #[inline]
  pub const fn origin(&self) -> Option<&'a str> {
    self.origin
  }
}

/// One position a diagnostic named that this render can actually draw.
///
/// Built once, before anything is resolved, so that the two orders this needs — ascending offsets
/// to resolve in, and the caller's own to draw in — are both available without asking the
/// diagnostic twice. `at` is the caller's order, and it is the only thing that survives the sort.
///
/// `input` is the INDEX into the list the caller passed, not the `Location::source` it came from.
/// The two are the same number, and the index is the one this layer owns: rule 5 of
/// [the numeric widths](crate#numeric-widths) makes it a `usize`, where spelling a `u32` here would
/// be declaring a ceiling that is `Location`'s to declare and not this file's.
#[derive(Debug, Clone, Copy)]
pub(super) struct Drawable<'a> {
  pub(super) at: usize,
  pub(super) input: usize,
  pub(super) from: Input<'a>,
  pub(super) span: Span,
  pub(super) text: Option<&'a str>,
  pub(super) primary: bool,
}

/// Which part of a span one mark draws.
///
/// A span drawn on one line is marked over the whole of what it covers there. A span drawn on more
/// than one is marked at its two ENDS and joined between them, so each end is a mark of its own on
/// a different line and the two are the same span — which is why this is a property of the mark
/// rather than of the row it lands in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Ends {
  /// The whole of a span that is drawn on one line.
  Whole,
  /// Where a span that reaches a later line begins.
  Opens,
  /// Where it finishes, and where its label is said.
  Closes,
}

/// What one marker row says, and where the position under it starts.
#[derive(Debug, Clone, Copy)]
pub(super) struct Phrase<'a> {
  pub(super) text: Option<&'a str>,
  pub(super) primary: bool,
  /// Where this was in the caller's order.
  pub(super) at: usize,
  /// The line the mark is drawn under. Carried rather than paired up outside, because the marks of
  /// a whole render are one flat run and an excerpt takes its slice of them by walking it.
  pub(super) line: u64,
  pub(super) ends: Ends,
  /// The connector column the span was given, counting from 1, or zero for a span that needs none.
  pub(super) depth: u64,
  /// Opens in the margin rather than with a row of its own — see
  /// [`Presentation::opens_in_margin`].
  pub(super) compact: bool,
}

impl Phrase<'_> {
  /// The style everything this mark draws is written in — the marker, the label, and the connector
  /// that joins them.
  pub(super) const fn role(&self) -> Role {
    if self.primary {
      Role::PrimaryLabel
    } else {
      Role::SecondaryLabel
    }
  }
}

/// One source line, and every mark that will be drawn under it.
///
/// `marks` indexes one flat run rather than owning a `Vec` of its own: the marker rows of a whole
/// render are a single allocation, and an excerpt names its slice of them.
///
/// It may name none. A line between a multi-line span's ends carries no mark and is drawn anyway,
/// because a bracket down the margin of lines a reader cannot see says nothing.
#[derive(Debug, Clone)]
pub(super) struct Excerpt<'a> {
  pub(super) line: Line<'a>,
  pub(super) marks: core::ops::Range<usize>,
  /// The line before this one in the same input, when lines were left out between them and a `...`
  /// row stands in for what is missing.
  pub(super) after: Option<u64>,
}

/// One multi-line span's connector: the rows it runs down, the column it runs in, and how it starts.
#[derive(Debug, Clone, Copy)]
pub(super) struct Connector {
  pub(super) first: u64,
  pub(super) last: u64,
  /// Counting from 1. Two spans open at the same row never share one — see [`Plan::block`].
  pub(super) depth: u64,
  pub(super) role: Role,
  /// Opens in the margin rather than with a row of its own — see
  /// [`Presentation::opens_in_margin`].
  pub(super) compact: bool,
}

impl Connector {
  /// Which part of this bracket the source row of `line` shows, and `None` where the bracket is
  /// not open at all.
  ///
  /// Arithmetic over the connector's two ends, so it is the renderer's and not a style's: a style
  /// that worked out for itself which line was which could disagree with the plan about where a
  /// span ends. What the style is asked is only what to DRAW for the answer.
  pub(super) const fn part_on(&self, line: u64) -> Option<Part> {
    if line < self.first || line > self.last {
      None
    } else if line == self.first {
      Some(Part::Opens)
    } else if line == self.last {
      Some(Part::Closes)
    } else {
      Some(Part::Runs)
    }
  }

  /// Whether the connector runs through the lines left out between `above` and `below`.
  ///
  /// A span's ends are both drawn, so neither can be inside a gap — which is what makes this a
  /// comparison against the two lines that bound it rather than against the lines it hides.
  pub(super) const fn spans_the_gap(&self, above: u64, below: u64) -> bool {
    self.first <= above && self.last >= below
  }
}

/// One input's whole block: what the `-->` line says, and the geometry every row under it shares.
#[derive(Debug, Clone)]
pub(super) struct Block<'a> {
  pub(super) origin: Option<&'a str>,
  /// Where the input's earliest caller position is, in the units the `-->` line reports.
  pub(super) line: u64,
  pub(super) column: u64,
  /// The earliest caller position anywhere in this input, which is where the input sits among the
  /// others.
  pub(super) at: usize,
  /// How many connector columns this input's multi-line spans need, and zero when it has none —
  /// which is what keeps an input without them flush against the gutter.
  pub(super) depth: u64,
  pub(super) excerpts: core::ops::Range<usize>,
  pub(super) connectors: core::ops::Range<usize>,
}

/// Where a span is drawn, before the lines are gathered into excerpts.
#[derive(Debug, Clone, Copy)]
struct Placement<'a> {
  at: usize,
  text: Option<&'a str>,
  primary: bool,
  opening: Opening<'a>,
  /// The last line the span is drawn on, for a span drawn on more than one.
  closing: Option<RegionLine<'a>>,
  depth: u64,
  compact: bool,
}

impl Placement<'_> {
  const fn first(&self) -> u64 {
    self.opening.line().line().number()
  }

  fn last(&self) -> u64 {
    match self.closing {
      Some(closing) => closing.line().number(),
      None => self.first(),
    }
  }
}

/// Renders a diagnostic and its source to a fixed-width terminal.
///
/// # What this draws, and what it does not yet
///
/// One excerpt per source LINE: the line, drawn once, and a row of markers under it for every
/// position that falls there. `^` for the primary position, `-` for a secondary one, which is the
/// convention a reader already knows. Marker rows are in the order the caller gave the labels, so
/// the primary leads whether or not it is the leftmost thing on the line.
///
/// A span covering more than one line is **bracketed**: its two ends are marked on the lines they
/// fall on, a connector runs down the margin between them, and its label is said where it closes.
/// So the lines of one input are drawn in SOURCE order, which is what lets a connector mean
/// anything — a row between a span's ends is a line that span covers, and a reader can read the
/// extent off the margin rather than off two line numbers. Their caller order survives where it
/// still says something: it orders the inputs, it chooses what the `-->` line points at, and it
/// orders the marker rows under one line.
///
/// Overlapping spans get **separate connector columns**, assigned so that a span opening later runs
/// to the right of every span already open. One column would leave a reader unable to tell which
/// opening a closing belongs to, which is a wrong answer rather than a plainer one.
///
/// What is not here: several markers **merged onto one row**. Two positions on a line still get a
/// marker row each, which is a row-assignment problem of the same kind and is the next one.
///
/// # What k labels cost, and what a span covering a million lines costs
///
/// Per input: **one forward pass** over its text, whatever the label count — layer 2 walks
/// forwards, and a span's two ends are visited in one merged ascending order off a carried cursor.
/// Per drawn line: **one** scan for where the line ends, **one** bounded walk over its geometry,
/// and **one** source row. Per label: one marker row, and the label's own text once — and, for a
/// label whose span reaches another line, **one** further bounded walk over the line it opens on.
///
/// That last walk is counted here rather than folded into the row's, because it happens before any
/// row exists: whether an opening needs a marker row of its own depends on whether the row will
/// DRAW the cell it opens at, and the plan settles that ahead of writing anything. It is bounded by
/// the same [`max_source_bytes`](Self::max_source_bytes), and it is at most one per multi-line
/// label — so it is the same k the marker rows already are, on lines the render was going to walk
/// regardless.
///
/// The marker row is the honest floor rather than a gap. Two labels on a line are two things to
/// point at and two things to say, and no arrangement of rows makes them one. Everything that is
/// painty's — the resolve, the line scan, the geometry, the excerpt — is once per line.
///
/// **Rows do not scale with lines covered**, and that is a decision rather than a happy accident.
/// A row costs a bounded walk — [`max_source_bytes`](Self::max_source_bytes) — so drawing every
/// line a span touches would be a product of a bounded per-row cost and an *unbounded* row count,
/// which is the shape that bounds nothing: a two-byte span reaching across a ten-megabyte file
/// would ask for ten million rows. So the lines drawn for a multi-line span are its opening, its
/// closing, and at most the three after the opening; everything else is one `...` row. The drawn
/// count is therefore a function of the LABEL count, which is the k the marker rows already cost,
/// and no new budget is needed to say so.
///
/// What that does not reach is unchanged and is stated where it was:
/// [`max_source_bytes`](Self::max_source_bytes) bounds no walk that layer 2 does to find a line in
/// the first place, and finding a span's far end is linear in that end's offset exactly as finding
/// its start is.
///
/// ```
/// use painty::{Diagnostic, Location, Severity, Source, Span, terminal::{Input, Terminal}};
///
/// let text = "type Widget {\n  width: Int\n}\n";
/// let message = "`width` is defined twice";
/// let diagnostic = Diagnostic::new(
///   "mylang::schema::duplicate-field",
///   Severity::Error,
///   &message,
///   Location::new(0, Span::new(16, 21)),
/// )
/// .with_primary_label("redefined here");
///
/// let mut out = String::new();
/// Terminal::plain()
///   .render(&diagnostic, &[Input::new(Source::new(text))], &mut out)
///   .unwrap();
///
/// assert!(out.starts_with("error[mylang::schema::duplicate-field]: `width` is defined twice\n"));
/// assert!(out.contains("2 |   width: Int\n"));
/// assert!(out.contains("  |   ^^^^^ redefined here\n"));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Terminal<P> {
  palette: P,
  capability: ColorCapability,
  tab_width: u64,
  /// How the rows are drawn — see [`like_rustc`](Self::like_rustc).
  ///
  /// A value rather than a type parameter on `Terminal`, and a `&'static dyn` rather than an
  /// enum matched at every hook. A caller reading `--style` out of a flag keeps ONE renderer
  /// type, which a type parameter would not have given it, and nothing in this crate's public
  /// signatures mentions the trait — which is what keeps the seam unfrozen until HTML has been
  /// through it.
  presentation: &'static dyn Presentation,
}

impl Terminal<Theme> {
  /// The built-in theme, with no colour until a caller asks for some.
  ///
  /// [`ColorCapability::None`] rather than a guess: a renderer constructed without being told what
  /// it is writing to should produce something that is correct everywhere, and escapes in a file
  /// are not that.
  #[must_use]
  pub fn plain() -> Self {
    Self {
      palette: Theme::new(),
      capability: ColorCapability::None,
      tab_width: LineCells::default_tab_width(),
      presentation: &Rustc,
    }
  }
}

impl<P: Palette> Terminal<P> {
  /// A renderer over any palette.
  #[must_use]
  pub fn with_palette(palette: P) -> Self {
    Self {
      palette,
      capability: ColorCapability::None,
      tab_width: LineCells::default_tab_width(),
      presentation: &Rustc,
    }
  }

  /// Draws in the shape `rustc` does — the default.
  ///
  /// An arrow to the location, an indented gutter, `^` and `-` under the cells a span occupies,
  /// and a multi-line span reaching back into the source to mark the cell it ends at.
  ///
  /// Named for the renderer whose shape it follows, and not byte-compatible with it. What painty
  /// promises is the arrangement, not the bytes.
  #[must_use]
  pub fn like_rustc(mut self) -> Self {
    self.presentation = &Rustc;
    self
  }

  /// Draws in the shape `miette` does.
  ///
  /// A boxed location line, a box-drawing wall, a label hanging from its own row, and a
  /// multi-line span bracketed down the margin instead of reaching into the source.
  ///
  /// The two differ in **glyphs and layout only**. Colour is [`Theme`]'s and is unchanged by
  /// this, which is the split [`Role`] exists to make; and *what* is pointed at is the plan's, so
  /// a style cannot move a mark onto text the diagnostic is not about. The one thing it does
  /// change is whether a multi-line span's end is marked on a CELL at all: this style says it in
  /// the margin, so it is not.
  ///
  /// Named for the renderer whose shape it follows, and not byte-compatible with it.
  #[must_use]
  pub fn like_miette(mut self) -> Self {
    self.presentation = &Miette;
    self
  }

  /// Sets how much colour the output can carry — see [`ColorChoice::resolve`](super::ColorChoice).
  #[must_use]
  pub fn with_capability(mut self, capability: ColorCapability) -> Self {
    self.capability = capability;
    self
  }

  /// Sets the distance between tab stops.
  #[must_use]
  pub fn with_tab_width(mut self, tab_width: u64) -> Self {
    self.tab_width = tab_width;
    self
  }

  /// Returns the display columns the marker row occupies for one drawn line.
  ///
  /// # Why this is public
  ///
  /// It is the renderer's geometry, and geometry has to be assertable *before* it becomes a
  /// string. A test that could only read the finished output would be a golden, and a golden
  /// records whatever the code did on the day it was written — which is how an expectation ends up
  /// agreeing with a defect. The renderer and its invariants call this same function.
  ///
  /// Never empty: a zero-width span is a caret, and a caret a reader cannot see is not a caret.
  ///
  /// Confined to the drawable window, and confined by the WALK rather than by clipping its result:
  /// asking for exact columns first and cutting them down afterwards costs work proportional to the
  /// whole line to place a caret that ends up under the elision mark. `marks_within` stops at the
  /// ceiling, so a span out past it costs the window and not the file.
  ///
  /// A span reaching past the drawn text is marked over the `…`, which is where a reader should
  /// look for the rest of it.
  pub fn underline(&self, drawn: RegionLine<'_>) -> core::ops::Range<u64> {
    let measure = self.measure();
    let mut marks = [Mark::new(drawn.covered(), ())];
    measure
      .cells(drawn.line())
      .place_marks(&mut marks, measure.budget);
    never_empty(marks[0].columns())
  }

  /// The widest an excerpt is drawn, in cells.
  ///
  /// # A budget on one factor of a product bounds nothing
  ///
  /// A rendered row is the source line's length times the tab width, and three rounds of review
  /// bounded neither. The tab width is clamped by [`LineCells::max_tab_width`] — that is the
  /// multiplier. The line length is the caller's, and it is the other factor. Neither cap reaches
  /// the product, so a file of tabs a few tens of MiB long asks for billions of cells: first as an
  /// allocation, and then, once the rows were streamed instead, as output to any writer willing to
  /// take it. A writer that refuses early stops it. A `String` or a log sink says yes to all of it.
  ///
  /// So the bound is on the thing that actually grows. Past this many cells an excerpt stops and
  /// says so with `…` rather than cutting in silence, and the marker row is clipped to the same
  /// window — a diagnostic that quietly truncates is worse than one that admits it. Beyond the
  /// ceiling the output is a function of this number rather than of the input: two files an order
  /// of magnitude apart in width render byte for byte the same.
  ///
  /// Wider than any terminal anybody uses, so no real diagnostic reaches it, and small enough that
  /// a row cannot become a denial of service. It bounds what this renderer DRAWS;
  /// [`LineCells::write_expanded`] still writes whatever it is handed, because a caller drawing its
  /// own excerpt owns the size of what it asked for.
  ///
  /// # What it covers: the excerpt, and not the caller's own words
  ///
  /// This bounds **source-excerpt geometry**. It does not bound the message, the code, the origin,
  /// the labels or the help text, and that is a decision rather than a gap. A whole render is
  /// therefore
  ///
  /// > a bound from this number, **plus** whatever the caller passed in.
  ///
  /// The two halves differ in kind. An excerpt **amplifies**: 257 bytes of tabs become 65,536
  /// cells, two hundred and fifty times what was handed over and nothing the caller could have
  /// predicted from it. Bounding that is painty's job because painty created it. Caller text
  /// **passes through**: a ten-megabyte label is printed once and produces ten megabytes, the
  /// caller already knows how long its own string is, and it can pass a shorter one.
  ///
  /// Truncating it would be the worse failure. A diagnostic exists to say something, and one that
  /// silently drops the part its author wrote is lying about what it was asked to report — an
  /// invisible loss, in exchange for bounding a size the caller had already chosen. `rustc` does not
  /// cut your error message either.
  ///
  /// Both halves are pinned in `tests/writer_discipline.rs`: that no small input yields a large
  /// excerpt, and that a large label really does come out whole.
  ///
  /// Nor does it bound the **connector margin** a bracketed span adds to the left of every row,
  /// which is one column per multi-line span and so is a function of the label count rather than of
  /// the source. That is the same k a marker row per label already is, and it is bounded for the
  /// same reason: the caller chose how many labels to send.
  #[inline]
  #[must_use]
  pub const fn max_rendered_width() -> u64 {
    4096
  }

  /// How many bytes of a source line an excerpt will examine.
  ///
  /// # This is the bound; the cell ceiling is a layout rule
  ///
  /// [`max_rendered_width`](Self::max_rendered_width) says how wide a row is DRAWN. It does not
  /// bound cost, and treating it as though it did is how five rounds of review kept finding one
  /// class a door along. Each budget was denominated in the resource the previous defect had
  /// consumed — allocation, then emission, then cells — and each was walked past through one it was
  /// not. A run of `U+200B` occupies no cells at all, so a cell ceiling is structurally unable to
  /// see it, and so would be a sixth ceiling denominated in whatever the sixth defect costs.
  ///
  /// Bytes are different in kind. Every input that has cost painty anything — tabs amplifying into
  /// cells, long lines, spans past the window, zero-width runs, combining sequences — first had to
  /// be **supplied**, and supplying it costs the caller a byte. A bound here sits upstream of the
  /// class rather than beside it: in this layer nothing consumes unbounded work without unbounded
  /// input, so one number bounds the walk, the row, the marker row and the segmentation under them.
  ///
  /// # It is spent by slicing, because a check is one cluster too late
  ///
  /// The budget used to be a test inside the loop, and a test runs *after* the segmenter has read
  /// the unit it rejects. A grapheme cluster has no length limit — a base with eight megabytes of
  /// combining marks is one — so the first unit could cost eight megabytes against a budget of
  /// sixty-four kilobytes, and it did: measured against a one-megabyte cluster, 8.21×. "Bytes
  /// examined" was false by up to one cluster, and one cluster is unbounded.
  ///
  /// So the segmenter is handed this many bytes and no more. What that costs is precision at the
  /// edge, in the safe direction: the last unit of a cut slice may be half of a cluster that
  /// continues past the cap, so it is not drawn, and a cluster that happened to end exactly at the
  /// cap is elided one unit early on a row that was being elided anyway. Half a cluster is not
  /// something a terminal can draw, and drawing it would be the other kind of wrong.
  ///
  /// # What it does not reach
  ///
  /// Written down rather than left to be discovered. Resolving a byte offset to a line and column
  /// is layer 2's scan, linear in the offset, as is finding where the line ENDS — and that one is
  /// linear in the line however small the span. Both are 1× in input the caller supplied, so they
  /// are pass-through rather than amplification — the distinction
  /// [`max_rendered_width`](Self::max_rendered_width) draws — but neither is bounded by this
  /// number, and no number placed here could bound them.
  ///
  /// Sixteen times the cell ceiling, so a line reaches it only by averaging under one cell per
  /// sixteen bytes. Ordinary text does not come close: ASCII is one to one, CJK three bytes to two
  /// cells, a long emoji sequence far denser than that. A line that trips this was built to.
  #[inline]
  #[must_use]
  pub const fn max_source_bytes() -> u64 {
    65_536
  }

  /// How this renderer measures a line, and what measuring one may spend.
  pub(super) fn measure(&self) -> Measure {
    Measure {
      tab_width: self.tab_width,
      budget: Budget {
        cells: Self::max_rendered_width(),
        bytes: Self::max_source_bytes(),
      },
    }
  }

  /// Writes `diagnostic` against the caller's inputs.
  ///
  /// # Why a list, and not one source
  ///
  /// This took a single [`Source`] and resolved every span against it, ignoring
  /// [`Location::source`](crate::Location::source) entirely. A diagnostic whose label points into
  /// a *different* input — "first defined here", in another file, which is the commonest
  /// multi-file diagnostic there is — was rendered against the wrong text, with a confident line
  /// number and a marker under whatever happened to be there. Silently wrong output, which is the
  /// failure this crate exists to prevent.
  ///
  /// So a location's `source` is used for what it is: an index into the list the producer was
  /// numbering. Out of range draws no excerpt rather than a fabricated one, on the same terms as
  /// [`Location::entire`](crate::Location::entire).
  ///
  /// painty still owns no mapping from an index to a filename — [`Input::origin`] is the caller's
  /// name for its own input, supplied because only the caller has it.
  pub fn render(
    &self,
    diagnostic: &Diagnostic<'_>,
    inputs: &[Input<'_>],
    out: &mut impl fmt::Write,
  ) -> fmt::Result {
    // Measured against the same stops and the same budget the rows below are, and by one value
    // rather than two, because the plan decides one thing about a row it does not draw — see
    // [`Measure`].
    let measure = self.measure();
    let style = self.presentation;
    let mut plan = Plan::of(diagnostic, inputs, measure, style);
    let gutter = plan.gutter();
    let mut paint = Painter::new(out, &self.palette, self.capability);

    style.header(&mut paint, diagnostic)?;

    let Plan {
      blocks,
      excerpts,
      marks,
      connectors,
    } = &mut plan;
    // One slot per connector column, resized per block and refilled per row.
    let mut margin: Vec<Option<(char, Role)>> = Vec::new();

    for block in blocks.iter() {
      // A header per input, and the key is the input INDEX rather than the origin string. Resolving
      // each span against its own input was half the multi-input fix; the other half is saying which
      // input a row came from, because a secondary label in another file was otherwise drawn under
      // the first file's header — the reader told, confidently and silently, that text came from a
      // file it did not come from. Two inputs with no origin, or with the same one, are still two
      // inputs, and only the index distinguishes them.
      //
      // A CHARACTER column, and the marker rows below are in DISPLAY columns. Two units in one
      // frame is deliberate, because the two rows are read by different things.
      //
      // This line is MACHINE-PARSED. `rustc` writes it, and editors, IDEs and LSP clients read
      // `file:line:column` off it to move a cursor — in characters, which is what every one of
      // them means by a column and what `Position::column` already answers. A display column here
      // is silently wrong on exactly the lines this crate is proudest of getting right: a tab or
      // a wide character makes the two disagree, and the consumer navigates to the wrong place
      // with no way to tell. Character's failure mode is visible and conventional; display's is
      // silent, which is the one thing this crate exists not to do.
      //
      // The marker rows stay in display columns because they are read by a HUMAN looking at the row
      // above them, where a caret has to sit under the glyph a terminal painted.
      //
      // It costs nothing, which is the smaller half of the argument and worth writing down: the
      // resolve that found this line computed the character column on its way past, so the header
      // is a value layer 2 already produced. The display column was a grapheme walk over every
      // cluster before the offset, unbounded, and the dominant term of a whole render at k=1.
      //
      // WHICH position it names is the caller's earliest one in this input, not the topmost line
      // drawn. A multi-line span opening twenty lines above the primary would otherwise send an
      // editor to a line the diagnostic is not about.
      //
      // The style writes it, because how a location is announced is one of the things the two
      // differ in; WHICH location it announces is decided here, and is not a style's to move.
      style.open_block(&mut paint, gutter, block)?;

      let running = &connectors[block.connectors.clone()];
      margin.clear();
      margin.resize(slot(block.depth), None);

      for index in block.excerpts.clone() {
        let excerpt = &excerpts[index];
        let number = excerpt.line.number();
        let cells = measure.cells(excerpt.line);
        let placed = &mut marks[excerpt.marks.clone()];
        let row = cells.place_marks(placed, measure.budget);

        // Lines were left out above this one, so a row stands where they would have been — with
        // the connectors of every span that runs THROUGH them, since a bracket that vanished over a
        // gap would read as two brackets.
        if let Some(above) = excerpt.after {
          fill(&mut margin, running, |connector| {
            connector
              .spans_the_gap(above, number)
              .then(|| style.bracket(Part::Runs, connector.role, connector.compact))
              .flatten()
          });
          style.elision_row(&mut paint, Frame::new(gutter, block.depth, &margin))?;
        }

        // WHICH part of a bracket a row shows is arithmetic over the connector's two ends and is
        // settled here; what stands there is the style's answer and nothing else.
        fill(&mut margin, running, |connector| {
          let part = connector.part_on(number)?;
          style.bracket(part, connector.role, connector.compact)
        });

        // The field is the style's, the margin and the text are not. Straight to the writer, not
        // through a `String` first: materialising the row commits the allocation before the writer
        // is ever consulted, so a caller with a bounded or refusing `fmt::Write` — the whole reason
        // this takes one — cannot decline what it never saw.
        //
        // And bounded, because streaming only moves the cost to a writer that accepts: a row is the
        // line's length times the tab width, both caller-owned, so the ceiling is on the product.
        // The `…` is inside the styled run deliberately — it stands where source would have stood,
        // occupies the one cell `underline` reserved for it, and is what stops the cut being
        // silent.
        //
        // Written ONCE for the line rather than once per label, and placed against the stop the one
        // walk found. `underline` would answer the same columns — it is `never_empty` over that same
        // call — but asking again would walk the window again.
        style.line_field(&mut paint, gutter, number)?;
        Frame::new(gutter, block.depth, &margin).margin(&mut paint)?;
        paint.styled_with(Role::SourceText, |shown| {
          cells.write_expanded_upto(shown, row.drawn_end)?;
          if row.elided {
            fmt::Write::write_char(shown, '\u{2026}')?;
          }
          Ok(())
        })?;
        paint.newline()?;

        // Under the source row, a span that OPENS here is open from the row that says so onwards —
        // which is whatever the style just drew in the margin, or the row it is about to write.
        for mark in placed.iter() {
          let phrase = *mark.payload();
          let columns = never_empty(mark.columns());
          let frame = Frame::new(gutter, block.depth, &margin);
          match phrase.ends {
            Ends::Whole => style.whole(&mut paint, frame, columns, phrase)?,
            Ends::Opens => {
              // Called for every opening, not only for the ones that need a row: whether one is
              // needed is the style's own rule, and testing `compact` here would be this file
              // holding half of it.
              style.opens(&mut paint, frame, columns.start, phrase)?;
              if let Some(column) = column_of(&mut margin, phrase.depth) {
                *column = style
                  .bracket(Part::Runs, phrase.role(), phrase.compact)
                  .map(|glyph| (glyph, phrase.role()));
              }
            }
            Ends::Closes => {
              style.closes(&mut paint, frame, columns.end - 1, phrase)?;
              if let Some(column) = column_of(&mut margin, phrase.depth) {
                *column = None;
              }
            }
          }
        }
      }
      style.close_block(&mut paint, gutter)?;
    }

    if let Some(help) = diagnostic.help() {
      style.help(&mut paint, gutter, help)?;
    }
    Ok(())
  }
}

/// How a line is measured, and what measuring it may spend.
///
/// One value rather than two parameters, and it exists because the PLAN takes a decision about a
/// row it does not draw. Whether a span's opening needs a marker row of its own is settled before
/// anything is written, and it is settled by asking what the drawn row will look like — so the two
/// have to measure the line identically, and a caller pairing a tab width with somebody else's
/// budget is a caret placed against a row nobody drew.
#[derive(Debug, Clone, Copy)]
pub(super) struct Measure {
  pub(super) tab_width: u64,
  pub(super) budget: Budget,
}

impl Measure {
  /// One line, as this render measures it.
  pub(super) const fn cells<'a>(&self, line: Line<'a>) -> LineCells<'a> {
    LineCells::new(line, self.tab_width)
  }

  /// Whether the row `line` is drawn as will show the cell `covered` STARTS on.
  ///
  /// Asked of the bounded walk that places every mark, rather than of a second notion of what is
  /// visible. The two budgets are denominated differently on purpose —
  /// [`max_source_bytes`](Terminal::max_source_bytes) bounds the input and
  /// [`max_rendered_width`](Terminal::max_rendered_width) bounds the cells — so a question about
  /// what a reader can SEE has to be put to the one that cuts the row. A guard denominated in bytes
  /// passes happily on a line of five thousand spaces whose row stops at four thousand and
  /// ninety-six cells.
  ///
  /// It bounds the prefix scan at the one call site as a side effect, which is the whole of what
  /// the byte test that used to sit there was doing: an opening the row DREW is fewer than
  /// [`Row::drawn_end`] bytes into the line, and that is at most [`Budget::bytes`]; an opening on a
  /// row that was not cut is at most the length of a line that fitted the byte budget entire.
  fn draws_the_start_of(&self, line: Line<'_>, covered: Span) -> bool {
    let mut marks = [Mark::new(covered, ())];
    self.cells(line).place_marks(&mut marks, self.budget);
    marks[0].starts_in_window()
  }
}

/// How many lines after a multi-line span's opening are shown before the rest are elided.
///
/// Three, and the number is a layout rule rather than a budget — see [`Terminal`]'s note on what a
/// span covering a million lines costs. It is what makes a span of six lines or fewer render whole
/// once the one-line gap below is filled, and what keeps every longer one to the same six rows.
const CONTEXT: u64 = 3;

/// Everything a render works out before it writes anything.
///
/// Four flat runs and no tree: an excerpt names its slice of the marks, and a block names its slice
/// of the excerpts and of the connectors. So a whole render is four allocations however many labels
/// and lines it turns out to have, and the gutter — which is as wide as the widest line number
/// anywhere in it — can be sized before the first row is written.
#[derive(Debug, Default)]
pub(super) struct Plan<'a> {
  pub(super) blocks: Vec<Block<'a>>,
  pub(super) excerpts: Vec<Excerpt<'a>>,
  pub(super) marks: Vec<Mark<Phrase<'a>>>,
  pub(super) connectors: Vec<Connector>,
}

impl<'a> Plan<'a> {
  /// Everything one render works out before it writes anything.
  ///
  /// Style-independent, and that is the claim this function makes rather than a convenience: which
  /// lines are drawn, which cells each end of each span occupies, and which column a bracket runs
  /// down are answers about the SOURCE and the caller's positions. A presentation chooses the
  /// glyphs and the rows that carry them; it does not get to move a mark. So both styles are built
  /// on this one plan, and `which_cells_are_marked_is_the_same_in_both_styles` is what holds that
  /// claim to more than an intention.
  pub(super) fn of(
    diagnostic: &Diagnostic<'a>,
    inputs: &[Input<'a>],
    measure: Measure,
    style: &dyn Presentation,
  ) -> Plan<'a> {
    // Every position the diagnostic names, in the order the caller gave them, and then only those
    // that can be drawn at all: an input the caller did not supply and a position with no span both
    // draw nothing, on the same terms as `Location::entire`.
    let mut positions = Vec::with_capacity(1 + diagnostic.labels().len());
    positions.push((diagnostic.primary(), diagnostic.primary_label(), true));
    for label in diagnostic.labels() {
      positions.push((label.location(), Some(label.text()), false));
    }
    let mut drawable: Vec<Drawable<'a>> = Vec::with_capacity(positions.len());
    drawable.extend(positions.into_iter().enumerate().filter_map(
      |(at, (location, text, primary))| {
        let input = location.source() as usize;
        Some(Drawable {
          at,
          input,
          from: *inputs.get(input)?,
          span: location.span()?,
          text,
          primary,
        })
      },
    ));

    // RESOLUTION order, which is not the order any of this is drawn in. Layer 2 walks forwards, so
    // a set of spans resolved in ascending order over one input costs one pass; resolved in the
    // caller's order it costs one pass EACH, and on a multi-megabyte line each pass is the whole
    // line again. The caller's order is carried in `at` and put back below.
    drawable.sort_unstable_by_key(|position| (position.input, position.span.start(), position.at));

    // One block per input, worked out completely before anything is written: the gutter is as wide
    // as the widest line number the whole render will show, and the first row does not know what is
    // coming.
    let mut plan = Plan::default();
    let mut run = 0;
    while run < drawable.len() {
      let input = drawable[run].input;
      let mut end = run;
      while end < drawable.len() && drawable[end].input == input {
        end += 1;
      }
      plan.block(&drawable[run..end], measure, style);
      run = end;
    }

    // By the order each input FIRST appears, not by its index: the primary is pushed first, so
    // sorting by index would move another file's label above the position the diagnostic is
    // actually about. Only the blocks are sorted — the LINES inside one are in source order, which
    // is what a connector running down the margin between two of them is able to mean.
    plan.blocks.sort_unstable_by_key(|block| block.at);
    plan
  }

  /// How many cells the line-number field takes: the widest number this whole render will show.
  ///
  /// Asked of the finished plan rather than of the first row, because the first row does not know
  /// what is coming and a gutter that widened partway down would leave every row above it
  /// misaligned.
  pub(super) fn gutter(&self) -> u64 {
    self
      .excerpts
      .iter()
      .map(|excerpt| digits(excerpt.line.number()))
      .max()
      .unwrap_or(1)
  }

  /// Works out one input's block: where each of its spans is drawn, which lines that puts on the
  /// page, and which column each multi-line connector runs down.
  pub(super) fn block(
    &mut self,
    drawable: &[Drawable<'a>],
    measure: Measure,
    style: &dyn Presentation,
  ) {
    let Some(leading) = drawable.first() else {
      return;
    };
    let source = leading.from.source;

    // ── Both ends of every span, in ONE forward walk ────────────────────────────────────────
    //
    // The walk is forward-only, so resolving the near ends in one loop and the far ends in another
    // would be two passes over the input. They are merged instead: the spans arrive in ascending
    // START order, a span's far end is queued the moment its near end reports there is one, and the
    // queue is drained ahead of the next start. Every stop is therefore visited in ascending offset
    // order off one cursor, and a multi-line span costs no pass that a single-line one does not.
    //
    // A close goes FIRST where two stops land on the same offset, and that ordering is load-bearing
    // rather than tidy. A walk asked for an offset it is already standing on cannot see that a line
    // break ended there — which is the one question a close exists to answer.
    let mut walk = Walk::new(source);
    let mut pending: BinaryHeap<Reverse<(usize, usize)>> = BinaryHeap::new();
    let mut placements: Vec<Placement<'a>> = Vec::with_capacity(drawable.len());
    // The order the one-pass claim rests on, checked where it is produced. A stop behind the cursor
    // would still render CORRECTLY — `Walk` restarts rather than answering from where it is — so no
    // test that reads output can see this go wrong, and a timer at this resolution cannot either.
    let mut reached = 0;
    for position in drawable {
      let start = walk.clamped(position.span).start();
      while let Some(Reverse((at, index))) = pending.peek().copied() {
        if at > start {
          break;
        }
        pending.pop();
        debug_assert!(
          at >= reached,
          "a closing at {at} behind a stop at {reached}"
        );
        reached = at;
        close(&mut walk, &mut placements[index]);
      }
      debug_assert!(
        start >= reached,
        "an opening at {start} behind a stop at {reached}"
      );
      reached = start;
      let opening = walk.open(position.span);
      if opening.reaches_another_line() {
        pending.push(Reverse((opening.span().end(), placements.len())));
      }
      placements.push(Placement {
        at: position.at,
        text: position.text,
        primary: position.primary,
        opening,
        closing: None,
        depth: 0,
        compact: false,
      });
    }
    while let Some(Reverse((at, index))) = pending.pop() {
      debug_assert!(
        at >= reached,
        "a closing at {at} behind a stop at {reached}"
      );
      reached = at;
      close(&mut walk, &mut placements[index]);
    }

    // ── A connector column each, and never two open at once in one column ───────────────────
    //
    // Outermost first, so a span that encloses another is assigned before it. A new span takes the
    // column after the rightmost one still open rather than the leftmost one free: the corner that
    // closes a span runs RIGHTWARDS from its own column, so a span placed left of one already open
    // would draw its corner straight through that one's bar and claim to close it.
    let mut order: Vec<usize> = (0..placements.len())
      .filter(|&index| placements[index].closing.is_some())
      .collect();
    order.sort_unstable_by_key(|&index| {
      let placement = &placements[index];
      (placement.first(), Reverse(placement.last()), placement.at)
    });
    // Still open, in the order they opened — which is also increasing in depth, so the deepest open
    // column is the last entry and the closed ones behind it can be dropped as they are passed.
    let mut open: Vec<(u64, u64)> = Vec::new();
    let mut deepest = 0;
    for index in order {
      let (first, last) = (placements[index].first(), placements[index].last());
      while let Some(&(closes, _)) = open.last() {
        if closes >= first {
          break;
        }
        open.pop();
      }
      let depth = open.last().map_or(0, |&(_, depth)| depth) + 1;
      placements[index].depth = depth;
      open.push((last, depth));
      deepest = deepest.max(depth);
    }

    // ── Which lines are drawn, and which openings need no row of their own ──────────────────
    //
    // Every line an end of a span falls on. Not every line a span covers: that count is the
    // caller's and a row is not.
    //
    // With duplicates until the compact decision has been taken off it, because one anchor is one
    // mark — so how many times a line appears here is how many marks it will carry, which is half
    // of what decides whether a span can open without a row of its own.
    let mut anchors: Vec<Line<'a>> = Vec::with_capacity(placements.len() * 2);
    for placement in &placements {
      anchors.push(placement.opening.line().line());
      if let Some(closing) = placement.closing {
        anchors.push(closing.line());
      }
    }
    anchors.sort_unstable_by_key(Line::number);

    // Whether a multi-line span opens in the MARGIN or with a row of its own. Both facts are
    // worked out here because one of them needs a measurement of a row that does not exist yet;
    // which of them matters is the style's, and the two styles disagree about both.
    //
    // `drawn` is the one a reader would not think to ask for, and it is the one whose absence made
    // this wrong. Suppressing a marker is only a saving if the cell the marker would have gone on
    // is on the page: a row is cut at `max_rendered_width` CELLS, so an opening five thousand
    // spaces into a line is out past the `…`, and a style told only that the line is blank would
    // leave the span with a glyph in the margin and nothing at all pointing into the row. It is
    // asked of `Measure::draws_the_start_of`, which is the same bounded walk that will place the
    // mark, and not of a second reckoning of what is visible.
    //
    // `blank` used to be the last term of a short-circuited `&&` whose previous term was `drawn`,
    // and that ordering was the only thing bounding it — "is this indentation" is a scan over as
    // many bytes as a caller cares to indent with. Handing a style three FACTS means computing
    // three facts, so the scan carries the geometry walk's own byte budget now. It changes nothing
    // observable, because an opening the row drew is inside that budget by construction; what it
    // changes is that the fact is true on its own rather than true given another one.
    for placement in &mut placements {
      if placement.closing.is_none() {
        continue;
      }
      let line = placement.opening.line().line();
      let covered = placement.opening.line().covered();
      let before = covered.start() - line.span().start();
      let blank = u64::try_from(before).is_ok_and(|bytes| bytes <= measure.budget.bytes)
        && line.text()[..before].chars().all(char::is_whitespace);
      placement.compact =
        style.opens_in_margin(Onset::new(measure.draws_the_start_of(line, covered), blank));
    }
    anchors.dedup_by_key(|line| line.number());

    // ── What is marked, and on which line ───────────────────────────────────────────────────
    let marks_from = self.marks.len();
    for placement in &placements {
      let ends = if placement.closing.is_some() {
        Ends::Opens
      } else {
        Ends::Whole
      };
      self.marks.push(Mark::new(
        placement.opening.line().covered(),
        Phrase {
          // A label is said once, where the span closes. An opening that repeated it would say one
          // thing twice about one span and leave a reader looking for the difference.
          text: if ends == Ends::Whole {
            placement.text
          } else {
            None
          },
          primary: placement.primary,
          at: placement.at,
          line: placement.first(),
          ends,
          depth: placement.depth,
          compact: placement.compact,
        },
      ));
      if let Some(closing) = placement.closing {
        self.marks.push(Mark::new(
          closing.covered(),
          Phrase {
            text: placement.text,
            primary: placement.primary,
            at: placement.at,
            line: closing.line().number(),
            ends: Ends::Closes,
            depth: placement.depth,
            compact: placement.compact,
          },
        ));
      }
    }
    // Line order for the excerpts to take their slices in, and the CALLER's order under each line —
    // so the primary leads whether or not it is the leftmost thing there.
    self.marks[marks_from..].sort_by_key(|mark| (mark.payload().line, mark.payload().at));

    let mut reaches: Vec<(u64, u64)> = placements
      .iter()
      .filter_map(|placement| {
        placement.closing?;
        let first = placement.first();
        Some((first, (first + CONTEXT).min(placement.last() - 1)))
      })
      .collect();
    reaches.sort_unstable();
    let reach_of = |line: u64| {
      let from = reaches.partition_point(|&(first, _)| first < line);
      let to = reaches.partition_point(|&(first, _)| first <= line);
      reaches[from..to]
        .iter()
        .map(|&(_, reach)| reach)
        .max()
        .unwrap_or(line)
    };

    let excerpts_from = self.excerpts.len();
    let mut cursor = marks_from;
    let mut above: Option<Line<'a>> = None;
    for &anchor in &anchors {
      let mut after = None;
      if let Some(previous) = above {
        let number = anchor.number();
        let mut shown = reach_of(previous.number()).clamp(previous.number(), number - 1);
        // One line left over reads worse as `...` than as the line itself, and it is the shape a
        // span of exactly six lines leaves behind.
        if number - shown == 2 {
          shown = number - 1;
        }
        let mut line = previous;
        while line.number() < shown {
          let Some(next) = source.line_after(line) else {
            break;
          };
          line = next;
          self.take_line(line, &mut cursor, None);
        }
        if line.number() < number - 1 {
          after = Some(line.number());
        }
      }
      self.take_line(anchor, &mut cursor, after);
      above = Some(anchor);
    }

    // ── The connectors ──────────────────────────────────────────────────────────────────────
    let connectors_from = self.connectors.len();
    for placement in &placements {
      let Some(closing) = placement.closing else {
        continue;
      };
      self.connectors.push(Connector {
        first: placement.first(),
        last: closing.line().number(),
        depth: placement.depth,
        role: if placement.primary {
          Role::PrimaryLabel
        } else {
          Role::SecondaryLabel
        },
        compact: placement.compact,
      });
    }

    // Where the input sits among the others, and what its `-->` line points at: the caller's
    // EARLIEST position in it, which is not the topmost line drawn. A multi-line span opening
    // twenty lines above the primary would otherwise send an editor to a line nobody asked about.
    let earliest = placements
      .iter()
      .min_by_key(|placement| placement.at)
      .expect("a block is built from at least one drawable position");
    self.blocks.push(Block {
      origin: leading.from.origin,
      line: earliest.first(),
      column: earliest.opening.at().column(),
      at: earliest.at,
      depth: deepest,
      excerpts: excerpts_from..self.excerpts.len(),
      connectors: connectors_from..self.connectors.len(),
    });
  }

  /// Adds one drawn line, taking whatever marks belong to it.
  ///
  /// The marks are already in line order and the lines are added in line order, so one cursor walks
  /// both — where asking "which marks are on this line" per line would be a scan of the marks per
  /// line. A line between a span's ends takes none, which is why the range can be empty.
  fn take_line(&mut self, line: Line<'a>, cursor: &mut usize, after: Option<u64>) {
    let from = *cursor;
    while self
      .marks
      .get(*cursor)
      .is_some_and(|mark| mark.payload().line == line.number())
    {
      *cursor += 1;
    }
    self.excerpts.push(Excerpt {
      line,
      marks: from..*cursor,
      after,
    });
  }
}

/// Resolves where a multi-line span finishes.
///
/// Kept as multi-line only if it really does finish on another line. Everything downstream — the
/// column assignment, the lines shown after the opening, the corner arithmetic — reads
/// `last > first` off this one place rather than re-deciding it.
fn close<'a>(walk: &mut Walk<'a>, placement: &mut Placement<'a>) {
  let closing = walk.close(&placement.opening);
  if closing.line().number() > placement.opening.line().line().number() {
    placement.closing = Some(closing);
  }
}

/// A connector column as an index into a row's margin.
///
/// There is one column per multi-line span and the spans came out of a `Vec`, so this cannot lose
/// anything. `try_from` rather than `as` all the same: if that reasoning were ever wrong the answer
/// is a column past the end of the margin, which draws nothing, rather than a column near the
/// gutter, which draws a bar under the wrong span.
pub(super) fn slot(column: u64) -> usize {
  usize::try_from(column).unwrap_or(usize::MAX)
}

/// The margin slot a connector's column occupies.
pub(super) fn column_of(
  margin: &mut [Option<(char, Role)>],
  depth: u64,
) -> Option<&mut Option<(char, Role)>> {
  margin.get_mut(slot(depth).checked_sub(1)?)
}

/// Refills every connector column of one row.
pub(super) fn fill(
  margin: &mut [Option<(char, Role)>],
  connectors: &[Connector],
  mut glyph: impl FnMut(&Connector) -> Option<char>,
) {
  for column in margin.iter_mut() {
    *column = None;
  }
  for connector in connectors {
    if let Some(character) = glyph(connector)
      && let Some(column) = column_of(margin, connector.depth)
    {
      *column = Some((character, connector.role));
    }
  }
}

/// A marker range that a reader can see.
///
/// A zero-width span is a caret, and a caret of no cells is not a caret. Shared by
/// [`Terminal::underline`] and the row that draws it so the two cannot disagree about it.
pub(super) fn never_empty(columns: core::ops::Range<u64>) -> core::ops::Range<u64> {
  if columns.end > columns.start {
    columns
  } else {
    columns.start..columns.start + 1
  }
}

/// How many decimal digits a line number occupies.
pub(super) fn digits(mut number: u64) -> u64 {
  let mut count = 1;
  while number >= 10 {
    number /= 10;
    count += 1;
  }
  count
}
