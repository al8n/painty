//! Second consumer: a Language Server Protocol `Diagnostic`.
//!
//! # Why this one and not a second renderer
//!
//! It asks for the resolved model in the opposite dialect to its sibling
//! `graphql_error_locations.rs`: **0-based** rather than 1-based, a half-open **range** rather than
//! a point, and characters counted in **UTF-16 code units** rather than in characters. A layer that
//! only ever satisfies one caller's conventions has not been shown to be a layer, and the second
//! caller is where a model that quietly baked one dialect in stops fitting.
//!
//! # The UTF-16 finding, recorded here because a test is where it can be checked
//!
//! The design's cost table says an LSP mapping "needs only line/column". That is true of ASCII and
//! false of anything else: LSP's default `positionEncoding` is `utf-16`, and painty reports
//! *character* columns, so the two part company on the first astral-plane character. painty does
//! not grow a fourth column unit for it — the resolved model hands back the line's text and the
//! byte offset within it, which is everything the conversion needs, and it stays out of a business
//! whose right answer depends on a protocol capability painty cannot see.
//!
//! # Also written against a hand-rolled error type
//!
//! No dependency, and no tokora. Half of the proof that painty's core hard-binds to nothing.

mod common;

use common::{DuplicateField, SCHEMA, json_string};
use painty::{Label, Severity, Source, Span};

/// LSP's `DiagnosticSeverity`.
fn severity_code(severity: Severity) -> u8 {
  match severity {
    Severity::Error => 1,
    Severity::Warning => 2,
    Severity::Advice => 4,
    // painty's ladder is `#[non_exhaustive]`, so a consumer outside the crate carries this arm.
    _ => 1,
  }
}

/// An LSP `Position`: 0-based line, and a character counted in UTF-16 code units.
///
/// # The narrowing is here, and that is the right place for it
///
/// LSP's `uinteger` is 32 bits; painty's ordinals are `u64`. The conversion is the protocol
/// adapter's, made where the protocol's limit is known, and it is `try_from` rather than a cast —
/// a document too large for LSP to address is something a server has to answer for, not something
/// a renderer should have silently clamped on its behalf several layers earlier.
fn lsp_position(source: Source<'_>, offset: usize) -> (u32, u32) {
  let at = source.position(offset);
  let line = source.line_at(offset);

  // A resolved offset never leaves the content of the line it resolved to, which is what makes
  // this an unguarded index rather than a clamp: `Source::floor` moves an offset out of a CRLF,
  // and an offset at a break is the end of the content before it.
  let content = line.span();
  let byte_in_line = at.offset() - content.start();
  let character = line.text()[..byte_in_line].encode_utf16().count();

  (
    u32::try_from(at.line() - 1).expect("the fixture is addressable by LSP"),
    u32::try_from(character).expect("the fixture is addressable by LSP"),
  )
}

/// An LSP `Range` for a span, resolved against the text it points into.
fn lsp_range(source: Source<'_>, span: Span) -> String {
  let region = source.resolve(span);
  let (start_line, start_character) = lsp_position(source, region.span().start());
  let (end_line, end_character) = lsp_position(source, region.span().end());

  format!(
    "{{\"start\":{{\"line\":{start_line},\"character\":{start_character}}},\
     \"end\":{{\"line\":{end_line},\"character\":{end_character}}}}}"
  )
}

/// Writes an LSP `Diagnostic`.
///
/// The `uri` is the caller's, not painty's: a [`Location`](painty::Location) carries a `source`
/// index into the caller's own list of inputs, and painty owns no mapping from that to a name.
fn lsp_diagnostic(error: &DuplicateField, source: Source<'_>, uri: &str) -> String {
  let labels = error.labels();
  let diagnostic = error.diagnostic(&labels);

  let mut out = String::from("{\"range\":");
  match diagnostic.primary().span() {
    Some(span) => out.push_str(&lsp_range(source, span)),
    // A diagnostic about the input as a whole. LSP's range is not optional, so the only honest
    // answer is the whole document, and the caller has to make that choice rather than painty.
    None => out.push_str(&lsp_range(source, Span::new(0, source.len()))),
  }

  out.push_str(&format!(
    ",\"severity\":{}",
    severity_code(diagnostic.severity())
  ));
  out.push_str(",\"code\":");
  out.push_str(&json_string(diagnostic.code()));
  out.push_str(",\"message\":");
  out.push_str(&json_string(&diagnostic.message().to_string()));

  if !diagnostic.labels().is_empty() {
    out.push_str(",\"relatedInformation\":[");
    for (index, label) in diagnostic.labels().iter().enumerate() {
      if index > 0 {
        out.push(',');
      }
      let range = match label.location().span() {
        Some(span) => lsp_range(source, span),
        None => lsp_range(source, Span::new(0, source.len())),
      };
      out.push_str(&format!(
        "{{\"location\":{{\"uri\":{},\"range\":{range}}},\"message\":{}}}",
        json_string(uri),
        json_string(label.text())
      ));
    }
    out.push(']');
  }

  out.push('}');
  out
}

