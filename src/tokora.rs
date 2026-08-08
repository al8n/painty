//! Building painty's view from `tokora`'s diagnostic contract.
//!
//! # An adapter, not a second contract
//!
//! painty's core knows nothing about tokora, and this module is the whole of the relationship: a
//! handful of [`From`] implementations and one function that reads a `&dyn Diagnose` into a
//! [`Diagnostic`]. It is optional because a Rust project with its own error type should be able to
//! draw an underline without adopting a parser-combinator library — the same argument that kept
//! the diagnostic contract out of any one front end, applied one level up.
//!
//! The alternative — painty declaring a `Diagnose`-shaped trait of its own and blanket-implementing
//! it for tokora's — is rejected on purpose. Coherence would allow it, and two structurally
//! identical traits maintained in two repositories drift *silently*, because a trait that has
//! fallen behind its twin still compiles. The conversions below do not: when tokora's contract
//! gains a method or changes a type, this module stops building, which is the failure direction
//! worth paying for.
//!
//! # It allocates nothing, and that is why the buffers are yours
//!
//! [`Diagnose`] answers its labels and its result path through indexed accessors, and painty's
//! view holds slices — because a slice is the shape that can eventually cross an FFI boundary, and
//! a trait object is not. Somewhere between the two, the labels have to land in memory. painty has
//! no allocator, so [`adapt`] writes them into storage the caller supplies, usually an array on
//! the stack, and says how many did not fit rather than dropping them quietly.
//!
//! ```
//! use painty::{Label, PathSegment, Source, Span, tokora::adapt};
//! # use core::fmt;
//! # use tokora::{SimpleSpan, diagnostic::{Code, Diagnose, Location, Severity}};
//! # struct Redefined;
//! # impl fmt::Display for Redefined {
//! #   fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("`width` is defined twice") }
//! # }
//! # impl Diagnose for Redefined {
//! #   fn code(&self) -> Code { Code::new("mylang::resolve::redefined") }
//! #   fn severity(&self) -> Severity { Severity::Error }
//! #   fn primary(&self) -> Location { Location::new(0, SimpleSpan::new(29, 34)) }
//! #   fn primary_label(&self) -> Option<&'static str> { Some("redefined here") }
//! #   fn labels(&self) -> usize { 1 }
//! #   fn label(&self, index: usize) -> Option<tokora::diagnostic::Label> {
//! #     (index == 0).then(|| tokora::diagnostic::Label::new(Location::new(0, SimpleSpan::new(16, 21)), "first defined here"))
//! #   }
//! #   fn path_segments(&self) -> usize { 0 }
//! #   fn path_segment(&self, _: usize) -> Option<tokora::diagnostic::PathSegment<'_>> { None }
//! #   fn help(&self) -> Option<&'static str> { Some("rename one of the two definitions") }
//! # }
//! let error = Redefined;
//!
//! let mut labels = [Label::new(painty::Location::entire(0), ""); 4];
//! let mut path: [PathSegment<'_>; 0] = [];
//! let adapted = adapt(&error, &mut labels, &mut path);
//! assert!(adapted.is_complete());
//!
//! let diagnostic = adapted.diagnostic();
//! assert_eq!(diagnostic.code(), "mylang::resolve::redefined");
//! assert_eq!(diagnostic.labels().len(), 1);
//!
//! let source = Source::new("type Widget {\n  width: Int\n  width: Int\n}\n");
//! let primary = source.resolve(diagnostic.primary().span().unwrap());
//! assert_eq!((primary.start().line(), primary.start().column()), (3, 3));
//! assert_eq!(primary.text(), "width");
//! ```

use ::tokora::diagnostic::{Diagnose, DiagnoseExt};

use crate::{Diagnostic, Label, Location, PathSegment, Severity, Span};

impl From<::tokora::diagnostic::Severity> for Severity {
  /// # An unrecognised rung becomes an [`Error`](Severity::Error)
  ///
  /// tokora's ladder is `#[non_exhaustive]`, so this match carries a fallback arm and a fourth
  /// rung added upstream will arrive here without breaking the build — the one place in this
  /// module where drift is not caught by the compiler. It is mapped to the loudest rung
  /// deliberately: a diagnostic shown more urgently than it deserves is noticed and reported,
  /// where one quietly demoted to advice is filtered out and never seen again.
  #[inline]
  fn from(severity: ::tokora::diagnostic::Severity) -> Self {
    match severity {
      ::tokora::diagnostic::Severity::Error => Self::Error,
      ::tokora::diagnostic::Severity::Warning => Self::Warning,
      ::tokora::diagnostic::Severity::Advice => Self::Advice,
      _ => Self::Error,
    }
  }
}

impl From<::tokora::SimpleSpan> for Span {
  #[inline]
  fn from(span: ::tokora::SimpleSpan) -> Self {
    Self::new(span.start(), span.end())
  }
}

