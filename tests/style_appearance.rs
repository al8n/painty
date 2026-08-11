//! What each presentation looks like, over one diagnostic set that all of them draw.
//!
//! # Why one table and not a file per style
//!
//! The design's gate for this phase is "the styles render the same diagnostic set, goldens per
//! style". A file of goldens each would make that a claim somebody has to keep true by hand — a
//! case added to one and not the others is invisible, and the style that is missing coverage is
//! the one nobody is looking at. Here the set is a list and each entry carries every render, so a
//! case cannot exist for one style only, and a style cannot exist without one.
//!
//! # These come after the invariants
//!
//! Same ordering as `tests/terminal_appearance.rs`, and for the same reason: a golden written
//! first becomes the thing the invariants are adjusted to agree with, and then both agree with the
//! defect. What is pinned here is *appearance* on a base held by
//! `which_cells_are_marked_is_a_function_of_the_span_alone` and by the structural invariants in
//! `src/terminal/tests.rs`. Re-blessing one of these should be cheap, and it is only safe because
//! the arithmetic underneath is held by something else.
//!
//! # The offsets are searched for, not written down
//!
//! Same discipline as `tests/common/mod.rs`, and the first draft of this file is why it is
//! restated: two spans were transcribed by hand, both were wrong by a character or two, and both
//! produced a perfectly plausible golden that pinned the wrong thing. A hardcoded offset makes a
//! test assert something while staying green.

#![cfg(feature = "terminal")]

use painty::{
  Diagnostic, Label, Location, Severity, Source, Span, Theme,
  terminal::{Input, Terminal},
};

