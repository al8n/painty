//! The `tokora` feature's adapter.
//!
//! # This file is absent, not empty, when the feature is off
//!
//! `Cargo.toml` names it with `required-features = ["tokora"]`, so cargo does not build the target
//! at all in any other configuration. `#![cfg(feature = "tokora")]` inside the file would have
//! compiled an empty test binary that reports as a passing suite, which is the shape of a gate
//! that proves nothing — the adapter and everything exercising it leave the build together, or the
//! claim that painty does not hard-bind to tokora is only prose.

use core::{cell::Cell, fmt};

use painty::{
  Label, Location, PathSegment, Severity, Source, Span,
  tokora::{Adapted, adapt},
};
use tokora::{
  SimpleSpan,
  diagnostic::{Code, Diagnose},
};

const SCHEMA: &str = "type Widget {\n  width: Int\n  width: Int\n}\n";

/// A tokora diagnostic with everything the contract can carry.
struct Redefined {
  labels: usize,
  path: &'static [tokora::diagnostic::PathSegment<'static>],
  severity: tokora::diagnostic::Severity,
}

impl Default for Redefined {
  fn default() -> Self {
    Self {
      labels: 1,
      path: &[],
      severity: tokora::diagnostic::Severity::Error,
    }
  }
}

impl fmt::Display for Redefined {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("`width` is defined twice")
  }
}

impl Diagnose for Redefined {
  fn code(&self) -> Code {
    Code::new("mylang::schema::duplicate-field")
  }

  fn severity(&self) -> tokora::diagnostic::Severity {
    self.severity
  }

  fn primary(&self) -> tokora::diagnostic::Location {
    tokora::diagnostic::Location::new(0, SimpleSpan::new(29, 34))
  }

  fn primary_label(&self) -> Option<&'static str> {
    Some("redefined here")
  }

  fn labels(&self) -> usize {
    self.labels
  }

  fn label(&self, index: usize) -> Option<tokora::diagnostic::Label> {
    (index < self.labels).then(|| {
      tokora::diagnostic::Label::new(
        tokora::diagnostic::Location::new(0, SimpleSpan::new(16, 21)),
        "first defined here",
      )
    })
  }

  fn path_segments(&self) -> usize {
    self.path.len()
  }

  fn path_segment(&self, index: usize) -> Option<tokora::diagnostic::PathSegment<'_>> {
    self.path.get(index).copied()
  }

  fn help(&self) -> Option<&'static str> {
    Some("rename one of the two definitions")
  }
}

fn empty_labels<const N: usize>() -> [Label<'static>; N] {
  [Label::new(Location::entire(0), ""); N]
}

