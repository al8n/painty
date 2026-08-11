//! How many lines are left out between two drawn ones, stated once for every output.
//!
//! # Why this is a module and not a rule each renderer keeps
//!
//! It was two copies for exactly one review round, and the divergence they produced is the reason
//! this file exists. The terminal decided elision over a whole input's **anchors** — every line an
//! end of any span falls on — while the HTML renderer decided it inside each mark, from that span's
//! own two ends. A seven-line span with a secondary label on line five came out as all seven lines
//! in one output and as `1 2 3 4 ⋮ 7` in the other, with **line six in neither**.
//!
//! That was not a drift between two statements of one rule. Both copies computed the same
//! arithmetic; they applied it to different inputs. So the repair is both halves: the arithmetic
//! lives here, and both renderers feed it the same thing — the anchors of one input, in order.
//!
//! # The rule needs three numbers, and finding that out is what made it shareable
//!
//! The terminal used to carry a sorted `Vec<(first, reach)>` of every multi-line span's context
//! reach, where `reach = min(first + CONTEXT, last - 1)`, and look up the maximum reach at the
//! previous anchor. **None of that survives contact with the clamp it feeds.** The result is
//! immediately clamped to `next - 1`, and `last` is itself an anchor strictly greater than the
//! `previous` the span opens at — so `last >= next`, so `last - 1 >= next - 1`, so the `last` term
//! is always dominated and drops out. Every span opening at one anchor therefore reaches the same
//! place, and the maximum over them is that place.
//!
//! What is left is [`shown_between`]: the previous anchor, the next anchor, and one boolean saying
//! whether any bracket opens at the previous one. Three numbers, no allocation, no ordering — which
//! is why a renderer with no heap can call the same function a renderer with one does. The `Vec`
//! and the binary search are gone from the terminal as well; a linear scan over the caller's own
//! labels replaces them, and it costs nothing at the sizes a diagnostic has.

#[cfg(test)]
mod tests;

/// How many lines after a bracket's opening are shown before the rest are elided.
///
/// Three, and it is a layout rule rather than a budget. What it buys is that the drawn row count is
/// a function of the LABEL count: a two-byte span reaching across a ten-megabyte file costs its
/// opening, three lines, a gap and its closing, and not ten million rows.
pub(crate) const CONTEXT: u64 = 3;

/// The last line drawn between two anchors, which is `previous` itself when nothing is drawn
/// between them.
///
/// `opens_a_bracket` says whether any span that reaches a later line **opens on** `previous`. Only
/// such a span earns context after it; an anchor that is merely where some other span ends, or
/// where a single-line label sits, is followed straight by the gap.
///
/// # The one-line remainder
///
/// One line left over between the context and the next anchor reads worse as a gap than as the line
/// itself — a `⋮` standing for a single row says less than the row does, and costs the same space.
/// It is also the shape a span of exactly six lines leaves behind, which is why a six-line span is
/// drawn whole and a seven-line one is not.
///
/// # Panics
///
/// Never in practice: `next > previous` for every caller, because the anchors of one input are
/// strictly increasing. `next - 1` and `next - shown` would underflow otherwise, and a debug build
/// says so rather than handing back a row count that quietly stopped being true.
pub(crate) const fn shown_between(previous: u64, next: u64, opens_a_bracket: bool) -> u64 {
  debug_assert!(next > previous, "anchors are strictly increasing");
  let context = if opens_a_bracket {
    previous + CONTEXT
  } else {
    previous
  };
  let last = next - 1;
  let mut shown = if context < last { context } else { last };
  if next - shown == 2 {
    shown = last;
  }
  shown
}
