//! What `Terminal::render` promises about HOW it writes, as distinct from what it writes.
//!
//! # Why this is one harness and not five tests
//!
//! Four rounds of review found four defects in `src/terminal/render.rs`, and the interesting thing
//! about them is not that they were in one file. It is that **none of them changed the output**:
//!
//! * a row materialised into a `String` before the writer was consulted produces the same bytes as
//!   one streamed to it;
//! * a style left open by a failed write produces the same bytes on a writer that never fails;
//! * a rendered column narrowed through `usize` produces the same bytes on a 64-bit machine;
//! * a row bounded only by the input's geometry produces the same bytes for any input small enough
//!   to look at.
//!
//! So the crate's goldens, invariants and hand-written placement oracle — all of which read output
//! — could not have caught any of them, and each fix arrived with a bespoke harness of its own.
//! That is the thing this file replaces. The renderer's contract with `fmt::Write` has dimensions
//! that no output-reading test can see, they are enumerated here, and they are checked over one
//! shared table of adversarial inputs rather than over whatever input each finding happened to
//! arrive with.
//!
//! # The five dimensions
//!
//! 1. **How much it emits**, against a writer that accepts everything. Bounded by policy —
//!    `Terminal::max_rendered_width` — rather than by the input.
//! 2. **How much it allocates.** Nothing proportional to a row.
//! 3. **What it has offered when it stops.** Never an SGR opener without the reset after it.
//! 4. **What it does on a target of a different pointer width.** That one is arithmetic rather than
//!    a property of a render, so it is pinned at the helper it lives in —
//!    `padding_is_counted_in_rendered_columns_and_not_in_pointer_width`, in `src/terminal/tests.rs`
//!    — and named here so the list of dimensions stays in one place.
//! 5. **How much WORK it does.** Not implied by any of the four above, and that is how the fourth
//!    defect survived: the ceiling bounded what was emitted and said nothing about what was computed
//!    in order to emit it. A span past the window cost a walk of the whole line to place one caret
//!    under an elision mark.
//!
//! # What dimension 5 does not cover, said plainly
//!
//! A whole render cannot be bounded in work, and no ceiling can change that: layer 2 turns a byte
//! offset into a line and column by scanning to it, so finding the line is proportional to the
//! offset. On an eight-megabyte line that scan measures around 58 ms and dominates everything else.
//!
//! So what is asserted is the property that CAN hold — the renderer's own geometry costs the window
//! and not the line — and it is asserted at `Terminal::underline`, the public entry to that
//! geometry, rather than at `render`, where the scan would drown it. Claiming a bound the code
//! cannot keep is the failure this file exists to prevent.
//!
//! # Why the counting allocator lives in its own test binary
//!
//! A `#[global_allocator]` is per-binary. Keeping it here rather than beside the appearance tests
//! means the tests that read output run under the ordinary allocator, and the one thing in the
//! crate that measures allocation is the one thing affected by measuring it.

#![cfg(feature = "terminal")]

use painty::{
  Diagnostic, Location, Severity, Source, Span, Theme,
  terminal::{ColorCapability, Input, Terminal},
};

/// Bytes this thread has asked the allocator for.
///
/// Thread-local rather than a single counter, because the harness runs tests in parallel and a
/// shared one would be measuring every other test at the same time. `const`-initialised so reading
/// it cannot itself allocate and re-enter the allocator. `realloc` is counted as well as `alloc`,
/// since a `String` reaching its size mostly does it by growing.
struct Counting;

thread_local! {
  static ALLOCATED: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

fn allocated() -> usize {
  ALLOCATED.with(core::cell::Cell::get)
}

unsafe impl core::alloc::GlobalAlloc for Counting {
  unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
    let _ = ALLOCATED.try_with(|bytes| bytes.set(bytes.get() + layout.size()));
    unsafe { core::alloc::GlobalAlloc::alloc(&std::alloc::System, layout) }
  }

  unsafe fn dealloc(&self, pointer: *mut u8, layout: core::alloc::Layout) {
    unsafe { core::alloc::GlobalAlloc::dealloc(&std::alloc::System, pointer, layout) }
  }

  unsafe fn realloc(
    &self,
    pointer: *mut u8,
    layout: core::alloc::Layout,
    new_size: usize,
  ) -> *mut u8 {
    let _ = ALLOCATED.try_with(|bytes| bytes.set(bytes.get() + new_size));
    unsafe { core::alloc::GlobalAlloc::realloc(&std::alloc::System, pointer, layout, new_size) }
  }
}

