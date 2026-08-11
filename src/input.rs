//! What a renderer is handed alongside a diagnostic: the caller's texts, and its names for them.
//!
//! Private, and re-exported flat from the crate root. It arrived with the terminal renderer and
//! lived in it until there was a second output; it is here now because it belongs to neither.

use crate::Source;

/// One of the caller's inputs: its text, and whatever the caller calls it.
///
/// A [`Location`](crate::Location) carries a `source: u32` that indexes the list the producer was
/// numbering, so a renderer needs that same list to resolve a span. Mapping an index to a *name*
/// stays with the caller — painty is not a source-file manager — which is why the name comes in
/// here rather than being looked up.
///
/// # Why this is not the terminal's
///
/// It was, for as long as the terminal was the only renderer, and moving it out is the first thing
/// `painty::html` needed that was not HTML. Every output that renders *source* takes the same two
/// things per input, and the alternative — a second identical type behind the second feature —
/// would make `painty::html::Input` and `painty::terminal::Input` two names a caller has to
/// convert between for no reason either of them could state.
///
/// `painty::terminal::Input` still resolves, so nothing that named it has to move. Both module
/// paths are unlinked here on purpose: each is behind a feature this file does not require, and a
/// link into one is broken in every build that leaves it off.
#[derive(Debug, Clone, Copy)]
pub struct Input<'a> {
  source: Source<'a>,
  origin: Option<&'a str>,
}

impl<'a> Input<'a> {
  /// An input with no name.
  #[inline]
  #[must_use]
  pub const fn new(source: Source<'a>) -> Self {
    Self {
      source,
      origin: None,
    }
  }

  /// Names the input — a path, a URL, whatever the caller has.
  #[inline]
  #[must_use]
  pub const fn with_origin(mut self, origin: &'a str) -> Self {
    self.origin = Some(origin);
    self
  }

  /// Returns the text.
  #[inline]
  pub const fn source(&self) -> Source<'a> {
    self.source
  }

  /// Returns the caller's name for it, if it gave one.
  #[inline]
  pub const fn origin(&self) -> Option<&'a str> {
    self.origin
  }
}
