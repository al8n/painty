//! What the two outputs have to agree about, and the axes that agreement is checked over.
//!
//! # Why this is asserted rather than structural
//!
//! HTML is a **peer** of the terminal renderer rather than a style of it: it implements no shared
//! trait, and every hook the styles are written against that positions or draws a mark is
//! denominated in display cells, which HTML has none of. So there is no type keeping the two
//! honest. What they do share is layer 2 and, since the round this file was rewritten in,
//! `crate::elide` — the rule for how many lines are left out between two drawn ones.
//!
//! # THE AXES, WRITTEN DOWN RATHER THAN THE CASES
//!
//! The first version of this file held two properties, each complete along one axis and blind to
//! the other, and **the defect lived in their product**:
//!
//! * `the_two_renderers_elide_the_same_spans` ranged over **span length**, 1..12, with exactly ONE
//!   mark. Every value passed.
//! * `the_two_renderers_draw_the_same_lines` ranged over **two marks**, both single-line. It passed
//!   too.
//!
//! A seven-line span *with a second mark inside it* needs a value on both axes at once, and neither
//! property ever took one: HTML elided lines five and six, drew line five in another mark's excerpt,
//! and **line six appeared in no output at all** while the terminal drew every line with no gap.
//! That is the third time on this branch that a derived case set was complete along one axis and
//! blind to another, so the fix is the axes and not another case:
//!
//! | axis | values |
//! |---|---|
//! | how many lines the first span covers | 1 ..= 12 — both sides of the six/seven turn |
//! | where a second mark sits | absent, or opening on each of lines 1 ..= 12 |
//! | how long the second mark is | one line, or reaching three lines further |
//! | how many marks | one, two, or three |
//!
//! [`the_two_renderers_draw_the_same_rows`] takes the product. Anything added here should extend a
//! row of that table rather than append a case below it.
//!
//! # What they are free to disagree about
//!
//! Everything that is not *which source a reader is shown*: the terminal draws a caret under a cell
//! and HTML wraps an element round bytes; the terminal cuts a row at its width ceiling and HTML lets
//! a browser scroll; the terminal marks only a multi-line span's two ends and HTML shades every line
//! it covers.

#![cfg(all(feature = "html", feature = "terminal"))]

use painty::{
  Diagnostic, Input, Label, Location, Severity, Source, Span,
  html::Html,
  terminal::{Input as TerminalInput, Terminal},
};

fn as_html(diagnostic: &Diagnostic<'_>, text: &str) -> String {
  let mut out = String::new();
  Html::new()
    .render(
      diagnostic,
      &[Input::new(Source::new(text)).with_origin("input")],
      &mut out,
    )
    .expect("a `String` accepts everything");
  out
}

fn as_terminal(diagnostic: &Diagnostic<'_>, text: &str) -> String {
  let mut out = String::new();
  Terminal::plain()
    .render(
      diagnostic,
      &[TerminalInput::new(Source::new(text)).with_origin("input")],
      &mut out,
    )
    .expect("a `String` accepts everything");
  out
}

/// The rows an HTML render drew, in order: a line number, or `...` for a gap.
///
/// Read off the class the row's number cell carries, so it says WHICH lines were drawn without
/// saying anything about how. A label row's number cell is empty, exactly as a terminal marker
/// row's number field is blank, and neither is a source row.
fn html_rows(document: &str) -> Vec<String> {
  const OPEN: &str = r#"<span class="painty-gutter painty-number">"#;
  document
    .match_indices(OPEN)
    .filter_map(|(at, _)| {
      let from = at + OPEN.len();
      let to = from
        + document[from..]
          .find('<')
          .expect("a number cell that closes");
      match &document[from..to] {
        "" => None,
        "\u{22ee}" => Some("...".to_owned()),
        number => Some(number.to_owned()),
      }
    })
    .collect()
}

/// The rows a terminal render drew, in order, in the same vocabulary.
///
/// A source row carries a number before the wall and a marker row carries a blank; the gap row
/// carries `...` and **no wall at all**, which is the rustc style saying that the numbers are what
/// is missing. Reading only the lines with a wall in them dropped every gap and made the grid below
/// pass over the first divergence it was pointed at — the helper is part of the oracle.
fn terminal_rows(rendered: &str) -> Vec<String> {
  rendered
    .lines()
    .filter_map(|line| match line.split_once('|') {
      Some((field, _)) => {
        let field = field.trim();
        (!field.is_empty()).then(|| field.to_owned())
      }
      None => (line.trim() == "...").then(|| "...".to_owned()),
    })
    .collect()
}

/// One point of the grid: a source, a primary span and up to two labels over it.
fn source_of(lines: usize) -> String {
  // Two bytes of content and a break, so line `n` starts at `3 * (n - 1)`.
  (1..=lines).map(|n| format!("{:02}\n", n % 100)).collect()
}

fn start_of(line: usize) -> usize {
  3 * (line - 1)
}

fn end_of(line: usize) -> usize {
  start_of(line) + 2
}

