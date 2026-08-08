//! The invariants of resolution, asserted over generated inputs.
//!
//! # Why these are properties and not golden files
//!
//! Layout code is traditionally tested by snapshot, and a snapshot of layout is fragile: it fails
//! on cosmetic change, it gets re-blessed reflexively, and a re-blessed snapshot records a bug as
//! the new truth. Splitting resolution out of the renderers is what makes the hard part assertable
//! *before* anything is stringified — an excerpt re-sliced by the offsets it reported equals the
//! text it came from, or it does not, and no amount of re-blessing can make it agree.
//!
//! # The oracle is deliberately a different implementation
//!
//! [`oracle`] answers "which line and column is this offset on" by walking characters and
//! accumulating; painty answers it by walking bytes and seeking. Two implementations of the same
//! function are worth having only if they are actually different, because a property test whose
//! oracle shares the subject's mistake proves nothing.
//!
//! # And the generator is deterministic
//!
//! A fixed seed, not a random one. A property failure that cannot be reproduced from the test's
//! own source is a bug report nobody can act on, and this suite has no dependency to shrink a
//! counterexample with — so the counterexample has to be the same one every run.

use painty::{Label, Location, Source, Span};

/// xorshift64. Small, deterministic, and enough to shuffle a corpus.
struct Rng(u64);

impl Rng {
  fn next(&mut self) -> u64 {
    let mut state = self.0;
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    self.0 = state;
    state
  }

  fn below(&mut self, bound: usize) -> usize {
    (self.next() % bound as u64) as usize
  }
}

/// The pieces a generated source is built from.
///
/// Every one of them is here because it has broken a text renderer somewhere: all three line
/// breaks, a break pair that could be read as one or two, multi-byte and astral characters, a
/// combining mark whose character count and grapheme count disagree, and a tab.
const PIECES: &[&str] = &[
  "a", "bc", "word", "  ", "\t", "\n", "\r\n", "\r", "\n\r", "\r\r", "é", "日本", "🎨", "e\u{301}",
  "",
];

fn generate(rng: &mut Rng) -> String {
  let count = rng.below(14);
  let mut out = String::new();
  for _ in 0..count {
    out.push_str(PIECES[rng.below(PIECES.len())]);
  }
  out
}

/// An independent answer to "which 1-based line and character column is `offset` on".
///
/// Walks characters and accumulates, where painty walks bytes and seeks. `offset` must be an
/// offset [`floor`] or [`ceil`] produced: at or before the end of `text`, on a character boundary,
/// and not between the two bytes of a CRLF.
fn oracle(text: &str, offset: usize) -> (u64, u64) {
  let mut line = 1u64;
  let mut column = 1u64;
  let mut characters = text.char_indices().peekable();

  while let Some((index, character)) = characters.next() {
    if index >= offset {
      break;
    }
    match character {
      '\n' => {
        line += 1;
        column = 1;
      }
      '\r' => {
        if matches!(characters.peek(), Some((_, '\n'))) {
          characters.next();
        }
        line += 1;
        column = 1;
      }
      _ => column += 1,
    }
  }

  (line, column)
}

/// Whether `offset` sits between the two bytes of one CRLF, which is one atom with a character
/// boundary in the middle of it.
fn splits_a_crlf(text: &str, offset: usize) -> bool {
  let bytes = text.as_bytes();
  offset > 0 && bytes[offset - 1] == b'\r' && matches!(bytes.get(offset), Some(b'\n'))
}

fn floor(text: &str, offset: usize) -> usize {
  let mut offset = offset.min(text.len());
  while !text.is_char_boundary(offset) {
    offset -= 1;
  }
  if splits_a_crlf(text, offset) {
    offset -= 1;
  }
  offset
}

fn ceil(text: &str, offset: usize) -> usize {
  let mut offset = offset.min(text.len());
  while !text.is_char_boundary(offset) {
    offset += 1;
  }
  if splits_a_crlf(text, offset) {
    offset += 1;
  }
  offset
}

fn is_break_byte(byte: u8) -> bool {
  byte == b'\n' || byte == b'\r'
}

/// Every claim [`Source::position`] and [`Source::line_at`] make about one offset.
fn check_position(text: &str, offset: usize) {
  let source = Source::new(text);
  let at = source.position(offset);
  let expected = floor(text, offset);

  assert_eq!(at.offset(), expected, "{text:?} @ {offset}: clamped offset");
  assert_eq!(
    (at.line(), at.column()),
    oracle(text, expected),
    "{text:?} @ {offset}: line and column"
  );

  let line = source.line_at(offset);
  assert_eq!(line.number(), at.line(), "{text:?} @ {offset}: line number");
  assert_eq!(
    source.line(at.line()),
    Some(line),
    "{text:?} @ {offset}: line lookup by number"
  );
  assert!(
    at.column() <= line.char_count() + 1,
    "{text:?} @ {offset}: column {} past the line's {} characters",
    at.column(),
    line.char_count()
  );
  assert_eq!(
    line.column_at(expected),
    at.column(),
    "{text:?} @ {offset}: the line and the source disagree on the column"
  );
}

