use super::LineCells;
use crate::{Source, Span};

/// The corpus, and what each member is here to break.
///
/// A corpus whose members are not individually justified is a corpus the next person trims. Each
/// of these defeats a *different* wrong definition of "column", and the first entry is the one
/// that makes all the wrong definitions look right.
const CORPUS: [(&str, &str); 21] = [
  (
    "abc",
    "the baseline where every wrong definition agrees: one byte, one character, one cell",
  ),
  (
    "日本語",
    "CJK — breaks column == character. Three characters, six cells",
  ),
  (
    "e\u{301}f",
    "a combining mark — breaks column == code point. The mark occupies none",
  ),
  (
    "a\tb",
    "a tab — breaks column == anything local. Its width depends on where it starts",
  ),
  (
    "\ta",
    "a leading tab — the stop arithmetic at column zero, where `% tab_width` is a special case",
  ),
  (
    "a\t\tb",
    "adjacent tabs — a second tab must advance a whole stop, not zero",
  ),
  (
    "🎨x",
    "an astral character — breaks column == byte and column == UTF-16 unit",
  ),
  (
    "👨\u{200d}👩\u{200d}👧x",
    "a ZWJ sequence — breaks grapheme == code point. Summing per-character widths overstates it",
  ),
  (
    "",
    "an empty line — the arithmetic must not need a first character",
  ),
  ("\t", "a line that is only a tab"),
  (
    "日\tx\u{301}\t🎨",
    "every hazard at once, so an error that only appears in combination has somewhere to appear",
  ),
  (
    "a\u{7f}b",
    "a control character — no width table entry, and it must not be treated as a hole",
  ),
  (
    "\u{2603}\u{fe0f}x",
    "a variation selector — the presentation sequence is WIDER than its base, so a unit's extent \
     cannot be read off the width of its first character",
  ),
  (
    "1\u{fe0f}\u{20e3}x",
    "a keycap — three code points and one key, the middle of which changes the width",
  ),
  (
    "\u{1f1ef}\u{1f1f5}x",
    "a regional indicator pair — one flag, and each half is a legal character on its own",
  ),
  (
    "\u{915}\u{94d}\u{937}\u{93f}x",
    "a Devanagari conjunct — the prefix widths rise 1, 1, 2, 3 through a single cluster, so any \
     extend-while-the-width-holds rule cuts it somewhere",
  ),
  (
    "\u{e01}\u{e33}x",
    "Thai sara am — a cluster whose second character ADDS a cell, the mirror of a combining mark \
     that adds none",
  ),
  (
    "\u{644}\u{627}x",
    "Arabic lam-alef — two clusters the STRING model scores as one cell, and the reason the model \
     is written down: painty reports two, which is what every terminal advances",
  ),
  (
    "\u{2d31}\u{2d7f}\u{2d3e}x",
    "Tifinagh with a joiner — the same rejection at its widest, three cells against one",
  ),
  (
    "a\u{200b}bx",
    "a zero-width space — a cluster of NO cells in the middle of a line, so a column can repeat \
     without going backwards",
  ),
  (
    "\u{301}ab",
    "a combining mark with nothing to combine with, at the very start — a defective sequence is \
     still a cluster, and it is the first one",
  ),
];

const TAB_WIDTHS: [u64; 4] = [1, 2, 4, 8];

fn measured(text: &str, tab_width: u64) -> LineCells<'_> {
  let source = Source::new(text);
  LineCells::new(
    source.line(1).expect("a source has a first line"),
    tab_width,
  )
}

#[test]
fn a_line_starts_in_column_one() {
  for (text, why) in CORPUS {
    for tab in TAB_WIDTHS {
      assert_eq!(measured(text, tab).column_at(0), 1, "{text:?}: {why}");
    }
  }
}

#[test]
fn a_column_never_goes_backwards() {
  // Monotonicity is the property an underline rests on: if a later offset could report a smaller
  // column, a span's start could be placed after its end.
  for (text, why) in CORPUS {
    for tab in TAB_WIDTHS {
      let cells = measured(text, tab);
      let mut previous = 0;
      for offset in 0..=text.len() {
        let column = cells.column_at(offset);
        assert!(
          column >= previous,
          "{text:?} @ {offset} with tab {tab}: {column} after {previous} — {why}"
        );
        previous = column;
      }
    }
  }
}

#[test]
fn no_column_reaches_past_the_line_it_measures() {
  for (text, why) in CORPUS {
    for tab in TAB_WIDTHS {
      let cells = measured(text, tab);
      let width = cells.width();
      for offset in 0..=text.len() + 2 {
        assert!(
          cells.column_at(offset) <= width + 1,
          "{text:?} @ {offset} with tab {tab}: past {width} cells — {why}"
        );
      }
    }
  }
}

#[test]
fn the_column_past_the_end_is_the_width_plus_one() {
  // The definition tying the two accessors together: a column is one plus the cells before it, so
  // the column at the end must be one plus all of them. A renderer that measured a span's end
  // separately from the line's width would drift here first.
  for (text, why) in CORPUS {
    for tab in TAB_WIDTHS {
      let cells = measured(text, tab);
      assert_eq!(
        cells.column_at(text.len()),
        cells.width() + 1,
        "{text:?} with tab {tab}: {why}"
      );
    }
  }
}

#[test]
fn expanding_a_line_produces_exactly_the_cells_it_was_measured_at() {
  // The invariant the whole renderer rests on. An underline is placed with `column_at` and drawn
  // under text emitted by `write_expanded`; if the two disagree by a single cell the underline
  // points at the wrong character, and nothing else here would notice.
  //
  // Stated from the definition rather than from what the code prints: the expansion writes spaces
  // to the next stop and the measurement advances to the next stop, so the expanded text must
  // occupy the width that was reported.
  for (text, why) in CORPUS {
    for tab in TAB_WIDTHS {
      let cells = measured(text, tab);
      let mut expanded = String::new();
      cells
        .write_expanded(&mut expanded)
        .expect("a String never fails to be written to");

      let source = Source::new(&expanded);
      let widened = LineCells::new(source.line(1).expect("a first line"), tab);
      assert_eq!(
        widened.width(),
        cells.width(),
        "{text:?} expanded to {expanded:?} with tab {tab}: {why}"
      );
      assert!(
        !expanded.contains('\t'),
        "{text:?}: a tab survived expansion"
      );
    }
  }
}

#[test]
fn a_display_column_equals_a_character_column_exactly_when_every_character_is_one_cell() {
  // The cross-check against layer 2. Where the two units must agree they are asserted to, and
  // where they must not, they are asserted not to — so a display column that silently fell back to
  // counting characters would fail on one side or the other.
  let plain = measured("abc", 4);
  for offset in 0..=3 {
    assert_eq!(plain.column_at(offset), plain.line().column_at(offset));
  }

  // Which inputs must differ is DERIVED rather than listed: the two coordinate systems coincide
  // everywhere exactly when every cluster is one character occupying one cell, and must part
  // company otherwise. A cluster of several characters shifts the character count; a cluster of
  // other than one cell shifts the display count.
  //
  // The premise this replaced was "not pure ASCII, therefore it differs", and Arabic lam-alef
  // falsified it: two clusters, one character and one cell each, so the two agree — on the very
  // input the round was about. A listed exemption there would have been an allowlist hiding the
  // one case that mattered.
  use unicode_segmentation::UnicodeSegmentation;
  use unicode_width::UnicodeWidthStr;

  for (text, why) in CORPUS {
    let cells = measured(text, 4);
    let one_for_one = !text.contains('\t')
      && text
        .graphemes(true)
        .all(|cluster| cluster.chars().count() == 1 && cluster.width() == 1);
    let differs = (0..=text.len()).any(|offset| {
      cells.line().text().is_char_boundary(offset)
        && cells.column_at(offset) != cells.line().column_at(offset)
    });
    assert_eq!(
      !differs, one_for_one,
      "{text:?}: display and character columns agree exactly when every cluster is one character \
       in one cell — {why}"
    );
  }
}

#[test]
fn a_combining_mark_occupies_no_cells_and_belongs_to_its_base() {
  // `e` then U+0301, then `f`. The mark is a character and two bytes and zero cells, and it is not
  // separately placeable: a terminal draws it on the `e`.
  //
  // This assertion changed when the geometry did, and the reason is a change of DEFINITION rather
  // than a test bent to fit new output. It used to say byte 1 is column 2 — "after the `e`" —
  // which is true of a prefix measured on its own and false of anything a terminal draws. An
  // offset inside a placement unit now reports where that unit begins, because that is where the
  // whole of it appears.
  let cells = measured("e\u{301}f", 4);
  assert_eq!(cells.column_at(0), 1);
  assert_eq!(
    cells.column_at(1),
    1,
    "inside the cluster, which is drawn at column one"
  );
  assert_eq!(
    cells.column_at(3),
    2,
    "the `f`, after the cluster's single cell"
  );
  assert_eq!(cells.width(), 2);
  assert_eq!(cells.line().char_count(), 3, "three characters, two cells");

  // And a span over only the mark is widened to the cluster, because half of one cannot be drawn.
  assert_eq!(cells.columns_for(Span::new(1, 3)), 1..2);
}

#[test]
fn a_cjk_character_occupies_two_cells() {
  let cells = measured("日本語", 4);
  assert_eq!(cells.column_at(0), 1);
  assert_eq!(cells.column_at(3), 3);
  assert_eq!(cells.column_at(6), 5);
  assert_eq!(cells.width(), 6);
  assert_eq!(cells.line().char_count(), 3, "three characters, six cells");
}

#[test]
fn a_tab_advances_to_the_next_stop_and_not_by_its_own_width() {
  for tab in TAB_WIDTHS {
    let cells = measured("a\tb", tab);
    let after = cells.column_at(2);
    assert_eq!(
      (after - 1) % tab,
      0,
      "tab {tab}: column {after} is not on a stop"
    );
    assert!(after > 2, "tab {tab}: the tab advanced nothing");
  }
  // The stop is absolute, not relative: with a stop every four cells, one character then a tab
  // reaches five, and three characters then a tab reaches the same five.
  assert_eq!(measured("a\tb", 4).column_at(2), 5);
  assert_eq!(measured("abc\tb", 4).column_at(4), 5);
  // And a tab that lands exactly on a stop still advances a whole one, or two tabs would collapse.
  assert_eq!(measured("abcd\tb", 4).column_at(5), 9);
}

#[test]
fn cells_between_measures_the_span_and_not_the_slice() {
  // The same `\t` is four cells at the head of a line and one cell three characters in, so a
  // renderer that measured `covered_text()` on its own would size an underline wrongly.
  let cells = measured("abc\tx", 4);
  assert_eq!(cells.cells_between(3, 4), 1, "the tab, reaching the stop");
  let leading = measured("\tx", 4);
  assert_eq!(
    leading.cells_between(0, 1),
    4,
    "the same tab, from column one"
  );
}

#[test]
fn a_zwj_sequence_is_measured_as_one_glyph_and_not_as_its_parts() {
  // This assertion was wrong on its first attempt, in the way this whole file exists to prevent: it
  // compared against a guessed constant — eight — rather than against the thing it had to differ
  // from. The per-character sum is six, so a measurement that summed characters gave seven and
  // passed. Planting exactly that defect is what exposed it.
  //
  // Stated from the definition instead: a sequence draws as one glyph, so measuring it must give
  // LESS than measuring its parts. That comparison cannot be satisfied by the defect, because the
  // defect makes the two equal by construction.
  let text = "👨\u{200d}👩\u{200d}👧x";
  let cells = measured(text, 4);
  let summed: u64 = text
    .chars()
    .map(|character| unicode_width::UnicodeWidthChar::width(character).unwrap_or(0) as u64)
    .sum();

  assert!(
    cells.width() < summed,
    "measured {} cells and the per-character sum is {summed}; a sequence was not seen as one",
    cells.width()
  );
  assert_eq!(
    cells.width(),
    3,
    "two cells for the family, one for the `x`"
  );
  assert_eq!(cells.column_at(text.len()), cells.width() + 1);
}

