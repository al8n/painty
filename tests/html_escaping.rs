//! Nothing a caller supplies can become markup.
//!
//! # Why this reads the document with a parser and not with `contains`
//!
//! An escaping test that greps the output for `<script` passes the moment the renderer escapes the
//! one string the test author thought of. The thing worth guaranteeing is the complement — that
//! **no** byte of caller text is markup, at **any** of the sinks — and a complement cannot be
//! spot-checked. So the document is tokenised against the finite set of tags painty writes, and
//! every byte that is not one of them has to be text; a raw `<`, `>`, `&`, `"` or `'` outside a tag
//! is a failure wherever it turns up, whether or not the corpus was aiming at that sink.
//!
//! The second half is the one a "does it contain a `<`" test cannot state at all: the text that
//! comes back out, **unescaped, has to be the caller's text verbatim**. An escaper that dropped the
//! hostile characters, or replaced them with `?`, would satisfy every safety assertion here and
//! silently corrupt the diagnostic. Both directions are asserted over the same corpus.
//!
//! # This test is only as good as its ability to fail
//!
//! Each escaping sink in `src/html/mod.rs` was planted in turn — `self.text(..)` replaced by
//! `self.markup(..)`, and `display` by a direct `write_fmt` — and this file was watched failing for
//! every one of them before any of it was believed. Reading the output and finding it correct is
//! not evidence: it is the same author checking the same assumption twice.

#![cfg(feature = "html")]

use core::fmt;

use painty::{Diagnostic, Input, Label, Location, Severity, Source, Span, html::Html};

/// Every character that would leave text and become markup, and the entity painty writes for it.
const ENTITIES: [(char, &str); 5] = [
  ('&', "&amp;"),
  ('<', "&lt;"),
  ('>', "&gt;"),
  ('"', "&quot;"),
  ('\'', "&#39;"),
];

/// Every element painty writes. Anything else in the document is a defect by construction.
const ELEMENTS: [&str; 5] = ["div", "p", "pre", "section", "span"];

/// A string carrying every character this file is about, plus the literal closing tag the design
/// names, plus a payload that would run if any of it escaped.
const HOSTILE: &str = r#"</span><script>alert("x" & 'y')</script><!-- > -->"#;

/// One line of source made of the same characters, for the cases whose span draws one row.
const HOSTILE_LINE: &str = r#"let a = "<b>" & '</span>' > x; // <!-- -->"#;

/// A message whose own `Display` writes the hostile text in fragments.
///
/// The point is the fragmentation. A message is a `&dyn Display`, so its text reaches the document
/// through `write_str` calls painty does not choose the boundaries of — and an escaper that
/// buffered, or that only looked at whole arguments, would let a `<` written on its own through.
struct Fragmented;

impl fmt::Display for Fragmented {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for character in HOSTILE.chars() {
      write!(f, "{character}")?;
    }
    f.write_str(" and then some")
  }
}

/// One thing painty was asked to render, and what has to come back out of it intact.
struct Case {
  what: &'static str,
  document: String,
  /// Every caller string that has to survive the round trip verbatim.
  intact: Vec<String>,
}

fn render(diagnostic: &Diagnostic<'_>, inputs: &[Input<'_>]) -> String {
  let mut out = String::new();
  Html::new()
    .render(diagnostic, inputs, &mut out)
    .expect("a `String` accepts everything");
  out
}

