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
//!
//! # Almost none of this is interpreted under Miri
//!
//! Four of the five dimensions above are RESOURCE dimensions — how much is emitted, how much is
//! allocated, how much work is done, and how wide a column is on a narrower target. Miri answers a
//! different question, whether an execution path has undefined behaviour, and it is not an
//! instrument for any of the four; the two timing tests below already say as much in their own
//! `ignore` reasons.
//!
//! What makes that decisive rather than philosophical is the size of the inputs. [`cases`] is sized
//! against `Terminal::max_source_bytes`, which is 65,536, so one render walks and segments tens of
//! thousands of bytes — and five of the tests below render the whole table, between six and roughly
//! two thousand times each. Four more build their own large inputs instead: two twenty-kilobyte
//! origins, a twenty-thousand-byte line, four caller strings of a hundred kilobytes apiece, and a
//! four-mebibyte fragment written until a mebibyte of it has been kept.
//!
//! CI has measured exactly one of them. Six of the eight cells in Miri run 31316096247 reached this
//! file at all — the other two had already ICEd in `tests/numeric_widths.rs` — and not one of the
//! six got past its SECOND test. [`a_cut_row_says_that_it_was_cut`], eight renders, took 7m37s on
//! the quickest cell and 48m34s on the slowest, and two cells were killed while still inside it. On
//! the four that finished it, [`the_size_contract::a_large_label_is_printed_in_full_and_not_cut`]
//! was still running 3h14m later when the job hit GitHub's six-hour ceiling. Six of the eight cells
//! died on that ceiling rather than on a finding.
//!
//! So every test here that renders a large input carries `#[cfg_attr(miri, ignore = "…")]`, and one
//! added below should carry one too. **Nothing about painty's execution leaves the interpreter's
//! view with them**, which is the part worth checking rather than asserting:
//!
//! * `Terminal::render` end to end — `tests/terminal_appearance.rs`, twenty-two tests over realistic
//!   inputs, 20s to 67s per cell.
//! * The elision path, which is what the table exists to reach —
//!   [`past_the_ceiling_the_output_stops_depending_on_the_input`], which is NOT ignored. It renders
//!   the two small tab cases, asserts the mark is there, and costs about half a minute.
//! * The ceiling and byte-budget arithmetic underneath it —
//!   `a_ceiling_stops_the_walk_where_a_whole_unit_stops`,
//!   `a_zero_width_run_is_stopped_by_the_byte_budget_and_by_nothing_else` and
//!   `slicing_a_line_at_the_budget_does_not_change_the_units_before_the_cut`, in
//!   `src/terminal/tests.rs`, all on inputs a few bytes long.
//! * A writer that refuses — `Budgeted`, in the same file.
//!
//! And the ignored tests are not skipped anywhere else: `cargo hack test -p painty
//! --feature-powerset` runs them on three operating systems, as do the coverage and sanitizer jobs.
//!
//! Test by test rather than a whole-file `#![cfg(not(miri))]` like `tests/numeric_widths.rs` carries,
//! and the reason is [`Counting`] below. It is the crate's only `unsafe`, so it is the only thing
//! here Miri can have a finding about, and it is exercised on every allocation this binary makes —
//! including the harness's own, with every test body ignored. The whole-file form would take it out
//! with them.

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
/// `cells` is what the WIDEST DRAWN LINE asks for, computed from the input rather than written
/// down, so a case cannot quietly stop being extreme. The widest rather than the only one, because
/// a span reaching past its first line has several and the budgets apply to each.
struct Case {
  what: &'static str,
  text: String,
  tab_width: u64,
  span: Span,
  cells: u64,
  /// Bytes of the widest drawn LINE, which is what the resource bound is denominated in. A case
  /// can be extreme in this and cost nothing in cells — that is the whole reason the two are
  /// separate.
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
  // The one case that is built AT the ceiling needs it as a length. `try_from` rather than `as`,
  // for the reason `what_the_renderer_emits_is_bounded_by_policy_and_not_by_the_input` gives below:
  // a suite about narrowing has no business narrowing, and a ceiling that did not fit would build a
  // much shorter line instead and quietly stop testing the boundary.
  let ceiling_bytes = usize::try_from(ceiling).expect("a ceiling the machine can allocate");
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
      text: format!("{}\n", "a".repeat(ceiling_bytes)),
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
    // Appended rather than inserted: `past_the_ceiling_the_output_stops_depending_on_the_input`
    // names two of the cases above by INDEX.
    Case {
      what: "a span bracketed across four lines, whose corners and connectors are new styled runs",
      text: "open {\n  a\n  b\n}\n".to_owned(),
      tab_width: 4,
      span: Span::new(0, 16),
      cells: 6,
      bytes: 6,
    },
    Case {
      what: "a bracket whose CLOSING line is past the ceiling, so its corner lands on the elision",
      text: format!("open\n{}\n", "a".repeat(20_000)),
      tab_width: 4,
      span: Span::new(0, 20_005),
      cells: 20_000,
      bytes: 20_000,
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
#[cfg_attr(
  miri,
  ignore = "a size bound over the whole 65,536-byte case table, which Miri neither checks nor can \
            afford"
)]
fn what_the_renderer_emits_is_bounded_by_policy_and_not_by_the_input() {
  // Dimension 1, and the one the other three do not imply. Streaming a row instead of building it
  // moved the cost from an allocation to the writer; against a writer that says yes — a `String`, a
  // log sink — the cost is still there, and 257 bytes of tabs asked for 65,536 cells.
  //
  // The bound is on rendered CELLS because that is what grows. Bounding the tab width bounds the
  // multiplier and bounding nothing bounds the line length, and a budget on one factor of a product
  // bounds nothing at all.
  let ceiling = Terminal::<Theme>::max_rendered_width();
  // A bracketed span draws at most seven rows of source width — an opening, the three lines after
  // it, a closing, and the two corners — each at most the ceiling plus its elision cell, plus the
  // frame's fixed furniture and the caller's own short strings. Deliberately slack: the claim is
  // that the size is a function of the POLICY rather than of the input, and an exact formula would
  // be a second implementation of the layout.
  //
  // The multiplier is what the rows-per-span rule allows, and it is where that rule shows up as a
  // number. It was 8 when a span was drawn on one line and a bracket now draws several — which is
  // the honest reading, since a bound that did not move when the rows did would be describing the
  // renderer it replaced.
  //
  // Saturating, and `try_from` rather than `as`: a harness that panics on its own arithmetic
  // reports nothing about the code, and a test about narrowing has no business narrowing.
  let allowed = usize::try_from(ceiling)
    .unwrap_or(usize::MAX)
    .saturating_mul(16);

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
#[cfg_attr(
  miri,
  ignore = "eight renders of the whole case table: 7m37s on the quickest Miri cell and 48m34s on \
            the slowest. The elision path it reaches stays interpreted by \
            `past_the_ceiling_the_output_stops_depending_on_the_input`"
)]
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
#[cfg_attr(
  miri,
  ignore = "thirty-two renders of the whole case table, measuring allocation — which under an \
            interpreter is the interpreter's and not the renderer's"
)]
fn the_renderer_allocates_nothing_proportional_to_a_row() {
  // Dimension 2. Both rows used to be built into `String`s before `out` was consulted, so a caller
  // with a bounded or refusing writer could not decline what it never saw.
  //
  // Measured rather than read: a streamed row and a materialised one produce identical bytes, so
  // there is no output to assert on.
  //
  // Two assertions, and the second is the one with teeth. A threshold cannot discriminate anything
  // on a twelve-cell case, where a materialised row would be twelve bytes; what it discriminates is
  // a row at the CEILING being built, so it sits below that with room. The equality below needs no
  // threshold at all.
  let ceiling = usize::try_from(Terminal::<Theme>::max_rendered_width()).unwrap_or(usize::MAX);
  let allowed = ceiling / 2;

  let mut spent_on = Vec::new();
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
      if budget == usize::MAX {
        spent_on.push(spent);
      }
    }
  }

  // And the sharp form, on the pair `past_the_ceiling_the_output_stops_depending_on_the_input`
  // already holds an order of magnitude apart: the same bookkeeping to the BYTE, so nothing here
  // tracks the width of the row at all. A threshold can be met by an allocation that grows slowly;
  // an equality cannot.
  let all = cases();
  assert_eq!(
    all[2].cells,
    all[1].cells * 10,
    "the two cases are no longer an order of magnitude apart, so this proves nothing"
  );
  assert_eq!(
    spent_on[1], spent_on[2],
    "{} cells and {} cells cost {} and {} bytes, so the renderer still allocates with the row",
    all[1].cells, all[2].cells, spent_on[1], spent_on[2]
  );
}

