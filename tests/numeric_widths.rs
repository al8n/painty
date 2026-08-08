//! Every public numeric is the width its rule gives it, and no position is ever clamped.
//!
//! # Why these are assertions and not scenarios
//!
//! The behaviour worth guaranteeing is that painty never reports a number that has been quietly
//! narrowed, and never publishes a width it cannot change later. The scenario that would exercise
//! a clamp needs a source with more than `u32::MAX` line breaks — over four gigabytes of text — so
//! no test is going to construct it, and a guarantee with no test behind it decays into a comment.
//! So the *property* is asserted instead, in five layers, each of which closes a way the one
//! above it could be true and useless.
//!
//! **Every signature, by function pointer.** [`pin`] ascribes the whole signature of every public
//! member that mentions a primitive integer. A parameter's width is as frozen as a return's —
//! `Span::new` taking `usize` is a promise to every caller who has already written one — so
//! pinning only returns would leave half the surface free to drift.
//!
//! **That the list is complete, by census.** [`pin`] is a list somebody maintains, and a list
//! nobody is forced to extend falls behind. So the public members are read out of `src/` in the
//! three shapes a numeric can be published in — a function signature, a public enum's variant
//! payload, a public field or associated constant — and each must appear in [`pin`]'s own source.
//! Adding one reddens until it is pinned; removing one reddens the ascription left behind.
//!
//! **That the census reads the whole crate, by walking it.** A file missing from the literal list
//! is a file no check here covers, and nothing about a green run would say so.
//!
//! **`u32`, by census.** Rule 3 in `README.md` is the only rule yielding a narrow type, and it
//! justifies exactly one member. So the exemption names that member's four lines rather than its
//! file: a file-level licence would extend to every future member of the file, which is the
//! open-endedness this sequence of reviews has been about. `src/tokora.rs` is *in* the census for
//! the mirror-image reason — the file where foreign widths legitimately appear is the last one to
//! leave unchecked.
//!
//! **The arithmetic, by census.** No saturating operation and no `as` cast on the path a position
//! is built by. A claim about source, checked against source, because prose about what code does
//! not do is uncompiled.
//!
//! # What still gets through
//!
//! Re-derived from what is built above rather than carried forward from an earlier draft.
//!
//! 1. **A member placed under the wrong rule.** A new `usize` that should have been a rule-2
//!    ordinal is pinned, complete, `u32`-free — and wrong. No text gate reads intent. The ordered
//!    rule in `README.md` is what places a member; review is what checks the placement. This one
//!    is not closable by any mechanism short of understanding the domain, so it is the residual
//!    that matters.
//! 2. **A declaration whose integer is not on the line the census keys on.** A signature wrapped
//!    across lines by a future edit, or a variant written multi-line. None exist: rustfmt holds
//!    every one of them inside a hundred columns, so this is a shape the crate does not currently
//!    produce rather than one it tolerates.
//! 3. **A numeric reached through a re-exported foreign type.** `src/lib.rs` re-exports only
//!    painty's own types today, so there is nothing to reach; a future `pub use` of somebody
//!    else's type would carry that type's widths past every check here.
//!
//! Only a walk of the public API — `syn`, a dependency and a machine — would close 2 and 3, and it
//! would not touch 1. Neither has occurred; 1 has, twice, and it was caught by review both times.
//!
//! # What "exact" rests on
//!
//! A line ordinal is at most one more than the text's length in bytes; Rust caps a single object
//! at `isize::MAX` bytes; so on the widest target this crate builds for, an ordinal cannot exceed
//! 2^63 and a `u64` cannot overflow. The increments are therefore plain `+`, and a debug build
//! panics if that reasoning is ever wrong — which is the direction to fail in. A saturating
//! increment would instead hand back a number indistinguishable from a real one.

use painty::{Line, LineBreak, Location, PathSegment, Position, Region, RegionLine, Source, Span};

/// Every file that declares part of the public surface.
///
/// A literal list, because `include_str!` needs one — and a literal list goes stale in silence, so
/// [`the_file_list_is_the_whole_crate`] walks `src/` and checks it. `src/tokora.rs` is here whether
/// or not its feature is on: `include_str!` reads the disk, not the build, which is what keeps the
/// adapter under the same censuses as everything else.
const CRATE: [(&str, &str); 10] = [
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
  ("src/tokora.rs", include_str!("../src/tokora.rs")),
];

