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
  terminal::{ColorCapability, Input, Terminal},
};

fn render(diagnostic: &Diagnostic<'_>, text: &str, origin: Option<&str>) -> String {
  let mut input = Input::new(Source::new(text));
  if let Some(origin) = origin {
    input = input.with_origin(origin);
  }
  let mut out = String::new();
  Terminal::plain()
    .render(diagnostic, &[input], &mut out)
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
      .render(&diagnostic, &[Input::new(Source::new("abc\n"))], &mut out)
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

/// Which escape families a rendered string contains.
///
/// Parsed rather than searched for, so a family cannot hide inside a longer sequence.
fn escape_families(rendered: &str) -> Vec<String> {
  let mut families = Vec::new();
  let bytes: Vec<char> = rendered.chars().collect();
  let mut at = 0;
  while at < bytes.len() {
    if bytes[at] == '\u{1b}' && bytes.get(at + 1) == Some(&'[') {
      let mut end = at + 2;
      while end < bytes.len() && bytes[end] != 'm' {
        end += 1;
      }
      let body: String = bytes[at + 2..end.min(bytes.len())].iter().collect();
      for parameter in body.split(';').filter(|p| !p.is_empty()) {
        families.push(parameter.to_owned());
      }
      at = end + 1;
    } else {
      at += 1;
    }
  }
  families
}

#[test]
fn each_capability_emits_only_the_escape_families_it_promises() {
  // The same class as the no-colour defect found while building, enumerated rather than fixed one
  // instance at a time: at every level, output that exceeds the level is the bug. The sixteen were
  // being emitted as `38;5;n`, which is the 256-colour form and a capability a sixteen-colour
  // terminal was never promised.
  //
  // A control character is in the source deliberately: no capability may turn one into an escape.
  let text = "let \u{7f}x = 1;\n";
  let message = "a message";
  let diagnostic = Diagnostic::new(
    "mylang::test::rule",
    Severity::Error,
    &message,
    Location::new(0, Span::new(4, 6)),
  )
  .with_primary_label("here")
  .with_help("do this");

  let at = |theme: Theme, capability| {
    let mut out = String::new();
    Terminal::with_palette(theme)
      .with_capability(capability)
      .render(&diagnostic, &[Input::new(Source::new(text))], &mut out)
      .expect("a String is writable");
    out
  };

  // None: no escape at all, whatever the theme asks for.
  for theme in [Theme::new(), Theme::high_contrast(), Theme::monochrome()] {
    let out = at(theme, ColorCapability::None);
    assert!(
      escape_families(&out).is_empty(),
      "a capability of none emitted {:?}",
      escape_families(&out)
    );
    assert!(out.contains('\u{7f}'), "the control character was altered");
  }

  // Sixteen: only attributes and the 30–37 / 90–97 foreground families, never `38`, which
  // introduces the 256-colour and truecolour forms.
  let sixteen = at(Theme::new(), ColorCapability::Ansi16);
  let families = escape_families(&sixteen);
  assert!(!families.is_empty(), "nothing was emitted at all");
  for family in &families {
    let code: u16 = family.parse().expect("a numeric SGR parameter");
    let allowed = matches!(code, 0..=9)
      || (30..=37).contains(&code)
      || (40..=47).contains(&code)
      || (90..=97).contains(&code)
      || (100..=107).contains(&code);
    assert!(
      allowed,
      "the sixteen-colour capability emitted SGR {code}, which it does not promise: {families:?}"
    );
  }
  assert!(
    !families.iter().any(|f| f == "38" || f == "48"),
    "the sixteen-colour capability emitted an extended-colour introducer: {families:?}"
  );

  // 256: `38;5;n` is allowed, `38;2;r;g;b` is not.
  let palette = at(
    Theme::new().with(
      painty::Role::Severity(Severity::Error),
      painty::Style::plain().with_foreground(painty::Color::Rgb(200, 30, 40)),
    ),
    ColorCapability::Ansi256,
  );
  let palette_families = escape_families(&palette);
  assert!(
    palette_families.iter().any(|f| f == "5"),
    "{palette_families:?}"
  );
  assert!(
    !palette_families.iter().any(|f| f == "2"),
    "the 256-colour capability emitted a truecolour introducer: {palette_families:?}"
  );

  // Truecolour: the `2` form is what distinguishes it, and its absence would mean the level did
  // nothing.
  let truecolour = at(
    Theme::new().with(
      painty::Role::Severity(Severity::Error),
      painty::Style::plain().with_foreground(painty::Color::Rgb(200, 30, 40)),
    ),
    ColorCapability::TrueColor,
  );
  assert!(escape_families(&truecolour).iter().any(|f| f == "2"));
}

#[test]
fn a_label_in_another_input_is_drawn_against_that_input() {
  // The defect: every span was resolved against one source, so a label pointing into input 1 was
  // rendered against input 0 — a confident line number and a marker under whatever happened to be
  // there. Silently wrong output.
  //
  // Asserted against the text the WRONG input would have shown, so the defect cannot satisfy it.
  let schema = "type Widget {\n  width: Int\n}\n";
  let other = "extend type Widget {\n  width: Float\n}\n";
  let message = "`width` is declared in two documents";
  let labels = [Label::new(
    Location::new(1, Span::new(23, 28)),
    "and again here",
  )];
  let diagnostic = Diagnostic::new(
    "mylang::schema::duplicate-field",
    Severity::Error,
    &message,
    Location::new(0, Span::new(16, 21)),
  )
  .with_primary_label("declared here")
  .with_labels(&labels);

  let mut out = String::new();
  Terminal::plain()
    .render(
      &diagnostic,
      &[
        Input::new(Source::new(schema)).with_origin("schema.graphql"),
        Input::new(Source::new(other)).with_origin("extension.graphql"),
      ],
      &mut out,
    )
    .expect("a String is writable");

  assert!(
    out.contains("width: Int"),
    "the primary input is missing\n{out}"
  );
  assert!(
    out.contains("width: Float"),
    "the label was not drawn against its own input\n{out}"
  );
  assert!(out.contains("schema.graphql:2:3"), "{out}");
}

#[test]
fn a_location_naming_an_input_that_was_not_supplied_draws_no_excerpt() {
  // Total rather than panicking or fabricating: an index past the list is the same situation as a
  // position with no span, and gets the same answer.
  let message = "a message";
  let labels = [Label::new(Location::new(7, Span::new(0, 1)), "elsewhere")];
  let diagnostic = Diagnostic::new(
    "mylang::test::rule",
    Severity::Error,
    &message,
    Location::new(0, Span::new(0, 3)),
  )
  .with_labels(&labels);

  let mut out = String::new();
  Terminal::plain()
    .render(&diagnostic, &[Input::new(Source::new("abc\n"))], &mut out)
    .expect("a String is writable");
  assert!(out.contains("^^^"), "{out}");
  assert!(
    !out.contains("elsewhere"),
    "an absent input was drawn anyway\n{out}"
  );
}
