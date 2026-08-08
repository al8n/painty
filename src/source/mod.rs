//! Layer 2: turning byte offsets into something a reader can be pointed at.
//!
//! Private, and re-exported flat from the crate root. Whatever a reader needs to know is on
//! [`Source`] and on the types it hands back.

mod line;
mod region;

pub use line::{Line, LineBreak, Lines};
pub use region::{Region, RegionLine, RegionLines};

use crate::Span;

#[cfg(test)]
mod tests;

/// A byte offset with the line and column it lands on.
///
/// Columns count **characters**, from 1. Not bytes, which no reader can see, and not display
/// cells, which need a Unicode width table and a medium to be a width *of* — the renderer that has
/// one converts, and that is what keeps this layer dependency-free.
///
/// # Two units, and the rule for telling them apart
///
/// A **byte offset** is a `usize`, because it is an index into the caller's `&str` and that is
/// what Rust slices with. It is exact: the widening to `u64` an FFI export needs is lossless from
/// every `usize` Rust has.
///
/// A **line or column ordinal** is a `u64`, and neither `u32` nor `usize`.
///
/// - Not `u32`. A position type is the one thing every renderer and every FFI consumer touches, so
///   its width is unchangeable once published, and `u32` is a bet that no source has more than
///   4,294,967,295 lines. The bet would very probably be won. It is still the wrong shape to
///   publish, because losing it means either wrapping or clamping, and a clamped ordinal is
///   indistinguishable from a real one — a value that *lies* rather than one that fails.
/// - Not `usize`, because it is platform-dependent, and the resolved model is meant to cross a C
///   ABI unchanged.
///
/// `u64` is what makes the whole question disappear rather than move. A line ordinal is at most
/// one more than the text's length in bytes, Rust caps a single object at `isize::MAX`, so on the
/// widest target this crate can be built for an ordinal cannot exceed 2^63. There is no clamping
/// anywhere on this path and no arithmetic here can overflow — which `tests/exact_positions.rs`
/// asserts against the source rather than leaving to this paragraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Position {
  offset: usize,
  line: u64,
  column: u64,
}

impl Position {
  /// Returns the byte offset.
  #[inline]
  pub const fn offset(&self) -> usize {
    self.offset
  }

  /// Returns the 1-based line number.
  #[inline]
  pub const fn line(&self) -> u64 {
    self.line
  }

  /// Returns the 1-based character column.
  #[inline]
  pub const fn column(&self) -> u64 {
    self.column
  }
}

/// How far a walk over the text has got, and what line it is on.
#[derive(Debug, Clone, Copy)]
struct Cursor {
  offset: usize,
  line: u64,
  line_start: usize,
}

impl Cursor {
  const START: Self = Self {
    offset: 0,
    line: 1,
    line_start: 0,
  };
}

/// Walks `bytes` from `cursor` to `target`, counting the line breaks in between.
///
/// `target` must be at or after `cursor.offset`; the walk is forward-only, which is the whole
/// reason resolving in offset order costs one pass rather than one pass per offset.
///
/// The line counter is incremented rather than saturated, and that is the point: it is a `u64`
/// bounded by the length of a `&str`, so it cannot reach its maximum, and if that reasoning were
/// ever wrong a debug build would panic here instead of handing back a number that quietly stopped
/// being true.
fn advance(bytes: &[u8], mut cursor: Cursor, target: usize) -> Cursor {
  let mut index = cursor.offset;
  while index < target {
    match line::break_at(bytes, index) {
      Some(line_break) => {
        let after = index + line_break.byte_len();
        if after > target {
          // A half-crossed break would leave `line_start` past `target`, and every caller here
          // slices between the two. `Source::floor` and `Source::ceil` move an offset out of a
          // CRLF before it ever gets here, so this is what makes the walk total rather than a case
          // that arises.
          break;
        }
        index = after;
        cursor.line += 1;
        cursor.line_start = index;
      }
      None => index += 1,
    }
  }
  cursor.offset = target;
  cursor
}

