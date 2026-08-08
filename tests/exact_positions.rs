//! Positions are exact, and this is what says so.
//!
//! # Why the assertion is here and not in a scenario
//!
//! The behaviour worth guaranteeing is that painty never reports a line or column that has been
//! quietly clamped. The scenario that would exercise a clamp needs a source with more than
//! `u32::MAX` line breaks — over four gigabytes of text — so no test is ever going to construct
//! one, and a guarantee with no test behind it decays into a comment.
//!
//! So the property is asserted instead of the scenario, in the two forms it can take.
//!
//! **The widths, by type.** Every ordinal in the resolved model is a `u64` and every byte offset
//! is a `usize`, checked by ascription below. A change that widens some of them and not others
//! fails here, which is the failure the mixed model deserves: two units that disagree are worse
//! than either one applied throughout.
//!
//! **The arithmetic, by census.** No saturating operation and no `as` cast appears anywhere on the
//! path a position is built by. That is a claim about *source*, so it is checked against the
//! source — the same shape of gate as pinning a public API, and for the same reason: prose about
//! what code does not do is uncompiled, and this program keeps finding prose that stopped being
//! true.
//!
//! # What "exact" rests on
//!
//! A line ordinal is at most one more than the text's length in bytes; Rust caps a single object
//! at `isize::MAX` bytes; so on the widest target this crate builds for, an ordinal cannot exceed
//! 2^63 and a `u64` cannot overflow. The increments are therefore plain `+`, and a debug build
//! panics if that reasoning is ever wrong — which is the direction to fail in. A saturating
//! increment would instead hand back a number indistinguishable from a real one.

use painty::{Source, Span};

/// The files a position is built by.
const RESOLUTION: [(&str, &str); 3] = [
  ("src/source/mod.rs", include_str!("../src/source/mod.rs")),
  ("src/source/line.rs", include_str!("../src/source/line.rs")),
  (
    "src/source/region.rs",
    include_str!("../src/source/region.rs"),
  ),
];

/// The integer types a cast could narrow to.
const INTEGERS: [&str; 12] = [
  "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
];

/// Strips the line comments, so prose about a cast is not mistaken for one.
///
/// Line comments only: this crate writes no block comments, and a stripper that handled them would
/// be more machinery than the thing it is guarding.
fn code_lines(source: &str) -> impl Iterator<Item = (usize, &str)> {
  source
    .lines()
    .enumerate()
    .map(|(index, line)| (index + 1, line))
    .filter(|(_, line)| !line.trim_start().starts_with("//"))
}

#[test]
fn no_position_is_built_by_a_saturating_operation() {
  let mut found = Vec::new();
  for (path, source) in RESOLUTION {
    for (number, line) in code_lines(source) {
      if line.contains("saturating_") {
        found.push(format!("{path}:{number}: {}", line.trim()));
      }
    }
  }
  assert!(
    found.is_empty(),
    "a saturating operation on the resolution path clamps a position without saying so; use \
     checked arithmetic, or `try_from` where two units genuinely meet:\n{}",
    found.join("\n")
  );
}

#[test]
fn no_position_is_built_by_a_narrowing_cast() {
  let mut found = Vec::new();
  for (path, source) in RESOLUTION {
    for (number, line) in code_lines(source) {
      for integer in INTEGERS {
        if line.contains(&format!("as {integer}")) {
          found.push(format!("{path}:{number}: {}", line.trim()));
        }
      }
    }
  }
  assert!(
    found.is_empty(),
    "an `as` cast on the resolution path can narrow silently; use `try_from` and decide what a \
     value that does not fit should do:\n{}",
    found.join("\n")
  );
}

#[test]
fn the_census_can_fail() {
  // The gate above is a substring search, and a substring search that is looking for the wrong
  // substring passes forever. Both patterns are exercised against text that does contain them, so
  // a change that breaks the matching itself reddens here rather than going quiet.
  let planted = "  let n = count.saturating_add(1) as u32;";
  assert!(planted.contains("saturating_"));
  assert!(planted.contains("as u32"));

  // ...and the comment stripper really does strip, so the two gates cannot be defeated by prose
  // and cannot be tripped by it either.
  let commented = "  // never write `count.saturating_add(1) as u32` here";
  assert_eq!(code_lines(commented).count(), 0);
  assert_eq!(code_lines(planted).count(), 1);
}

#[test]
fn every_ordinal_is_a_u64_and_every_byte_offset_is_a_usize() {
  let source = Source::new("one\ntwo\nthree\n");
  let region = source.resolve(Span::new(2, 9));
  let line = source.line(1).expect("a source has a first line");
  let drawn = region.lines().next().expect("a region is drawn somewhere");

  // Ascribed, not compared. No integer type coerces to another, so each of these is a
  // compile-time assertion about one member of the model, and a change that widens some of them
  // and not the others cannot get past the set.
  let position_line: u64 = region.start().line();
  let position_column: u64 = region.start().column();
  let position_offset: usize = region.start().offset();
  let line_number: u64 = line.number();
  let line_width: u64 = line.char_count();
  let lines_in_source: u64 = source.line_count();
  let lines_in_region: u64 = region.line_count();
  let columns: core::ops::Range<u64> = drawn.columns();

  // And the values are the right ones, so the ascriptions above are over live code rather than
  // over accessors nobody calls.
  assert_eq!((position_line, position_column, position_offset), (1, 3, 2));
  assert_eq!((line_number, line_width), (1, 3));
  // Four lines in the source, counting the empty one the trailing break leaves; the region runs
  // from line 1 to the `h` on line 3.
  assert_eq!((lines_in_source, lines_in_region), (4, 3));
  assert_eq!(columns, 3..4);
}

#[test]
fn the_last_line_number_and_the_line_count_are_the_same_number() {
  // `Source::line_count` reads the last line's ordinal rather than counting into a `usize` and
  // converting. That is only correct while line numbers start at 1 and never skip, so the identity
  // is pinned rather than assumed.
  for text in [
    "",
    "a",
    "a\n",
    "a\nb",
    "a\r\nb\rc\n",
    "\n\n\n",
    "\r\n\r\n",
    "日\n本\n",
  ] {
    let source = Source::new(text);
    let walked = source.lines().count() as u64;
    assert_eq!(source.line_count(), walked, "{text:?}");
    assert_eq!(
      source.lines().last().expect("at least one line").number(),
      walked,
      "{text:?}"
    );
  }
}
