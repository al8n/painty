//! What a serialiser owes, which is less than a renderer owes and is owed exactly.
//!
//! # The one property, and why the rest are geometry
//!
//! This surface draws no mark, chooses no row and takes no decision about the source. So the
//! invariant that a style changes appearance and not which source is marked is **inherited** —
//! there is nothing here for it to be false of — and the property this file exists for is the one
//! that makes that inheritance real: [`the_image_says_what_the_terminal_says`] reconstructs each
//! row of the document and compares it, character for character, with what
//! [`Terminal::render`](crate::terminal::Terminal::render) wrote, over every style.
//!
//! Everything else is about the **document**: that a row is as wide as the cells it holds, that
//! the viewport covers every row, that caller text comes back out verbatim, and that a class the
//! document uses is one the stylesheet knows about.

use core::fmt;

use super::{ADVANCE, BASELINE, Document, LINE_HEIGHT, PAD, ROLES, ansi256_to_rgb, class};
use crate::{
  Ansi16, Color, Diagnostic, Label, Location, Role, Severity, Source, Span, Style, Theme,
  terminal::{ColorCapability, Input, Terminal},
};

/// The harness is a `std` program whatever the crate under it is, so the strings these render into
/// come from there rather than from `alloc`, which this crate does not enable.
use std::{
  format,
  string::{String, ToString},
  vec::Vec,
};

/// Each style, named, so a failure says which one.
fn styles() -> [(&'static str, Terminal<Theme>); 4] {
  [
    ("rustc", Terminal::plain().like_rustc()),
    ("miette", Terminal::plain().like_miette()),
    ("ariadne", Terminal::plain().like_ariadne()),
    ("codespan", Terminal::plain().like_codespan()),
  ]
}

fn as_svg(terminal: &Terminal<Theme>, diagnostic: &Diagnostic<'_>, inputs: &[Input<'_>]) -> String {
  let mut out = String::new();
  terminal
    .render_svg(diagnostic, inputs, &mut out)
    .expect("a `String` accepts everything");
  out
}

