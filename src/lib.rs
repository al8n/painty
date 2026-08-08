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

#[cfg(feature = "tokora")]
#[cfg_attr(docsrs, doc(cfg(feature = "tokora")))]
pub mod tokora;

pub use diagnostic::{Diagnostic, Label, Location, PathSegment, Severity, Span};
pub use source::{Line, LineBreak, Lines, Position, Region, RegionLine, RegionLines, Source};
