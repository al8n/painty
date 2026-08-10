//! The medium-independent half of appearance: what a piece of text *is*, and what a theme wants
//! done with it.
//!
//! Private, and re-exported flat from the crate root.

mod color;

pub use color::{Ansi16, Color};

use crate::Severity;

#[cfg(test)]
mod tests;

/// What a piece of rendered text **is**, rather than how it should look.
///
/// # Why a theme maps roles and not escape codes
///
/// A theme cannot be a table of ANSI sequences, because HTML cannot consume one and neither can a
/// native widget tree. So the vocabulary is semantic and lives here, in the medium-independent
/// core: the terminal renderer maps a role to a [`Style`] and then to SGR, an HTML renderer maps
/// the same role to a CSS class, and an FFI export hands the role across and lets the far side
/// decide. One theme drives every output, which is what makes "not hardcoded" structural rather
/// than a convention.
///
/// This is why the type is **not** behind the `terminal` feature despite arriving with it: a
/// colour vocabulary that existed only for the terminal would have to be duplicated the moment a
/// second renderer wanted one.
///
/// `#[non_exhaustive]`, so a role can be added without breaking a [`Palette`] outside this crate —
/// an implementor there carries a fallback arm. It binds other crates only, so painty's own
/// matches stay exhaustive and a new role fails to compile until [`Theme`] has decided what to do
/// with it, which is the direction worth having.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Role {
  /// The header's severity word, and the underline that agrees with it.
  Severity(Severity),
  /// The phrase attached to the primary position.
  PrimaryLabel,
  /// The phrase attached to a secondary position.
  SecondaryLabel,
  /// The bars and brackets down the left of an excerpt.
  Gutter,
  /// A line number in the gutter.
  LineNumber,
  /// The source text itself, unstyled by default — the slot exists so a caller can dim it.
  SourceText,
  /// The line saying what the reader can do about it.
  Help,
  /// The stable machine identifier, `mylang::schema::duplicate-field`.
  Code,
}

/// What a theme wants done with a [`Role`], in terms every medium can interpret.
///
/// Attributes rather than escapes, so an HTML renderer can turn `bold` into a class and a native
/// one into a font weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Style {
  foreground: Option<Color>,
  background: Option<Color>,
  bold: bool,
  italic: bool,
  underline: bool,
}

impl Style {
  /// A style that asks for nothing.
  #[inline]
  #[must_use]
  pub const fn plain() -> Self {
    Self {
      foreground: None,
      background: None,
      bold: false,
      italic: false,
      underline: false,
    }
  }

  /// Sets the foreground colour.
  #[inline]
  #[must_use]
  pub const fn with_foreground(mut self, colour: Color) -> Self {
    self.foreground = Some(colour);
    self
  }

  /// Sets the background colour.
  #[inline]
  #[must_use]
  pub const fn with_background(mut self, colour: Color) -> Self {
    self.background = Some(colour);
    self
  }

  /// Asks for bold.
  #[inline]
  #[must_use]
  pub const fn with_bold(mut self) -> Self {
    self.bold = true;
    self
  }

  /// Asks for italic.
  #[inline]
  #[must_use]
  pub const fn with_italic(mut self) -> Self {
    self.italic = true;
    self
  }

  /// Asks for an underline.
  #[inline]
  #[must_use]
  pub const fn with_underline(mut self) -> Self {
    self.underline = true;
    self
  }

  /// Returns the foreground colour, if the theme asked for one.
  #[inline]
  pub const fn foreground(&self) -> Option<Color> {
    self.foreground
  }

  /// Returns the background colour, if the theme asked for one.
  #[inline]
  pub const fn background(&self) -> Option<Color> {
    self.background
  }

  /// Returns whether the theme asked for bold.
  #[inline]
  pub const fn bold(&self) -> bool {
    self.bold
  }

  /// Returns whether the theme asked for italic.
  #[inline]
  pub const fn italic(&self) -> bool {
    self.italic
  }