#[test]
fn the_whole_contract_reaches_the_view() {
  let error = Redefined::default();
  let mut labels = empty_labels::<4>();
  let mut path: [PathSegment<'_>; 4] = [PathSegment::Index(0); 4];

  let adapted = adapt(&error, &mut labels, &mut path);
  assert!(adapted.is_complete());

  let diagnostic = adapted.diagnostic();
  assert_eq!(diagnostic.code(), "mylang::schema::duplicate-field");
  assert_eq!(diagnostic.severity(), Severity::Error);
  assert_eq!(diagnostic.message().to_string(), "`width` is defined twice");
  assert_eq!(diagnostic.primary(), Location::new(0, Span::new(29, 34)));
  assert_eq!(diagnostic.primary_label(), Some("redefined here"));
  assert_eq!(diagnostic.help(), Some("rename one of the two definitions"));
  assert_eq!(
    diagnostic.labels(),
    &[Label::new(
      Location::new(0, Span::new(16, 21)),
      "first defined here"
    )]
  );
  assert!(diagnostic.path().is_empty());
}

#[test]
fn the_adapted_view_resolves_against_a_source_like_any_other() {
  let error = Redefined::default();
  let mut labels = empty_labels::<4>();
  let mut path: [PathSegment<'_>; 0] = [];
  let adapted = adapt(&error, &mut labels, &mut path);
  let diagnostic = adapted.diagnostic();

  let source = Source::new(SCHEMA);
  let primary = source.resolve(
    diagnostic
      .primary()
      .span()
      .expect("a positioned diagnostic"),
  );
  assert_eq!(primary.text(), "width");
  assert_eq!(
    (primary.start().line(), primary.start().column()),
    (3, 3),
    "the second `width`"
  );

  let secondary = source.resolve(
    diagnostic.labels()[0]
      .location()
      .span()
      .expect("a positioned label"),
  );
  assert_eq!(
    (secondary.start().line(), secondary.start().column()),
    (2, 3),
    "the first `width`"
  );
}

#[test]
fn a_result_path_crosses_intact() {
  let error = Redefined {
    path: &[
      tokora::diagnostic::PathSegment::Field("heroes"),
      tokora::diagnostic::PathSegment::Index(2),
      tokora::diagnostic::PathSegment::Field("name"),
    ],
    ..Redefined::default()
  };
  let mut labels = empty_labels::<4>();
  let mut path: [PathSegment<'_>; 4] = [PathSegment::Index(0); 4];

  let adapted = adapt(&error, &mut labels, &mut path);
  assert!(adapted.is_complete());
  assert_eq!(
    adapted.diagnostic().path(),
    &[
      PathSegment::Field("heroes"),
      PathSegment::Index(2),
      PathSegment::Field("name")
    ]
  );
}

#[test]
fn overflow_is_reported_rather_than_dropped_in_silence() {
  let error = Redefined {
    labels: 5,
    path: &[
      tokora::diagnostic::PathSegment::Field("heroes"),
      tokora::diagnostic::PathSegment::Index(2),
    ],
    ..Redefined::default()
  };
  let mut labels = empty_labels::<2>();
  let mut path: [PathSegment<'_>; 1] = [PathSegment::Index(0)];

  let adapted = adapt(&error, &mut labels, &mut path);
  assert!(!adapted.is_complete());
  assert!(adapted.labels_dropped());
  assert!(adapted.path_segments_dropped());
  assert_eq!(adapted.diagnostic().labels().len(), 2);
  assert_eq!(adapted.diagnostic().path().len(), 1);
}

#[test]
fn a_buffer_that_exactly_fits_is_not_reported_as_overflowing() {
  // The boundary the probe exists for. Two labels declared, two slots: the probe is what tells
  // this apart from three labels in two slots, and it must not itself invent an overflow.
  let error = Redefined {
    labels: 2,
    ..Redefined::default()
  };
  let mut labels = empty_labels::<2>();

  let adapted = adapt(&error, &mut labels, &mut []);
  assert!(adapted.is_complete());
  assert!(!adapted.labels_dropped());
  assert_eq!(adapted.diagnostic().labels().len(), 2);
}

#[test]
fn empty_buffers_are_a_legitimate_answer() {
  let error = Redefined {
    labels: 0,
    ..Redefined::default()
  };
  let adapted = adapt(&error, &mut [], &mut []);

  assert!(adapted.is_complete());
  assert!(adapted.diagnostic().labels().is_empty());
}

#[test]
fn every_rung_of_the_ladder_maps() {
  for (from, to) in [
    (tokora::diagnostic::Severity::Error, Severity::Error),
    (tokora::diagnostic::Severity::Warning, Severity::Warning),
    (tokora::diagnostic::Severity::Advice, Severity::Advice),
  ] {
    assert_eq!(Severity::from(from), to);
  }
}

/// An impl whose declared count is larger than what its accessor will answer.
///
/// tokora's own iterators fail closed on that, and the adapter is built on them rather than on the
/// count, so the hole has to stop the walk here too. The `Cell` is what makes the defect a moving
/// one rather than a constant, which is the shape a `&self` accessor can produce without `unsafe`.
struct Lying {
  answered: Cell<usize>,
}

impl fmt::Display for Lying {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("a diagnostic that overstates itself")
  }
}

impl Diagnose for Lying {
  fn code(&self) -> Code {
    Code::new("mylang::test::lying")
  }
  fn severity(&self) -> tokora::diagnostic::Severity {
    tokora::diagnostic::Severity::Warning
  }
  fn primary(&self) -> tokora::diagnostic::Location {
    tokora::diagnostic::Location::entire(0)
  }
  fn primary_label(&self) -> Option<&'static str> {
    None
  }
  fn labels(&self) -> usize {
    1_000_000
  }
  fn label(&self, index: usize) -> Option<tokora::diagnostic::Label> {
    self.answered.set(self.answered.get() + 1);
    (index == 0).then(|| {
      tokora::diagnostic::Label::new(
        tokora::diagnostic::Location::new(0, SimpleSpan::new(0, 1)),
        "the only real one",
      )
    })
  }
  fn path_segments(&self) -> usize {
    0
  }
  fn path_segment(&self, _: usize) -> Option<tokora::diagnostic::PathSegment<'_>> {
    None
  }
  fn help(&self) -> Option<&'static str> {
    None
  }
}

