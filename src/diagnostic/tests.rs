use super::{Diagnostic, Label, Location, PathSegment, Severity, Span};

#[test]
fn a_span_reports_what_it_covers() {
  let span = Span::new(4, 9);
  assert_eq!((span.start(), span.end(), span.len()), (4, 9, 5));
  assert!(!span.is_empty());
  assert!(span.contains(4));
  assert!(span.contains(8));
  assert!(!span.contains(9));

  let caret = Span::empty(4);
  assert!(caret.is_empty());
  assert_eq!(caret.len(), 0);
  // Half-open, so an empty span contains nothing at all — not even its own offset.
  assert!(!caret.contains(4));
}

#[test]
fn a_location_keeps_the_difference_between_a_range_and_a_whole_input() {
  let at = Location::new(3, Span::new(1, 2));
  assert_eq!(at.source(), 3);
  assert_eq!(at.span(), Some(Span::new(1, 2)));
  assert!(!at.is_entire());

  let whole = Location::entire(3);
  assert_eq!(whole.source(), 3);
  assert_eq!(whole.span(), None);
  assert!(whole.is_entire());
  assert_ne!(at, whole);
}

#[test]
fn ordering_puts_an_unpositioned_label_first_within_its_input() {
  let mut labels = [
    Label::new(Location::new(0, Span::new(1, 2)), "positioned"),
    Label::new(Location::entire(0), "whole input"),
  ];
  Label::order(&mut labels);

  assert_eq!(labels[0].text(), "whole input");
  assert_eq!(labels[1].text(), "positioned");
}

#[test]
fn ordering_is_by_input_before_offset() {
  let mut labels = [
    Label::new(
      Location::new(1, Span::new(0, 1)),
      "second input, first byte",
    ),
    Label::new(Location::new(0, Span::new(90, 99)), "first input, late"),
  ];
  Label::order(&mut labels);

  assert_eq!(labels[0].text(), "first input, late");
  assert_eq!(labels[1].text(), "second input, first byte");
}

#[test]
fn ordering_puts_an_enclosing_range_before_the_one_nested_in_it() {
  let mut labels = [
    Label::new(Location::new(0, Span::new(4, 8)), "nested"),
    Label::new(Location::new(0, Span::new(4, 20)), "encloses"),
    Label::new(
      Location::new(0, Span::new(4, 4)),
      "empty, at the same place",
    ),
  ];
  Label::order(&mut labels);

  assert_eq!(labels[0].text(), "encloses");
  assert_eq!(labels[1].text(), "nested");
  assert_eq!(labels[2].text(), "empty, at the same place");
}

#[test]
fn ordering_breaks_a_remaining_tie_on_the_text_so_it_is_deterministic() {
  let location = Location::new(0, Span::new(4, 8));
  let mut labels = [
    Label::new(location, "zebra"),
    Label::new(location, "alpha"),
    Label::new(location, "mango"),
  ];
  Label::order(&mut labels);

  assert_eq!(labels[0].text(), "alpha");
  assert_eq!(labels[1].text(), "mango");
  assert_eq!(labels[2].text(), "zebra");
}

#[test]
fn ordering_keeps_every_label_it_was_given() {
  let original = [
    Label::new(Location::new(0, Span::new(9, 12)), "c"),
    Label::new(Location::new(0, Span::new(1, 4)), "a"),
    Label::new(Location::entire(1), "d"),
    Label::new(Location::new(0, Span::new(5, 6)), "b"),
  ];
  let mut ordered = original;
  Label::order(&mut ordered);

  for label in &original {
    assert!(ordered.contains(label), "{label:?} was lost");
  }
  assert_eq!(ordered.len(), original.len());
}

#[test]
fn a_diagnostic_starts_with_only_what_every_diagnostic_has() {
  let message = "unbalanced braces";
  let diagnostic = Diagnostic::new(
    "mylang::parse::unbalanced",
    Severity::Warning,
    &message,
    Location::new(0, Span::new(2, 3)),
  );

  assert_eq!(diagnostic.code(), "mylang::parse::unbalanced");
  assert_eq!(diagnostic.severity(), Severity::Warning);
  assert_eq!(diagnostic.primary(), Location::new(0, Span::new(2, 3)));
  assert_eq!(diagnostic.primary_label(), None);
  assert!(diagnostic.labels().is_empty());
  assert!(diagnostic.path().is_empty());
  assert_eq!(diagnostic.help(), None);
}

#[test]
fn the_optional_parts_are_the_ones_that_are_attached() {
  let message = "unbalanced braces";
  let labels = [Label::new(Location::new(0, Span::new(0, 1)), "opened here")];
  let path = [PathSegment::Field("hero"), PathSegment::Index(0)];

  let diagnostic = Diagnostic::new(
    "mylang::parse::unbalanced",
    Severity::Error,
    &message,
    Location::new(0, Span::new(2, 3)),
  )
  .with_primary_label("expected `}`")
  .with_labels(&labels)
  .with_path(&path)
  .with_help("close the block");

  assert_eq!(diagnostic.primary_label(), Some("expected `}`"));
  assert_eq!(diagnostic.labels(), &labels);
  assert_eq!(diagnostic.path(), &path);
  assert_eq!(diagnostic.help(), Some("close the block"));
}