#[global_allocator]
static COUNTING: Counting = Counting;

/// Accepts everything and keeps only the count.
///
/// Holds no buffer, so a measurement taken around it is the renderer's own allocation and a render
/// into it is not bounded by anything the test does.
#[derive(Default)]
struct Accepting {
  taken: usize,
}

impl core::fmt::Write for Accepting {
  fn write_str(&mut self, text: &str) -> core::fmt::Result {
    self.taken += text.chars().count();
    Ok(())
  }
}

/// Takes a fixed number of characters and then declines, holding nothing.
///
/// Separate from [`Recording`] below, and the separation is the point: measuring allocation through
/// a writer that keeps a copy measures the HARNESS. The first draft used the recording one for
/// dimension 2 and reported thirty-three kilobytes, all of them its own buffer.
struct Bounded {
  budget: usize,
  taken: usize,
}

impl core::fmt::Write for Bounded {
  fn write_str(&mut self, text: &str) -> core::fmt::Result {
    for _ in text.chars() {
      if self.taken == self.budget {
        return Err(core::fmt::Error);
      }
      self.taken += 1;
    }
    Ok(())
  }
}

/// Takes a fixed number of characters, then declines, keeping everything it was OFFERED.
///
/// What painty writes is painty's; how much is taken is the writer's. Dimension 3 is therefore
/// asserted over what was offered — a record of what was ACCEPTED could not show it, because a
/// writer that refuses the body refuses the reset too.
struct Recording {
  budget: usize,
  taken: usize,
  offered: String,
}

impl Recording {
  fn new(budget: usize) -> Self {
    Self {
      budget,
      taken: 0,
      offered: String::new(),
    }
  }
}

impl core::fmt::Write for Recording {
  fn write_str(&mut self, text: &str) -> core::fmt::Result {
    self.offered.push_str(text);
    for _ in text.chars() {
      if self.taken == self.budget {
        return Err(core::fmt::Error);
      }
      self.taken += 1;
    }
    Ok(())
  }
}

/// Whether the text finishes with a style still open.
///
/// Counting openers against resets does NOT work, and the first draft of dimension 3 failed on a
/// correct frame because of it: `anstyle` renders bold and colour as two separate sequences and
/// closes both with one reset, so a balanced frame has more openers than resets. What has to hold
/// is the state at the END — walk the sequences in order, and the last complete one must be a
/// reset.
///
/// An incomplete sequence is skipped rather than parsed, because a refused write can leave one: a
/// terminal does not act on a sequence it never received the end of, so neither does this.
fn ends_styled(text: &str) -> bool {
  let characters: Vec<char> = text.chars().collect();
  let mut open = false;
  let mut at = 0;
  while at < characters.len() {
    if characters[at] == '\u{1b}' && characters.get(at + 1) == Some(&'[') {
      let mut end = at + 2;
      while end < characters.len() && (characters[end].is_ascii_digit() || characters[end] == ';') {
        end += 1;
      }
      if characters.get(end) == Some(&'m') {
        let parameters: String = characters[at + 2..end].iter().collect();
        open = parameters != "0";
        at = end + 1;
        continue;
      }
    }
    at += 1;
  }
  open
}

/// One adversarial input: compact source, extreme rendered geometry.
///
/// `cells` is what the line ASKS for, computed from the input rather than written down, so a case
/// cannot quietly stop being extreme.
struct Case {
  what: &'static str,
  text: String,
  tab_width: u64,
  span: Span,
  cells: u64,
  /// Bytes of the LINE, which is what the resource bound is denominated in. A case can be extreme
  /// in this and cost nothing in cells — that is the whole reason the two are separate.
  bytes: u64,
}

impl Case {
  /// Whether the excerpt must be cut, derived from both budgets rather than from either.
  fn is_cut(&self) -> bool {
    self.cells > Terminal::<Theme>::max_rendered_width()
      || self.bytes > Terminal::<Theme>::max_source_bytes()
  }
}

