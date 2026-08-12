//! The committed SVGs in `assets/svg/` are what the renderer produces today.
//!
//! # These are appearance goldens, and the ordering is the same as the terminal's
//!
//! `tests/terminal_appearance.rs` states why a golden comes *after* the invariants: written first,
//! it becomes the thing the invariants get adjusted to agree with, and then both agree with the
//! defect. These sit on the same footing. What holds the arithmetic is
//! `src/terminal/svg/tests.rs` — that every unit is drawn at the cell the terminal gave it, and
//! that a caret shares the coordinate of the glyph it marks. What is pinned *here* is only that the
//! images in the repository are the ones the code writes, so re-blessing is cheap and safe.
//!
//! # Why the files are committed rather than generated on demand
//!
//! They are the only part of this crate a reader can look at without running it, and an image that
//! nothing regenerates goes stale silently. This test is what stops that: a renderer change that
//! reaches the image cannot land without the image changing with it.

#![cfg(feature = "svg")]

use painty::terminal::Terminal;

#[path = "common/specimens.rs"]
mod specimens;

fn rendered(specimen: &specimens::Specimen) -> String {
  let diagnostic = (specimen.build)(specimen.text);
  let mut svg = String::new();
  Terminal::plain()
    .render_svg(&diagnostic, &specimen.inputs(), &mut svg)
    .expect("a String accepts everything");
  svg
}

#[test]
fn every_committed_image_is_what_the_renderer_writes_today() {
  for specimen in specimens::SPECIMENS {
    let path = format!("assets/svg/{}.svg", specimen.name);
    let committed = std::fs::read_to_string(&path).unwrap_or_else(|error| {
      panic!("{path} is missing ({error}); `cargo run --example svg_gallery --features svg`")
    });
    assert_eq!(
      rendered(specimen),
      committed,
      "{path} is not what the renderer writes; re-bless with \
       `cargo run --example svg_gallery --features svg`"
    );
  }
}

/// The gallery is four pictures of four cases, not four pictures of one.
///
/// A gallery whose entries differ only decoratively teaches a reader nothing, and this is the
/// assertion that fails if someone adds a fifth specimen that is the second one in another colour.
#[test]
fn the_specimens_differ_in_what_they_show() {
  let images: Vec<String> = specimens::SPECIMENS.iter().map(rendered).collect();
  for (i, a) in images.iter().enumerate() {
    for (j, b) in images.iter().enumerate().skip(i + 1) {
      assert_ne!(
        a,
        b,
        "{} and {} render identically",
        specimens::SPECIMENS[i].name,
        specimens::SPECIMENS[j].name
      );
    }
  }
}

/// Every cell coordinate is a multiple of the cell width, in every specimen.
///
/// This is the property the row-stretching design could not hold, so it is worth an assertion that
/// does not depend on any particular image: `textLength` with `lengthAdjust="spacing"` distributes
/// one correction across a row and lands glyphs between cells. Named against the defect rather than
/// against the current output — a wrong renderer produces coordinates off the grid, and this fails
/// on the first one.
#[test]
fn every_placed_cell_sits_on_the_grid() {
  const CELL: usize = 8;

  for specimen in specimens::SPECIMENS {
    let svg = rendered(specimen);
    let mut placed = 0usize;
    for fragment in svg.split("<tspan x=\"").skip(1) {
      let digits: String = fragment.chars().take_while(char::is_ascii_digit).collect();
      let x: usize = digits.parse().expect("a tspan x is a number");
      assert_eq!(
        x % CELL,
        0,
        "{}: a cell is drawn at x={x}, which is not a multiple of {CELL}",
        specimen.name
      );
      placed += 1;
    }
    assert!(
      placed > 0,
      "{}: no placed cells found, so this assertion checked nothing",
      specimen.name
    );
  }
}
