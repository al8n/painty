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
    // This assertion used to be its own opposite — that the control character reached the output
    // untouched — and it changed because the DEFINITION did, not because the output did. A control
    // character in caller-supplied text is an instruction to the terminal, and a renderer whose
    // whole promise is deciding what the terminal is told cannot forward one unread.
    assert!(
      !out.contains('\u{7f}'),
      "a control character was forwarded to the terminal\n{out}"
    );
    assert!(
      out.contains('\u{2421}'),
      "and it was not shown either\n{out}"
    );
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
fn every_input_an_excerpt_comes_from_is_named_above_it() {
  // The half of the multi-input fix that did not land. Resolving each span against its own input
  // stopped the renderer drawing the wrong TEXT; the header stayed single-origin, so the right text
  // appeared under the wrong filename. That is the same class of defect the resolution fix closed —
  // a reader told, confidently and silently, where something is — and it is worse than no name at
  // all, because a name that is present is believed.
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

  // The whole frame, because what was wrong was the ARRANGEMENT rather than any one value: the
  // second file's row sat under the first file's header, and only the order of the rows says so.
  assert_eq!(
    out,
    "\
error[mylang::schema::duplicate-field]: `width` is declared in two documents
 --> schema.graphql:2:3
  |
2 |   width: Int
  |   ^^^^^ declared here
  |
 --> extension.graphql:2:3
  |
2 |   width: Float
  |   ----- and again here
  |
"
  );
}

