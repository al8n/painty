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

/// Texts a carried cursor can be wrong about, each for a reason a single-span walk never meets.
#[cfg(feature = "terminal")]
const WALKED: [(&str, &str); 8] = [
  (
    "",
    "an empty text, where every span resolves to the same place",
  ),
  (
    "one\ntwo\nthree\n",
    "the ordinary case, three lines and a trailing break",
  ),
  (
    "one\r\ntwo\r\n",
    "CRLF — two bytes for one break, and a character boundary in the middle of it",
  ),
  (
    "one\rtwo\rthree",
    "lone CR, which `str::lines` does not break on",
  ),
  (
    "\n\n\na\n",
    "empty lines, where a line's start and end coincide and a cursor can sit on either",
  ),
  (
    "héllo\nwörld\n",
    "multi-byte characters, so `floor` and `ceil` have somewhere to move an end to",
  ),
  (
    "🎨a\n日本語\n",
    "astral and wide characters, whose interior offsets are not character boundaries",
  ),
  (
    "a\u{301}\r\n\u{301}b",
    "a combining mark either side of a CRLF, and a defective one starting a line",
  ),
];

#[test]
#[cfg(feature = "terminal")]
fn walking_a_set_of_spans_answers_what_resolving_each_one_does() {
  // The walk carries a cursor and remembers the line it is standing on, and both are state that a
  // one-shot `resolve` does not have — so what has to be checked is not that the walk is plausible
  // but that it is the SAME FUNCTION. Every span of every text, forwards, so a memo that is right
  // for the offset it was taken at and stale one line later has nowhere to hide.
  for (text, why) in WALKED {
    let source = Source::new(text);
    let mut walk = super::Walk::new(source);

    for start in 0..=text.len() {
      for end in start..=text.len() {
        let span = Span::new(start, end);
        let opening = walk.open(span);

        let region = source.resolve(span);
        assert_eq!(
          opening.at(),
          region.start(),
          "{text:?} {start}..{end}: {why}"
        );
        assert_eq!(
          Some(opening.line()),
          region.lines().next(),
          "{text:?} {start}..{end}: {why}"
        );
        assert_eq!(
          opening.reaches_another_line(),
          region.is_multiline(),
          "{text:?} {start}..{end}: {why}"
        );
        // The far end, and the reason it is checked against `lines().last()` rather than against
        // `end()`: a span that swallowed its own trailing newline ENDS on a line it is not drawn
        // on, and the closing bracket of a multi-line span goes on the line it is drawn on.
        if opening.reaches_another_line() {
          assert_eq!(
            walk.close(&opening),
            region.lines().last().expect("a region covers a line"),
            "{text:?} {start}..{end}: {why}"
          );
        }
      }
    }
  }
}

#[test]
#[cfg(feature = "terminal")]
fn a_walk_handed_a_span_behind_it_restarts_rather_than_answering_from_where_it_is() {
  // The one thing a carried cursor must not do. `advance` is forward-only — it walks `while index <
  // target` and then ASSIGNS the target — so a cursor asked to go backwards would keep the line
  // number it had reached and hand back a position on a line the offset is not on. Silently wrong
  // coordinates, which is the failure this crate exists to prevent.
  //
  // Answered by making the walk total rather than by documenting an order for callers to keep: the
  // renderer sorts, and this is what holds when something else does not.
  for (text, why) in WALKED {
    let source = Source::new(text);
    let mut walk = super::Walk::new(source);

    let offsets: Vec<usize> = (0..=text.len()).rev().collect();
    for offset in offsets {
      let span = Span::empty(offset);
      let opening = walk.open(span);
      let region = source.resolve(span);
      assert_eq!(
        opening.at(),
        region.start(),
        "{text:?} @ {offset} descending: {why}"
      );
      assert_eq!(
        Some(opening.line()),
        region.lines().next(),
        "{text:?} @ {offset} descending: {why}"
      );
    }
  }
}

#[test]
#[cfg(feature = "terminal")]
fn a_walk_asked_to_close_behind_itself_restarts_too() {
  // The companion to the test above, for the other end. `close` stops ON a break that ends exactly
  // at its target, so it leaves the cursor at an offset BEFORE the one it was sent to — which is
  // the state a naive forward-only guard would then read as "already past it" and answer from.
  //
  // Descending, so every call is behind the one before it, and against `resolve` rather than
  // against a plausible-looking answer.
  for (text, why) in WALKED {
    let source = Source::new(text);
    let mut walk = super::Walk::new(source);

    let starts: Vec<usize> = (0..=text.len()).rev().collect();
    for start in starts {
      let span = Span::new(start, text.len());
      let opening = walk.open(span);
      if !opening.reaches_another_line() {
        continue;
      }
      assert_eq!(
        walk.close(&opening),
        source
          .resolve(span)
          .lines()
          .last()
          .expect("a region covers a line"),
        "{text:?} from {start} descending: {why}"
      );
    }
  }
}