/// One input's text, addressed by byte offset.
///
/// # Why resolution is a layer and not an implementation detail of a renderer
///
/// A terminal renderer and an HTML renderer both emit text, so their shared work could live inside
/// either of them. A native front end does not emit text: it builds a widget tree, on the far side
/// of an FFI boundary, in another language, and it cannot consume a string. It needs the
/// *structure* — which line, which columns, which excerpt, which label belongs where. So the
/// resolution work is a public data model of its own rather than something each renderer redoes.
///
/// It earns that twice over. The invariants of layout are assertable here, *before* anything is
/// stringified — that a re-sliced excerpt equals the span it came from, that a multi-line region
/// opens and closes on the lines it covers, that no reported column exceeds its line — and a
/// property test over those cannot be re-blessed into agreeing with a bug the way a golden
/// snapshot can.
///
/// # It is the text, and nothing about a file
///
/// painty is not a source-file manager. A [`Location`](crate::Location) carries a `source: u32`
/// index into whatever list of inputs the caller was given, and turning that index into a name or
/// into the text itself stays with the caller — which already holds that mapping, and every
/// attempt to own it ends in a virtual filesystem.
///
/// # Everything here is total
///
/// Offsets arrive from a producer that may have parsed a different revision of the text, and a
/// renderer that panics on one has lost the diagnostic it was asked to show. Nothing here returns
/// a `Result` and nothing panics on an out-of-range or mid-character offset; the clamping rules
/// are on [`resolve`](Self::resolve) and [`Line::column_at`].
///
/// # Nothing here allocates
///
/// Every type borrows the caller's text and every walk is an iterator. The cost of that choice is
/// stated rather than hidden: a `Source` holds no line index, so a lookup by offset scans from the
/// top of the text. Resolving a set of positions in offset order — which
/// [`Label::order`](crate::Label::order) exists to arrange — keeps the whole set to one pass.
///
/// ```
/// use painty::{Source, Span};
///
/// let source = Source::new("query {\n  hero { name }\n}\n");
///
/// let at = source.position(10);
/// assert_eq!((at.line(), at.column()), (2, 3));
///
/// let region = source.resolve(Span::new(10, 14));
/// assert_eq!(region.text(), "hero");
/// assert!(!region.is_multiline());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Source<'a> {
  text: &'a str,
}

impl<'a> Source<'a> {
  /// Wraps the text of one input.
  #[inline]
  pub const fn new(text: &'a str) -> Self {
    Self { text }
  }

