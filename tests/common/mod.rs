//! A local error type that has never heard of tokora.
//!
//! This is the fixture behind the claim that painty does not hard-bind to anything. Both consumer
//! suites are written against it, and neither of them — nor this file — names a dependency: an
//! ordinary Rust error, a [`Display`](core::fmt::Display) impl, and painty's borrowed view built
//! by hand. If painty ever needs a foreign trait to be useful, these stop compiling.

#![allow(dead_code)]

use core::fmt;

use painty::{Diagnostic, Label, Location, PathSegment, Severity, Span};

/// The document both consumer suites report against.
///
/// It carries an astral-plane character on purpose. A byte offset, a character column and a UTF-16
/// code-unit offset all agree on ASCII, so a fixture without one cannot tell a consumer that gets
/// the distinction right from one that never had to.
pub const SCHEMA: &str = "\"A widget 🎨\"\ntype Widget {\n  width: Int\n  width: Int\n}\n";

/// Returns the byte offsets of the two `width` field declarations.
///
/// Searched for rather than written down. A hardcoded offset that is wrong makes a test assert the
/// wrong thing while staying green, and these offsets are the *input* to what is under test — the
/// line and column they resolve to are what the suites pin, and those are checkable by eye against
/// [`SCHEMA`].
fn width_offsets() -> (usize, usize) {
  let first = SCHEMA.find("width:").expect("the fixture declares `width`");
  let after = first + "width:".len();
  let again = after
    + SCHEMA[after..]
      .find("width:")
      .expect("the fixture declares `width` twice");
  (first, again)
}

/// A field declared twice in one type.
#[derive(Debug)]
pub struct DuplicateField {
  pub name: &'static str,
  pub again: Span,
  pub first: Span,
  pub path: &'static [PathSegment<'static>],
}

impl DuplicateField {
  /// The fixture both suites report.
  pub fn fixture() -> Self {
    let (first, again) = width_offsets();
    let width = "width".len();
    Self {
      name: "width",
      again: Span::new(again, again + width),
      first: Span::new(first, first + width),
      path: &[],
    }
  }

  /// The same error reached through a result rather than a document, so the result path is not
  /// empty.
  pub fn with_path(mut self, path: &'static [PathSegment<'static>]) -> Self {
    self.path = path;
    self
  }

  /// The secondary labels, in storage the caller keeps.
  pub fn labels(&self) -> [Label<'static>; 1] {
    [Label::new(
      Location::new(0, self.first),
      "first defined here",
    )]
  }

  /// painty's view of this error.
  pub fn diagnostic<'a>(&'a self, labels: &'a [Label<'a>]) -> Diagnostic<'a> {
    Diagnostic::new(
      "mylang::schema::duplicate-field",
      Severity::Error,
      self,
      Location::new(0, self.again),
    )
    .with_primary_label("redefined here")
    .with_labels(labels)
    .with_path(self.path)
    .with_help("rename one of the two definitions")
  }
}

impl fmt::Display for DuplicateField {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "`{}` is defined twice", self.name)
  }
}

/// Escapes `text` for a JSON string body.
///
/// Both suites write JSON and need the same handful of escapes; neither is testing a JSON library,
/// so this is the smallest correct one rather than a dependency.
pub fn json_string(text: &str) -> String {
  let mut out = String::with_capacity(text.len() + 2);
  out.push('"');
  for character in text.chars() {
    match character {
      '"' => out.push_str("\\\""),
      '\\' => out.push_str("\\\\"),
      '\n' => out.push_str("\\n"),
      '\r' => out.push_str("\\r"),
      '\t' => out.push_str("\\t"),
      c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
      c => out.push(c),
    }
  }
  out.push('"');
  out
}
