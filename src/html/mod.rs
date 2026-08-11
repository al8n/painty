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
//! assert!(out.contains(r#"<span class="painty-label painty-primary">redefined here</span>"#));
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
//! which is stated in lines and byte spans, and which every medium has, and **the elision rule**,
//! which is one function both call. Not the terminal's plan: that is private to its module, it is
//! built by asking a `Presentation` and a cell budget questions, and it is four `Vec`s in a renderer
//! that has no allocator.
//!
//! The elision rule is shared because it was not, for one review round, and the first thing two
//! copies of it did was disagree about which lines a reader is shown. It is worth being exact about
//! what that cost, because it was not drift: both copies computed the same arithmetic, and they
//! applied it to different inputs — the terminal to a whole input's anchors, this renderer to each
//! mark's own two ends. So the repair was both halves, the rule *and* the input, and the anchors are
//! now this renderer's too.
//!
//! [`Region`]: crate::Region
//! [`RegionLine`]: crate::RegionLine
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
//! | `painty-excerpt` | a `<pre>` | every row of one input |
//! | `painty-row` | a `<span>` | one row: a source line, a label, or a gap |
//! | `painty-gutter` | the number cell | the column a terminal draws its bars down — [`Role::Gutter`] |
//! | `painty-number` | the number cell | the line number — [`Role::LineNumber`] |
//! | `painty-text` | a `<span>` | the line's own text — [`Role::SourceText`] |
//! | `painty-covered` | a `<span>` | one run of the line that a mark covers |
//! | `painty-primary`, `painty-secondary` | a covered run, a label | which position it belongs to — [`Role::PrimaryLabel`], [`Role::SecondaryLabel`] |
//! | `painty-annotation` | a row | a row saying a label rather than showing source |
//! | `painty-label` | a `<span>` | the phrase attached to a position |
//! | `painty-elision` | a row | lines left out between two rows |
//! | `painty-help` | a `<p>` | what the reader can do — [`Role::Help`] |
//!
//! Three of them repay a sentence. `painty-gutter` and `painty-number` sit on the **same** element,
//! because HTML has no bars for a gutter to be made of and the line-number cell is the whole of the
//! column they would occupy — an embedder that wants the wall draws a `border-right` there.
//! `painty-covered` is emitted **even when it is empty**, which is what a zero-width span is: the
//! terminal has to widen that to one cell to make a caret, and HTML hands an empty element to
//! `::before` instead. And a run covered by two overlapping marks carries **both** roles, because
//! two sibling elements cannot overlap and the line is cut at every mark boundary instead — which
//! is the thing a terminal solves by giving each mark a row of its own.
//!
//! # What it does not do, deliberately
//!
//! * **No cell arithmetic.** No display width, no tab expansion, no padding. A tab stays a tab and
//!   `tab-size` decides what it is worth; a CJK ideograph is two columns because the font says so.
//!   That is the whole of the cost the design predicted HTML would not pay.
//! * **No row is ever cut.** The terminal stops a row at `Terminal::max_rendered_width` cells
//!   because a terminal cannot show more; a browser scrolls, so a cut here would destroy something
//!   the medium can display.
//! * **No `Palette`.** Classes are the theming seam, and a palette resolved here would be inline
//!   styles under another name. [`Role`] still governs, through the class names above.
//! * **No `path()`.** [`Diagnostic::path`](crate::Diagnostic::path) is not rendered, on the same
//!   terms as the terminal: the two outputs draw the same thing, and a breadcrumb only one of them
//!   has is a divergence a reader has to discover.
//! * **No balancing on a refused write.** A writer that refuses mid-render leaves the document with
//!   elements unclosed, and the error says so. The terminal has to be stricter — an SGR opener it
//!   never resets escapes into the terminal's own state and colours everything after the
//!   diagnostic — and an unclosed `<div>` in a string a caller has just been handed an `Err` for
//!   has no equivalent reach.
//!
//! # What a render costs, and the budget it does not need
//!
//! The terminal's two policy constants bound a **row**: `max_source_bytes` bounds the bytes its
//! geometry examines to place one, and `max_rendered_width` bounds the cells it emits. Both are
//! about the conversion into display cells. **Neither axis exists here**, because this renderer
//! performs no per-row geometry: it slices a line at boundaries layer 2 has already put on
//! character boundaries. So there is no second budget vocabulary, and none is invented.
//!
//! What is bounded, and by what:
//!
//! * **Rows: by the label count.** The rows of a block are its **anchors** — the distinct lines an
//!   end of any span falls on — plus at most three context lines after each bracket and one gap row.
//!   A span reaching across a million lines costs six rows, not a million. Same rule as the
//!   terminal, and now literally the same function; `the_two_renderers_draw_the_same_rows` holds
//!   them together over a grid of span length by mark placement.
//! * **Emitted source: one copy of the lines drawn.** No line is drawn twice, which the same grid
//!   asserts. It was not always so: with an excerpt per MARK, sixty-four labels on one
//!   four-hundred-byte line emitted forty-five kilobytes where the terminal emitted seven, and the
//!   amplification was in the line's own length. So the output is `O(input + k)` and there is
//!   nothing here for a budget to cap.
//! * **Passes over one input: two, whatever the label count.** One locates the position the origin
//!   reports; one visits every span's two ends in ascending offset order. The second never turns
//!   round, which is what the terminal buys with a `BinaryHeap` of far ends and what this buys with
//!   an ordering scan. Rendering mark by mark used to cost one restart per span reaching past a
//!   later one, and measured 4.9× the terminal at thirty-two nested spans; it is now 0.4×.
//!
//! **The one axis that is this renderer's own is `O(k²)` CPU in the caller's label count**, and it
//! is stated rather than capped. Ordering without memory is a selection scan — of the stops, of the
//! anchors, of the labels under a line — where the terminal sorts once and searches. Measured on
//! this machine at 0.028 µs·k²: 0.2 ms at a hundred labels, 31 ms at a thousand, 0.47 s at four
//! thousand, against the terminal's `O(k log k)` 2.3 ms and 9.2 ms. It is integer comparisons over
//! an array the caller has already built and paid `O(k)` memory for; it allocates nothing and it
//! emits nothing extra. A ceiling would have to *drop* labels, which is the one thing this crate
//! refuses to do, so the number is published instead. What would remove it is a caller-provided
//! scratch buffer to sort in — the same answer §10 of the design already contemplates for elision
//! in layer 2 — and nothing here forecloses it.

