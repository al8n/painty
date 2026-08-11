//! Layer 3, HTML: a document the embedder themes, with nothing of the caller's text left as markup.
//!
//! ```
//! use painty::{Diagnostic, Input, Location, Severity, Source, Span, html::Html};
//!
//! let text = "type Widget {\n  width: Int\n}\n";
//! let diagnostic = Diagnostic::new(
//!   "mylang::schema::duplicate-field",
//!   Severity::Error,
//!   &"`width` is defined twice",
//!   Location::new(0, Span::new(16, 21)),
//! )
//! .with_primary_label("redefined here");
//!
//! let mut out = String::new();
//! Html::new()
//!   .render(&diagnostic, &[Input::new(Source::new(text))], &mut out)
//!   .unwrap();
//!
//! assert!(out.contains(r#"<span class="painty-covered painty-primary">width</span>"#));
//! assert!(out.contains(r#"<p class="painty-label painty-primary">redefined here</p>"#));
//! ```
//!
//! # It is a peer of the terminal renderer, not a style of it
//!
//! `Presentation`, the seam the four terminal styles are written against, is private and this does
//! not implement it. Writing this renderer is what settles that, and it settles it three ways
//! rather than one:
//!
//! * **A glyph is not an element.** One hook *returns* a `char` to stand in one cell, and four more
//!   position a mark by display column. HTML has no cell; its mark is an element wrapped around the
//!   span's bytes and its alignment is the browser's. Those five are unanswerable here whatever
//!   their parameters are re-denominated to.
//! * **The row changes hands.** The terminal writes the source row itself and every style puts its
//!   marks on rows UNDERNEATH it, so the row is the one thing all four styles agreed about and it
//!   stayed in the renderer. Here the mark is INSIDE the row — the line is split at the covered
//!   bytes — so the row cannot be the renderer's. That is not a signature; it is which side of the
//!   seam a whole responsibility sits on.
//! * **What is missing is as telling as what does not fit.** Nothing in that seam opens the render.
//!   A terminal style gets away with it because its header is the first thing written; here a root
//!   element has to be opened before the header and closed after the help, and two of the seam's
//!   parameters — a "is this the first block" flag and a "was anything drawn" flag — exist only to
//!   let a style work out whether its frame is open. An element that closes itself needs neither.
//!
//! What the two renderers share is **layer 2** — [`Source`], [`Region`], [`RegionLine`], [`Line`] —
//! which is stated in lines and byte spans, and which every medium has. Not the terminal's plan:
//! that is private to its module, it is built by asking a `Presentation` and a cell budget
//! questions, and it is four `Vec`s in a renderer that has no allocator.
//!
//! [`Region`]: crate::Region
//! [`Line`]: crate::Line
//!
//! # The class vocabulary
//!
//! **Classes, never inline styles**, so the embedder owns appearance completely. painty writes no
//! `style` attribute, no `<style>` element and no colour of any kind; a caller ships one stylesheet
//! and every diagnostic it renders obeys it.
//!
//! The classes that carry *meaning* are [`Role`]'s — the medium-independent vocabulary the terminal
//! maps to SGR — so a theme's roles and a stylesheet's selectors are the same list. The rest name
//! *structure*, which is the half a terminal has no elements for.
//!
//! | class | on | what it names |
//! |---|---|---|
//! | `painty` | the root `<div>` | one whole diagnostic |
//! | `painty-error`, `painty-warning`, `painty-advice` | the root, the severity word | the rung — [`Role::Severity`] |
//! | `painty-header` | a `<p>` | the severity, the code and the message |
//! | `painty-severity` | a `<span>` | the severity word itself |
//! | `painty-code` | a `<span>` | the machine identifier — [`Role::Code`] |
//! | `painty-message` | a `<span>` | what went wrong |
//! | `painty-block` | a `<section>` | everything drawn for ONE input |
//! | `painty-origin` | a `<p>` | where that input is, and what it is called |
//! | `painty-name` | a `<span>` | the caller's name for the input, when it gave one |
//! | `painty-position` | a `<span>` | `line:column`, in CHARACTER columns |
//! | `painty-mark` | a `<div>` | one marked position: its excerpt and its label |
//! | `painty-primary`, `painty-secondary` | a mark, a covered run, a label | which position it belongs to — [`Role::PrimaryLabel`], [`Role::SecondaryLabel`] |
//! | `painty-excerpt` | a `<pre>` | the source rows of one mark |
//! | `painty-row` | a `<span>` | one source line |
//! | `painty-gutter` | the number cell | the column a terminal draws its bars down — [`Role::Gutter`] |
//! | `painty-number` | the number cell | the line number — [`Role::LineNumber`] |
//! | `painty-text` | a `<span>` | the line's own text — [`Role::SourceText`] |
//! | `painty-covered` | a `<span>` | the bytes the span covers on this row |
//! | `painty-elision` | a row | lines left out between two rows of one span |
//! | `painty-label` | a `<p>` | the phrase attached to a position — the two `Role`s above colour it |
//! | `painty-help` | a `<p>` | what the reader can do — [`Role::Help`] |
//!
//! Two of them repay a sentence. `painty-gutter` and `painty-number` sit on the **same** element,
//! because HTML has no bars for a gutter to be made of and the line-number cell is the whole of the
//! column they would occupy — an embedder that wants the wall draws a `border-right` there.
//! `painty-covered` is emitted **even when it is empty**, which is what a zero-width span is: the
//! terminal has to widen that to one cell to make a caret, and HTML hands an empty element to
//! `::before` instead.
//!
//! # What it does not do, deliberately
//!
//! * **No cell arithmetic.** No display width, no tab expansion, no padding. A tab stays a tab and
//!   `tab-size` decides what it is worth; a CJK ideograph is two columns because the font says so.
//!   That is the whole of the cost the design predicted HTML would not pay.
//! * **No row is ever cut.** The terminal stops a row at
//!   `Terminal::max_rendered_width` cells because a terminal cannot show more; a browser scrolls,
//!   so a cut here would destroy something the medium can display. Output is therefore linear in
//!   the source the caller chose to point at, exactly as the caller's own text is.
//! * **No `Palette`.** Classes are the theming seam, and a palette resolved here would be inline
//!   styles under another name. [`Role`] still governs, through the class names above.
//! * **No `path()`.** [`Diagnostic::path`](crate::Diagnostic::path) is not rendered, on the same
//!   terms as the terminal: the two outputs draw the same thing, and a breadcrumb only one of them
//!   has is a divergence a reader has to discover.
//! * **No merged rows.** Two labels on one line are two `painty-mark` elements, each with its own
//!   excerpt of that line, where the terminal draws the line once and hangs two marker rows under
//!   it. The terminal *has* to merge — its marks are positioned against one shared gutter, so a
//!   line drawn twice is a line a reader has to reconcile. Nothing here is shared between two
//!   marks, so merging would buy a shorter document and cost a mark its own container. It is also
//!   what an allocation-free renderer can afford: merging means grouping by line, and grouping
//!   means somewhere to put the groups.
//! * **No balancing on a refused write.** A writer that refuses mid-render leaves the document with
//!   elements unclosed, and the error says so. The terminal has to be stricter — an SGR opener it
//!   never resets escapes into the terminal's own state and colours everything after the
//!   diagnostic — and an unclosed `<div>` in a string a caller has just been handed an `Err` for
//!   has no equivalent reach.
//!
//! # What a render costs
//!
//! **Rows are bounded by the label count, not by the lines a span covers.** A span reaching across
//! a million lines draws its opening, at most three lines after it, one elision row and its closing
//! — the same six as the terminal, by the same rule, so the two outputs elide the same spans. The
//! rule is restated here rather than shared because the terminal's plan is private to its own
//! module and is built of `Vec`s this renderer has no allocator for;
//! `the_two_renderers_elide_the_same_spans` is what holds the two copies together.
//!
//! **Nothing allocates.** The `html` feature implies no `std` and no `alloc` — CI builds it for
//! `thumbv6m-none-eabi` — so the ordering a plan would sort is done by selection scans over the
//! caller's own labels instead, which cost `O(k²)` comparisons on integers and no memory at all.
//!
//! **Passes over one input: three, independent of the label count**, plus one restart per span that
//! reaches another line and ends after a later-starting span begins. One pass locates the position
//! the block header reports, one carries layer 2's forward walk across every label in the input,
//! and the third is the bounded walk over the lines between a multi-line span's ends. The terminal
//! keeps the restart at zero by draining far ends out of a `BinaryHeap`; that heap is the
//! allocation this renderer does not have.

