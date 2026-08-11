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
pub use render::Terminal;
pub use width::LineCells;

/// Re-exported so that `painty::terminal::Input` keeps resolving. It moved to the crate root when
/// the HTML renderer turned out to need the same type — see [`Input`]. Unlinked
/// deliberately: `painty::html` is behind its own feature, so a link to it is broken in every
/// build that enables this one and not that one.
pub use crate::Input;

#[cfg(test)]
mod tests;