#[test]
fn measurement_is_total_over_offsets_no_producer_should_have_produced() {
  // Mid-character, past the end, and before the line. A renderer that panicked here would lose the
  // diagnostic it was asked to draw, which is the failure layer 2 is arranged to avoid and this
  // layer must not reintroduce.
  for (text, _) in CORPUS {
    for tab in TAB_WIDTHS {
      let cells = measured(text, tab);
      for offset in 0..=text.len() + 4 {
        let _ = cells.column_at(offset);
      }
    }
  }
}

#[test]
fn a_tab_width_of_zero_is_raised_rather_than_dividing_by_it() {
  let cells = measured("a\tb", 0);
  assert_eq!(cells.tab_width(), 1);
  assert_eq!(cells.column_at(2), 3, "the tab is one cell");
}

// ── Colour detection ────────────────────────────────────────────────────────────────────────
//
// Whether a terminal supports colour is an environment question, and an environment is the one
// thing these tests cannot have an oracle for. So none of this touches the process: the decision
// is a pure function of a captured snapshot, and what is asserted is the DECISION PROCEDURE —
// given these inputs, this answer.

use super::{ColorCapability, ColorChoice, Environment};

/// Every value each input can take, chosen so that each one changes a different branch.
const NO_COLOR: [Option<&str>; 3] = [None, Some(""), Some("0")];
const FORCE: [Option<&str>; 4] = [None, Some(""), Some("0"), Some("1")];
const CLICOLOR: [Option<&str>; 3] = [None, Some("0"), Some("1")];
const TERM: [Option<&str>; 4] = [None, Some("dumb"), Some("xterm"), Some("xterm-256color")];
const COLORTERM: [Option<&str>; 4] = [None, Some("truecolor"), Some("24bit"), Some("nonsense")];

fn every_environment(mut visit: impl FnMut(Environment<'static>)) {
  for no_color in NO_COLOR {
    for force in FORCE {
      for clicolor in CLICOLOR {
        for term in TERM {
          for colorterm in COLORTERM {
            for is_terminal in [false, true] {
              visit(
                Environment::new()
                  .with_no_color(no_color)
                  .with_clicolor_force(force)
                  .with_clicolor(clicolor)
                  .with_term(term)
                  .with_colorterm(colorterm)
                  .with_terminal(is_terminal),
              );
            }
          }
        }
      }
    }
  }
}

#[test]
fn an_explicit_choice_is_never_overruled_by_the_environment() {
  // Level one of the precedence, and the only one with nothing above it. If any environment can
  // move these, the caller's `--color` flag is a suggestion rather than a decision.
  let mut seen = 0;
  every_environment(|environment| {
    seen += 1;
    assert_eq!(
      ColorChoice::Never.resolve(&environment),
      ColorCapability::None,
      "{environment:?}"
    );
    assert_ne!(
      ColorChoice::Always.resolve(&environment),
      ColorCapability::None,
      "{environment:?}"
    );
  });
  assert_eq!(seen, 3 * 4 * 3 * 4 * 4 * 2, "the sweep is not exhaustive");
}

#[test]
fn no_color_is_presence_and_not_truthiness() {
  // The convention is explicit that `NO_COLOR=0` still means no colour, which is the case a
  // reasonable person implements wrongly. Empty is the documented exception.
  let terminal = Environment::new()
    .with_terminal(true)
    .with_term(Some("xterm"));
  assert_eq!(
    ColorChoice::Auto.resolve(&terminal.with_no_color(Some("0"))),
    ColorCapability::None,
    "`NO_COLOR=0` is still no colour"
  );
  assert_eq!(
    ColorChoice::Auto.resolve(&terminal.with_no_color(Some(""))),
    ColorCapability::Ansi16,
    "empty is the documented exception"
  );
  assert_eq!(
    ColorChoice::Auto.resolve(&terminal.with_no_color(None)),
    ColorCapability::Ansi16
  );
}

#[test]
fn no_color_outranks_the_force_convention() {
  // Level two over level three. Nothing else in the sweep contradicts these two at once, so
  // without this the ordering between them is never exercised.
  let environment = Environment::new()
    .with_no_color(Some("1"))
    .with_clicolor_force(Some("1"))
    .with_term(Some("xterm-256color"))
    .with_terminal(true);
  assert_eq!(
    ColorChoice::Auto.resolve(&environment),
    ColorCapability::None
  );
}

#[test]
fn forcing_outranks_the_stream_not_being_a_terminal() {
  // Level three over level four, and the entire reason `CLICOLOR_FORCE` exists: a pipe into
  // `less -R` or a CI log renders escapes perfectly well.
  let piped = Environment::new()
    .with_terminal(false)
    .with_term(Some("xterm"));
  assert_eq!(
    ColorChoice::Auto.resolve(&piped),
    ColorCapability::None,
    "a pipe gets none by default"
  );
  assert_eq!(
    ColorChoice::Auto.resolve(&piped.with_clicolor_force(Some("1"))),
    ColorCapability::Ansi16,
    "and colour when forced"
  );
  // `0` and empty do not force, or the variable's presence alone would be the decision.
  assert_eq!(
    ColorChoice::Auto.resolve(&piped.with_clicolor_force(Some("0"))),
    ColorCapability::None
  );
  assert_eq!(
    ColorChoice::Auto.resolve(&piped.with_clicolor_force(Some(""))),
    ColorCapability::None
  );
}

#[test]
fn forcing_outranks_a_dumb_terminal_and_clicolor_zero() {
  let hostile = Environment::new()
    .with_terminal(true)
    .with_term(Some("dumb"))
    .with_clicolor(Some("0"));
  assert_eq!(ColorChoice::Auto.resolve(&hostile), ColorCapability::None);
  assert_eq!(
    ColorChoice::Auto.resolve(&hostile.with_clicolor_force(Some("1"))),
    ColorCapability::Ansi16,
    "forced past both"
  );
}

#[test]
fn being_a_terminal_outranks_clicolor_and_the_term_type() {
  // Level four over level five: a non-terminal is already off, so `CLICOLOR=1` cannot switch it
  // back on. The reverse ordering would make redirection depend on a variable.
  let piped = Environment::new()
    .with_terminal(false)
    .with_clicolor(Some("1"))
    .with_term(Some("xterm-256color"))
    .with_colorterm(Some("truecolor"));
  assert_eq!(ColorChoice::Auto.resolve(&piped), ColorCapability::None);
}

#[test]
fn the_level_is_read_even_when_colour_was_forced() {
  // Forcing colour onto a pipe and being handed monochrome would be a strange reward for asking,
  // so the gate and the level are separate questions.
  let forced = Environment::new()
    .with_terminal(false)
    .with_clicolor_force(Some("1"));
  assert_eq!(
    ColorChoice::Auto.resolve(&forced.with_colorterm(Some("truecolor"))),
    ColorCapability::TrueColor
  );
  assert_eq!(
    ColorChoice::Auto.resolve(&forced.with_term(Some("xterm-256color"))),
    ColorCapability::Ansi256
  );
  assert_eq!(ColorChoice::Auto.resolve(&forced), ColorCapability::Ansi16);
}

#[test]
fn the_level_comes_from_colorterm_before_term() {
  let terminal = Environment::new().with_terminal(true);
  // `COLORTERM` wins where they disagree: a truecolour terminal often still reports `xterm`.
  assert_eq!(
    ColorChoice::Auto.resolve(
      &terminal
        .with_term(Some("xterm"))
        .with_colorterm(Some("24bit"))
    ),
    ColorCapability::TrueColor
  );
  // ...and a value that means nothing does not promote anything.
  assert_eq!(
    ColorChoice::Auto.resolve(
      &terminal
        .with_term(Some("xterm-256color"))
        .with_colorterm(Some("nonsense"))
    ),
    ColorCapability::Ansi256
  );
  assert_eq!(
    ColorChoice::Auto.resolve(&terminal.with_term(Some("xterm"))),
    ColorCapability::Ansi16
  );
}

#[test]
fn every_level_of_the_precedence_is_contradicted_by_the_one_above_it() {
  // The gate against a precedence bug hiding where nothing disagrees. Each row sets up a decision
  // and then adds the level above it, and the answer must change. Compared against what the WRONG
  // order would produce, rather than against a value typed by hand: if any pair were reordered,
  // the two sides of the pair would agree and the assertion would be unsatisfiable.
  let base = Environment::new().with_term(Some("xterm"));
  let pairs: [(Environment<'_>, Environment<'_>); 4] = [
    // the stream, contradicted by forcing
    (
      base.with_terminal(false),
      base.with_terminal(false).with_clicolor_force(Some("1")),
    ),
    // forcing, contradicted by NO_COLOR
    (
      base.with_terminal(false).with_clicolor_force(Some("1")),
      base
        .with_terminal(false)
        .with_clicolor_force(Some("1"))
        .with_no_color(Some("1")),
    ),
    // a dumb terminal, contradicted by forcing
    (
      base.with_terminal(true).with_term(Some("dumb")),
      base
        .with_terminal(true)
        .with_term(Some("dumb"))
        .with_clicolor_force(Some("1")),
    ),
    // CLICOLOR=0, contradicted by forcing
    (
      base.with_terminal(true).with_clicolor(Some("0")),
      base
        .with_terminal(true)
        .with_clicolor(Some("0"))
        .with_clicolor_force(Some("1")),
    ),
  ];
  for (lower, with_higher) in pairs {
    assert_ne!(
      ColorChoice::Auto.resolve(&lower),
      ColorChoice::Auto.resolve(&with_higher),
      "the level above did not override: {lower:?} against {with_higher:?}"
    );
  }
}

#[test]
fn a_capability_orders_from_none_upwards() {
  // A renderer narrows a style by comparing capabilities, so the order has to be the one a reader
  // would assume.
  assert!(ColorCapability::None < ColorCapability::Ansi16);
  assert!(ColorCapability::Ansi16 < ColorCapability::Ansi256);
  assert!(ColorCapability::Ansi256 < ColorCapability::TrueColor);
}

// ── The renderer ────────────────────────────────────────────────────────────────────────────
//
// Written before any golden, and stated from what must hold rather than from what the code
// prints. A golden written first records whatever the renderer did that day, and then the
// invariant gets written to agree with the golden instead of with the definition — which is
// exactly how the cell test above came to compare against a guessed eight.

use super::{Input, Terminal};
use crate::{Diagnostic, Label, Location, Severity, Theme};

/// A diagnostic assembled over `text`, with whatever positions the caller names.
fn diagnose<'a>(
  primary: Location,
  labels: &'a [Label<'a>],
  message: &'a dyn core::fmt::Display,
) -> Diagnostic<'a> {
  Diagnostic::new("mylang::test::rule", Severity::Error, message, primary)
    .with_primary_label("here")
    .with_labels(labels)
}

fn render(diagnostic: &Diagnostic<'_>, text: &str) -> String {
  let mut out = String::new();
  Terminal::plain()
    .render(diagnostic, &[Input::new(Source::new(text))], &mut out)
    .expect("a String never fails to be written to");
  out
}

#[test]
fn an_underline_covers_exactly_the_cells_its_span_occupies() {
  // The first invariant, and the one the corpus is for: a marker row that is not the width of the
  // text above it points at the wrong characters. Cross-checked against `LineCells`, which is
  // separately invariant-tested, rather than against the renderer's own arithmetic.
  for (text, why) in CORPUS {
    if text.is_empty() {
      continue;
    }
    let source = Source::new(text);
    let line = source.line(1).expect("a first line");
    let cells = LineCells::new(line, 4);
    let terminal = Terminal::plain();

    let mut start = 0;
    while start < text.len() {
      if !text.is_char_boundary(start) {
        start += 1;
        continue;
      }
      let mut end = start;
      while end <= text.len() {
        if text.is_char_boundary(end) {
          let region = source.resolve(Span::new(start, end));
          if let Some(drawn) = region.lines().next() {
            let marks = terminal.underline(drawn);
            let expected = cells
              .cells_between(drawn.covered().start(), drawn.covered().end())
              .max(1);
            assert_eq!(
              marks.end - marks.start,
              expected,
              "{text:?} {start}..{end}: {why}"
            );
            assert_eq!(
              marks.start,
              cells.column_at(drawn.covered().start()),
              "{text:?} {start}..{end}: the marker starts off the span"
            );
          }
        }
        end += 1;
      }
      start += 1;
    }
  }
}

