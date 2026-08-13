//! The committed documents in `assets/html/` are what the renderer produces today, and they are
//! HTML.
//!
//! # The same corpus as the SVG gallery, on purpose
//!
//! `tests/common/specimens.rs` is reached by `#[path]` from four places now — the two examples that
//! write the goldens and the two tests that compare them — so a specimen is one definition and a
//! fifth one gets both surfaces. That is also what makes the two galleries worth having side by
//! side: the same four diagnostics, drawn by two renderers that share only layer 2 and the elision
//! rule, are the only direct evidence a reader has that the two agree about what they are showing.
//!
//! # Two of the three gates are the SVG gallery's, and the third could not be
//!
//! `tests/svg_gallery.rs` pins that every committed image is a fresh render, that the specimens
//! differ in what they show, and — the structural one — that every placed cell sits on the grid.
//! The first two transfer unchanged. The third does not: HTML has no grid, no cell and no
//! coordinate, and the whole of what this renderer refuses to do is arithmetic of that kind.
//!
//! A gallery with only the first two gates would be a gallery that never checks its output is
//! HTML. Re-render, compare bytes, done — and the same two assertions would pass just as happily
//! over four files of unbalanced tag soup. So the structural invariant here is the one HTML
//! actually has: **the document parses, and nothing a caller supplied is part of the structure.**
//!
//! # Why html5ever and not this repository's own opinion
//!
//! `tests/html_escaping.rs` reads painty's documents with a tokeniser written in that file, and
//! deliberately so — it is intolerant where a browser recovers, which is the property that test
//! needs. But it is painty's own account of what painty writes, checked against painty's own list
//! of the tags painty writes. Every part of that loop is this repository's, so it cannot answer
//! "is this HTML"; it can only answer "is this the markup we meant to emit".
//!
//! This one asks something the repository cannot answer about itself, so it asks somebody else:
//! [html5ever], the WHATWG tokeniser and tree builder that Servo ships and that `scraper` and the
//! rest of the ecosystem are built on. It is independent in the way that matters — it was written
//! against the specification, not against painty, it has never seen this crate, and it recovers
//! from exactly the malformations a browser recovers from. So the verdict `errors.is_empty()`
//! carries is the specification's: **the parser had nothing to repair.** An unbalanced element, a
//! stray end tag and a `<` that never became a tag are all parse errors under that definition, and
//! the tree that comes back out of it is the tree a browser would build rather than the one painty
//! believes it wrote.
//!
//! [`markup5ever_rcdom`] holds the tree html5ever builds. It is the smallest thing that does, and
//! its own README is worth quoting rather than paraphrasing: it "is built for the express purpose
//! of writing automated tests for the `html5ever` and `xml5ever` crates", is unsupported, and "has
//! not been fuzzed or tested against arbitrary, malicious, or nontrivial inputs". That is the
//! express purpose it is put to here, and every input is a document this repository generated, so
//! the warning is about a use nobody is making of it. The judgement that matters —
//! `errors.is_empty()` — is html5ever's either way; `RcDom` only stores what it is handed.
//!
//! Both are `[dev-dependencies]`, so painty's own graph is untouched: the `no-tokora` job's
//! `cargo tree --edges normal` still reports no dependencies at all in the default configuration.
//!
//! # The hostile document is not a fifth specimen
//!
//! Escaping needs a caller string made of `<`, `&`, `"`, `'` and a literal `</style>`, and none of
//! the four specimens contains one, because a gallery is a picture of the ordinary case. Adding a
//! fifth would put a picture of an attack in `assets/svg/` too, and would move a corpus the SVG
//! gallery's goldens are committed against. So the hostile case is built here, rendered here and
//! never written to disk; it is the only thing in this file that is not shared.
//!
//! # This test is only as good as its ability to fail
//!
//! Both halves were planted and both were watched failing.
//!
//! One escape was dropped — `src/escape/mod.rs`'s entity table was made to write `'<' => "<"` —
//! and `caller_text_that_looks_like_markup_comes_back_as_text` failed naming the case, on 45 parse
//! errors html5ever raised over the tags the caller's string had turned into. The other three
//! tests **passed** under that plant, which is the whole argument for the hostile document: not
//! one of the four specimens contains a `<`, so re-rendering them and diffing the bytes says
//! nothing whatever about escaping.
//!
//! `Page::close` was then made to write nothing, and
//! `every_specimen_is_html_a_parser_that_is_not_ours_accepts` failed naming the specimen —
//! `duplicate-field, as the renderer writes it` — on `Unexpected open tag div at end of body`.
//! That is the gate the SVG grid assertion is the analogue of, and it fires on a defect that
//! leaves every byte of the caller's text correctly escaped.
//!
//! [html5ever]: https://github.com/servo/html5ever
//! [`markup5ever_rcdom`]: https://crates.io/crates/markup5ever_rcdom

