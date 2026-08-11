//! Layer 2: turning byte offsets into something a reader can be pointed at.
//!
//! Private, and re-exported flat from the crate root. Whatever a reader needs to know is on
//! [`Source`] and on the types it hands back.

mod line;
mod region;

pub use line::{Line, LineBreak, Lines};
pub use region::{Region, RegionLine, RegionLines};

/// The clipping rule, for the renderer that walks a region's lines itself instead of asking
/// [`Region::lines`] for them.
///
/// One rule with two consumers rather than two rules, which is the whole reason it is a function —
/// see its own note.
#[cfg(feature = "html")]
pub(crate) use region::clip;

use crate::Span;

#[cfg(test)]
mod tests;

/// A byte offset with the line and column it lands on.
///
/// Columns count **characters**, from 1. Not bytes, which no reader can see, and not display
/// cells, which need a Unicode width table and a medium to be a width *of* — the renderer that has
/// one converts, and that is what keeps this layer dependency-free.
///
/// # Two units, placed by [the numeric widths](crate#numeric-widths)
///
/// [`line`](Self::line) and [`column`](Self::column) are ordinals in the resolved model, so rule 1
/// makes them `u64`. [`offset`](Self::offset) is an index into the caller's `&str`, so rule 4
/// makes it a `usize` — the type Rust slices with, exact, and lossless to widen for an FFI export
/// from every `usize` Rust has.
///
/// The ordinals are worth one more sentence, because this is the type the rule was written for. A
/// position is what every renderer and every FFI consumer touches, so its width is unchangeable
/// once published; `u32` would be a bet that no source has more than 4,294,967,295 lines, and
/// losing that bet means wrapping or clamping — a clamped ordinal is indistinguishable from a real
/// one, so it is a number that *lies* rather than one that fails.
///
/// `u64` makes the question disappear rather than move it. An ordinal is at most one more than the
/// text's length in bytes and Rust caps a single object at `isize::MAX`, so on the widest target
/// this crate builds for it cannot exceed 2^63. Nothing on this path clamps and no arithmetic here
/// can overflow — which `tests/numeric_widths.rs` asserts against the source rather than leaving
/// to this paragraph.
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

/// How far a walk over the text has got, what line it is on, and which column of it.
#[derive(Debug, Clone, Copy)]
struct Cursor {
  offset: usize,
  line: u64,
  line_start: usize,
  column: u64,
}

impl Cursor {
  const START: Self = Self {
    offset: 0,
    line: 1,
    line_start: 0,
    column: 1,
  };
}

/// Whether `byte` continues a character rather than starting one.
///
/// UTF-8 continuation bytes are `10xxxxxx` and nothing else is, so a character can be counted from
/// its first byte without decoding it.
#[inline]
const fn continues_a_character(byte: u8) -> bool {
  byte & 0b1100_0000 == 0b1000_0000
}

/// Walks `bytes` from `cursor` to `target`, counting the line breaks and the characters in between.
///
/// `target` must be at or after `cursor.offset`; the walk is forward-only, which is the whole
/// reason resolving in offset order costs one pass rather than one pass per offset.
///
/// # The column is counted here, not scanned for afterwards
///
/// It used to be read off `text[line_start..offset]` once the walk had arrived, which is a SECOND
/// pass over the same bytes and — the part that mattered — a pass that starts over for every
/// offset. Carrying the walk between k positions on one line therefore bought nothing: the walk
/// advanced twenty bytes and the column re-counted eight megabytes, k times. One counter here
/// makes the carried cursor mean what it says.
///
/// The line counter is incremented rather than saturated, and that is the point: it is a `u64`
/// bounded by the length of a `&str`, so it cannot reach its maximum, and if that reasoning were
/// ever wrong a debug build would panic here instead of handing back a number that quietly stopped
/// being true. The column is bounded the same way, by the length of one line.
fn advance(bytes: &[u8], cursor: Cursor, target: usize) -> Cursor {
  advance_to(bytes, cursor, target, false)
}