#[test]
fn a_declared_count_is_not_believed() {
  let error = Lying {
    answered: Cell::new(0),
  };
  let mut labels = empty_labels::<8>();

  let adapted = adapt(&error, &mut labels, &mut []);

  // One real label, and the walk stopped at the hole rather than reading a million indices or
  // reserving for them.
  assert_eq!(adapted.diagnostic().labels().len(), 1);
  assert!(!adapted.labels_dropped());
  assert_eq!(error.answered.get(), 2, "the walk should stop at the hole");
}

/// An impl that declares a huge count and then **answers every index**.
///
/// [`Lying`]'s hole is what tokora's iterators stop at; this one has none, so nothing upstream
/// stops the walk and the adapter's own ceiling is the only thing that can. It is the shape a
/// caller's safe code can take by accident — a generated `label` that recomputes rather than
/// indexing — and before the ceiling existed it turned a four-slot buffer into a million calls.
struct Endless {
  answered: Cell<usize>,
}

impl fmt::Display for Endless {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("a diagnostic with no end of labels")
  }
}

impl Diagnose for Endless {
  fn code(&self) -> Code {
    Code::new("mylang::test::endless")
  }
  fn severity(&self) -> tokora::diagnostic::Severity {
    tokora::diagnostic::Severity::Warning
  }
  fn primary(&self) -> tokora::diagnostic::Location {
    tokora::diagnostic::Location::entire(0)
  }
  fn primary_label(&self) -> Option<&'static str> {
    None
  }
  fn labels(&self) -> usize {
    // Not `usize::MAX`, only because a run against a regressed adapter has to finish in order to
    // report the regression. The hazard this guards is `usize::MAX`, and the assertion below is
    // the same either way.
    1_000_000
  }
  fn label(&self, _: usize) -> Option<tokora::diagnostic::Label> {
    self.answered.set(self.answered.get() + 1);
    Some(tokora::diagnostic::Label::new(
      tokora::diagnostic::Location::new(0, SimpleSpan::new(0, 1)),
      "another one",
    ))
  }
  fn path_segments(&self) -> usize {
    1_000_000
  }
  fn path_segment(&self, _: usize) -> Option<tokora::diagnostic::PathSegment<'_>> {
    Some(tokora::diagnostic::PathSegment::Index(0))
  }
  fn help(&self) -> Option<&'static str> {
    None
  }
}

/// An impl whose declared counts are ZERO and whose accessors have data.
///
/// The shape that made the completeness flag lie. `DiagnoseExt`'s iterators snapshot the count when
/// they are built and yield nothing once the cursor reaches it, so a count of zero produced an
/// empty walk over an accessor with labels behind it — and `is_complete` said the diagnostic had
/// arrived whole. It also counts every call, so "the counts are never read" is asserted rather
/// than argued.
struct UnderReports {
  counts_read: Cell<usize>,
  labels_read: Cell<usize>,
  path_read: Cell<usize>,
}

impl UnderReports {
  fn new() -> Self {
    Self {
      counts_read: Cell::new(0),
      labels_read: Cell::new(0),
      path_read: Cell::new(0),
    }
  }
}

impl fmt::Display for UnderReports {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("a diagnostic that under-reports its own size")
  }
}

impl Diagnose for UnderReports {
  fn code(&self) -> Code {
    Code::new("mylang::test::under-reports")
  }
  fn severity(&self) -> tokora::diagnostic::Severity {
    tokora::diagnostic::Severity::Error
  }
  fn primary(&self) -> tokora::diagnostic::Location {
    tokora::diagnostic::Location::entire(0)
  }
  fn primary_label(&self) -> Option<&'static str> {
    None
  }
  fn labels(&self) -> usize {
    self.counts_read.set(self.counts_read.get() + 1);
    0
  }
  fn label(&self, index: usize) -> Option<tokora::diagnostic::Label> {
    self.labels_read.set(self.labels_read.get() + 1);
    (index < 3).then(|| {
      tokora::diagnostic::Label::new(
        tokora::diagnostic::Location::new(0, SimpleSpan::new(0, 1)),
        "real, and behind a zero",
      )
    })
  }
  fn path_segments(&self) -> usize {
    self.counts_read.set(self.counts_read.get() + 1);
    0
  }
  fn path_segment(&self, index: usize) -> Option<tokora::diagnostic::PathSegment<'_>> {
    self.path_read.set(self.path_read.get() + 1);
    (index < 3).then_some(tokora::diagnostic::PathSegment::Index(7))
  }
  fn help(&self) -> Option<&'static str> {
    None
  }
}

