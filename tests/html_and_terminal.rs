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
//!
//! The round after that one found a third: **a span of zero bytes** at the first byte of a line.
//! Neither of the two axes then present had a zero value — a span covered one line or more, and a
//! second mark was placed but never sized — and a caret at a line start is where the walk's two
//! kinds of stop collide. So the table grew rather than gaining a case, and it is the table that is
//! the claim:
//!
//! | axis | values |
//! |---|---|
//! | what the primary is | a caret at a line start, or a span covering 1 ..= 12 lines — both sides of the six/seven turn |
//! | where a second mark sits | absent, on each of lines 1 ..= 12, or at the position after the trailing break |
//! | how big that second mark is | zero bytes, one line, or reaching three lines further |
//! | how many marks | two, or three |
//! | what ends a line | `\n`, `\r\n`, `\r` |
//!
//! [`the_two_renderers_draw_the_same_rows`] takes the product — 3,120 points. Anything added here
//! should extend a row of that table rather than append a case below it.
//!
//! # And one place layer 2 is the oracle rather than the other renderer
//!
//! A cross-renderer property is **structurally blind to anything the two renderers share**: they
//! both call `crate::elide` now, so a change to that rule moves them together and this file stays
//! green. Proved by planting each of its clauses. The corollary is the one that matters here — for
//! a shape where a *third*, independent answer exists, the third answer is the oracle, and
//! [`a_caret_is_drawn_on_the_line_layer_two_puts_it_on`] uses `Region::lines` for exactly that.
//! It is also the only check on this file's terminal side that does not go through the HTML
//! renderer's agreement with it.
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

/// The three things that can end a line, so that every offset the walk treats specially is reached
/// with each of them.
const BREAKS: [&str; 3] = ["\n", "\r\n", "\r"];

/// A source of `lines` numbered lines, each two bytes of content and one break.
fn source_of(lines: usize, ending: &str) -> String {
  (1..=lines)
    .map(|number| format!("{:02}{ending}", number % 100))
    .collect()
}

/// Where line `number`'s content begins.
fn start_of(number: usize, ending: &str) -> usize {
  (2 + ending.len()) * (number - 1)
}

/// One past line `number`'s content.
fn end_of(number: usize, ending: &str) -> usize {
  start_of(number, ending) + 2
}

/// What the primary position is: the axis a zero-byte span is a value on.
#[derive(Debug, Clone, Copy)]
enum Primary {
  /// A caret at the first byte of line two — the offset immediately after a line break, which is
  /// where the walk's close semantics and its open semantics disagree on purpose.
  Caret,
  /// A span from the top of the input through line `n`.
  Lines(usize),
}

/// What a second mark is: where it opens, and how big it is.
#[derive(Debug, Clone, Copy)]
enum Extent {
  /// Zero bytes — a caret at that line's first byte.
  Caret,
  /// That line's content.
  Line,
  /// That line and three more.
  Reaching,
}

/// Where a second mark opens.
#[derive(Debug, Clone, Copy)]
enum Where {
  Line(usize),
  /// The position after the trailing break, which is the empty last line.
  AfterTheLastBreak,
}

/// Every second mark the grid takes, `None` first.
fn second_marks(lines: usize) -> Vec<Option<(Where, Extent)>> {
  let mut out = vec![None];
  let places = (1..=12usize)
    .map(Where::Line)
    .chain(core::iter::once(Where::AfterTheLastBreak));
  for place in places {
    for extent in [Extent::Caret, Extent::Line, Extent::Reaching] {
      out.push(Some((place, extent)));
    }
  }
  let _ = lines;
  out
}

fn span_of(place: Where, extent: Extent, lines: usize, ending: &str) -> Span {
  let at = match place {
    Where::Line(number) => start_of(number, ending),
    Where::AfterTheLastBreak => (2 + ending.len()) * lines,
  };
  match extent {
    Extent::Caret => Span::empty(at),
    Extent::Line => Span::new(at, at + 2),
    Extent::Reaching => {
      let last = match place {
        Where::Line(number) => (number + 3).min(lines),
        Where::AfterTheLastBreak => lines,
      };
      Span::new(at, end_of(last, ending).max(at))
    }
  }
}