/// A `Display` that synthesizes `target` bytes by writing `chunk` until it has, and stops when it
/// is refused.
///
/// # It BORROWS its fragment, and that is the whole reason this can be measured
///
/// The hazard being measured is that a caller value of a few words makes painty allocate without
/// bound, so the instrument has to charge painty for painty's bytes and nobody else's. A `Display`
/// that builds its own fragment allocates inside the measurement window, and in the oversized case
/// below the fragment IS the whole message — which made the first shape of this test charge the
/// renderer four mebibytes the *caller* had spent and read a correct capture as a broken one.
///
/// So the fragment is built once by the test, outside every window, and this writes it. What the
/// counter then sees between the two reads is the renderer's, and nothing here is on the invoice.
#[cfg(feature = "svg")]
struct Synthesizing<'a> {
  chunk: &'a str,
  target: usize,
}

#[cfg(feature = "svg")]
impl core::fmt::Display for Synthesizing<'_> {
  fn fmt(&self, out: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    let mut written = 0;
    while written < self.target {
      out.write_str(self.chunk)?;
      written += self.chunk.len();
    }
    Ok(())
  }
}

#[cfg(feature = "svg")]
#[test]
#[cfg_attr(
  miri,
  ignore = "three renders whose inputs are megabytes, measuring allocation — which under an \
            interpreter is the interpreter's and not the renderer's. The capture's boundary \
            arithmetic stays interpreted by \
            `the_capture_admits_exactly_the_ceiling_and_reports_what_it_refused`, in \
            `src/terminal/svg/mod.rs`"
)]
fn the_svg_capture_refuses_a_message_before_it_allocates_it() {
  // Dimension 2, for the one input in this crate that is HELD rather than streamed. `render_svg`
  // renders twice and may only ask a `Display` once, so it formats the message into a string that
  // outlives the measuring pass — and a `&dyn Display` is two words that can answer with a
  // gigabyte. Before the ceiling, a caller value smaller than this comment made the renderer
  // allocate and spend without limit, and it did so before `out` was offered the first byte it
  // would have refused.
  //
  // Measured rather than read: a refusal after the bytes have been appended returns the same error
  // to the same caller as a refusal before, so there is no output to assert on. This is the only
  // instrument that separates them.
  let ceiling = usize::try_from(Terminal::<Theme>::max_svg_message_bytes()).unwrap_or(usize::MAX);

  // Every fragment any case below writes, allocated once and here — see [`Synthesizing`] for why
  // that placement is the instrument rather than a tidiness. Sliced rather than re-allocated, so
  // the small fragment costs nothing either.
  let synthesized = "x".repeat(ceiling * 4);
  let oversized = synthesized.as_str();
  let drip = &synthesized[..4096];

  fn spend(message: &Synthesizing<'_>) -> (core::fmt::Result, usize) {
    let diagnostic = Diagnostic::new(
      "mylang::synthesized",
      Severity::Error,
      message,
      Location::new(0, Span::new(0, 3)),
    )
    .with_primary_label("here");
    let inputs = [Input::new(Source::new("let x = 1;\n"))];
    // Accepts everything and holds nothing, so the refusal below is the ceiling's and the bytes
    // counted are the renderer's.
    let mut out = Accepting::default();

    let before = allocated();
    let outcome = Terminal::plain().render_svg(&diagnostic, &inputs, &mut out);
    (outcome, allocated() - before)
  }

  // ONE fragment, four times the ceiling, and this is the case that separates the two orders.
  // Refused before the append it costs nothing whatever; appended and then refused it costs four
  // mebibytes, and no later refusal gives them back. The drip below cannot ask this, because a
  // fragment that crosses the ceiling by 4096 bytes is over it by 4096 bytes either way.
  //
  // It also asks a second thing, and it is worth naming because a passing run is the only evidence
  // for it: that `write_fmt` hands the sink the caller's fragment rather than materialising the
  // `Arguments` into a string of its own first. If it did, the bytes would be on this invoice.
  let (outcome, spent) = spend(&Synthesizing {
    chunk: oversized,
    target: oversized.len(),
  });
  //
  // Measured: zero bytes as it stands, and 4,194,304 with the append moved in front of the test.
  // The threshold is the ceiling rather than either, so it is a statement about the order and not
  // a transcript of one allocator's arithmetic.
  assert!(
    outcome.is_err(),
    "a message four times the ceiling rendered"
  );
  assert!(
    spent < ceiling,
    "one write of {} bytes cost {spent}, so the capture appended it before refusing it",
    oversized.len()
  );

  // And the drip, which is the shape a `Display` would actually take. Held bytes stop at the
  // ceiling; what is counted is above that only because a `String` reaching a size reallocates on
  // the way and this counter charges every one of those in full — measured at 2,093,056, which is
  // the doubling sum for one mebibyte held. Without the ceiling it is that arithmetic over
  // sixty-four mebibytes instead, so the threshold sits at twice what was measured and a
  // thirtieth of what the defect costs.
  let (outcome, spent) = spend(&Synthesizing {
    chunk: drip,
    target: ceiling * 64,
  });
  assert!(
    outcome.is_err(),
    "a message sixty-four times the ceiling rendered"
  );
  assert!(
    spent < ceiling * 4,
    "sixty-four mebibytes offered a fragment at a time cost {spent}, which is not a bound"
  );

  // Not vacuous in the other direction, and this is the half that says the ceiling is a ceiling
  // rather than a refusal: a message just under it renders, and costs about what it is.
  let (outcome, spent) = spend(&Synthesizing {
    chunk: drip,
    target: ceiling - 4096,
  });
  assert!(
    outcome.is_ok(),
    "a message under the ceiling was refused anyway"
  );
  assert!(
    spent >= ceiling - 4096,
    "a message of {} bytes was rendered having allocated {spent}, so it was never held",
    ceiling - 4096
  );
}