#[test]
fn a_diagnostic_maps_to_a_zero_based_half_open_range() {
  let error = DuplicateField::fixture();
  let written = lsp_diagnostic(&error, Source::new(SCHEMA), "file:///widget.graphql");

  // Line 4 of SCHEMA, 0-based 3; `width` starts at its third character, 0-based 2, and runs five.
  assert_eq!(
    written,
    "{\"range\":{\"start\":{\"line\":3,\"character\":2},\"end\":{\"line\":3,\"character\":7}}\
     ,\"severity\":1\
     ,\"code\":\"mylang::schema::duplicate-field\"\
     ,\"message\":\"`width` is defined twice\"\
     ,\"relatedInformation\":[{\"location\":{\"uri\":\"file:///widget.graphql\"\
     ,\"range\":{\"start\":{\"line\":2,\"character\":2},\"end\":{\"line\":2,\"character\":7}}}\
     ,\"message\":\"first defined here\"}]}"
  );
}

#[test]
fn utf16_and_character_columns_part_company_on_an_astral_character() {
  let source = Source::new(SCHEMA);
  let quote = SCHEMA.rfind('"').expect("the description is quoted");

  // painty's own unit: eleven characters precede the closing quote, so it is at column 12.
  let at = source.position(quote);
  assert_eq!((at.line(), at.column()), (1, 12));

  // LSP's unit: the emoji is one character and two UTF-16 code units, so the 0-based character is
  // 12 where the 0-based character column would be 11. A consumer that reused the column here
  // would place every LSP position after an emoji one unit early.
  let (line, character) = lsp_position(source, quote);
  assert_eq!((line, character), (0, 12));
  assert_ne!(u64::from(character), at.column() - 1);
}

#[test]
fn a_range_over_the_astral_character_itself_is_two_code_units_wide() {
  let source = Source::new(SCHEMA);
  let emoji = SCHEMA.find('🎨').expect("the description carries an emoji");
  let range = lsp_range(source, Span::new(emoji, emoji + '🎨'.len_utf8()));

  assert_eq!(
    range,
    "{\"start\":{\"line\":0,\"character\":10},\"end\":{\"line\":0,\"character\":12}}"
  );
}

#[test]
fn a_multiline_range_spans_the_lines_the_region_covers() {
  let source = Source::new(SCHEMA);
  let first = SCHEMA.find("width:").expect("the fixture declares `width`");
  let last = SCHEMA.rfind("Int").expect("the fixture declares a type") + "Int".len();

  let region = source.resolve(Span::new(first, last));
  assert!(region.is_multiline());
  assert_eq!(region.line_count(), 2);

  assert_eq!(
    lsp_range(source, Span::new(first, last)),
    "{\"start\":{\"line\":2,\"character\":2},\"end\":{\"line\":3,\"character\":12}}"
  );
}

#[test]
fn an_unpositioned_label_leaves_the_choice_of_range_with_the_caller() {
  let error = DuplicateField::fixture();
  let source = Source::new(SCHEMA);

  let labels = [Label::new(painty::Location::entire(0), "somewhere in here")];
  let diagnostic = error.diagnostic(&labels);

  // painty reports the absence rather than inventing `0..len`; the writer above is where the
  // whole-document fallback is chosen, because LSP is the protocol that insists on a range.
  assert!(diagnostic.labels()[0].location().span().is_none());
  assert_eq!(
    lsp_range(source, Span::new(0, source.len())),
    "{\"start\":{\"line\":0,\"character\":0},\"end\":{\"line\":5,\"character\":0}}"
  );
}
