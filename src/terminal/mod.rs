//! Layer 3, the terminal: ANSI, box drawing, and the cell arithmetic a fixed-width medium needs.
//!
//! Public, unlike the other modules, because a caller selects this renderer by name.

mod detect;
mod render;
mod width;

// Not selectable yet, and so not compiled into a release: the second style exists at this commit
// to be the second IMPLEMENTATION the seam is derived from, and publishing a selector before the
// seam is cut would freeze it at whatever one file happened to need first. The next commit takes
// this attribute off and makes both styles a caller's choice.
#[cfg(test)]
mod miette;

pub use detect::{CapturedEnvironment, ColorCapability, ColorChoice, Environment};
pub use render::{Input, Terminal};
pub use width::LineCells;

#[cfg(test)]
mod tests;
