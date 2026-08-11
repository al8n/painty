use core::fmt::Write as _;

use super::{
  Html,
  escape::{Escaped, entity_of, escaped},
};
use crate::{Diagnostic, Input, Location, Severity, Source, Span};

/// The harness is a `std` program whatever the crate under it is, so the string these render into
/// comes from there rather than from `alloc`, which this crate does not enable.
use std::string::String;

fn escape(text: &str) -> String {
  let mut out = String::new();
  Escaped(&mut out)
    .write_str(text)
    .expect("a `String` accepts everything");
  out
}

#[test]
fn every_escaped_character_has_an_entity_and_escaping_is_idempotent_in_meaning() {
  for character in escaped() {
    let entity = entity_of(character).expect("a character on the escape list has an entity");
    assert!(
      entity.starts_with('&') && entity.ends_with(';'),
      "{character:?} is written as {entity:?}, which is not a character reference"
    );
    // Every entity begins with `&`, so escaping one again has to escape that `&` and nothing else.
    // If it did not, an already-escaped run would round-trip and the escaper would be losing text.
    let twice = escape(entity);
    assert_eq!(
      twice,
      std::format!("&amp;{}", &entity[1..]),
      "escaping {entity:?} again did not escape its own ampersand"
    );
  }
}

#[test]
fn nothing_else_is_touched() {
  // A control character is not markup in HTML, so it is passed through where the terminal renderer
  // substitutes a picture for it. The tab matters most: it is the one a `<pre>` renders.
  let text = "a\tb\u{1b}[31mc\u{0}d\u{9b}e";
  assert_eq!(escape(text), text);
}

#[test]
fn a_run_with_nothing_to_escape_is_written_whole() {
  assert_eq!(escape(""), "");
  assert_eq!(escape("plain text"), "plain text");
}

#[test]
fn every_hostile_character_is_escaped_wherever_it_sits() {
  assert_eq!(escape("</span>&\"'<"), "&lt;/span&gt;&amp;&quot;&#39;&lt;");
  assert_eq!(escape("&lt;"), "&amp;lt;");
}

fn render(diagnostic: &Diagnostic<'_>, inputs: &[Input<'_>]) -> String {
  let mut out = String::new();
  Html::new()
    .render(diagnostic, inputs, &mut out)
    .expect("a `String` accepts everything");
  out
}

/// One primary position and nothing else. The message is taken by double reference so that the
/// returned diagnostic borrows for as long as the caller's own text does.
fn simple<'a>(span: Span, message: &'a &'a str) -> Diagnostic<'a> {
  Diagnostic::new("code", Severity::Error, message, Location::new(0, span))
    .with_primary_label("here")
}