#[test]
#[cfg_attr(
  miri,
  ignore = "six hundred and twenty-four points, half of them a terminal render, measured at 402s a \
            cell — and the question is whether two outputs agree about a line number, which the \
            interpreter is not an instrument for. The renderers' own paths are interpreted by the \
            two tests below and by the whole of `src/`'s unit suite."
)]
fn the_two_renderers_draw_the_same_rows() {
  const LINES: usize = 20;
  let text = source_of(LINES);
  let message = "the same rows either way";

  // Every point is checked and every failure is reported, rather than stopping at the first. A
  // grid exists to say WHICH region of it is wrong — "the first two-mark case" and "every case with
  // a mark inside a multi-line span" are different diagnoses, and an assertion that stops cannot
  // tell them apart.
  let mut checked = 0usize;
  let mut failed: Vec<String> = Vec::new();
  for covers in 1..=12usize {
    let primary = Span::new(0, end_of(covers));

    for second in std::iter::once(None).chain((1..=12usize).map(Some)) {
      for reaches in [0usize, 3] {
        for third in [None, Some(2usize)] {
          let mut labels: Vec<Label<'_>> = Vec::new();
          if let Some(second) = second {
            let last = (second + reaches).min(LINES);
            labels.push(Label::new(
              Location::new(0, Span::new(start_of(second), end_of(last))),
              "second",
            ));
          }
          if let Some(third) = third {
            labels.push(Label::new(
              Location::new(0, Span::new(start_of(third), end_of(third))),
              "third",
            ));
          }

          let diagnostic =
            Diagnostic::new("code", Severity::Error, &message, Location::new(0, primary))
              .with_primary_label("first")
              .with_labels(&labels);

          let html = as_html(&diagnostic, &text);
          let terminal = as_terminal(&diagnostic, &text);
          checked += 1;

          let (drawn, expected) = (html_rows(&html), terminal_rows(&terminal));

          // No line twice. This is the anti-amplification claim stated rather than inferred: it is
          // what keeps the emitted source bytes at one copy of the lines drawn, and it is what
          // sixty-four labels on one line used to break — sixty-four copies of that line, against
          // the terminal's one.
          //
          // Over the LINES only. A block can hold two gaps — two spans far apart in one input is
          // the ordinary way to get one — and counting the marker as a repeated row said so for
          // sixty-six points of this grid before the difference was noticed.
          let mut lines: Vec<&String> = drawn.iter().filter(|row| *row != "...").collect();
          let total = lines.len();
          lines.sort();
          lines.dedup();
          if lines.len() != total {
            failed.push(format!(
              "covers={covers} second={second:?} reaches={reaches} third={third:?}: html drew a \
               line twice, {drawn:?}"
            ));
          }

          if drawn != expected {
            failed.push(format!(
              "covers={covers} second={second:?} reaches={reaches} third={third:?}: html {drawn:?} \
               against terminal {expected:?}"
            ));
          }
        }
      }
    }
  }

  // The grid is the claim, so an empty or shrunken one has to fail rather than pass silently.
  assert_eq!(
    checked,
    12 * 13 * 2 * 2,
    "the grid did not take every value"
  );
  assert!(
    failed.is_empty(),
    "{} of {checked} points draw different rows:\n{}",
    failed.len(),
    failed.join("\n")
  );
}

#[test]
fn which_source_is_marked_is_the_same_in_both_outputs() {
  let text = "let value = compute(1, 2);\nlet other = value + 1;\n";
  let primary = Span::new(12, 19);
  let secondary = Span::new(39, 44);
  let message = "two marks, two outputs";
  let labels = [Label::new(Location::new(0, secondary), "and this")];
  let diagnostic = Diagnostic::new("code", Severity::Error, &message, Location::new(0, primary))
    .with_primary_label("this")
    .with_labels(&labels);

  let html = as_html(&diagnostic, text);

  // What HTML says is marked: the contents of every `painty-covered` element.
  const OPEN: &str = r#"<span class="painty-covered painty-"#;
  let marked: Vec<&str> = html
    .match_indices(OPEN)
    .map(|(at, _)| {
      let from = at + html[at..].find(r#"">"#).expect("a covered element opens") + 2;
      let to = from + html[from..].find('<').expect("a covered element closes");
      &html[from..to]
    })
    .collect();

  // Derived from the diagnostic rather than written out, so the oracle is the caller's spans and
  // not a transcription of one output.
  let expected = [
    &text[primary.start()..primary.end()],
    &text[secondary.start()..secondary.end()],
  ];
  assert_eq!(marked, expected, "\n{html}");

  // And the terminal marks the same source. Its markers are cells rather than bytes, so what is
  // comparable is their length over an ASCII line: one cell per character, primary then secondary.
  let terminal = as_terminal(&diagnostic, text);
  assert!(
    terminal.contains(&"^".repeat(expected[0].chars().count())),
    "\n{terminal}"
  );
  assert!(
    terminal.contains(&"-".repeat(expected[1].chars().count())),
    "\n{terminal}"
  );
}

#[test]
fn the_two_renderers_say_the_same_labels_in_the_same_order() {
  // A label is said where its span closes, and the labels under one line are in the caller's order.
  // Both are the terminal's rules; nothing but this holds HTML to them.
  let text = source_of(9);
  let message = "three of them";
  let labels = [
    Label::new(
      Location::new(0, Span::new(start_of(3), end_of(3))),
      "on three",
    ),
    Label::new(
      Location::new(0, Span::new(start_of(1), end_of(3))),
      "one to three",
    ),
  ];
  let diagnostic = Diagnostic::new(
    "code",
    Severity::Error,
    &message,
    Location::new(0, Span::new(start_of(3), end_of(3))),
  )
  .with_primary_label("primary on three")
  .with_labels(&labels);

  let html = as_html(&diagnostic, &text);
  let terminal = as_terminal(&diagnostic, &text);

  let order = ["primary on three", "on three", "one to three"];
  for pair in order.windows(2) {
    let (before, after) = (pair[0], pair[1]);
    assert!(
      html.find(before) < html.find(after),
      "{before:?} should precede {after:?}\n{html}"
    );
    assert!(
      terminal.find(before) < terminal.find(after),
      "{before:?} should precede {after:?}\n{terminal}"
    );
  }
}