fn cases() -> Vec<Case> {
  let ceiling = Terminal::<Theme>::max_rendered_width();
  vec![
    Case {
      what: "an ordinary diagnostic, which must not be elided at all",
      text: "type Widget {\n  width: Int\n}\n".to_owned(),
      tab_width: 4,
      span: Span::new(16, 21),
      cells: 12,
      bytes: 12,
    },
    Case {
      what: "a line of tabs: 257 bytes asking for 65,536 cells",
      text: format!("{}\n", "\t".repeat(256)),
      tab_width: 256,
      span: Span::new(0, 256),
      cells: 256 * 256,
      bytes: 256,
    },
    Case {
      what: "the same shape ten times over, which must render identically",
      text: format!("{}\n", "\t".repeat(2560)),
      tab_width: 256,
      span: Span::new(0, 2560),
      cells: 2560 * 256,
      bytes: 2560,
    },
    Case {
      what: "a long plain line, where the ceiling is reached without any tab multiplier",
      text: format!("{}\n", "a".repeat(20_000)),
      tab_width: 4,
      span: Span::new(0, 5),
      cells: 20_000,
      bytes: 20_000,
    },
    Case {
      what: "a span out PAST the ceiling, whose caret has nowhere of its own to sit",
      text: format!("{}\n", "a".repeat(20_000)),
      tab_width: 4,
      span: Span::new(19_000, 19_010),
      cells: 20_000,
      bytes: 20_000,
    },
    Case {
      what: "a line that stops just under the ceiling and must not be cut",
      text: format!("{}\n", "a".repeat(ceiling as usize)),
      tab_width: 4,
      span: Span::new(0, 5),
      cells: ceiling,
      bytes: ceiling,
    },
    Case {
      what: "ninety kilobytes of zero-width spaces, which cost no cells whatever",
      text: format!("{}\n", "\u{200b}".repeat(30_000)),
      tab_width: 4,
      span: Span::new(0, 3),
      cells: 0,
      bytes: 90_000,
    },
    Case {
      what: "one base and forty thousand combining marks: one cluster, one cell, eighty kilobytes",
      text: format!("a{}\n", "\u{301}".repeat(40_000)),
      tab_width: 4,
      span: Span::new(0, 1),
      cells: 1,
      bytes: 80_001,
    },
  ]
}

fn render_into(case: &Case, out: &mut impl core::fmt::Write) -> core::fmt::Result {
  let message = "a message";
  let diagnostic = Diagnostic::new(
    "mylang::test::rule",
    Severity::Error,
    &message,
    Location::new(0, case.span),
  )
  .with_primary_label("here");
  Terminal::plain().with_tab_width(case.tab_width).render(
    &diagnostic,
    &[Input::new(Source::new(&case.text))],
    out,
  )
}

#[test]
fn what_the_renderer_emits_is_bounded_by_policy_and_not_by_the_input() {
  // Dimension 1, and the one the other three do not imply. Streaming a row instead of building it
  // moved the cost from an allocation to the writer; against a writer that says yes — a `String`, a
  // log sink — the cost is still there, and 257 bytes of tabs asked for 65,536 cells.
  //
  // The bound is on rendered CELLS because that is what grows. Bounding the tab width bounds the
  // multiplier and bounding nothing bounds the line length, and a budget on one factor of a product
  // bounds nothing at all.
  let ceiling = Terminal::<Theme>::max_rendered_width();
  // Two rows, each at most the ceiling plus its elision cell, plus the frame's fixed furniture and
  // the caller's own short strings. Deliberately slack: the claim is that the size is a function of
  // the POLICY rather than of the input, and an exact formula would be a second implementation of
  // the layout.
  //
  // Saturating, and `try_from` rather than `as`: a harness that panics on its own arithmetic
  // reports nothing about the code, and a test about narrowing has no business narrowing.
  let allowed = usize::try_from(ceiling)
    .unwrap_or(usize::MAX)
    .saturating_mul(8);

  for case in cases() {
    let mut out = Accepting::default();
    render_into(&case, &mut out).expect("a counting writer never refuses");
    assert!(
      out.taken < allowed,
      "{}: asked for {} cells and emitted {} characters, over the {allowed} a ceiling of {ceiling} \
       allows",
      case.what,
      case.cells,
      out.taken
    );
  }
}

#[test]
fn past_the_ceiling_the_output_stops_depending_on_the_input() {
  // The sharp form of the same property, and the one that cannot be satisfied by a bound that is
  // merely generous: two inputs an order of magnitude apart in rendered width must produce the same
  // frame, byte for byte. Anything that still scaled with the input would differ here however large
  // the allowance in the test above.
  let all = cases();
  let small = &all[1];
  let ten_times = &all[2];
  assert_eq!(
    ten_times.cells,
    small.cells * 10,
    "the two cases are no longer an order of magnitude apart, so this proves nothing"
  );

  let mut one = String::new();
  render_into(small, &mut one).expect("a String is writable");
  let mut other = String::new();
  render_into(ten_times, &mut other).expect("a String is writable");

  assert_eq!(
    one, other,
    "{} cells and {} cells rendered differently, so output still tracks the input past the ceiling",
    small.cells, ten_times.cells
  );
  assert!(
    one.contains('…'),
    "neither was elided, so the comparison above is not about the ceiling: {one:?}"
  );
}

