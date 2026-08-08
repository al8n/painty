use super::{LineBreak, Source};
use crate::Span;

/// The line's number, its text, and what ended it.
fn shape(line: crate::Line<'_>) -> (u64, &str, Option<LineBreak>) {
  (line.number(), line.text(), line.line_break())
}

#[test]
fn empty_source_is_one_empty_line() {
  let source = Source::new("");
  assert!(source.is_empty());
  assert_eq!(source.line_count(), 1);

  let line = source.line(1).unwrap();
  assert_eq!(shape(line), (1, "", None));
  assert_eq!(source.line(2), None);

  let at = source.position(0);
  assert_eq!((at.line(), at.column(), at.offset()), (1, 1, 0));
}

#[test]
fn a_source_without_a_trailing_break_ends_on_its_last_line() {
  let source = Source::new("one\ntwo");
  assert_eq!(source.line_count(), 2);
  assert_eq!(
    shape(source.line(1).unwrap()),
    (1, "one", Some(LineBreak::Lf))
  );
  assert_eq!(shape(source.line(2).unwrap()), (2, "two", None));

  // The offset one past the final byte is still on line 2 — there is no line 3 to put it on.
  let end = source.position(7);
  assert_eq!((end.line(), end.column()), (2, 4));
}

#[test]
fn a_trailing_break_adds_the_empty_line_after_it() {
  let source = Source::new("one\n");
  assert_eq!(source.line_count(), 2);
  assert_eq!(shape(source.line(2).unwrap()), (2, "", None));

  // This is the divergence from `str::lines`, and it is the whole reason for it: an offset at the
  // end of a file that ends in a newline has somewhere to be.
  let end = source.position(4);
  assert_eq!((end.line(), end.column()), (2, 1));
}

#[test]
fn all_three_break_kinds_are_recognised() {
  let source = Source::new("lf\ncrlf\r\ncr\rlast");
  assert_eq!(source.line_count(), 4);
  assert_eq!(
    shape(source.line(1).unwrap()),
    (1, "lf", Some(LineBreak::Lf))
  );
  assert_eq!(
    shape(source.line(2).unwrap()),
    (2, "crlf", Some(LineBreak::CrLf))
  );
  assert_eq!(
    shape(source.line(3).unwrap()),
    (3, "cr", Some(LineBreak::Cr))
  );
  assert_eq!(shape(source.line(4).unwrap()), (4, "last", None));
}

#[test]
fn a_lone_carriage_return_is_a_break_where_str_lines_says_it_is_not() {
  let source = Source::new("first\rsecond");
  assert_eq!(source.line_count(), 2);
  assert_eq!("first\rsecond".lines().count(), 1);

  let at = source.position(6);
  assert_eq!((at.line(), at.column()), (2, 1));
}

#[test]
fn an_offset_between_the_two_bytes_of_a_crlf_is_moved_out_of_it() {
  let source = Source::new("one\r\ntwo");

  // Offset 4 is the `\n`. It is a character boundary, so a producer can name it, but a CRLF is one
  // break and half of one is not a place — so it is treated like half a character and moved to the
  // atom's edge.
  let split = source.position(4);
  assert_eq!(split.offset(), 3);
  assert_eq!((split.line(), split.column()), (1, 4));

  // Which is the same answer the `\r` itself gets: end of line 1, one past its three characters.
  assert_eq!(source.position(3), split);

  let after = source.position(5);
  assert_eq!((after.line(), after.column()), (2, 1));

  // A span reaching into the break widens past the whole of it rather than splitting it.
  assert_eq!(source.resolve(Span::new(0, 4)).span(), Span::new(0, 5));
  assert_eq!(source.resolve(Span::new(4, 8)).span(), Span::new(3, 8));
}

#[test]
fn columns_count_characters_not_bytes() {
  let source = Source::new("日本語x");

  assert_eq!(source.position(0).column(), 1);
  assert_eq!(source.position(3).column(), 2);
  assert_eq!(source.position(9).column(), 4);
  assert_eq!(source.line(1).unwrap().char_count(), 4);
}

#[test]
fn an_offset_inside_a_character_resolves_to_that_character() {
  let source = Source::new("日本");

  // Bytes 1 and 2 are the tail of the first character.
  assert_eq!(source.position(1).offset(), 0);
  assert_eq!(source.position(2).offset(), 0);
  assert_eq!(source.position(3).offset(), 3);
}

#[test]
fn resolution_clamps_a_span_that_the_text_cannot_express() {
  let source = Source::new("héllo");

  // Past the end.
  assert_eq!(source.resolve(Span::new(99, 200)).span(), Span::new(6, 6));
  // Inverted: the end collapses onto the start rather than staying behind it.
  assert_eq!(source.resolve(Span::new(4, 1)).span(), Span::new(4, 4));
  // Both ends inside the two-byte `é`, so both move outwards.
  let region = source.resolve(Span::new(2, 2));
  assert_eq!(region.span(), Span::new(1, 3));
  assert_eq!(region.text(), "é");
}

