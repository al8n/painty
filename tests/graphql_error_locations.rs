//! First consumer: a GraphQL response's `errors[]` entry.
//!
//! # What this suite is for
//!
//! It is not a GraphQL implementation and painty is not going to grow one. It exists to answer a
//! question about the *boundary*: is the resolved model enough, on its own, to write a consumer
//! that renders nothing? The specification's error entry wants a message, a `{line, column}` per
//! location, a result path and an extension code, and every one of those has to come out of
//! painty's view and painty's resolution or the layer is not a layer.
//!
//! Its sibling, `lsp_diagnostic.rs`, asks the same question from the opposite direction — 0-based,
//! UTF-16, half-open ranges rather than 1-based character points. Two consumers with no rendering
//! between them is what makes the boundary real rather than an artefact of one caller.
//!
//! # And it is written against a hand-rolled error type
//!
//! `common::DuplicateField` is an ordinary Rust struct with a `Display` impl. Nothing in this file
//! or that one depends on tokora, so this suite is also half of the proof that painty's core does
//! not: it compiles and passes under `--no-default-features`, in a build whose dependency graph is
//! empty.

mod common;

use common::{DuplicateField, SCHEMA, json_string};
use painty::{PathSegment, Source};

/// Writes one entry of a GraphQL response's `errors` list.
///
/// The specification (§7.1.2) says an error "must contain an entry with the key `message`", "should
/// contain an entry with the key `locations`" whose entries are `{line, column}` "describing the
/// beginning of an associated syntax element", and — for an error during execution — a `path`
/// whose entries "should be strings for Object fields, and 0-indexed integers for List entries".
fn error_entry(error: &DuplicateField, source: Source<'_>) -> String {
  let labels = error.labels();
  let diagnostic = error.diagnostic(&labels);

  let mut out = String::from("{\"message\":");
  out.push_str(&json_string(&diagnostic.message().to_string()));

  // `locations` describes the BEGINNING of a syntax element, so it is a point rather than a range.
  // The primary position of a diagnostic about the input as a whole has no span, and the entry
  // then carries no `locations` at all — which is what the specification's "should" leaves room
  // for, and is more honest than pointing at line 1.
  if let Some(span) = diagnostic.primary().span() {
    let at = source.position(span.start());
    out.push_str(&format!(
      ",\"locations\":[{{\"line\":{},\"column\":{}}}]",
      at.line(),
      at.column()
    ));
  }

  if !diagnostic.path().is_empty() {
    out.push_str(",\"path\":[");
    for (index, segment) in diagnostic.path().iter().enumerate() {
      if index > 0 {
        out.push(',');
      }
      match segment {
        PathSegment::Field(name) => out.push_str(&json_string(name)),
        PathSegment::Index(at) => out.push_str(&at.to_string()),
      }
    }
    out.push(']');
  }

  out.push_str(",\"extensions\":{\"code\":");
  out.push_str(&json_string(diagnostic.code()));
  out.push_str("}}");
  out
}

#[test]
fn an_error_entry_carries_the_position_the_diagnostic_points_at() {
  let error = DuplicateField::fixture();
  let entry = error_entry(&error, Source::new(SCHEMA));

  // Line 4 of SCHEMA is the second `  width: Int`, and `width` starts at its third character.
  assert_eq!(
    entry,
    "{\"message\":\"`width` is defined twice\
     \",\"locations\":[{\"line\":4,\"column\":3}]\
     ,\"extensions\":{\"code\":\"mylang::schema::duplicate-field\"}}"
  );
}

#[test]
fn a_result_path_reaches_the_entry_as_written() {
  let error = DuplicateField::fixture().with_path(&[
    PathSegment::Field("heroes"),
    PathSegment::Index(2),
    PathSegment::Field("name"),
  ]);
  let entry = error_entry(&error, Source::new(SCHEMA));

  assert!(
    entry.contains("\"path\":[\"heroes\",2,\"name\"]"),
    "the path did not survive the boundary: {entry}"
  );
}

#[test]
fn locations_count_characters_so_an_astral_character_moves_the_column_by_one() {
  let source = Source::new(SCHEMA);

  // The closing quote of the description, which follows a four-byte emoji on line 1.
  let quote = SCHEMA.rfind('"').expect("the description is quoted");
  let at = source.position(quote);

  assert_eq!(at.line(), 1);
  assert_eq!(at.offset(), 14);
  // Twelve characters precede it; fourteen bytes do, and twelve UTF-16 code units do.
  assert_eq!(at.column(), 12);
}

#[test]
fn a_diagnostic_about_the_whole_input_reports_no_location_rather_than_a_made_up_one() {
  let error = DuplicateField::fixture();
  let labels = error.labels();
  let diagnostic = error
    .diagnostic(&labels)
    .with_primary_label("the document as a whole");

  // The view keeps the distinction, so the writer can act on it. Nothing in painty converts a
  // whole-input location into `0..len` on the caller's behalf.
  assert!(diagnostic.primary().span().is_some());

  let whole = painty::Diagnostic::new(
    diagnostic.code(),
    diagnostic.severity(),
    diagnostic.message(),
    painty::Location::entire(0),
  );
  assert!(whole.primary().is_entire());
  assert!(whole.primary().span().is_none());

  let entry = {
    let mut out = String::from("{\"message\":");
    out.push_str(&json_string(&whole.message().to_string()));
    if let Some(span) = whole.primary().span() {
      let at = Source::new(SCHEMA).position(span.start());
      out.push_str(&format!(
        ",\"locations\":[{{\"line\":{},\"column\":{}}}]",
        at.line(),
        at.column()
      ));
    }
    out.push('}');
    out
  };
  assert_eq!(entry, "{\"message\":\"`width` is defined twice\"}");
}