/// The same walk, stopping **on** a line break that ends exactly at `target` rather than crossing
/// it.
///
/// That is the difference between where an offset IS and the last line a region reaching it is
/// DRAWN on, and it is [`Region::lines`]'s rule rather than a second one: a span that swallowed its
/// own trailing newline has nothing of it to draw on the line after the break, so the line before
/// is the one it closes on. Stated here, where the walk can act on it, instead of being recovered
/// afterwards from a column of 1 — which needs the span's own start line to be right about, and
/// leaves the caller holding a line the walk has already gone past.
#[cfg(any(feature = "terminal", feature = "html"))]
fn advance_to_drawn_end(bytes: &[u8], cursor: Cursor, target: usize) -> Cursor {
  advance_to(bytes, cursor, target, true)
}

fn advance_to(bytes: &[u8], mut cursor: Cursor, target: usize, stop_on_a_break: bool) -> Cursor {
  let mut index = cursor.offset;
  let mut reached = target;
  while index < target {
    match line::break_at(bytes, index) {
      Some(line_break) => {
        let after = index + line_break.byte_len();
        if after > target {
          // A half-crossed break would leave `line_start` past `target`, and every caller here
          // slices between the two. `Source::floor` and `Source::ceil` move an offset out of a
          // CRLF before it ever gets here, so this is what makes the walk total rather than a case
          // that arises.
          //
          // Stopping here is also what puts an offset inside a CRLF at the END of the line's
          // content rather than a column past its width: a break occupies no column, and the
          // counter has not reached it.
          break;
        }
        if stop_on_a_break && after == target {
          // Stopped BEFORE the break, so the cursor keeps the line the break ended and the column
          // one past its last character. `reached` is where the walk actually got to rather than
          // where it was sent, because a cursor that claims an offset it is not standing on is the
          // one thing a carried walk must never hand on.
          reached = index;
          break;
        }
        index = after;
        cursor.line += 1;
        cursor.line_start = index;
        cursor.column = 1;
      }
      None => {
        if !matches!(bytes.get(index), Some(byte) if continues_a_character(*byte)) {
          cursor.column += 1;
        }
        index += 1;
      }
    }
  }
  cursor.offset = reached;
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

  /// Returns the line after `line`, or `None` when it ended the text.
  ///
  /// Linear in the line it returns rather than in the offset it starts at, which is what makes it
  /// the way to reach a line a renderer is already standing next to: [`line`](Self::line) and
  /// [`line_at`](Self::line_at) both scan from the top.
  ///
  /// Agrees with [`Lines`] on the empty line a trailing break leaves behind, because it is the same
  /// scan: `"a\n"` has a second line and this returns it.
  ///
  /// Gated on the outputs that have a use for it, exactly as [`Walk`] is: a renderer drawing the
  /// lines BETWEEN a multi-line span's ends walks from one to the next, and nothing else here does.
  /// Both text renderers draw them, so both are on the gate.
  #[cfg(any(feature = "terminal", feature = "html"))]
  pub(crate) fn line_after(&self, line: Line<'a>) -> Option<Line<'a>> {
    let line_break = line.line_break()?;
    let start = line.span().end() + line_break.byte_len();
    Some(line::scan(self.text, start, line.number() + 1))
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
  /// - an end before its own start is taken to that start — **before** either end is moved to an
  ///   atom boundary, so an inverted span resolves exactly where the empty span at its start would,
  ///   and never to less than that;
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
    // ORDER MATTERS, and getting it wrong erased the character the caller asked about.
    //
    // The inversion is repaired against the REQUESTED start, before either end is moved to an atom
    // boundary. Repairing it against the *floored* start instead lets the atom clamp run first and
    // hand the repair a smaller number to catch up to: in `"🎨"`, `Span::new(3, 0)` floors the
    // start to 0, the end then repairs to 0 rather than to 3, and the glyph the span pointed into
    // resolves to an empty range — while `Span::empty(3)`, which asks a strictly weaker question,
    // correctly resolves to the whole glyph.
    //
    // Repairing first cannot invert the result. `requested_end >= requested_start`; `floor` only
    // moves back and `ceil` only moves forward, and where either clamps to the text's length it
    // clamps to the same length. A non-inverted span is unaffected either way, since its end
    // already dominates both its own start and that start's floor.
    let clamped = self.clamped(span);
    let (start, end) = (clamped.start(), clamped.end());

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

  /// The byte range `span` resolves to: the inversion repaired first, then each end moved out to
  /// its atom.
  ///
  /// Shared by [`resolve`](Self::resolve) and [`Walk`] rather than written twice. The order the
  /// three steps run in is the subject of the comment in `resolve`, and a second copy of it is a
  /// second place for that order to be got wrong.
  fn clamped(&self, span: Span) -> Span {
    let requested_end = span.end().max(span.start());
    Span::new(self.floor(span.start()), self.ceil(requested_end))
  }

  /// Resolves `offset` starting from a cursor already positioned at or before it.
  ///
  /// Everything a position says is read off the walk that got here — line, column and all —
  /// because the walk is the only thing that has been over the bytes. Counted as an ordinal
  /// throughout rather than converted from a `usize` count, so there is no cast on the path a
  /// position is built by.
  fn position_from(&self, cursor: Cursor, offset: usize) -> (Position, Cursor) {
    let cursor = advance(self.text.as_bytes(), cursor, offset);
    (
      Position {
        offset,
        line: cursor.line,
        column: cursor.column,
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

/// Resolves a set of spans against one text in a single forward walk.
///
/// # What it is for
///
/// [`Source::resolve`] starts at the top of the text every time, so k spans cost k walks — the
/// linearity [`Source`]'s own note about resolving "in offset order" describes but has no API for.
/// A renderer drawing k labels paid it k times, and on a multi-megabyte line each one is the whole
/// line again. This carries the walk's position between calls and keeps the [`Line`] it is standing
/// on, so a set of spans on one line costs **one** pass over the input and **one** scan for the
/// line's end however many of them there are.
///
/// # Total, not preconditioned
///
/// A span starting before where the walk has got to restarts it from the top. That is deliberately
/// not a documented requirement on the caller: a requirement is checked by whoever remembers it,
/// and the one thing that must never happen here is a *wrong* position. An unordered caller pays
/// what [`Source::resolve`] would have charged it and gets the same answer.
///
/// The answers are [`Source::resolve`]'s, and `walking_a_set_of_spans_answers_what_resolving_each_
/// one_does` holds the two together over the corpus at every offset — a carried cursor is exactly
/// the kind of state that can be right on the case it was written for and wrong one line later.
///
/// Gated on the outputs that have it: this is layer 2's capability rather than the terminal's, and
/// the gate follows the consumer so that a `--no-default-features` build does not carry code
/// nothing reaches.
///
/// **HTML arrived and wanted it, which this note predicted.** What it did not predict is the
/// reason: the terminal drains a multi-line span's far ends out of a `BinaryHeap` so that every
/// stop of a whole block is visited in one ascending order, and `html` is allocation-free — it has
/// no heap, so it opens and closes each span in turn and pays a restart wherever a later-starting
/// span ends before an earlier one. Totality is what makes that a cost rather than a defect.
#[cfg(any(feature = "terminal", feature = "html"))]
pub(crate) struct Walk<'a> {
  source: Source<'a>,
  cursor: Cursor,
  /// The line `cursor` stands on, once something has needed it. Finding where a line ENDS is
  /// linear in the line whatever the span, so it is found once per line rather than once per span.
  line: Option<Line<'a>>,
}

#[cfg(any(feature = "terminal", feature = "html"))]
impl<'a> Walk<'a> {
  /// A walk over `source`, positioned at the top of it.
  #[inline]
  pub(crate) const fn new(source: Source<'a>) -> Self {
    Self {
      source,
      cursor: Cursor::START,
      line: None,
    }
  }

  /// Returns the byte range `span` resolves to, without walking anything.
  ///
  /// [`Source::resolve`]'s clamping, offered on its own so that a caller ordering a set of stops
  /// knows where each one lands before it asks for any of them. Exposed rather than re-derived
  /// because the order the three clamping steps run in is subtle enough to have erased a character
  /// once, and a second copy of it is a second place to get it wrong.
  ///
  /// Gated on the renderer that orders its stops ahead of visiting them. The HTML renderer visits
  /// each span's two ends as it reaches it, so it never has to know where a stop lands before
  /// asking for it.
  #[cfg(feature = "terminal")]
  #[inline]
  pub(crate) fn clamped(&self, span: Span) -> Span {
    self.source.clamped(span)
  }

  /// Returns where `span` starts, the first line it is drawn on, and whether it is drawn on
  /// another.
  ///
  /// The first two are exactly
  /// `(source.resolve(span).start(), source.resolve(span).lines().next().unwrap())`, and the
  /// `unwrap` is why this returns no `Option`: a region always covers at least the line it starts
  /// on, so the first line is never absent and a caller has no case to handle.
  pub(crate) fn open(&mut self, span: Span) -> Opening<'a> {
    let clamped = self.source.clamped(span);
    if clamped.start() < self.cursor.offset {
      self.cursor = Cursor::START;
      self.line = None;
    }

    let (at, cursor) = self.source.position_from(self.cursor, clamped.start());
    self.cursor = cursor;
    let line = self.standing_on(cursor);
    // What `Region::is_multiline` says, decided from the first line alone so that a renderer can
    // ask before paying for the walk to the far end. A span reaching only as far as the break that
    // ended this line has nothing of itself to draw on the next one, which is the same rule
    // `Region::lines` applies from the other direction.
    let content = line.span().end();
    let reaches_another_line =
      clamped.end() > content + line.line_break().map_or(0, |ended| ended.byte_len());

    Opening {
      at,
      line: region::clip(line, clamped),
      span: clamped,
      reaches_another_line,
    }
  }

  /// Returns the LAST line `opening`'s span is drawn on, with the part of it that falls there.
  ///
  /// Exactly `source.resolve(span).lines().last().unwrap()`. Only meaningful for a span that
  /// [reaches another line](Opening::reaches_another_line) — for one that does not, the answer is
  /// [`Opening::line`], which the caller already holds.
  ///
  /// Forward from wherever the walk has got to, so a caller that interleaves this with
  /// [`open`](Self::open) in ascending offset order pays for one pass over the input rather than
  /// two. Total anyway: a span behind the cursor restarts the walk, on the same terms as `open`.
  pub(crate) fn close(&mut self, opening: &Opening<'a>) -> RegionLine<'a> {
    let span = opening.span;
    if span.end() < self.cursor.offset {
      self.cursor = Cursor::START;
      self.line = None;
    }

    let cursor = advance_to_drawn_end(self.source.text.as_bytes(), self.cursor, span.end());
    self.cursor = cursor;
    region::clip(self.standing_on(cursor), span)
  }

  /// The line `cursor` stands on, scanned once and then remembered.
  ///
  /// Keyed on where the line STARTS rather than on its number, because that is what the cursor
  /// carries and what `line::scan` would be handed. A number would have to agree with the walk by
  /// a second argument.
  fn standing_on(&mut self, cursor: Cursor) -> Line<'a> {
    match self.line {
      Some(line) if line.span().start() == cursor.line_start => line,
      _ => {
        let scanned = line::scan(self.source.text, cursor.line_start, cursor.line);
        self.line = Some(scanned);
        scanned
      }
    }
  }
}

/// Where a span begins, the first line it is drawn on, and whether it is drawn on any other.
///
/// The third answer is here rather than left to the caller because it decides whether the walk is
/// asked to go on to the far end at all, and a renderer deriving it for itself would be stating
/// [`Region`]'s rule about a swallowed trailing newline a second time — in the one place where
/// getting it wrong draws a bracket around a line the span does not cover.
#[cfg(any(feature = "terminal", feature = "html"))]
#[derive(Debug, Clone, Copy)]
pub(crate) struct Opening<'a> {
  at: Position,
  line: RegionLine<'a>,
  span: Span,
  reaches_another_line: bool,
}

#[cfg(any(feature = "terminal", feature = "html"))]
impl<'a> Opening<'a> {
  /// Returns where the span begins.
  #[inline]
  pub(crate) const fn at(&self) -> Position {
    self.at
  }

  /// Returns the first line the span is drawn on, with the part of it that falls there.
  #[inline]
  pub(crate) const fn line(&self) -> RegionLine<'a> {
    self.line
  }

  /// Returns the byte range the span resolved to.
  #[inline]
  pub(crate) const fn span(&self) -> Span {
    self.span
  }

  /// Returns whether the span is drawn on a line after [`line`](Self::line).
  #[inline]
  pub(crate) const fn reaches_another_line(&self) -> bool {
    self.reaches_another_line
  }
}