/// The files a position is built by.
const RESOLUTION: [&str; 3] = [
  "src/source/mod.rs",
  "src/source/line.rs",
  "src/source/region.rs",
];

/// This file, so the completeness census can read the ascriptions out of [`pin`].
const SELF: &str = include_str!("numeric_widths.rs");

/// The primitive integer types.
const INTEGERS: [&str; 12] = [
  "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
];

/// Every line in the crate that rule 3 permits to say `u32`, named exactly.
///
/// `Location::source` is a key into the caller's own list of inputs: painty never computes it, so
/// it has no site at which it could be narrowed, and it round-trips through a contract that
/// already spells it `u32`. That licence belongs to those four lines and to nothing else — not to
/// the rest of their file, and not to a member added to it next year.
const RULE_THREE: [(&str, &str); 4] = [
  ("src/diagnostic/location.rs", "source: u32,"),
  (
    "src/diagnostic/location.rs",
    "pub const fn new(source: u32, span: Span) -> Self {",
  ),
  (
    "src/diagnostic/location.rs",
    "pub const fn entire(source: u32) -> Self {",
  ),
  (
    "src/diagnostic/location.rs",
    "pub const fn source(&self) -> u32 {",
  ),
];

/// Strips the line comments, so prose about a cast is not mistaken for one.
///
/// Line comments only: this crate writes no block comments, and a stripper that handled them would
/// be more machinery than the thing it guards.
fn code_lines(source: &str) -> impl Iterator<Item = (usize, &str)> {
  source
    .lines()
    .enumerate()
    .map(|(index, line)| (index + 1, line.trim()))
    .filter(|(_, line)| !line.starts_with("//"))
}

/// Whether `line` mentions `integer` as a token rather than as part of a longer word.
fn mentions(line: &str, integer: &str) -> bool {
  let boundary = |byte: Option<u8>| !byte.is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_');
  let bytes = line.as_bytes();
  line.match_indices(integer).any(|(at, _)| {
    boundary(at.checked_sub(1).map(|before| bytes[before]))
      && boundary(bytes.get(at + integer.len()).copied())
  })
}

/// The type an `impl` line opens, or `None` if the line is not one.
///
/// `impl<'a> Source<'a> {` is `Source`; `impl<'a> Iterator for RegionLines<'a> {` is `RegionLines`.
fn impl_type(line: &str) -> Option<&str> {
  let rest = line.strip_prefix("impl")?;
  // Skip the impl's own generics, which may nest: `impl<'a, D: Trait<'a>>`.
  let rest = if rest.starts_with('<') {
    let mut depth = 0usize;
    let end = rest
      .char_indices()
      .find_map(|(at, character)| match character {
        '<' => {
          depth += 1;
          None
        }
        '>' => {
          depth -= 1;
          (depth == 0).then_some(at + 1)
        }
        _ => None,
      })?;
    &rest[end..]
  } else {
    rest
  };

  // A trait impl names the trait first, so the implementing type is what follows the last `for`.
  let subject = match rest.rfind(" for ") {
    Some(at) => &rest[at + " for ".len()..],
    None => rest,
  };
  let subject = subject.trim().trim_end_matches('{').trim();
  let subject = subject.split(['<', ' ']).next()?;
  subject.rsplit("::").next().filter(|name| !name.is_empty())
}