mod escape;

#[cfg(test)]
mod tests;

use core::fmt;

use escape::Escaped;

use crate::{
  Diagnostic, Input, Position, RegionLine, Role, Severity, Source, Span,
  source::{Walk, clip},
};

/// How many lines after a multi-line span's opening are shown before the rest are elided.
///
/// Three, and the same three the terminal uses, so that a span rendered both ways is elided both
/// ways. It is a layout rule rather than a budget: what it buys is that the row count is a function
/// of the LABEL count, so a two-byte span reaching across a ten-megabyte file costs six rows and
/// not ten million.
const CONTEXT: u64 = 3;

/// Renders a diagnostic and its source as HTML.
///
/// One element per thing, one class per role, and every byte of the caller's text escaped on its
/// way out. See [the module documentation](self) for the class vocabulary and for what this
/// deliberately does not do.
///
/// # Nothing to configure, and it is `#[non_exhaustive]` anyway
///
/// A terminal renderer has to know a palette, a colour capability and a tab width because it
/// resolves appearance itself. This resolves none: appearance is the stylesheet's. So there is
/// nothing to configure today, and the attribute is what keeps that from being a promise — a
/// caller cannot write `Html` as a literal, so a field added later is not a breaking change.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Html;

impl Html {
  /// The renderer.
  #[inline]
  #[must_use]
  pub const fn new() -> Self {
    Self
  }