#[test]
fn a_cut_row_says_that_it_was_cut() {
  // Elision has to be visible. A diagnostic that quietly truncates is worse than one that admits
  // it: the reader believes they are looking at the line.
  //
  // Asserted in both directions, so the mark cannot start appearing on lines that were never cut —
  // which would be its own kind of lie.
  let ceiling = Terminal::<Theme>::max_rendered_width();
  for case in cases() {
    let mut out = String::new();
    render_into(&case, &mut out).expect("a String is writable");
    let cut = case.is_cut();
    assert_eq!(
      out.contains('…'),
      cut,
      "{}: asked for {} cells against a ceiling of {ceiling} and the elision mark was {}",
      case.what,
      case.cells,
      if out.contains('…') {
        "present"
      } else {
        "absent"
      }
    );

    // And the caret stays over a cell the source row actually drew. A row stops before the first
    // unit that would straddle the ceiling, so where it ends is NOT the ceiling — clipping the
    // marker to the ceiling instead would put the caret past the end of the line it points into.
    // The source row is the one above the marker row, found by position rather than by guessing at
    // its text.
    let rows: Vec<&str> = out.lines().collect();
    let marker = rows
      .iter()
      .position(|row| row.contains('^'))
      .expect("a marker row");
    let caret = rows[marker]
      .chars()
      .position(|character| character == '^')
      .expect("a caret");
    let drawn = rows[marker - 1].chars().count();
    assert!(
      caret < drawn,
      "{}: the caret sits at column {caret} of a source row that drew {drawn}:\n{out}",
      case.what
    );
  }
}

#[test]
fn the_renderer_allocates_nothing_proportional_to_a_row() {
  // Dimension 2. Both rows used to be built into `String`s before `out` was consulted, so a caller
  // with a bounded or refusing writer could not decline what it never saw.
  //
  // Measured rather than read: a streamed row and a materialised one produce identical bytes, so
  // there is no output to assert on. Well above the handful of small vectors and one line number
  // the renderer legitimately allocates, and well below a row.
  let allowed = usize::try_from(Terminal::<Theme>::max_rendered_width()).unwrap_or(usize::MAX) / 2;

  for case in cases() {
    for budget in [0, 40, 400, usize::MAX] {
      let mut out = Bounded { budget, taken: 0 };
      let before = allocated();
      let _ = render_into(&case, &mut out);
      let spent = allocated() - before;
      assert!(
        spent < allowed,
        "{}: rendering {} cells to a writer with a budget of {budget} cost {spent} bytes, over the \
         {allowed} allowed",
        case.what,
        case.cells
      );
    }
  }
}

#[test]
fn a_refused_write_is_reported_and_leaves_no_style_open() {
  // Dimension 3. `styled_with` wrote the opener, then `body(..)?` — so any failure returned before
  // the reset and left the caller's terminal wearing a style. The closure form was introduced for
  // exactly this property and does not provide it: a closure stops a caller FORGETTING the close
  // and does nothing about the body NOT REACHING it.
  //
  // Swept over every budget for the ordinary frame, because which write fails is the whole subject
  // and a hand-picked position would be picked by someone who believed the code was right. Sampled
  // for the extreme cases, where the interesting boundaries are the same ones and the frame is
  // thousands of cells wide.
  let terminal = Terminal::with_palette(Theme::new()).with_capability(ColorCapability::TrueColor);
  let message = "a message";

  for case in cases() {
    let diagnostic = Diagnostic::new(
      "mylang::test::rule",
      Severity::Error,
      &message,
      Location::new(0, case.span),
    )
    .with_primary_label("here");
    let inputs = [Input::new(Source::new(&case.text))];
    let styled = terminal.with_tab_width(case.tab_width);

    let mut full = String::new();
    styled
      .render(&diagnostic, &inputs, &mut full)
      .expect("a String is writable");
    assert!(
      !ends_styled(&full),
      "{}: the frame ends styled even when nothing refuses",
      case.what
    );

    let length = full.chars().count();
    let budgets: Vec<usize> = if length <= 400 {
      (0..length).collect()
    } else {
      (0..200).chain((200..length).step_by(97)).collect()
    };
    for budget in budgets {
      let mut out = Recording::new(budget);
      let result = styled.render(&diagnostic, &inputs, &mut out);
      assert!(
        result.is_err(),
        "{}: refusing after {budget} of {length} characters was reported as success",
        case.what
      );
      assert!(
        !ends_styled(&out.offered),
        "{}: refusing after {budget} characters left a style open: {:?}",
        case.what,
        out.offered
      );
    }
  }
}