/// Every public member of the crate that mentions a primitive integer, as `Type::name` paired
/// with the site it was read from.
///
/// Three declaration shapes, because a numeric can be published in three ways and pinning only
/// the first would leave the same kind of hole this file exists to close: a function's signature,
/// a public enum's variant payload, and a public field or associated constant.
fn public_numeric_members() -> Vec<(String, String)> {
  let mut found = Vec::new();
  for (path, source) in CRATE {
    let mut owner = "<none>";
    let mut in_public_enum = None;

    for (number, line) in code_lines(source) {
      if let Some(name) = impl_type(line) {
        owner = name;
      }
      if let Some(rest) = line.strip_prefix("pub enum ") {
        let name = rest.split(['<', ' ', '{']).next().unwrap_or_default();
        owner = name;
        in_public_enum = Some(name);
      }
      if line == "}" {
        in_public_enum = None;
      }
      if let Some(rest) = line.strip_prefix("pub struct ") {
        owner = rest.split(['<', ' ', '{']).next().unwrap_or_default();
      }

      let mentions_integer = INTEGERS.iter().any(|integer| mentions(line, integer));
      if !mentions_integer {
        continue;
      }

      // A function, whose whole signature is pinned.
      if let Some(rest) = line
        .strip_prefix("pub const fn ")
        .or_else(|| line.strip_prefix("pub fn "))
      {
        let name = rest.split(['(', '<']).next().unwrap_or_default();
        found.push((format!("{owner}::{name}"), format!("{path}:{number}")));
        continue;
      }

      // A variant payload of a public enum — `Index(u64),`.
      if let Some(name) = in_public_enum
        && line.starts_with(|c: char| c.is_ascii_uppercase())
        && line.contains('(')
      {
        let variant = line.split('(').next().unwrap_or_default();
        found.push((format!("{name}::{variant}"), format!("{path}:{number}")));
        continue;
      }

      // A public field or associated constant — `pub offset: usize,`.
      if let Some(rest) = line.strip_prefix("pub ")
        && let Some(name) = rest.split(':').next()
        && !rest.starts_with("fn ")
      {
        let name = name.trim_start_matches("const ").trim();
        found.push((format!("{owner}::{name}"), format!("{path}:{number}")));
      }
    }
  }
  found
}

/// The `let _: … = …;` lines of [`pin`], which are what the completeness census reads.
fn ascriptions() -> Vec<&'static str> {
  code_lines(SELF)
    .map(|(_, line)| line)
    .filter(|line| line.starts_with("let _:"))
    .collect()
}