#[test]
#[cfg_attr(
  miri,
  ignore = "roughly two thousand renders of the whole case table, one per budget in the sweep. The \
            refusal path itself stays interpreted by `Budgeted` in `src/terminal/tests.rs`"
)]
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
    // Every presentation. The reset discipline is the PAINTER's and the styles share it, which is
    // exactly why it is worth asking once per style: a style writes rows of its own and some of
    // them — the boxed underline and the label hanging off it, the arrow that crosses a margin —
    // open two styled runs where another opens one, so "the body did not reach the reset" has a
    // further shape to happen in for each.
    for styled in [
      terminal.with_tab_width(case.tab_width).like_rustc(),
      terminal.with_tab_width(case.tab_width).like_miette(),
      terminal.with_tab_width(case.tab_width).like_ariadne(),
      terminal.with_tab_width(case.tab_width).like_codespan(),
    ] {
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
  #[cfg_attr(
    miri,
    ignore = "a size bound over six of the eight cases, two of them twenty kilobytes"
  )]
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
      .saturating_mul(16);

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
  #[cfg_attr(
    miri,
    ignore = "a size bound over two twenty-kilobyte origins; over seven minutes under tree borrows \
              on the development machine, and unmeasured on any CI cell because none reached it"
  )]
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
  #[cfg_attr(
    miri,
    ignore = "a size bound over a twenty-thousand-byte line rendered at two label counts"
  )]
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
  #[cfg_attr(
    miri,
    ignore = "four hundred kilobytes of caller text through one render. This is the one that ended \
              the Miri run: it had not finished 3h14m in when the job hit its six-hour ceiling, on \
              every cell that got as far as starting it"
  )]
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
///
/// # How the two sides are sampled, because the first version of this flaked
///
/// It timed three iterations of each side, one loop after the other, and divided the totals. That
/// reported 2.12 on a contended macOS runner for a ratio that is 1.06 — enough to fail a bound of
/// two — and the reason is structural rather than bad luck. The numerator and the denominator are
/// DIFFERENT functions timed at DIFFERENT moments, so their noise does not cancel the way it does
/// in `the_geometry_costs_the_window_and_not_the_line`, where both sides are one function at two
/// sizes. A total over three iterations also has no defence at all against one interruption.
///
/// So: the two sides ALTERNATE, so that a runner which slows down partway through slows both; and
/// each side keeps its FASTEST iteration rather than its total. Both are pure deterministic
/// computation over warm memory, so the true cost is what the machine does when nothing interrupts
/// it, and every source of noise here is one-sided — an interruption can only add time. The
/// fastest sample of several is the standard estimator for that, and one clean sample per side is
/// all it needs.
fn passes_over_layer_two(megabytes: usize, offset: usize, positions: usize) -> f64 {
  /// Enough that one clean sample of each side is overwhelmingly likely, and few enough that this
  /// stays affordable: `cargo hack test --feature-powerset` runs this test once per feature
  /// combination that includes `terminal`.
  const ROUNDS: usize = 5;

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
  let inputs = [Input::new(Source::new(&text))];

  let resolve = || core::hint::black_box(source.resolve(span).lines().next());
  let render = || {
    let mut out = String::new();
    Terminal::plain()
      .render(&diagnostic, &inputs, &mut out)
      .expect("a String is writable");
    core::hint::black_box(out);
  };

  // Warmed first, so neither side pays for the other's cold pages or first-call code paths.
  resolve();
  render();

  let mut layer_two = f64::MAX;
  let mut rendered = f64::MAX;
  for _ in 0..ROUNDS {
    let started = std::time::Instant::now();
    resolve();
    layer_two = layer_two.min(started.elapsed().as_secs_f64());

    let started = std::time::Instant::now();
    render();
    rendered = rendered.min(started.elapsed().as_secs_f64());
  }
  rendered / layer_two
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
  //   near span   k=1 1.05x   k=8 1.05x
  //   far span    k=1 1.02x   k=8 1.02x
  //
  // Repeated five times end to end, the widest reading of any of the four was 1.057 and the
  // narrowest 1.016 — a spread of about one and a half per cent, where the version this replaced
  // could report 2.12 for the same property.
  //
  // # What the bound can and cannot discriminate, stated rather than implied
  //
  // Two and a half, and it is worth being exact about what that buys, because the first draft of
  // this claimed more than a wall clock can deliver and failed on a contended runner for it.
  //
  // It catches what this test exists for, by a wide margin. With the carried cursor removed so
  // that each label resolves from the top again, k=8 measures 8.16x near and 5.79x far — two to
  // three times the bound — while both ends of k=1 stay green under that same plant, at 1.05x and
  // 1.02x, which is the whole reason the k axis exists. Restoring the header's grapheme walk
  // measures around 16x.
  //
  // It does NOT discriminate a single extra pass over the input, which would land near 2.0 and
  // inside the noise this instrument has. Pretending otherwise is what a tighter number here would
  // be doing. A pass that cannot be seen at this resolution has to be caught by reading the code,
  // or by the shape tests above that count output rather than time it.
  const ALLOWED: f64 = 2.5;

  // Re-measured once before failing. A genuine regression is deterministic and fails both times; a
  // scheduling hiccup that survives the fastest-of-five sampling twice in a row is not a thing this
  // suite should be reporting as a defect in the renderer. It costs nothing on the passing path.
  let measure = |what: &str, offset: usize, positions: usize| {
    let first = passes_over_layer_two(8, offset, positions);
    if first < ALLOWED {
      return;
    }
    let again = passes_over_layer_two(8, offset, positions);
    assert!(
      again < ALLOWED,
      "a render of {positions} positions {what} cost {first:.2} and then {again:.2} times layer 2 \
       alone; one forward pass over the input is the bound, and it does not widen with the label \
       count"
    );
  };

  for positions in [1, 8] {
    measure("at the start of the line", 0, positions);
    measure("along the line", 8_000_000 - 10, positions);
  }
}