/// How long the renderer's own geometry takes on a line of `megabytes`, with the span placed past
/// the drawable window so that the answer is an elision marker either way.
///
/// Repeated, because one call at this size is a few hundred microseconds and a single sample of
/// that is mostly timer.
fn geometry_cost(megabytes: usize) -> core::time::Duration {
  let text = format!("{}\n", "a".repeat(megabytes * 1_000_000));
  let source = Source::new(&text);
  let far = megabytes * 1_000_000 - 10;
  let region = source.resolve(Span::new(far, far + 5));
  let line = region.lines().next().expect("a line");
  let terminal = Terminal::plain();
  let _ = terminal.underline(line);

  let started = std::time::Instant::now();
  for _ in 0..20 {
    core::hint::black_box(terminal.underline(core::hint::black_box(line)));
  }
  started.elapsed()
}

#[test]
#[cfg_attr(
  miri,
  ignore = "an asymptotic measurement over nine megabytes of input, which Miri would take hours to \
            walk and is not checking anyway"
)]
fn the_geometry_costs_the_window_and_not_the_line() {
  // Dimension 5. `underline` asked `columns_for` for exact columns and clipped them afterwards, so
  // a span past the window walked every placement unit in the line — grapheme segmentation over
  // twenty million of them — to produce a caret under an elision mark. Work with no output at all.
  //
  // A RATIO and not a threshold, deliberately. Machine speed cancels, so this cannot flake on a
  // loaded runner or pass on a fast one, and it states the actual property: the cost does not
  // depend on the length of the line. Measured while writing it, in the same debug profile the
  // suite runs in:
  //
  //   bounded    1 MB 0.7689 ms/call   8 MB 0.7720 ms/call   ratio 1.004
  //   unbounded  1 MB 187.53 ms/call   8 MB 1482.90 ms/call  ratio 7.91
  //
  // Eight times the line for the same window, so a walk of the line shows up as very nearly eight
  // and a walk of the window as very nearly one. Three is a wide corridor between them.
  let small = geometry_cost(1);
  let large = geometry_cost(8);
  assert!(
    small > core::time::Duration::from_micros(50),
    "the smaller measurement is {small:?}, too close to the timer to divide by"
  );

  let growth = large.as_secs_f64() / small.as_secs_f64();
  assert!(
    growth < 3.0,
    "eight times the line cost {growth:.2} times the geometry ({small:?} then {large:?}) — the walk \
     is following the line rather than stopping at the window"
  );
}

/// The two halves of the size contract, stated as one inequality.
///
/// `max_rendered_width` bounds the EXCERPT and not the caller's own strings, so what a render
/// produces is a bound from the policy plus whatever was passed in. Both halves need pinning and
/// they pin against each other: assert only the first and a future change could satisfy it by
/// truncating a label; assert only the second and the amplification bound could quietly lapse.
mod the_size_contract {
  use super::{Accepting, cases, render_into};
  use painty::{
    Diagnostic, Location, Severity, Source, Span, Theme,
    terminal::{Input, Terminal},
  };

  #[test]
  fn no_small_input_produces_a_large_excerpt() {
    // The half painty owns, because painty creates it: an excerpt AMPLIFIES. 257 bytes of tabs ask
    // for 65,536 cells, two hundred and fifty times what was handed over, and a caller could not
    // have predicted it from the input.
    //
    // Every case here carries short caller text, so anything large in the output came from the
    // source — which is what makes this the amplification statement rather than a restatement of
    // the emission bound.
    let ceiling = Terminal::<Theme>::max_rendered_width();
    let allowed = usize::try_from(ceiling)
      .unwrap_or(usize::MAX)
      .saturating_mul(8);

    let mut examined = 0;
    for case in cases() {
      // Only a SMALL input can demonstrate amplification. The cases that are large in themselves —
      // the zero-width and combining runs — are about the resource bound instead, and are covered
      // by the dimensions above; counting them here would let a case that proves nothing look like
      // evidence.
      if case.text.len() >= allowed {
        continue;
      }
      examined += 1;

      let mut out = Accepting::default();
      render_into(&case, &mut out).expect("a counting writer never refuses");
      assert!(
        out.taken < allowed,
        "{}: {} bytes of source became {} characters, over the {allowed} a ceiling of {ceiling} \
         allows",
        case.what,
        case.text.len(),
        out.taken
      );
    }
    assert!(
      examined >= 4,
      "only {examined} cases were small enough to show amplification, so the table has drifted away \
       from what this test is for"
    );
  }

