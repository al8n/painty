use super::LineCells;
use crate::Source;

/// The corpus, and what each member is here to break.
///
/// A corpus whose members are not individually justified is a corpus the next person trims. Each
/// of these defeats a *different* wrong definition of "column", and the first entry is the one
/// that makes all the wrong definitions look right.
const CORPUS: [(&str, &str); 12] = [
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

  for (text, why) in CORPUS {
    if text.is_ascii() && !text.contains('\t') {
      continue;
    }
    let cells = measured(text, 4);
    let differs = (0..=text.len()).any(|offset| {
      cells.line().text().is_char_boundary(offset)
        && cells.column_at(offset) != cells.line().column_at(offset)
    });
    assert!(differs, "{text:?} measures the same as characters — {why}");
  }
}

#[test]
fn a_combining_mark_occupies_no_cells() {
  // `e` then U+0301. The mark is a character and two bytes, and zero cells.
  let cells = measured("e\u{301}f", 4);
  assert_eq!(cells.column_at(0), 1);
  assert_eq!(cells.column_at(1), 2, "after `e`");
  assert_eq!(cells.column_at(3), 2, "after the mark, which draws nothing");
  assert_eq!(cells.width(), 2);
  assert_eq!(cells.line().char_count(), 3, "three characters, two cells");
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
