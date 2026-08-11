//! What every style writes through: the palette, the capability, and the sanitizer.
//!
//! None of this is presentation. A style chooses glyphs and rows; it never decides whether an
//! escape may be emitted, what a role's colour is, or whether a caller's string reaches the
//! terminal unread. Those are one answer for the whole renderer, and a style that could give a
//! second one would be a style that can defeat [`ColorCapability::None`] — which is the guarantee
//! this crate exists to make.

use core::fmt;

use super::ColorCapability;
use crate::{Color, Palette, Role, Style};

/// The surface a render is drawn on.
///
/// Holds the writer as `&mut dyn` rather than by generic parameter, so that
/// [`Presentation`](super::present::Presentation) has no type parameter in its methods and stays
/// object-safe. The cost is one indirect call per write and the buy is that a style can be
/// selected at run time, from a flag, without the renderer's type changing.
pub(super) struct Painter<'a> {
  out: &'a mut dyn fmt::Write,
  palette: &'a dyn Palette,
  capability: ColorCapability,
}

impl<'a> Painter<'a> {
  /// A surface over `out`, styled by `palette` down to what `capability` can carry.
  pub(super) fn new(
    out: &'a mut dyn fmt::Write,
    palette: &'a dyn Palette,
    capability: ColorCapability,
  ) -> Self {
    Self {
      out,
      palette,
      capability,
    }
  }

  /// painty's own frame characters, written as they are.
  ///
  /// Not sanitized, and that is safe for exactly one reason: what goes through here is a literal
  /// in this crate. Anything a caller supplied goes through [`shown`](Self::shown) or
  /// [`styled`](Self::styled).
  pub(super) fn frame(&mut self, text: &str) -> fmt::Result {
    self.out.write_str(text)
  }

  /// One of painty's own frame characters.
  pub(super) fn frame_char(&mut self, character: char) -> fmt::Result {
    self.out.write_char(character)
  }

  /// A number painty computed — a line, a column — which no caller can put an escape into.
  pub(super) fn frame_fmt(&mut self, args: fmt::Arguments<'_>) -> fmt::Result {
    self.out.write_fmt(args)
  }

  /// A line break.
  pub(super) fn newline(&mut self) -> fmt::Result {
    self.out.write_char('\n')
  }

  /// Writes caller-supplied text with every control character replaced by a visible stand-in.
  ///
  /// The capability gate decides which escapes painty PRODUCES; it said nothing about the ones a
  /// caller's own strings contain, so a message, a code, an origin or a label holding
  /// `\x1b[38;5;196m` reached the terminal verbatim — at every level,
  /// [`ColorCapability::None`] included. Escape injection through diagnostic text, and it defeated
  /// the per-level guarantee from the one direction that guarantee did not control: its input.
  ///
  /// Source excerpts are handled a layer down, by
  /// [`LineCells::write_expanded`](super::LineCells::write_expanded), because there the walk that
  /// writes a cluster is the walk that counted its cells. The strings here are never measured, so
  /// they are substituted at the point they are written.
  ///
  /// A TAB is the sharp edge of that split. Down there a tab is a device unit spent against a
  /// stop; up here there is no stop — the frame's columns are painty's, not the caller's — and
  /// U+0009 is a C0 control that moves the cursor as surely as ESC sets a colour. So it is shown
  /// as `␉` like the rest, and [`control_picture`](super::width::control_picture) is what says so,
  /// rather than an exception at this call site that the next writer of caller text would not know
  /// to repeat.
  pub(super) fn shown(&mut self, text: impl fmt::Display) -> fmt::Result {
    // Fully qualified rather than `write!` over an imported trait: the numeric census rejects a
    // renamed import outright — `Write as _` is a name it cannot see through — and it offers no
    // allowlist on purpose.
    fmt::Write::write_fmt(&mut Shown(self.out), format_args!("{text}"))
  }

  /// Writes `text` in the style the palette gives `role`, and nothing at all when it asks for
  /// nothing.
  pub(super) fn styled(&mut self, role: Role, text: &str) -> fmt::Result {
    self.styled_with(role, |shown| fmt::Write::write_str(shown, text))
  }