  #[test]
  fn alternating_inputs_do_not_multiply_their_origins_or_their_source_rows() {
    // Caller text is pass-through at 1x — that is the contract — and this path made it k, twice
    // over and for two different reasons.
    //
    // The header is emitted on a CHANGE of input, and a change was only the last one seen, so
    // labels alternating between two files re-emitted both origins on every switch. Two large
    // origins and many small labels multiply the caller's own strings by the number of runs, which
    // is amplification of exactly the kind the contract says never happens to them.
    //
    // Then the same k in the other column. Every label was its own EXCERPT, so k labels on one line
    // rewrote that line's source row k times and wrote k copies of the label text under it —
    // `label_count × excerpt_width`, from a bounded window the size contract says is a function of
    // the policy. The row is written once now, with k marker rows under it, and k marker rows is
    // the honest floor: two labels on one line are two things to point at.
    let origin_one = format!("src/{}.rs", "a".repeat(20_000));
    let origin_two = format!("src/{}.rs", "b".repeat(20_000));
    let first = "alpha bravo charlie\n";
    let second = "delta echo foxtrot\n";
    let message = "a message";

    // Alternating deliberately: grouped input would pass even with the defect present.
    let labels: Vec<_> = (0..8)
      .map(|index| {
        painty::Label::new(
          Location::new(index % 2, Span::new(0, 5)),
          if index % 2 == 0 { "here" } else { "there" },
        )
      })
      .collect();
    let diagnostic = Diagnostic::new(
      "mylang::test::rule",
      Severity::Error,
      &message,
      Location::new(0, Span::new(6, 11)),
    )
    .with_primary_label("primary")
    .with_labels(&labels);

    let mut out = String::new();
    Terminal::plain()
      .render(
        &diagnostic,
        &[
          Input::new(Source::new(first)).with_origin(&origin_one),
          Input::new(Source::new(second)).with_origin(&origin_two),
        ],
        &mut out,
      )
      .expect("a String is writable");

    assert_eq!(
      out.matches(origin_one.as_str()).count(),
      1,
      "the first origin was emitted {} times",
      out.matches(origin_one.as_str()).count()
    );
    assert_eq!(
      out.matches(origin_two.as_str()).count(),
      1,
      "the second origin was emitted {} times",
      out.matches(origin_two.as_str()).count()
    );

    // One source row per input line, whatever the label count. Nine labels land on two lines, so
    // the two lines are written once each — and the value a per-label renderer produces is named,
    // so the wrong implementation cannot satisfy this by accident.
    for (row, per_label) in [("alpha bravo charlie", 5), ("delta echo foxtrot", 4)] {
      let drawn = out.matches(row).count();
      assert_ne!(
        drawn, per_label,
        "{row:?} was drawn once per label rather than once per line\n{out}"
      );
      assert_eq!(drawn, 1, "{row:?} was drawn {drawn} times\n{out}");
    }

    // And nothing was dropped to get there: every label still has a marker row of its own, because
    // coalescing merges the ROW and not what is said under it. Counted with the marker attached, so
    // that "there" is not also counted as a "here".
    assert_eq!(
      out.matches("- here").count(),
      4,
      "a label on a shared line lost its marker row\n{out}"
    );
    assert_eq!(out.matches("- there").count(), 4, "{out}");
    assert_eq!(out.matches("^ primary").count(), 1, "{out}");
    assert_eq!(
      out.matches('^').count(),
      5,
      "the primary's caret is five cells over `bravo`\n{out}"
    );

    // And the primary's file still leads, so grouping has not moved another file above the position
    // the diagnostic is actually about.
    assert!(
      out.find(origin_one.as_str()) < out.find(origin_two.as_str()),
      "grouping reordered the inputs away from where the primary is"
    );
    // The primary leads within its line too. Its span starts at byte 6 and the labels sharing the
    // row start at byte 0, so a renderer ordering marker rows by column would put it second.
    let primary = out.find("primary").expect("the primary's label");
    let first_secondary = out.find("here").expect("a secondary label");
    assert!(
      primary < first_secondary,
      "the primary lost its place on a shared line\n{out}"
    );
  }