  /// Writes `diagnostic` against the caller's inputs.
  ///
  /// A location's `source` indexes `inputs`, exactly as it does for the terminal: out of range
  /// draws no block rather than a fabricated one, on the same terms as
  /// [`Location::entire`](crate::Location::entire). A diagnostic that names no drawable position
  /// still renders — its header, its help and nothing between them.
  pub fn render(
    &self,
    diagnostic: &Diagnostic<'_>,
    inputs: &[Input<'_>],
    out: &mut impl fmt::Write,
  ) -> fmt::Result {
    Page { out }.render(diagnostic, inputs)
  }
}

/// One position a diagnostic named that this render can draw.
///
/// `input` is the INDEX into the list the caller passed. The two orders a render needs are both
/// read off `at` and `span` without resolving anything: the blocks come out in the order the inputs
/// first appear, and the marks inside one come out in source order.
#[derive(Debug, Clone, Copy)]
struct Drawable<'a> {
  /// Where this was in the caller's order. The primary is zero.
  at: usize,
  input: usize,
  span: Span,
  text: Option<&'a str>,
  primary: bool,
}

/// Every position the diagnostic names that this render can draw, in the caller's order.
///
/// Returned as an iterator rather than a collection because there is nowhere to collect into, and
/// `Clone` because the two orderings below re-walk it instead of sorting it.
fn drawables<'a>(
  diagnostic: &Diagnostic<'a>,
  supplied: usize,
) -> impl Iterator<Item = Drawable<'a>> + Clone {
  core::iter::once((diagnostic.primary(), diagnostic.primary_label(), true))
    .chain(
      diagnostic
        .labels()
        .iter()
        .map(|label| (label.location(), Some(label.text()), false)),
    )
    .enumerate()
    .filter_map(move |(at, (location, text, primary))| {
      // `try_from` rather than the `as` the terminal spends here. Both are exact on every target
      // painty builds for; this one needs no pair of const assertions to say so, and `html` builds
      // for targets `terminal` does not.
      let input = usize::try_from(location.source()).ok()?;
      if input >= supplied {
        return None;
      }
      Some(Drawable {
        at,
        input,
        span: location.span()?,
        text,
        primary,
      })
    })
}

