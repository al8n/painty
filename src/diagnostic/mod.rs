//! What painty is given: a borrowed view of one finished diagnostic.
//!
//! Private, and re-exported flat from the crate root — the public vocabulary is small enough that
//! a second path to each name would buy nothing. Whatever a reader needs to know is on the types,
//! not here, for the same reason.

mod location;
mod path;
mod severity;
mod span;

pub use location::{Label, Location};
pub use path::PathSegment;
pub use severity::Severity;
pub use span::Span;

#[cfg(test)]
mod tests;

use core::fmt;

/// One finished diagnostic, borrowed for as long as it takes to render it.
///
/// # This is a data seam, and that is the whole design decision
///
/// The obvious shape for a renderer's input is a trait — ask the error for its code, its severity,
/// its labels. painty does not take one, and the reason generalises an argument the family already
/// made once. A diagnostic contract must not live in a *parser* crate, because a project using a
/// different parser would then drag a GraphQL library in to draw an underline. That argument does
/// not stop at parsers: a Rust project with its own error type should not have to adopt anybody's
/// parser-combinator library either, and requiring one would reproduce the defect one level up.
///
/// So the input is plain borrowed data painty defines itself, over nothing but [`core`]. Producing
/// one from a foreign contract is a *conversion*, and conversions live behind features: the
/// `tokora` feature's `painty::tokora` is the first, and it is one small module.
///
/// **Not a second `Diagnose`-shaped trait, blanket-implemented for tokora's.** Coherence would
/// allow it and it is the wrong answer: two structurally identical traits maintained in two
/// repositories drift, and the drift is silent, because a trait that has fallen behind its twin
/// keeps compiling. A conversion function does not. The failure direction is the whole argument,
/// and it is recorded here so it is not re-proposed.
///
/// The cost is one borrowed view materialised per diagnostic rather than reading straight through
/// a trait object. Every field is `Copy` and every string borrows, so that is a handful of machine
/// words on the stack: nothing here reaches an allocator.
///
/// # The message is a [`Display`](fmt::Display), not a string
///
/// A diagnostic's prose is the one part of it that varies with the input — the name that was
/// rejected, the type that was expected — and materialising it into a `&str` would mean the caller
/// allocating a message painty might elide anyway. `&dyn Display` is `Copy`, needs no allocator,
/// and lets a renderer write straight into a buffer it already owns. It also means an error type
/// that already implements [`Display`](fmt::Display) — which in Rust is all of them — needs
/// nothing further to be renderable.
///
/// # Building one
///
/// Four things are required, because a diagnostic that cannot answer them is not one:
/// [`code`](Self::code), [`severity`](Self::severity), the message, and a
/// [`primary`](Self::primary) position. The rest are `with_*`.
///
/// ```
/// use painty::{Diagnostic, Label, Location, Severity, Span};
///
/// let message = format_args!("`{}` is defined twice", "width");
/// let labels = [Label::new(Location::new(0, Span::new(8, 13)), "first defined here")];
///
/// let diagnostic = Diagnostic::new(
///   "mylang::resolve::redefined",
///   Severity::Error,
///   &message,
///   Location::new(0, Span::new(40, 45)),
/// )
/// .with_primary_label("redefined here")
/// .with_labels(&labels)
/// .with_help("rename one of the two definitions");
///
/// assert_eq!(diagnostic.code(), "mylang::resolve::redefined");
/// assert_eq!(diagnostic.labels().len(), 1);
/// assert_eq!(diagnostic.help(), Some("rename one of the two definitions"));
/// assert_eq!(diagnostic.message().to_string(), "`width` is defined twice");
/// ```
#[derive(Clone, Copy)]
pub struct Diagnostic<'a> {
  code: &'a str,
  severity: Severity,
  message: &'a dyn fmt::Display,
  primary: Location,
  primary_label: Option<&'a str>,
  labels: &'a [Label<'a>],
  path: &'a [PathSegment<'a>],
  help: Option<&'a str>,
}

impl<'a> Diagnostic<'a> {
  /// A diagnostic with the four things every one of them has.
  ///
  /// `code` is the stable machine identifier for the rule — `mylang::resolve::unknown-name` — and
  /// not the message, which is prose and is expected to improve. painty neither validates its
  /// shape nor renders it specially; it carries it so a consumer of the resolved model can key off
  /// something that does not move.
  #[inline]
  #[must_use]
  pub const fn new(
    code: &'a str,
    severity: Severity,
    message: &'a dyn fmt::Display,
    primary: Location,
  ) -> Self {
    Self {
      code,
      severity,
      message,
      primary,
      primary_label: None,
      labels: &[],
      path: &[],
      help: None,
    }
  }

  /// Attaches a phrase to the primary position, where a short one says more than an underline
  /// alone.
  #[inline]
  #[must_use]
  pub const fn with_primary_label(mut self, text: &'a str) -> Self {
    self.primary_label = Some(text);
    self
  }

  /// Attaches the secondary labels.
  ///
  /// A slice rather than anything cleverer, because the resolved model is meant to cross an FFI
  /// boundary eventually and a slice is the shape that can. [`Label::order`] sorts one in place if
  /// the caller wants resolution to run in a single forward pass.
  #[inline]
  #[must_use]
  pub const fn with_labels(mut self, labels: &'a [Label<'a>]) -> Self {
    self.labels = labels;
    self
  }

  /// Attaches the result path, root segment first.
  #[inline]
  #[must_use]
  pub const fn with_path(mut self, path: &'a [PathSegment<'a>]) -> Self {
    self.path = path;
    self
  }

  /// Attaches the line saying what the reader can do about it.
  #[inline]
  #[must_use]
  pub const fn with_help(mut self, help: &'a str) -> Self {
    self.help = Some(help);
    self
  }

  /// Returns the stable identifier for this diagnostic's rule.
  #[inline]
  pub const fn code(&self) -> &'a str {
    self.code
  }

  /// Returns how much the diagnostic asks of its reader.
  #[inline]
  pub const fn severity(&self) -> Severity {
    self.severity
  }

  /// Returns the prose, renderable on demand and only if somebody asks.
  #[inline]
  pub const fn message(&self) -> &'a dyn fmt::Display {
    self.message
  }

  /// Returns the position the diagnostic is about.
  #[inline]
  pub const fn primary(&self) -> Location {
    self.primary
  }

  /// Returns the phrase for the primary position, if the rule gave one.
  #[inline]
  pub const fn primary_label(&self) -> Option<&'a str> {
    self.primary_label
  }

  /// Returns the secondary labels.
  #[inline]
  pub const fn labels(&self) -> &'a [Label<'a>] {
    self.labels
  }

  /// Returns the result path, root segment first, or an empty slice for a diagnostic about a
  /// document rather than a result.
  #[inline]
  pub const fn path(&self) -> &'a [PathSegment<'a>] {
    self.path
  }

  /// Returns what the reader can do about it, where the rule had something actionable to say.
  #[inline]
  pub const fn help(&self) -> Option<&'a str> {
    self.help
  }
}

impl fmt::Debug for Diagnostic<'_> {
  /// Renders the message through its [`Display`](fmt::Display), since a `&dyn Display` has no
  /// [`Debug`](fmt::Debug) of its own and the prose is the informative part.
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Diagnostic")
      .field("code", &self.code)
      .field("severity", &self.severity)
      .field("message", &format_args!("{}", self.message))
      .field("primary", &self.primary)
      .field("primary_label", &self.primary_label)
      .field("labels", &self.labels)
      .field("path", &self.path)
      .field("help", &self.help)
      .finish()
  }
}
