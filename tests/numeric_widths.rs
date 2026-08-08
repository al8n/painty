//! Every public numeric is the width the rule gives it, and no position is ever clamped.
//!
//! # Why these are assertions and not scenarios
//!
//! The behaviour worth guaranteeing is that painty never reports a number that has been quietly
//! narrowed. The scenario that would exercise one needs a source with more than `u32::MAX` line
//! breaks — over four gigabytes of text — so no test is going to construct it, and a guarantee
//! with no test behind it decays into a comment. So the *property* is asserted instead, in the
//! three forms it can take.
//!
//! **Every width, by ascription.** No integer type coerces to another, so binding each accessor's
//! result to its declared type is a compile-time assertion about that member. The list below is
//! the whole public numeric surface, not a sample — which matters, because the review that
//! prompted this file found a member nobody had classified, and a test that pins only the members
//! somebody thought of cannot notice that.
//!
//! **`u32`, by census.** Rule 3 of the numeric widths in `README.md` is the only
//! one that yields a narrow type, so a `u32` anywhere else is a ceiling nobody justified. Checked
//! against the source, because it has to catch a member that does not exist yet — which no
//! ascription can.
//!
//! **The arithmetic, by census.** No saturating operation and no `as` cast on the path a position
//! is built by. A claim about source, checked against source, for the reason this program keeps
//! rediscovering: prose about what code does not do is uncompiled.
//!
//! # What the last two do not cover
//!
//! A census over text is not a census over the API. A new member spelled `u64` or `usize` that
//! should have been something else passes all three of these; only a `syn`-based walk of the
//! public surface would catch it, and that is a dependency and a machine for a crate with roughly
//! thirty numeric members. The gap is recorded rather than papered over: the ordered rule in the
//! README is what places a new member, and this file is what keeps the placed ones honest.
//!
//! # What "exact" rests on
//!
//! A line ordinal is at most one more than the text's length in bytes; Rust caps a single object
//! at `isize::MAX` bytes; so on the widest target this crate builds for, an ordinal cannot exceed
//! 2^63 and a `u64` cannot overflow. The increments are therefore plain `+`, and a debug build
//! panics if that reasoning is ever wrong — which is the direction to fail in. A saturating
//! increment would instead hand back a number indistinguishable from a real one.

use painty::{Location, PathSegment, Source, Span};

/// The files a position is built by.
const RESOLUTION: [(&str, &str); 3] = [
  ("src/source/mod.rs", include_str!("../src/source/mod.rs")),
  ("src/source/line.rs", include_str!("../src/source/line.rs")),
  (
    "src/source/region.rs",
    include_str!("../src/source/region.rs"),
  ),
];

/// Every file the crate is made of.
///
/// A literal list rather than a directory walk: a walk that stopped finding files would report
/// nothing and pass, and this is the check that is supposed to notice something new.
const CRATE: [(&str, &str); 9] = [
  ("src/lib.rs", include_str!("../src/lib.rs")),
  (
    "src/diagnostic/mod.rs",
    include_str!("../src/diagnostic/mod.rs"),
  ),
  (
    "src/diagnostic/location.rs",
    include_str!("../src/diagnostic/location.rs"),
  ),
  (
    "src/diagnostic/path.rs",
    include_str!("../src/diagnostic/path.rs"),
  ),
  (
    "src/diagnostic/severity.rs",
    include_str!("../src/diagnostic/severity.rs"),
  ),
  (
    "src/diagnostic/span.rs",
    include_str!("../src/diagnostic/span.rs"),
  ),
  ("src/source/mod.rs", include_str!("../src/source/mod.rs")),
  ("src/source/line.rs", include_str!("../src/source/line.rs")),
  (
    "src/source/region.rs",
    include_str!("../src/source/region.rs"),
  ),
];

/// The one file rule 3 applies in.
///
/// `Location::source` is a key into the caller's own list of inputs. painty never computes it, so
/// it has no site at which it could be narrowed, and it has to round-trip through a contract that
/// already spells it `u32`. `src/tokora.rs` is absent from [`CRATE`] for the same reason turned
/// around: it is nothing but conversions from a foreign contract's widths, so every integer type
/// in it is that contract's choice rather than painty's.
const FOREIGN_KEY: &str = "src/diagnostic/location.rs";

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

/// Rule 3 is the only rule that yields a narrow type, so `u32` outside its one site is a ceiling
/// nobody argued for.
///
/// This is the only one of the three censuses that can catch a member which does not exist yet.
/// The ascriptions below pin the widths of members somebody has already thought about; the review
/// that prompted this file found one nobody had, and the shape of that miss was a `u32` sitting
/// quietly in a public enum.
#[test]
fn u32_appears_only_where_a_foreign_key_round_trips() {
  let mut found = Vec::new();
  for (path, source) in CRATE {
    if path == FOREIGN_KEY {
      continue;
    }
    for (number, line) in code_lines(source) {
      if line.contains("u32") {
        found.push(format!("{path}:{number}: {}", line.trim()));
      }
    }
  }
  assert!(
    found.is_empty(),
    "a `u32` outside {FOREIGN_KEY} is a ceiling on something painty computes. Place the member \
     with the ordered rule in README.md#numeric-widths: only rule 3 — a key painty never computes, \
     round-tripping through a contract that already spells it `u32` — gives a narrow type.\n{}",
    found.join("\n")
  );
}

