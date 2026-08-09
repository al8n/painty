//! What the terminal renderer looks like.
//!
//! # These come after the invariants, and that ordering is load-bearing
//!
//! A golden records whatever the code did on the day it was written. Written first, it becomes the
//! thing the invariants get adjusted to agree with — and then both agree with the defect. The
//! structural invariants in `src/terminal/tests.rs` were written first and from the definition, so
//! what is pinned here is *appearance* on top of a base that is already correct: the arrangement of
//! rows, the characters chosen, the spacing. Re-blessing one of these should be cheap, and it is
//! only safe because the arithmetic underneath is held by something else.
//!
//! # Where a number is pinned, it is pinned against the defect
//!
//! A renderer is full of numbers a person reads off the output and writes down, and a number
//! written down that way agrees with whatever produced it. So wherever an offset or a count matters
//! here, the assertion names the value a *wrong* implementation would produce and requires the
//! output to differ, rather than naming the right value and hoping it was right.

#![cfg(feature = "terminal")]

use painty::{
  Diagnostic, Label, Location, Severity, Source, Span, Theme,
  terminal::{ColorCapability, Terminal},
};

fn render(diagnostic: &Diagnostic<'_>, text: &str, origin: Option<&str>) -> String {
  let mut out = String::new();
  Terminal::plain()
    .render(diagnostic, Source::new(text), origin, &mut out)
    .expect("a String never fails to be written to");
  out
}

const SCHEMA: &str = "type Widget {\n  width: Int\n  width: Int\n}\n";

#[test]
fn a_single_label_reads_the_way_a_reader_expects() {
  let message = "`width` is defined twice";
  let diagnostic = Diagnostic::new(
    "mylang::schema::duplicate-field",
    Severity::Error,
    &message,
    Location::new(0, Span::new(29, 34)),
  )
  .with_primary_label("redefined here")
  .with_help("rename one of the two definitions");

  assert_eq!(
    render(&diagnostic, SCHEMA, Some("widget.graphql")),
    "\
error[mylang::schema::duplicate-field]: `width` is defined twice
 --> widget.graphql:3:3
  |
3 |   width: Int
  |   ^^^^^ redefined here
  |
  = help: rename one of the two definitions
"
  );
}

#[test]
fn a_secondary_label_is_drawn_with_a_different_marker() {
  let message = "`width` is defined twice";
  let labels = [Label::new(
    Location::new(0, Span::new(16, 21)),
    "first defined here",
  )];
  let diagnostic = Diagnostic::new(
    "mylang::schema::duplicate-field",
    Severity::Error,
    &message,
    Location::new(0, Span::new(29, 34)),
  )
  .with_primary_label("redefined here")
  .with_labels(&labels);

  assert_eq!(
    render(&diagnostic, SCHEMA, None),
    "\
error[mylang::schema::duplicate-field]: `width` is defined twice
 --> 3:3
  |
3 |   width: Int
  |   ^^^^^ redefined here
  |
2 |   width: Int
  |   ----- first defined here
  |
"
  );
}

#[test]
fn a_position_with_nothing_to_point_at_prints_only_its_header() {
  let message = "this document was generated, so it has no positions";
  let diagnostic = Diagnostic::new(
    "mylang::input::synthesized",
    Severity::Warning,
    &message,
    Location::entire(0),
  );
  assert_eq!(
    render(&diagnostic, SCHEMA, None),
    "warning[mylang::input::synthesized]: this document was generated, so it has no positions\n"
  );
}

#[test]
fn a_wide_character_gets_a_marker_as_wide_as_it_draws() {
  // Two cells per ideograph, so the marker under three of them is six characters and not three.
  // Asserted against the count a character-counting renderer would produce, so that the wrong
  // implementation cannot satisfy it — the right count is also written out, but it is the
  // inequality that carries the weight.
  let text = "let 日本語 = 1;\n";
  let message = "a name in another script";
  let start = text.find('日').expect("the fixture has one");
  let end = start + "日本語".len();
  let diagnostic = Diagnostic::new(
    "mylang::test::wide",
    Severity::Error,
    &message,
    Location::new(0, Span::new(start, end)),
  )
  .with_primary_label("three characters");

  let out = render(&diagnostic, text, None);
  let marker = out
    .lines()
    .find_map(|row| row.split_once('^').map(|_| row))
    .expect("a marker row");
  let carets = marker.chars().filter(|c| *c == '^').count();

  let counted_characters = 3;
  assert_ne!(
    carets, counted_characters,
    "measured characters, not cells\n{out}"
  );
  assert_eq!(carets, 6, "{out}");
  assert_eq!(
    out,
    "\
error[mylang::test::wide]: a name in another script
 --> 1:5
  |
1 | let 日本語 = 1;
  |     ^^^^^^ three characters
  |
"
  );
}