  #[test]
  fn labels_on_one_line_add_marker_rows_and_not_excerpts() {
    // The output half of the k axis, and the half no timing can see: an excerpt is bounded by the
    // ceiling, so drawing k of them costs `k × 4096` characters and no measurable time next to a
    // walk of an eight-megabyte line. It has to be counted instead.
    //
    // A line well over the ceiling, so an excerpt is expensive, and the labels crowded at its start
    // so that a marker row is cheap — which is what makes the difference between the two shapes
    // enormous rather than merely visible.
    let text = format!("{}\n", "a".repeat(20_000));
    let message = "a message";
    let emitted = |count: usize| {
      let labels: Vec<painty::Label<'_>> = (1..count)
        .map(|nth| painty::Label::new(Location::new(0, Span::new(nth * 10, nth * 10 + 5)), "there"))
        .collect();
      let diagnostic = Diagnostic::new(
        "mylang::test::rule",
        Severity::Error,
        &message,
        Location::new(0, Span::new(0, 5)),
      )
      .with_primary_label("here")
      .with_labels(&labels);

      let mut out = Accepting::default();
      Terminal::plain()
        .render(&diagnostic, &[Input::new(Source::new(&text))], &mut out)
        .expect("a counting writer never refuses");
      out.taken
    };

    let one = emitted(1);
    let eight = emitted(8);
    let excerpt = usize::try_from(Terminal::<Theme>::max_rendered_width()).unwrap_or(usize::MAX);
    assert!(
      one > excerpt,
      "the line is not over the ceiling, so an extra excerpt would be cheap and this proves nothing"
    );

    // Seven more labels may add seven marker rows. They may not add seven excerpts, which is what
    // the value below names: a renderer drawing one excerpt per label grows by `7 × 4096` here.
    let per_label_excerpts = 7 * excerpt;
    let grew = eight - one;
    assert!(
      grew < excerpt,
      "seven more labels on one line added {grew} characters, which is an excerpt apiece rather \
       than a marker row apiece — a per-label renderer grows by about {per_label_excerpts}"
    );

    // And they really were drawn, so the assertion above cannot be satisfied by dropping them.
    let mut frame = String::new();
    let labels: Vec<painty::Label<'_>> = (1..8)
      .map(|nth| painty::Label::new(Location::new(0, Span::new(nth * 10, nth * 10 + 5)), "there"))
      .collect();
    let diagnostic = Diagnostic::new(
      "mylang::test::rule",
      Severity::Error,
      &message,
      Location::new(0, Span::new(0, 5)),
    )
    .with_primary_label("here")
    .with_labels(&labels);
    Terminal::plain()
      .render(&diagnostic, &[Input::new(Source::new(&text))], &mut frame)
      .expect("a String is writable");
    assert_eq!(
      frame.matches("- there").count(),
      7,
      "a label lost its marker row"
    );
    assert_eq!(
      frame.matches('…').count(),
      1,
      "the line over the ceiling was excerpted more than once"
    );
  }

  #[test]
  fn a_large_label_is_printed_in_full_and_not_cut() {
    // The half painty does NOT own, pinned deliberately so that a future change which starts
    // truncating fails here rather than passing quietly.
    //
    // Caller text PASSES THROUGH: it is printed once, so ten megabytes in is ten megabytes out.
    // There is no amplification to bound and no surprise to prevent — the caller knows how long its
    // own string is and can pass a shorter one. Cutting it would be the worse failure: a diagnostic
    // that silently drops the part its author wrote is lying about what it was asked to report, and
    // the caller cannot see that it happened.
    let long = "x".repeat(100_000);
    let message = format!("message {long}");
    let source = "fn main() {}\n";
    let diagnostic = Diagnostic::new(
      "mylang::test::rule",
      Severity::Error,
      &message,
      Location::new(0, Span::new(0, 2)),
    )
    .with_primary_label(&long)
    .with_help(&long);

    let mut out = String::new();
    Terminal::plain()
      .render(
        &diagnostic,
        &[Input::new(Source::new(source)).with_origin(&long)],
        &mut out,
      )
      .expect("a String is writable");

    // Four caller strings carry it — message, label, help and origin — and each must carry it
    // whole. Counted rather than merely found, so truncating one of the four cannot pass.
    assert_eq!(
      out.matches(long.as_str()).count(),
      4,
      "a caller string was cut: the label appears {} times in {} characters of output",
      out.matches(long.as_str()).count(),
      out.chars().count()
    );
    assert!(
      out.chars().count() > 4 * long.chars().count(),
      "the output is shorter than the text handed to it"
    );

    // And no elision mark anywhere, because nothing here is an excerpt over the ceiling. The `…` is
    // the excerpt's business; a caller string never earns one.
    assert!(
      !out.contains('…'),
      "caller text was marked as elided, which is what this test exists to forbid"
    );
  }
}