#[test]
fn one_input_is_named_once_however_many_excerpts_it_has() {
  // The converse, and the reason the header is keyed on the input rather than emitted per excerpt:
  // two labels in the same file are one file, and repeating its name between them would be noise a
  // reader has to re-read to discover says nothing new.
  let schema = "type Widget {\n  width: Int\n  width: Int\n}\n";
  let message = "`width` is defined twice";
  let labels = [Label::new(
    Location::new(0, Span::new(29, 34)),
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
      &[Input::new(Source::new(schema)).with_origin("schema.graphql")],
      &mut out,
    )
    .expect("a String is writable");

  assert_eq!(
    out.matches("--> ").count(),
    1,
    "the same input was announced more than once: {out}"
  );
  assert_eq!(
    out.matches("schema.graphql").count(),
    1,
    "the same origin was repeated: {out}"
  );
}

#[test]
fn two_unnamed_inputs_are_still_two_inputs() {
  // Keying the header on the origin STRING rather than the input index would collapse these two
  // into one announcement, and the second excerpt would again sit under a header that is not its
  // own — the defect surviving in the case where nothing names it. Two inputs with the same origin
  // would collapse the same way.
  let first = "alpha\n";
  let second = "beta\n";
  let message = "a message";
  let labels = [Label::new(Location::new(1, Span::new(0, 4)), "and here")];
  let diagnostic = Diagnostic::new(
    "mylang::test::rule",
    Severity::Error,
    &message,
    Location::new(0, Span::new(0, 5)),
  )
  .with_primary_label("here")
  .with_labels(&labels);

  let mut out = String::new();
  Terminal::plain()
    .render(
      &diagnostic,
      &[
        Input::new(Source::new(first)),
        Input::new(Source::new(second)),
      ],
      &mut out,
    )
    .expect("a String is writable");

  assert_eq!(
    out.matches("--> ").count(),
    2,
    "an unnamed second input was folded into the first: {out}"
  );
  assert!(out.contains("alpha"), "{out}");
  assert!(out.contains("beta"), "{out}");
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

#[test]
fn no_capability_lets_caller_text_steer_the_terminal() {
  // Escape injection. The capability gate governed only the styles the palette produced, so a
  // message, a code, an origin, a label or a line of source containing `\x1b[38;5;196m` reached the
  // terminal verbatim — under `ColorCapability::None` as readily as under truecolour. That defeats
  // the per-level guarantee in the test above from the one direction it does not control: its
  // input.
  let attack = "\u{1b}[38;5;196m";
  let text = format!("let x = \"{attack}\";\n");
  let message = format!("unexpected {attack} here");
  let label_text = format!("this {attack}");
  let origin = format!("src/{attack}.rs");
  let labels = [Label::new(
    Location::new(0, Span::new(4, 5)),
    label_text.as_str(),
  )];
  let diagnostic = Diagnostic::new(
    "mylang::test::rule",
    Severity::Error,
    &message,
    Location::new(0, Span::new(8, 12)),
  )
  .with_primary_label(&label_text)
  .with_labels(&labels)
  .with_help(&message);

  for capability in [
    ColorCapability::None,
    ColorCapability::Ansi16,
    ColorCapability::Ansi256,
    ColorCapability::TrueColor,
  ] {
    let mut out = String::new();
    Terminal::with_palette(Theme::new())
      .with_capability(capability)
      .render(
        &diagnostic,
        &[Input::new(Source::new(&text)).with_origin(&origin)],
        &mut out,
      )
      .expect("a String is writable");

    if capability == ColorCapability::None {
      assert!(
        !out.contains('\u{1b}'),
        "an ESC reached a no-colour terminal: {out:?}"
      );
    }
    // At every level the 256-colour family the INPUT asked for must be absent, because no level was
    // asked for it by the palette. Under `Ansi16` that is also the level's own promise, and under
    // truecolour it is what proves the escape came from painty rather than from the text.
    let families = escape_families(&out);
    assert!(
      !families.iter().any(|f| f == "38"),
      "{capability:?} passed the input's own escape through: {families:?}"
    );
    // The ESC, not the text after it. Written the other way round this contradicted its own
    // neighbour below — "shown rather than swallowed" means the visible `␛[38;5;196m` is exactly
    // what should be there — and what makes a sequence a sequence is the byte that introduces it.
    assert!(
      !out.contains("\u{1b}[38;5;196m"),
      "{capability:?} wrote the injected sequence: {out:?}"
    );
    // Shown rather than swallowed: a reader has to be able to see what was in the file.
    assert!(
      out.contains('\u{241b}'),
      "{capability:?} dropped the escape instead of showing it: {out:?}"
    );
  }
}

#[test]
fn a_sanitized_control_character_does_not_move_the_marker() {
  // The substitution happens where the text is written, so it is only safe because it is
  // width-preserving: the width table gives every control character one cell, and each stand-in is
  // one cell and one cluster. If that ever stops being true the caret moves, so it is asserted
  // rather than assumed.
  let message = "a message";
  let of = |text: &str| {
    let diagnostic = Diagnostic::new(
      "mylang::test::rule",
      Severity::Error,
      &message,
      Location::new(0, Span::new(9, 10)),
    );
    let mut out = String::new();
    Terminal::plain()
      .render(&diagnostic, &[Input::new(Source::new(text))], &mut out)
      .expect("a String is writable");
    out
  };
  let marker_column = |out: &str| {
    out
      .lines()
      .find(|line| line.contains('^'))
      .and_then(|line| line.find('^'))
      .expect("a marker row")
  };

  assert_eq!(
    marker_column(&of("let ab = 1;\n")),
    marker_column(&of("let \u{1b}b = 1;\n"))
  );
}

#[test]
fn every_control_picture_is_the_standard_one_for_its_character() {
  // The table in `control_picture` is transcribed, so it is checked against the definition rather
  // than trusted: the Control Pictures block runs from U+2400 in code point order, and DEL has its
  // own at U+2421. The arithmetic lives here because `src/` may not spell a `u32`.
  //
  // Also the width claim the substitution rests on, asserted at the character rather than inferred
  // from a rendered frame: every stand-in must be one cell, or the caret moves.
  use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

  let text: String = (0u32..=0x9f)
    .filter_map(char::from_u32)
    .filter(|c| *c != '\t' && *c != '\n' && *c != '\r')
    .collect();
  let mut out = String::new();
  let message = "a message";
  let diagnostic = Diagnostic::new(
    "mylang::test::rule",
    Severity::Error,
    &message,
    Location::new(0, Span::new(0, 1)),
  );
  Terminal::plain()
    .render(
      &diagnostic,
      &[Input::new(Source::new(&format!("{text}\n")))],
      &mut out,
    )
    .expect("a String is writable");

  for raw in (0u32..=0x1f).filter_map(char::from_u32) {
    if raw == '\t' || raw == '\n' || raw == '\r' {
      continue;
    }
    let expected = char::from_u32(0x2400 + raw as u32).expect("the block is contiguous");
    assert!(
      out.contains(expected),
      "U+{:04X} was not shown as {expected:?}",
      raw as u32
    );
    assert!(
      !out.contains(raw),
      "U+{:04X} reached the output",
      raw as u32
    );
    // Both sides of the width-preserving claim. The substitution happens where the text is
    // written and the measurement reads the ORIGINAL, so a control that did not measure one cell,
    // or a stand-in that was not one cell, would move every column after it.
    assert_eq!(expected.width(), Some(1), "{expected:?} is not one cell");
    assert_eq!(
      raw.to_string().as_str().width(),
      1,
      "U+{:04X} does not measure one cell, so substituting it moves the caret",
      raw as u32
    );
  }
  assert!(out.contains('\u{2421}'), "DEL was not shown");
  assert_eq!('\u{2421}'.width(), Some(1));
  assert_eq!('\u{fffd}'.width(), Some(1));
  for raw in (0x80u32..=0x9f).filter_map(char::from_u32) {
    assert!(
      !out.contains(raw),
      "U+{:04X} reached the output",
      raw as u32
    );
  }
}

/// A message whose own `Display` writes a tab.
///
/// The sanitizer is an adapter over `fmt::Write` precisely so that this is covered: text a
/// caller's type writes is as caller-supplied as text it hands over directly.
struct TabbyMessage;

impl core::fmt::Display for TabbyMessage {
  fn fmt(&self, out: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(out, "written\tby\ta\tDisplay")
  }
}

#[test]
fn a_tab_in_non_source_text_cannot_move_the_cursor_either() {
  // The hole the escape substitution left behind. It shows every C0 character except U+0009, and
  // the exclusion was written for SOURCE text, where `write_expanded` prices a tab against a stop
  // before anything else sees it. Nothing else has a stop. A tab in a message, a code, an origin, a
  // label or a help line was emitted verbatim at every capability — and a tab MOVES THE CURSOR, so
  // caller text could still push the frame around, which is the one thing the substitution exists
  // to stop.
  //
  // All six paths at once, because they reach the sanitizer through three different call sites and
  // the message reaches it through a caller's own `Display`.
  let code = "mylang::test\u{9}rule";
  let origin = "src/we\u{9}ird.rs";
  let primary = "primary\tlabel";
  let secondary = "second\tlabel";
  let help = "help\ttext";
  // Counted from the inputs rather than written down: a literal is a second thing to get wrong, and
  // one written too low would have passed.
  let expected = [code, origin, primary, secondary, help]
    .iter()
    .map(|text| text.matches('\t').count())
    .sum::<usize>()
    + TabbyMessage.to_string().matches('\t').count();
  assert!(
    expected > 0,
    "the inputs carry no tab, so nothing below proves anything"
  );

  let labels = [Label::new(Location::new(0, Span::new(4, 5)), secondary)];
  let diagnostic = Diagnostic::new(
    code,
    Severity::Error,
    &TabbyMessage,
    Location::new(0, Span::new(0, 3)),
  )
  .with_primary_label(primary)
  .with_labels(&labels)
  .with_help(help);

  for capability in [
    ColorCapability::None,
    ColorCapability::Ansi16,
    ColorCapability::Ansi256,
    ColorCapability::TrueColor,
  ] {
    let mut out = String::new();
    Terminal::with_palette(Theme::new())
      .with_capability(capability)
      .render(
        &diagnostic,
        &[Input::new(Source::new("abc\ndef\n")).with_origin(origin)],
        &mut out,
      )
      .expect("a String is writable");

    assert!(
      !out.contains('\t'),
      "{capability:?} forwarded a tab from caller text: {out:?}"
    );
    // Shown rather than dropped, and counted rather than merely present: a substitution that fired
    // on one of the six paths and not the rest would satisfy the assertion above.
    assert_eq!(
      out.matches('\u{2409}').count(),
      expected,
      "{capability:?} did not show every tab: {out:?}"
    );
  }
}

#[test]
fn a_tab_in_source_text_is_still_expanded_rather_than_shown() {
  // The other half, and the one the fix above had to be careful of. `control_picture` now answers
  // `␉` for a tab like it does for every other C0 character, so what keeps a SOURCE tab expanded is
  // that `write_expanded` matches the tab cluster BEFORE it consults the lookup. Drawing one `␉`
  // where a stop's worth of cells was counted would misplace every marker on the line — which the
  // golden in `a_tab_is_expanded_and_the_marker_lands_under_what_it_points_at` pins; this pins the
  // ordering that the golden depends on, at the character.
  let message = "a message";
  let diagnostic = Diagnostic::new(
    "mylang::test::rule",
    Severity::Error,
    &message,
    Location::new(0, Span::new(2, 3)),
  );
  let mut out = String::new();
  Terminal::plain()
    .render(&diagnostic, &[Input::new(Source::new("a\tb\n"))], &mut out)
    .expect("a String is writable");

  assert!(!out.contains('\u{2409}'), "a source tab was shown: {out:?}");
  assert!(!out.contains('\t'), "a source tab was forwarded: {out:?}");
  assert!(
    out.contains("a   b"),
    "a source tab was not expanded to its stop: {out:?}"
  );
}

#[test]
fn no_control_character_at_all_survives_a_caller_string() {
  // The class, rather than the members of it that anyone thought to name. ESC and the tab were each
  // found by hand and pinned as themselves, and a list of named inputs is not a guarantee — it is
  // a record of what has been looked for. This is the guarantee: the characters that can steer a
  // terminal are C0, DEL and C1, that set is finite, and every one of them goes through every
  // caller string.
  //
  // The tab is the evidence that the difference is real. It was the ONE member the substitution
  // declined, and nothing here knows or cares that it was ever special.
  //
  // At no-colour only, and that is what makes the assertion clean rather than weaker: that
  // capability promises no escape sequences at all, so painty contributes no control character but
  // the row breaks it writes itself, and anything else in the output came from the input.
  let source = "abc\ndef\n";
  let controls = (0u32..=0x1f)
    .chain(0x7fu32..=0x9f)
    .filter_map(char::from_u32);
  for raw in controls {
    // Stated from the definition, not read back from the crate: the Control Pictures block is
    // U+2400 upwards in code point order, DEL has its own, and C1 has none at all.
    let picture = match raw {
      '\u{7f}' => '\u{2421}',
      '\u{80}'..='\u{9f}' => '\u{fffd}',
      _ => char::from_u32(0x2400 + raw as u32).expect("the block is contiguous"),
    };
    let code = format!("mylang{raw}rule");
    let origin = format!("src/a{raw}b.rs");
    let message = format!("a{raw}message");
    let primary = format!("primary{raw}label");
    let secondary = format!("second{raw}label");
    let help = format!("help{raw}text");
    let occurrences = 6;

    let labels = [Label::new(
      Location::new(0, Span::new(4, 5)),
      secondary.as_str(),
    )];
    let diagnostic = Diagnostic::new(
      &code,
      Severity::Error,
      &message,
      Location::new(0, Span::new(0, 3)),
    )
    .with_primary_label(&primary)
    .with_labels(&labels)
    .with_help(&help);

    let mut out = String::new();
    Terminal::plain()
      .render(
        &diagnostic,
        &[Input::new(Source::new(source)).with_origin(&origin)],
        &mut out,
      )
      .expect("a String is writable");

    // LF is the one exception, and it is a limit of the observation rather than of the guarantee:
    // painty ends every row with one, so a caller's own cannot be told apart from the frame's. The
    // count below still pins it, because `␊` can only have come from the input.
    if raw != '\n' {
      assert!(
        !out.contains(raw),
        "U+{:04X} reached the terminal through a caller string: {out:?}",
        raw as u32
      );
    }
    assert_eq!(
      out.matches(picture).count(),
      occurrences,
      "U+{:04X} was shown on {} of the {occurrences} caller strings: {out:?}",
      raw as u32,
      out.matches(picture).count()
    );
  }
}

/// A writer that accepts a fixed number of characters and then declines, keeping a copy of
/// everything it was OFFERED — including what it refused.
///
/// What painty writes is painty's; how much is taken is the writer's. So the property below is
/// asserted over what was offered: a writer bounded at any point must never be left holding an
/// opener that painty did not follow with a reset. A record of what was ACCEPTED could not show it,
/// because a writer that refuses the body refuses the reset too.
struct Recording {
  budget: usize,
  taken: usize,
  offered: String,
}

impl core::fmt::Write for Recording {
  fn write_str(&mut self, text: &str) -> core::fmt::Result {
    self.offered.push_str(text);
    for _ in text.chars() {
      if self.taken == self.budget {
        return Err(core::fmt::Error);
      }
      self.taken += 1;
    }
    Ok(())
  }
}

/// Whether the text finishes with a style still open.
///
/// Counting openers against resets does NOT work here, and the first draft of this test failed on a
/// correct frame because of it: `anstyle` renders bold and colour as two separate sequences and
/// closes both with one reset, so a balanced frame has more openers than resets. What has to hold is
/// the state at the END — walk the sequences in order, and the last complete one must be the reset.
///
/// An incomplete sequence is skipped rather than parsed, because a refused write can leave one: the
/// terminal does not act on a sequence it never received the end of, so neither does this.
fn ends_styled(text: &str) -> bool {
  let characters: Vec<char> = text.chars().collect();
  let mut open = false;
  let mut at = 0;
  while at < characters.len() {
    if characters[at] == '\u{1b}' && characters.get(at + 1) == Some(&'[') {
      let mut end = at + 2;
      while end < characters.len() && (characters[end].is_ascii_digit() || characters[end] == ';') {
        end += 1;
      }
      if characters.get(end) == Some(&'m') {
        let parameters: String = characters[at + 2..end].iter().collect();
        open = parameters != "0";
        at = end + 1;
        continue;
      }
    }
    at += 1;
  }
  open
}

#[test]
fn a_style_this_opened_is_closed_however_far_the_writer_gets() {
  // The claim that justified the closure form was wrong, and precisely so: a closure stops a caller
  // FORGETTING the reset, and does nothing about the body NOT REACHING it. The `?` on a failing
  // write was exactly that second thing, so a coloured render into a bounded writer returned with an
  // SGR still open and everything the caller's terminal printed afterwards wearing it.
  //
  // Swept over every budget rather than one chosen to land somewhere interesting: WHICH write fails
  // is the whole subject, so this refuses at each position in turn instead of at a position picked
  // by someone who believed the code was already right.
  let text = "type Widget {\n  width: Int\n}\n";
  let message = "`width` is defined twice";
  let diagnostic = Diagnostic::new(
    "mylang::schema::duplicate-field",
    Severity::Error,
    &message,
    Location::new(0, Span::new(16, 21)),
  )
  .with_primary_label("redefined here");
  let terminal = Terminal::with_palette(Theme::new()).with_capability(ColorCapability::TrueColor);
  let inputs = [Input::new(Source::new(text)).with_origin("widget.graphql")];

  let mut full = String::new();
  terminal
    .render(&diagnostic, &inputs, &mut full)
    .expect("a String is writable");
  assert!(
    !escape_families(&full).is_empty(),
    "nothing in this frame is styled, so refusing inside it would prove nothing: {full:?}"
  );
  assert!(
    !ends_styled(&full),
    "the frame ends styled even when nothing refuses: {full:?}"
  );

  let length = full.chars().count();
  for budget in 0..length {
    let mut out = Recording {
      budget,
      taken: 0,
      offered: String::new(),
    };
    let result = terminal.render(&diagnostic, &inputs, &mut out);
    assert!(
      result.is_err(),
      "refusing after {budget} of {length} characters was reported as success"
    );
    assert!(
      !ends_styled(&out.offered),
      "refusing after {budget} characters left a style open: {:?}",
      out.offered
    );
  }
}

/// Bytes this thread has asked the allocator for.
///
/// The property below is a resource one — whether a row exists as a value before the writer is
/// consulted — and no amount of reading the output can see it, because a streamed row and a
/// materialised one produce the same bytes. That is the point of the fix and the reason this
/// harness is here instead of an assertion about text.
///
/// Thread-local rather than a single counter: the harness runs tests in parallel, and a shared one
/// would be measuring every other test at the same time. `const`-initialised so that reading it
/// cannot itself allocate and re-enter the allocator. `realloc` is counted as well as `alloc`,
/// because a `String` reaching its size mostly does it by growing.
struct Counting;

thread_local! {
  static ALLOCATED: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

fn allocated() -> usize {
  ALLOCATED.with(core::cell::Cell::get)
}

unsafe impl core::alloc::GlobalAlloc for Counting {
  unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
    let _ = ALLOCATED.try_with(|bytes| bytes.set(bytes.get() + layout.size()));
    unsafe { core::alloc::GlobalAlloc::alloc(&std::alloc::System, layout) }
  }

  unsafe fn dealloc(&self, pointer: *mut u8, layout: core::alloc::Layout) {
    unsafe { core::alloc::GlobalAlloc::dealloc(&std::alloc::System, pointer, layout) }
  }

  unsafe fn realloc(
    &self,
    pointer: *mut u8,
    layout: core::alloc::Layout,
    new_size: usize,
  ) -> *mut u8 {
    let _ = ALLOCATED.try_with(|bytes| bytes.set(bytes.get() + new_size));
    unsafe { core::alloc::GlobalAlloc::realloc(&std::alloc::System, pointer, layout, new_size) }
  }
}

#[global_allocator]
static COUNTING: Counting = Counting;

/// A writer that takes a fixed number of characters and then declines.
///
/// Holds no buffer, so anything the measurement below sees was allocated by the renderer.
struct Refusing {
  budget: usize,
  taken: usize,
}

impl core::fmt::Write for Refusing {
  fn write_str(&mut self, text: &str) -> core::fmt::Result {
    for _ in text.chars() {
      if self.taken == self.budget {
        return Err(core::fmt::Error);
      }
      self.taken += 1;
    }
    Ok(())
  }
}

#[test]
#[cfg_attr(
  miri,
  ignore = "measures allocation volume, which is not what Miri is checking, and walks tens of \
            thousands of cells to do it"
)]
fn a_row_is_written_to_the_caller_rather_than_built_before_it() {
  // Both rows of an excerpt are as long as the line is WIDE, and that is caller geometry: the
  // source length times the tab width. Bounding the tab width — which `LineCells` does — bounds the
  // multiplier and not the product, so the line below is 256 tabs and 65,536 cells from 257 bytes.
  //
  // Building either row into a `String` first spends that memory before `out` is ever asked, so a
  // caller whose writer is bounded or failing cannot decline what it never saw. `render` takes an
  // `fmt::Write` precisely so that it can.
  let text = format!("{}\n", "\t".repeat(256));
  let width = 256 * 256;
  let message = "a message";
  let diagnostic = Diagnostic::new(
    "mylang::test::rule",
    Severity::Error,
    &message,
    Location::new(0, Span::new(0, 256)),
  )
  .with_primary_label("all of it");
  let terminal = Terminal::plain().with_tab_width(256);
  let inputs = [Input::new(Source::new(&text))];

  // Well above what the renderer legitimately allocates — a handful of small vectors and one line
  // number — and far below either row, so neither a spurious pass nor a spurious failure is close.
  let ceiling = 8 * 1024;

  for (budget, row) in [(1_024, "the source row"), (70_000, "the marker row")] {
    let mut out = Refusing { budget, taken: 0 };
    let before = allocated();
    let result = terminal.render(&diagnostic, &inputs, &mut out);
    let spent = allocated() - before;

    assert!(
      result.is_err(),
      "{row}: the writer refused at {budget} of {width} cells and the render reported success"
    );
    assert_eq!(
      out.taken, budget,
      "{row}: the writer was not filled, so the refusal did not happen where this test aims it"
    );
    assert!(
      spent < ceiling,
      "{row}: refusing after {budget} cells still cost {spent} bytes, over the {ceiling} allowed — \
       a row of {width} was built before the writer was consulted"
    );
  }
}