#[test]
#[cfg(feature = "terminal")]
fn the_line_after_one_is_the_line_the_walk_would_have_reached() {
  // A renderer showing the lines between a multi-line span's ends steps from one to the next rather
  // than scanning from the top for each, so the step has to answer what the walk would have. Held
  // against `lines`, which is what every other consumer sees — including on the two edges that make
  // this crate part company with `str::lines`: a trailing break leaves an empty last line, and a
  // source with no trailing break ends where its text does.
  for (text, why) in WALKED {
    let source = Source::new(text);
    let walked: Vec<crate::Line<'_>> = source.lines().collect();
    for pair in walked.windows(2) {
      assert_eq!(source.line_after(pair[0]), Some(pair[1]), "{text:?}: {why}");
    }
    assert_eq!(
      source.line_after(*walked.last().expect("a source has a line")),
      None,
      "{text:?}: a line after the last one: {why}"
    );
  }
}

/// The harness is a `std` program whatever the crate under it is, so the owned strings this
/// property builds come from there rather than from `alloc`, which this crate does not enable.
#[cfg(feature = "html")]
use std::{borrow::ToOwned, string::String, vec::Vec};

/// The pieces an awkward source is built from.
///
/// The same set `tests/resolution_invariants.rs` generates from, spelled out again because these
/// two predicates are crate-private and that suite is a consumer: all three line breaks, a break
/// pair that could be read as one or two, multi-byte and astral characters, a combining mark, and
/// a tab.
#[cfg(feature = "html")]
const PIECES: [&str; 12] = [
  "a", "bc", "  ", "\t", "\n", "\r\n", "\r", "\n\r", "é", "日本", "🎨", "e\u{301}",
];

/// Every source those pieces make at up to three of them, plus the two degenerate ones.
#[cfg(feature = "html")]
fn awkward() -> impl Iterator<Item = String> {
  let singles = PIECES.into_iter().map(str::to_owned);
  let pairs = PIECES
    .into_iter()
    .flat_map(|left| PIECES.into_iter().map(move |right| alloc(left, right)));
  let triples = PIECES.into_iter().flat_map(|left| {
    PIECES
      .into_iter()
      .map(move |middle| alloc(&alloc(left, middle), "a\nb"))
  });
  ["".to_owned(), "\n".to_owned()]
    .into_iter()
    .chain(singles)
    .chain(pairs)
    .chain(triples)
}

#[cfg(feature = "html")]
fn alloc(left: &str, right: &str) -> String {
  let mut out = String::from(left);
  out.push_str(right);
  out
}

/// The predicate the HTML renderer asks of one line and the iterator the terminal takes per region
/// are one rule, and this is what says so.
///
/// [`Region::lines`] walks a region's lines by count from where it starts; [`draws_on`] answers
/// "is this span here" for a line the caller already holds, which is what a renderer visiting an
/// input's lines in one forward pass needs. Two derivations of one rule is the shape that put a
/// walk past its own far end in this file once already, so they are held together over every span
/// of every awkward source rather than by the argument that they ought to agree.
#[cfg(feature = "html")]
#[test]
fn every_line_a_region_draws_is_a_line_it_says_it_draws_on() {
  for text in awkward() {
    let source = Source::new(&text);
    for start in 0..=text.len() {
      for end in 0..=text.len() {
        let region = source.resolve(Span::new(start, end));
        let span = region.span();

        let drawn: Vec<u64> = region.lines().map(|on| on.line().number()).collect();
        let said: Vec<u64> = source
          .lines()
          .filter(|line| super::draws_on(*line, span))
          .map(|line| line.number())
          .collect();
        assert_eq!(
          drawn, said,
          "{text:?} {start}..{end} resolves to {span:?}: `Region::lines` and `draws_on` disagree"
        );

        // And exactly one of them is the line the label is said on: the last.
        let ended: Vec<u64> = source
          .lines()
          .filter(|line| super::ends_on(*line, span))
          .map(|line| line.number())
          .collect();
        let expected: Vec<u64> = drawn.last().copied().into_iter().collect();
        assert_eq!(
          ended, expected,
          "{text:?} {start}..{end} resolves to {span:?}: `ends_on` does not name the last line \
           drawn"
        );

        // The part of the line each one covers is `clip`'s, which is what `RegionLines` yields, so
        // a renderer that walks the lines itself slices the same bytes.
        for on in region.lines() {
          assert_eq!(
            super::clip(on.line(), span),
            on,
            "{text:?} {start}..{end}: clipping line {} by hand differs",
            on.line().number()
          );
        }
      }
    }
  }
}