#[test]
fn an_underline_is_never_empty() {
  // A zero-width span is a caret, and a caret a reader cannot see is not a caret. Asserted over
  // every position in the corpus rather than at one hand-picked offset.
  for (text, why) in CORPUS {
    let source = Source::new(text);
    let terminal = Terminal::plain();
    for offset in 0..=text.len() {
      let region = source.resolve(Span::empty(offset));
      if let Some(drawn) = region.lines().next() {
        let marks = terminal.underline(drawn);
        assert!(marks.end > marks.start, "{text:?} @ {offset}: {why}");
      }
    }
  }
}

#[test]
fn the_gutter_is_as_wide_as_the_widest_line_number_it_shows() {
  // Stated as a property of the output's shape: every row that carries the bar puts it in the same
  // column, and that column is one past the widest number. A gutter sized from the first number
  // rather than the widest is the defect, and it only shows when a diagnostic crosses a power of
  // ten — so the case is constructed rather than waited for.
  let text: String = (1..=12).map(|n| format!("line {n}\n")).collect();
  let source = Source::new(&text);
  let first = source.line(9).expect("line nine");
  let second = source.line(10).expect("line ten");
  let message = "crossing a power of ten";
  let labels = [Label::new(
    Location::new(
      0,
      Span::new(second.span().start(), second.span().start() + 4),
    ),
    "the wider number",
  )];
  let diagnostic = diagnose(
    Location::new(0, Span::new(first.span().start(), first.span().start() + 4)),
    &labels,
    &message,
  );

  let out = render(&diagnostic, &text);
  let bars: Vec<usize> = out
    .lines()
    .filter(|row| row.contains('|'))
    .map(|row| row.find('|').expect("a bar"))
    .collect();
  assert!(bars.len() >= 4, "expected several rows carrying a bar");
  assert!(
    bars.iter().all(|column| *column == bars[0]),
    "the bar moves between rows: {bars:?}\n{out}"
  );
  // Two digits, then a space: the bar sits in column three. Compared against the value the DEFECT
  // would produce — a gutter sized from the first number seen, which is one digit wide — so the
  // wrong implementation cannot satisfy this.
  let sized_from_the_first_number = 1 + 1;
  assert_ne!(
    bars[0], sized_from_the_first_number,
    "gutter sized from the first number\n{out}"
  );
  assert_eq!(bars[0], 2 + 1);
}

#[test]
fn nothing_is_emitted_for_a_style_that_asks_for_nothing() {
  // Two claims. A palette that asks for nothing produces no escape at all — not an escape that
  // sets nothing and immediately resets. And a capability of none produces no escape whatever the
  // palette asks for, because a bold sequence written into a file is as wrong as a red one.
  let text = "let x = 1;\n";
  let message = "a message";
  let diagnostic = diagnose(Location::new(0, Span::new(4, 5)), &[], &message);

  let plain = render(&diagnostic, text);
  assert!(
    !plain.contains('\u{1b}'),
    "an escape from a plain render:\n{plain:?}"
  );

  let mut monochrome_at_none = String::new();
  Terminal::with_palette(Theme::monochrome())
    .render(
      &diagnostic,
      &[Input::new(Source::new(text))],
      &mut monochrome_at_none,
    )
    .expect("a String is writable");
  assert!(!monochrome_at_none.contains('\u{1b}'));

  // ...and the same theme at a capability that can carry escapes does emit them, or the assertion
  // above would be satisfied by a renderer that never styles anything.
  let mut monochrome_in_colour = String::new();
  Terminal::with_palette(Theme::monochrome())
    .with_capability(ColorCapability::Ansi16)
    .render(
      &diagnostic,
      &[Input::new(Source::new(text))],
      &mut monochrome_in_colour,
    )
    .expect("a String is writable");
  assert!(
    monochrome_in_colour.contains('\u{1b}'),
    "monochrome still asks for bold, so something must be emitted"
  );
}

#[test]
fn rendering_is_total() {
  // The fourth invariant. Every diagnostic the model can hold produces output rather than a panic
  // or an empty string — including the shapes a producer reaches by accident.
  let message = "a message";
  let cases: [(&str, Location, &str); 8] = [
    (
      "abc\n",
      Location::new(0, Span::new(0, 3)),
      "an ordinary span",
    ),
    (
      "abc\n",
      Location::new(0, Span::empty(1)),
      "a label of zero width",
    ),
    (
      "abc\n",
      Location::new(0, Span::new(3, 3)),
      "a label at end of line",
    ),
    (
      "\n\nabc\n",
      Location::new(0, Span::empty(0)),
      "a label whose line is empty",
    ),
    (
      "abc",
      Location::new(0, Span::new(2, 3)),
      "the final byte with no trailing newline",
    ),
    ("", Location::new(0, Span::empty(0)), "an empty source"),
    ("abc\n", Location::entire(0), "no position at all"),
    (
      "abc\n",
      Location::new(0, Span::new(99, 120)),
      "a span past the end",
    ),
  ];

  for (text, primary, why) in cases {
    let bare = Diagnostic::new("mylang::test::rule", Severity::Advice, &message, primary);
    for diagnostic in [bare, bare.with_primary_label("here").with_help("do this")] {
      let out = render(&diagnostic, text);
      assert!(!out.is_empty(), "{why}: nothing was written");
      assert!(out.ends_with('\n'), "{why}: no trailing newline\n{out:?}");
      assert!(
        out.starts_with("advice[mylang::test::rule]: "),
        "{why}: no header\n{out:?}"
      );
    }
  }
}

#[test]
fn a_diagnostic_with_no_labels_still_shows_its_position() {
  // The "no labels" case is not the same as "no excerpt": a primary position with no phrase
  // attached still has a line worth showing.
  let text = "alpha\nbeta\n";
  let message = "a message";
  let diagnostic = Diagnostic::new(
    "mylang::test::rule",
    Severity::Warning,
    &message,
    Location::new(0, Span::new(6, 10)),
  );
  let out = render(&diagnostic, text);
  assert!(out.contains("2 | beta\n"), "{out}");
  assert!(out.contains("^^^^"), "{out}");
}

#[test]
fn a_whole_input_position_shows_no_excerpt_rather_than_a_fabricated_one() {
  // Layer 2 keeps the difference between "at this span" and "about this input as a whole", and the
  // renderer must not spend it: pointing at line one would be a coordinate the producer never gave.
  let text = "alpha\nbeta\n";
  let message = "a message";
  let diagnostic = Diagnostic::new(
    "mylang::test::rule",
    Severity::Error,
    &message,
    Location::entire(0),
  );
  let out = render(&diagnostic, text);
  assert!(!out.contains("-->"), "{out}");
  assert!(!out.contains('^'), "{out}");
  assert_eq!(out.lines().count(), 1, "{out}");
}

/// The clusters a span must be widened to.
///
/// This used to be a hand-rolled second model — walk characters, join on the identity of a joiner —
/// kept precisely because it did not share the crate's prefix-slicing model. It has been retired,
/// and the reason is worth keeping: **it was the same kind of thing the crate's own defect was.** It
/// recognised joiners from a list of code points, and measured against `unicode-segmentation` it is
/// wrong in both directions — it merges `"a\u{200d}b"`, which UAX#29 splits, and it splits `"กำ"`
/// and `"क्षि"`, which UAX#29 does not.
///
/// A known-wrong oracle left in a suite eventually disagrees with the standard, and the next person
/// makes the code match the oracle. The independence it was providing now comes from `PLACEMENTS`,
/// which is hand-written cells rather than a second algorithm — and an algorithm is what could
/// share a model in the first place.
fn clusters(text: &str) -> Vec<(usize, usize)> {
  use unicode_segmentation::UnicodeSegmentation;
  text
    .grapheme_indices(true)
    .map(|(at, cluster)| (at, at + cluster.len()))
    .collect()
}

#[test]
fn a_span_touching_a_cluster_is_widened_to_the_whole_cluster() {
  // The defect this replaced: a span over one component of a joined emoji produced an empty range
  // that widened into the cell of the character AFTER the cluster, so the marker pointed at the
  // wrong glyph.
  for (text, why) in CORPUS {
    if text.contains('\t') || text.is_empty() {
      continue;
    }
    let cells = measured(text, 4);
    for (start, end) in clusters(text) {
      // Every interior slice of a cluster must produce the same marker as the whole cluster.
      let whole = cells.columns_for(Span::new(start, end));
      let mut inner = start;
      while inner < end {
        if text.is_char_boundary(inner) {
          let mut outer = inner + 1;
          while outer <= end {
            if text.is_char_boundary(outer) {
              assert_eq!(
                cells.columns_for(Span::new(inner, outer)),
                whole,
                "{text:?}: {inner}..{outer} inside cluster {start}..{end} — {why}"
              );
            }
            outer += 1;
          }
        }
        inner += 1;
      }
      // A cluster with cells marks them; a cluster with none marks nothing, and anchors where the
      // next unit starts. The never-empty caret is the RENDERER's rule — `an_underline_is_never_
      // empty` — not this layer's, and conflating the two would hide a zero-width cluster's real
      // geometry behind a widening that happens somewhere else.
      let cluster_cells = cells.column_at(end.min(text.len())) - cells.column_at(start);
      if cluster_cells == 0 && end < text.len() {
        assert_eq!(
          whole.start, whole.end,
          "{text:?}: cluster {start}..{end} occupies no cells, so it marks none — {why}"
        );
      } else {
        assert!(
          whole.end > whole.start,
          "{text:?}: cluster {start}..{end} has cells but marks none — {why}"
        );
      }
    }
  }
}

#[test]
fn a_marker_never_lands_on_a_cell_outside_the_span_it_describes() {
  // Stated as the property a reader actually cares about: whatever the marker covers, the cell
  // immediately after it must belong to something the span does not touch.
  let text = "👨\u{200d}👩\u{200d}👧x";
  let cells = measured(text, 4);
  let family = cells.columns_for(Span::new(7, 11));
  let following = cells.column_at(text.len() - 1);
  assert!(
    family.end <= following,
    "the marker for an interior component reaches the following character: {family:?} against {following}"
  );
  assert_eq!(
    family,
    1..3,
    "the whole family, which is what a terminal draws"
  );
}

#[test]
fn a_tab_width_is_bounded_at_both_ends() {
  // Caller-supplied and multiplied into every column, so it is bounded rather than trusted.
  // `u64::MAX` used to panic in `column_at` with an add overflow, and would have made
  // `write_expanded` emit that many spaces.
  for asked in [
    0,
    1,
    4,
    LineCells::max_tab_width(),
    LineCells::max_tab_width() + 1,
    u64::MAX,
  ] {
    let cells = measured("a\tb", asked);
    assert!(cells.tab_width() >= 1);
    assert!(cells.tab_width() <= LineCells::max_tab_width());
    // Total: no panic, and the arithmetic stays somewhere a terminal could draw.
    let width = cells.width();
    assert!(
      width <= LineCells::max_tab_width() + 2,
      "asked {asked}, got {width} cells"
    );
    let mut expanded = String::new();
    cells.write_expanded(&mut expanded).expect("writable");
    assert!(expanded.len() <= LineCells::max_tab_width() as usize + 2);
  }
}

/// A writer that takes a fixed number of characters and then declines.
struct Budgeted {
  budget: usize,
  taken: usize,
}