/// The earliest position of the next input to be drawn, in the order the inputs first appear.
///
/// A selection scan rather than a sort, because a sort needs somewhere to put the result. `all`
/// yields in ascending `at`, so the first match is the smallest, and "this is where its input first
/// appears" is exactly "no earlier position names the same input".
///
/// By first APPEARANCE and not by index: the primary is first in `all`, so ordering the blocks by
/// input index would put another file's label above the position the diagnostic is about.
fn next_block<'a>(
  all: impl Iterator<Item = Drawable<'a>> + Clone,
  after: Option<usize>,
) -> Option<Drawable<'a>> {
  all
    .clone()
    .filter(|position| after.is_none_or(|previous| position.at > previous))
    .find(|position| {
      all
        .clone()
        .all(|other| other.at >= position.at || other.input != position.input)
    })
}

/// The next position of one input in SOURCE order, after the one keyed by `after`.
///
/// Keyed on the span's start as the caller gave it, and on the caller's own order to break a tie.
/// The start is used unclamped on purpose: clamping only ever moves an offset backwards to an atom
/// boundary, which is monotonic, so it cannot invert two positions — and the walk below is total
/// against any order regardless.
fn next_position<'a>(
  all: impl Iterator<Item = Drawable<'a>>,
  input: usize,
  after: Option<(usize, usize)>,
) -> Option<Drawable<'a>> {
  all
    .filter(|position| position.input == input)
    .filter(|position| after.is_none_or(|key| key < (position.span.start(), position.at)))
    .min_by_key(|position| (position.span.start(), position.at))
}

/// The CSS class one [`Role`] is written as.
///
/// Exhaustive, which is the point of routing every class through it: `Role` is `#[non_exhaustive]`
/// for other crates and closed inside this one, so a role added later fails to compile here until
/// HTML has decided what it is called. A table in the module documentation could not do that.
///
/// [`Role::Gutter`] is the one that does not fit cleanly, and the mapping says so rather than
/// hiding it: the gutter is "the bars and brackets down the left of an excerpt", HTML draws no
/// bars, and the class lands on the line-number cell — the column those bars would have occupied.
const fn class(role: Role) -> &'static str {
  match role {
    Role::Severity(Severity::Error) => "painty-error",
    Role::Severity(Severity::Warning) => "painty-warning",
    Role::Severity(Severity::Advice) => "painty-advice",
    Role::PrimaryLabel => "painty-primary",
    Role::SecondaryLabel => "painty-secondary",
    Role::Gutter => "painty-gutter",
    Role::LineNumber => "painty-number",
    Role::SourceText => "painty-text",
    Role::Help => "painty-help",
    Role::Code => "painty-code",
  }
}

/// Which of the two label roles a position carries.
const fn role_of(primary: bool) -> Role {
  if primary {
    Role::PrimaryLabel
  } else {
    Role::SecondaryLabel
  }
}

/// The document being written, and the only thing that decides what reaches the writer unread.
///
/// The split is the terminal painter's, for the same reason: [`markup`](Self::markup) is a literal
/// in this crate, [`number`](Self::number) is a number painty computed, and everything a caller
/// supplied goes through [`text`](Self::text) or [`display`](Self::display). An escaping census is
/// then a question about two method names rather than about every string in the file.
struct Page<'a> {
  out: &'a mut dyn fmt::Write,
}

