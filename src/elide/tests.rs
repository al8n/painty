use super::{CONTEXT, shown_between};

/// Every clause of the rule, over a grid, against an independent statement of it.
///
/// # Why the oracle is spelled out rather than reused
///
/// Both renderers call [`shown_between`], so a change to it moves them **together** and every
/// cross-renderer property still passes — which is what sharing a rule is for and also what makes
/// those properties structurally unable to pin it. Planting the three clauses one at a time proved
/// that: removing the one-line remainder and halving [`CONTEXT`] reddened the two renderers' own
/// behavioural tests, and dropping the bracket condition reddened **nothing anywhere**.
///
/// So the rule needs a pin that does not go through either renderer, and the oracle has to come
/// from the sentence rather than from the code: context only after a bracket, cut at the next
/// anchor, and one line left over drawn rather than hidden.
#[test]
fn the_rule_is_its_three_clauses_and_nothing_else() {
  for previous in 1..=6u64 {
    for gap in 1..=15u64 {
      let next = previous + gap;
      for opens_a_bracket in [false, true] {
        let shown = shown_between(previous, next, opens_a_bracket);

        // Clause 1: context is earned by a bracket and by nothing else.
        let reach = if opens_a_bracket {
          previous + CONTEXT
        } else {
          previous
        };
        // Clause 2: it never reaches the next anchor.
        let capped = reach.min(next - 1);
        // Clause 3: one line left over is drawn rather than stood over.
        let expected = if next - capped == 2 { next - 1 } else { capped };

        assert_eq!(
          shown, expected,
          "shown_between({previous}, {next}, {opens_a_bracket})"
        );

        // And the two things every caller reads off it, stated as ranges rather than derived
        // again: nothing before the anchor itself, nothing past the one after.
        assert!(
          (previous..next).contains(&shown),
          "shown_between({previous}, {next}, {opens_a_bracket}) = {shown} is outside \
           {previous}..{next}"
        );
        // A gap stands for at least two lines, never for one and never for none.
        let elided = next - 1 - shown;
        assert_ne!(
          elided, 1,
          "shown_between({previous}, {next}, {opens_a_bracket}) leaves one line to stand a gap over"
        );
      }
    }
  }
}

/// An anchor that opens no bracket is followed straight by the gap.
///
/// The clause no renderer test reached. Two labels ten lines apart draw their two lines and nothing
/// between them, where a rule that gave context to every anchor would draw three more.
#[test]
fn an_anchor_with_no_bracket_earns_no_context() {
  assert_eq!(shown_between(1, 10, false), 1);
  assert_eq!(shown_between(1, 10, true), 1 + CONTEXT);
  // Except where the one-line remainder overrides it, which is the one case the two agree on.
  assert_eq!(shown_between(1, 3, false), 2);
  assert_eq!(shown_between(1, 3, true), 2);
}

/// The degenerate call answers "nothing between them" rather than panicking or wrapping.
///
/// Both callers keep `next > previous` where they build their anchors, so this is unreachable — and
/// it is written down as behaviour rather than as a `debug_assert` for the reason a whole review
/// round was about: an assertion that only exists in debug leaves the build that ships subtracting
/// past zero, and `next - 1` at `next == 0` is `u64::MAX`, which is a row count that quietly stopped
/// being true.
#[test]
fn a_degenerate_pair_draws_nothing_between_and_does_not_wrap() {
  for previous in [1u64, 2, 7, u64::MAX] {
    for opens_a_bracket in [false, true] {
      assert_eq!(shown_between(previous, previous, opens_a_bracket), previous);
      if previous > 1 {
        assert_eq!(
          shown_between(previous, previous - 1, opens_a_bracket),
          previous
        );
      }
      assert_eq!(shown_between(previous, 0, opens_a_bracket), previous);
    }
  }
}
