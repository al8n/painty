#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(missing_docs)]

// The test harness is a `std` program whatever the crate under it is, so a `--no-default-features`
// build still needs the crate to link it before `#[test]` can be expanded. Only under `cfg(test)`,
// and only when the feature that would have brought it is off: nothing in `src/` may reach for it.
#[cfg(all(test, not(feature = "std")))]
extern crate std;

mod diagnostic;
mod source;
mod style;

// Every renderer that reads source text takes the caller's inputs the same way, so the type that
// carries one sits below all of them rather than inside the first. The same goes for deciding which
// lines are left out: two copies of that rule produced a diagnostic whose two renderings disagreed
// about which source a reader was shown.
#[cfg(any(feature = "terminal", feature = "html"))]
mod elide;
#[cfg(any(feature = "terminal", feature = "html"))]
mod input;

#[cfg(feature = "html")]
#[cfg_attr(docsrs, doc(cfg(feature = "html")))]
pub mod html;

#[cfg(feature = "terminal")]
#[cfg_attr(docsrs, doc(cfg(feature = "terminal")))]
pub mod terminal;

#[cfg(feature = "tokora")]
#[cfg_attr(docsrs, doc(cfg(feature = "tokora")))]
pub mod tokora;

pub use diagnostic::{Diagnostic, Label, Location, PathSegment, Severity, Span};
#[cfg(any(feature = "terminal", feature = "html"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "terminal", feature = "html"))))]
pub use input::Input;
pub use source::{Line, LineBreak, Lines, Position, Region, RegionLine, RegionLines, Source};
pub use style::{Ansi16, Color, Palette, Role, Style, Theme};