impl core::fmt::Write for Budgeted {
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
fn padding_is_counted_in_rendered_columns_and_not_in_pointer_width() {
  // A display column is a `u64` by rule 1, and `{:width$}` takes a `usize`. The two met at an
  // `as usize` in the marker row's indent, which is exact where this test runs and NARROWS on a
  // 32-bit target — where a line of tabs a few tens of MiB long reaches a column past `u32::MAX`
  // and the marker is drawn back near the gutter, under nothing.
  //
  // Tested at the helper rather than through a render, because the defect is arithmetic and
  // reproducing it through the renderer would mean building a 32-bit target and gigabytes of cells.
  use super::render::pad;

  // Exact counts around the chunk boundary, so an off-by-one in the loop or the remainder shows up
  // rather than being absorbed.
  for count in [0u64, 1, 63, 64, 65, 127, 128, 129, 500] {
    let mut out = String::new();
    pad(&mut out, count).expect("a String is writable");
    assert_eq!(out.len() as u64, count, "{count} spaces were not written");
    assert!(out.chars().all(|c| c == ' '), "{out:?} is not all spaces");
  }

  // Past `u32::MAX`, which is where a 32-bit `usize` would wrap. Measured against a writer that
  // stops early rather than by writing four billion spaces: the question is whether the count is
  // treated as the large number it is, and a narrowing implementation would think it had 99 left to
  // write, satisfy this writer, and report success.
  let past = u64::from(u32::MAX) + 100;
  let narrowed = past as u32 as u64;
  assert!(
    narrowed < 1_000,
    "the discriminator needs a count that wraps to something small, and this wraps to {narrowed}"
  );

  let mut out = Budgeted {
    budget: 1_000,
    taken: 0,
  };
  assert!(
    pad(&mut out, past).is_err(),
    "a count past u32::MAX was satisfied by a writer that only took 1000 characters"
  );
  assert_eq!(
    out.taken, 1_000,
    "the writer was filled to {} rather than its budget, so padding stopped short",
    out.taken
  );
}

#[test]
fn bounding_the_geometry_walk_did_not_change_what_it_answers() {
  // `marks_within` exists because asking `columns_for` for exact columns and clipping them
  // afterwards costs a walk of the whole line for a span that will not be drawn. The risk in
  // rewriting a walk to stop early is that it also changes what the walk SAYS, and that would be a
  // marker in the wrong cell — the defect class three earlier rounds were about.
  //
  // So the rewrite is held against the original: given no ceiling to stop at, the two must agree at
  // every span of every corpus member, not merely on the spans someone thought to try.
  for (text, why) in CORPUS {
    let cells = measured(text, 4);
    for start in 0..=text.len() {
      if !text.is_char_boundary(start) {
        continue;
      }
      for end in start..=text.len() {
        if !text.is_char_boundary(end) {
          continue;
        }
        let span = Span::new(start, end);
        assert_eq!(
          one_mark(&cells, span, unbounded()).0,
          cells.columns_for(span),
          "{text:?}: the bounded walk disagrees with `columns_for` at {start}..{end} — {why}"
        );
      }
    }
  }
}

#[test]
fn a_ceiling_stops_the_walk_where_a_whole_unit_stops() {
  // The stop is NOT the ceiling, and everything placed against the row depends on knowing that. A
  // unit is drawn whole or not at all, so the walk halts before one that would straddle — and a tab
  // can be 256 cells, so the row can end far short of what was allowed.
  //
  // Four tabs at a width of 4, so the line is 16 cells in units of 4 and a ceiling can be asked to
  // land inside one.
  let cells = measured("\t\t\t\t", 4);
  assert_eq!(cells.width(), 16);
  // The stop is reported in BYTES, which is the unit the row is replayed to. One tab is one byte
  // and four cells, so a stop of `n` bytes is `4n` cells and the two are the same statement.
  for (limit, tabs, elided) in [
    (16, 4, false),
    (15, 3, true),
    (13, 3, true),
    (12, 3, true),
    (11, 2, true),
    (3, 0, true),
    (0, 0, true),
  ] {
    let (_, row) = one_mark(&cells, Span::new(0, 4), cells_only(limit));
    assert_eq!(
      (row.drawn_end, row.elided),
      (tabs, elided),
      "a ceiling of {limit} cells over four four-cell tabs"
    );
    assert!(
      row.drawn_end as u64 * 4 <= limit,
      "a ceiling of {limit} let {} cells through",
      row.drawn_end * 4
    );
  }

  // And a span reaching past the drawn text is marked over the elision cell, which sits at
  // `visible + 1`. Without that the underline would stop at the last drawn cell and claim the span
  // ended with the row.
  let (columns, row) = one_mark(&cells, Span::new(0, 4), cells_only(11));
  assert_eq!(row.drawn_end, 2, "two whole tabs fit under eleven cells");
  assert_eq!(columns, 1..10, "the mark does not reach the `…` cell");

  // A span that begins past the window has nowhere of its own, so it anchors on that same cell.
  let (columns, _) = one_mark(&cells, Span::new(3, 4), cells_only(11));
  assert_eq!(columns, 9..10);
}

/// Where a terminal paints each cluster of a line, written from the terminal side.
///
/// Hand-written rather than computed, and that is the point: a table derived from the crate's own
/// units would agree with any implementation of them. A row is the clusters in order with the cells
/// each occupies, and the line is their concatenation — so the BOUNDARIES are hand-written too, not
/// read out of the segmenter. That is what makes this an oracle rather than a restatement: the
/// crate's units are checked against a table computed by no library, and the segmenter is checked
/// against it as well.
///
/// Multi-cluster rows exist for the divergence set. `لا` is two clusters of one cell each, and
/// `unicode-width` scores the pair 1 — the whole subject of the round that produced this table. It
/// is pinned at two because that is what every terminal does, and because a row that pins it is
/// what makes the rejection falsifiable instead of merely asserted.
type Placement = (&'static [(&'static str, u64)], &'static str);

const PLACEMENTS: [Placement; 17] = [
  (
    &[("\u{2603}\u{fe0f}", 2), ("x", 1)],
    "an emoji-presentation snowman is two cells, not the one its base character measures",
  ),
  (
    &[("1\u{fe0f}\u{20e3}", 2), ("x", 1)],
    "a keycap is drawn as one two-cell key",
  ),
  (
    &[("\u{231a}\u{fe0e}", 1), ("x", 1)],
    "and VS15 the other way: text presentation narrows the watch to one cell",
  ),
  (
    &[("\u{1f1ef}\u{1f1f5}", 2), ("x", 1)],
    "a flag is one two-cell glyph, not two halves",
  ),
  (
    &[("\u{1f1ef}\u{1f1f5}", 2), ("\u{1f1ef}", 1), ("x", 1)],
    "and a THIRD regional indicator starts a new cluster — GB12/13 pair from the left, so an odd \
     one is alone and one cell",
  ),
  (
    &[("\u{1f44d}\u{1f3fd}", 2), ("x", 1)],
    "a skin-tone modifier joins its base: two cells, where a legacy terminal advances four",
  ),
  (
    &[("\u{915}\u{94d}\u{937}\u{93f}", 3), ("x", 1)],
    "a conjunct is one cluster, and its cells are the whole of it",
  ),
  (
    &[("\u{e01}\u{e33}", 2), ("x", 1)],
    "the vowel sign adds a cell to its base's cluster",
  ),
  (
    &[("\u{1100}\u{1161}\u{11a8}", 2), ("x", 1)],
    "Hangul lead, vowel and trail are one syllable in two cells",
  ),
  (
    &[("\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}", 2), ("x", 1)],
    "the joined family, kept from the round before so that fix stays fixed",
  ),
  (
    &[("a\u{200d}", 1), ("b", 1), ("x", 1)],
    "a lone ZWJ between letters does NOT join them — merging these was the retired oracle's own \
     defect, so it is pinned here",
  ),
  (
    &[("\u{644}", 1), ("\u{627}", 1), ("x", 1)],
    "Arabic lam then alef: two cells. `unicode-width` scores the pair 1, and this row is the \
     rejection of that rule made falsifiable — every terminal advances two, because the lam's \
     cluster has closed and may already have been reported past before the alef arrives",
  ),
  (
    &[("\u{644}\u{651}", 1), ("\u{627}", 1), ("x", 1)],
    "and with a transparent between them: the shadda folds into the lam's cluster, so the shape is \
     unchanged and the pair is still two cells",
  ),
  (
    &[("\u{5d0}\u{200d}", 1), ("\u{5dc}", 1), ("x", 1)],
    "Hebrew alef-ZWJ-lamed, the same rejection through a different rule: the ZWJ joins the alef's \
     cluster and the lamed still starts its own",
  ),
  (
    &[("\u{2d31}\u{2d7f}", 2), ("\u{2d3e}", 1), ("x", 1)],
    "Tifinagh with a consonant joiner — three cells against the string model's one, the widest gap \
     between the two models and so the row that would move furthest if anyone adopted it",
  ),
  (
    &[("\u{1780}\u{17d2}\u{1781}", 1), ("x", 1)],
    "Khmer coeng is listed among those same string rules and AGREES on this data: it is one \
     cluster. The canary — the divergence set moves with Unicode versions, and this row is where \
     that shows up",
  ),
  (
    &[("\u{1161}", 0), ("x", 1)],
    "a lone jungseong is a cluster of no cells at all, so the model has to place a marker for \
     something that occupies nothing",
  ),
];

#[test]
fn a_cluster_occupies_its_own_cells_and_the_next_character_follows_them() {
  // R1 was prefix slicing; R2 was prefix-width extension. Both derived a unit's EXTENT from
  // incremental prefix measurement, which is how neither grapheme segmentation nor display width is
  // defined, and both put every marker after a variation-selector sequence one cell early.
  for (row, why) in PLACEMENTS {
    let text: String = row.iter().map(|(cluster, _)| *cluster).collect();
    let cells = measured(&text, 4);

    let mut at = 0;
    let mut column = 1;
    for (cluster, expected) in row {
      let end = at + cluster.len();
      let after = column + expected;

      // Every offset inside the cluster reports where the cluster begins.
      for inside in at..end {
        if text.is_char_boundary(inside) {
          assert_eq!(
            cells.column_at(inside),
            column,
            "{text:?}: offset {inside} in cluster {cluster:?} — {why}"
          );
        }
      }
      // And a span over the cluster covers exactly its own cells — the whole of them, and none of
      // its neighbours'. A zero-cell cluster is the exception the model names: its marker is the
      // never-empty caret, which sits on the next unit's cell.
      let marked = cells.columns_for(Span::new(at, end));
      if *expected == 0 {
        assert_eq!(marked, column..column, "{text:?}: {cluster:?} — {why}");
      } else {
        assert_eq!(marked, column..after, "{text:?}: {cluster:?} — {why}");
      }

      at = end;
      column = after;
    }

    assert_eq!(
      cells.width(),
      column - 1,
      "{text:?}: the line is its clusters' cells — {why}"
    );
  }
}

#[test]
fn the_hand_written_boundaries_are_the_ones_uax29_gives() {
  // The other direction on the table above, and the reason it is worth having twice: those rows are
  // hand-written, so they can be wrong. If the segmenter disagrees with a row, one of the two is
  // and the test says which input to look at — rather than the crate quietly agreeing with whichever
  // of them it happens to be built on.
  use unicode_segmentation::UnicodeSegmentation;

  for (row, why) in PLACEMENTS {
    let text: String = row.iter().map(|(cluster, _)| *cluster).collect();
    let written: Vec<&str> = row.iter().map(|(cluster, _)| *cluster).collect();
    let segmented: Vec<&str> = text.graphemes(true).collect();
    assert_eq!(written, segmented, "{text:?}: {why}");
  }
}

#[test]
fn the_width_authority_and_the_boundary_authority_carry_the_same_data() {
  // Two crates, two Unicode tables, and they version independently. A skew moves cluster boundaries
  // under width numbers that were measured against the old ones — which is silent, because both
  // crates keep working. Named here so it becomes a failure instead.
  // Spelled with different integer types by the two crates, so compared component by component
  // rather than as tuples.
  let (width_major, width_minor, width_patch) = unicode_width::UNICODE_VERSION;
  let (seg_major, seg_minor, seg_patch) = unicode_segmentation::UNICODE_VERSION;
  assert_eq!(
    (
      u64::from(width_major),
      u64::from(width_minor),
      u64::from(width_patch)
    ),
    (seg_major, seg_minor, seg_patch),
    "the width authority and the boundary authority are on different Unicode data"
  );
}

