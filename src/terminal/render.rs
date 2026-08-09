use core::fmt;

use super::{
  ColorCapability, LineCells,
  width::{Budget, Mark, Row, control_picture},
};
use crate::{
  Color, Diagnostic, Line, Palette, RegionLine, Role, Source, Span, Style, Theme, source::Walk,
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
struct Drawable<'a> {
  at: usize,
  input: usize,
  from: Input<'a>,
  span: Span,
  text: Option<&'a str>,
  primary: bool,
}

/// What one marker row says, and where the position under it starts.
#[derive(Debug, Clone, Copy)]
struct Phrase<'a> {
  text: Option<&'a str>,
  primary: bool,
  /// The 1-based CHARACTER column the position begins at, which is what the `-->` line reports for
  /// the first position of each input. Read off the resolve that found the line rather than
  /// measured again: layer 2 computed it on the way past, and it is the same number
  /// [`Position::column`](crate::Position::column) hands any other consumer.
  column: u64,
  /// Where this was in the caller's order.
  at: usize,
}

/// One source line, and every mark that will be drawn under it.
///
/// `marks` indexes one flat run rather than owning a `Vec` of its own: the marker rows of a whole
/// render are a single allocation, and an excerpt names its slice of them.
#[derive(Debug, Clone)]
struct Excerpt<'a> {
  input: usize,
  origin: Option<&'a str>,
  line: Line<'a>,
  /// The earliest caller position on this line, which is where the line sits within its input.
  at: usize,
  /// The earliest caller position anywhere in this input, which is where the input sits.
  input_at: usize,
  marks: core::ops::Range<usize>,
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
/// A span covering more than one line is drawn on the **first** line it touches. Brackets and the
/// connectors that join a span's ends across lines are a row-assignment problem, and they are the
/// expensive part of a terminal renderer rather than an afterthought; they arrive next — as is
/// merging several markers onto ONE row, which is the same problem. What is here is total and
/// correct about what it draws, and it is not yet everything a reader will eventually want drawn.
///
/// # What k labels cost
///
/// Per input: **one forward pass** over its text, whatever the label count — layer 2 walks
/// forwards, so the positions are resolved in ascending order off a carried cursor. Per drawn line:
/// **one** scan for where the line ends, **one** bounded walk over its geometry, and **one** source
/// row. Per label: one marker row, and the label's own text once.
///
/// That last one is the honest floor rather than a gap. Two labels on a line are two things to
/// point at and two things to say, and no arrangement of rows makes them one. Everything that is
/// painty's — the resolve, the line scan, the geometry, the excerpt — is once per line.
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
    }
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
    let cells = LineCells::new(drawn.line(), self.tab_width);
    let mut marks = [Mark::new(drawn.covered(), ())];
    cells.place_marks(&mut marks, Self::budget());
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

  /// What one excerpt of this renderer may spend.
  fn budget() -> Budget {
    Budget {
      cells: Self::max_rendered_width(),
      bytes: Self::max_source_bytes(),
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
    let severity = diagnostic.severity();
    self.styled(out, Role::Severity(severity), severity.as_str())?;
    self.styled(out, Role::Code, "[")?;
    self.styled(out, Role::Code, diagnostic.code())?;
    self.styled(out, Role::Code, "]")?;
    out.write_str(": ")?;
    write_shown(out, diagnostic.message())?;
    out.write_char('\n')?;

    // Every position the diagnostic names, in the order the caller gave them, and then only those
    // that can be drawn at all: an input the caller did not supply and a position with no span both
    // draw nothing, on the same terms as `Location::entire`.
    let mut positions = Vec::new();
    positions.push((diagnostic.primary(), diagnostic.primary_label(), true));
    for label in diagnostic.labels() {
      positions.push((label.location(), Some(label.text()), false));
    }
    let mut drawable: Vec<Drawable<'_>> = positions
      .into_iter()
      .enumerate()
      .filter_map(|(at, (location, text, primary))| {
        let input = location.source() as usize;
        Some(Drawable {
          at,
          input,
          from: *inputs.get(input)?,
          span: location.span()?,
          text,
          primary,
        })
      })
      .collect();

    // RESOLUTION order, which is not the order any of this is drawn in. Layer 2 walks forwards, so
    // a set of spans resolved in ascending order over one input costs one pass; resolved in the
    // caller's order it costs one pass EACH, and on a multi-megabyte line each pass is the whole
    // line again. The caller's order is carried in `at` and put back below.
    drawable.sort_unstable_by_key(|position| (position.input, position.span.start(), position.at));

    // One excerpt per input LINE, not per label. Two labels on one line used to be two excerpts,
    // each re-resolving the span, re-scanning for the line's end and rewriting the whole source row
    // under its own copy of the label text — so a bounded source window and a borrowed string came
    // out k times. They share the row now, and the k that remains is k marker rows, which is the
    // one part of this that genuinely cannot be one.
    //
    // Coalescing CONSECUTIVE entries is enough because the sort put them there: line number rises
    // with the start offset, so every position on one line of one input is a contiguous run.
    let mut excerpts: Vec<Excerpt<'_>> = Vec::new();
    let mut marks: Vec<Mark<Phrase<'_>>> = Vec::new();
    let mut walk: Option<(usize, Walk<'_>)> = None;
    for position in &drawable {
      let walking = match &mut walk {
        Some((input, walking)) if *input == position.input => walking,
        _ => {
          &mut walk
            .insert((position.input, Walk::new(position.from.source)))
            .1
        }
      };
      let (starts, drawn) = walking.first_line(position.span);

      match excerpts.last_mut() {
        Some(last)
          if last.input == position.input && last.line.number() == drawn.line().number() =>
        {
          last.at = last.at.min(position.at);
          last.marks.end += 1;
        }
        _ => excerpts.push(Excerpt {
          input: position.input,
          origin: position.from.origin,
          line: drawn.line(),
          at: position.at,
          input_at: position.at,
          marks: marks.len()..marks.len() + 1,
        }),
      }
      marks.push(Mark::new(
        drawn.covered(),
        Phrase {
          text: position.text,
          primary: position.primary,
          column: starts.column(),
          at: position.at,
        },
      ));
    }

    // DRAWING order, and the two sorts below are what puts the caller's order back.
    //
    // By the order each input FIRST appears, not by its index: the primary is pushed first, so
    // sorting by index would move another file's label above the position the diagnostic is
    // actually about. An input's first appearance is the earliest caller position anywhere in it,
    // which the sort above has already made contiguous — so it is one linear pass, where asking
    // "have I seen this input" per excerpt was a scan of the distinct inputs per excerpt, and k
    // labels over d empty inputs kept everything else at O(k) while that went quadratic.
    let mut run = 0;
    while run < excerpts.len() {
      let input = excerpts[run].input;
      let mut end = run;
      let mut earliest = usize::MAX;
      while end < excerpts.len() && excerpts[end].input == input {
        earliest = earliest.min(excerpts[end].at);
        end += 1;
      }
      for excerpt in &mut excerpts[run..end] {
        excerpt.input_at = earliest;
      }
      run = end;
    }
    excerpts.sort_unstable_by_key(|excerpt| (excerpt.input_at, excerpt.at));
    // And the marker rows under one line are in the caller's order too, so the primary — pushed
    // first — leads whether or not it is the leftmost thing on the line.
    for excerpt in &excerpts {
      marks[excerpt.marks.clone()].sort_unstable_by_key(|mark| mark.payload().at);
    }

    // Sized before anything is written, because the gutter is as wide as the widest number it will
    // show and the first row does not know what is coming.
    let gutter = excerpts
      .iter()
      .map(|excerpt| digits(excerpt.line.number()))
      .max()
      .unwrap_or(1);

    // A header per input, and the key is the input INDEX rather than the origin string. Resolving
    // each span against its own input was half the multi-input fix; the other half is saying which
    // input a row came from, because a secondary label in another file was otherwise drawn under
    // the first file's header — the reader told, confidently and silently, that text came from a
    // file it did not come from. Two inputs with no origin, or with the same one, are still two
    // inputs, and only the index distinguishes them.
    let mut shown_input = None;
    for excerpt in &excerpts {
      let cells = LineCells::new(excerpt.line, self.tab_width);
      let placed = &mut marks[excerpt.marks.clone()];
      let Some(leading) = placed.first().map(|mark| *mark.payload()) else {
        continue;
      };
      let row = cells.place_marks(placed, Self::budget());

      if shown_input != Some(excerpt.input) {
        // A CHARACTER column, and the marker row below is in DISPLAY columns. Two units in one
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
        // The marker row stays in display columns because it is read by a HUMAN looking at the row
        // above it, where a caret has to sit under the glyph a terminal painted.
        //
        // It costs nothing, which is the smaller half of the argument and worth writing down: the
        // resolve that found this line computed the character column on its way past, so the header
        // is a value layer 2 already produced. The display column was a grapheme walk over every
        // cluster before the offset, unbounded, and the dominant term of a whole render at k=1.
        pad(out, gutter)?;
        out.write_str("--> ")?;
        if let Some(origin) = excerpt.origin {
          write_shown(out, origin)?;
          out.write_char(':')?;
        }
        writeln!(out, "{}:{}", excerpt.line.number(), leading.column)?;
        self.bar(out, gutter)?;
        shown_input = Some(excerpt.input);
      }
      self.excerpt(out, gutter, &cells, &row, placed)?;
      self.bar(out, gutter)?;
    }

    if let Some(help) = diagnostic.help() {
      pad(out, gutter + 1)?;
      out.write_str("= ")?;
      self.styled(out, Role::Help, "help")?;
      out.write_str(": ")?;
      self.styled(out, Role::Help, help)?;
      out.write_char('\n')?;
    }
    Ok(())
  }

  /// One `  |` separator row.
  fn bar(&self, out: &mut impl fmt::Write, gutter: u64) -> fmt::Result {
    pad(out, gutter + 1)?;
    self.styled(out, Role::Gutter, "|")?;
    out.write_char('\n')
  }

  /// One source line, written once, and a marker row for every mark on it.
  fn excerpt(
    &self,
    out: &mut impl fmt::Write,
    gutter: u64,
    cells: &LineCells<'_>,
    row: &Row,
    placed: &[Mark<Phrase<'_>>],
  ) -> fmt::Result {
    let number = cells.line().number();
    pad(out, gutter - digits(number))?;
    self.styled(out, Role::LineNumber, &number.to_string())?;
    out.write_char(' ')?;
    self.styled(out, Role::Gutter, "|")?;
    out.write_char(' ')?;
    // Straight to `out`, not through a `String` first: materialising the row commits the allocation
    // before `out` is ever consulted, so a caller with a bounded or refusing `fmt::Write` — the
    // whole reason this takes one — cannot decline what it never saw.
    //
    // And bounded, because streaming only moves the cost to a writer that accepts: a row is the
    // line's length times the tab width, both caller-owned, so the ceiling is on the product. The
    // `…` is inside the styled run deliberately — it stands where source would have stood, occupies
    // the one cell `underline` reserved for it, and is what stops the cut being silent.
    //
    // Written ONCE for the line rather than once per label, and placed against the stop the one
    // walk found. `underline` would answer the same columns — it is `never_empty` over that same
    // call — but asking again would walk the window again.
    self.styled_with(out, Role::SourceText, |shown| {
      cells.write_expanded_upto(shown, row.drawn_end)?;
      if row.elided {
        fmt::Write::write_char(shown, '…')?;
      }
      Ok(())
    })?;
    out.write_char('\n')?;

    for mark in placed {
      self.marker_row(out, gutter, never_empty(mark.columns()), *mark.payload())?;
    }
    Ok(())
  }

  /// One row of markers, and whatever the label attached to it says.
  fn marker_row(
    &self,
    out: &mut impl fmt::Write,
    gutter: u64,
    marks: core::ops::Range<u64>,
    phrase: Phrase<'_>,
  ) -> fmt::Result {
    pad(out, gutter + 1)?;
    self.styled(out, Role::Gutter, "|")?;
    out.write_char(' ')?;
    pad(out, marks.start - 1)?;

    let role = if phrase.primary {
      Role::PrimaryLabel
    } else {
      Role::SecondaryLabel
    };
    let marker = if phrase.primary { '^' } else { '-' };
    self.styled_with(out, role, |shown| {
      for _ in 0..marks.end - marks.start {
        fmt::Write::write_char(shown, marker)?;
      }
      Ok(())
    })?;
    if let Some(text) = phrase.text {
      out.write_char(' ')?;
      self.styled(out, role, text)?;
    }
    out.write_char('\n')
  }

  /// Writes `text` in the style the palette gives `role`, and nothing at all when it asks for
  /// nothing.
  fn styled(&self, out: &mut impl fmt::Write, role: Role, text: &str) -> fmt::Result {
    // Fully qualified for the reason `write_shown` is, below: the numeric census cannot see through
    // an imported `fmt::Write`, and this file keeps the trait out of scope rather than exempt.
    self.styled_with(out, role, |shown| fmt::Write::write_str(shown, text))
  }

  /// The same, for a row that is written rather than held.
  ///
  /// `body` receives the sanitizer, so what it writes is substituted exactly as a `&str` would be,
  /// and it goes through to `out` as it is written. That is what lets a row whose length is decided
  /// by the input be produced without ever existing as one value.
  ///
  /// # The reset is not conditional on the body succeeding
  ///
  /// The opener and the reset have to be balanced, and a closure alone does not balance them. It
  /// stops a caller *forgetting* the reset; it does nothing about the body *not reaching* it, and a
  /// `?` on a failing write is exactly that — which is how this shipped leaving an SGR open whenever
  /// a bounded writer refused mid-row. Two different failure modes, and only the first is a
  /// consequence of the shape.
  ///
  /// So the body's result is held rather than propagated, the reset is attempted either way, and the
  /// body's error is the one returned — it says what actually went wrong, where the reset's would
  /// only say that the writer is still refusing.
  ///
  /// **Panics are deliberately not covered.** Restoring the terminal while unwinding means writing
  /// to the writer from a `Drop`, and a panic there during an unwind aborts the process: it would
  /// trade a caller's recoverable bug for a dead one, using the very writer that just misbehaved,
  /// and buy nothing at all under `panic = "abort"`. The reset is best-effort on the error path,
  /// which is the path a caller can actually reach by design.
  fn styled_with<W: fmt::Write>(
    &self,
    out: &mut W,
    role: Role,
    body: impl FnOnce(&mut Shown<'_, W>) -> fmt::Result,
  ) -> fmt::Result {
    let style = self.narrow(self.palette.style(role));
    if style.is_plain() {
      return body(&mut Shown(out));
    }
    let ansi = to_anstyle(style);
    // Once an opener has been offered a reset is offered too, including when the opener was itself
    // refused. That keeps the rule total — an opener is never the last thing this writes — instead
    // of leaving a case where it depends on how far the writer got. `and_then` is what holds the
    // other half: a refused opener must not run the body, which is the expensive part.
    let opened = write!(out, "{}", ansi.render());
    let written = opened.and_then(|()| body(&mut Shown(out)));
    let reset = write!(out, "{}", ansi.render_reset());
    written.and(reset)
  }

  /// Brings a style down to what the output can carry.
  ///
  /// [`ColorCapability::None`] means **no escape sequences at all**, not "no colour but keep the
  /// bold". A capability of none is a file or a pipe, and a bold escape written into a file is as
  /// wrong as a red one. A caller who wants attributes without colour — the honest fallback for a
  /// two-colour terminal — asks for [`Theme::monochrome`] at a capability that can carry escapes,
  /// which is a different request and gets a different answer.
  fn narrow(&self, style: Style) -> Style {
    match self.capability {
      ColorCapability::None => Style::plain(),
      ColorCapability::Ansi16 => style.to_ansi16(),
      ColorCapability::Ansi256 => style.to_ansi256(),
      ColorCapability::TrueColor => style,
    }
  }
}

/// Writes caller-supplied text with every control character replaced by a visible stand-in.
///
/// The capability gate decides which escapes painty PRODUCES; it said nothing about the ones a
/// caller's own strings contain, so a message, a code, an origin or a label holding
/// `\x1b[38;5;196m` reached the terminal verbatim — at every level, `ColorCapability::None`
/// included. Escape injection through diagnostic text, and it defeated the per-level guarantee from
/// the one direction that guarantee did not control: its input.
///
/// Source excerpts are handled a layer down, by
/// [`LineCells::write_expanded`](super::LineCells::write_expanded), because there the walk that
/// writes a cluster is the walk that counted its cells. The strings here are never measured, so
/// they are substituted at the point they are written.
///
/// A TAB is the sharp edge of that split. Down there a tab is a device unit spent against a stop;
/// up here there is no stop — the frame's columns are painty's, not the caller's — and U+0009 is a
/// C0 control that moves the cursor as surely as ESC sets a colour. So it is shown as `␉` like the
/// rest, and `control_picture` is what says so, rather than an exception at this call site that the
/// next writer of caller text would not know to repeat.
fn write_shown(out: &mut impl fmt::Write, text: impl fmt::Display) -> fmt::Result {
  // Fully qualified rather than `write!` over an imported trait: the numeric census rejects a
  // renamed import outright — `Write as _` is a name it cannot see through — and it offers no
  // allowlist on purpose.
  fmt::Write::write_fmt(&mut Shown(out), format_args!("{text}"))
}

/// A writer that substitutes as it goes.
///
/// An adapter rather than a function over `&str`, so that a message's own [`fmt::Display`] is
/// covered: the text a caller's type writes is as caller-supplied as the text it hands over
/// directly, and a `Display` that emits an escape would otherwise walk straight past this.
struct Shown<'a, W: fmt::Write>(&'a mut W);

impl<W: fmt::Write> fmt::Write for Shown<'_, W> {
  fn write_str(&mut self, text: &str) -> fmt::Result {
    for character in text.chars() {
      // `self.0`, not `self` — the default `write_char` forwards to `write_str`.
      self
        .0
        .write_char(control_picture(character).unwrap_or(character))?;
    }
    Ok(())
  }
}

/// Writes `count` spaces.
///
/// # Why this is not `{:width$}`
///
/// The formatter's width is a `usize`, and a display column is a `u64` — rule 1 of
/// [the numeric widths](crate#numeric-widths), because it is painty's own rendered geometry and not
/// anything about the machine. Passing one to the other needs an `as usize`, which is exact on a
/// 64-bit target and **narrows on a 32-bit one**: a line of tabs a few tens of MiB long reaches a
/// column past `u32::MAX`, the cast wraps it to a small number, and the marker is drawn near the
/// gutter under nothing at all. The tab bound does not save this. Clamping the tab width bounds the
/// multiplier; the line length is the caller's and still owns the product.
///
/// So the count stays `u64` and is spent in chunks a `usize` certainly holds. Streamed rather than
/// refused, because totality is the rule here — a diagnostic that declines to draw is a diagnostic
/// lost — and chunked rather than one space at a time so an ordinary gutter costs one `write_str`.
/// The writer is consulted once per chunk, so a bounded one still stops early.
pub(super) fn pad(out: &mut impl fmt::Write, count: u64) -> fmt::Result {
  const SPACES: &str = "                                                                ";
  let mut left = count;
  while left >= SPACES.len() as u64 {
    out.write_str(SPACES)?;
    left -= SPACES.len() as u64;
  }
  // Below the chunk length now, so this is the one narrowing in the file that cannot lose anything.
  out.write_str(&SPACES[..left as usize])
}

/// A marker range that a reader can see.
///
/// A zero-width span is a caret, and a caret of no cells is not a caret. Shared by
/// [`Terminal::underline`] and the row that draws it so the two cannot disagree about it.
fn never_empty(columns: core::ops::Range<u64>) -> core::ops::Range<u64> {
  if columns.end > columns.start {
    columns
  } else {
    columns.start..columns.start + 1
  }
}

/// How many decimal digits a line number occupies.
fn digits(mut number: u64) -> u64 {
  let mut count = 1;
  while number >= 10 {
    number /= 10;
    count += 1;
  }
  count
}

fn to_anstyle(style: Style) -> anstyle::Style {
  let mut effects = anstyle::Effects::new();
  if style.bold() {
    effects |= anstyle::Effects::BOLD;
  }
  if style.italic() {
    effects |= anstyle::Effects::ITALIC;
  }
  if style.underline() {
    effects |= anstyle::Effects::UNDERLINE;
  }
  anstyle::Style::new()
    .fg_color(style.foreground().map(to_anstyle_color))
    .bg_color(style.background().map(to_anstyle_color))
    .effects(effects)
}

fn to_ansi_color(colour: crate::Ansi16) -> anstyle::AnsiColor {
  use crate::Ansi16;
  match colour {
    Ansi16::Black => anstyle::AnsiColor::Black,
    Ansi16::Red => anstyle::AnsiColor::Red,
    Ansi16::Green => anstyle::AnsiColor::Green,
    Ansi16::Yellow => anstyle::AnsiColor::Yellow,
    Ansi16::Blue => anstyle::AnsiColor::Blue,
    Ansi16::Magenta => anstyle::AnsiColor::Magenta,
    Ansi16::Cyan => anstyle::AnsiColor::Cyan,
    Ansi16::White => anstyle::AnsiColor::White,
    Ansi16::BrightBlack => anstyle::AnsiColor::BrightBlack,
    Ansi16::BrightRed => anstyle::AnsiColor::BrightRed,
    Ansi16::BrightGreen => anstyle::AnsiColor::BrightGreen,
    Ansi16::BrightYellow => anstyle::AnsiColor::BrightYellow,
    Ansi16::BrightBlue => anstyle::AnsiColor::BrightBlue,
    Ansi16::BrightMagenta => anstyle::AnsiColor::BrightMagenta,
    Ansi16::BrightCyan => anstyle::AnsiColor::BrightCyan,
    Ansi16::BrightWhite => anstyle::AnsiColor::BrightWhite,
  }
}

fn to_anstyle_color(colour: Color) -> anstyle::Color {
  match colour {
    // The SIXTEEN, not their indices in the 256 palette. Emitting `Ansi256(1)` where `Ansi(Red)` is
    // meant produces an escape a sixteen-colour terminal was never promised — the same defect as a
    // capability of none emitting a bold sequence, one level along: output exceeding the capability
    // it was narrowed to. `tests/terminal_appearance.rs` now enumerates, per level, which escape
    // families may appear.
    Color::Ansi16(sixteen) => anstyle::Color::Ansi(to_ansi_color(sixteen)),
    Color::Ansi256(index) => anstyle::Color::Ansi256(anstyle::Ansi256Color(index)),
    Color::Rgb(red, green, blue) => anstyle::Color::Rgb(anstyle::RgbColor(red, green, blue)),
  }
}