/// One diagnostic and what each style draws for it.
///
/// Held in pieces rather than as a [`Diagnostic`], because a `Diagnostic` borrows its labels and
/// the labels are built from offsets this file searches for at run time.
struct Case {
  why: &'static str,
  text: &'static str,
  origin: Option<&'static str>,
  code: &'static str,
  severity: Severity,
  message: &'static str,
  /// `None` for a position the caller could not give — [`Location::entire`].
  primary: Option<Span>,
  primary_label: Option<&'static str>,
  labels: Vec<(Span, &'static str)>,
  help: Option<&'static str>,
  rustc: &'static str,
  miette: &'static str,
  ariadne: &'static str,
  codespan: &'static str,
}

/// The span of `needle`'s first occurrence in `text`.
fn first(text: &str, needle: &str) -> Span {
  let at = text
    .find(needle)
    .unwrap_or_else(|| panic!("{text:?} does not contain {needle:?}"));
  Span::new(at, at + needle.len())
}

/// The span of `needle`'s last occurrence in `text`.
fn last(text: &str, needle: &str) -> Span {
  let at = text
    .rfind(needle)
    .unwrap_or_else(|| panic!("{text:?} does not contain {needle:?}"));
  Span::new(at, at + needle.len())
}

/// The span reaching from `opens`' first occurrence to the end of `closes`' last.
fn between(text: &str, opens: &str, closes: &str) -> Span {
  Span::new(first(text, opens).start(), last(text, closes).end())
}

fn cases() -> Vec<Case> {
  const SCHEMA: &str = "type Widget {\n  width: Int\n  width: Int\n}\n";
  const ONE_LINE: &str = "type Widget { width: Int, width: Float }\n";
  const HERO: &str = "query Hero {\n  hero {\n    name\n    friends\n  }\n}\n";
  const BRANCHES: &str = "let x = if a {\n  1\n} else {\n  2\n};\n";
  const FRAGMENT: &str = "fragment F on Query {\n  a\n  b\n  c\n  d\n  e\n  f\n  g\n}\n";
  const NESTED: &str = "outer (\n  inner (\n    x\n  )\n)\n";
  const WIDE: &str = "\tlet x = \u{65e5}\u{672c};\n";
  const SHARED: &str = "  identifier(\n    body\n  )\n";
  const BORROWS: &str = "let a = &x;\nlet b = &y;\nuse(a);\nuse(b);\n";

  vec![
    Case {
      why: "one label, and the whole header with it",
      text: SCHEMA,
      origin: Some("widget.graphql"),
      code: "mylang::schema::duplicate-field",
      severity: Severity::Error,
      message: "`width` is defined twice",
      primary: Some(last(SCHEMA, "width")),
      primary_label: Some("redefined here"),
      labels: Vec::new(),
      help: Some("rename one of the two definitions"),
      rustc: "\
error[mylang::schema::duplicate-field]: `width` is defined twice
 --> widget.graphql:3:3
  |
3 |   width: Int
  |   ^^^^^ redefined here
  |
  = help: rename one of the two definitions
",
      miette: "\
mylang::schema::duplicate-field

  × `width` is defined twice
   ╭─[widget.graphql:3:3]
 3 │   width: Int
   ·   ━━┳━━
   ·     ┗━━ redefined here
   ╰────
  help: rename one of the two definitions
",
      ariadne: "\
[mylang::schema::duplicate-field] Error: `width` is defined twice
   ╭─[ widget.graphql:3:3 ]
   │
 3 │   width: Int
   │   ──┬──
   │     ╰── redefined here
   │
   │ Help: rename one of the two definitions
───╯
",
      codespan: "\
error[mylang::schema::duplicate-field]: `width` is defined twice
  ┌─ widget.graphql:3:3
  │
3 │   width: Int
  │   ^^^^^ redefined here
  │
  = rename one of the two definitions
",
    },
    Case {
      why: "a primary and a secondary, which each style distinguishes its own way",
      text: SCHEMA,
      origin: None,
      code: "mylang::schema::duplicate-field",
      severity: Severity::Error,
      message: "`width` is defined twice",
      primary: Some(last(SCHEMA, "width")),
      primary_label: Some("redefined here"),
      labels: vec![(first(SCHEMA, "width"), "first defined here")],
      help: None,
      rustc: "\
error[mylang::schema::duplicate-field]: `width` is defined twice
 --> 3:3
  |
2 |   width: Int
  |   ----- first defined here
3 |   width: Int
  |   ^^^^^ redefined here
  |
",
      miette: "\
mylang::schema::duplicate-field

  × `width` is defined twice
   ╭─[3:3]
 2 │   width: Int
   ·   ──┬──
   ·     ╰── first defined here
 3 │   width: Int
   ·   ━━┳━━
   ·     ┗━━ redefined here
   ╰────
",
      ariadne: "\
[mylang::schema::duplicate-field] Error: `width` is defined twice
   ╭─[ 3:3 ]
   │
 2 │   width: Int
   │   ──┬──
   │     ╰── first defined here
 3 │   width: Int
   │   ──┬──
   │     ╰── redefined here
───╯
",
      codespan: "\
error[mylang::schema::duplicate-field]: `width` is defined twice
  ┌─ 3:3
  │
2 │   width: Int
  │   ----- first defined here
3 │   width: Int
  │   ^^^^^ redefined here
  │
",
    },
    Case {
      why: "two labels on one line, in the caller's order and not by column",
      text: ONE_LINE,
      origin: Some("widget.graphql"),
      code: "mylang::schema::duplicate-field",
      severity: Severity::Error,
      message: "`width` is defined twice",
      primary: Some(last(ONE_LINE, "width")),
      primary_label: Some("redefined here"),
      labels: vec![(first(ONE_LINE, "width"), "first defined here")],
      help: None,
      rustc: "\
error[mylang::schema::duplicate-field]: `width` is defined twice
 --> widget.graphql:1:27
  |
1 | type Widget { width: Int, width: Float }
  |                           ^^^^^ redefined here
  |               ----- first defined here
  |
",
      miette: "\
mylang::schema::duplicate-field

  × `width` is defined twice
   ╭─[widget.graphql:1:27]
 1 │ type Widget { width: Int, width: Float }
   ·                           ━━┳━━
   ·                             ┗━━ redefined here
   ·               ──┬──
   ·                 ╰── first defined here
   ╰────
",
      ariadne: "\
[mylang::schema::duplicate-field] Error: `width` is defined twice
   ╭─[ widget.graphql:1:27 ]
   │
 1 │ type Widget { width: Int, width: Float }
   │                           ──┬──
   │                             ╰── redefined here
   │               ──┬──
   │                 ╰── first defined here
───╯
",
      codespan: "\
error[mylang::schema::duplicate-field]: `width` is defined twice
  ┌─ widget.graphql:1:27
  │
1 │ type Widget { width: Int, width: Float }
  │                           ^^^^^ redefined here
  │               ----- first defined here
  │
",
    },
    Case {
      why: "a multi-line span whose opening line holds nothing before it",
      text: HERO,
      origin: Some("hero.graphql"),
      code: "mylang::query::too-deep",
      severity: Severity::Error,
      message: "this selection set is nested too deeply",
      primary: Some(between(HERO, "hero {", "  }")),
      primary_label: Some("this selection set"),
      labels: Vec::new(),
      help: None,
      rustc: "\
error[mylang::query::too-deep]: this selection set is nested too deeply
 --> hero.graphql:2:3
  |
2 | /   hero {
3 | |     name
4 | |     friends
5 | |   }
  | |___^ this selection set
  |
",
      miette: "\
mylang::query::too-deep

  × this selection set is nested too deeply
   ╭─[hero.graphql:2:3]
 2 │ ┏   hero {
 3 │ ┃     name
 4 │ ┃     friends
 5 │ ┣   }
   · ┗━━━━ this selection set
   ╰────
",
      ariadne: "\
[mylang::query::too-deep] Error: this selection set is nested too deeply
   ╭─[ hero.graphql:2:3 ]
   │
 2 │ ╭─▶   hero {
 3 │ │       name
 4 │ │       friends
 5 │ ├─▶   }
   │ │
   │ ╰──── this selection set
───╯
",
      codespan: "\
error[mylang::query::too-deep]: this selection set is nested too deeply
  ┌─ hero.graphql:2:3
  │
2 │ ╭   hero {
3 │ │     name
4 │ │     friends
5 │ │   }
  │ ╰───^ this selection set
  │
",
    },
    Case {
      why: "a multi-line span with text before it, which one style needs a row for",
      text: BRANCHES,
      origin: None,
      code: "mylang::type::branches",
      severity: Severity::Error,
      message: "the branches disagree",
      primary: Some(between(BRANCHES, "if a", "}")),
      primary_label: Some("this expression"),
      labels: Vec::new(),
      help: None,
      rustc: "\
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
",
      miette: "\
mylang::type::branches

  × the branches disagree
   ╭─[1:9]
 1 │ ┏ let x = if a {
 2 │ ┃   1
 3 │ ┃ } else {
 4 │ ┃   2
 5 │ ┣ };
   · ┗━━━━ this expression
   ╰────
",
      ariadne: "\
[mylang::type::branches] Error: the branches disagree
   ╭─[ 1:9 ]
   │
 1 │ ╭─▶ let x = if a {
 2 │ │     1
 3 │ │   } else {
 4 │ │     2
 5 │ ├─▶ };
   │ │
   │ ╰──── this expression
───╯
",
      codespan: "\
error[mylang::type::branches]: the branches disagree
  ┌─ 1:9
  │
1 │   let x = if a {
  │ ╭─────────^
2 │ │   1
3 │ │ } else {
4 │ │   2
5 │ │ };
  │ ╰─^ this expression
  │
",
    },
    Case {
      why: "long enough that the middle is elided",
      text: FRAGMENT,
      origin: None,
      code: "mylang::query::unused-fragment",
      severity: Severity::Warning,
      message: "this fragment is never used",
      primary: Some(between(FRAGMENT, "fragment", "}")),
      primary_label: Some("declared here"),
      labels: Vec::new(),
      help: None,
      rustc: "\
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
",
      miette: "\
mylang::query::unused-fragment

  ⚠ this fragment is never used
   ╭─[1:1]
 1 │ ┏ fragment F on Query {
 2 │ ┃   a
 3 │ ┃   b
 4 │ ┃   c
   ⋮ ┃
 9 │ ┣ }
   · ┗━━━━ declared here
   ╰────
",
      ariadne: "\
[mylang::query::unused-fragment] Warning: this fragment is never used
   ╭─[ 1:1 ]
   │
 1 │ ╭─▶ fragment F on Query {
 2 │ │     a
 3 │ │     b
 4 │ │     c
   ┆ ┆
 9 │ ├─▶ }
   │ │
   │ ╰──── declared here
───╯
",
      codespan: "\
warning[mylang::query::unused-fragment]: this fragment is never used
  ┌─ 1:1
  │
1 │ ╭ fragment F on Query {
2 │ │   a
3 │ │   b
4 │ │   c
  · │
9 │ │ }
  │ ╰─^ declared here
  │
",
    },
    Case {
      why: "two multi-line spans open at once, which is what needs a second column",
      text: NESTED,
      origin: None,
      code: "mylang::type::nested",
      severity: Severity::Error,
      message: "two of these disagree",
      primary: Some(between(NESTED, "(", ")")),
      primary_label: Some("this one"),
      labels: vec![(between(NESTED, "inner", "  )"), "and this one")],
      help: None,
      rustc: "\
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
",
      miette: "\
mylang::type::nested

  × two of these disagree
   ╭─[1:7]
 1 │ ┏  outer (
 2 │ ┃╭   inner (
 3 │ ┃│     x
 4 │ ┃├   )
   · ┃╰──── and this one
 5 │ ┣  )
   · ┗━━━━━ this one
   ╰────
",
      ariadne: "\
[mylang::type::nested] Error: two of these disagree
   ╭─[ 1:7 ]
   │
 1 │ ╭──▶ outer (
 2 │ │╭─▶   inner (
 3 │ ││       x
 4 │ │├─▶   )
   │ ││
   │ │╰──── and this one
 5 │ ├──▶ )
   │ │
   │ ╰───── this one
───╯
",
      codespan: "\
error[mylang::type::nested]: two of these disagree
  ┌─ 1:7
  │
1 │    outer (
  │ ╭────────^
2 │ │╭   inner (
3 │ ││     x
4 │ ││   )
  │ │╰───' and this one
5 │ │  )
  │ ╰──^ this one
  │
",
    },
    Case {
      why: "a tab and two-cell ideographs, where a cell is not a character",
      text: WIDE,
      origin: None,
      code: "mylang::test::wide",
      severity: Severity::Advice,
      message: "a wide name behind a tab",
      primary: Some(first(WIDE, "\u{65e5}\u{672c}")),
      primary_label: Some("two cells each"),
      labels: Vec::new(),
      help: None,
      rustc: "\
advice[mylang::test::wide]: a wide name behind a tab
 --> 1:10
  |
1 |     let x = 日本;
  |             ^^^^ two cells each
  |
",
      miette: "\
mylang::test::wide

  ☞ a wide name behind a tab
   ╭─[1:10]
 1 │     let x = 日本;
   ·             ━━┳━
   ·               ┗━━ two cells each
   ╰────
",
      ariadne: "\
[mylang::test::wide] Advice: a wide name behind a tab
   ╭─[ 1:10 ]
   │
 1 │     let x = 日本;
   │             ──┬─
   │               ╰── two cells each
───╯
",
      codespan: "\
advice[mylang::test::wide]: a wide name behind a tab
  ┌─ 1:10
  │
1 │     let x = 日本;
  │             ^^^^ two cells each
  │
",
    },
    Case {
      why: "nothing to point at, so neither style opens a block",
      text: "type Widget {}\n",
      origin: None,
      code: "mylang::input::synthesized",
      severity: Severity::Warning,
      message: "this document was generated, so it has no positions",
      primary: None,
      primary_label: None,
      labels: Vec::new(),
      help: None,
      rustc: "\
warning[mylang::input::synthesized]: this document was generated, so it has no positions
",
      miette: "\
mylang::input::synthesized

  ⚠ this document was generated, so it has no positions
",
      ariadne: "\
[mylang::input::synthesized] Warning: this document was generated, so it has no positions
",
      codespan: "\
warning[mylang::input::synthesized]: this document was generated, so it has no positions
",
    },
    // What closing the compact form's residual looks like. The `/` in the margin used to be a
    // corner row reaching to the opening cell, and which of the two a reader got depended on
    // whether the line happened to carry another label — so this render is the one place the
    // change is visible, and pinning it is what stops it drifting back.
    Case {
      why: "a multi-line span sharing its opening line with a label",
      text: SHARED,
      origin: None,
      code: "mylang::type::arity",
      severity: Severity::Error,
      message: "this call takes one argument",
      primary: Some(between(SHARED, "identifier", ")")),
      primary_label: Some("this call"),
      labels: vec![(first(SHARED, "identifier"), "declared with none")],
      help: None,
      rustc: "\
error[mylang::type::arity]: this call takes one argument
 --> 1:3
  |
1 | /   identifier(
  | |   ---------- declared with none
2 | |     body
3 | |   )
  | |___^ this call
  |
",
      miette: "\
mylang::type::arity

  × this call takes one argument
   ╭─[1:3]
 1 │ ┏   identifier(
   · ┃   ─────┬────
   · ┃        ╰── declared with none
 2 │ ┃     body
 3 │ ┣   )
   · ┗━━━━ this call
   ╰────
",
      ariadne: "\
[mylang::type::arity] Error: this call takes one argument
   ╭─[ 1:3 ]
   │
 1 │ ╭─▶   identifier(
   │ │     ─────┬────
   │ │          ╰── declared with none
 2 │ │       body
 3 │ ├─▶   )
   │ │
   │ ╰──── this call
───╯
",
      codespan: "\
error[mylang::type::arity]: this call takes one argument
  ┌─ 1:3
  │
1 │ ╭   identifier(
  │ │   ---------- declared with none
2 │ │     body
3 │ │   )
  │ ╰───^ this call
  │
",
    },
    // Two spans that overlap without nesting, which is the only arrangement where a corner runs
    // through a column that is not empty — and where the styles disagree about what to do with the
    // bar it crosses. Nothing else in this table reaches it: nested spans close innermost-first, so
    // the column a closing corner passes is always free by the time it passes.
    Case {
      why: "two multi-line spans that partially overlap, so a corner crosses a bar still in use",
      text: BORROWS,
      origin: None,
      code: "mylang::borrow::overlapping",
      severity: Severity::Error,
      message: "these two borrows overlap",
      primary: Some(between(BORROWS, "&x", "use(a)")),
      primary_label: Some("this one"),
      labels: vec![(between(BORROWS, "&y", "use(b)"), "and this one")],
      help: None,
      rustc: "\
error[mylang::borrow::overlapping]: these two borrows overlap
 --> 1:9
  |
1 |    let a = &x;
  |  __________^
2 | |  let b = &y;
  | | _________-
3 | || use(a);
  | |_______^ this one
4 |  | use(b);
  |  |______- and this one
  |
",
      miette: "\
mylang::borrow::overlapping

  × these two borrows overlap
   ╭─[1:9]
 1 │ ┏  let a = &x;
 2 │ ┃╭ let b = &y;
 3 │ ┣│ use(a);
   · ┗━━━━━ this one
 4 │  ├ use(b);
   ·  ╰──── and this one
   ╰────
",
      ariadne: "\
[mylang::borrow::overlapping] Error: these two borrows overlap
   ╭─[ 1:9 ]
   │
 1 │ ╭──▶ let a = &x;
 2 │ │╭─▶ let b = &y;
 3 │ ├──▶ use(a);
   │ ││
   │ ╰───── this one
 4 │  ├─▶ use(b);
   │  │
   │  ╰──── and this one
───╯
",
      codespan: "\
error[mylang::borrow::overlapping]: these two borrows overlap
  ┌─ 1:9
  │
1 │    let a = &x;
  │ ╭──────────^
2 │ │  let b = &y;
  │ │╭─────────'
3 │ ││ use(a);
  │ ╰│──────^ this one
4 │  │ use(b);
  │  ╰──────' and this one
  │
",
    },
  ]
}

fn render(case: &Case, terminal: Terminal<Theme>) -> String {
  let labels: Vec<Label<'_>> = case
    .labels
    .iter()
    .map(|(span, text)| Label::new(Location::new(0, *span), text))
    .collect();
  let primary = case
    .primary
    .map_or_else(|| Location::entire(0), |span| Location::new(0, span));
  let mut diagnostic =
    Diagnostic::new(case.code, case.severity, &case.message, primary).with_labels(&labels);
  if let Some(label) = case.primary_label {
    diagnostic = diagnostic.with_primary_label(label);
  }
  if let Some(help) = case.help {
    diagnostic = diagnostic.with_help(help);
  }

  let mut input = Input::new(Source::new(case.text));
  if let Some(origin) = case.origin {
    input = input.with_origin(origin);
  }
  let mut out = String::new();
  terminal
    .with_tab_width(4)
    .render(&diagnostic, &[input], &mut out)
    .expect("a String never fails to be written to");
  out
}

/// Every style, paired with the column of the table it is pinned by.
///
/// One list rather than an assertion per style, so that adding a style is adding a row here and a
/// column there — and a style added to only one of the two does not compile.
fn styles(case: &Case) -> Vec<(&'static str, Terminal<Theme>, &'static str)> {
  vec![
    ("rustc", Terminal::plain().like_rustc(), case.rustc),
    ("miette", Terminal::plain().like_miette(), case.miette),
    ("ariadne", Terminal::plain().like_ariadne(), case.ariadne),
    ("codespan", Terminal::plain().like_codespan(), case.codespan),
  ]
}

#[test]
fn each_style_draws_the_whole_set() {
  for case in cases() {
    for (name, terminal, expected) in styles(&case) {
      assert_eq!(
        render(&case, terminal),
        expected,
        "the {name} style: {}",
        case.why
      );
    }
  }
}

#[test]
fn no_two_styles_draw_the_same_bytes() {
  // A style that renders identically to one already here is not a style, it is a second name for
  // one — and the rule this phase works to is that what has no divergence behind it does not get a
  // file.
  //
  // Over the whole SET rather than case by case, and the difference is a finding rather than a
  // convenience. Two styles can agree on a diagnostic that exercises nothing they differ in, and
  // the corpus holds one: a diagnostic naming no position it can draw is a header and nothing
  // else, and `codespan`'s header is `rustc`'s to the byte — two renderers designed independently
  // of one another arrived at `error[code]: message`. Written per case, this would have to be
  // satisfied by inventing a difference in the one method where two of the styles genuinely have
  // none, which is the opposite of what these files exist to measure.
  //
  // What it does still catch is a whole style that is a copy: for two styles to pass here they
  // have to differ SOMEWHERE in ten diagnostics that between them cover a header, a location line,
  // one label and two, a compact and a non-compact multi-line opening, two open at once, an
  // elision, a help line, and a block that is never opened.
  let all = cases();
  let names: Vec<&str> = styles(&all[0]).iter().map(|(name, _, _)| *name).collect();
  for (index, name) in names.iter().enumerate() {
    for (offset, other) in names[index + 1..].iter().enumerate() {
      let differs = all
        .iter()
        .any(|case| styles(case)[index].2 != styles(case)[index + 1 + offset].2);
      assert!(
        differs,
        "{name} and {other} drew the same bytes for every case in the set, so one of them is not \
         a style"
      );
    }
  }
}

#[test]
fn the_default_is_the_style_rustc_uses() {
  // `like_rustc` is a no-op on a fresh renderer, and it has to be: `Terminal::plain()` was the
  // whole API before there was a choice, and every caller of it is asking for what it drew then.
  for case in cases() {
    assert_eq!(
      render(&case, Terminal::plain()),
      render(&case, Terminal::plain().like_rustc()),
      "{}",
      case.why
    );
  }
}

#[test]
fn a_style_is_the_last_one_asked_for() {
  // Not a builder that accumulates. A caller mapping a `--style` flag through a match arm per
  // variant writes one call per arm, and an option that could not be changed back would make the
  // order of two arms matter.
  let cases = cases();
  let case = &cases[0];
  assert_eq!(
    render(case, Terminal::plain().like_miette().like_rustc()),
    case.rustc
  );
  assert_eq!(
    render(case, Terminal::plain().like_rustc().like_miette()),
    case.miette
  );
  assert_eq!(
    render(case, Terminal::plain().like_miette().like_ariadne()),
    case.ariadne
  );
  assert_eq!(
    render(case, Terminal::plain().like_ariadne().like_rustc()),
    case.rustc
  );
}
