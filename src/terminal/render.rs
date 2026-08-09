use core::fmt;

use super::{ColorCapability, LineCells};
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
  pub fn underline(&self, drawn: RegionLine<'_>) -> core::ops::Range<u64> {
    let cells = LineCells::new(drawn.line(), self.tab_width);
    let marks = cells.columns_for(drawn.covered());
    if marks.end > marks.start {
      marks
    } else {
      marks.start..marks.start + 1
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
    write!(out, "{}", diagnostic.message())?;
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
        excerpts.push((drawn, text, primary, input.origin));
      }
    }

    let gutter = excerpts
      .iter()
      .map(|(drawn, _, _, _)| digits(drawn.line().number()))
      .max()
      .unwrap_or(1);

    if let Some((drawn, _, _, origin)) = excerpts.first() {
      let column = LineCells::new(drawn.line(), self.tab_width).column_at(drawn.covered().start());
      write!(out, "{:width$}--> ", "", width = gutter as usize)?;
      if let Some(origin) = origin {
        write!(out, "{origin}:")?;
      }
      writeln!(out, "{}:{column}", drawn.line().number())?;
    }

    for (index, (drawn, text, primary, _)) in excerpts.iter().enumerate() {
      if index == 0 {
        self.bar(out, gutter)?;
      }
      self.excerpt(out, gutter, *drawn, *text, *primary)?;
      self.bar(out, gutter)?;
    }

    if let Some(help) = diagnostic.help() {
      write!(out, "{:width$}= ", "", width = gutter as usize + 1)?;
      self.styled(out, Role::Help, "help")?;
      out.write_str(": ")?;
      self.styled(out, Role::Help, help)?;
      out.write_char('\n')?;
    }
    Ok(())
  }

  /// One `  |` separator row.
  fn bar(&self, out: &mut impl fmt::Write, gutter: u64) -> fmt::Result {
    write!(out, "{:width$}", "", width = gutter as usize + 1)?;
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
    write!(
      out,
      "{:width$}",
      "",
      width = (gutter - digits(number)) as usize
    )?;
    self.styled(out, Role::LineNumber, &number.to_string())?;
    out.write_char(' ')?;
    self.styled(out, Role::Gutter, "|")?;
    out.write_char(' ')?;
    let mut expanded = String::new();
    cells.write_expanded(&mut expanded)?;
    self.styled(out, Role::SourceText, &expanded)?;
    out.write_char('\n')?;

    let marks = self.underline(drawn);
    write!(out, "{:width$}", "", width = gutter as usize + 1)?;
    self.styled(out, Role::Gutter, "|")?;
    out.write_char(' ')?;
    write!(out, "{:width$}", "", width = (marks.start - 1) as usize)?;

    let role = if primary {
      Role::PrimaryLabel
    } else {
      Role::SecondaryLabel
    };
    let marker = if primary { '^' } else { '-' };
    let row: String = core::iter::repeat_n(marker, (marks.end - marks.start) as usize).collect();
    self.styled(out, role, &row)?;
    if let Some(text) = text {
      out.write_char(' ')?;
      self.styled(out, role, text)?;
    }
    out.write_char('\n')
  }

  /// Writes `text` in the style the palette gives `role`, and nothing at all when it asks for
  /// nothing.
  fn styled(&self, out: &mut impl fmt::Write, role: Role, text: &str) -> fmt::Result {
    let style = self.narrow(self.palette.style(role));
    if style.is_plain() {
      return out.write_str(text);
    }
    let ansi = to_anstyle(style);
    write!(out, "{}{text}{}", ansi.render(), ansi.render_reset())
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