impl Page<'_> {
  /// painty's own markup, written as it is.
  fn markup(&mut self, text: &'static str) -> fmt::Result {
    self.out.write_str(text)
  }

  /// A number painty computed — a line, a column — which no caller can put a `<` into.
  fn number(&mut self, number: u64) -> fmt::Result {
    self.out.write_fmt(format_args!("{number}"))
  }

  /// Caller-supplied text.
  fn text(&mut self, text: &str) -> fmt::Result {
    // Fully qualified rather than `write!` over an imported trait, matching the terminal painter:
    // the numeric census cannot see through a renamed import and offers no allowlist.
    fmt::Write::write_str(&mut Escaped(self.out), text)
  }

  /// A caller-supplied [`fmt::Display`], whose own impl writes into the document.
  fn display(&mut self, value: &dyn fmt::Display) -> fmt::Result {
    fmt::Write::write_fmt(&mut Escaped(self.out), format_args!("{value}"))
  }

  /// Opens an element carrying painty's own classes.
  fn open(&mut self, element: &'static str, classes: &[&'static str]) -> fmt::Result {
    self.markup("<")?;
    self.markup(element)?;
    self.markup(" class=\"")?;
    for (index, class) in classes.iter().enumerate() {
      if index > 0 {
        self.markup(" ")?;
      }
      self.markup(class)?;
    }
    self.markup("\">")
  }

  /// Closes one.
  fn close(&mut self, element: &'static str) -> fmt::Result {
    self.markup("</")?;
    self.markup(element)?;
    self.markup(">")
  }

  /// One lifetime for the diagnostic and the inputs, unified by variance at the call above. The
  /// two orderings below are opaque iterator types, and an opaque type's item is not covariant, so
  /// they have to be produced at one lifetime rather than shortened to one.
  fn render<'a>(&mut self, diagnostic: &Diagnostic<'a>, inputs: &[Input<'a>]) -> fmt::Result {
    // The root element is opened before anything is written into it and closed after the help,
    // which is what a terminal style has to spend two hooks and a `Drawn` flag to approximate.
    self.open(
      "div",
      &["painty", class(Role::Severity(diagnostic.severity()))],
    )?;
    self.markup("\n")?;
    self.header(diagnostic)?;

    let mut previous = None;
    while let Some(earliest) = next_block(drawables(diagnostic, inputs.len()), previous) {
      previous = Some(earliest.at);
      self.block(
        earliest,
        inputs[earliest.input],
        drawables(diagnostic, inputs.len()),
      )?;
    }

    if let Some(help) = diagnostic.help() {
      self.open("p", &[class(Role::Help)])?;
      self.text(help)?;
      self.close("p")?;
      self.markup("\n")?;
    }

    self.close("div")?;
    self.markup("\n")
  }

  /// The severity, the code and the message.
  fn header(&mut self, diagnostic: &Diagnostic<'_>) -> fmt::Result {
    self.open("p", &["painty-header"])?;
    self.open("span", &["painty-severity"])?;
    // painty's own word for the rung, not the caller's.
    self.markup(diagnostic.severity().as_str())?;
    self.close("span")?;
    self.open("span", &[class(Role::Code)])?;
    self.text(diagnostic.code())?;
    self.close("span")?;
    self.open("span", &["painty-message"])?;
    self.display(diagnostic.message())?;
    self.close("span")?;
    self.close("p")?;
    self.markup("\n")
  }

  /// Everything drawn for one input.
  fn block<'a>(
    &mut self,
    earliest: Drawable<'a>,
    input: Input<'a>,
    all: impl Iterator<Item = Drawable<'a>> + Clone,
  ) -> fmt::Result {
    let source = input.source();
    let mut walk = Walk::new(source);
    self.open("section", &["painty-block"])?;
    self.markup("\n")?;
    // WHICH position the block reports is the caller's earliest in this input, not the topmost row
    // drawn — the terminal's choice, kept so the two outputs name the same place. A multi-line span
    // opening twenty lines above the primary would otherwise announce a line nobody asked about.
    //
    // A CHARACTER column, for the same reason the terminal's is one: this triple is what a reader
    // copies into an editor, and every editor means characters by a column.
    //
    // Asked of the walk rather than of `Source::position`, so that the block's second pass is the
    // walk's own restart rather than a scan of its own: the earliest CALLER position is rarely the
    // earliest SOURCE position, and one of the two orders has to be paid for.
    self.origin(input.origin(), walk.open(earliest.span).at())?;

    let mut previous = None;
    while let Some(position) = next_position(all.clone(), earliest.input, previous) {
      previous = Some((position.span.start(), position.at));
      self.mark(&mut walk, source, position)?;
    }

    self.close("section")?;
    self.markup("\n")
  }

  /// What the input is called, and where in it the block begins.
  fn origin(&mut self, origin: Option<&str>, at: Position) -> fmt::Result {
    self.open("p", &["painty-origin"])?;
    if let Some(origin) = origin {
      self.open("span", &["painty-name"])?;
      self.text(origin)?;
      self.close("span")?;
    }
    self.open("span", &["painty-position"])?;
    self.number(at.line())?;
    self.markup(":")?;
    self.number(at.column())?;
    self.close("span")?;
    self.close("p")?;
    self.markup("\n")
  }

  /// One position: the rows it is drawn on, and the phrase attached to it.
  ///
  /// A container per position, which the terminal has no hook for and no need of — there, a mark's
  /// rows are interleaved with every other mark's under one shared gutter. Here they are one
  /// element, so a browser can fold it, link to it, or lay it beside its label.
  fn mark<'a>(
    &mut self,
    walk: &mut Walk<'a>,
    source: Source<'a>,
    position: Drawable<'a>,
  ) -> fmt::Result {
    let opening = walk.open(position.span);
    let span = opening.span();
    let role = class(role_of(position.primary));

    self.open("div", &["painty-mark", role])?;
    self.markup("\n")?;
    self.open("pre", &["painty-excerpt"])?;
    self.row(opening.line(), role)?;

    if opening.reaches_another_line() {
      let closing = walk.close(&opening);
      let first = opening.line().line().number();
      let last = closing.line().number();

      // The terminal's rule, restated: the lines after the opening are shown up to `CONTEXT`, and
      // one line left over reads worse as a gap than as the line itself — which is the shape a span
      // of exactly six lines leaves behind.
      let mut shown = (first + CONTEXT).min(last - 1);
      if last - shown == 2 {
        shown = last - 1;
      }

      let mut line = opening.line().line();
      while line.number() < shown {
        let Some(next) = source.line_after(line) else {
          break;
        };
        line = next;
        self.markup("\n")?;
        self.row(clip(line, span), role)?;
      }
      if line.number() < last - 1 {
        self.markup("\n")?;
        self.elision()?;
      }
      self.markup("\n")?;
      self.row(closing, role)?;
    }

    self.close("pre")?;
    self.markup("\n")?;
    if let Some(text) = position.text {
      self.open("p", &["painty-label", role])?;
      self.text(text)?;
      self.close("p")?;
      self.markup("\n")?;
    }
    self.close("div")?;
    self.markup("\n")
  }

  /// One source row: the number, then the line with the covered bytes wrapped.
  ///
  /// The split is the whole difference between this renderer and a terminal style. There the source
  /// row is written by the renderer and every mark goes on a row UNDER it, so no style participates
  /// in the line itself; here the mark is INSIDE the line, and the row cannot be written without
  /// knowing where the span falls.
  fn row(&mut self, line: RegionLine<'_>, role: &'static str) -> fmt::Result {
    self.open("span", &["painty-row"])?;
    self.open("span", &[class(Role::Gutter), class(Role::LineNumber)])?;
    self.number(line.line().number())?;
    self.close("span")?;
    self.open("span", &[class(Role::SourceText)])?;

    // Both ends are character boundaries: the span's, because resolution moved them out to their
    // atoms, and the line's, because a line is delimited by breaks. So the three slices are
    // infallible, and the covered one is exactly `RegionLine::covered_text`.
    let text = line.line().text();
    let base = line.line().span().start();
    let from = line.covered().start() - base;
    let to = line.covered().end() - base;

    self.text(&text[..from])?;
    // Emitted even when it is empty, which is what a zero-width span is. The terminal has to widen
    // that to one cell to make a caret visible; an empty element is the same statement in a medium
    // where `::before` can draw one.
    self.open("span", &["painty-covered", role])?;
    self.text(&text[from..to])?;
    self.close("span")?;
    self.text(&text[to..])?;

    self.close("span")?;
    self.close("span")
  }

  /// The row standing for the lines left out between two rows of one span.
  ///
  /// The glyph is painty's, because content is: an embedder that could only restyle it would be
  /// left with a gap that says nothing in an unstyled document, which is the same silent cut the
  /// terminal's `…` exists to prevent.
  fn elision(&mut self) -> fmt::Result {
    self.open("span", &["painty-row", "painty-elision"])?;
    self.open("span", &[class(Role::Gutter), class(Role::LineNumber)])?;
    self.markup("\u{22ee}")?;
    self.close("span")?;
    self.open("span", &[class(Role::SourceText)])?;
    self.close("span")?;
    self.close("span")
  }
}