#[test]
fn a_tab_is_expanded_and_the_marker_lands_under_what_it_points_at() {
  // The tab is one character and four cells, so a marker placed by character offset would sit
  // three cells to the left of the text it is about.
  let text = "\tlet x = 1;\n";
  let message = "an indented statement";
  let diagnostic = Diagnostic::new(
    "mylang::test::tabbed",
    Severity::Error,
    &message,
    Location::new(0, Span::new(5, 6)),
  )
  .with_primary_label("this one");

  let out = render(&diagnostic, text, None);
  let marker_row = out
    .lines()
    .find(|row| row.contains('^'))
    .expect("a marker row");
  let source_row = out
    .lines()
    .find(|row| row.contains("let"))
    .expect("a source row");

  let placed_by_character = source_row.find('x').expect("the x") - 3;
  let caret = marker_row.find('^').expect("the caret");
  assert_ne!(
    caret, placed_by_character,
    "placed by character, not by cell\n{out}"
  );
  assert_eq!(caret, source_row.find('x').expect("the x"), "{out}");

  assert!(
    !out.contains('\t'),
    "a tab survived into the output\n{out:?}"
  );
  assert_eq!(
    out,
    "\
error[mylang::test::tabbed]: an indented statement
 --> 1:9
  |
1 |     let x = 1;
  |         ^ this one
  |
"
  );
}

#[test]
fn a_span_over_the_last_byte_of_a_file_with_no_trailing_newline() {
  let text = "alpha\nbeta";
  let message = "the last thing in the file";
  let diagnostic = Diagnostic::new(
    "mylang::test::final-byte",
    Severity::Advice,
    &message,
    Location::new(0, Span::new(9, 10)),
  )
  .with_primary_label("here");

  assert_eq!(
    render(&diagnostic, text, None),
    "\
advice[mylang::test::final-byte]: the last thing in the file
 --> 2:4
  |
2 | beta
  |    ^ here
  |
"
  );
}

#[test]
fn colour_appears_only_when_the_capability_allows_it() {
  // Appearance in the other sense. The same diagnostic at three capabilities: no escapes at all,
  // then the sixteen, then truecolour — and the escape a truecolour terminal gets must differ from
  // the one a sixteen-colour terminal gets, or the narrowing is not happening.
  let message = "a message";
  let diagnostic = Diagnostic::new(
    "mylang::test::rule",
    Severity::Error,
    &message,
    Location::new(0, Span::new(0, 3)),
  );
  let theme = Theme::new().with(
    painty::Role::Severity(Severity::Error),
    painty::Style::plain().with_foreground(painty::Color::Rgb(200, 30, 40)),
  );

  let at = |capability| {
    let mut out = String::new();
    Terminal::with_palette(theme)
      .with_capability(capability)
      .render(&diagnostic, Source::new("abc\n"), None, &mut out)
      .expect("a String is writable");
    out
  };

  let none = at(ColorCapability::None);
  let sixteen = at(ColorCapability::Ansi16);
  let palette = at(ColorCapability::Ansi256);
  let truecolour = at(ColorCapability::TrueColor);

  assert!(!none.contains('\u{1b}'), "{none:?}");
  assert!(sixteen.contains('\u{1b}'));
  assert_ne!(sixteen, palette, "the palette narrowing changed nothing");
  assert_ne!(palette, truecolour, "the truecolour path changed nothing");

  // And the text is identical once the escapes are removed, so narrowing changes appearance and
  // never content.
  let strip = |rendered: &str| {
    let mut out = String::new();
    let mut chars = rendered.chars();
    while let Some(character) = chars.next() {
      if character == '\u{1b}' {
        for inner in chars.by_ref() {
          if inner == 'm' {
            break;
          }
        }
      } else {
        out.push(character);
      }
    }
    out
  };
  assert_eq!(strip(&sixteen), none);
  assert_eq!(strip(&truecolour), none);
}