/// Every claim [`Source::resolve`] makes about one span.
fn check_region(text: &str, requested: Span) {
  let source = Source::new(text);
  let region = source.resolve(requested);
  let span = region.span();
  let context = format!("{text:?} @ {requested:?}");

  // Clamping put both ends somewhere this text can express, in the documented direction.
  assert!(span.end() <= text.len(), "{context}: past the end");
  assert!(
    text.is_char_boundary(span.start()),
    "{context}: start split"
  );
  assert!(text.is_char_boundary(span.end()), "{context}: end split");
  assert_eq!(
    span.start(),
    floor(text, requested.start()),
    "{context}: start"
  );
  // `requested.start()`, not `span.start()`. Keying the repair off the RESOLVED start is exactly
  // the production defect this suite failed to catch for four rounds: the assertion was derived
  // from the implementation rather than from the rule, so the two agreed and the generator's
  // inverted spans over `"🎨"` and `"\r\n"` — which it did produce — passed.
  assert_eq!(
    span.end(),
    ceil(text, requested.end().max(requested.start())),
    "{context}: end"
  );

  // Stated as a property in its own right, because the equation above can only ever agree with
  // whatever it is written to expect: an inverted span asks a weaker question than the empty span
  // at its start, so it must never resolve to less.
  let degenerate = source.resolve(Span::empty(requested.start()));
  assert!(
    span.start() <= degenerate.span().start() && span.end() >= degenerate.span().end(),
    "{context}: resolved to less than the empty span at its own start, {:?}",
    degenerate.span()
  );

  // The excerpt is the source re-sliced by the offsets the region reported.
  assert_eq!(region.text(), &text[span.start()..span.end()], "{context}");
  assert_eq!(
    region.start().offset(),
    span.start(),
    "{context}: start offset"
  );
  assert_eq!(region.end().offset(), span.end(), "{context}: end offset");
  assert_eq!(
    (region.start().line(), region.start().column()),
    oracle(text, span.start()),
    "{context}: start position"
  );
  assert_eq!(
    (region.end().line(), region.end().column()),
    oracle(text, span.end()),
    "{context}: end position"
  );

  // The lines it is drawn on are consecutive, start where it starts, and are as many as it says.
  let drawn: Vec<_> = region.lines().collect();
  assert_eq!(
    drawn.len(),
    region.line_count() as usize,
    "{context}: line_count"
  );
  assert_eq!(
    region.is_multiline(),
    drawn.len() > 1,
    "{context}: multiline"
  );
  assert!(!drawn.is_empty(), "{context}: a region is drawn somewhere");
  for (step, line) in drawn.iter().enumerate() {
    assert_eq!(
      line.line().number(),
      region.start().line() + step as u64,
      "{context}: consecutive lines"
    );
  }

  // And they stop on the line holding the region's LAST BYTE — not the line its exclusive end
  // reaches, which for a region that swallowed its own trailing newline is the one after.
  //
  // The tiling check below is blind to that difference on its own: an extra trailing line covers
  // nothing, so adding it breaks no coverage rule and leaves every byte still accounted for. Not
  // hypothetical — the defect was planted, and until this assertion existed only the hand-written
  // cases reddened.
  let expected_last_line = if span.is_empty() {
    region.start().line()
  } else {
    oracle(text, floor(text, span.end() - 1)).0
  };
  assert_eq!(
    drawn
      .last()
      .expect("a region is drawn somewhere")
      .line()
      .number(),
    expected_last_line,
    "{context}: last drawn line"
  );

  // Every byte of the region is covered by exactly one of those lines, unless it is part of a line
  // break — which is the whole of what "the lines a region is drawn on" means.
  let mut covered = vec![false; span.len()];
  for line in &drawn {
    let content = line.line().span();
    assert!(
      line.covered().start() >= content.start() && line.covered().end() <= content.end(),
      "{context}: coverage escaped its line"
    );
    assert!(
      line.covered().start() >= span.start() && line.covered().end() <= span.end(),
      "{context}: coverage escaped the region"
    );
    assert_eq!(
      line.covered_text(),
      &text[line.covered().start()..line.covered().end()],
      "{context}: covered text"
    );
    assert_eq!(
      line.columns(),
      line.line().column_at(line.covered().start())..line.line().column_at(line.covered().end()),
      "{context}: columns"
    );
    assert!(
      line.columns().end <= line.line().char_count() + 1,
      "{context}: a column past the line's width"
    );

    for offset in line.covered().start()..line.covered().end() {
      assert!(!covered[offset - span.start()], "{context}: covered twice");
      covered[offset - span.start()] = true;
    }
  }
  for (index, was_covered) in covered.iter().enumerate() {
    let offset = span.start() + index;
    assert_eq!(
      *was_covered,
      !is_break_byte(text.as_bytes()[offset]),
      "{context}: byte {offset} is {}covered",
      if *was_covered { "" } else { "un" }
    );
  }
}