/// The corpus: every sink a caller's bytes can reach, one case each, and then the shapes that
/// reach several at once.
fn corpus() -> Vec<Case> {
  let mut cases = Vec::new();
  let plain = "fn main() {}\n";
  let message = HOSTILE;

  // The source text, on the three paths a row is written by: before the covered run, inside it,
  // and after it.
  let one_line = format!("{HOSTILE_LINE}\n");
  cases.push(Case {
    what: "source text, with the span inside a hostile line",
    document: render(
      &Diagnostic::new(
        "code",
        Severity::Error,
        &"plain",
        Location::new(0, Span::new(8, 13)),
      )
      .with_primary_label("plain"),
      &[Input::new(Source::new(&one_line))],
    ),
    intact: vec![HOSTILE_LINE.to_owned()],
  });

  // A zero-width span in a hostile line: the covered element is empty and the whole line is on the
  // two sides of it.
  cases.push(Case {
    what: "source text, around a zero-width span",
    document: render(
      &Diagnostic::new(
        "code",
        Severity::Error,
        &"plain",
        Location::new(0, Span::empty(9)),
      ),
      &[Input::new(Source::new(&one_line))],
    ),
    intact: vec![HOSTILE_LINE.to_owned()],
  });

  // A multi-line span, so the opening row, the rows between the ends and the closing row are all
  // written. Long enough to elide, so the rows after the gap are written too.
  let hostile_source = format!("{HOSTILE_LINE}\n").repeat(9);
  cases.push(Case {
    what: "source text, across an elided multi-line span",
    document: render(
      &Diagnostic::new(
        "code",
        Severity::Error,
        &"plain",
        Location::new(0, Span::new(0, hostile_source.len())),
      )
      .with_primary_label("plain"),
      &[Input::new(Source::new(&hostile_source))],
    ),
    intact: vec![HOSTILE_LINE.to_owned()],
  });

  // The code, the message, the origin, the label and the help, each on its own so that a failure
  // names the sink.
  cases.push(Case {
    what: "the code",
    document: render(
      &Diagnostic::new(
        HOSTILE,
        Severity::Error,
        &"plain",
        Location::new(0, Span::new(0, 2)),
      ),
      &[Input::new(Source::new(plain))],
    ),
    intact: vec![HOSTILE.to_owned()],
  });

  cases.push(Case {
    what: "the message",
    document: render(
      &Diagnostic::new(
        "code",
        Severity::Error,
        &message,
        Location::new(0, Span::new(0, 2)),
      ),
      &[Input::new(Source::new(plain))],
    ),
    intact: vec![HOSTILE.to_owned()],
  });

  cases.push(Case {
    what: "the message, written in fragments by its own Display",
    document: render(
      &Diagnostic::new(
        "code",
        Severity::Error,
        &Fragmented,
        Location::new(0, Span::new(0, 2)),
      ),
      &[Input::new(Source::new(plain))],
    ),
    intact: vec![format!("{HOSTILE} and then some")],
  });

  cases.push(Case {
    what: "the origin",
    document: render(
      &Diagnostic::new(
        "code",
        Severity::Error,
        &"plain",
        Location::new(0, Span::new(0, 2)),
      ),
      &[Input::new(Source::new(plain)).with_origin(HOSTILE)],
    ),
    intact: vec![HOSTILE.to_owned()],
  });

  cases.push(Case {
    what: "the primary label",
    document: render(
      &Diagnostic::new(
        "code",
        Severity::Error,
        &"plain",
        Location::new(0, Span::new(0, 2)),
      )
      .with_primary_label(HOSTILE),
      &[Input::new(Source::new(plain))],
    ),
    intact: vec![HOSTILE.to_owned()],
  });

  let labels = [Label::new(Location::new(0, Span::new(3, 7)), HOSTILE)];
  cases.push(Case {
    what: "a secondary label",
    document: render(
      &Diagnostic::new(
        "code",
        Severity::Error,
        &"plain",
        Location::new(0, Span::new(0, 2)),
      )
      .with_labels(&labels),
      &[Input::new(Source::new(plain))],
    ),
    intact: vec![HOSTILE.to_owned()],
  });

  cases.push(Case {
    what: "the help",
    document: render(
      &Diagnostic::new(
        "code",
        Severity::Error,
        &"plain",
        Location::new(0, Span::new(0, 2)),
      )
      .with_help(HOSTILE),
      &[Input::new(Source::new(plain))],
    ),
    intact: vec![HOSTILE.to_owned()],
  });

  // Everything at once, over two inputs, so no sink is left to a case of its own.
  let both = [Label::new(Location::new(1, Span::new(4, 12)), HOSTILE)];
  cases.push(Case {
    what: "every sink at once, over two inputs",
    document: render(
      &Diagnostic::new(
        HOSTILE,
        Severity::Advice,
        &Fragmented,
        Location::new(0, Span::new(0, HOSTILE_LINE.len())),
      )
      .with_primary_label(HOSTILE)
      .with_labels(&both)
      .with_help(HOSTILE),
      &[
        Input::new(Source::new(&one_line)).with_origin(HOSTILE),
        Input::new(Source::new(&one_line)).with_origin(HOSTILE),
      ],
    ),
    intact: vec![
      HOSTILE.to_owned(),
      HOSTILE_LINE.to_owned(),
      format!("{HOSTILE} and then some"),
    ],
  });

  cases
}

/// What the document turns out to be made of.
#[derive(Debug, Default)]
struct Parsed {
  /// Every text run, concatenated, with the entities decoded back to what they stand for.
  text: String,
  /// The elements that were opened and closed, innermost last while they are open.
  opened: Vec<String>,
}

