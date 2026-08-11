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
  // And the lines of one input are in SOURCE order, which is not the caller's: the primary here is
  // the LOWER of the two and is still announced by the `-->` line, while the row for line 2 is
  // drawn above the row for line 3.
  //
  // That is not a preference. A multi-line span is bracketed by a connector running down a
  // contiguous run of rows, so a row between its two ends has to be a line it covers; caller order
  // gives no such guarantee, and a bracket drawn over rows in caller order would claim to cover
  // lines it does not. Caller order keeps the jobs it can still do — which input leads, what the
  // `-->` points at, and the order of the marker rows under one line.
  //
  // The `|` that used to separate consecutive excerpts goes with it, for the same reason: a
  // separator between two lines of one bracket breaks the connector that is the whole point of it.
  // It stays where a block ends.
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
2 |   width: Int
  |   ----- first defined here
3 |   width: Int
  |   ^^^^^ redefined here
  |
"
  );
}

#[test]
fn two_labels_on_one_line_share_the_line_and_keep_their_own_marker_rows() {
  // The arrangement, pinned as a whole frame because what was wrong was the arrangement. Each label
  // used to be its own excerpt, so a line carrying two of them was printed twice — the same source
  // row, the same bounded window, once per label, with the second copy telling a reader nothing the
  // first had not.
  //
  // One row, two marker rows, and the primary first: its span starts AFTER the secondary's, so a
  // renderer ordering the rows by column would swap them.
  let text = "type Widget { width: Int, width: Float }\n";
  let message = "`width` is defined twice";
  let labels = [Label::new(
    Location::new(0, Span::new(14, 19)),
    "first defined here",
  )];
  let diagnostic = Diagnostic::new(
    "mylang::schema::duplicate-field",
    Severity::Error,
    &message,
    Location::new(0, Span::new(26, 31)),
  )
  .with_primary_label("redefined here")
  .with_labels(&labels);

  assert_eq!(
    render(&diagnostic, text, Some("widget.graphql")),
    "\
error[mylang::schema::duplicate-field]: `width` is defined twice
 --> widget.graphql:1:27
  |
1 | type Widget { width: Int, width: Float }
  |                           ^^^^^ redefined here
  |               ----- first defined here
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
  // The header says 6 where the caret sits in cell 9, and the two are both right — see
  // `the_arrow_line_and_the_marker_row_are_in_different_units`.
  assert_eq!(
    out,
    "\
error[mylang::test::tabbed]: an indented statement
 --> 1:6
  |
1 |     let x = 1;
  |         ^ this one
  |
"
  );
}

#[test]
fn the_arrow_line_and_the_marker_row_are_in_different_units() {
  // Two units in one frame, on purpose, and this is where they part company.
  //
  // The `-->` line is MACHINE-PARSED: `rustc` writes it and editors, IDEs and LSP clients read
  // `line:column` off it to move a cursor, in CHARACTERS. The marker row is read by a human looking
  // at the row above it, in the DISPLAY cells a terminal painted. On plain ASCII the two agree,
  // which is why the disagreement has to be constructed — and a line with a tab and a wide
  // character is the crate's own named worst case for exactly this.
  //
  // Asserted as a difference first. A renderer that reported display columns in the header would
  // satisfy any single-number assertion here by putting the same number in both places, and would
  // send every consumer to the wrong column on every tabbed or wide line, silently.
  let text = "\t日本語 x = 1;\n";
  let at = text.find('x').expect("the fixture has one");
  let message = "a name after a tab and three ideographs";
  let diagnostic = Diagnostic::new(
    "mylang::test::units",
    Severity::Error,
    &message,
    Location::new(0, Span::new(at, at + 1)),
  )
  .with_primary_label("this one");

  let out = render(&diagnostic, text, Some("widget.graphql"));

  // The header's number, as a consumer would parse it out.
  let header = out
    .lines()
    .find(|row| row.contains("-->"))
    .expect("a header row");
  let reported: u64 = header
    .rsplit(':')
    .next()
    .expect("a column")
    .parse()
    .expect("a number");

  // Layer 2's answer for the same offset, which is the unit every consumer of this line means.
  let source = Source::new(text);
  let character = source.position(at).column();
  assert_eq!(character, 6, "tab, three ideographs, a space, then `x`");
  assert_eq!(
    reported, character,
    "the header disagrees with `Position::column`, which is the number an editor would have \
     computed for itself\n{out}"
  );

  // And the marker, which is the other unit: the tab reaches the stop at 4, each ideograph is two
  // cells, then a space, so `x` is PAINTED in cell 12. Measured off the rendered rows with
  // `unicode-width`'s own answer rather than with painty's cluster arithmetic, so this is not the
  // renderer agreeing with itself.
  use unicode_width::UnicodeWidthStr;

  let marker_row = out
    .lines()
    .find(|row| row.contains('^'))
    .expect("a marker row");
  let source_row = out
    .lines()
    .find(|row| row.contains('x'))
    .expect("a source row");
  // Both rows carry the same gutter — `1 | ` and `  | ` — so the cell within the LINE is what is
  // after the bar and the space, in either.
  let after_the_bar = |row: &str| {
    let bar = row.find('|').expect("a gutter bar");
    row[bar + 2..].to_owned()
  };
  let drawn = after_the_bar(source_row);
  let display = drawn[..drawn.find('x').expect("the x")].width() as u64 + 1;
  let carets = after_the_bar(marker_row);
  let caret = carets.find('^').expect("the caret") as u64 + 1;

  assert_eq!(display, 12, "{out}");
  assert_eq!(
    caret, display,
    "the caret is not under the character it points at\n{out}"
  );
  assert_ne!(
    display, character,
    "the two units agree on this line, so it proves nothing about the distinction\n{out}"
  );

  assert_eq!(
    out,
    "\
error[mylang::test::units]: a name after a tab and three ideographs
 --> widget.graphql:1:6
  |
1 |     日本語 x = 1;
  |            ^ this one
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

// ── Multi-line spans ────────────────────────────────────────────────────────────────────────────
//
// The arrangement, in the two forms an opening takes and against each hazard the corpus already
// carried on the single-line axis. What is pinned here is appearance; that the right CELLS are
// marked whatever the rows turn out to be is `which_cells_are_marked_does_not_depend_on_how_the_
// rows_were_assigned` in `src/terminal/tests.rs`, and it is what makes re-blessing any of these
// safe.

#[test]
fn a_multi_line_span_is_bracketed_between_the_lines_it_covers() {
  // The compact opening: nothing else is drawn under line 2 and nothing but blanks precedes the
  // span on it, so the `/` in the margin opens the bracket and no corner row is needed.
  let text = "query Hero {\n  hero {\n    name\n    friends\n  }\n}\n";
  let message = "this selection set is nested too deeply";
  let start = text.find("hero {").expect("the fixture selects");
  let end = text.rfind("  }").expect("the fixture closes") + "  }".len();
  let diagnostic = Diagnostic::new(
    "mylang::query::too-deep",
    Severity::Error,
    &message,
    Location::new(0, Span::new(start, end)),
  )
  .with_primary_label("this selection set");

  let out = render(&diagnostic, text, Some("hero.graphql"));
  // Asserted against the shape this replaced, so the old behaviour cannot satisfy it: a span drawn
  // on the first line it touches shows line 2 and none of the rest.
  assert!(
    out.contains("5 | |   }"),
    "the closing line was not drawn\n{out}"
  );
  assert_eq!(
    out,
    "\
error[mylang::query::too-deep]: this selection set is nested too deeply
 --> hero.graphql:2:3
  |
2 | /   hero {
3 | |     name
4 | |     friends
5 | |   }
  | |___^ this selection set
  |
"
  );
}

#[test]
fn a_multi_line_span_that_has_text_before_it_opens_with_a_corner() {
  // The other opening. `let x = ` precedes the span, so a `/` in the margin would leave a reader
  // unable to tell where on the line the span begins — the underscore run says it exactly.
  let text = "let x = if a {\n  1\n} else {\n  2\n};\n";
  let message = "the branches disagree";
  let start = text.find("if a").expect("the fixture branches");
  let end = text.rfind('}').expect("the fixture closes") + 1;
  let diagnostic = Diagnostic::new(
    "mylang::type::branches",
    Severity::Error,
    &message,
    Location::new(0, Span::new(start, end)),
  )
  .with_primary_label("this expression");

  assert_eq!(
    render(&diagnostic, text, None),
    "\
error[mylang::type::branches]: the branches disagree
 --> 1:9
  |
1 |   let x = if a {
  |  _________^
2 | |   1
3 | | } else {
4 | |   2
5 | | };
  | |_^ this expression
  |
"
  );
}

#[test]
fn a_long_multi_line_span_shows_its_opening_and_says_what_it_left_out() {
  // Nine lines covered, six rows drawn. The `...` is not decoration: a row costs a bounded walk,
  // so a renderer drawing every line a span touches would cost the line count — which is the
  // caller's and unbounded — times that walk. What bounds it is drawn here.
  let text = "fragment F on Query {\n  a\n  b\n  c\n  d\n  e\n  f\n  g\n}\n";
  let message = "this fragment is never used";
  let diagnostic = Diagnostic::new(
    "mylang::query::unused-fragment",
    Severity::Warning,
    &message,
    Location::new(0, Span::new(0, text.len() - 1)),
  )
  .with_primary_label("declared here");

  let out = render(&diagnostic, text, None);
  let drawn = out
    .lines()
    .filter(|row| row.trim_start().starts_with(|c: char| c.is_ascii_digit()))
    .count();
  assert_ne!(
    drawn,
    text.lines().count(),
    "every line the span covers was drawn\n{out}"
  );
  assert_eq!(drawn, 5, "{out}");
  assert_eq!(
    out,
    "\
warning[mylang::query::unused-fragment]: this fragment is never used
 --> 1:1
  |
1 | / fragment F on Query {
2 | |   a
3 | |   b
4 | |   c
... |
9 | | }
  | |_^ declared here
  |
"
  );
}

#[test]
fn two_multi_line_spans_that_overlap_run_in_columns_of_their_own() {
  // One column would draw one bar where two spans are open, and a reader could not tell which
  // opening a closing belongs to — a wrong answer rather than a plainer one. The inner span opens
  // later, so it runs to the RIGHT of the outer, and the outer's closing corner crosses it: what
  // that says is that this bracket closes over the other one.
  let text = "outer (\n  inner (\n    x\n  )\n)\n";
  let message = "two of these disagree";
  let inner = text.find("inner").expect("the fixture nests");
  let inner_end = text.rfind("  )").expect("the fixture closes") + "  )".len();
  let labels = [Label::new(
    Location::new(0, Span::new(inner, inner_end)),
    "and this one",
  )];
  let diagnostic = Diagnostic::new(
    "mylang::type::nested",
    Severity::Error,
    &message,
    Location::new(
      0,
      Span::new(text.find('(').expect("the fixture opens"), text.len() - 1),
    ),
  )
  .with_primary_label("this one")
  .with_labels(&labels);

  assert_eq!(
    render(&diagnostic, text, None),
    "\
error[mylang::type::nested]: two of these disagree
 --> 1:7
  |
1 |    outer (
  |  ________^
2 | |/   inner (
3 | ||     x
4 | ||   )
  | ||___- and this one
5 | |  )
  | |__^ this one
  |
"
  );
}

#[test]
fn a_multi_line_span_opening_in_a_cjk_run_and_closing_after_a_tab() {
  // The two ends measured against DIFFERENT lines, and each against a different hazard: the
  // opening sits in a run of two-cell ideographs, the closing past a tab that is one character and
  // four cells. A renderer counting characters for either would put its end in the wrong cell, and
  // both numbers are pinned against the one it would produce.
  let text = "let \u{65e5}\u{672c}\u{8a9e} =\n\tvalue;\n";
  let message = "a binding written across two lines";
  let diagnostic = Diagnostic::new(
    "mylang::test::units",
    Severity::Error,
    &message,
    Location::new(
      0,
      Span::new(
        text.find('\u{65e5}').expect("the fixture has one"),
        text.len() - 1,
      ),
    ),
  )
  .with_primary_label("this binding");

  let out = render(&diagnostic, text, None);
  let closing = out
    .lines()
    .find(|row| row.contains("this binding"))
    .expect("a closing corner");
  // `let ` is four characters and four cells, so the OPENING agrees either way and is not evidence.
  // The closing is: `\tvalue;` is seven characters and ten cells, so a character-counting renderer
  // puts the caret three cells left of the semicolon it is about.
  let run = closing.find('_').expect("a closing run of underscores");
  assert_ne!(
    closing.find('^'),
    Some(run + "\tvalue;".chars().count()),
    "the closing end was placed by character, not by cell\n{out}"
  );
  assert_eq!(closing.find('^'), Some(run + 10), "{out}");
  assert_eq!(
    out,
    "\
error[mylang::test::units]: a binding written across two lines
 --> 1:5
  |
1 |   let 日本語 =
  |  _____^
2 | |     value;
  | |__________^ this binding
  |
"
  );
}

#[test]
fn a_multi_line_span_crossing_a_crlf_boundary() {
  // Two bytes per break and no column for either, so a bracket that counted the break as a
  // character would close one cell to the right of where it does.
  let text = "alpha\r\nbeta\r\ngamma\r\n";
  let message = "written on a machine that ends lines with two bytes";
  let diagnostic = Diagnostic::new(
    "mylang::test::crlf",
    Severity::Error,
    &message,
    Location::new(0, Span::new(2, 15)),
  )
  .with_primary_label("crossing two breaks");

  let out = render(&diagnostic, text, None);
  assert!(!out.contains('\r'), "a carriage return survived\n{out:?}");
  assert_eq!(
    out,
    "\
error[mylang::test::crlf]: written on a machine that ends lines with two bytes
 --> 1:3
  |
1 |   alpha
  |  ___^
2 | | beta
3 | | gamma
  | |__^ crossing two breaks
  |
"
  );
}

#[test]
fn a_multi_line_span_over_the_final_byte_of_a_file_with_no_trailing_newline() {
  // The single-line case is above; this is the same edge reached from a line the span did not
  // start on, where the closing end is the last byte of the text and there is no break after it.
  let text = "alpha\nbeta";
  let message = "the last thing in the file";
  let diagnostic = Diagnostic::new(
    "mylang::test::final-byte",
    Severity::Advice,
    &message,
    Location::new(0, Span::new(3, 10)),
  )
  .with_primary_label("to the very end");

  assert_eq!(
    render(&diagnostic, text, None),
    "\
advice[mylang::test::final-byte]: the last thing in the file
 --> 1:4
  |
1 |   alpha
  |  ____^
2 | | beta
  | |____^ to the very end
  |
"
  );
}

#[test]
fn a_span_that_swallows_its_own_trailing_newline_closes_on_the_line_it_covers() {
  // `Region::lines` stops before the line an exclusive end at column 1 falls on, because there is
  // nothing of the span to draw there. The bracket has to agree: a closing on line 3 would put a
  // marker under text the span does not cover, and would say the span is three lines long.
  let text = "aaa\nbbb\nccc\n";
  let message = "a span that swallowed its own newline";
  let diagnostic = Diagnostic::new(
    "mylang::test::swallowed",
    Severity::Error,
    &message,
    Location::new(0, Span::new(0, 8)),
  )
  .with_primary_label("two lines, not three");

  let out = render(&diagnostic, text, None);
  assert!(
    !out.contains("ccc"),
    "a line the span does not cover was drawn\n{out}"
  );
  assert_eq!(
    out,
    "\
error[mylang::test::swallowed]: a span that swallowed its own newline
 --> 1:1
  |
1 | / aaa
2 | | bbb
  | |___^ two lines, not three
  |
"
  );
}

#[test]
fn a_label_inside_a_multi_line_span_keeps_the_connector_beside_it() {
  // A marker row is a row like any other, so the bracket runs down its margin too — otherwise the
  // connector would appear to stop wherever a reader was told something.
  let text = "type Widget {\n  width: Int\n  width: Int\n}\n";
  let message = "`width` is defined twice";
  let labels = [Label::new(
    Location::new(0, Span::new(29, 34)),
    "redefined here",
  )];
  let diagnostic = Diagnostic::new(
    "mylang::schema::duplicate-field",
    Severity::Error,
    &message,
    Location::new(0, Span::new(0, text.len() - 1)),
  )
  .with_primary_label("in this type")
  .with_labels(&labels);

  assert_eq!(
    render(&diagnostic, text, None),
    "\
error[mylang::schema::duplicate-field]: `width` is defined twice
 --> 1:1
  |
1 | / type Widget {
2 | |   width: Int
3 | |   width: Int
  | |   ----- redefined here
4 | | }
  | |_^ in this type
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

#[test]
fn attributes_without_colour_is_a_palette_and_not_a_capability() {
  // The request `ColorCapability::None` is NOT: bold and underline, no colour. Three doc sites said
  // that rung granted it — that "attributes still work" there — and the renderer has always emitted
  // no escape at all, because that rung is a file or a pipe and a bold escape in a file is as wrong
  // as a red one.
  //
  // The docs were the defect, and this is the alternative they now point at, exercised rather than
  // asserted: a capability says what the MEDIUM carries, a palette says what to draw, so styling
  // without colour is `Theme::monochrome` at a capability that can carry escapes.
  let message = "a message";
  let diagnostic = Diagnostic::new(
    "mylang::test::rule",
    Severity::Error,
    &message,
    Location::new(0, Span::new(0, 3)),
  );
  let at = |capability| {
    let mut out = String::new();
    Terminal::with_palette(Theme::monochrome())
      .with_capability(capability)
      .render(&diagnostic, &[Input::new(Source::new("abc\n"))], &mut out)
      .expect("a String is writable");
    out
  };

  // Carried: bold, and nothing else. `1` is the only family monochrome asks for, so a colour
  // appearing here would mean the theme or the narrowing had started adding one.
  let sixteen = at(ColorCapability::Ansi16);
  let families = escape_families(&sixteen);
  assert!(
    !families.is_empty(),
    "monochrome emitted no attribute at a capability that carries them: {sixteen:?}"
  );
  assert!(
    families.iter().all(|family| family == "1" || family == "0"),
    "monochrome asked for something other than bold: {families:?}"
  );

  // And the rung that carries nothing carries nothing, whatever the palette wants. This is the half
  // the documentation used to deny.
  let none = at(ColorCapability::None);
  assert!(
    !none.contains('\u{1b}'),
    "a no-escape capability emitted one for an attribute: {none:?}"
  );
  assert_eq!(
    none,
    at(ColorCapability::None),
    "the no-escape rendering is not even stable"
  );
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
