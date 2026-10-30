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
//! Everything else is about the **document**: that every glyph is at the cell the terminal gave it,
//! that the viewport covers every row, that caller text comes back out verbatim and parseable, and
//! that a class the document uses is one the stylesheet knows about.

use core::fmt;

use super::{ADVANCE, BASELINE, Document, LINE_HEIGHT, PAD, ROLES, ansi256_to_rgb, class};
use crate::{
  Ansi16, Color, Diagnostic, Label, Location, Role, Severity, Source, Span, Style, Theme,
  terminal::{ColorCapability, Input, Terminal, width::cells_from},
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

/// One placement unit of a row: where the document says it is, what it draws, and under which
/// class.
struct Unit {
  x: u64,
  class: Option<String>,
  text: String,
}

/// One row of the image: which row of the render it is, the text on it, and every unit of it.
struct Row {
  index: usize,
  text: String,
  units: Vec<Unit>,
}

/// The units of one row's element content.
///
/// Total over the content rather than a search through it, and that is what makes it a gate as well
/// as a parser: **every character of a row has to be inside a unit element**, so a surface that
/// wrote text anywhere else — or an element this does not know about — fails here instead of
/// quietly producing a row that still reads correctly.
fn units(content: &str) -> Vec<Unit> {
  let mut units = Vec::new();
  let mut class: Option<String> = None;
  let mut rest = content;
  while let Some(at) = rest.find('<') {
    assert!(
      rest[..at].is_empty(),
      "a row carries {:?} outside any unit",
      &rest[..at]
    );
    let from = &rest[at..];
    if let Some(after) = from.strip_prefix("<tspan x=\"") {
      let quote = after.find('"').expect("an attribute value is quoted");
      let x: u64 = after[..quote]
        .parse()
        .expect("a coordinate is a whole number");
      // Past the closing quote and the `>` after it.
      let body = &after[quote + 2..];
      let end = body.find('<').expect("a unit element is closed");
      // The escaping census for `>`, made structural: a raw one is well-formed XML, so nothing
      // downstream would notice it, and the round trip through `unescape` reproduces the caller's
      // row either way. Here is the only place that can tell the two apart.
      assert!(
        !body[..end].contains('>'),
        "a unit carries an unescaped `>`: {:?}",
        &body[..end]
      );
      units.push(Unit {
        x,
        class: class.clone(),
        text: unescape(&body[..end]),
      });
      rest = body[end..]
        .strip_prefix("</tspan>")
        .expect("a unit is closed by its own element");
    } else if let Some(after) = from.strip_prefix("<tspan class=\"") {
      let quote = after.find('"').expect("an attribute value is quoted");
      class = Some(after[..quote].to_string());
      rest = &after[quote + 2..];
    } else if let Some(after) = from.strip_prefix("</tspan>") {
      class = None;
      rest = after;
    } else {
      panic!("a row holds an element this surface does not write: {from:?}");
    }
  }
  assert!(rest.is_empty(), "a row ends with {rest:?} outside any unit");
  units
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
    let units = units(&from[open + 1..close]);
    let text: String = units.iter().map(|unit| unit.text.as_str()).collect();
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
      units,
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

  /// Every cluster class the placement model distinguishes, in one line: a wide ideograph, a base
  /// with a combining mark, a base with a variation selector, a regional indicator pair and a ZWJ
  /// sequence. Each is one cluster and none of them is one scalar, which is the whole point —
  /// measured scalar by scalar the emoji alone is two cells too wide.
  ///
  /// A second line with a carriage return on it, because `CR LF` is the one cluster whose width
  /// and whose stand-ins disagree.
  const CLUSTERS: &str = "let a = \"漢e\u{301}☃\u{fe0f}🇺🇸👩\u{200d}💻\";\r\nlet b = 2;\n";

  static ONE: [Input<'static>; 1] = [Input::new(Source::new(PLAIN))];
  static TWO: [Input<'static>; 2] = [
    Input::new(Source::new(PLAIN)),
    Input::new(Source::new(LONG)),
  ];
  static WIDE_INPUT: [Input<'static>; 1] = [Input::new(Source::new(WIDE))];
  static CLUSTER_INPUT: [Input<'static>; 1] = [Input::new(Source::new(CLUSTERS))];
  static NONE: [Input<'static>; 0] = [];

  static SECOND: [Label<'static>; 1] = [Label::new(
    Location::new(0, Span::new(16, 21)),
    "first defined here",
  )];
  static ELSEWHERE: [Label<'static>; 1] = [Label::new(
    Location::new(1, Span::new(0, 2)),
    "and over here",
  )];
  /// A label carrying the clusters itself, and one whose span lands INSIDE the ZWJ sequence — the
  /// case `columns_for` widens to a whole unit, and the case a re-segmenting surface splits.
  static INSIDE: [Label<'static>; 1] = [Label::new(
    Location::new(0, Span::new(28, 32)),
    "inside a joined 👩\u{200d}💻 sequence",
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
      "every cluster class, and a mark inside one",
      Diagnostic::new(
        "code",
        Severity::Error,
        &"a message with a 👩\u{200d}💻 in it",
        Location::new(0, Span::new(9, 40)),
      )
      .with_primary_label("this literal, with a ☃\u{fe0f} in the label")
      .with_labels(&INSIDE)
      .with_help("and a help line with 漢字 and e\u{301} in it"),
      &CLUSTER_INPUT[..],
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
fn every_unit_is_drawn_at_the_cell_the_terminal_gave_it() {
  // The placement model, asserted where it is now stated. A row used to be stretched to a total and
  // this read the total back; a total is exactly what could be right while the glyphs under it were
  // not, so what is read back now is every unit's own coordinate.
  //
  // Against the TERMINAL's row rather than against the surface's bookkeeping — the walk starts from
  // the row `Terminal::render` wrote — which is why the corpus carries a tab, two wide clusters and
  // a joined emoji.
  for (style, terminal) in styles() {
    for (case, diagnostic, inputs) in corpus() {
      let image = as_svg(&terminal, &diagnostic, inputs);
      let plain = as_terminal(&terminal, &diagnostic, inputs);
      let expected = lines(&plain);

      for row in rows(&image) {
        let line = expected
          .get(row.index)
          .unwrap_or_else(|| panic!("{style}/{case}: the image has a row the render does not"));
        // The oracle is `LineCells` over the terminal's own row, and that is the whole strength of
        // this gate. Summing the units' widths as it goes would only check the document against
        // ITSELF: a surface that segmented the row per scalar measures every fragment consistently
        // and lays them out consistently, so the arithmetic closes while a caret two cells away
        // from its glyph sits in the finished image. Placing the marker row is `column_at`'s job,
        // so `column_at` is what the source row has to agree with.
        let source = Source::new(line);
        let cells = crate::terminal::LineCells::new(
          source.line(1).expect("a row is one line"),
          crate::terminal::LineCells::default_tab_width(),
        );
        let mut at = 0;
        for unit in &row.units {
          assert!(
            line[at..].starts_with(unit.text.as_str()),
            "{style}/{case}: row {} draws {:?} where the render has {:?}",
            row.index,
            unit.text,
            &line[at..]
          );
          assert_eq!(
            unit.x,
            PAD + (cells.column_at(at) - 1) * ADVANCE,
            "{style}/{case}: row {}, unit {:?}, is drawn at {} and belongs in column {}",
            row.index,
            unit.text,
            unit.x,
            cells.column_at(at)
          );
          at += unit.text.len();
        }
        assert_eq!(
          at,
          line.len(),
          "{style}/{case}: row {} left {:?} undrawn",
          row.index,
          &line[at..]
        );
      }

      let mut widest = 0;
      for line in &expected {
        widest = widest.max(cells_from(line));
      }
      let rows_drawn = u64::try_from(expected.len()).expect("a row count fits sixty-four bits");

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
  // Every `<` in the document opens an element this surface wrote. Looking for `<script` is what
  // this used to do, and a unit is one cluster: the caller's `<` is a unit of its own now, so a
  // document that carried it verbatim would still hold no `<script` anywhere. A census over the
  // character cannot be walked past that way.
  const ELEMENTS: [&str; 8] = [
    "<svg ", "</svg>", "<style>", "</style>", "<text ", "</text>", "<tspan ", "</tspan>",
  ];
  for (at, _) in image.match_indices('<') {
    let from = &image[at..];
    assert!(
      ELEMENTS.iter().any(|element| from.starts_with(element)),
      "the document has a `<` that opens nothing this surface writes: {:?}",
      &from[..from.len().min(48)]
    );
  }
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
    let mut document = Document::new(&mut bounded);
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
fn a_caret_is_at_the_same_coordinate_as_the_glyph_it_marks() {
  // The failure this crate exists to prevent, asserted as a COORDINATE. Every other gate here
  // compares the image's rows to the terminal's rows, which a surface can satisfy while placing
  // them anywhere; this one reads the two numbers that have to be equal and compares them.
  //
  // The marked cluster is the last thing on the line, and everything to its left is a cluster whose
  // width is not the sum of its scalars' — so any re-segmentation anywhere on the row moves the
  // caret off the glyph and this says so.
  const TARGET: &str = "👩\u{200d}💻";
  const LEADING: &str = "let x = \"a漢e\u{301}☃\u{fe0f}🇺🇸";
  let line = std::format!("{LEADING}{TARGET}\";\n");
  let at = line.find(TARGET).expect("the target is on the line");
  let inputs = [Input::new(Source::new(&line))];
  let diagnostic = Diagnostic::new(
    "code",
    Severity::Error,
    &"the last cluster on the line",
    Location::new(0, Span::new(at, at + TARGET.len())),
  )
  .with_primary_label("here");

  for (style, terminal) in styles() {
    let image = as_svg(&terminal, &diagnostic, &inputs);
    let drawn = rows(&image);

    let source = drawn
      .iter()
      .find(|row| row.text.contains(TARGET))
      .unwrap_or_else(|| panic!("{style}: no row of the image draws the marked line"));
    // One unit, not three. A surface that split the sequence would draw `👩`, U+200D and `💻` at
    // three coordinates, and the first of them would still be in the right place.
    let glyphs: Vec<&Unit> = source
      .units
      .iter()
      .filter(|unit| unit.text == TARGET)
      .collect();
    assert_eq!(
      glyphs.len(),
      1,
      "{style}: the joined sequence reached the document as {} units: {:?}",
      glyphs.len(),
      source
        .units
        .iter()
        .map(|unit| &unit.text)
        .collect::<Vec<_>>()
    );

    // Found by CLASS and not by glyph: the four styles mark with `^`, `━┳` and `─┬`, and a test
    // that knew which would be testing the style rather than the placement.
    let primary = class(Role::PrimaryLabel);
    let marker = drawn
      .iter()
      .filter(|row| {
        row.index > source.index
          && row
            .units
            .iter()
            .any(|unit| unit.class.as_deref() == Some(primary))
      })
      .min_by_key(|row| row.index)
      .unwrap_or_else(|| panic!("{style}: nothing is marked under the line"));
    let marked: Vec<&Unit> = marker
      .units
      .iter()
      .filter(|unit| unit.class.as_deref() == Some(primary))
      .collect();
    assert!(
      marked.len() >= 2,
      "{style}: a two-cell cluster was marked in {} cells",
      marked.len()
    );
    assert_eq!(
      marked[0].x, glyphs[0].x,
      "{style}: the mark is at {} and the glyph it names is at {}",
      marked[0].x, glyphs[0].x
    );
    assert_eq!(
      marked[1].x,
      glyphs[0].x + ADVANCE,
      "{style}: the mark's second cell is not the cluster's second cell"
    );
  }
}

#[test]
fn a_caller_display_is_formatted_exactly_once() {
  // Two passes over one render must not be two questions to one caller. A `Display` is entitled to
  // be stateful — a counter, a cursor, a value that consumes what it reports — and nothing in its
  // contract says two formattings agree.
  //
  // This one's answers differ in LENGTH as well as in content, which is what separates a gate that
  // sees the defect from one that sees only its symptom: formatted twice, the viewport is measured
  // against text the document does not contain.
  struct Counting(core::cell::Cell<usize>);

  impl fmt::Display for Counting {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
      self.0.set(self.0.get() + 1);
      for _ in 0..self.0.get() {
        out.write_str("asked ")?;
      }
      Ok(())
    }
  }

  let counting = Counting(core::cell::Cell::new(0));
  let diagnostic = Diagnostic::new(
    "code",
    Severity::Error,
    &counting,
    Location::new(0, Span::new(0, 3)),
  )
  .with_primary_label("here");
  static INPUTS: [Input<'static>; 1] = [Input::new(Source::new("let x = 1;\n"))];

  let image = as_svg(&Terminal::plain(), &diagnostic, &INPUTS[..]);
  assert_eq!(
    counting.0.get(),
    1,
    "the caller's `Display` was formatted {} times",
    counting.0.get()
  );
  let drawn = rows(&image);
  assert!(
    drawn
      .iter()
      .any(|row| row.text.ends_with("asked ") && !row.text.ends_with("asked asked ")),
    "the document does not carry the one answer the caller gave: {image}"
  );

  // And the viewport is the extent of what the document actually holds, measured off the document
  // rather than off a second render — which is the assertion a caller with a stateful `Display`
  // could not otherwise make, because asking again is the thing being checked.
  let widest = drawn
    .iter()
    .map(|row| cells_from(&row.text))
    .max()
    .expect("the render has rows");
  assert_eq!(
    attribute(&image, "width"),
    Some((PAD * 2 + widest * ADVANCE).to_string()),
    "the viewport was measured against text the document does not contain: {image}"
  );
}

#[test]
fn a_scalar_xml_forbids_is_replaced_and_moves_nothing() {
  // U+FFFE and U+FFFF are valid Rust `char`s, are outside XML 1.0's `Char` production, and have no
  // Control Pictures glyph — so the sanitizer that keeps every C0 character out of the document
  // passes them straight through, and one of them anywhere makes the whole image unparseable.
  //
  // The oracle is the same render with U+FFFD written by the caller instead. Byte identity is what
  // asserts both halves at once: that the substitution happened, and that it moved nothing — a
  // stand-in of a different width would put every later unit on a different cell and the documents
  // would differ in every coordinate after the first.
  fn image_of(hole: char) -> String {
    let source = std::format!("let a = \"{hole}\";\nlet b = 2;\n");
    let text = std::format!("a message with {hole} in it");
    let label = std::format!("a label with {hole} in it");
    let help = std::format!("a help line with {hole} in it");
    let inputs = [Input::new(Source::new(&source)).with_origin("origin")];
    let labels = [Label::new(Location::new(0, Span::new(17, 19)), &label)];
    let diagnostic = Diagnostic::new(
      "code",
      Severity::Error,
      &text,
      Location::new(0, Span::new(8, 12)),
    )
    .with_primary_label(&label)
    .with_labels(&labels)
    .with_help(&help);
    as_svg(&Terminal::plain(), &diagnostic, &inputs)
  }

  let benign = image_of('\u{fffd}');
  for hole in ['\u{fffe}', '\u{ffff}'] {
    let image = image_of(hole);
    assert!(
      !image.contains(hole),
      "{hole:?} reached the document, which no XML parser will read"
    );
    assert_eq!(
      image, benign,
      "{hole:?} was not replaced by the stand-in, or was replaced by one of another width"
    );
  }
  // Not vacuous in the other direction: a noncharacter the production PERMITS is left alone, so
  // this is the `Char` production and not a wider rule made up beside it.
  let permitted = image_of('\u{fdd0}');
  assert!(
    permitted.contains('\u{fdd0}'),
    "a scalar XML permits was substituted anyway: {permitted}"
  );
}

#[test]
fn a_carriage_return_is_one_cell_wherever_it_is_measured() {
  // `write_expanded_upto` prices a control cluster per STAND-IN rather than by the cluster's own
  // width, and `CR LF` is the one input where the two differ: UAX#29 joins it into a single cluster
  // of width 1, and it is drawn as `␍␊`, which is two cells.
  //
  // It cannot reach that walk, and this is why: a `Line`'s text is what lies between two breaks, so
  // no line of any source holds a U+000A for the CR to join.
  let source = Source::new("a\r\nb\r\nc");
  let mut seen = 0;
  for line in source.lines() {
    assert!(
      !line.text().contains('\n'),
      "a line of a source carries the break that ended it: {:?}",
      line.text()
    );
    seen += 1;
  }
  assert_eq!(seen, 3, "the source did not split where it was meant to");

  // And where a `CR LF` CAN arrive — caller text, through the sanitizer — it is two pictures and
  // two cells, in the extent and in the document alike. Read as a coordinate: the unit after it
  // sits two cells along.
  static INPUTS: [Input<'static>; 1] = [Input::new(Source::new("let x = 1;\n"))];
  let diagnostic = Diagnostic::new(
    "code",
    Severity::Error,
    &"before\r\nafter",
    Location::new(0, Span::new(0, 3)),
  );
  let image = as_svg(&Terminal::plain(), &diagnostic, &INPUTS[..]);
  let message = rows(&image)
    .into_iter()
    .find(|row| row.text.contains('\u{240d}'))
    .expect("the message reached the document");
  let at = message
    .units
    .iter()
    .position(|unit| unit.text == "\u{240d}")
    .expect("the carriage return is a unit of its own");
  assert_eq!(
    message.units[at + 1].text,
    "\u{240a}",
    "the two halves of a `CR LF` did not both reach the document"
  );
  assert_eq!(
    message.units[at + 2].x,
    message.units[at].x + 2 * ADVANCE,
    "a `CR LF` was drawn as two pictures and measured as one cell"
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

#[test]
fn a_synthesized_message_cannot_outrun_the_writer() {
  // The attack this ceiling exists for, in the shape a caller would build it: a `Display` value of
  // two words against a writer that refuses its very first byte. Formatting once bounds how often
  // the `Display` is ASKED; it bounds nothing about what the `Display` answers, and the answer is
  // held rather than streamed — so before the ceiling, the render allocated and spent whatever the
  // value chose, and only then offered `out` a byte it was always going to refuse.
  //
  // The `Display` here is well behaved: it stops the moment it is refused. Everything it manages to
  // write, it was permitted to write, so its counter measures the bound and not its own restraint.
  const CHUNK: usize = 4096;
  const FAR_PAST: u64 = 64 << 20;

  struct Synthesizing(core::cell::Cell<u64>);

  impl fmt::Display for Synthesizing {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
      let chunk = "x".repeat(CHUNK);
      while self.0.get() < FAR_PAST {
        out.write_str(&chunk)?;
        self.0.set(self.0.get() + CHUNK as u64);
      }
      Ok(())
    }
  }

  /// Refuses its first byte, and records whether it was ever offered one.
  struct Deaf(bool);

  impl fmt::Write for Deaf {
    fn write_str(&mut self, _text: &str) -> fmt::Result {
      self.0 = true;
      Err(fmt::Error)
    }
  }

  let synthesizing = Synthesizing(core::cell::Cell::new(0));
  let diagnostic = Diagnostic::new(
    "code",
    Severity::Error,
    &synthesizing,
    Location::new(0, Span::new(0, 3)),
  )
  .with_primary_label("here");
  static INPUTS: [Input<'static>; 1] = [Input::new(Source::new("let x = 1;\n"))];

  let mut deaf = Deaf(false);
  assert!(
    Terminal::plain()
      .render_svg(&diagnostic, &INPUTS[..], &mut deaf)
      .is_err(),
    "a message past the ceiling was rendered"
  );

  let ceiling = Terminal::<Theme>::max_svg_message_bytes();
  assert!(
    synthesizing.0.get() <= ceiling,
    "the `Display` was allowed to synthesize {} bytes against a ceiling of {ceiling}",
    synthesizing.0.get()
  );
  // The half a byte count alone cannot see. A ceiling reached by measuring the whole answer and
  // then reporting on it satisfies the assertion above only because the counter stops at the
  // refusal — this says the refusal came before `out` was consulted at all, which is what makes the
  // ceiling a bound on this render rather than a bound on what gets drawn.
  assert!(
    !deaf.0,
    "the writer was offered a byte, so the message was spent before it could refuse"
  );
}

#[test]
fn the_message_ceiling_is_where_it_says_it_is() {
  // A caller can predict this refusal only if the published number is the number, so the two sides
  // of it are asserted rather than the middle: at the ceiling exactly the render is ordinary, and
  // one byte past it there is no render at all.
  fn render_message(bytes: usize) -> (fmt::Result, String) {
    let message = "x".repeat(bytes);
    let diagnostic = Diagnostic::new(
      "code",
      Severity::Error,
      &message,
      Location::new(0, Span::new(0, 3)),
    )
    .with_primary_label("here");
    static INPUTS: [Input<'static>; 1] = [Input::new(Source::new("let x = 1;\n"))];

    let mut out = String::new();
    let outcome = Terminal::plain().render_svg(&diagnostic, &INPUTS[..], &mut out);
    (outcome, out)
  }

  let ceiling = usize::try_from(Terminal::<Theme>::max_svg_message_bytes())
    .expect("the ceiling fits this target's pointer");

  let (outcome, image) = render_message(ceiling);
  assert!(
    outcome.is_ok(),
    "a message of exactly the ceiling was refused"
  );
  assert!(
    image.matches(">x</tspan>").count() >= ceiling,
    "a message of exactly the ceiling did not come out whole"
  );

  let (outcome, image) = render_message(ceiling + 1);
  assert!(outcome.is_err(), "a message past the ceiling was rendered");
  // What a caller reads to tell this refusal from its own writer's: `fmt::Error` carries no
  // payload, and this writer refuses nothing, so an empty `out` is the only thing that says the
  // message was the reason.
  assert!(
    image.is_empty(),
    "a writer that refuses nothing was still handed {} bytes of a refused render",
    image.len()
  );
}

#[test]
fn the_capture_admits_exactly_the_ceiling_and_reports_what_it_refused() {
  // The arithmetic, at the boundary and in one place, because the ceiling published to callers is
  // only predictable if `exactly this many` is on the accepting side of it.
  //
  // What this test CANNOT see is whether the refusal came before or after the append: both orders
  // refuse the same write and return the same error, and they differ only in what was allocated on
  // the way. That is measured where allocation can be measured —
  // `the_svg_capture_refuses_a_message_before_it_allocates_it`, in `tests/writer_discipline.rs`.
  use super::Captured;

  let mut captured = Captured::new(8);
  fmt::Write::write_str(&mut captured, "abc").expect("three bytes fit under eight");
  fmt::Write::write_str(&mut captured, "defgh").expect("eight bytes are not past eight");
  assert_eq!(
    captured.whole(),
    Ok(String::from("abcdefgh")),
    "a capture filled to the ceiling did not hand back what it took"
  );

  let mut captured = Captured::new(8);
  fmt::Write::write_str(&mut captured, "abcdefgh").expect("eight bytes are not past eight");
  fmt::Write::write_str(&mut captured, "i").expect_err("nine bytes are past eight");
  assert_eq!(
    captured.whole(),
    Err(fmt::Error),
    "a capture that refused a write handed back the truncation as a whole message"
  );

  // And the refusal is of the WRITE rather than of the byte past the ceiling: a fragment that
  // would cross is not split, because half a message is the truncation this refuses to perform.
  let mut captured = Captured::new(8);
  fmt::Write::write_str(&mut captured, "abcdefghi").expect_err("nine bytes are past eight");
  assert_eq!(
    captured.whole(),
    Err(fmt::Error),
    "an oversized write was partly kept"
  );
}

#[test]
fn a_display_that_ignores_its_error_is_still_refused() {
  // `core::fmt::write` returns what the `Display` returned, not what the writer under it did — so a
  // `Display` that discards the error it is handed and returns `Ok` reports success over a capture
  // that refused half of it. Trusting the `?` alone renders a SILENTLY TRUNCATED message, which is
  // the failure the ceiling would otherwise have introduced in exchange for the one it fixed.
  struct Deaf;

  impl fmt::Display for Deaf {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
      let ceiling = usize::try_from(Terminal::<Theme>::max_svg_message_bytes())
        .expect("the ceiling fits this target's pointer");
      let _ = out.write_str(&"x".repeat(ceiling));
      // Refused, and ignored, and reported as success.
      let _ = out.write_str("y");
      Ok(())
    }
  }

  let diagnostic = Diagnostic::new(
    "code",
    Severity::Error,
    &Deaf,
    Location::new(0, Span::new(0, 3)),
  )
  .with_primary_label("here");
  static INPUTS: [Input<'static>; 1] = [Input::new(Source::new("let x = 1;\n"))];

  let mut out = String::new();
  assert!(
    Terminal::plain()
      .render_svg(&diagnostic, &INPUTS[..], &mut out)
      .is_err(),
    "a truncated message was rendered as though it were whole"
  );
  assert!(
    out.is_empty(),
    "the refused render still wrote {} bytes",
    out.len()
  );
}