/// What layer 2 alone costs to resolve one span and hand back its line, and what a whole render of
/// `positions` of them costs on top.
///
/// Both are linear in the line, so the figure that means anything is the ratio: how many passes
/// over the caller's input the renderer adds to the one layer 2 must make.
///
/// The extra positions are spread along the SAME line, from its start to the primary's offset.
/// That is the shape a renderer resolving each label independently pays most for — every one of
/// them is another walk from the top and another scan for where the line ends — and it is also the
/// shape that coalesces to a single excerpt, so the output stays small and what is being timed is
/// the WORK rather than the writing.
fn passes_over_layer_two(megabytes: usize, offset: usize, positions: usize) -> f64 {
  let text = format!("{}\n", "a".repeat(megabytes * 1_000_000));
  let span = Span::new(offset, offset + 5);
  let message = "a message";
  let labels: Vec<painty::Label<'_>> = (1..positions)
    .map(|nth| {
      let at = offset * nth / positions;
      painty::Label::new(Location::new(0, Span::new(at, at + 5)), "there")
    })
    .collect();
  let diagnostic = Diagnostic::new(
    "mylang::test::rule",
    Severity::Error,
    &message,
    Location::new(0, span),
  )
  .with_primary_label("here")
  .with_labels(&labels);

  let source = Source::new(&text);
  let _ = source.resolve(span).lines().next();
  let started = std::time::Instant::now();
  for _ in 0..3 {
    core::hint::black_box(source.resolve(span).lines().next());
  }
  let layer_two = started.elapsed().as_secs_f64();

  let started = std::time::Instant::now();
  for _ in 0..3 {
    let mut out = String::new();
    Terminal::plain()
      .render(&diagnostic, &[Input::new(Source::new(&text))], &mut out)
      .expect("a String is writable");
    core::hint::black_box(out);
  }
  started.elapsed().as_secs_f64() / layer_two
}

#[test]
#[cfg_attr(
  miri,
  ignore = "an asymptotic measurement over nine megabytes of input, which Miri is not checking"
)]
fn the_renderer_adds_a_bounded_number_of_passes_over_the_input() {
  // The render-level companion to `the_geometry_costs_the_window_and_not_the_line`, which asserts
  // at `underline` — and that is exactly why a second unbounded pass in the SHIPPING path survived
  // it. A property held at a helper says nothing about the function that ships.
  //
  // What cannot be asserted here is that a render is bounded, because it is not and no budget can
  // make it so: layer 2 turns a byte offset into a line and column by scanning to it, and finding
  // where a line ENDS is linear in the line whatever the span. So what is asserted is the number of
  // PASSES, which is a real bound and the thing a regression would move.
  //
  // # The k axis, which is the half that was missing
  //
  // This measured one label, and one label is exactly the shape that cannot see the defect it was
  // meant to hold off. Every label was resolved from the top of the input and drawn as its own
  // excerpt, so the whole cost went as k — 22.7 ms of it per label on an eight-megabyte line — and
  // a pass count pinned at k=1 stayed green through eight rounds of review while it did.
  //
  // So both ends are measured, and against the SAME bound rather than a looser one for k=8: the
  // claim is that the pass count does not depend on the label count at all. Measured while writing
  // this, in the same debug profile the suite runs in:
  //
  //   near span   k=1 1.05x   k=8 1.06x
  //   far span    k=1 1.02x   k=8 1.02x
  //
  // Two allows that nearly twice over and refuses a doubling. It is not a precise instrument and
  // does not need to be: with the carried cursor removed so that each label resolves from the top
  // again, k=8 measures 7.97x near and 5.71x far, because eight labels are eight walks to their
  // own offsets and eight scans for where the line ends. Both ends of k=1 stay green under that
  // same plant, at 1.06x and 1.02x, which is the whole reason this axis exists.
  const ALLOWED: f64 = 2.0;

  for positions in [1, 8] {
    let near = passes_over_layer_two(8, 0, positions);
    assert!(
      near < ALLOWED,
      "a render of {positions} positions at the start of the line cost {near:.2} times layer 2 \
       alone, so the renderer has taken on a pass that does not depend on where the span is"
    );

    let far = passes_over_layer_two(8, 8_000_000 - 10, positions);
    assert!(
      far < ALLOWED,
      "a render of {positions} positions along the line cost {far:.2} times layer 2 alone; one \
       forward pass over the input is the bound, and it does not widen with the label count"
    );
  }
}
