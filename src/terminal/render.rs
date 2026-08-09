use core::fmt;

use super::{ColorCapability, LineCells, width::control_picture};
use crate::{Color, Diagnostic, Palette, RegionLine, Role, Source, Style, Theme};

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

/// Renders a diagnostic and its source to a fixed-width terminal.
///
/// # What this draws, and what it does not yet
///
/// One excerpt per position: the line it falls on, and a row of markers under it. `^` for the
/// primary position, `-` for a secondary one, which is the convention a reader already knows.
///
/// A span covering more than one line is drawn on the **first** line it touches. Brackets and the
/// connectors that join a span's ends across lines are a row-assignment problem, and they are the
/// expensive part of a terminal renderer rather than an afterthought; they arrive next. Two labels
/// on the same line likewise get one excerpt each rather than being merged onto shared rows. What
/// is here is total and correct about what it draws — it is not yet everything a reader will
/// eventually want drawn.
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
  /// Clipped to [`max_rendered_width`](Self::max_rendered_width) plus the cell holding the elision
  /// mark, and clipped HERE rather than where the row is drawn so that the two cannot disagree: the
  /// marker row is drawn from this range, so a span out past the ceiling puts the caret under the
  /// `…` — which is where a reader should look for it — rather than under a cell the source row
  /// never drew.
  pub fn underline(&self, drawn: RegionLine<'_>) -> core::ops::Range<u64> {
    let cells = LineCells::new(drawn.line(), self.tab_width);
    let marks = cells.columns_for(drawn.covered());
    // Clipped to where the SOURCE ROW actually stopped, which is not the ceiling: a unit is drawn
    // whole, so the row halts before one that would straddle it and a tab can be 256 cells. Asking
    // `visible_within` — the same function `excerpt` draws from — is what keeps the elision mark and
    // the caret in the same column.
    let (visible, elided) = cells.visible_within(Self::max_rendered_width());
    let (start, end) = if elided {
      let mark = visible + 1;
      (marks.start.min(mark), marks.end.min(mark + 1))
    } else {
      (marks.start, marks.end)
    };
    if end > start {
      start..end
    } else {
      start..start + 1
    }
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
  #[inline]
  #[must_use]
  pub const fn max_rendered_width() -> u64 {
    4096
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

    // Every excerpt this will draw, so the gutter can be sized before any of them is written.
    let mut excerpts = Vec::new();
    let mut positions = Vec::new();
    positions.push((diagnostic.primary(), diagnostic.primary_label(), true));
    for label in diagnostic.labels() {
      positions.push((label.location(), Some(label.text()), false));
    }
    for (location, text, primary) in positions {
      let Some(input) = inputs.get(location.source() as usize) else {
        continue;
      };
      if let Some(span) = location.span()
        && let Some(drawn) = input.source.resolve(span).lines().next()
      {
        excerpts.push((drawn, text, primary, location.source(), input.origin));
      }
    }

    let gutter = excerpts
      .iter()
      .map(|(drawn, _, _, _, _)| digits(drawn.line().number()))
      .max()
      .unwrap_or(1);

    // A header per input, and the key is the input INDEX rather than the origin string. Resolving
    // each span against its own input was half the multi-input fix; the other half is saying which
    // input a row came from, because a secondary label in another file was otherwise drawn under
    // the first file's header — the reader told, confidently and silently, that text came from a
    // file it did not come from. Two inputs with no origin, or with the same one, are still two
    // inputs, and only the index distinguishes them.
    let mut shown_input = None;
    for (drawn, text, primary, source, origin) in &excerpts {
      if shown_input != Some(*source) {
        let column =
          LineCells::new(drawn.line(), self.tab_width).column_at(drawn.covered().start());
        pad(out, gutter)?;
        out.write_str("--> ")?;
        if let Some(origin) = origin {
          write_shown(out, origin)?;
          out.write_char(':')?;
        }
        writeln!(out, "{}:{column}", drawn.line().number())?;
        self.bar(out, gutter)?;
        shown_input = Some(*source);
      }
      self.excerpt(out, gutter, *drawn, *text, *primary)?;
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

  /// One source line and the marker row under it.
  fn excerpt(
    &self,
    out: &mut impl fmt::Write,
    gutter: u64,
    drawn: RegionLine<'_>,
    text: Option<&str>,
    primary: bool,
  ) -> fmt::Result {
    let cells = LineCells::new(drawn.line(), self.tab_width);

    let number = drawn.line().number();
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
    let (visible, elided) = cells.visible_within(Self::max_rendered_width());
    self.styled_with(out, Role::SourceText, |shown| {
      cells.write_expanded_within(shown, visible)?;
      if elided {
        fmt::Write::write_char(shown, '…')?;
      }
      Ok(())
    })?;
    out.write_char('\n')?;

    let marks = self.underline(drawn);
    pad(out, gutter + 1)?;
    self.styled(out, Role::Gutter, "|")?;
    out.write_char(' ')?;
    pad(out, marks.start - 1)?;

    let role = if primary {
      Role::PrimaryLabel
    } else {
      Role::SecondaryLabel
    };
    let marker = if primary { '^' } else { '-' };
    self.styled_with(out, role, |shown| {
      for _ in 0..marks.end - marks.start {
        fmt::Write::write_char(shown, marker)?;
      }
      Ok(())
    })?;
    if let Some(text) = text {
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
