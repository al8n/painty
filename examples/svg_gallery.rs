//! Renders one SVG per diagnostic shape into `assets/svg/`.
//!
//! These are the crate's appearance goldens for the SVG surface, and running this example is how
//! they are re-blessed:
//!
//! ```text
//! cargo run --example svg_gallery --features svg
//! ```
//!
//! `tests/svg_gallery.rs` fails when a committed file and a fresh render disagree, so a renderer
//! change that reaches the image cannot land without the image changing with it. The invariants
//! underneath — which cell each unit is drawn at, and that a caret shares its glyph's coordinate —
//! are held in `src/terminal/svg/tests.rs`, which is what makes re-blessing these cheap.

use painty::terminal::Terminal;

#[path = "../tests/common/specimens.rs"]
mod specimens;

fn main() {
  let dir = std::path::Path::new("assets/svg");
  std::fs::create_dir_all(dir).expect("create assets/svg");

  for specimen in specimens::SPECIMENS {
    let diagnostic = (specimen.build)(specimen.text);
    let mut svg = String::new();
    Terminal::plain()
      .render_svg(&diagnostic, &specimen.inputs(), &mut svg)
      .expect("a String accepts everything");

    let path = dir.join(format!("{}.svg", specimen.name));
    std::fs::write(&path, &svg).expect("write svg");
    println!(
      "{:<18} {:>6} bytes  {}",
      specimen.name,
      svg.len(),
      path.display()
    );
  }
}