  /// The same, for a row that is written rather than held.
  ///
  /// `body` receives the sanitizer, so what it writes is substituted exactly as a `&str` would be,
  /// and it goes through to the writer as it is written. That is what lets a row whose length is
  /// decided by the input be produced without ever existing as one value.
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
  pub(super) fn styled_with(
    &mut self,
    role: Role,
    body: impl FnOnce(&mut Shown<'_>) -> fmt::Result,
  ) -> fmt::Result {
    let style = self.narrow(self.palette.style(role));
    if style.is_plain() {
      return body(&mut Shown(self.out));
    }
    let ansi = to_anstyle(style);
    // Once an opener has been offered a reset is offered too, including when the opener was itself
    // refused. That keeps the rule total — an opener is never the last thing this writes — instead
    // of leaving a case where it depends on how far the writer got. `and_then` is what holds the
    // other half: a refused opener must not run the body, which is the expensive part.
    let opened = self.out.write_fmt(format_args!("{}", ansi.render()));
    let written = opened.and_then(|()| body(&mut Shown(self.out)));
    let reset = self.out.write_fmt(format_args!("{}", ansi.render_reset()));
    written.and(reset)
  }

  /// Writes `count` spaces — see [`pad`].
  pub(super) fn pad(&mut self, count: u64) -> fmt::Result {
    pad(self.out, count)
  }

  /// The connector columns of one row, in whatever glyphs the style put in them.
  ///
  /// Shared, and it is worth saying why when everything around it diverges: the SLOTS are the
  /// plan's — one per multi-line span, assigned so a later span runs to the right of an earlier
  /// one — and only what stands in them is a style's. A style that could write its own margin
  /// could put a bracket in another span's column.
  pub(super) fn margin(&mut self, columns: &[Option<(char, Role)>]) -> fmt::Result {
    for column in columns {
      match column {
        Some((glyph, role)) => self.styled(*role, glyph.encode_utf8(&mut [0; 4]))?,
        None => self.frame_char(' ')?,
      }
    }
    Ok(())
  }

  /// Brings a style down to what the output can carry.
  ///
  /// [`ColorCapability::None`] means **no escape sequences at all**, not "no colour but keep the
  /// bold". A capability of none is a file or a pipe, and a bold escape written into a file is as
  /// wrong as a red one. A caller who wants attributes without colour — the honest fallback for a
  /// two-colour terminal — asks for [`Theme::monochrome`](crate::Theme::monochrome) at a
  /// capability that can carry escapes, which is a different request and gets a different answer.
  fn narrow(&self, style: Style) -> Style {
    match self.capability {
      ColorCapability::None => Style::plain(),
      ColorCapability::Ansi16 => style.to_ansi16(),
      ColorCapability::Ansi256 => style.to_ansi256(),
      ColorCapability::TrueColor => style,
    }
  }
}

/// A writer that substitutes as it goes.
///
/// An adapter rather than a function over `&str`, so that a message's own [`fmt::Display`] is
/// covered: the text a caller's type writes is as caller-supplied as the text it hands over
/// directly, and a `Display` that emits an escape would otherwise walk straight past this.
pub(super) struct Shown<'a>(&'a mut dyn fmt::Write);

impl fmt::Write for Shown<'_> {
  fn write_str(&mut self, text: &str) -> fmt::Result {
    for character in text.chars() {
      // `self.0`, not `self` — the default `write_char` forwards to `write_str`.
      self
        .0
        .write_char(super::width::control_picture(character).unwrap_or(character))?;
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
pub(super) fn pad(out: &mut (impl fmt::Write + ?Sized), count: u64) -> fmt::Result {
  const SPACES: &str = "                                                                ";
  let mut left = count;
  while left >= SPACES.len() as u64 {
    out.write_str(SPACES)?;
    left -= SPACES.len() as u64;
  }
  // Below the chunk length now, so this is the one narrowing in the crate that cannot lose
  // anything, and it is the one site `cast_possible_truncation` is silenced at.
  //
  // Silenced on this statement rather than on `pad`, so that it covers this cast and not whatever
  // a later edit adds to the function. `expect` rather than `allow`: if the cast goes away — the
  // chunk loop rewritten, the remainder spent some other way — the attribute becomes a lie, and
  // this is the spelling that says so instead of sitting there.
  #[expect(
    clippy::cast_possible_truncation,
    reason = "the loop above runs until `left < SPACES.len()`, which is 64, so the value converts \
              exactly on a pointer of any width"
  )]
  let remainder = left as usize;
  out.write_str(&SPACES[..remainder])
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