#[test]
fn the_censuses_can_fail() {
  // Each gate above is a substring search, and a substring search looking for the wrong substring
  // passes forever. Every pattern is exercised against text that does contain it, so a change that
  // breaks the matching itself reddens here rather than going quiet.
  let planted = "  let n = count.saturating_add(1) as u32;";
  assert!(planted.contains("saturating_"));
  assert!(planted.contains("as u32"));
  assert!(planted.contains("u32"));

  // ...and the comment stripper really does strip, so no gate can be defeated by prose, nor
  // tripped by it.
  let commented = "  // never write `count.saturating_add(1) as u32` here";
  assert_eq!(code_lines(commented).count(), 0);
  assert_eq!(code_lines(planted).count(), 1);
}

#[test]
fn every_public_numeric_is_the_width_its_rule_gives_it() {
  let text = "one\ntwo\nthree\n";
  let source = Source::new(text);
  let region = source.resolve(Span::new(2, 9));
  let line = source.line(1).expect("a source has a first line");
  let drawn = region.lines().next().expect("a region is drawn somewhere");
  let span = Span::new(2, 9);
  let location = Location::new(0, span);

  // Ascribed, not compared. No integer type coerces to another, so each binding is a compile-time
  // assertion about one member. This is the WHOLE public numeric surface: adding a member here is
  // part of adding one to the crate, and a change that widens some and not others cannot pass.

  // Rule 1 — a line or column in the resolved model.
  let position_line: u64 = region.start().line();
  let position_column: u64 = region.start().column();
  let line_number: u64 = line.number();
  let line_width: u64 = line.char_count();
  let column_of: u64 = line.column_at(1);
  let lines_in_source: u64 = source.line_count();
  let lines_in_region: u64 = region.line_count();
  let columns: core::ops::Range<u64> = drawn.columns();

  // Rule 2 — an ordinal in data the producer built.
  let path_index: PathSegment<'_> = PathSegment::Index(2u64);

  // Rule 3 — a key into a structure painty does not own.
  let input: u32 = location.source();

  // Rule 4 — everything else: an index or a count of things in memory.
  let position_offset: usize = region.start().offset();
  let span_start: usize = span.start();
  let span_end: usize = span.end();
  let span_len: usize = span.len();
  let break_len: usize = line
    .line_break()
    .expect("the first line of the fixture ends in a break")
    .byte_len();
  let source_len: usize = source.len();

  // And the values are the right ones, so the ascriptions are over live code rather than over
  // accessors nobody calls.
  assert_eq!((position_line, position_column, position_offset), (1, 3, 2));
  assert_eq!((line_number, line_width, column_of), (1, 3, 2));
  // Four lines in the source, counting the empty one the trailing break leaves; the region runs
  // from line 1 to the `h` on line 3.
  assert_eq!((lines_in_source, lines_in_region), (4, 3));
  assert_eq!(columns, 3..4);
  assert_eq!(path_index.to_string(), "2");
  assert_eq!(input, 0);
  assert_eq!((span_start, span_end, span_len), (2, 9, 7));
  assert_eq!((break_len, source_len), (1, text.len()));
}

/// The adapter's two counts, which only exist when its feature is on.
#[cfg(feature = "tokora")]
#[test]
fn the_adapters_overflow_counts_are_rule_four() {
  use painty::tokora::adapt;

  // A `Diagnose` with nothing to place, so the adapter is exercised without a fixture type: the
  // widths are what this test is about, not the walk, which `tokora_adapter.rs` covers.
  struct Empty;
  impl core::fmt::Display for Empty {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
      f.write_str("nothing to say")
    }
  }
  impl tokora::diagnostic::Diagnose for Empty {
    fn code(&self) -> tokora::diagnostic::Code {
      tokora::diagnostic::Code::new("mylang::test::empty")
    }
    fn severity(&self) -> tokora::diagnostic::Severity {
      tokora::diagnostic::Severity::Advice
    }
    fn primary(&self) -> tokora::diagnostic::Location {
      tokora::diagnostic::Location::entire(0)
    }
    fn primary_label(&self) -> Option<&'static str> {
      None
    }
    fn labels(&self) -> usize {
      0
    }
    fn label(&self, _: usize) -> Option<tokora::diagnostic::Label> {
      None
    }
    fn path_segments(&self) -> usize {
      0
    }
    fn path_segment(&self, _: usize) -> Option<tokora::diagnostic::PathSegment<'_>> {
      None
    }
    fn help(&self) -> Option<&'static str> {
      None
    }
  }

  let adapted = adapt(&Empty, &mut [], &mut []);
  let dropped_labels: usize = adapted.dropped_labels();
  let dropped_path: usize = adapted.dropped_path_segments();
  assert_eq!((dropped_labels, dropped_path), (0, 0));
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
