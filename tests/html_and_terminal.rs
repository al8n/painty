//! What the two outputs have to agree about, and what they are free to disagree about.
//!
//! # Why these are the two claims worth holding
//!
//! HTML is a **peer** of the terminal renderer rather than a style of it: it implements no shared
//! trait, and every hook the styles are written against that positions or draws a mark is
//! denominated in display cells, which HTML has none of. So there is no type keeping the two
//! honest, and the properties that would otherwise be structural have to be asserted.
//!
//! Two of them are load-bearing, and they are the two the terminal's own cross-style property
//! covers between styles:
//!
//! 1. **Which source is marked is a function of the diagnostic, not of the output.** A reader shown
//!    an HTML rendering and a terminal rendering of one diagnostic must be shown the same bytes as
//!    the thing that is wrong. This is the one a shared plan would have made free, and there is no
//!    shared plan — the terminal's is private to its module and is four `Vec`s that a renderer
//!    which builds for `thumbv6m-none-eabi` cannot have.
//! 2. **Which lines are left out is the same decision.** The elision rule is stated twice, once per
//!    renderer, for the same reason. Two copies of a rule diverge silently; this is what makes that
//!    a failing test instead.
//!
//! What they are free to disagree about is everything else, and the disagreements are the point of
//! having two outputs at all: the terminal draws a caret under a cell and HTML wraps an element
//! around bytes; the terminal cuts a row at its width ceiling and HTML lets a browser scroll; the
//! terminal says a label on a row of its own and HTML hangs it off the mark's own element.

#![cfg(all(feature = "html", feature = "terminal"))]

use painty::{
  Diagnostic, Input, Label, Location, Severity, Source, Span,
  html::Html,
  terminal::{Input as TerminalInput, Terminal},
};

/// The inputs both renderers are given.
fn inputs<'a>(texts: &'a [&'a str]) -> Vec<Input<'a>> {
  texts
    .iter()
    .map(|text| Input::new(Source::new(text)).with_origin("input"))
    .collect()
}

fn as_html(diagnostic: &Diagnostic<'_>, inputs: &[Input<'_>]) -> String {
  let mut out = String::new();
  Html::new()
    .render(diagnostic, inputs, &mut out)
    .expect("a `String` accepts everything");
  out
}

fn as_terminal(diagnostic: &Diagnostic<'_>, inputs: &[TerminalInput<'_>]) -> String {
  let mut out = String::new();
  Terminal::plain()
    .render(diagnostic, inputs, &mut out)
    .expect("a `String` accepts everything");
  out
}

/// Every line number an HTML render drew, in the order it drew them.
///
/// Read off the class the row's number cell carries rather than off the layout, so it says which
/// lines were drawn without saying anything about how.
fn html_lines(document: &str) -> Vec<String> {
  const OPEN: &str = r#"<span class="painty-gutter painty-number">"#;
  document
    .match_indices(OPEN)
    .map(|(at, _)| {
      let from = at + OPEN.len();
      let to = from
        + document[from..]
          .find('<')
          .expect("a number cell that closes");
      document[from..to].to_owned()
    })
    .collect()
}

/// Every line number a terminal render drew, in the order it drew them.
///
/// A source row is the one that begins with a number before the wall; a marker row's field is
/// blank, and the elision row's is `...`.
fn terminal_lines(row: &str) -> Vec<String> {
  row
    .lines()
    .filter_map(|line| {
      let (field, _) = line.split_once('|')?;
      let field = field.trim();
      (!field.is_empty()).then(|| field.to_owned())
    })
    .collect()
}

/// The same SET of lines, which is the strongest form this claim has.
///
/// Not the same sequence: HTML gives every mark its own excerpt, so a line carrying two labels is
/// drawn twice there and once in the terminal. That difference is a layout choice and is documented
/// as one; which lines a reader is shown is not, and it is what this holds.
#[test]
fn the_two_renderers_draw_the_same_lines() {
  let text = "alpha\nbeta\ngamma\ndelta\nepsilon\n";
  let message = "the same lines either way";
  let labels = [Label::new(Location::new(0, Span::new(0, 5)), "and here")];
  let diagnostic = Diagnostic::new(
    "code",
    Severity::Error,
    &message,
    Location::new(0, Span::new(23, 28)),
  )
  .with_primary_label("here")
  .with_labels(&labels);

  let texts = [text];
  let html = as_html(&diagnostic, &inputs(&texts));
  let terminal = as_terminal(&diagnostic, &[TerminalInput::new(Source::new(text))]);

  let mut drawn = html_lines(&html);
  drawn.sort();
  drawn.dedup();
  let mut expected = terminal_lines(&terminal);
  expected.sort();
  expected.dedup();

  assert_eq!(drawn, expected, "\n{html}\n{terminal}");
}

#[test]
fn the_two_renderers_elide_the_same_spans() {
  // The rule turns over between six lines and seven: at six there is one line left, which reads
  // worse as a gap than as the line itself. Both sides of the turn, both renderers.
  let text = "x\n".repeat(20);

  for lines in 1..=12usize {
    // A span from the start of line 1 to the last byte of line `lines`. Each line is `x\n`.
    let end = lines * 2 - 1;
    let message = "as far as it goes";
    let diagnostic = Diagnostic::new(
      "code",
      Severity::Error,
      &message,
      Location::new(0, Span::new(0, end)),
    )
    .with_primary_label("here");

    let texts = [text.as_str()];
    let html = as_html(&diagnostic, &inputs(&texts));
    let terminal = as_terminal(&diagnostic, &[TerminalInput::new(Source::new(&text))]);

    let elided_in_html = html.contains("painty-elision");
    let elided_in_terminal = terminal.contains("...");
    assert_eq!(
      elided_in_html, elided_in_terminal,
      "a span of {lines} lines is elided in one output and not the other\n{html}\n{terminal}"
    );

    // The same rows, in the same order, with each renderer's own mark for the gap.
    let expected: Vec<String> = terminal_lines(&terminal)
      .into_iter()
      .map(|field| {
        if field == "..." {
          "\u{22ee}".to_owned()
        } else {
          field
        }
      })
      .collect();
    assert_eq!(
      html_lines(&html),
      expected,
      "a span of {lines} lines draws different rows\n{html}\n{terminal}"
    );
  }
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

  let texts = [text];
  let html = as_html(&diagnostic, &inputs(&texts));

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
  let terminal = as_terminal(&diagnostic, &[TerminalInput::new(Source::new(text))]);
  assert!(
    terminal.contains(&"^".repeat(expected[0].chars().count())),
    "\n{terminal}"
  );
  assert!(
    terminal.contains(&"-".repeat(expected[1].chars().count())),
    "\n{terminal}"
  );
}