#[test]
fn a_unicode_data_update_must_re_derive_this_set() {
  // `DIVERGENCE` is a survey, and the suite cannot repeat it. `unicode-width` exposes only this
  // constant and two sealed traits, so the only way to look for a SEVENTH cross-cluster family from
  // outside is to measure strings — and the pairs alone are the square of the code space, before
  // the Arabic rule's arbitrarily many interposed transparents. A search narrowed to the scripts
  // already known to diverge would be re-deriving the list it is checking.
  //
  // So the trigger is pinned rather than the result. A Unicode release is what introduces a script
  // ligature rule, and this stops the suite when one lands, instead of letting six hand-surveyed
  // families keep describing data they were not surveyed against.
  //
  // If this fires, the fix is NOT to bump the literal. Re-run the survey against the new data, and
  // correct `DIVERGENCE` and `PLACEMENTS` first — bumping it alone converts a caught change into an
  // uncaught one, which is the whole failure this exists to prevent.
  //
  // Only the width authority is spelled out. `the_width_authority_and_the_boundary_authority_carry_
  // the_same_data` holds the segmenter equal to it, so one literal pins both and there is no second
  // number to leave behind.
  assert_eq!(
    unicode_width::UNICODE_VERSION,
    (17, 0, 0),
    "the width tables moved to new Unicode data; the divergence set was surveyed against 17.0.0 and \
     has to be surveyed again before this number changes"
  );
}

#[test]
fn a_placement_unit_is_a_grapheme_cluster() {
  // A conformance check rather than a cross-check: it restates that the crate asks the authority,
  // and its value is that it fails the moment something starts inferring extents again. The
  // evidence that the authority is the RIGHT model is in `PLACEMENTS`, whose numbers are what a
  // terminal paints and come from no library at all.
  use unicode_segmentation::UnicodeSegmentation;

  for (text, why) in CORPUS {
    let cells = measured(text, 4);
    let boundaries: Vec<usize> = text.grapheme_indices(true).map(|(at, _)| at).collect();
    for (at, cluster) in text.grapheme_indices(true) {
      let column = cells.column_at(at);
      for inside in at..at + cluster.len() {
        if text.is_char_boundary(inside) {
          assert_eq!(
            cells.column_at(inside),
            column,
            "{text:?}: offset {inside} is inside the cluster at {at} — {why}"
          );
        }
      }
      // And the boundary after it starts a new column, so units are not merged either.
      let after = at + cluster.len();
      if boundaries.contains(&after) && cluster != "\u{200b}" {
        assert!(
          cells.column_at(after) >= column,
          "{text:?}: the cluster after {at} went backwards — {why}"
        );
      }
    }
  }
}

/// The inputs where per-cluster placement and whole-string measurement give different answers.
///
/// Both values, exactly. A one-sided assertion would let a `unicode-width` bump that drops a rule
/// reclassify an input as agreeing and stay green — so each of these is pinned from both ends, and
/// a change to one of THEM fails loudly with the input in hand.
///
/// Six families, SURVEYED against Unicode 17 data rather than computed. What these rows establish
/// is the behaviour of the inputs in them; a seventh family arriving upstream is not something they
/// can report, because nothing here goes looking for one. That gap is named rather than papered
/// over: `a_unicode_data_update_must_re_derive_this_set` pins the event that would introduce one.
/// Every rule that operates INSIDE one cluster agrees, and those are in `PLACEMENTS` above.
const DIVERGENCE: [(&str, u64, u64, &str); 8] = [
  ("\u{644}\u{627}", 2, 1, "Arabic lam-alef"),
  ("\u{644}\u{622}", 2, 1, "and the alef-madda form of it"),
  (
    "\u{644}\u{651}\u{627}",
    2,
    1,
    "and with a transparent interposed, which folds into the lam's cluster",
  ),
  ("\u{5d0}\u{200d}\u{5dc}", 2, 1, "Hebrew alef-ZWJ-lamed"),
  ("\u{1a15}\u{1a17}\u{200d}\u{1a10}", 2, 1, "Buginese"),
  ("\u{a4f8}\u{a4fc}", 2, 1, "Lisu tone pair"),
  ("\u{10c32}\u{200d}\u{10c03}", 2, 1, "Old Turkic"),
  (
    "\u{2d31}\u{2d7f}\u{2d3e}",
    3,
    1,
    "Tifinagh, the widest gap between the two models",
  ),
];

#[test]
fn a_lines_width_is_the_sum_of_its_clusters() {
  // The model, stated as an assertion: `width` is the clusters' cells added up. Held over every
  // input, including the ones where whole-string measurement disagrees — which is the next test.
  use unicode_segmentation::UnicodeSegmentation;
  use unicode_width::UnicodeWidthStr;

  let divergent = DIVERGENCE.map(|(text, _, _, _)| text);
  for (text, why) in CORPUS {
    if text.contains('\t') {
      continue;
    }
    let summed: u64 = text.graphemes(true).map(|c| c.width() as u64).sum();
    assert_eq!(measured(text, 4).width(), summed, "{text:?}: {why}");
    // And for everything outside the named set the two models coincide, so a NEW divergence — a
    // rule added to `unicode-width`, or a boundary moved under it — is reported rather than
    // absorbed.
    if !divergent.iter().any(|member| text.contains(member)) {
      assert_eq!(
        summed,
        text.width() as u64,
        "{text:?} has started diverging from whole-string width. Add it to `DIVERGENCE` with both \
         values and keep the per-cluster model — do not switch to whole-string measurement, which \
         cannot place a marker: {why}"
      );
    }
  }
}

#[test]
fn the_rejected_string_rules_are_pinned_at_both_values() {
  // The rejection made falsifiable. Each of these is a case where `unicode-width` scores the whole
  // string narrower than its clusters, and painty deliberately reports the clusters, because the
  // first one has closed — and may already have been reported past by a `CSI 6n` — before the
  // second arrives. No cursor-addressable terminal can implement the narrowing.
  //
  // Pinned at BOTH numbers so the set cannot change quietly in either direction: a rule dropped
  // upstream fires the second assertion, a boundary moved fires the first.
  use unicode_segmentation::UnicodeSegmentation;
  use unicode_width::UnicodeWidthStr;

  for (text, clustered, whole, why) in DIVERGENCE {
    let summed: u64 = text.graphemes(true).map(|c| c.width() as u64).sum();
    assert_eq!(summed, clustered, "{why}: the clusters' cells moved");
    assert_eq!(
      text.width() as u64,
      whole,
      "{why}: `unicode-width`'s rule for this changed"
    );
    assert_ne!(clustered, whole, "{why} is in the wrong table — it agrees");
    assert_eq!(
      measured(text, 4).width(),
      clustered,
      "{why}: painty followed the string model"
    );

    // And the placement consequence, which is the thing that would actually be wrong: the follower
    // sits after all the clusters' cells, not after the ligature's.
    let line = format!("{text}x");
    let cells = measured(&line, 4);
    assert_eq!(
      cells.column_at(text.len()),
      clustered + 1,
      "{why}: the character after the pair moved to where no terminal puts it"
    );
  }
}

#[test]
fn expanding_a_line_forwards_no_control_character() {
  // `write_expanded` is public: a caller drawing its own excerpt gets whatever this writes, with no
  // renderer in between to catch an escape. The renderer does substitute a second time on the way
  // out — which is why a plant that disabled this one survived until this test existed.
  let controls = (0u32..=0x1f).chain(0x7fu32..=0x9f);
  for raw in controls.filter_map(char::from_u32) {
    if raw == '\t' || raw == '\n' || raw == '\r' {
      continue;
    }
    let text = format!("let {raw}x = 1;\n");
    let mut expanded = String::new();
    measured(&text, 4)
      .write_expanded(&mut expanded)
      .expect("a String is writable");
    assert!(
      !expanded.contains(raw),
      "U+{:04X} was written to the terminal",
      raw as u32
    );
    assert!(
      expanded.contains(control_picture_for(raw)),
      "U+{:04X} was dropped rather than shown",
      raw as u32
    );
  }
}

/// The stand-in a character is expected to be shown as, stated here rather than imported, so the
/// test does not restate whatever the crate happens to do.
fn control_picture_for(character: char) -> char {
  match character {
    '\u{7f}' => '\u{2421}',
    '\u{80}'..='\u{9f}' => '\u{fffd}',
    _ => char::from_u32(0x2400 + character as u32).expect("the block is contiguous"),
  }
}

#[test]
fn a_line_terminator_never_reaches_this_layer() {
  // Layer 2 hands out line text with the break excluded, and every column here depends on that:
  // `"\r\n"` is ONE cluster of one cell, so a terminator that slipped through would not merely add
  // a trailing cell — it would make the line's last cluster the break, and any marker resolved
  // against a following line would be measured against the wrong text. Pinned as its own class
  // rather than left as a property of another layer's implementation.
  for text in ["alpha\nbeta\n", "alpha\r\nbeta\r\n", "alpha\rbeta\r"] {
    let source = Source::new(text);
    let mut number = 1;
    while let Some(line) = source.line(number) {
      assert!(
        !line.text().contains(['\r', '\n', '\u{2028}']),
        "{text:?} line {number} carries its terminator: {:?}",
        line.text()
      );
      number += 1;
    }
  }

  // And the converse, so the set is closed rather than open-ended: LF, CRLF and CR are the breaks,
  // and U+2028 LINE SEPARATOR is ordinary text that measures like any other cluster. A renderer
  // that started splitting on it would put half a line under a marker measured against the whole.
  let separator = Source::new("alpha\u{2028}beta");
  assert!(separator.line(2).is_none(), "U+2028 split a line");
  assert_eq!(measured("alpha\u{2028}beta", 4).width(), 10);
}

#[test]
fn the_ambiguous_width_convention_is_the_one_painty_measures_against() {
  // `unicode-width` also ships `width_cjk`, where East Asian Ambiguous characters are two cells
  // instead of one. Every number in `PLACEMENTS` and `DIVERGENCE` was written against the plain
  // one, so a call to the CJK variant appearing anywhere in the crate would silently reinterpret
  // the whole table. Asserted at a character that distinguishes them rather than by grepping for a
  // name, so a re-export or an alias cannot get past it.
  use unicode_width::UnicodeWidthStr;

  assert_eq!(
    "\u{2018}".width(),
    1,
    "the ambiguous convention has changed"
  );
  assert_eq!("\u{2018}".width_cjk(), 2, "and this is the one not taken");
  assert_eq!(
    measured("\u{2018}x", 4).column_at(3),
    2,
    "painty has started measuring ambiguous characters as two cells"
  );
}

/// One span through the geometry walk, which takes a whole line's worth of them at a time.
///
/// The walk is per LINE rather than per mark — one row is drawn, so one walk places everything
/// under it — and these cases are each about one span, so they go through the same door carrying
/// one.
fn one_mark(
  cells: &LineCells<'_>,
  span: Span,
  budget: super::width::Budget,
) -> (core::ops::Range<u64>, super::width::Row) {
  let mut marks = [super::width::Mark::new(span, ())];
  let row = cells.place_marks(&mut marks, budget);
  (marks[0].columns(), row)
}

/// No ceiling at all, for holding the bounded walk against the unbounded one.
fn unbounded() -> super::width::Budget {
  super::width::Budget {
    cells: u64::MAX,
    bytes: u64::MAX,
  }
}

/// A cell ceiling with no byte bound, for the cases that are about layout.
fn cells_only(cells: u64) -> super::width::Budget {
  super::width::Budget {
    cells,
    bytes: u64::MAX,
  }
}

