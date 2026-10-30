//! Everything resolution hands back borrows the **source text**, not the `&Source` it came through.
//!
//! # The regression this exists to make impossible
//!
//! `Source<'a>` borrows a caller's `&'a str`. Every accessor that returns a piece of that text —
//! a [`Line`], a [`Region`], a `&str` — must return it tied to `'a`. Returning it tied to `&self`
//! instead compiles perfectly well inside the crate and silently makes the value unusable past the
//! borrow: a caller who builds a `Source` in a helper and returns a `Line` from it stops
//! compiling, for a reason that reads like their mistake rather than painty's change.
//!
//! It is a real hazard rather than a hypothetical one. A review found that the signature pins in
//! `tests/numeric_widths.rs` accepted exactly that regression, because tying the receiver borrow
//! and the source lifetime to one `'a` lets the first absorb the second. Those pins are fixed —
//! and this file asserts the same property from the side a caller actually feels, which is where
//! it is legible without knowing anything about higher-ranked function pointers.
//!
//! # How it asserts it
//!
//! Every function here builds a `Source` in a local, drops it at the end of the function, and
//! returns something that came out of it. If any accessor regresses to `-> Thing<'_>`, that
//! function stops compiling with "cannot return value referencing local variable" and names itself.
//! There is nothing to run — the assertion is that this file builds — so the test at the bottom
//! only exercises the results, to keep the functions from being dead code that nobody notices has
//! rotted.

use painty::{Line, Lines, Region, RegionLine, Source, Span};

fn text(source: &str) -> &str {
  Source::new(source).text()
}

fn lines(source: &str) -> Lines<'_> {
  Source::new(source).lines()
}

fn line(source: &str) -> Option<Line<'_>> {
  Source::new(source).line(1)
}

fn line_at(source: &str) -> Line<'_> {
  Source::new(source).line_at(0)
}

fn line_text(source: &str) -> &str {
  Source::new(source).line_at(0).text()
}

fn region(source: &str) -> Region<'_> {
  Source::new(source).resolve(Span::new(0, 3))
}

fn region_text(source: &str) -> &str {
  Source::new(source).resolve(Span::new(0, 3)).text()
}

fn region_line(source: &str) -> Option<RegionLine<'_>> {
  Source::new(source).resolve(Span::new(0, 3)).lines().next()
}

fn covered_text(source: &str) -> Option<&str> {
  Source::new(source)
    .resolve(Span::new(0, 3))
    .lines()
    .next()
    .map(|drawn| drawn.covered_text())
}

fn nested_line(source: &str) -> Option<Line<'_>> {
  // Two hops: the line comes out of a region that came out of a source, and all three of them are
  // gone by the time it is returned.
  Source::new(source)
    .resolve(Span::new(0, 3))
    .lines()
    .next()
    .map(|drawn| drawn.line())
}

#[test]
fn every_borrow_outlives_the_source_it_came_through() {
  let owned = String::from("alpha\nbeta\n");

  assert_eq!(text(&owned), owned);
  assert_eq!(lines(&owned).count(), 3);
  assert_eq!(line(&owned).expect("a first line").number(), 1);
  assert_eq!(line_at(&owned).number(), 1);
  assert_eq!(line_text(&owned), "alpha");
  assert_eq!(region(&owned).text(), "alp");
  assert_eq!(region_text(&owned), "alp");
  assert_eq!(
    region_line(&owned).expect("a drawn line").covered(),
    Span::new(0, 3)
  );
  assert_eq!(covered_text(&owned), Some("alp"));
  assert_eq!(nested_line(&owned).expect("a drawn line").number(), 1);
}