fn as_terminal(
  terminal: &Terminal<Theme>,
  diagnostic: &Diagnostic<'_>,
  inputs: &[Input<'_>],
) -> String {
  let mut out = String::new();
  terminal
    .render(diagnostic, inputs, &mut out)
    .expect("a `String` accepts everything");
  out
}

/// The five entities the escaper writes, read back.
fn unescape(text: &str) -> String {
  let mut out = String::new();
  let mut rest = text;
  while let Some(at) = rest.find('&') {
    out.push_str(&rest[..at]);
    let from = &rest[at..];
    let (entity, character) = [
      ("&amp;", '&'),
      ("&lt;", '<'),
      ("&gt;", '>'),
      ("&quot;", '"'),
      ("&#39;", '\''),
    ]
    .into_iter()
    .find(|(entity, _)| from.starts_with(entity))
    .expect("the document contains only the five entities the escaper writes");
    out.push(character);
    rest = &from[entity.len()..];
  }
  out.push_str(rest);
  out
}

/// The value of one attribute of an element, which the tests read back off the document rather
/// than off the code that wrote it.
fn attribute(element: &str, name: &str) -> Option<String> {
  let key = format!(" {name}=\"");
  let at = element.find(&key)? + key.len();
  let rest = &element[at..];
  let end = rest.find('"').expect("an attribute value is quoted");
  Some(rest[..end].to_string())
}

/// One row of the image: which row of the render it is, and the text on it.
struct Row {
  index: usize,
  text: String,
}

/// Every row of a document, placed at the index its baseline decodes to.
///
/// Read off `y` rather than off the order the elements appear in, so that the placement is checked
/// rather than assumed: a row written at the wrong baseline lands at the wrong index and the
/// comparison against the terminal fails with it.
fn rows(document: &str) -> Vec<Row> {
  let mut rows = Vec::new();
  let mut rest = document;
  while let Some(at) = rest.find("<text ") {
    let from = &rest[at..];
    let open = from.find('>').expect("an element is closed");
    let close = from.find("</text>").expect("a row element is closed");
    let baseline: u64 = attribute(&from[..=open], "y")
      .expect("a row carries a baseline")
      .parse()
      .expect("a baseline is a number");
    let content = &from[open + 1..close];
    // The runs of a row, with their elements taken off: what is left is exactly the characters
    // the terminal put on that row.
    let mut text = String::new();
    let mut inner = content;
    while let Some(mark) = inner.find('<') {
      text.push_str(&unescape(&inner[..mark]));
      let after = inner[mark..].find('>').expect("an element is closed");
      inner = &inner[mark + after + 1..];
    }
    text.push_str(&unescape(inner));
    // Exactly, not by rounding. The first version of this took `/ LINE_HEIGHT` alone, and
    // integer division swallowed a baseline one unit off — every row still decoded to its own
    // index, so a planted shift was invisible. Deriving the index from the document only checks
    // the document if the derivation is injective.
    let below = baseline - PAD - BASELINE;
    assert_eq!(
      below % LINE_HEIGHT,
      0,
      "a row is drawn at {baseline}, which is not a baseline any row has"
    );
    rows.push(Row {
      index: usize::try_from(below / LINE_HEIGHT).expect("a row index fits a pointer"),
      text,
    });
    rest = &from[close..];
  }
  rows
}

/// The rows of a terminal render, without the empty one a trailing break leaves behind.
fn lines(render: &str) -> Vec<&str> {
  let mut lines: Vec<&str> = render.split('\n').collect();
  if lines.last().is_some_and(|last| last.is_empty()) {
    lines.pop();
  }
  lines
}

/// One diagnostic per shape worth drawing, named.
///
/// The axes are the ones that reach this surface: how many rows the render has, whether a bracket
/// puts something in the margin, whether a gap row appears, whether the cell arithmetic is
/// non-trivial, and whether there is any block at all.
fn corpus() -> Vec<(&'static str, Diagnostic<'static>, &'static [Input<'static>])> {
  const PLAIN: &str = "type Widget {\n  width: Int\n  width: String\n}\n";
  const LONG: &str = "a1\na2\na3\na4\na5\na6\na7\na8\na9\na10\na11\na12\na13\na14\n";
  const WIDE: &str = "let x = 1;\n\t幅 = \"漢字\";\nlet z = 3;\n";
  const MESSAGE: &str = "`width` is defined twice";

  static ONE: [Input<'static>; 1] = [Input::new(Source::new(PLAIN))];
  static TWO: [Input<'static>; 2] = [
    Input::new(Source::new(PLAIN)),
    Input::new(Source::new(LONG)),
  ];
  static WIDE_INPUT: [Input<'static>; 1] = [Input::new(Source::new(WIDE))];
  static NONE: [Input<'static>; 0] = [];

  static SECOND: [Label<'static>; 1] = [Label::new(
    Location::new(0, Span::new(16, 21)),
    "first defined here",
  )];
  static ELSEWHERE: [Label<'static>; 1] = [Label::new(
    Location::new(1, Span::new(0, 2)),
    "and over here",
  )];

  std::vec![
    (
      "one span",
      Diagnostic::new(
        "code",
        Severity::Error,
        &MESSAGE,
        Location::new(0, Span::new(29, 34)),
      )
      .with_primary_label("redefined here"),
      &ONE[..],
    ),
    (
      "two spans and a help",
      Diagnostic::new(
        "code",
        Severity::Warning,
        &MESSAGE,
        Location::new(0, Span::new(29, 34)),
      )
      .with_primary_label("redefined here")
      .with_labels(&SECOND)
      .with_help("rename one of them"),
      &ONE[..],
    ),
    (
      "a bracket and an elision",
      Diagnostic::new(
        "code",
        Severity::Advice,
        &MESSAGE,
        Location::new(0, Span::new(0, 41)),
      )
      .with_primary_label("this whole thing"),
      &ONE[..],
    ),
    (
      "a second input",
      Diagnostic::new(
        "code",
        Severity::Error,
        &MESSAGE,
        Location::new(0, Span::new(29, 34)),
      )
      .with_primary_label("here")
      .with_labels(&ELSEWHERE),
      &TWO[..],
    ),
    (
      "a tab and two wide clusters",
      Diagnostic::new(
        "code",
        Severity::Error,
        &MESSAGE,
        Location::new(0, Span::new(15, 21)),
      )
      .with_primary_label("here"),
      &WIDE_INPUT[..],
    ),
    (
      "nothing drawable",
      Diagnostic::new("code", Severity::Error, &MESSAGE, Location::entire(0))
        .with_help("there is no block here"),
      &NONE[..],
    ),
  ]
}

/// A diagnostic whose every caller-supplied string is markup.
///
/// Shared by two gates. The escaping one needs it for the obvious reason; the writer one needs it
/// because an escaped run is written in PIECES, and a refusal landing between two of them is the
/// only way a run can fail after it has already opened.
fn hostile() -> (Diagnostic<'static>, &'static [Input<'static>]) {
  const HOSTILE: &str = "</text><script>a & b</script>\nlet y = \"'quoted'\";\n";
  const MESSAGE: &str = "<b>&amp;</b> — a \"message\" that is also 'markup'";
  static INPUTS: [Input<'static>; 1] = [Input::new(Source::new(HOSTILE))];
  static LABELS: [Label<'static>; 1] = [Label::new(
    Location::new(0, Span::new(38, 47)),
    "</tspan>&<tspan>",
  )];
  (
    Diagnostic::new(
      "<code>&'\"</code>",
      Severity::Error,
      &MESSAGE,
      Location::new(0, Span::new(0, 7)),
    )
    // Long, with its first escapable character at the END, and that shape is deliberate: the
    // escaper writes the run up to it as ONE piece, so a budget can be exhausted by a write
    // longer than the eight bytes a closing element costs — which is the only window in which
    // a run that already opened can be left half-written with room to close it.
    .with_primary_label("a label long enough that its first escaped piece will not fit &")
    .with_labels(&LABELS)
    .with_help("close the <tag> & move on"),
    &INPUTS[..],
  )
}

#[test]
fn the_image_says_what_the_terminal_says() {
  // The property a serialiser is FOR. Every style, because the point of putting this under the
  // painter rather than beside the renderer is that a style costs nothing here — and a claim that
  // costs nothing is a claim nothing is checking until this asserts it.
  for (style, terminal) in styles() {
    for (case, diagnostic, inputs) in corpus() {
      let image = as_svg(&terminal, &diagnostic, inputs);
      let plain = as_terminal(&terminal, &diagnostic, inputs);
      let expected = lines(&plain);
      let drawn = rows(&image);

      for row in &drawn {
        let line = expected.get(row.index).unwrap_or_else(|| {
          panic!(
            "{style}/{case}: the image has a row {} the render does not",
            row.index
          )
        });
        assert_eq!(
          &row.text, line,
          "{style}/{case}: row {} of the image is not row {} of the render",
          row.index, row.index
        );
      }
      // And nothing was dropped. A row the terminal drew and the image did not is only allowed to
      // be a row with nothing on it, which is what costs no element.
      for (index, line) in expected.iter().enumerate() {
        if line.is_empty() {
          continue;
        }
        assert!(
          drawn.iter().any(|row| row.index == index),
          "{style}/{case}: row {index}, {line:?}, is in the render and not in the image"
        );
      }
    }
  }
}

#[test]
fn a_row_is_as_wide_as_the_cells_it_holds_and_the_viewport_covers_them_all() {
  // `textLength` is the whole of the placement model: the row is stretched to exactly its cells,
  // so a wrong length is every glyph on that row in the wrong place. Measured against the
  // TERMINAL's own row rather than against the surface's bookkeeping, which is why the corpus
  // carries a tab and two wide clusters.
  for (style, terminal) in styles() {
    for (case, diagnostic, inputs) in corpus() {
      let image = as_svg(&terminal, &diagnostic, inputs);
      let plain = as_terminal(&terminal, &diagnostic, inputs);
      let expected = lines(&plain);

      let mut widest = 0;
      for line in &expected {
        widest = widest.max(super::cells_from(line));
      }
      let rows_drawn = u64::try_from(expected.len()).expect("a row count fits sixty-four bits");

      for element in image.split("<text ").skip(1) {
        let open = element.find('>').expect("an element is closed");
        let head = &element[..open];
        let content = rows(&format!("<text {element}"));
        let row = content.first().expect("an element that opened has a row");
        let cells = super::cells_from(&row.text);
        match attribute(head, "textLength") {
          Some(length) => assert_eq!(
            length,
            (cells * ADVANCE).to_string(),
            "{style}/{case}: row {} is {cells} cells and is stretched to {length}",
            row.index
          ),
          None => assert_eq!(
            cells, 0,
            "{style}/{case}: row {} holds {cells} cells and was written without a length",
            row.index
          ),
        }
      }

      let view = attribute(&image, "viewBox").expect("a document declares a viewport");
      assert_eq!(
        view,
        format!(
          "0 0 {} {}",
          PAD * 2 + widest * ADVANCE,
          PAD * 2 + rows_drawn * LINE_HEIGHT
        ),
        "{style}/{case}: the viewport is not the extent of the render"
      );
      assert_eq!(
        attribute(&image, "width"),
        Some((PAD * 2 + widest * ADVANCE).to_string()),
        "{style}/{case}: the width and the viewport disagree"
      );
      assert_eq!(
        attribute(&image, "height"),
        Some((PAD * 2 + rows_drawn * LINE_HEIGHT).to_string()),
        "{style}/{case}: the height and the viewport disagree"
      );
    }
  }
}

#[test]
fn every_class_the_document_uses_is_one_the_stylesheet_knows() {
  // `class` is exhaustive over `Role`, so a role added later fails to compile there. What that
  // does NOT do is add the role to `ROLES`, and a role missing from `ROLES` has no rule — so this
  // reads the classes out of the finished document and checks each against the list the
  // stylesheet was written from.
  let known: Vec<&'static str> = ROLES.into_iter().map(class).collect();
  let mut seen = Vec::new();
  for (_, terminal) in styles() {
    for (_, diagnostic, inputs) in corpus() {
      let image = as_svg(&terminal, &diagnostic, inputs);
      for element in image.split("class=\"").skip(1) {
        let end = element.find('"').expect("a class attribute is quoted");
        let used = element[..end].to_string();
        assert!(
          known.iter().any(|name| *name == used),
          "the document uses {used:?}, which the stylesheet is not written for"
        );
        if !seen.contains(&used) {
          seen.push(used);
        }
      }
    }
  }
  // And the corpus reaches enough of them for the check above to be worth anything. Not all ten:
  // the three severities are one role each and a render has one severity.
  assert!(
    seen.len() >= 8,
    "the corpus only reached {} of the classes: {seen:?}",
    seen.len()
  );
}

#[test]
fn the_capability_does_not_reach_the_image() {
  // A capability says which escape sequences a TERMINAL may be sent. This medium has none, so a
  // caller who never told painty what its terminal supports still gets a coloured image — and the
  // four answers have to be one document, or the rung would be deciding appearance in a place it
  // knows nothing about.
  let (_, diagnostic, inputs) = corpus().remove(1);
  let base = Terminal::plain();
  let image = as_svg(&base, &diagnostic, inputs);
  // The half the four-way comparison below is structurally unable to see: a narrowing applied to
  // the stylesheet would empty it for EVERY capability, and four identical empty documents agree.
  // `Terminal::plain()` is `ColorCapability::None`, which is exactly the rung that would take this
  // away.
  assert!(
    image.contains(".painty-error{fill:#ff0000;font-weight:bold}"),
    "the default capability took the palette out of the stylesheet: {image}"
  );
  for capability in [
    ColorCapability::None,
    ColorCapability::Ansi16,
    ColorCapability::Ansi256,
    ColorCapability::TrueColor,
  ] {
    let other = as_svg(&base.with_capability(capability), &diagnostic, inputs);
    assert_eq!(image, other, "{capability:?} changed the image");
  }
  // And it is not vacuous: the same rungs really do change the terminal's own output.
  let mut plain = String::new();
  base
    .render(&diagnostic, inputs, &mut plain)
    .expect("a `String` accepts everything");
  let mut coloured = String::new();
  base
    .with_capability(ColorCapability::TrueColor)
    .render(&diagnostic, inputs, &mut coloured)
    .expect("a `String` accepts everything");
  assert_ne!(plain, coloured, "the capability changed nothing anywhere");
}

#[test]
fn caller_text_that_looks_like_markup_comes_back_out_verbatim() {
  // Every place a caller's own bytes reach the document: the source, the message, a label, the
  // origin and the help. The oracle is the terminal's own row, so this asserts round-tripping
  // rather than the absence of a substring — an escaper that DROPPED the hostile characters would
  // satisfy a grep and fail here.
  let (diagnostic, inputs) = hostile();
  let terminal = Terminal::plain();
  let image = as_svg(&terminal, &diagnostic, inputs);
  let plain = as_terminal(&terminal, &diagnostic, inputs);
  assert!(
    !image.contains("<script"),
    "the document carries the caller's element: {image}"
  );
  for (index, line) in lines(&plain).iter().enumerate() {
    if line.is_empty() {
      continue;
    }
    let row = rows(&image)
      .into_iter()
      .find(|row| row.index == index)
      .unwrap_or_else(|| panic!("row {index} is missing from the image"));
    assert_eq!(&row.text, line, "row {index} did not survive escaping");
  }
}

#[test]
fn a_palette_that_asks_for_nothing_writes_no_rule() {
  // A rule for a plain role is a rule that says nothing, and the source text is plain in every
  // built-in theme — so an embedder's own stylesheet reaches `painty-text` without having to
  // out-specify one of painty's.
  let (_, diagnostic, inputs) = corpus().remove(1);
  let image = as_svg(&Terminal::plain(), &diagnostic, inputs);
  assert!(
    image.contains("class=\"painty-text\""),
    "the source text carries no class at all: {image}"
  );
  assert!(
    !image.contains(".painty-text{"),
    "a role the theme asks nothing of was given a rule: {image}"
  );

  // Attributes with no colour still earn one, which is what separates "the theme wants nothing"
  // from "the theme wants no colour".
  let bold = Terminal::with_palette(Theme::monochrome());
  let image = as_svg(&bold, &diagnostic, inputs);
  assert!(
    image.contains(".painty-warning{font-weight:bold}"),
    "a colourless attribute was dropped with the colour: {image}"
  );
  assert!(
    !image.contains("fill:"),
    "a monochrome theme put a colour in the document: {image}"
  );
}

#[test]
fn a_background_is_the_one_property_the_medium_drops() {
  // Stated rather than discovered. SVG text has no background, and drawing one means a rectangle
  // as wide as a run that has not been written yet — which is the buffering this surface refuses.
  let (_, diagnostic, inputs) = corpus().remove(1);
  let theme = Theme::new().with(
    Role::Help,
    Style::plain()
      .with_background(Color::Rgb(1, 2, 3))
      .with_foreground(Color::Rgb(4, 5, 6)),
  );
  let image = as_svg(&Terminal::with_palette(theme), &diagnostic, inputs);
  assert!(
    image.contains(".painty-help{fill:#040506}"),
    "the foreground went missing with the background: {image}"
  );
  assert!(
    !image.contains("010203"),
    "a background reached the document: {image}"
  );
}

#[test]
fn a_colour_an_ansi_index_only_names_is_written_as_the_scheme_xterm_ships() {
  // The cube's levels are 0 and then 95 upwards in steps of 40; the ramp is 8 upwards in steps of
  // 10, and it stops at 238 rather than reaching white. Spot values at both ends of each region
  // and at the boundary between them, because an off-by-one in either formula is a colour that is
  // merely wrong rather than obviously so.
  assert_eq!(ansi256_to_rgb(0), Ansi16::Black.to_rgb());
  assert_eq!(ansi256_to_rgb(15), Ansi16::BrightWhite.to_rgb());
  assert_eq!(ansi256_to_rgb(16), (0, 0, 0));
  assert_eq!(ansi256_to_rgb(21), (0, 0, 255));
  assert_eq!(ansi256_to_rgb(196), (255, 0, 0));
  assert_eq!(ansi256_to_rgb(231), (255, 255, 255));
  assert_eq!(ansi256_to_rgb(232), (8, 8, 8));
  assert_eq!(ansi256_to_rgb(255), (238, 238, 238));

  // And the whole path, from a theme to a rule.
  let (_, diagnostic, inputs) = corpus().remove(1);
  let theme = Theme::new().with(
    Role::Help,
    Style::plain().with_foreground(Color::Ansi256(196)),
  );
  let image = as_svg(&Terminal::with_palette(theme), &diagnostic, inputs);
  assert!(
    image.contains(".painty-help{fill:#ff0000}"),
    "a 256-palette index did not reach the document as a colour: {image}"
  );
}

/// Accepts `left` bytes and then refuses, which is the writer `tests/writer_discipline.rs`
/// exists for: a document that had to be materialised to be measured could not be refused.
///
/// It keeps what it took, because "how much" is the weaker half. A surface that carried on after a
/// refusal would slip its LATER, shorter writes past a budget its earlier one exhausted, and the
/// result is a document with a hole in the middle rather than a short one.
struct Bounded {
  left: usize,
  written: usize,
  took: String,
}

impl Bounded {
  fn new(left: usize) -> Self {
    Self {
      left,
      written: 0,
      took: String::new(),
    }
  }
}

impl fmt::Write for Bounded {
  fn write_str(&mut self, text: &str) -> fmt::Result {
    if text.len() > self.left {
      return Err(fmt::Error);
    }
    self.left -= text.len();
    self.written += text.len();
    self.took.push_str(text);
    Ok(())
  }
}

/// One styled run, written to a document with `left` bytes of budget, and what the writer kept.
///
/// Driven at the surface's own interface rather than through a render, and that is the point.
/// [`Shown`](crate::terminal) hands this surface **one character at a time**, so a refusal through
/// a render always leaves fewer bytes than a closing element costs and the hazard below is
/// unreachable from there — by a property of the sanitizer rather than of this file, and one the
/// escaper beside it does not share. A run offered whole is what the contract actually says, so it
/// is what the contract is tested against.
fn one_run(left: usize, text: &str) -> String {
  use super::super::paint::Surface as _;

  let mut bounded = Bounded::new(left);
  {
    let mut document = Document::new(&mut bounded, &[64]);
    let _ = document
      .open(Role::PrimaryLabel, Style::plain())
      .and_then(|()| fmt::Write::write_str(&mut document, text))
      .and(document.close(Role::PrimaryLabel, Style::plain()));
    let _ = document.finish();
  }
  bounded.took
}

#[test]
fn a_run_refused_after_it_opened_is_never_closed() {
  // A run's element is written lazily, so it can open and then have its text refused — and the
  // balance call that follows is `Painter::styled_with`'s, which offers the closing half whatever
  // the body did. The terminal needs that call, because an SGR opener it never resets escapes into
  // the terminal's own state; this surface answers it by writing nothing, because a closing element
  // is SHORTER than the text that was just refused and would otherwise slip past the same budget.
  const TEXT: &str = "a run long enough that a closing element fits in what is left of the budget";
  let whole = one_run(usize::MAX, TEXT);
  for left in 0..whole.len() {
    let took = one_run(left, TEXT);
    assert!(
      whole.starts_with(&took),
      "at {left} bytes the surface wrote {took:?}, which is not a prefix of {whole:?}"
    );
  }
}

#[test]
fn a_writer_that_refuses_stops_the_document() {
  // The hostile diagnostic rather than a plain one, and that is the whole difference between a
  // gate that sees a stopped document and one that sees a HOLED one: an escaped run reaches the
  // writer in pieces, so a budget can run out between two of them — which is the one place a run
  // can fail after it has already opened.
  let (diagnostic, inputs) = hostile();
  let document = as_svg(&Terminal::plain(), &diagnostic, inputs);
  let whole = document.len();

  // EVERY budget, not three of them. Three was the first shape of this and it was blind: whether a
  // refusal lands in a markup write or in an escaped one is a function of the budget, so a guard
  // removed from one of the two is invisible to any sample that never stops in it. The document is
  // a kilobyte and a render is microseconds, so the sweep is the cheaper claim as well as the
  // stronger one.
  for left in 0..whole {
    let mut bounded = Bounded::new(left);
    assert!(
      Terminal::plain()
        .render_svg(&diagnostic, inputs, &mut bounded)
        .is_err(),
      "a writer that stops at {left} bytes accepted a document of {whole}"
    );
    assert!(
      bounded.written <= left,
      "the writer was handed more than it agreed to at {left}"
    );
    assert!(
      document.starts_with(&bounded.took),
      "what the writer took at {left} bytes is not a prefix of the document: {:?}",
      bounded.took
    );
  }

  // And it was STREAMED rather than assembled and offered. A document materialised first arrives
  // as one write that does not fit, so a writer with half the document's budget would accept none
  // of it — which is the property the two passes exist to keep and the one a byte count can see.
  let mut bounded = Bounded::new(whole / 2);
  let _ = Terminal::plain().render_svg(&diagnostic, inputs, &mut bounded);
  assert!(
    bounded.written > 0,
    "a writer offered half the document was handed none of it, so the document was materialised"
  );

  // The other end, so the sweep above is a statement about refusal rather than about this surface
  // being unable to finish: given the whole budget the document comes out entire.
  let mut exact = Bounded::new(whole);
  Terminal::plain()
    .render_svg(&diagnostic, inputs, &mut exact)
    .expect("a writer with the whole document's budget accepts it");
  assert_eq!(
    exact.took, document,
    "the document is not a function of the render"
  );
}

#[test]
fn no_row_of_a_render_carries_a_tab() {
  // The precondition `cells_from` rests on rather than defends: a tab is the one unit whose width
  // is a function of where it already is, and this surface measures fragments with no running
  // column. It is sound because nothing delivers one — source text is expanded against its stops
  // before it is written, and every other string goes through the sanitizer, which replaces
  // U+0009 with a one-cell picture. Asserted here because a branch for it in `cells_from` would
  // be a branch no plant could kill.
  const TABBED: &str = "\tlet\tx\t= 1;\n\t\tlet y = 2;\n";
  static INPUTS: [Input<'static>; 1] = [Input::new(Source::new(TABBED))];
  const MESSAGE: &str = "a\tmessage\twith\ttabs";
  let labels = [Label::new(Location::new(0, Span::new(14, 24)), "a\tlabel")];
  let diagnostic = Diagnostic::new(
    "co\tde",
    Severity::Error,
    &MESSAGE,
    Location::new(0, Span::new(1, 4)),
  )
  .with_primary_label("here\ttoo")
  .with_labels(&labels)
  .with_help("a\thelp\tline");

  for (style, terminal) in styles() {
    let image = as_svg(&terminal, &diagnostic, &INPUTS[..]);
    for row in rows(&image) {
      assert!(
        !row.text.contains('\t'),
        "{style}: row {} reached the surface with a tab in it: {:?}",
        row.index,
        row.text
      );
    }
    // Not vacuous: the tabs are in the render, as expansion and as pictures.
    assert!(
      image.contains('\u{2409}'),
      "{style}: the corpus's tabs did not reach the document at all"
    );
  }
}