#[test]
fn a_count_of_zero_does_not_hide_data_the_accessor_has() {
  let error = UnderReports::new();
  let mut labels = empty_labels::<2>();
  let mut path: [PathSegment<'_>; 2] = [PathSegment::Index(0); 2];

  let adapted = adapt(&error, &mut labels, &mut path);

  // The accessor has three of each and the buffers hold two, so both truncated — and the flag has
  // to say so. Reading the declared zero instead reported a complete view over nothing.
  assert_eq!(adapted.diagnostic().labels().len(), 2);
  assert_eq!(adapted.diagnostic().path().len(), 2);
  assert!(adapted.labels_dropped());
  assert!(adapted.path_segments_dropped());
  assert!(!adapted.is_complete());

  // And the declared counts were never asked for at all.
  assert_eq!(
    error.counts_read.get(),
    0,
    "the adapter read a count it is not allowed to trust"
  );
}

#[test]
fn a_zero_capacity_buffer_still_reports_the_data_it_could_not_take() {
  let error = UnderReports::new();
  let adapted = adapt(&error, &mut [], &mut []);

  assert!(adapted.labels_dropped());
  assert!(adapted.path_segments_dropped());
  assert!(!adapted.is_complete());
  // One probe each, and no count.
  assert_eq!((error.labels_read.get(), error.path_read.get()), (1, 1));
  assert_eq!(error.counts_read.get(), 0);
}

#[test]
fn the_accessor_is_called_capacity_plus_one_times_and_no_more() {
  let error = UnderReports::new();
  let mut labels = empty_labels::<2>();
  let mut path: [PathSegment<'_>; 1] = [PathSegment::Index(0)];

  let adapted = adapt(&error, &mut labels, &mut path);
  assert!(!adapted.is_complete());

  // Two slots plus a probe; one slot plus a probe. Asserted rather than argued, because the bound
  // is the whole reason the overflow report is a boolean.
  assert_eq!(error.labels_read.get(), 3);
  assert_eq!(error.path_read.get(), 2);
  assert_eq!(error.counts_read.get(), 0);
}

#[test]
fn the_work_is_bounded_by_the_buffer_and_not_by_the_diagnostic() {
  let error = Endless {
    answered: Cell::new(0),
  };
  let mut labels = empty_labels::<3>();
  let mut path: [PathSegment<'_>; 2] = [PathSegment::Index(0); 2];

  let adapted = adapt(&error, &mut labels, &mut path);

  // Three slots filled and one probe to learn there was more: four calls into caller code, not a
  // million. This is the assertion the API decision was made for — counting the overflow exactly
  // is what would require all million.
  assert_eq!(
    error.answered.get(),
    4,
    "the adapter asked the diagnostic more than capacity + 1 times"
  );
  assert!(adapted.labels_dropped());
  assert!(adapted.path_segments_dropped());
  assert!(!adapted.is_complete());
  assert_eq!(adapted.diagnostic().labels().len(), 3);
  assert_eq!(adapted.diagnostic().path().len(), 2);
}

#[test]
fn an_empty_buffer_costs_exactly_one_question() {
  let error = Endless {
    answered: Cell::new(0),
  };
  let adapted = adapt(&error, &mut [], &mut []);

  assert_eq!(error.answered.get(), 1, "zero slots, one probe");
  assert!(adapted.labels_dropped());
}

/// The view is `Copy`, so it outlives the [`Adapted`] it came out of only through the buffers.
#[test]
fn the_view_borrows_the_buffers_it_was_given() {
  let error = Redefined::default();
  let mut labels = empty_labels::<4>();
  let mut path: [PathSegment<'_>; 0] = [];

  let adapted: Adapted<'_> = adapt(&error, &mut labels, &mut path);
  let first = adapted.diagnostic();
  let second = adapted.diagnostic();

  assert_eq!(first.labels(), second.labels());
  assert_eq!(first.code(), second.code());
}