#[test]
fn an_inverted_span_resolves_where_its_own_empty_span_would() {
  // The regression. Repairing the inversion after clamping the start to an atom let the repaired
  // end land BEFORE the requested start, so the character the span pointed into vanished. An
  // inverted span asks a weaker question than the empty span at its start, and must never get a
  // narrower answer than that one.
  let emoji = Source::new("🎨");
  let whole = emoji.resolve(Span::empty(3)).span();
  assert_eq!(whole, Span::new(0, 4));
  for inverted in [Span::new(3, 0), Span::new(3, 1), Span::new(3, 2)] {
    assert_eq!(emoji.resolve(inverted).span(), whole, "{inverted:?}");
    assert_eq!(emoji.resolve(inverted).text(), "🎨", "{inverted:?}");
  }

  // The same on the other atom, where the split is a character boundary and only the CRLF rule
  // catches it.
  let crlf = Source::new("a\r\nb");
  assert_eq!(crlf.resolve(Span::empty(2)).span(), Span::new(1, 3));
  assert_eq!(crlf.resolve(Span::new(2, 0)).span(), Span::new(1, 3));
  assert_eq!(crlf.resolve(Span::new(2, 1)).span(), Span::new(1, 3));

  // And the mirror: an inverted span whose END is mid-atom. The end is taken to the start, so the
  // end's own position stops mattering — what is left is the empty span at the start.
  assert_eq!(emoji.resolve(Span::new(4, 1)).span(), Span::empty(4));
  assert_eq!(emoji.resolve(Span::new(0, 2)).span(), Span::new(0, 4));
  assert_eq!(crlf.resolve(Span::new(3, 2)).span(), Span::empty(3));
}

#[test]
fn a_span_covering_the_final_byte_of_a_source_with_no_trailing_break() {
  let source = Source::new("one\ntwo");
  let region = source.resolve(Span::new(6, 7));

  assert_eq!(region.text(), "o");
  assert_eq!(region.line_count(), 1);
  assert!(!region.is_multiline());
  assert_eq!((region.start().line(), region.start().column()), (2, 3));
  assert_eq!((region.end().line(), region.end().column()), (2, 4));
}

#[test]
fn a_multiline_region_covers_the_lines_it_is_drawn_on() {
  let source = Source::new("alpha\nbeta\ngamma\n");
  let region = source.resolve(Span::new(3, 13));

  assert!(region.is_multiline());
  assert_eq!(region.line_count(), 3);

  let mut lines = region.lines();

  let first = lines.next().unwrap();
  assert_eq!(first.line().number(), 1);
  assert_eq!(first.covered_text(), "ha");
  assert_eq!(first.columns(), 4..6);

  let second = lines.next().unwrap();
  assert_eq!(second.line().number(), 2);
  assert_eq!(second.covered_text(), "beta");
  assert_eq!(second.columns(), 1..5);

  let third = lines.next().unwrap();
  assert_eq!(third.line().number(), 3);
  assert_eq!(third.covered_text(), "ga");
  assert_eq!(third.columns(), 1..3);

  assert!(lines.next().is_none());
}

#[test]
fn a_region_that_swallows_its_own_newline_is_not_drawn_on_the_next_line() {
  let source = Source::new("alpha\nbeta\n");
  let region = source.resolve(Span::new(0, 6));

  // The exclusive end really is on line 2 — that is where the offset is.
  assert_eq!((region.end().line(), region.end().column()), (2, 1));
  // But there is nothing of the region to draw there.
  assert_eq!(region.line_count(), 1);
  assert!(!region.is_multiline());

  let drawn = region.lines().next().unwrap();
  assert_eq!(drawn.line().number(), 1);
  assert_eq!(drawn.covered_text(), "alpha");
}

#[test]
fn a_region_that_is_only_a_newline_covers_nothing_visible() {
  let source = Source::new("alpha\nbeta");
  let region = source.resolve(Span::new(5, 6));

  assert_eq!(region.line_count(), 1);
  let drawn = region.lines().next().unwrap();
  assert_eq!(drawn.line().number(), 1);
  assert!(drawn.covered().is_empty());
  assert_eq!(drawn.covered_text(), "");
  assert_eq!(drawn.columns(), 6..6);
}

#[test]
fn an_empty_span_is_a_caret_on_one_line() {
  let source = Source::new("alpha\nbeta\n");
  let region = source.resolve(Span::empty(8));

  assert_eq!(region.line_count(), 1);
  assert_eq!(region.text(), "");
  assert_eq!((region.start().line(), region.start().column()), (2, 3));
  assert_eq!(region.start(), region.end());

  let drawn = region.lines().next().unwrap();
  assert_eq!(drawn.columns(), 3..3);
}

#[test]
fn line_at_and_position_agree_on_every_offset() {
  let source = Source::new("one\r\ntwo\rthree\nfour");

  for offset in 0..=source.len() {
    if !source.text().is_char_boundary(offset) {
      continue;
    }
    let position = source.position(offset);
    let line = source.line_at(offset);
    assert_eq!(line.number(), position.line(), "at offset {offset}");
    assert_eq!(
      line.column_at(offset),
      position.column(),
      "at offset {offset}"
    );
  }
}

#[test]
fn the_first_position_of_every_line_is_column_one() {
  let source = Source::new("one\r\ntwo\rthree\nfour\n");

  for line in source.lines() {
    let at = source.position(line.span().start());
    assert_eq!((at.line(), at.column()), (line.number(), 1));
  }
}

#[test]
fn lines_is_fused_and_terminates() {
  let source = Source::new("one\ntwo\n");
  let mut lines = source.lines();

  assert_eq!(lines.by_ref().count(), 3);
  assert!(lines.next().is_none());
  assert!(lines.next().is_none());
}