mod escape;

#[cfg(test)]
mod tests;

use core::fmt;

use escape::Escaped;

use crate::{
  Diagnostic, Input, Line, Position, Role, Severity, Source, Span, elide,
  source::{Walk, clip, draws_on, ends_on},
};

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

  /// Everything drawn for one input: its origin, then its rows in source order.
  ///
  /// # One pass, and one row per line
  ///
  /// The rows are the input's **anchors** — every line an end of any span falls on — with the lines
  /// between them filled in or elided by [`elide::shown_between`], which is the terminal's rule and
  /// is now the same function. Deciding it per MARK instead was this renderer's first defect: a
  /// seven-line span with a secondary label on line five elided lines five and six while another
  /// mark drew line five, so line six was in no output at all and the gap stood for a line the same
  /// document showed.
  ///
  /// The anchors are reached by visiting every span's two ends in **ascending offset order**, a
  /// close before an open where they land together. That is what the terminal buys with a
  /// `BinaryHeap` of far ends; here it is a selection scan over the caller's own labels, which needs
  /// no memory, and it means the walk never turns round. A block therefore costs two passes over its
  /// input — one to locate the position the origin reports, one for the stops — whatever the label
  /// count, where rendering mark by mark cost one restart per span that reached past a later one.
  fn block<'a>(
    &mut self,
    earliest: Drawable<'a>,
    input: Input<'a>,
    all: impl Iterator<Item = Drawable<'a>> + Clone,
  ) -> fmt::Result {
    let source = input.source();
    let marks = in_input(all, earliest.input, source);

    self.open("section", &["painty-block"])?;
    self.markup("\n")?;
    // WHICH position the block reports is the caller's earliest in this input, not the topmost row
    // drawn — the terminal's choice, kept so the two outputs name the same place. A multi-line span
    // opening twenty lines above the primary would otherwise announce a line nobody asked about.
    //
    // A CHARACTER column, for the same reason the terminal's is one: this triple is what a reader
    // copies into an editor, and every editor means characters by a column.
    self.origin(input.origin(), source.position(earliest.span.start()))?;
    self.open("pre", &["painty-excerpt"])?;

    let mut walk = Walk::new(source);
    let mut first = true;
    let mut above: Option<Line<'a>> = None;
    let mut stop = None;
    while let Some(next) = next_stop(marks.clone(), stop) {
      stop = Some(next);
      let (offset, opens) = next;
      let anchor = if opens {
        walk.opens_on(offset)
      } else {
        walk.closes_on(offset)
      };
      // Several stops land on one line — a span's own two ends where it is drawn on one line, two
      // labels on one line, one span closing where another opens. The anchors are the DISTINCT
      // lines, and the stops arrive in ascending order, so the repeats are consecutive.
      if above.is_some_and(|previous| previous.number() >= anchor.number()) {
        continue;
      }

      if let Some(previous) = above {
        let shown = elide::shown_between(
          previous.number(),
          anchor.number(),
          opens_a_bracket(marks.clone(), previous),
        );
        let mut line = previous;
        while line.number() < shown {
          let Some(after) = source.line_after(line) else {
            break;
          };
          line = after;
          self.separate(&mut first)?;
          self.row(line, marks.clone())?;
        }
        // Read off the line actually reached rather than off `shown`, so a source that ends early
        // says so with a gap instead of claiming rows it never drew.
        if line.number() < anchor.number() - 1 {
          self.separate(&mut first)?;
          self.elision()?;
        }
      }
      self.separate(&mut first)?;
      self.row(anchor, marks.clone())?;
      above = Some(anchor);
    }

    self.close("pre")?;
    self.markup("\n")?;
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

  /// A break between two rows of one excerpt, and nothing before the first.
  ///
  /// `<pre>` keeps what it is given: a break before the first row is a blank line an HTML parser
  /// silently swallows, and one after the last is a blank line it does not.
  fn separate(&mut self, first: &mut bool) -> fmt::Result {
    if *first {
      *first = false;
      return Ok(());
    }
    self.markup("\n")
  }

  /// One source row: the number, the line with every mark on it wrapped, and a row for each label
  /// said here.
  ///
  /// The line is drawn ONCE however many labels fall on it, which is the terminal's rule too. It
  /// used to be drawn once per label, because each mark had an excerpt of its own — sixty-four
  /// labels on one four-hundred-byte line produced forty-five kilobytes against the terminal's
  /// seven, and the amplification was in the line's own length.
  fn row<'a>(
    &mut self,
    line: Line<'a>,
    marks: impl Iterator<Item = (Drawable<'a>, Span)> + Clone,
  ) -> fmt::Result {
    self.open("span", &["painty-row"])?;
    self.open("span", &[class(Role::Gutter), class(Role::LineNumber)])?;
    self.number(line.number())?;
    self.close("span")?;
    self.open("span", &[class(Role::SourceText)])?;
    self.runs(line, marks.clone())?;
    self.close("span")?;
    self.close("span")?;

    // A label is said where its span closes, in the caller's order under one line — both the
    // terminal's rules, kept so a reader moving between the two outputs finds them in one order.
    let mut previous = None;
    while let Some(position) = next_label(marks.clone(), line, previous) {
      previous = Some(position.at);
      self.markup("\n")?;
      self.annotation(position)?;
    }
    Ok(())
  }

  /// The line's own text, split wherever a mark begins or ends.
  ///
  /// # Segments rather than one element per mark
  ///
  /// Two spans can overlap on one line, and two sibling elements cannot. So the line is cut at every
  /// mark boundary and each run carries the roles of the marks covering IT — which handles
  /// overlapping, nested and disjoint spans by the same arithmetic, and is the thing a terminal
  /// solves instead by giving every mark a row of its own.
  fn runs<'a>(
    &mut self,
    line: Line<'a>,
    marks: impl Iterator<Item = (Drawable<'a>, Span)> + Clone,
  ) -> fmt::Result {
    let content = line.span();
    let text = line.text();
    let base = content.start();
    // Only the marks this line shows, each clipped to it — layer 2's own two rules, so a span that
    // is somewhere else cannot contribute a degenerate range here.
    let here = marks
      .filter(move |(_, span)| draws_on(line, *span))
      .map(move |(position, span)| (position.primary, clip(line, span).covered()));

    let mut at = content.start();
    loop {
      // A zero-width mark is a caret: an empty element between two characters, which `::before` can
      // draw and which the terminal has to widen to a whole cell to make visible at all.
      let sitting = roles(
        here
          .clone()
          .filter(|(_, covered)| covered.is_empty() && covered.start() == at),
      );
      if sitting.0 || sitting.1 {
        self.covered(sitting)?;
        self.close("span")?;
      }
      if at == content.end() {
        return Ok(());
      }

      // Every boundary is a cut, so no mark begins or ends strictly inside the run below and each
      // one either covers the whole of it or none of it.
      let next = here
        .clone()
        .flat_map(|(_, covered)| [covered.start(), covered.end()])
        .filter(|boundary| *boundary > at)
        .min()
        .unwrap_or(content.end());
      let covering = roles(here.clone().filter(|(_, covered)| {
        !covered.is_empty() && covered.start() <= at && covered.end() >= next
      }));
      let run = &text[at - base..next - base];
      if covering.0 || covering.1 {
        self.covered(covering)?;
        self.text(run)?;
        self.close("span")?;
      } else {
        self.text(run)?;
      }
      at = next;
    }
  }

  /// Opens the element around one run, in the roles of the marks covering it.
  fn covered(&mut self, (primary, secondary): (bool, bool)) -> fmt::Result {
    match (primary, secondary) {
      (true, true) => self.open(
        "span",
        &[
          "painty-covered",
          class(Role::PrimaryLabel),
          class(Role::SecondaryLabel),
        ],
      ),
      (true, false) => self.open("span", &["painty-covered", class(Role::PrimaryLabel)]),
      _ => self.open("span", &["painty-covered", class(Role::SecondaryLabel)]),
    }
  }

  /// The row a label is said on, under the source row its span closes on.
  ///
  /// The terminal's marker row without the markers: its number field is blank for the same reason,
  /// which is that the row is about the one above it.
  fn annotation(&mut self, position: Drawable<'_>) -> fmt::Result {
    self.open("span", &["painty-row", "painty-annotation"])?;
    self.open("span", &[class(Role::Gutter), class(Role::LineNumber)])?;
    self.close("span")?;
    self.open("span", &["painty-label", class(role_of(position.primary))])?;
    self.text(
      position
        .text
        .expect("a label row is written only for a position that has one"),
    )?;
    self.close("span")?;
    self.close("span")
  }

  /// The row standing for the lines left out between two anchors.
  ///
  /// The glyph is painty's, because content is: an embedder that could only restyle it would be
  /// left with a gap that says nothing in an unstyled document, which is the same silent cut the
  /// terminal's `...` exists to prevent.
  fn elision(&mut self) -> fmt::Result {
    self.open("span", &["painty-row", "painty-elision"])?;
    self.open("span", &[class(Role::Gutter), class(Role::LineNumber)])?;
    self.markup("\u{22ee}")?;
    self.close("span")?;
    self.close("span")
  }
}