#![cfg(feature = "html")]

use html5ever::{
  ParseOpts, QualName, local_name, ns, parse_fragment, tendril::TendrilSink,
  tokenizer::TokenizerOpts, tree_builder::TreeBuilderOpts,
};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use painty::{Diagnostic, Input, Label, Location, Severity, Source, Span, html::Html};

#[path = "common/specimens.rs"]
mod specimens;

/// Every element painty's HTML renderer writes, and the whole of what may appear in a document.
///
/// Kept here rather than read off the renderer for the same reason the parser is somebody else's:
/// a census that derives its expectation from the thing under test asserts nothing. This is the
/// list in the `html` module's class table, transcribed by hand.
const ELEMENTS: [&str; 5] = ["div", "p", "pre", "section", "span"];

/// A caller string made of every character that would otherwise become markup, the closing tag the
/// design names, and a payload that would run if any of it escaped.
const HOSTILE: &str = r#"</style><script>alert("x" & 'y')</script>"#;

/// The same characters shaped as a line of source, so the row writer is a sink under test too.
const HOSTILE_LINE: &str = r#"let a = "<b>" & '</style>' > x;"#;

fn render(diagnostic: &Diagnostic<'_>, inputs: &[Input<'_>]) -> String {
  let mut html = String::new();
  Html::new()
    .render(diagnostic, inputs, &mut html)
    .expect("a String accepts everything");
  html
}

fn rendered(specimen: &specimens::Specimen) -> String {
  render(&(specimen.build)(specimen.text), &specimen.inputs())
}

fn committed(specimen: &specimens::Specimen) -> String {
  let path = format!("assets/html/{}.html", specimen.name);
  std::fs::read_to_string(&path).unwrap_or_else(|error| {
    panic!("{path} is missing ({error}); `cargo run --example html_gallery --features html`")
  })
}

#[test]
fn every_committed_document_is_what_the_renderer_writes_today() {
  for specimen in specimens::SPECIMENS {
    let path = format!("assets/html/{}.html", specimen.name);
    assert_eq!(
      rendered(specimen),
      committed(specimen),
      "{path} is not what the renderer writes; re-bless with \
       `cargo run --example html_gallery --features html`"
    );
  }
}

/// The gallery is four documents of four cases, not four documents of one.
///
/// The SVG gallery's assertion, over the same corpus and for the same reason: a gallery whose
/// entries differ only decoratively teaches a reader nothing. It is worth having on both surfaces
/// rather than on one, because two specimens can differ in geometry and not in markup — a wide
/// glyph moves every cell in an SVG row and changes nothing at all about the element tree.
#[test]
fn the_specimens_differ_in_what_they_show() {
  let documents: Vec<String> = specimens::SPECIMENS.iter().map(rendered).collect();
  for (i, a) in documents.iter().enumerate() {
    for (j, b) in documents.iter().enumerate().skip(i + 1) {
      assert_ne!(
        a,
        b,
        "{} and {} render identically",
        specimens::SPECIMENS[i].name,
        specimens::SPECIMENS[j].name
      );
    }
  }
}