#[test]
fn a_zero_width_run_is_stopped_by_the_byte_budget_and_by_nothing_else() {
  // The denomination defect, at the unit that has it. A cell budget cannot see a cluster that
  // occupies no cells: `drawn` never advances, the check never trips, and a line of U+200B is
  // walked and written in full however long it is. Five rounds of ceilings each measured the
  // resource the last defect spent, and this one spends none of them.
  //
  // Bytes are what it does spend, because it had to be supplied.
  let zero_width = "\u{200b}".repeat(4_000);
  let cells = measured(&zero_width, 4);
  assert_eq!(cells.width(), 0, "the premise: this line occupies no cells");

  // Under a cell ceiling alone the walk reaches the end, which is exactly the hole.
  let (_, unstopped) = one_mark(&cells, Span::new(0, 3), cells_only(4_096));
  assert!(
    !unstopped.elided,
    "a cell ceiling cannot stop a line of no cells, and this test would be proving nothing"
  );
  assert_eq!(unstopped.drawn_end, zero_width.len());

  // Under a byte budget it stops, and stops where the budget says.
  let budget = super::width::Budget {
    cells: 4_096,
    bytes: 900,
  };
  let (_, stopped) = one_mark(&cells, Span::new(0, 3), budget);
  assert!(stopped.elided, "the byte budget did not stop the walk");
  assert!(
    stopped.drawn_end <= 900,
    "the walk examined {} bytes against a budget of 900",
    stopped.drawn_end
  );

  // And the writer replays to that offset rather than re-deriving one, so it cannot disagree.
  let mut written = String::new();
  cells
    .write_expanded_upto(&mut written, stopped.drawn_end)
    .expect("a String is writable");
  assert_eq!(
    written.len(),
    stopped.drawn_end,
    "the row written is not the row the geometry measured"
  );
}

#[test]
fn a_combining_mark_run_is_bounded_the_same_way() {
  // The other member of the class, and a different shape: these clusters are not zero-width because
  // they are invisible, but because they attach. A base followed by ten thousand combining marks is
  // ONE cluster of one cell, so a cell budget sees a single unit and stops nowhere at all.
  let combining = format!("a{}", "\u{301}".repeat(10_000));
  let cells = measured(&combining, 4);
  assert_eq!(cells.width(), 1, "the premise: one base, one cell");

  let budget = super::width::Budget {
    cells: 4_096,
    bytes: 900,
  };
  let (_, row) = one_mark(&cells, Span::new(0, 1), budget);
  assert!(
    row.elided,
    "a twenty-kilobyte cluster passed a nine-hundred-byte budget"
  );
  assert!(
    row.drawn_end <= 900,
    "the walk examined {} bytes against a budget of 900",
    row.drawn_end
  );
}

#[test]
#[cfg_attr(
  miri,
  ignore = "an asymptotic measurement over sixteen megabytes of input, which Miri would take hours \
            to walk and is not checking anyway"
)]
fn the_byte_budget_bounds_what_is_examined_and_not_only_what_is_drawn() {
  // The budget said "bytes examined" and checked a unit's end AFTER the segmenter had produced it.
  // A grapheme cluster has no length limit, so the first unit can be the whole line: the check
  // fired once, having read every byte to get there.
  //
  // A RATIO, and for the reason `the_geometry_costs_the_window_and_not_the_line` uses one — machine
  // speed cancels, so this cannot flake on a loaded runner or pass on a fast one, and it states the
  // property rather than a number. Eight times the input for the same budget: a walk that follows
  // the cluster shows up as very nearly eight, one that stops at the budget as very nearly one.
  // With the check restored in place of the slice it measures 8.12x here, which is the 8.21x the
  // audit reported; sliced, it is 1.00x.
  fn one_cluster(megabytes: usize) -> core::time::Duration {
    // One base and as many combining marks as fit, which UAX#29 calls a single cluster.
    let text = format!("a{}", "\u{301}".repeat(megabytes * 500_000));
    let cells = measured(&text, 4);
    assert_eq!(cells.width(), 1, "the premise: one cluster, one cell");
    let budget = super::width::Budget {
      cells: 4_096,
      bytes: 65_536,
    };

    let _ = one_mark(&cells, Span::new(0, 1), budget);
    let started = std::time::Instant::now();
    for _ in 0..20 {
      let (_, row) = one_mark(&cells, Span::new(0, 1), budget);
      core::hint::black_box(row.drawn_end);
    }
    started.elapsed()
  }

  let small = one_cluster(2);
  let large = one_cluster(16);
  assert!(
    small > core::time::Duration::from_micros(50),
    "the smaller measurement is {small:?}, too close to the timer to divide by"
  );

  let growth = large.as_secs_f64() / small.as_secs_f64();
  assert!(
    growth < 3.0,
    "eight times the cluster cost {growth:.2} times the walk ({small:?} then {large:?}) — the \
     budget is being checked after the segmenter has read the unit rather than spent before it"
  );
}

#[test]
fn slicing_a_line_at_the_budget_does_not_change_the_units_before_the_cut() {
  // What the slice rests on. Handing the segmenter a prefix could in principle move a boundary
  // inside it, and a moved boundary is a marker in the wrong cell — the defect class three rounds
  // of review were about. It cannot move one, because a cluster's start is decided by what precedes
  // it and by the cluster before it, both of which are in the prefix; only the LAST cluster of the
  // prefix can be a fragment, and the walk drops it.
  //
  // Asserted rather than argued, at every cap of every corpus member.
  use unicode_segmentation::UnicodeSegmentation;

  for (text, why) in CORPUS {
    let whole: Vec<(usize, &str)> = text.grapheme_indices(true).collect();
    for cap in 0..=text.len() {
      if !text.is_char_boundary(cap) {
        continue;
      }
      let prefix: Vec<(usize, &str)> = text[..cap].grapheme_indices(true).collect();
      // Every cluster of the prefix but its last must be a cluster of the whole line, at the same
      // offset and with the same extent.
      let kept = prefix.len().saturating_sub(usize::from(cap < text.len()));
      assert_eq!(
        prefix[..kept],
        whole[..kept],
        "{text:?} cut at {cap}: the units before the cut moved — {why}"
      );
    }
  }
}

// ── Multi-line spans ────────────────────────────────────────────────────────────────────────────
//
// What Phase 2's gate names — CJK, combining marks, tabs, CRLF, a span over the final byte with no
// trailing newline — already had single-line coverage. What had none is any of them CROSSED with a
// span that opens on one line and closes on another, which is where the two ends are measured
// against DIFFERENT lines and a connector has to agree with both.

/// One multi-line case, and the hazard it crosses the multi-line axis with.
///
/// The first span is the primary; the rest arrive as labels. They are permuted, so which one is
/// primary is not a property of the case.
struct Bracketed {
  text: &'static str,
  spans: Vec<Span>,
  why: &'static str,
}

fn bracketed() -> Vec<Bracketed> {
  let case = |text: &'static str, spans: &[Span], why: &'static str| Bracketed {
    text,
    spans: spans.to_vec(),
    why,
  };
  vec![
    case(
      "query Hero {\n  hero {\n    name\n    friends\n  }\n}\n",
      &[Span::new(15, 44)],
      "a block whose opening line holds nothing before it, which is the compact form",
    ),
    case(
      "let x = if a {\n  1\n} else {\n  2\n};\n",
      &[Span::new(8, 33)],
      "an opening with text before it, which needs a corner row of its own",
    ),
    case(
      "let \u{65e5}\u{672c}\u{8a9e} =\n\tvalue;\n",
      &[Span::new(6, 25)],
      "opens INSIDE a CJK run and closes after a tab: two cells one end, a tab stop the other",
    ),
    case(
      "alpha\r\nbeta\r\ngamma\r\n",
      &[Span::new(2, 15)],
      "crosses CRLF boundaries, where a line break is two bytes and no column",
    ),
    case(
      "a\u{301}bc\ndef\n",
      &[Span::new(1, 8)],
      "opens inside a combining sequence, so the opening widens back to its base",
    ),
    case(
      "alpha\nbeta",
      &[Span::new(3, 10)],
      "closes on the final byte of a source with no trailing newline",
    ),
    case(
      "aaa\nbbb\nccc\n",
      &[Span::new(0, 8)],
      "swallows its own trailing newline, so it ENDS on a line it is not drawn on",
    ),
    case(
      "aaa\n\nccc\n",
      &[Span::new(0, 5)],
      "closes on an empty line, where the covered range is empty and the caret is still a caret",
    ),
    case(
      "outer (\n  inner (\n    x\n  )\n)\n",
      &[Span::new(6, 29), Span::new(10, 26)],
      "two multi-line spans open at once, which is what needs a second column",
    ),
    case(
      "a (\n b\n)\nc (\n d\n)\n",
      &[Span::new(2, 8), Span::new(11, 17)],
      "two multi-line spans that do NOT overlap, which need only one column between them",
    ),
    case(
      "start {\n a\n b\n c\n d\n e\n f\n g\n h\n i\n j\n}\nend\n",
      &[Span::new(6, 39)],
      "long enough that the middle is elided, which is what bounds the rows",
    ),
    case(
      "q {\n  a\n  b\n  c\n}\n",
      &[Span::new(2, 17), Span::new(10, 11)],
      "a single-line label INSIDE a multi-line span, whose row carries the connector too",
    ),
    case(
      "one\ntwo\nthree\nfour\nfive\n",
      &[Span::new(4, 18), Span::new(0, 3)],
      "a single-line label ABOVE one, so the caller's order and the source order disagree",
    ),
    case(
      "a {\n\tb\n\tc\n}\n",
      &[Span::new(2, 11)],
      "tabs inside the bracketed lines, where a cell is not a character",
    ),
  ]
}

/// The tab width every multi-line case is measured at.
const BRACKETED_TAB: u64 = 4;

fn render_spans(text: &str, spans: &[Span]) -> String {
  let labels: Vec<Label<'_>> = spans[1..]
    .iter()
    .map(|span| Label::new(Location::new(0, *span), "there"))
    .collect();
  let message = "a message";
  let diagnostic = diagnose(Location::new(0, spans[0]), &labels, &message);
  let mut out = String::new();
  Terminal::plain()
    .with_tab_width(BRACKETED_TAB)
    .render(&diagnostic, &[Input::new(Source::new(text))], &mut out)
    .expect("a String never fails to be written to");
  out
}

/// Every ordering of `count` positions, so that "the caller's order" is not one arrangement the
/// renderer happens to be right about.
fn orderings(count: usize) -> Vec<Vec<usize>> {
  let mut all = vec![Vec::new()];
  for _ in 0..count {
    let mut next = Vec::new();
    for order in &all {
      for candidate in 0..count {
        if !order.contains(&candidate) {
          let mut grown = order.clone();
          grown.push(candidate);
          next.push(grown);
        }
      }
    }
    all = next;
  }
  all
}

/// The line a rendered row is about, and where the source text starts in it.
///
/// Read out of the row rather than computed from a gutter width and a connector count, because
/// those are precisely what a layout defect moves. The line is expanded by `LineCells` and matched
/// as the row's suffix, so the offset is whatever the renderer actually put in front of it.
fn source_row(row: &str, source: Source<'_>) -> Option<(u64, usize, usize)> {
  let characters: Vec<char> = row.chars().collect();
  let indent = characters.iter().take_while(|c| **c == ' ').count();
  let digits: String = characters[indent..]
    .iter()
    .take_while(|c| c.is_ascii_digit())
    .copied()
    .collect();
  if digits.is_empty() {
    return None;
  }
  let after: String = characters[indent + digits.len()..]
    .iter()
    .take(3)
    .copied()
    .collect();
  if after != " | " {
    return None;
  }
  let number: u64 = digits.parse().expect("a rendered line number is a number");
  let line = source
    .line(number)
    .expect("a rendered line number names a line of the input");
  let mut expanded = String::new();
  LineCells::new(line, BRACKETED_TAB)
    .write_expanded(&mut expanded)
    .expect("a String never fails to be written to");
  assert!(
    row.ends_with(&expanded),
    "the row for line {number} does not end with the line it claims to show:\n{row:?}\n{expanded:?}"
  );
  Some((
    number,
    indent + digits.len() + 1,
    characters.len() - expanded.chars().count(),
  ))
}