/// One input's positions, each with the byte range it actually resolves to.
///
/// Clamped once at the top rather than at every question below: the order the three clamping steps
/// run in has erased a character before now, and every predicate here is a comparison against a
/// clamped end.
fn in_input<'a>(
  all: impl Iterator<Item = Drawable<'a>> + Clone,
  input: usize,
  source: Source<'a>,
) -> impl Iterator<Item = (Drawable<'a>, Span)> + Clone {
  all
    .filter(move |position| position.input == input)
    .map(move |position| {
      let span = source.clamped(position.span);
      (position, span)
    })
}

/// The next place the walk has to stop, after the one it is standing on.
///
/// Every span's two ends, in ascending offset order. **A close goes first where two land on one
/// offset**, which is the terminal's rule and is load-bearing for the same reason: a walk asked for
/// an offset it is already standing on cannot see that a line break ended there, and that is the one
/// question a close exists to answer. `false` sorts before `true`, so the pair orders itself.
fn next_stop<'a>(
  marks: impl Iterator<Item = (Drawable<'a>, Span)>,
  after: Option<(usize, bool)>,
) -> Option<(usize, bool)> {
  marks
    .flat_map(|(_, span)| [(span.end(), false), (span.start(), true)])
    .filter(|stop| after.is_none_or(|previous| previous < *stop))
    .min()
}

