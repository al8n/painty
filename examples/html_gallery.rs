//! Renders one HTML document per diagnostic shape into `assets/html/`.
//!
//! These are the crate's appearance goldens for the HTML surface, and running this example is how
//! they are re-blessed:
//!
//! ```text
//! cargo run --example html_gallery --features html
//! ```
//!
//! `tests/html_gallery.rs` fails when a committed file and a fresh render disagree, so a renderer
//! change that reaches the document cannot land without the document changing with it. It is the
//! same corpus `examples/svg_gallery.rs` draws — one definition in `tests/common/specimens.rs`,
//! reached by `#[path]` — so the two surfaces can be read side by side and a fifth specimen gets
//! both.
//!
//! The documents carry no stylesheet, because painty writes none: every class in them is named in
//! the [`html`](painty::html) module documentation, and an embedder supplies the appearance.

use painty::html::Html;

#[path = "../tests/common/specimens.rs"]
mod specimens;

fn main() {
  let dir = std::path::Path::new("assets/html");
  std::fs::create_dir_all(dir).expect("create assets/html");

  for specimen in specimens::SPECIMENS {
    let diagnostic = (specimen.build)(specimen.text);
    let mut html = String::new();
    Html::new()
      .render(&diagnostic, &specimen.inputs(), &mut html)
      .expect("a String accepts everything");

    let path = dir.join(format!("{}.html", specimen.name));
    std::fs::write(&path, &html).expect("write html");
    println!(
      "{:<18} {:>6} bytes  {}",
      specimen.name,
      html.len(),
      path.display()
    );
  }
}