  /// Returns whether the theme asked for an underline.
  #[inline]
  pub const fn underline(&self) -> bool {
    self.underline
  }

  /// Returns whether the style asks for nothing at all.
  ///
  /// A renderer uses this to skip emitting an escape it would immediately have to undo.
  #[inline]
  pub const fn is_plain(&self) -> bool {
    self.foreground.is_none()
      && self.background.is_none()
      && !self.bold
      && !self.italic
      && !self.underline
  }

  /// Narrows every colour in the style to the sixteen.
  #[inline]
  #[must_use]
  pub const fn to_ansi16(mut self) -> Self {
    if let Some(colour) = self.foreground {
      self.foreground = Some(colour.to_ansi16());
    }
    if let Some(colour) = self.background {
      self.background = Some(colour.to_ansi16());
    }
    self
  }

  /// Narrows every colour in the style to the 256-colour palette.
  #[inline]
  #[must_use]
  pub const fn to_ansi256(mut self) -> Self {
    if let Some(colour) = self.foreground {
      self.foreground = Some(colour.to_ansi256());
    }
    if let Some(colour) = self.background {
      self.background = Some(colour.to_ansi256());
    }
    self
  }

  /// Drops every colour, keeping the attributes.
  ///
  /// What a two-colour terminal gets: bold and underline still carry a difference a reader can see.
  ///
  /// A palette's tool, not a capability's. `ColorChoice::Never` does **not** select this — it
  /// resolves to `ColorCapability::None`, which emits no escape at all, because that choice is
  /// usually a file or a pipe and a bold escape in a file is as wrong as a red one. A caller who
  /// wants attributes without colour builds a palette from this — [`Theme::monochrome`] is the
  /// built-in one — and renders at a capability that can carry escapes.
  #[inline]
  #[must_use]
  pub const fn without_color(mut self) -> Self {
    self.foreground = None;
    self.background = None;
    self
  }
}

/// Asked for a role's style; how it decides is its business.
///
/// # This is the open half, and it is free
///
/// A consumer can compute styles however it likes — read a configuration file, mirror the user's
/// terminal theme, vary by role at run time — without painty dictating a representation. [`Theme`]
/// is then simply the built-in implementor rather than the only possible one.
///
/// The third axis is already open without any trait at all: a renderer that wants a colour model
/// painty does not have is a *renderer*, not a colour. An OKLCH-based HTML renderer takes the
/// [`Role`], ignores [`Style`] entirely, and consults its own palette. Nothing here has to change
/// for that to work, which is the test of whether the seam is in the right place.
pub trait Palette {
  /// Returns how this palette wants `role` rendered.
  fn style(&self, role: Role) -> Style;
}

/// painty's built-in palette.
///
/// # Constructed by method rather than by struct literal
///
/// The design asked for a struct literal, and that turns out to be incompatible with the other
/// thing it asked for. [`Role`] is `#[non_exhaustive]` precisely so a role can be added later
/// without breaking a [`Palette`] — but public fields here would make adding one a breaking change
/// to `Theme`, which takes back exactly what `Role`'s openness was for. The `with_*` setters are
/// `const` and chain, so a caller writes one expression either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Theme {
  error: Style,
  warning: Style,
  advice: Style,
  primary_label: Style,
  secondary_label: Style,
  gutter: Style,
  line_number: Style,
  source_text: Style,
  help: Style,
  code: Style,
}