/// Whether any span that reaches a later line OPENS on this one.
///
/// The one thing about the marks that [`elide::shown_between`] needs. Only a bracket earns context
/// after it; an anchor that is merely where something ended, or where a single-line label sits, is
/// followed straight by the gap.
fn opens_a_bracket<'a>(
  mut marks: impl Iterator<Item = (Drawable<'a>, Span)>,
  line: Line<'_>,
) -> bool {
  let content = line.span();
  marks.any(|(_, span)| {
    content.start() <= span.start() && span.start() <= content.end() && !ends_on(line, span)
  })
}

/// The next label said under `line`, after the one at `after` in the caller's order.
fn next_label<'a>(
  marks: impl Iterator<Item = (Drawable<'a>, Span)>,
  line: Line<'_>,
  after: Option<usize>,
) -> Option<Drawable<'a>> {
  marks
    .filter(|(position, span)| position.text.is_some() && ends_on(line, *span))
    .map(|(position, _)| position)
    .filter(|position| after.is_none_or(|previous| position.at > previous))
    .min_by_key(|position| position.at)
}

/// Which roles a run carries: primary, secondary, or both where two marks overlap it.
fn roles(marks: impl Iterator<Item = (bool, Span)>) -> (bool, bool) {
  marks.fold((false, false), |(primary, secondary), (is_primary, _)| {
    (primary || is_primary, secondary || !is_primary)
  })
}