/// Every specimen's document is HTML, by the judgement of a parser this repository did not write.
///
/// Both the fresh render and the file on disk, which are different claims: the first is about the
/// renderer, the second is about the bytes a reader of the repository actually sees, and a
/// hand-edited asset is caught by this before anyone runs the example again.
#[test]
fn every_specimen_is_html_a_parser_that_is_not_ours_accepts() {
  for specimen in specimens::SPECIMENS {
    let diagnostic = (specimen.build)(specimen.text);
    for (what, document) in [
      (
        format!("{}, as the renderer writes it", specimen.name),
        rendered(specimen),
      ),
      (
        format!("{}, as committed", specimen.name),
        committed(specimen),
      ),
    ] {
      let parsed = certify(&what, &document);

      // And the caller's own strings survived as text. Without this the census above would be
      // satisfied by a renderer that emitted the right elements around nothing: an escaper that
      // dropped what it could not represent leaves a perfectly well-formed document saying less
      // than it was asked to.
      let message = diagnostic.message().to_string();
      assert!(
        parsed.text.contains(&message),
        "{what}: the message `{message}` is not in the document's text"
      );
      if let Some(label) = diagnostic.primary_label() {
        assert!(
          parsed.text.contains(label),
          "{what}: the primary label `{label}` is not in the document's text"
        );
      }
      if let Some(help) = diagnostic.help() {
        assert!(
          parsed.text.contains(help),
          "{what}: the help `{help}` is not in the document's text"
        );
      }
    }
  }
}

/// Nothing a caller supplies becomes structure, by the same parser's reading.
///
/// The complement of `tests/html_escaping.rs`, not a repeat of it. That file asserts the bytes —
/// that every `<` in the output is one painty wrote and every entity is the one the table names.
/// This asserts the consequence, in the only terms that matter to a browser: after a
/// specification-conforming parse, the caller's text is *text*, the element census is unchanged,
/// and there is no `<script>`, no `<style>` and no comment anywhere in the tree.
///
/// Both directions, over one document. A renderer that deleted the hostile characters instead of
/// escaping them would pass every safety assertion here and silently corrupt the diagnostic, so
/// the strings have to come back out verbatim as well as inertly.
#[test]
fn caller_text_that_looks_like_markup_comes_back_as_text() {
  let what = "every sink at once, with markup in the caller's text";
  let source = format!("{HOSTILE_LINE}\n");
  let labels = [Label::new(Location::new(0, Span::new(0, 3)), HOSTILE)];
  let document = render(
    &Diagnostic::new(
      HOSTILE,
      Severity::Error,
      &HOSTILE,
      Location::new(0, Span::new(8, 13)),
    )
    .with_primary_label(HOSTILE)
    .with_labels(&labels)
    .with_help(HOSTILE),
    &[Input::new(Source::new(&source)).with_origin(HOSTILE)],
  );

  let parsed = certify(what, &document);

  // Six sinks say it — the code, the message, the origin, the primary label, the secondary label
  // and the help — and the COUNT is asserted rather than the presence, because `contains` is
  // satisfied by one survivor and there is no reason to believe the sink that survived is the sink
  // a future defect reaches.
  let sinks = parsed.text.matches(HOSTILE).count();
  assert_eq!(
    sinks, 6,
    "{what}: the hostile string came back out of {sinks} sinks, not the six that were asked to \
     write it"
  );
  assert!(
    parsed.text.contains(HOSTILE_LINE),
    "{what}: the source line did not come back out of the document as text"
  );
}

/// What a document turned out to be made of, once somebody else had read it.
struct Certified {
  /// Every text node in document order, concatenated, with character references decoded — by the
  /// parser, from the specification's table, not by anything in this repository.
  text: String,
  /// How many elements the tree holds, so an assertion over an empty tree cannot pass.
  elements: usize,
}

