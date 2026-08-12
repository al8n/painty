//! The four diagnostics the SVG gallery draws.
//!
//! One definition, reached from two places by `#[path]`: `examples/svg_gallery.rs` writes them to
//! `assets/svg/`, and `tests/svg_gallery.rs` re-renders them and compares. Two copies of these
//! literals would drift, and the drift would be invisible — the test would pass against its own
//! copy while the committed image described the other one.
//!
//! Each specimen names the thing it is here to show, because a gallery whose entries differ only
//! decoratively is four pictures of one case.

#![allow(dead_code)]

use painty::{Diagnostic, Input, Location, Severity, Source, Span};

/// A diagnostic and the inputs it points into, under the name its file takes.
pub struct Specimen {
  pub name: &'static str,
  pub text: &'static str,
  pub build: fn(&'static str) -> Diagnostic<'static>,
}

impl Specimen {
  pub fn inputs(&self) -> [Input<'static>; 1] {
    [Input::new(Source::new(self.text))]
  }
}

/// One primary label on one line, plus help. The shape the `render_svg` doc-comment uses.
fn duplicate_field(_: &'static str) -> Diagnostic<'static> {
  Diagnostic::new(
    "mylang::schema::duplicate-field",
    Severity::Error,
    &"`width` is defined twice",
    Location::new(0, Span::new(15, 20)),
  )
  .with_primary_label("redefined here")
  .with_help("rename one of them, or remove the second")
}

/// A warning with no help: severity changes the header's class and nothing else.
fn unused_binding(_: &'static str) -> Diagnostic<'static> {
  Diagnostic::new(
    "mylang::lint::unused",
    Severity::Warning,
    &"`total` is never read",
    Location::new(0, Span::new(4, 9)),
  )
  .with_primary_label("assigned here")
}

/// Two-cell glyphs — the case the cell-advance seam exists for.
///
/// `型` occupies two cells, so everything after it on the source row is displaced by two, and the
/// caret row has to be displaced with it. A renderer that measured the row's total width instead
/// of carrying each unit's advance puts the carets under the wrong glyphs here and nowhere else.
fn wide_glyphs(_: &'static str) -> Diagnostic<'static> {
  Diagnostic::new(
    "mylang::type::mismatch",
    Severity::Error,
    &"expected an integer, found a string",
    Location::new(0, Span::new(6, 12)),
  )
  .with_primary_label("this is a string")
}

/// Advice, four source lines, and a span on the third — so the anchor walk has somewhere to go.
fn naming_advice(_: &'static str) -> Diagnostic<'static> {
  Diagnostic::new(
    "mylang::style::naming",
    Severity::Advice,
    &"single-letter names are hard to search for",
    Location::new(0, Span::new(30, 31)),
  )
  .with_primary_label("consider a longer name")
  .with_help("`y` appears twice in this function")
}

pub const SPECIMENS: &[Specimen] = &[
  Specimen {
    name: "duplicate-field",
    text: "type Widget {\n  width: Int\n  width: String\n}\n",
    build: duplicate_field,
  },
  Specimen {
    name: "unused-binding",
    text: "let total = count + 1;\n",
    build: unused_binding,
  },
  Specimen {
    name: "wide-glyphs",
    text: "型 = \"幅\"\n",
    build: wide_glyphs,
  },
  Specimen {
    name: "naming-advice",
    text: "fn main() {\n  let x = 1;\n  let y = 2;\n  print(x + y);\n}\n",
    build: naming_advice,
  },
];