#[test]
fn a_zero_width_span_is_an_empty_element_rather_than_a_widened_one() {
  let text = "let x = 1;\n";
  let message = "expected a value";
  let out = render(
    &simple(Span::empty(8), &message),
    &[Input::new(Source::new(text))],
  );
  assert!(
    out.contains(r#"<span class="painty-covered painty-primary"></span>"#),
    "{out}"
  );
}

#[test]
fn a_diagnostic_naming_nothing_drawable_still_renders_its_header_and_help() {
  let message = "the whole document is wrong";
  let diagnostic = Diagnostic::new("code", Severity::Warning, &message, Location::entire(0))
    .with_help("start again");
  let out = render(&diagnostic, &[Input::new(Source::new("anything"))]);

  assert!(
    out.contains(r#"<div class="painty painty-warning">"#),
    "{out}"
  );
  assert!(out.contains("the whole document is wrong"), "{out}");
  assert!(
    out.contains(r#"<p class="painty-help">start again</p>"#),
    "{out}"
  );
  assert!(!out.contains("painty-block"), "{out}");
}

#[test]
fn an_input_the_caller_did_not_supply_draws_no_block() {
  let message = "elsewhere";
  let diagnostic = Diagnostic::new(
    "code",
    Severity::Error,
    &message,
    Location::new(7, Span::new(0, 1)),
  );
  let out = render(&diagnostic, &[Input::new(Source::new("anything"))]);
  assert!(!out.contains("painty-block"), "{out}");
}

#[test]
fn the_block_reports_the_callers_earliest_position_and_not_the_topmost_row() {
  // The label opens on line 1 and the primary is on line 3. The block header names the primary,
  // because that is where the caller said the diagnostic is.
  let text = "one\ntwo\nthree\n";
  let message = "the third line";
  let labels = [crate::Label::new(
    Location::new(0, Span::new(0, 3)),
    "and the first",
  )];
  let diagnostic = Diagnostic::new(
    "code",
    Severity::Error,
    &message,
    Location::new(0, Span::new(8, 13)),
  )
  .with_labels(&labels);

  let out = render(&diagnostic, &[Input::new(Source::new(text))]);
  assert!(
    out.contains(r#"<span class="painty-position">3:1</span>"#),
    "{out}"
  );
  // The rows are still in source order inside the block: line 1 is drawn before line 3.
  let first = out.find(">one<").expect("line 1 is drawn");
  let third = out.find(">three<").expect("line 3 is drawn");
  assert!(first < third, "{out}");
}

#[test]
fn two_inputs_are_two_blocks_in_the_order_they_first_appear() {
  let first = "alpha\n";
  let second = "beta\n";
  let message = "in the second input";
  let labels = [crate::Label::new(
    Location::new(0, Span::new(0, 5)),
    "and the first",
  )];
  let diagnostic = Diagnostic::new(
    "code",
    Severity::Error,
    &message,
    Location::new(1, Span::new(0, 4)),
  )
  .with_labels(&labels);

  let out = render(
    &diagnostic,
    &[
      Input::new(Source::new(first)).with_origin("first.txt"),
      Input::new(Source::new(second)).with_origin("second.txt"),
    ],
  );
  let second_at = out.find("second.txt").expect("the second input is named");
  let first_at = out.find("first.txt").expect("the first input is named");
  assert!(
    second_at < first_at,
    "the primary's input opens the render: {out}"
  );
  assert_eq!(out.matches("painty-block").count(), 2, "{out}");
}

#[test]
fn a_long_span_is_elided_to_six_rows() {
  let mut text = String::new();
  for number in 1..=40 {
    std::writeln!(&mut text, "line {number}").expect("a `String` accepts everything");
  }
  let message = "all of it";
  let out = render(
    &simple(Span::new(0, text.len()), &message),
    &[Input::new(Source::new(&text))],
  );

  assert_eq!(out.matches(r#"class="painty-row""#).count(), 5, "{out}");
  assert_eq!(out.matches("painty-elision").count(), 1, "{out}");
  assert!(out.contains(">line 1<"), "{out}");
  assert!(out.contains(">line 4<"), "{out}");
  assert!(!out.contains(">line 5<"), "{out}");
}

#[test]
fn a_span_of_six_lines_is_drawn_whole() {
  let text = "1\n2\n3\n4\n5\n6\n7\n";
  let message = "six of them";
  // Lines 1 to 6: the span stops before the break that ends line 6.
  let out = render(
    &simple(Span::new(0, 11), &message),
    &[Input::new(Source::new(text))],
  );
  assert_eq!(out.matches(r#"class="painty-row""#).count(), 6, "{out}");
  assert!(!out.contains("painty-elision"), "{out}");
}

/// Sources chosen for the edges of the row writer: every line break, no trailing break, an empty
/// source, an astral character, a combining mark, a CRLF a span can be pointed into the middle of,
/// and a tab that a `<pre>` keeps and a terminal expands.
const AWKWARD: [&str; 8] = [
  "",
  "\n",
  "no trailing break",
  "crlf\r\nlines\r\n",
  "lone\rcarriage\rreturns",
  "\u{1f3a8} astral\nand more\n",
  "e\u{301}\u{301} combining\n",
  "\ttabbed\tline\n\tanother\n",
];

#[test]
#[cfg_attr(
  miri,
  ignore = "about two thousand renders, and the failure it is looking for is a panic on a slice at \
            a character boundary rather than undefined behaviour — so the interpreter is not the \
            instrument for it, and it costs four minutes a cell. Every path it walks is walked by \
            the twelve tests above, which Miri does interpret."
)]
fn every_span_over_every_awkward_source_renders_without_panicking() {
  // Every offset pair, not a chosen few: the slicing in `row` is the one place a byte offset is
  // turned into three `&str` slices, and an offset that is not a character boundary panics rather
  // than misrenders. Resolution is what guarantees it cannot happen, and this is what makes that
  // guarantee reach this renderer rather than only layer 2.
  for text in AWKWARD {
    for start in 0..=text.len() {
      for end in 0..=text.len() {
        let message = "totality";
        let out = render(
          &simple(Span::new(start, end), &message),
          &[Input::new(Source::new(text))],
        );
        assert!(
          out.starts_with(r#"<div class="painty painty-error">"#) && out.ends_with("</div>\n"),
          "{text:?} {start}..{end}: {out}"
        );
        // The covered element is always written, so every render reaches the slicing.
        assert!(out.contains("painty-covered"), "{text:?} {start}..{end}");
      }
    }
  }
}

#[test]
fn a_tab_is_left_alone_where_the_terminal_would_expand_it() {
  let text = "\tif x {\n";
  let message = "the browser decides what a tab is worth";
  let out = render(
    &simple(Span::new(1, 3), &message),
    &[Input::new(Source::new(text))],
  );
  assert!(out.contains("<span class=\"painty-text\">\t"), "{out}");
}