/// The whole public numeric surface, one ascription per signature.
///
/// A function pointer rather than a typed variable per parameter: it pins the parameters and the
/// return in one line, and it cannot be written while forgetting an argument. The lifetimes are
/// concrete rather than higher-ranked because these methods borrow a `Self` that already carries
/// one — a `for<'a>` annotation is *more* general than the method and does not compile.
///
/// The witness gives `'a` somewhere to come from. It has no other purpose, and a lifetime used
/// only inside the body would read to clippy as an unused one.
fn pin<'a>(_witness: &'a ()) {
  // Rule 1 — a line or column in the resolved model.
  let _: fn(&Position) -> u64 = Position::line;
  let _: fn(&Position) -> u64 = Position::column;
  let _: fn(&'a Line<'a>) -> u64 = Line::number;
  let _: fn(&'a Line<'a>) -> u64 = Line::char_count;
  let _: fn(&'a Line<'a>, usize) -> u64 = Line::column_at;
  let _: fn(&'a Source<'a>) -> u64 = Source::line_count;
  let _: fn(&'a Source<'a>, u64) -> Option<Line<'a>> = Source::line;
  let _: fn(&'a Region<'a>) -> u64 = Region::line_count;
  let _: fn(&'a RegionLine<'a>) -> core::ops::Range<u64> = RegionLine::columns;

  // Rule 2 — an ordinal in data the producer built.
  let _: fn(u64) -> PathSegment<'a> = PathSegment::Index;

  // Rule 3 — a key into a structure painty does not own.
  let _: fn(u32, Span) -> Location = Location::new;
  let _: fn(u32) -> Location = Location::entire;
  let _: fn(&Location) -> u32 = Location::source;

  // Rule 4 — everything else: an index or a count of things in memory.
  let _: fn(usize, usize) -> Span = Span::new;
  let _: fn(usize) -> Span = Span::empty;
  let _: fn(&Span) -> usize = Span::start;
  let _: fn(&Span) -> usize = Span::end;
  let _: fn(&Span) -> usize = Span::len;
  let _: fn(&Span, usize) -> bool = Span::contains;
  let _: fn(&Position) -> usize = Position::offset;
  let _: fn(&LineBreak) -> usize = LineBreak::byte_len;
  let _: fn(&'a Source<'a>) -> usize = Source::len;
  let _: fn(&'a Source<'a>, usize) -> Line<'a> = Source::line_at;
  let _: fn(&'a Source<'a>, usize) -> Position = Source::position;

  #[cfg(feature = "tokora")]
  {
    use painty::tokora::Adapted;
    let _: fn(&Adapted<'a>) -> usize = Adapted::dropped_labels;
    let _: fn(&Adapted<'a>) -> usize = Adapted::dropped_path_segments;
  }
}

#[test]
fn the_file_list_is_the_whole_crate() {
  // Every census here reads `CRATE`, so a source file missing from it is a file no census covers —
  // and nothing about a passing run would say so. Walked at run time and compared, rather than
  // trusted: the literal is what `include_str!` requires, and this is what keeps the literal true.
  fn walk(directory: &std::path::Path, found: &mut Vec<String>, root: &std::path::Path) {
    for entry in std::fs::read_dir(directory).expect("src/ is readable") {
      let path = entry.expect("a readable directory entry").path();
      if path.is_dir() {
        walk(&path, found, root);
      } else if path.extension().is_some_and(|extension| extension == "rs") {
        let relative = path
          .strip_prefix(root)
          .expect("a path under the crate root");
        found.push(relative.to_string_lossy().replace('\\', "/"));
      }
    }
  }

  let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
  let mut on_disk = Vec::new();
  walk(&root.join("src"), &mut on_disk, root);
  on_disk.sort();

  // `tests.rs` files are unit tests rather than declarations, and are the one thing the list omits.
  let mut expected: Vec<String> = CRATE.iter().map(|(path, _)| (*path).to_owned()).collect();
  expected.extend(
    on_disk
      .iter()
      .filter(|path| path.ends_with("/tests.rs"))
      .cloned(),
  );
  expected.sort();

  assert_eq!(
    on_disk, expected,
    "`src/` and the census file list disagree. A new source file is covered by no check here until \
     it is added to CRATE; a deleted one leaves an entry that reads an empty file."
  );
}

#[test]
fn every_public_numeric_member_is_pinned() {
  pin(&());

  let ascribed = ascriptions();
  let mut missing = Vec::new();
  for (symbol, site) in public_numeric_members() {
    if !ascribed.iter().any(|line| line.contains(&symbol)) {
      missing.push(format!("{site}: {symbol}"));
    }
  }
  assert!(
    missing.is_empty(),
    "a public member declares a primitive integer and nothing pins it. Place it with the ordered \
     rule in README.md#numeric-widths, then add a `let _: fn(..) -> ..` line for it to `pin`:\n{}",
    missing.join("\n")
  );
}

#[test]
fn no_ascription_outlives_the_function_it_pins() {
  // The other direction, so the list can only be as long as the surface. Without it a removed or
  // renamed member leaves an ascription that still compiles against nothing anybody calls, and the
  // census above keeps passing over a list that has quietly stopped describing the crate.
  let signatures = public_numeric_members();
  let mut stale = Vec::new();
  for line in ascriptions() {
    let symbol = line
      .rsplit(" = ")
      .next()
      .unwrap_or_default()
      .trim_end_matches(';');
    if !signatures.iter().any(|(found, _)| found == symbol) {
      stale.push(symbol.to_string());
    }
  }
  assert!(
    stale.is_empty(),
    "`pin` ascribes a signature the crate no longer has:\n{}",
    stale.join("\n")
  );
}

#[test]
fn u32_appears_only_where_a_foreign_key_round_trips() {
  let mut unlicensed = Vec::new();
  let mut used = [false; RULE_THREE.len()];

  for (path, source) in CRATE {
    for (number, line) in code_lines(source) {
      if !mentions(line, "u32") {
        continue;
      }
      match RULE_THREE
        .iter()
        .position(|&(file, text)| file == path && text == line)
      {
        Some(index) => used[index] = true,
        None => unlicensed.push(format!("{path}:{number}: {line}")),
      }
    }
  }

  assert!(
    unlicensed.is_empty(),
    "a `u32` outside the four lines rule 3 licenses is a ceiling on something painty computes. \
     Place the member with the ordered rule in README.md#numeric-widths: only rule 3 — a key \
     painty never computes, round-tripping through a contract that already spells it `u32` — \
     gives a narrow type.\n{}",
    unlicensed.join("\n")
  );

  // An allowlist entry that matches nothing has stopped licensing anything and is now just an
  // exemption waiting to be reused by something it was never argued for.
  let stale: Vec<_> = RULE_THREE
    .iter()
    .zip(used)
    .filter(|(_, matched)| !matched)
    .map(|((path, text), _)| format!("{path}: {text}"))
    .collect();
  assert!(
    stale.is_empty(),
    "a rule 3 exemption no longer matches any line; delete it rather than leaving it open:\n{}",
    stale.join("\n")
  );
}

#[test]
fn no_position_is_built_by_a_saturating_operation() {
  let mut found = Vec::new();
  for (path, source) in CRATE {
    if !RESOLUTION.contains(&path) {
      continue;
    }
    for (number, line) in code_lines(source) {
      if line.contains("saturating_") {
        found.push(format!("{path}:{number}: {line}"));
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
  for (path, source) in CRATE {
    if !RESOLUTION.contains(&path) {
      continue;
    }
    for (number, line) in code_lines(source) {
      for integer in INTEGERS {
        if line.contains(&format!("as {integer}")) {
          found.push(format!("{path}:{number}: {line}"));
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
fn the_censuses_can_fail() {
  // Every gate above is a text search, and a text search looking for the wrong text passes
  // forever. Each pattern is exercised against input that does contain it, so a change breaking
  // the matching itself reddens here rather than going quiet.
  let planted = "pub const fn depth(&self) -> u32 { count.saturating_add(1) as u32 }";
  assert!(planted.contains("saturating_"));
  assert!(planted.contains("as u32"));
  assert!(mentions(planted, "u32"));

  // Token matching, not substring matching: a longer word that merely contains one is not a
  // mention, or every `u32::try_from` in a comment would read as a ceiling.
  assert!(!mentions("let x = fu32bar;", "u32"));
  assert!(mentions("fn f() -> u32 {", "u32"));

  // The impl-type reader really does read the three shapes the crate writes.
  assert_eq!(impl_type("impl Span {"), Some("Span"));
  assert_eq!(impl_type("impl<'a> Source<'a> {"), Some("Source"));
  assert_eq!(
    impl_type("impl<'a> Iterator for RegionLines<'a> {"),
    Some("RegionLines")
  );
  assert_eq!(impl_type("pub const fn len(&self) -> usize {"), None);

  // And the comment stripper strips, so no gate can be defeated by prose nor tripped by it.
  assert_eq!(code_lines("  // never write `x as u32` here").count(), 0);
  assert_eq!(code_lines(planted).count(), 1);

  // The two censuses over `pin` are reading something rather than an empty list.
  assert!(ascriptions().len() >= 20);
  assert!(public_numeric_members().len() >= 20);
}

#[test]
fn the_pinned_widths_are_the_values_the_crate_produces() {
  // The ascriptions are compile-time and touch no values, so this is what keeps them over live
  // code rather than over accessors nobody calls.
  let text = "one\ntwo\nthree\n";
  let source = Source::new(text);
  let region = source.resolve(Span::new(2, 9));
  let line = source.line(1).expect("a source has a first line");
  let drawn = region.lines().next().expect("a region is drawn somewhere");

  assert_eq!(
    (
      region.start().line(),
      region.start().column(),
      region.start().offset()
    ),
    (1, 3, 2)
  );
  assert_eq!(
    (line.number(), line.char_count(), line.column_at(1)),
    (1, 3, 2)
  );
  // Four lines in the source, counting the empty one the trailing break leaves; the region runs
  // from line 1 to the `h` on line 3.
  assert_eq!((source.line_count(), region.line_count()), (4, 3));
  assert_eq!(drawn.columns(), 3..4);
  assert_eq!(PathSegment::Index(2).to_string(), "2");
  assert_eq!(Location::new(0, Span::new(2, 9)).source(), 0);
  assert_eq!(source.len(), text.len());
  assert_eq!(
    line
      .line_break()
      .expect("the first line ends in a break")
      .byte_len(),
    1
  );
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