  /// Returns the text.
  #[inline]
  pub const fn text(&self) -> &'a str {
    self.text
  }

  /// Returns the text's length in bytes.
  #[inline]
  pub const fn len(&self) -> usize {
    self.text.len()
  }

  /// Returns whether the text is empty — which is still one (empty) line.
  #[inline]
  pub const fn is_empty(&self) -> bool {
    self.text.is_empty()
  }

  /// Iterates every line, from the first.
  ///
  /// See [`Lines`] for the two edge cases worth knowing: an empty source is one empty line, and a
  /// trailing break adds a final empty one.
  #[inline]
  pub const fn lines(&self) -> Lines<'a> {
    Lines::new(self.text)
  }

  /// Returns how many lines the text has.
  ///
  /// Linear in the length of the text. A caller that is about to walk the lines anyway should walk
  /// them instead of counting first.
  ///
  /// Read off the last line's own number rather than counted with [`Iterator::count`], which would
  /// answer a `usize` and need a cast to become the ordinal this reports. There is always a last
  /// line — see [`Lines`].
  #[inline]
  pub fn line_count(&self) -> u64 {
    self.lines().last().map_or(1, |line| line.number())
  }

  /// Returns line `number`, counting from 1, or `None` past the end.
  ///
  /// Linear in the offset of that line.
  #[inline]
  pub fn line(&self, number: u64) -> Option<Line<'a>> {
    self.lines().find(|line| line.number() == number)
  }

  /// Returns the line `offset` falls on, clamped into the text.
  ///
  /// Linear in `offset`.
  #[inline]
  pub fn line_at(&self, offset: usize) -> Line<'a> {
    let cursor = advance(self.text.as_bytes(), Cursor::START, self.floor(offset));
    line::scan(self.text, cursor.line_start, cursor.line)
  }

  /// Returns the line and column `offset` lands on.
  ///
  /// Total: an offset past the end of the text answers the end of the text, and an offset inside a
  /// character answers that character's own position rather than splitting it. Linear in `offset`.
  #[inline]
  pub fn position(&self, offset: usize) -> Position {
    self.position_from(Cursor::START, self.floor(offset)).0
  }

  /// Resolves `span` against the text.
  ///
  /// # Clamping, because a span and a text can disagree
  ///
  /// A span is produced by something that read the input; the text is whatever the caller hands
  /// over now, and the two are not guaranteed to be the same revision. Rather than refuse or
  /// panic, resolution moves the range to the nearest one this text can express, and
  /// [`Region::span`] reports what it settled on:
  ///
  /// - an offset past the end of the text becomes the end of the text;
  /// - an end before its own start is taken to that start, so a resolved region is never
  ///   inverted;
  /// - a boundary inside an **atom** moves outwards — the start back to the atom's first byte, the
  ///   end on past its last. Widening rather than narrowing, because half an atom is not something
  ///   a reader can be shown, and a region that grew by one is repairable where one that silently
  ///   dropped it is not.
  ///
  /// There are two kinds of atom, and the second is easy to miss: a multi-byte **character**, and
  /// a **CRLF**, whose two bytes are one line break with a perfectly good character boundary in
  /// the middle of it.
  ///
  /// Both ends of the resulting region are character boundaries, which is what makes
  /// [`Region::text`] and [`RegionLine::covered_text`] infallible slices, and neither is inside a
  /// break, which is what keeps a region's coverage inside the region.
  ///
  /// ```
  /// use painty::{Source, Span};
  ///
  /// let source = Source::new("héllo");
  ///
  /// // 1..2 is the first byte of a two-byte character; both ends move outwards.
  /// let region = source.resolve(Span::new(1, 2));
  /// assert_eq!(region.span(), Span::new(1, 3));
  /// assert_eq!(region.text(), "é");
  ///
  /// // Past the end, and inverted, both land somewhere expressible.
  /// assert_eq!(source.resolve(Span::new(99, 120)).span(), Span::new(6, 6));
  /// ```
  pub fn resolve(&self, span: Span) -> Region<'a> {
    let start = self.floor(span.start());
    let end = self.ceil(span.end().max(start));

    let (start_position, cursor) = self.position_from(Cursor::START, start);
    let first_line_start = cursor.line_start;
    let (end_position, _) = self.position_from(cursor, end);

    // A region whose exclusive end sits at column 1 of a later line reaches that line without
    // covering anything on it — the shape a span that swallowed its own trailing newline has. The
    // last line a reader sees it on is the one before.
    let last_line = if end_position.line > start_position.line && end_position.column == 1 {
      end_position.line - 1
    } else {
      end_position.line
    };

    Region::new(
      self.text,
      Span::new(start, end),
      start_position,
      end_position,
      first_line_start,
      last_line,
    )
  }

  /// Resolves `offset` starting from a cursor already positioned at or before it.
  fn position_from(&self, cursor: Cursor, offset: usize) -> (Position, Cursor) {
    let cursor = advance(self.text.as_bytes(), cursor, offset);

    // An offset can land inside the break that ends a line: a span may name the newline itself,
    // and a character boundary sits between the `\r` and the `\n` of a CRLF. A break occupies no
    // column, so counting up to the first one puts such an offset at the end of the line's
    // content rather than at a column past the line's own width.
    let segment = &self.text[cursor.line_start..offset];
    let visible = segment.find(['\r', '\n']).unwrap_or(segment.len());
    // Counted as an ordinal from the start rather than converted from a `usize` count, so there is
    // no cast on the path a position is built by.
    let column = segment[..visible]
      .chars()
      .fold(1u64, |column, _| column + 1);
    (
      Position {
        offset,
        line: cursor.line,
        column,
      },
      cursor,
    )
  }

  /// Clamps `offset` into the text and back to the start of whatever atom it lands inside.
  fn floor(&self, offset: usize) -> usize {
    let mut offset = offset.min(self.text.len());
    while !self.text.is_char_boundary(offset) {
      offset -= 1;
    }
    if self.splits_a_crlf(offset) {
      offset -= 1;
    }
    offset
  }

  /// Clamps `offset` into the text and on past whatever atom it lands inside.
  fn ceil(&self, offset: usize) -> usize {
    let mut offset = offset.min(self.text.len());
    while !self.text.is_char_boundary(offset) {
      offset += 1;
    }
    if self.splits_a_crlf(offset) {
      offset += 1;
    }
    offset
  }

  /// Returns whether `offset` sits between the two bytes of one CRLF.
  ///
  /// A `\r\n` is one line break, so an offset inside it names half of an atom the same way an
  /// offset inside a multi-byte character does — and it is a character boundary, so nothing else
  /// here would catch it. Widening past it keeps every position on a line whose content actually
  /// reaches it, which is what lets a resolved region's coverage stay inside the region.
  #[inline]
  fn splits_a_crlf(&self, offset: usize) -> bool {
    let bytes = self.text.as_bytes();
    offset > 0 && bytes[offset - 1] == b'\r' && matches!(bytes.get(offset), Some(b'\n'))
  }
}