impl Theme {
  /// The colours painty renders with unless told otherwise.
  ///
  /// Foreground only. A palette legible on a dark terminal can be unreadable on a light one and
  /// there is no reliable way to detect which, so the default sets no background and the question
  /// does not arise.
  #[must_use]
  pub const fn new() -> Self {
    Self {
      error: Style::plain()
        .with_foreground(Color::Ansi16(Ansi16::BrightRed))
        .with_bold(),
      warning: Style::plain()
        .with_foreground(Color::Ansi16(Ansi16::BrightYellow))
        .with_bold(),
      advice: Style::plain()
        .with_foreground(Color::Ansi16(Ansi16::BrightCyan))
        .with_bold(),
      primary_label: Style::plain().with_foreground(Color::Ansi16(Ansi16::BrightRed)),
      secondary_label: Style::plain().with_foreground(Color::Ansi16(Ansi16::BrightBlue)),
      gutter: Style::plain().with_foreground(Color::Ansi16(Ansi16::BrightBlack)),
      line_number: Style::plain().with_foreground(Color::Ansi16(Ansi16::BrightBlack)),
      source_text: Style::plain(),
      help: Style::plain().with_foreground(Color::Ansi16(Ansi16::BrightGreen)),
      code: Style::plain().with_foreground(Color::Ansi16(Ansi16::BrightBlack)),
    }
  }

  /// Heavier weights and the brightest colours, for a terminal or a reader that needs them.
  #[must_use]
  pub const fn high_contrast() -> Self {
    let base = Self::new();
    Self {
      primary_label: base.primary_label.with_bold(),
      secondary_label: base.secondary_label.with_bold(),
      gutter: Style::plain()
        .with_foreground(Color::Ansi16(Ansi16::BrightWhite))
        .with_bold(),
      line_number: Style::plain()
        .with_foreground(Color::Ansi16(Ansi16::BrightWhite))
        .with_bold(),
      source_text: Style::plain().with_bold(),
      help: base.help.with_bold(),
      code: Style::plain()
        .with_foreground(Color::Ansi16(Ansi16::BrightWhite))
        .with_bold(),
      ..base
    }
  }

  /// Attributes only, no colour at all.
  ///
  /// The honest fallback for a two-colour terminal, and the way to ask for styling without colour:
  /// bold and underline still separate a header from an underline from a help line.
  ///
  /// It has to be paired with a capability that can carry escapes to have any effect.
  /// `ColorCapability::None` emits none at all, so this theme and any other render the same there —
  /// which is the point of that rung rather than a limitation of this one.
  #[must_use]
  pub const fn monochrome() -> Self {
    Self {
      error: Style::plain().with_bold(),
      warning: Style::plain().with_bold(),
      advice: Style::plain().with_bold(),
      primary_label: Style::plain(),
      secondary_label: Style::plain(),
      gutter: Style::plain(),
      line_number: Style::plain(),
      source_text: Style::plain(),
      help: Style::plain(),
      code: Style::plain(),
    }
  }

  /// Replaces the style for one role.
  #[must_use]
  pub const fn with(mut self, role: Role, style: Style) -> Self {
    match role {
      Role::Severity(Severity::Error) => self.error = style,
      Role::Severity(Severity::Warning) => self.warning = style,
      Role::Severity(Severity::Advice) => self.advice = style,
      Role::PrimaryLabel => self.primary_label = style,
      Role::SecondaryLabel => self.secondary_label = style,
      Role::Gutter => self.gutter = style,
      Role::LineNumber => self.line_number = style,
      Role::SourceText => self.source_text = style,
      Role::Help => self.help = style,
      Role::Code => self.code = style,
    }
    self
  }
}

impl Default for Theme {
  #[inline]
  fn default() -> Self {
    Self::new()
  }
}

impl Palette for Theme {
  fn style(&self, role: Role) -> Style {
    match role {
      Role::Severity(Severity::Error) => self.error,
      Role::Severity(Severity::Warning) => self.warning,
      Role::Severity(Severity::Advice) => self.advice,
      Role::PrimaryLabel => self.primary_label,
      Role::SecondaryLabel => self.secondary_label,
      Role::Gutter => self.gutter,
      Role::LineNumber => self.line_number,
      Role::SourceText => self.source_text,
      Role::Help => self.help,
      Role::Code => self.code,
    }
  }
}

impl<P> Palette for &P
where
  P: Palette + ?Sized,
{
  #[inline]
  fn style(&self, role: Role) -> Style {
    (**self).style(role)
  }
}
