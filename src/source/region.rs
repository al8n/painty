use core::iter::FusedIterator;

use super::{Line, Lines, Position};
use crate::Span;

/// A span resolved against the text it points into.
///
/// Produced by [`Source::resolve`](crate::Source::resolve), which is also where the clamping rules
/// that make this total are documented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Region<'a> {
  source: &'a str,
  span: Span,
  start: Position,
  end: Position,
  first_line_start: usize,
  last_line: u32,
}

impl<'a> Region<'a> {
  pub(super) const fn new(
    source: &'a str,
    span: Span,
    start: Position,
    end: Position,
    first_line_start: usize,
    last_line: u32,
  ) -> Self {
    Self {
      source,
      span,
      start,
      end,
      first_line_start,
      last_line,
    }
  }

  /// Returns the byte range the region covers, after clamping.
  ///
  /// This is the range every other answer here is consistent with, and it is not necessarily the
  /// range that was asked for — see [`Source::resolve`](crate::Source::resolve).
  #[inline]
  pub const fn span(&self) -> Span {
    self.span
  }

  /// Returns where the region begins.
  #[inline]
  pub const fn start(&self) -> Position {
    self.start
  }

  /// Returns where the region ends, one past its last byte.
  ///
  /// A region that ends exactly at a line break resolves its end to column 1 of the *following*
  /// line, because that is where the offset is. The lines a reader would see it on are
  /// [`lines`](Self::lines), which does not include that one.
  #[inline]
  pub const fn end(&self) -> Position {
    self.end
  }

  /// Returns the covered text.
  ///
  /// Equal to re-slicing the source by [`span`](Self::span), which is an invariant worth stating
  /// because it is the one a resolution bug breaks first.
  #[inline]
  pub fn text(&self) -> &'a str {
    &self.source[self.span.start()..self.span.end()]
  }

  /// Returns whether the region is drawn across more than one line.
  #[inline]
  pub const fn is_multiline(&self) -> bool {
    self.last_line > self.start.line()
  }

  /// Returns how many lines [`lines`](Self::lines) will yield.
  #[inline]
  pub const fn line_count(&self) -> u32 {
    self.last_line - self.start.line() + 1
  }

  /// Iterates the lines the region is drawn on, each with the part of it that falls there.
  ///
  /// # Which lines those are
  ///
  /// Every line from the one the region starts on to the one its last byte is on. A region whose
  /// exclusive end lands on column 1 of the next line — the common case of a span that swallows
  /// its own trailing newline — stops at the line before it, because there is nothing of it to
  /// draw there. That is the difference between this and [`end`](Self::end), and it is what makes
  /// a multi-line bracket open and close on the lines the region actually covers.
  ///
  /// ```
  /// use painty::{Source, Span};
  ///
  /// let source = Source::new("first\nsecond\nthird\n");
  ///
  /// // `first\nsecond\n` — the trailing newline puts the exclusive end on line 3, column 1.
  /// let region = source.resolve(Span::new(0, 13));
  /// assert_eq!(region.end().line(), 3);
  /// assert_eq!(region.end().column(), 1);
  ///
  /// let drawn: Vec<u32> = region.lines().map(|line| line.line().number()).collect();
  /// assert_eq!(drawn, [1, 2]);
  /// assert_eq!(region.line_count(), 2);
  /// ```
  #[inline]
  pub fn lines(&self) -> RegionLines<'a> {
    RegionLines {
      lines: Lines::resume(self.source, self.first_line_start, self.start.line()),
      span: self.span,
      remaining: self.line_count(),
    }
  }
}

/// One line of a [`Region`], with the part of the region that falls on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegionLine<'a> {
  line: Line<'a>,
  covered: Span,
}

impl<'a> RegionLine<'a> {
  /// Returns the line.
  #[inline]
  pub const fn line(&self) -> Line<'a> {
    self.line
  }

  /// Returns the byte range of the region that falls on this line.
  ///
  /// Clipped to the line's content, so it never includes the break that ended it. It is empty
  /// where the region touches the line without covering any of its text — a caret, or a span that
  /// reaches a line only to swallow its newline.
  #[inline]
  pub const fn covered(&self) -> Span {
    self.covered
  }

  /// Returns the text [`covered`](Self::covered) names.
  #[inline]
  pub fn covered_text(&self) -> &'a str {
    let base = self.line.span().start();
    &self.line.text()[self.covered.start() - base..self.covered.end() - base]
  }

  /// Returns the 1-based character columns the region occupies on this line, as `start..end`.
  ///
  /// Half-open like the byte range, so an empty region gives an empty column range and a renderer
  /// drawing a caret there knows to draw one cell rather than none.
  #[inline]
  pub fn columns(&self) -> core::ops::Range<u32> {
    self.line.column_at(self.covered.start())..self.line.column_at(self.covered.end())
  }
}

/// The iterator [`Region::lines`] returns.
#[derive(Debug, Clone)]
pub struct RegionLines<'a> {
  lines: Lines<'a>,
  span: Span,
  remaining: u32,
}

impl<'a> Iterator for RegionLines<'a> {
  type Item = RegionLine<'a>;

  fn next(&mut self) -> Option<Self::Item> {
    if self.remaining == 0 {
      return None;
    }
    let line = self.lines.next()?;
    self.remaining -= 1;

    // Clipped at BOTH ends of the line's content, not just the near one. A region can begin inside
    // the break that ended a line — a span naming the newline itself does — and a start clamped
    // only from below would then sit past the text it indexes into.
    let content = line.span();
    let start = self.span.start().max(content.start()).min(content.end());
    let end = self.span.end().min(content.end()).max(start);

    Some(RegionLine {
      line,
      covered: Span::new(start, end),
    })
  }

  /// Exact, and safely so: the count is [`Region::line_count`], which painty computed from the
  /// source itself rather than taking it from a caller.
  #[inline]
  fn size_hint(&self) -> (usize, Option<usize>) {
    let remaining = self.remaining as usize;
    (remaining, Some(remaining))
  }
}

impl FusedIterator for RegionLines<'_> {}
