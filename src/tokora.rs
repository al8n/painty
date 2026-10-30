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

use ::tokora::diagnostic::Diagnose;

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

/// What [`adapt`] produced, and whether anything did not fit.
///
/// [`is_complete`](Self::is_complete) is the question worth asking before rendering: a `false`
/// means a buffer was too small and part of the diagnostic is missing from the view.
///
/// # Why "whether" and not "how many"
///
/// This reported exact counts until the walk behind them was priced. An exact count of what was
/// dropped can only be had by looking at everything that was dropped, and the things being looked
/// at are answers from a **caller-implemented trait**. A safe [`Diagnose`] impl may declare a
/// billion labels and go on answering `Some` for every index; counting the overflow then means
/// invoking that code a billion times to fill a buffer that stopped accepting items at four. A
/// caller passing a small array — the shape this adapter recommends — would have bought a hang.
///
/// So the counts became booleans, and [`adapt`] asks for exactly one item beyond what it can
/// store. The information given up is real and small: a renderer can say "and more labels" but no
/// longer "and three more".
///
/// Three alternatives were considered and are recorded so they are not re-proposed:
///
/// - **Read the count from [`Diagnose::labels`]** and subtract. That is the one number tokora
///   documents as untrustworthy — see [`adapt`] — so it would trade an unbounded walk for a
///   wrong answer.
/// - **Walk a bounded number past the end** and report "at least *n*". A bound nobody can choose
///   is a knob, and the honest value of it is one.
/// - **Report nothing.** A caller cannot then tell a complete render from a truncated one, which
///   is the only thing this type exists to say.
///
/// The precision was also worth less than it looked. What an "exact" count is exact *about* is one
/// walk of an impl that is free to answer differently on the next one, so it was never a stable
/// quantity to begin with.
#[derive(Debug, Clone, Copy)]
#[must_use = "the view is inside this; `diagnostic()` takes it out"]
pub struct Adapted<'a> {
  diagnostic: Diagnostic<'a>,
  labels_dropped: bool,
  path_segments_dropped: bool,
}

impl<'a> Adapted<'a> {
  /// Returns the view.
  #[inline]
  pub const fn diagnostic(&self) -> Diagnostic<'a> {
    self.diagnostic
  }

  /// Returns whether a secondary label did not fit in the buffer it was given.
  #[inline]
  pub const fn labels_dropped(&self) -> bool {
    self.labels_dropped
  }

  /// Returns whether a result-path segment did not fit in the buffer it was given.
  #[inline]
  pub const fn path_segments_dropped(&self) -> bool {
    self.path_segments_dropped
  }

  /// Returns whether the whole diagnostic reached the view.
  #[inline]
  pub const fn is_complete(&self) -> bool {
    !self.labels_dropped && !self.path_segments_dropped
  }
}

/// Reads `diagnose` into a [`Diagnostic`], writing its labels and result path into `labels` and
/// `path`.
///
/// # Sizing the buffers
///
/// A diagnostic carries nought to five labels in practice, so a small array on the stack is the
/// expected shape. Anything past the end of either buffer is dropped, and
/// [`Adapted::is_complete`] says so. A caller with no labels to place passes `&mut []`.
///
/// # No number the caller supplies decides anything here
///
/// [`Diagnose::labels`] and [`Diagnose::path_segments`] are **never called**. Not to size a buffer,
/// not to bound a walk, not to decide whether anything was dropped. The accessors are driven
/// directly, from index zero, stopping at the first `None`.
///
/// That is stricter than it first appears, and it is deliberate rather than stylistic. Reading the
/// counts through [`DiagnoseExt`](::tokora::diagnostic::DiagnoseExt)'s iterators — which is what
/// this did — snapshots `labels()` when the iterator is *constructed* and yields nothing once the
/// cursor reaches it. An impl answering `0` there while `label(0)` returns `Some` therefore handed
/// this function an empty walk, and [`Adapted::is_complete`] said the diagnostic had arrived whole
/// while its labels were silently gone. A count that cannot be trusted to be large enough cannot be
/// trusted to be small enough either, and the second direction corrupts a flag rather than merely
/// wasting time.
///
/// So the accessor is the only authority. Where it and the count disagree, the count is not
/// consulted, which also means this can yield more than tokora's own iterators would.
///
/// # The work is bounded by YOUR buffer, not by the diagnostic
///
/// A [`Diagnose`] impl is caller-written code, so it is untrusted input even though it is safe
/// Rust. Each collection costs at most `capacity + 1` accessor calls: `capacity` to fill the
/// buffer, and one more to learn whether there was anything else. An impl with a billion labels
/// costs the same as one with five.
///
/// What this does **not** buy is totality. `code`, `severity`, `primary` and the accessors are all
/// caller code, and any one of them may take as long as it likes; dropping the count removes one
/// such call and one such *decision*, not the possibility that a caller hangs in a method this
/// function has to call. `tests/tokora_adapter.rs` counts the calls rather than trusting this
/// paragraph, and asserts that the counts are never called at all.
pub fn adapt<'a, 'buffer>(
  diagnose: &'a dyn Diagnose,
  labels: &'buffer mut [Label<'a>],
  path: &'buffer mut [PathSegment<'a>],
) -> Adapted<'buffer>
where
  'a: 'buffer,
{
  // Indexed accessors, not `DiagnoseExt`'s iterators: those read the declared count when they are
  // constructed, and a count of zero over an accessor that has data made this report a complete
  // diagnostic with its labels missing.
  //
  // The return type is spelled out because a `&mut [T]` is invariant in `T`. tokora's label text is
  // `&'static str`, so `Label::from` produces a `Label<'static>` and the buffer's element type
  // would have to equal that exactly — which would demand `'a: 'static` of the diagnostic. Naming
  // the closure's result puts the (perfectly ordinary) shortening coercion on the value instead.
  let (labels, labels_dropped) = fill(labels, |index| {
    diagnose
      .label(index)
      .map(|label| -> Label<'a> { label.into() })
  });
  let (path, path_segments_dropped) = fill(path, |index| {
    diagnose.path_segment(index).map(PathSegment::from)
  });

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
    labels_dropped,
    path_segments_dropped,
  }
}

/// Reads `item` from index zero into `buffer`, and says whether there was more.
///
/// Calls `item` at most `buffer.len() + 1` times, and that ceiling is the whole point: `item` runs
/// a caller's [`Diagnose`] accessor, which this function does not control and cannot bound.
/// Draining it to count the remainder — what an exact overflow count costs — lets a small buffer
/// and a large diagnostic turn one adapter call into an unbounded one.
///
/// The final call is the probe, and it is the least that can distinguish a buffer that happened to
/// fit everything from one that did not. Nothing else decides that: no declared count is read, so
/// an implementation that under-reports cannot make this report a complete view over a truncated
/// one.
fn fill<T>(buffer: &mut [T], mut item: impl FnMut(usize) -> Option<T>) -> (&[T], bool) {
  let mut written = 0;
  while written < buffer.len() {
    let Some(value) = item(written) else {
      return (&buffer[..written], false);
    };
    buffer[written] = value;
    written += 1;
  }

  let dropped = item(written).is_some();
  (&buffer[..written], dropped)
}