/// Tokenises `document` against the tags painty writes, failing on anything else.
///
/// Deliberately intolerant. A real HTML parser recovers from a stray `<`, which is the one thing
/// that must not be recovered from here.
fn parse(what: &str, document: &str) -> Parsed {
  let mut out = Parsed::default();
  let mut stack: Vec<&str> = Vec::new();
  let bytes = document.as_bytes();
  let mut at = 0;

  while at < document.len() {
    match bytes[at] {
      b'<' => {
        let end = document[at..]
          .find('>')
          .unwrap_or_else(|| panic!("{what}: a `<` at {at} that never closes"))
          + at;
        let tag = &document[at + 1..end];
        at = end + 1;
        if let Some(name) = tag.strip_prefix('/') {
          assert!(
            ELEMENTS.contains(&name),
            "{what}: `</{name}>` is not an element painty writes"
          );
          let open = stack
            .pop()
            .unwrap_or_else(|| panic!("{what}: `</{name}>` closes nothing"));
          assert_eq!(open, name, "{what}: `</{name}>` closes a `<{open}>`");
          out.opened.push(name.to_owned());
        } else {
          let (name, attributes) = tag
            .split_once(' ')
            .unwrap_or_else(|| panic!("{what}: `<{tag}>` carries no class"));
          assert!(
            ELEMENTS.contains(&name),
            "{what}: `<{name}>` is not an element painty writes"
          );
          let classes = attributes
            .strip_prefix("class=\"")
            .and_then(|rest| rest.strip_suffix('"'))
            .unwrap_or_else(|| {
              panic!("{what}: `<{tag}>` is not `<{name} class=\"..\">` and nothing else")
            });
          for class in classes.split(' ') {
            assert!(
              !class.is_empty()
                && class
                  .bytes()
                  .all(|byte| byte.is_ascii_lowercase() || byte == b'-'),
              "{what}: `{class}` is not one of painty's own class names"
            );
          }
          stack.push(name);
        }
      }
      b'&' => {
        let (character, entity) = ENTITIES
          .iter()
          .find(|(_, entity)| document[at..].starts_with(entity))
          .unwrap_or_else(|| {
            panic!(
              "{what}: an `&` at {at} that is not one of painty's entities: {:?}",
              &document[at..document.len().min(at + 12)]
            )
          });
        out.text.push(*character);
        at += entity.len();
      }
      raw @ (b'>' | b'"' | b'\'') => panic!(
        "{what}: a raw `{}` in text at {at}: {:?}",
        raw as char,
        &document[at.saturating_sub(24)..document.len().min(at + 24)]
      ),
      _ => {
        let run = document[at..]
          .find(['<', '&', '>', '"', '\''])
          .unwrap_or(document.len() - at);
        out.text.push_str(&document[at..at + run]);
        at += run;
      }
    }
  }

  assert!(
    stack.is_empty(),
    "{what}: {stack:?} was opened and never closed"
  );
  out
}

#[test]
fn no_byte_of_a_callers_text_becomes_markup() {
  for case in corpus() {
    let parsed = parse(case.what, &case.document);
    assert!(
      !parsed.opened.is_empty(),
      "{}: nothing was rendered at all",
      case.what
    );
  }
}

#[test]
fn every_hostile_string_comes_back_out_of_the_document_verbatim() {
  for case in corpus() {
    let parsed = parse(case.what, &case.document);
    for intact in &case.intact {
      // The source arrives split across rows, so the line breaks are the one thing the document
      // does not carry between them. Every other byte has to be there, in order.
      for fragment in intact.split('\n').filter(|line| !line.is_empty()) {
        assert!(
          parsed.text.contains(fragment),
          "{}: {fragment:?} did not survive the round trip\n--- decoded ---\n{}\n--- document \
           ---\n{}",
          case.what,
          parsed.text,
          case.document
        );
      }
    }
  }
}

#[test]
fn the_corpus_actually_contains_what_it_claims_to() {
  // A corpus that had lost its hostile characters would make every assertion above vacuous, and
  // nothing else here would notice. Checked against the same table the parser uses.
  for (character, _) in ENTITIES {
    assert!(
      HOSTILE.contains(character),
      "the corpus string does not contain {character:?}"
    );
  }
  assert!(HOSTILE.contains("</span>"), "the design names this one");

  // And the documents have to be reaching the paths that escape. A case whose document held no
  // entity at all would pass the parser trivially.
  for case in corpus() {
    assert!(
      ENTITIES
        .iter()
        .any(|(_, entity)| case.document.contains(entity)),
      "{}: the document contains no escaped character, so nothing was tested",
      case.what
    );
  }
}