#[test]
fn positions_agree_with_an_independent_walk_over_generated_sources() {
  let mut rng = Rng(0x5EED_1234_ABCD_0001);
  for _ in 0..400 {
    let text = generate(&mut rng);
    for offset in 0..=text.len() + 2 {
      check_position(&text, offset);
    }
  }
}

#[test]
fn regions_tile_the_span_they_resolved_over_generated_sources() {
  let mut rng = Rng(0x5EED_1234_ABCD_0002);
  for _ in 0..400 {
    let text = generate(&mut rng);
    let bound = text.len() + 3;
    for _ in 0..24 {
      let start = rng.below(bound);
      let end = rng.below(bound);
      check_region(&text, Span::new(start.min(end), start.max(end)));
    }
    // The ends are where the arithmetic goes wrong, so they are visited every time rather than
    // being left to the generator to stumble on.
    check_region(&text, Span::new(0, text.len()));
    check_region(&text, Span::empty(text.len()));
    check_region(&text, Span::empty(0));
  }
}

#[test]
fn resolution_never_panics_on_an_inverted_or_out_of_range_span() {
  let mut rng = Rng(0x5EED_1234_ABCD_0003);
  for _ in 0..200 {
    let text = generate(&mut rng);
    let source = Source::new(&text);

    // The far end of the address space, which no text can express.
    let far = source.resolve(Span::new(usize::MAX - 1, usize::MAX));
    assert_eq!(far.span(), Span::empty(text.len()));
    assert_eq!(far.text(), "");
    assert_eq!(far.lines().count(), 1);

    for _ in 0..16 {
      let low = rng.below(text.len() + 8);
      let high = rng.below(text.len() + 8);

      // Inverted on purpose: the end is before the start, which is a producer defect painty is
      // required to survive rather than diagnose.
      let requested = Span::new(low.max(high), low.min(high));
      let inverted = source.resolve(requested);
      assert!(
        inverted.span().end() >= inverted.span().start(),
        "an inverted span stayed inverted"
      );
      check_region(&text, requested);
    }
  }
}

#[test]
fn ordering_is_a_permutation_that_a_forward_walk_can_follow() {
  let mut rng = Rng(0x5EED_1234_ABCD_0004);
  const TEXTS: [&str; 4] = ["alpha", "beta", "gamma", "delta"];

  for _ in 0..300 {
    let count = rng.below(9);
    let mut labels: Vec<Label<'static>> = (0..count)
      .map(|_| {
        let source = rng.below(3) as u32;
        let text = TEXTS[rng.below(TEXTS.len())];
        if rng.below(6) == 0 {
          Label::new(Location::entire(source), text)
        } else {
          let start = rng.below(40);
          let end = start + rng.below(12);
          Label::new(Location::new(source, Span::new(start, end)), text)
        }
      })
      .collect();

    let mut original = labels.clone();
    Label::order(&mut labels);

    // A permutation: same multiset in, same multiset out.
    original.sort_unstable_by_key(|label| {
      (
        label.location().source(),
        label
          .location()
          .span()
          .map(|span| (span.start(), span.end())),
        label.text(),
      )
    });
    let mut sorted_result = labels.clone();
    sorted_result.sort_unstable_by_key(|label| {
      (
        label.location().source(),
        label
          .location()
          .span()
          .map(|span| (span.start(), span.end())),
        label.text(),
      )
    });
    assert_eq!(original, sorted_result, "ordering lost or invented a label");

    // A forward walk can follow it: within one input, starts never go backwards, an unpositioned
    // label never follows a positioned one, and an enclosing range never follows what it encloses.
    for pair in labels.windows(2) {
      let (left, right) = (pair[0].location(), pair[1].location());
      if left.source() != right.source() {
        assert!(left.source() < right.source(), "inputs out of order");
        continue;
      }
      match (left.span(), right.span()) {
        (None, _) => {}
        (Some(_), None) => panic!("an unpositioned label followed a positioned one"),
        (Some(before), Some(after)) => {
          assert!(before.start() <= after.start(), "starts went backwards");
          if before.start() == after.start() {
            assert!(before.end() >= after.end(), "a nested range came first");
          }
        }
      }
    }

    // Deterministic: ordering an already-ordered slice changes nothing, and ordering a reshuffled
    // copy of the same labels reaches the same answer.
    let stable = {
      let mut again = labels.clone();
      Label::order(&mut again);
      again
    };
    assert_eq!(labels, stable, "ordering is not idempotent");
  }
}