/// Every cell the rendered output marks, as `(line, display column)`.
///
/// Underscores and bars are connectors and are deliberately not counted: what a bracket asserts is
/// that its two ENDS are where they are, and the run between them is how a reader gets from one to
/// the other.
fn marked(rendered: &str, source: Source<'_>) -> Vec<(u64, u64)> {
  let mut cells = Vec::new();
  let mut about = None;
  for row in rendered.lines() {
    if let Some(found) = source_row(row, source) {
      about = Some(found);
      continue;
    }
    let Some((number, bar, code)) = about else {
      continue;
    };
    let characters: Vec<char> = row.chars().collect();
    // An under-row carries the gutter's bar in the gutter's column and nothing before it. The
    // header, the `= help` line and the `...` of an elision all fail that and are skipped.
    if characters.get(bar) != Some(&'|') || characters[..bar].iter().any(|c| *c != ' ') {
      continue;
    }
    let Some(start) = characters
      .iter()
      .skip(code)
      .position(|c| *c == '^' || *c == '-')
    else {
      continue;
    };
    let from = code + start;
    let run = characters[from..]
      .iter()
      .take_while(|c| **c == '^' || **c == '-')
      .count();
    for step in 0..run {
      cells.push((
        number,
        u64::try_from(from + step - code + 1).expect("a column"),
      ));
    }
  }
  cells.sort_unstable();
  cells
}

/// Whether a span's opening is announced by a `/` in the margin instead of by a marker of its own.
///
/// The renderer's rule restated from the spec rather than read back off the renderer: a multi-line
/// span whose opening line carries no other mark and holds nothing but blanks before it opens with
/// a `/` and no corner row, so its start cell is not marked. What that costs a reader is which
/// column INSIDE THE INDENTATION the span begins at, and what it buys is a row.
///
/// The renderer has a THIRD condition that this deliberately does not restate: the row must draw
/// the cell the span opens at, or the `/` would stand in for a marker that was never on the page.
/// It is left out because the corpus cannot reach it — every case here is asserted to render
/// without a `…`, so every opening in it is drawn, and a condition that is constantly true would
/// only be an untested branch of the oracle. The cases that DO reach it are
/// `an_opening_the_row_does_not_draw_is_marked_rather_than_compacted` and its pair, which read the
/// rows by index for exactly that reason.
fn opens_compactly(source: Source<'_>, spans: &[Span], span: Span) -> bool {
  let region = source.resolve(span);
  if !region.is_multiline() {
    return false;
  }
  let first = region.start().line();
  let marks = spans
    .iter()
    .map(|other| {
      let other = source.resolve(*other);
      let opens = usize::from(other.start().line() == first);
      let closes =
        usize::from(other.is_multiline() && other.start().line() + other.line_count() - 1 == first);
      opens + closes
    })
    .sum::<usize>();
  if marks != 1 {
    return false;
  }
  let drawn = region.lines().next().expect("a region covers a line");
  let line = drawn.line();
  let before = drawn.covered().start() - line.span().start();
  line.text()[..before].chars().all(char::is_whitespace)
}

/// Which cells a set of spans ASKS to have marked.
///
/// Layer 2 decides the lines and the ends; [`Terminal::underline`] decides the cells, and it is the
/// public entry the hand-written `PLACEMENTS` table already holds to the cell grid. The two
/// dimensions are separated on purpose: what this oracle must not share with the renderer is how
/// rows and ends are ASSIGNED, and that comes from `Region::lines` rather than from the walk the
/// renderer uses — which `walking_a_set_of_spans_answers_what_resolving_each_one_does` holds it to.
fn asked_for(source: Source<'_>, spans: &[Span]) -> Vec<(u64, u64)> {
  let terminal = Terminal::plain().with_tab_width(BRACKETED_TAB);
  let mut cells = Vec::new();
  for span in spans {
    let region = source.resolve(*span);
    let drawn: Vec<_> = region.lines().collect();
    let first = *drawn.first().expect("a region covers at least one line");
    let last = *drawn.last().expect("a region covers at least one line");
    if drawn.len() == 1 {
      for column in terminal.underline(first) {
        cells.push((first.line().number(), column));
      }
    } else {
      // Two ends, and only two: a bracket says where a span starts and where it stops, and the
      // lines between are shown rather than marked.
      if !opens_compactly(source, spans, *span) {
        cells.push((first.line().number(), terminal.underline(first).start));
      }
      cells.push((last.line().number(), terminal.underline(last).end - 1));
    }
  }
  cells.sort_unstable();
  cells
}

#[test]
fn which_cells_are_marked_does_not_depend_on_how_the_rows_were_assigned() {
  // The property goldens cannot state. A golden pins one arrangement of rows; this pins that the
  // arrangement is not what decides WHAT is pointed at — the same spans mark the same cells of the
  // same lines however the rows came out, and those cells are the ones layer 2 asked for.
  //
  // Two failure directions, and it is worth naming them because a weaker version of this test would
  // catch only the second. Against the oracle, it catches a bracket that closes on the wrong line
  // or marks the wrong end of it — including the shape this feature replaced, where a multi-line
  // span was drawn on its first line and nowhere else. Against the permutations, it catches an
  // arrangement that is right only for the order the case was written in.
  for case in bracketed() {
    let source = Source::new(case.text);
    let oracle = asked_for(source, &case.spans);
    for order in orderings(case.spans.len()) {
      let spans: Vec<Span> = order.iter().map(|index| case.spans[*index]).collect();
      let rendered = render_spans(case.text, &spans);
      assert!(
        !rendered.contains('\u{2026}'),
        "{:?}: a case elided its source, so the parse below is not exact\n{rendered}",
        case.text
      );
      assert_eq!(
        marked(&rendered, source),
        oracle,
        "{:?} in caller order {order:?}: {}\n{rendered}",
        case.text,
        case.why
      );
      // And the one opening that has no marker still says which line it is on, because the `/`
      // that stands in for it is on that line's own row. Both directions: every compact opening
      // has one, and no other row does.
      assert_eq!(
        compact_rows(&rendered, source),
        spans
          .iter()
          .filter(|span| opens_compactly(source, &spans, **span))
          .map(|span| source.resolve(*span).start().line())
          .collect::<Vec<_>>(),
        "{:?} in caller order {order:?}: {}\n{rendered}",
        case.text,
        case.why
      );
    }
  }
}

/// The lines whose source row opens a span with a `/`.
fn compact_rows(rendered: &str, source: Source<'_>) -> Vec<u64> {
  let mut found = Vec::new();
  for row in rendered.lines() {
    let Some((number, bar, code)) = source_row(row, source) else {
      continue;
    };
    let characters: Vec<char> = row.chars().collect();
    for _ in characters[bar + 2..code].iter().filter(|c| **c == '/') {
      found.push(number);
    }
  }
  found.sort_unstable();
  found
}

// ── An opening the row does not draw ────────────────────────────────────────────────────────────
//
// The corpus above is read back with `source_row`, which finds a row's geometry by matching the
// whole expanded line as the row's suffix — so it can only describe a row that shows all of its
// line, and `which_cells_are_marked_does_not_depend_on_how_the_rows_were_assigned` asserts outright
// that no case elided. That is the right premise for what it pins and it is why the corpus cannot
// reach these cases: the whole subject here is a row the CELL budget cut.
//
// Read by index into the row instead. Every row of a block shares one prefix, so a character index
// is the alignment a reader checks — a caret is under the thing it points at exactly when the two
// indices agree.

/// What every row of the fixtures below carries before its source text: line 1, the gutter's bar,
/// one blank connector column, and the blank that separates it from the source. Its length is
/// therefore the character index of display column 1.
///
/// A literal rather than a measurement, because measuring it is what these fixtures deny: the row
/// does not show its line, so nothing here can be located by matching what the row was meant to
/// show. Each case asserts the row begins with it, which is what stops a change in the geometry
/// quietly re-basing every index below.
const FIELD: &str = "1 |   ";

/// A prefix long enough that the row drawn for it stops before it ends, and what it is made of.
///
/// Both overrun [`Terminal::max_rendered_width`] in CELLS while staying well inside
/// [`Terminal::max_source_bytes`], which is the gap the defect lived in: the compact decision asked
/// the byte budget whether the opening could be read and drew its conclusion about a row that the
/// cell budget had already cut.
fn overrunning_prefixes() -> Vec<(String, &'static str)> {
  vec![
    (
      " ".repeat(5_000),
      "five thousand spaces, which is five thousand cells and five thousand bytes",
    ),
    (
      "\t".repeat(2_000),
      "two thousand tabs, which is eight thousand cells for two thousand bytes — the amplification \
       a byte budget cannot see",
    ),
  ]
}

/// The row that shows line 1, and the character index of every marker drawn under it.
fn opening_row(rendered: &str) -> (String, Vec<usize>) {
  let mut rows = rendered.lines();
  let source = rows
    .by_ref()
    .find(|row| row.starts_with("1 | "))
    .expect("line 1 was drawn")
    .to_owned();
  let mut markers = Vec::new();
  for row in rows {
    // An under-row carries the gutter's bar and a blank after it. The next source row, the closing
    // bar and the `= help` line all fail that, and each of them ends what is under line 1.
    if !row.starts_with("  | ") {
      break;
    }
    if let Some(at) = row.chars().position(|c| c == '^' || c == '-') {
      markers.push(at);
    }
  }
  markers.sort_unstable();
  (source, markers)
}

#[test]
fn an_opening_the_row_does_not_draw_is_marked_rather_than_compacted() {
  // The compact opening trades a span's start MARKER for a `/` in the margin, and that trade is
  // only a saving when the cell the marker would have gone on is on the page. Behind a prefix wider
  // than the row, it is not: the reader is left with a `/` and nothing at all pointing into the
  // row, for a span whose opening the renderer never drew.
  //
  // Both halves of the defect are pinned, and the second is why each prefix is rendered twice. The
  // suppression was decided by `alone` — one anchor on the line — so an unrelated label on that
  // same line turned it off and the span suddenly acquired a start marker at the elision cell. The
  // marked set moved for a reason that is not about either span's endpoints. Here the pair must
  // agree: the unrelated label adds its own cell and moves nothing.
  for (prefix, why) in overrunning_prefixes() {
    let text = format!("{prefix}open\nclose\n");
    let opens = Span::new(
      prefix.len(),
      text.find("close").expect("closes") + "close".len(),
    );
    // Inside the indentation, so it is a mark on line 1 and nothing else: what it exists to do is
    // make `alone` false.
    let unrelated = Span::new(0, 1);

    let mut rows = Vec::new();
    for spans in [vec![opens], vec![opens, unrelated]] {
      let shared = spans.len() > 1;
      let rendered = render_spans(&text, &spans);
      let (source, markers) = opening_row(&rendered);
      let ellipsis = source
        .chars()
        .position(|c| c == '\u{2026}')
        .unwrap_or_else(|| panic!("the premise: the row for {why} was not cut"));
      assert!(
        source.starts_with(FIELD),
        "the opening was compacted onto a row that stops at {ellipsis}, or the geometry moved: \
         {why}"
      );
      // The marker is under the `…`, which is where the rest of the line went and so is where a
      // reader is told to look for the opening.
      assert_eq!(
        markers,
        if shared {
          vec![FIELD.len(), ellipsis]
        } else {
          vec![ellipsis]
        },
        "{why}, {}an unrelated label",
        if shared { "with " } else { "without " }
      );
      rows.push((source.len(), ellipsis));
    }
    // Not the assertion above restated: that one holds each render to the cell the span resolves
    // to, this one holds the ROW steady, so the pair cannot agree by both having moved.
    assert_eq!(rows[0], rows[1], "the row itself moved: {why}");
  }
}

#[test]
fn an_opening_the_row_does_draw_still_opens_compactly_on_a_cut_row() {
  // The other direction, and what makes the gate above a discrimination rather than a switch that
  // turns the compact form off whenever a row is cut. This row IS cut — the line runs far past the
  // ceiling — but the span opens two cells in, where the row draws it, so nothing about the opening
  // is out past the `…` and the `/` still says everything a corner row would.
  let text = format!("  {}\nclose\n", "x".repeat(10_000));
  let opens = Span::new(2, text.find("close").expect("closes") + "close".len());

  let rendered = render_spans(&text, &[opens]);
  let (source, markers) = opening_row(&rendered);
  assert!(
    source.contains('\u{2026}'),
    "the premise: the row was not cut"
  );
  assert!(
    source.starts_with("1 | / "),
    "the opening is drawn on this row, so it still needs no row of its own"
  );
  assert_eq!(
    markers,
    Vec::<usize>::new(),
    "a compact opening carries no marker, and the closing is on another line"
  );
}

