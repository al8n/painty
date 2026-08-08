use core::iter::FusedIterator;

use crate::Span;

/// What ends a line.
///
/// # Three of them, and the third is why this is not [`str::lines`]
///
/// `\n` and `\r\n` are the two everybody expects. A lone `\r` is here because the inputs painty
/// renders say so: GraphQL's grammar lists all three as line terminators, and a document written
/// on a classic Mac editor still exists. [`str::lines`] recognises only the first two, so a
/// diagnostic resolved through it would report a line number that disagrees with the editor the
/// reader is looking at — silently, and only for the files nobody tests with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineBreak {
  /// A line feed, `\n`.
  Lf,
  /// A carriage return followed by a line feed, `\r\n`.
  CrLf,
  /// A lone carriage return, `\r`.
  Cr,
}

impl LineBreak {
  /// Returns how many bytes the break occupies.
  #[inline]
  pub const fn byte_len(&self) -> usize {
    match self {
      Self::Lf | Self::Cr => 1,
      Self::CrLf => 2,
    }
  }

  /// Returns the break's own text.
  #[inline]
  pub const fn as_str(&self) -> &'static str {
    match self {
      Self::Lf => "\n",
      Self::CrLf => "\r\n",
      Self::Cr => "\r",
    }
  }
}

/// Returns the break beginning at `index`, if one does.
///
/// Byte-wise on purpose: no byte of a multi-byte UTF-8 sequence is below `0x80`, so `\r` and `\n`
/// can only ever be themselves and this needs no character decoding.
#[inline]
pub(super) fn break_at(bytes: &[u8], index: usize) -> Option<LineBreak> {
  match bytes.get(index) {
    Some(b'\n') => Some(LineBreak::Lf),
    Some(b'\r') if matches!(bytes.get(index + 1), Some(b'\n')) => Some(LineBreak::CrLf),
    Some(b'\r') => Some(LineBreak::Cr),
    _ => None,
  }
}

/// Reads the line that begins at `start`, which must be a line start.
pub(super) fn scan(source: &str, start: usize, number: u64) -> Line<'_> {
  let bytes = source.as_bytes();
  let mut index = start;
  let line_break = loop {
    match break_at(bytes, index) {
      Some(found) => break Some(found),
      None if index >= bytes.len() => break None,
      None => index += 1,
    }
  };

  Line {
    number,
    start,
    text: &source[start..index],
    line_break,
  }
}

/// One line of a source, without whatever ended it.
///
/// The terminator is reported separately rather than being part of [`text`](Self::text): a
/// renderer underlining a line wants its content, and a renderer that has to know whether the
/// input was CRLF can ask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Line<'a> {
  number: u64,
  start: usize,
  text: &'a str,
  line_break: Option<LineBreak>,
}

impl<'a> Line<'a> {
  /// Returns the 1-based line number.
  #[inline]
  pub const fn number(&self) -> u64 {
    self.number
  }

  /// Returns the line's content, not including whatever ended it.
  #[inline]
  pub const fn text(&self) -> &'a str {
    self.text
  }

  /// Returns the byte range of the line's content, not including whatever ended it.
  #[inline]
  pub const fn span(&self) -> Span {
    Span::new(self.start, self.start + self.text.len())
  }

  /// Returns what ended the line, or `None` for the last line of a source that does not end in a
  /// break.
  #[inline]
  pub const fn line_break(&self) -> Option<LineBreak> {
    self.line_break
  }

  /// Returns how many characters the line's content holds.
  ///
  /// Characters, not display cells: a CJK ideograph counts once here and occupies two columns on a
  /// terminal. Keeping display width out of resolution is what keeps this layer free of a Unicode
  /// table — the conversion belongs to the renderer that has a medium to convert for.
  #[inline]
  pub fn char_count(&self) -> u64 {
    self.text.chars().fold(0u64, |count, _| count + 1)
  }

  /// Returns the 1-based character column of `offset` on this line.
  ///
  /// Total, and clamping: an offset before the line answers `1`, an offset past its content
  /// answers one past the last character, and an offset inside a multi-byte character answers the
  /// column of that character rather than splitting it.
  ///
  /// ```
  /// use painty::Source;
  ///
  /// let source = Source::new("héllo\nworld");
  /// let line = source.line(1).unwrap();
  ///
  /// assert_eq!(line.column_at(0), 1);
  /// assert_eq!(line.column_at(1), 2); // `é` starts here
  /// assert_eq!(line.column_at(2), 2); // ...and this is its second byte
  /// assert_eq!(line.column_at(3), 3);
  /// assert_eq!(line.column_at(99), 6); // one past the last of five characters
  /// ```
  pub fn column_at(&self, offset: usize) -> u64 {
    // Clamped into the line first, then measured from its start. Spelled with `max`/`min` rather
    // than a saturating subtraction so that nothing on the path a column is built by is a
    // saturating operation — see `tests/exact_positions.rs`, which asserts exactly that.
    let mut relative = offset.clamp(self.start, self.start + self.text.len()) - self.start;
    while !self.text.is_char_boundary(relative) {
      relative -= 1;
    }
    self.text[..relative]
      .chars()
      .fold(1u64, |column, _| column + 1)
  }
}

/// The iterator [`Source::lines`](crate::Source::lines) returns.
///
/// # A source always has at least one line, and a trailing break adds one
///
/// `""` is one empty line. `"a\n"` is two — `"a"`, then the empty line after it — which is what an
/// editor shows and where a span pointing at the very end of the file resolves to. This is the one
/// place painty deliberately parts company with [`str::lines`], which drops that final empty line
/// and would leave an end-of-file position with nowhere to be.
#[derive(Debug, Clone)]
pub struct Lines<'a> {
  source: &'a str,
  next: usize,
  number: u64,
  done: bool,
}

impl<'a> Lines<'a> {
  #[inline]
  pub(super) const fn new(source: &'a str) -> Self {
    Self {
      source,
      next: 0,
      number: 1,
      done: false,
    }
  }

  /// Resumes the walk at `start`, which must be the first byte of line `number`.
  #[inline]
  pub(super) const fn resume(source: &'a str, start: usize, number: u64) -> Self {
    Self {
      source,
      next: start,
      number,
      done: false,
    }
  }
}

impl<'a> Iterator for Lines<'a> {
  type Item = Line<'a>;

  fn next(&mut self) -> Option<Self::Item> {
    if self.done {
      return None;
    }

    let line = scan(self.source, self.next, self.number);
    match line.line_break {
      Some(line_break) => {
        self.next = line.span().end() + line_break.byte_len();
        self.number += 1;
      }
      None => self.done = true,
    }
    Some(line)
  }
}

impl FusedIterator for Lines<'_> {}