/// Parses `document` as an HTML fragment and asserts that html5ever had nothing to repair and
/// nothing to build that painty did not write.
///
/// A `body` fragment rather than a whole document, because that is what painty emits and what an
/// embedder pastes in. Parsing it as a document would have the tree builder insert `<html>`,
/// `<head>` and `<body>` around it and report the missing doctype, which are the parser's
/// observations about a document painty never claimed to have written.
fn certify(what: &str, document: &str) -> Certified {
  let dom = parse_fragment(
    RcDom::default(),
    ParseOpts {
      // Both halves report through `TreeSink::parse_error` either way; this only decides whether
      // the message says which token, which is the difference between a useful failure and a
      // count.
      tokenizer: TokenizerOpts {
        exact_errors: true,
        ..TokenizerOpts::default()
      },
      tree_builder: TreeBuilderOpts {
        exact_errors: true,
        ..TreeBuilderOpts::default()
      },
    },
    QualName::new(None, ns!(html), local_name!("body")),
    Vec::new(),
    false,
  )
  .one(document.to_owned());

  let errors = dom.errors.borrow();
  assert!(
    errors.is_empty(),
    "{what}: html5ever had {} thing(s) to repair, so this is not the HTML it looks like: {:?}",
    errors.len(),
    errors
  );
  drop(errors);

  // `parse_fragment` hangs the fragment under an `<html>` element of its own making. That one is
  // the parser's, so it is stepped over rather than censused.
  let root = dom.document.children.borrow();
  let [html] = root.as_slice() else {
    panic!(
      "{what}: a parsed fragment has exactly one root, and this has {}",
      root.len()
    );
  };

  let mut certified = Certified {
    text: String::new(),
    elements: 0,
  };
  let mut roots = 0usize;
  for child in html.children.borrow().iter() {
    match &child.data {
      // The `\n` painty writes after the closing `</div>`.
      NodeData::Text { contents } if contents.borrow().trim().is_empty() => {}
      NodeData::Element { name, .. } => {
        roots += 1;
        assert_eq!(
          name.local.as_ref(),
          "div",
          "{what}: a document is one `<div class=\"painty ..\">` and this one is rooted in a \
           `<{}>`",
          name.local
        );
      }
      other => panic!("{what}: a {other:?} sits beside the diagnostic at the top of the document"),
    }
    walk(what, child, &mut certified);
  }
  assert_eq!(
    roots, 1,
    "{what}: a document is one diagnostic, and this one holds {roots}"
  );
  assert!(
    certified.elements > 0,
    "{what}: no elements, so this assertion checked nothing"
  );
  certified
}

/// The census, over one node and everything under it.
fn walk(what: &str, node: &Handle, out: &mut Certified) {
  match &node.data {
    NodeData::Text { contents } => out.text.push_str(&contents.borrow()),
    NodeData::Element { name, attrs, .. } => {
      out.elements += 1;
      let element = name.local.as_ref();
      assert!(
        ELEMENTS.contains(&element),
        "{what}: `<{element}>` is not an element painty writes, so the caller's text became \
         structure"
      );
      let attrs = attrs.borrow();
      let [class] = attrs.as_slice() else {
        panic!(
          "{what}: `<{element}>` carries {} attributes, and painty writes exactly one",
          attrs.len()
        );
      };
      assert_eq!(
        class.name.local.as_ref(),
        "class",
        "{what}: `<{element}>` carries a `{}` attribute, and painty writes only `class`",
        class.name.local
      );
      for token in class.value.split(' ') {
        assert!(
          token.starts_with("painty"),
          "{what}: `{token}` on a `<{element}>` is not one of painty's own class names"
        );
      }
    }
    // painty writes none of these, so any of them is caller text that escaped. A comment is the
    // one worth naming: `<!-- -->` is the shape a `>` in a caller's string most easily reaches.
    other => panic!("{what}: the document holds a {other:?}, and painty writes no such thing"),
  }

  for child in node.children.borrow().iter() {
    walk(what, child, out);
  }
}