#[test]
fn every_span_open_on_a_row_has_a_connector_column_to_itself() {
  // The other half of row assignment, and the half the cells above are blind to. Two spans open at
  // once in ONE column draw one bar where a reader has to see two, and every cell they mark is
  // still in the right place — so this is asserted on the MARGIN rather than on the markers.
  //
  // Counted against layer 2: a span contributes a bar to the source row of every line it covers
  // after the one it opens on. An opening contributes none, because what opens it is the corner
  // below it or the `/` that stands in for one, and neither is a bar.
  for case in bracketed() {
    let source = Source::new(case.text);
    let running: Vec<(u64, u64)> = case
      .spans
      .iter()
      .filter_map(|span| {
        let region = source.resolve(*span);
        region.is_multiline().then(|| {
          (
            region.start().line(),
            region.start().line() + region.line_count() - 1,
          )
        })
      })
      .collect();

    let rendered = render_spans(case.text, &case.spans);
    for row in rendered.lines() {
      let Some((number, bar, code)) = source_row(row, source) else {
        continue;
      };
      let characters: Vec<char> = row.chars().collect();
      let bars = characters[bar + 2..code]
        .iter()
        .filter(|c| **c == '|')
        .count();
      let open = running
        .iter()
        .filter(|(first, last)| *first < number && number <= *last)
        .count();
      assert_eq!(
        bars, open,
        "{:?} line {number}: {}\n{rendered}",
        case.text, case.why
      );
    }
  }
}

#[test]
fn the_lines_of_one_input_are_drawn_in_source_order() {
  // Not a taste, and not the order the caller gave: a connector runs down a contiguous run of rows,
  // so a row between a span's two ends has to BE a line that span covers. Caller order gives no
  // such guarantee — it would draw line 9 above line 2 and run a bracket upwards through it — so it
  // keeps the jobs it can still do: which input leads, what the `-->` points at, and the order of
  // the marker rows under one line.
  for case in bracketed() {
    let source = Source::new(case.text);
    for order in orderings(case.spans.len()) {
      let spans: Vec<Span> = order.iter().map(|index| case.spans[*index]).collect();
      let rendered = render_spans(case.text, &spans);
      let mut previous = 0;
      for row in rendered.lines() {
        if let Some((number, _, _)) = source_row(row, source) {
          assert!(
            number > previous,
            "{:?} in caller order {order:?}: line {number} came after {previous}\n{rendered}",
            case.text
          );
          previous = number;
        }
      }
    }
  }
}

#[test]
fn a_bracket_costs_the_same_rows_however_many_lines_it_covers() {
  // The product this feature introduces, and the answer to it.
  //
  // A row costs a bounded walk — `max_source_bytes` — so a renderer drawing every line a span
  // touches would be pricing a bounded per-row cost against an UNBOUNDED row count, and a budget on
  // one factor of a product bounds nothing. The rows are bounded instead: an opening, a closing, at
  // most three lines after the opening, and one `...` for the rest. Nothing here is a new budget,
  // because the count is now a function of the LABEL count, which the marker rows already were.
  //
  // Asserted as an equality across three orders of magnitude rather than as a ceiling, because a
  // ceiling generous enough to hold is a ceiling a linear growth fits under.
  let mut rows = None;
  let mut bytes = None;
  for lines in [8u64, 64, 512] {
    let text = format!("open\n{}close\n", "x\n".repeat(lines as usize));
    let span = Span::new(0, text.len());
    let rendered = render_spans(&text, &[span]);
    let counted = rendered.lines().count();
    assert_eq!(
      *rows.get_or_insert(counted),
      counted,
      "a span over {lines} lines drew a different number of rows\n{rendered}"
    );
    // And the same in bytes, up to the gutter widening — which is the only thing here that may
    // grow, and grows as the logarithm.
    let length = rendered.len();
    let previous = *bytes.get_or_insert(length);
    assert!(
      length < previous + 64,
      "a span over {lines} lines rendered {length} bytes where {previous} was enough\n{rendered}"
    );
  }
  assert!(
    rows.expect("three sizes were measured") < 16,
    "a bracketed span costs {rows:?} rows, which is not a bound anybody would call one"
  );
}

// ── The second style, written out ───────────────────────────────────────────────────────────────
//
// Goldens, and they live beside the invariants for one commit only: `render_boxed` is not a choice
// a caller can make yet, so there is no public entry an integration test could reach. They move to
// `tests/style_appearance.rs` with the seam, where the two styles render one diagnostic set and
// the gate "the same set, both styles" is structural rather than a claim.

fn boxed(diagnostic: &Diagnostic<'_>, text: &str, origin: Option<&str>) -> String {
  let mut input = Input::new(Source::new(text));
  if let Some(origin) = origin {
    input = input.with_origin(origin);
  }
  let mut out = String::new();
  Terminal::plain()
    .with_tab_width(4)
    .render_boxed(diagnostic, &[input], &mut out)
    .expect("a String never fails to be written to");
  out
}

#[test]
fn a_label_hangs_from_its_own_row_rather_than_sitting_on_the_marker() {
  // The whole header, because every part of it differs: the code is a line of its own, the
  // severity is a glyph and not a word, and the location line is the top of a box rather than an
  // arrow. What follows is two rows per label where the first style writes one.
  let text = "type Widget {\n  width: Int\n  width: Int\n}\n";
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
    boxed(&diagnostic, text, Some("widget.graphql")),
    "\
mylang::schema::duplicate-field

  × `width` is defined twice
   ╭─[widget.graphql:3:3]
 3 │   width: Int
   ·   ━━┳━━
   ·     ┗━━ redefined here
   ╰────
  help: rename one of the two definitions
"
  );
}

#[test]
fn the_primary_is_heavy_and_a_secondary_is_light() {
  // A box-drawing style has no second marker character to spare — `─` is the underline — so the
  // distinction `^` against `-` carries is carried by WEIGHT here. Asserted because losing it is
  // silent: both rows would still be underlines of the right width, under the right text, and a
  // reader would simply not be told which position the diagnostic is about.
  let text = "type Widget {\n  width: Int\n  width: Int\n}\n";
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

  let out = boxed(&diagnostic, text, None);
  assert!(
    out.contains('┳') && out.contains('┬'),
    "the two positions are drawn alike, so nothing says which one the diagnostic is about\n{out}"
  );
  assert_eq!(
    out,
    "\
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
"
  );
}

#[test]
fn two_labels_on_one_line_keep_their_own_pair_of_rows() {
  // The primary's span starts AFTER the secondary's and is still written first, which is the
  // caller's order surviving where it still says something.
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
    boxed(&diagnostic, text, Some("widget.graphql")),
    "\
mylang::schema::duplicate-field

  × `width` is defined twice
   ╭─[widget.graphql:1:27]
 1 │ type Widget { width: Int, width: Float }
   ·                           ━━┳━━
   ·                             ┗━━ redefined here
   ·               ──┬──
   ·                 ╰── first defined here
   ╰────
"
  );
}

#[test]
fn a_multi_line_span_is_bracketed_down_the_margin_and_marks_no_cell() {
  // The deepest divergence, and the design named it in advance: this style boxes where the other
  // indents. The bracket opens, runs and closes in the margin of the SOURCE rows themselves, and
  // the label turns the corner under it — so a multi-line span has no marker on any cell at all,
  // where the first style puts one on the cell its last character occupies.
  let text = "query Hero {\n  hero {\n    name\n    friends\n  }\n}\n";
  let message = "this selection set is nested too deeply";
  let diagnostic = Diagnostic::new(
    "mylang::query::too-deep",
    Severity::Error,
    &message,
    Location::new(0, Span::new(15, 44)),
  )
  .with_primary_label("this selection set");

  let out = boxed(&diagnostic, text, Some("hero.graphql"));
  assert!(
    !out.contains('┳'),
    "a bracketed span marked a cell, which is the other style's form\n{out}"
  );
  assert_eq!(
    out,
    "\
mylang::query::too-deep

  × this selection set is nested too deeply
   ╭─[hero.graphql:2:3]
 2 │ ┏   hero {
 3 │ ┃     name
 4 │ ┃     friends
 5 │ ┣   }
   · ┗━━━━ this selection set
   ╰────
"
  );
}

#[test]
fn an_opening_with_text_before_it_costs_this_style_no_row() {
  // The other style needs a row here — `let x = ` precedes the span, so a glyph in the margin
  // would not say which column it begins at, and an underscore run says it exactly. This style
  // cannot say it at all: the bracket is in the margin whatever precedes the span, so the opening
  // COLUMN is what it gives up, and it gives it up on every multi-line span rather than on some.
  let text = "let x = if a {\n  1\n} else {\n  2\n};\n";
  let message = "the branches disagree";
  let diagnostic = Diagnostic::new(
    "mylang::type::branches",
    Severity::Error,
    &message,
    Location::new(0, Span::new(8, 33)),
  )
  .with_primary_label("this expression");

  assert_eq!(
    boxed(&diagnostic, text, None),
    "\
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
"
  );
}

#[test]
fn what_a_long_span_left_out_is_said_in_the_wall() {
  // `⋮` in the wall's column rather than `...` in the number's. Both are true and they say
  // different things; a boxed style has a wall to say it in.
  let text = "fragment F on Query {\n  a\n  b\n  c\n  d\n  e\n  f\n  g\n}\n";
  let message = "this fragment is never used";
  let diagnostic = Diagnostic::new(
    "mylang::query::unused-fragment",
    Severity::Warning,
    &message,
    Location::new(0, Span::new(0, text.len() - 1)),
  )
  .with_primary_label("declared here");

  assert_eq!(
    boxed(&diagnostic, text, None),
    "\
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
"
  );
}

#[test]
fn two_overlapping_brackets_run_in_columns_of_their_own() {
  // One column would leave a reader unable to tell which opening a closing belongs to. The inner
  // span opens later so it runs to the RIGHT, and the outer's closing run crosses it: what the
  // crossing says is that this bracket closes over that one, exactly as the underscore run does
  // in the other style.
  let text = "outer (\n  inner (\n    x\n  )\n)\n";
  let message = "two of these disagree";
  let labels = [Label::new(
    Location::new(0, Span::new(10, 26)),
    "and this one",
  )];
  let diagnostic = Diagnostic::new(
    "mylang::type::nested",
    Severity::Error,
    &message,
    Location::new(0, Span::new(6, 29)),
  )
  .with_primary_label("this one")
  .with_labels(&labels);

  assert_eq!(
    boxed(&diagnostic, text, None),
    "\
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
"
  );
}

#[test]
fn an_underline_is_as_wide_as_the_cells_it_covers_and_not_the_characters() {
  // The cell arithmetic is the renderer's and not the style's, and this is what says so: a tab
  // before the span and two-cell ideographs inside it, placed by the same walk that places the
  // other style's carets.
  let text = "\tlet x = \u{65e5}\u{672c};\n";
  let message = "a wide name behind a tab";
  let diagnostic = Diagnostic::new(
    "mylang::test::wide",
    Severity::Advice,
    &message,
    Location::new(0, Span::new(9, 15)),
  )
  .with_primary_label("two cells each");

  let out = boxed(&diagnostic, text, None);
  // Four cells, which is what two ideographs draw as. A style counting characters would write two.
  assert_eq!(
    out
      .lines()
      .find(|row| row.contains('┳'))
      .map(|row| row.chars().filter(|c| *c == '━' || *c == '┳').count()),
    Some(4),
    "the underline is not the width of what it covers\n{out}"
  );
  assert_eq!(
    out,
    "\
mylang::test::wide

  ☞ a wide name behind a tab
   ╭─[1:10]
 1 │     let x = 日本;
   ·             ━━┳━
   ·               ┗━━ two cells each
   ╰────
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
    boxed(&diagnostic, "type Widget {}\n", None),
    "\
mylang::input::synthesized

  ⚠ this document was generated, so it has no positions
"
  );
}