/// Every point of the table in this file's header.
///
/// The rows drawn have to be the same sequence, gaps included, and no line may be drawn twice.
/// Every point is checked and every failure reported rather than stopping at the first: "the first
/// two-mark case is wrong" and "every case with a caret at a line start is wrong" are different
/// diagnoses, and a stopping assertion cannot tell them apart. Both rounds this file has caught a
/// defect in, it was the shape of the failing REGION that named the cause.
#[test]
fn the_two_renderers_draw_the_same_rows() {
  const LINES: usize = 20;

  let mut checked = 0usize;
  let mut failed: Vec<String> = Vec::new();
  for ending in BREAKS {
    let text = source_of(LINES, ending);
    let message = "the same rows either way";

    let primaries = core::iter::once(Primary::Caret).chain((1..=12usize).map(Primary::Lines));
    for primary in primaries {
      let span = match primary {
        Primary::Caret => Span::empty(start_of(2, ending)),
        Primary::Lines(covers) => Span::new(0, end_of(covers, ending)),
      };

      for second in second_marks(LINES) {
        for third in [None, Some(2usize)] {
          let mut labels: Vec<Label<'_>> = Vec::new();
          if let Some((place, extent)) = second {
            labels.push(Label::new(
              Location::new(0, span_of(place, extent, LINES, ending)),
              "second",
            ));
          }
          if let Some(third) = third {
            labels.push(Label::new(
              Location::new(0, Span::new(start_of(third, ending), end_of(third, ending))),
              "third",
            ));
          }

          let diagnostic =
            Diagnostic::new("code", Severity::Error, &message, Location::new(0, span))
              .with_primary_label("first")
              .with_labels(&labels);

          let html = as_html(&diagnostic, &text);
          let terminal = as_terminal(&diagnostic, &text);
          checked += 1;

          let (drawn, expected) = (html_rows(&html), terminal_rows(&terminal));
          let at = format!(
            "break={:?} primary={primary:?} second={second:?} third={third:?}",
            ending
          );

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
            failed.push(format!("{at}: html drew a line twice, {drawn:?}"));
          }

          if drawn != expected {
            failed.push(format!(
              "{at}: html {drawn:?} against terminal {expected:?}"
            ));
          }
        }
      }
    }
  }

  // The grid is the claim, so an empty or shrunken one has to fail rather than pass silently.
  assert_eq!(
    checked,
    BREAKS.len() * 13 * (1 + 13 * 3) * 2,
    "the grid did not take every value"
  );
  assert!(
    failed.is_empty(),
    "{} of {checked} points draw different rows:\n{}",
    failed.len(),
    failed.join("\n")
  );
}

/// A caret is drawn on the line layer 2 puts it on — in both outputs, at every offset of every
/// awkward source.
///
/// # Layer 2 is the oracle here, and it has to be
///
/// Everywhere else in this file one renderer's output is checked against the other's, and that is
/// structurally blind to anything the two share. It is also blind in one more direction nobody had
/// named until the round this test was written in: **it says nothing about the terminal on its
/// own.** A zero-width-span defect on that side would have been as silent as the one on this side
/// was.
///
/// A caret is the one shape where a third, independent answer exists and is exact.
/// [`Region::lines`] yields precisely the lines a span is drawn on, and for a span of no bytes that
/// is one line — so the row a reader is shown is decidable without asking either renderer. Both are
/// held to it.
///
/// The sources are chosen so that every offset the walk treats specially is reached: the first byte
/// after each of the three line breaks, the position after a trailing break, the end of a line's
/// content, an offset inside a CRLF, one inside a multi-byte character, and the end of a source
/// that has no trailing break at all.
#[test]
fn a_caret_is_drawn_on_the_line_layer_two_puts_it_on() {
  const AWKWARD: [&str; 10] = [
    "",
    "\n",
    "l1\nl2\nl3\n",
    "l1\r\nl2\r\nl3\r\n",
    "l1\rl2\rl3\r",
    "l1\nl2\r\nl3\rl4",
    "no trailing break",
    "\u{1f3a8}\n\u{1f3a8}\n",
    "e\u{301}\ne\u{301}\n",
    "\t\n\t\n",
  ];

  let mut checked = 0usize;
  for text in AWKWARD {
    let source = Source::new(text);
    for at in 0..=text.len() {
      let message = "a caret";
      let diagnostic = Diagnostic::new(
        "code",
        Severity::Error,
        &message,
        Location::new(0, Span::empty(at)),
      )
      .with_primary_label("here");

      // What layer 2 says, which is neither renderer's answer.
      let expected: Vec<String> = source
        .resolve(Span::empty(at))
        .lines()
        .map(|on| on.line().number().to_string())
        .collect();
      assert_eq!(expected.len(), 1, "a caret is drawn on exactly one line");

      let html = as_html(&diagnostic, text);
      let terminal = as_terminal(&diagnostic, text);
      checked += 1;

      assert_eq!(
        html_rows(&html),
        expected,
        "{text:?} at {at}: HTML draws rows layer 2 does not\n{html}"
      );
      assert_eq!(
        terminal_rows(&terminal),
        expected,
        "{text:?} at {at}: the terminal draws rows layer 2 does not\n{terminal}"
      );

      // And the position each one announces, which is the other place a stop resolved with the
      // wrong semantics would show: `Source::position` is ordinary and `closes_on` is not.
      let position = source.position(at);
      let announced = format!("{}:{}", position.line(), position.column());
      assert!(
        html.contains(&format!(
          r#"<span class="painty-position">{announced}</span>"#
        )),
        "{text:?} at {at}: HTML announces something other than {announced}\n{html}"
      );
      // Both renderers are given the same origin name, and the terminal writes it into the
      // machine-parsed triple.
      assert!(
        terminal.contains(&format!(" --> input:{announced}\n")),
        "{text:?} at {at}: the terminal announces something other than {announced}\n{terminal}"
      );
    }
  }

  assert_eq!(
    checked,
    AWKWARD.iter().map(|text| text.len() + 1).sum::<usize>(),
    "the sweep did not reach every offset"
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
  let ending = "\n";
  let text = source_of(9, ending);
  let message = "three of them";
  let labels = [
    Label::new(
      Location::new(0, Span::new(start_of(3, ending), end_of(3, ending))),
      "on three",
    ),
    Label::new(
      Location::new(0, Span::new(start_of(1, ending), end_of(3, ending))),
      "one to three",
    ),
  ];
  let diagnostic = Diagnostic::new(
    "code",
    Severity::Error,
    &message,
    Location::new(0, Span::new(start_of(3, ending), end_of(3, ending))),
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