impl From<::tokora::diagnostic::Location> for Location {
  #[inline]
  fn from(location: ::tokora::diagnostic::Location) -> Self {
    match location.span() {
      Some(span) => Self::new(location.source(), span.into()),
      None => Self::entire(location.source()),
    }
  }
}

impl From<::tokora::diagnostic::Label> for Label<'static> {
  #[inline]
  fn from(label: ::tokora::diagnostic::Label) -> Self {
    Self::new(label.location().into(), label.text())
  }
}

impl<'a> From<::tokora::diagnostic::PathSegment<'a>> for PathSegment<'a> {
  #[inline]
  fn from(segment: ::tokora::diagnostic::PathSegment<'a>) -> Self {
    match segment {
      ::tokora::diagnostic::PathSegment::Field(name) => Self::Field(name),
      // Widening, so nothing can be lost. painty's index is deliberately wider than tokora's:
      // painty's model is not shaped by tokora's — that is what makes this an adapter rather than
      // a re-export — and a result path entry has no 32-bit cap in any specification.
      ::tokora::diagnostic::PathSegment::Index(index) => Self::Index(u64::from(index)),
    }
  }
}

/// What [`adapt`] produced, and what it could not fit.
///
/// [`is_complete`](Self::is_complete) is the question worth asking before rendering: a `false`
/// means the buffers were too small and part of the diagnostic is missing from the view.
#[derive(Debug, Clone, Copy)]
#[must_use = "the view is inside this; `diagnostic()` takes it out"]
pub struct Adapted<'a> {
  diagnostic: Diagnostic<'a>,
  dropped_labels: usize,
  dropped_path_segments: usize,
}

impl<'a> Adapted<'a> {
  /// Returns the view.
  #[inline]
  pub const fn diagnostic(&self) -> Diagnostic<'a> {
    self.diagnostic
  }

  /// Returns how many secondary labels did not fit in the buffer they were given.
  #[inline]
  pub const fn dropped_labels(&self) -> usize {
    self.dropped_labels
  }

  /// Returns how many result-path segments did not fit in the buffer they were given.
  #[inline]
  pub const fn dropped_path_segments(&self) -> usize {
    self.dropped_path_segments
  }

  /// Returns whether the whole diagnostic reached the view.
  #[inline]
  pub const fn is_complete(&self) -> bool {
    self.dropped_labels == 0 && self.dropped_path_segments == 0
  }
}

/// Reads `diagnose` into a [`Diagnostic`], writing its labels and result path into `labels` and
/// `path`.
///
/// # Sizing the buffers
///
/// A diagnostic carries nought to five labels in practice, so a small array on the stack is the
/// expected shape. Anything past the end of either buffer is dropped and counted — see
/// [`Adapted::is_complete`]. A caller with no labels to place passes `&mut []`.
///
/// The buffers are **not** sized from [`Diagnose::labels`], deliberately. That count comes from the
/// implementor and nothing in the type system makes it true; tokora's own iterators refuse to
/// forward it for the same reason, having measured a safe impl that declares a million labels
/// behind one real one. Reading the count and reserving from it reproduces exactly that hazard.
/// What arrives here is what tokora's adapters would actually yield, which stops at the first
/// index the accessor does not answer.
pub fn adapt<'a, 'buffer>(
  diagnose: &'a dyn Diagnose,
  labels: &'buffer mut [Label<'a>],
  path: &'buffer mut [PathSegment<'a>],
) -> Adapted<'buffer>
where
  'a: 'buffer,
{
  // The return type is spelled out because a `&mut [T]` is invariant in `T`. tokora's label text
  // is `&'static str`, so `Label::from` produces a `Label<'static>` and the buffer's element type
  // would have to equal that exactly — which would demand `'a: 'static` of the diagnostic. Naming
  // the closure's result puts the (perfectly ordinary) shortening coercion on the value instead.
  let (labels, dropped_labels) = fill(
    labels,
    diagnose
      .labels_iter()
      .map(|label| -> Label<'a> { Label::from(label) }),
  );
  let (path, dropped_path_segments) =
    fill(path, diagnose.path_segments_iter().map(PathSegment::from));

  let mut diagnostic = Diagnostic::new(
    diagnose.code().as_str(),
    diagnose.severity().into(),
    diagnose,
    diagnose.primary().into(),
  )
  .with_labels(labels)
  .with_path(path);

  if let Some(text) = diagnose.primary_label() {
    diagnostic = diagnostic.with_primary_label(text);
  }
  if let Some(text) = diagnose.help() {
    diagnostic = diagnostic.with_help(text);
  }

  Adapted {
    diagnostic,
    dropped_labels,
    dropped_path_segments,
  }
}

/// Writes as much of `items` into `buffer` as fits, returning the filled prefix and the overflow.
fn fill<T>(buffer: &mut [T], items: impl Iterator<Item = T>) -> (&[T], usize) {
  let mut written = 0;
  let mut dropped = 0;

  for item in items {
    match buffer.get_mut(written) {
      Some(slot) => {
        *slot = item;
        written += 1;
      }
      None => dropped += 1,
    }
  }

  (&buffer[..written], dropped)
}
