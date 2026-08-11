//! Layer 3, the terminal: ANSI, box drawing, and the cell arithmetic a fixed-width medium needs.
//!
//! Public, unlike the other modules, because a caller selects this renderer by name.

mod ariadne;
mod codespan;
mod detect;
mod miette;
mod paint;
mod present;
mod render;
mod rustc;
mod width;

pub use detect::{CapturedEnvironment, ColorCapability, ColorChoice, Environment};
pub use render::{Input, Terminal};
pub use width::LineCells;

#[cfg(test)]
mod tests;
