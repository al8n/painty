/// A half-open byte range `start..end` within one input's text.
///
/// # It is a range, not a position in a document
///
/// A span on its own names no input. [`Location`](super::Location) is the pair that does, and
/// resolution happens against a [`Source`](crate::Source) the caller chose — see
/// [`Source::resolve`](crate::Source::resolve), which is where a span meets text and where a span
/// that disagrees with that text is made harmless.
///
/// # A malformed span is a rendering input, not a bug to panic on
///
/// Nothing here validates `start` and `end`, and nothing asserts that one is before the other.
/// There is no text at construction to validate against, and a producer whose offsets came from a
/// different revision of a file will hand a renderer a range past the end of it; a renderer that
/// panics on one is worse than useless, because the diagnostic it was asked to show is the thing
/// that gets lost. Noticing a producer's defect is not painty's job and it has no channel to
/// report one, so the behaviour is defined rather than asserted:
///
/// - out of range is [`Source::resolve`](crate::Source::resolve)'s to clamp, and it documents how;
/// - **inverted** — an `end` before its own `start` — reads here as the empty range at `start`:
///   [`len`](Self::len) saturates to zero, [`is_empty`](Self::is_empty) is true, and
///   [`contains`](Self::contains) is false for every offset. Resolution takes the end to the start
///   rather than leaving it behind, so a resolved region is never inverted.
///
/// A debug assertion here would undo all of that in exactly the builds most people run.
///
/// ```
/// use painty::Span;
///
/// let span = Span::new(4, 9);
/// assert_eq!(span.start(), 4);
/// assert_eq!(span.end(), 9);
/// assert_eq!(span.len(), 5);
///
/// let caret = Span::empty(4);
/// assert!(caret.is_empty());
/// assert_eq!(caret.start(), caret.end());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
  start: usize,
  end: usize,
}

impl Span {
  /// A range from `start` to `end`.
  ///
  /// An `end` before its own `start` is accepted and reads as the empty range at `start` — see the
  /// type's documentation for why that is defined rather than asserted.
  #[inline]
  pub const fn new(start: usize, end: usize) -> Self {
    Self { start, end }
  }

  /// The empty range at `at` — a caret rather than an underline.
  #[inline]
  pub const fn empty(at: usize) -> Self {
    Self { start: at, end: at }
  }

  /// Returns the first byte offset in the range.
  #[inline]
  pub const fn start(&self) -> usize {
    self.start
  }

  /// Returns the offset one past the last byte in the range.
  #[inline]
  pub const fn end(&self) -> usize {
    self.end
  }

  /// Returns how many bytes the range covers, saturating at zero for an inverted pair.
  #[inline]
  pub const fn len(&self) -> usize {
    self.end.saturating_sub(self.start)
  }

  /// Returns whether the range covers no bytes.
  #[inline]
  pub const fn is_empty(&self) -> bool {
    self.end <= self.start
  }

  /// Returns whether `offset` falls inside the range.
  ///
  /// Half-open, so the range's own [`end`](Self::end) is not contained and an empty range contains
  /// nothing.
  #[inline]
  pub const fn contains(&self, offset: usize) -> bool {
    self.start <= offset && offset < self.end
  }
}
