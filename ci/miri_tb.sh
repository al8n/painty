#!/usr/bin/env bash
#
# Miri under TREE BORROWS. The Stacked Borrows twin is `ci/miri_sb.sh`; the two differ by exactly
# one `MIRIFLAGS` entry and are kept as separate files so that a failure names which aliasing
# model rejected the program.
#
# Run by the `miri-tb` matrix in `.github/workflows/miri.yml`, one cell per target.
#
#     ci/miri_tb.sh x86_64-unknown-linux-gnu
#
# WHY THIS EXISTS ON A CRATE WITH NO `unsafe`. It does not yet have anything to find, and that is
# the point of installing it now: the design's last phase exports the resolved model across a C
# ABI, and its gate is that the ownership rule round-trips and holds under Miri. A gate added
# alongside the code it checks is a gate whose shape was chosen to suit that code.
#
# THE HOST IS NOT THE TARGET. Every cell runs on `ubuntu-latest`, apple targets included: Miri
# interprets a target, so the host only matters when something in the dependency graph has to be
# built natively — a C build script, typically. `painty` has no build-time C anywhere and, at the
# time of writing, no dev-dependencies at all, so the graph is pure Rust and the host is free.
# smear learned this the expensive way: `criterion` pulled in a `cc::Build` and pinned its apple
# Miri cells to a macOS runner that queued for hours. Check the dependency graph before assuming
# this still holds.
set -euo pipefail

if [ -z "${1:-}" ]; then
  echo "Error: TARGET is not provided" >&2
  exit 1
fi

TARGET="$1"

# Only the targets this matrix actually names. A `case` arm for a target no cell runs is a claim
# nothing checks; a cell with no arm fails loudly at link time, which is the direction to fail in.
if [ "$(uname)" = "Linux" ]; then
  case "$TARGET" in
    i686-unknown-linux-gnu)
      sudo apt-get update && sudo apt-get install -y gcc-multilib
      ;;
    powerpc64-unknown-linux-gnu)
      sudo apt-get update && sudo apt-get install -y gcc-powerpc64-linux-gnu
      ;;
  esac
fi

rustup toolchain install nightly --component miri
rustup override set nightly
cargo miri setup --target "$TARGET"

# `cargo-nextest` is what runs the tests — see the note above the invocation for why. The Miri
# workflow installs it with `taiki-e/install-action`; a local run of this script has to have it
# already, and saying so here is cheaper than reading cargo's `no such command` from inside a
# wrapper that is three tools deep.
if ! cargo nextest --version >/dev/null 2>&1; then
  echo "Error: cargo-nextest is not installed (cargo install --locked cargo-nextest)" >&2
  exit 1
fi

export MIRIFLAGS="-Zmiri-strict-provenance -Zmiri-disable-isolation -Zmiri-symbolic-alignment-check -Zmiri-tree-borrows"

# `-p painty` NAMES the package rather than letting the selection default. A cargo target filter
# that matches nothing warns and exits 0; a named package that has been renamed is
# `error: package ID specification did not match any packages`, exit 101. `--all-features` because
# the three outputs are independently selectable and the resolution layer they share is what has
# the pointer arithmetic in it.
#
# `--lib --tests` AND NOT `--all-targets`, which also selects benches. The two select the same
# thing today, because painty declares no bench targets — and that is exactly why the flag has to
# change now rather than when it starts mattering. The first `[[bench]]` anybody adds reddens both
# Miri legs with `can't call foreign function posix_spawnattr_init`, an error naming a libc symbol
# and pointing nowhere near a bench harness. cronp is red on precisely this. `--lib` and `--tests`
# both name targets that exist, so neither is a filter that would warn and exit 0.
#
# `ci/miri_sb.sh` carries the same line and has to keep carrying it: a fix that lands in one of two
# sibling scripts and not the other is a defect this program keeps repeating.
#
# WHAT `--tests` SELECTS AND DOES NOT INTERPRET. Two integration targets opt out from inside the
# file, and they are named here because a reader of the line below would otherwise take `--tests` at
# its word. Nine units are selected and nine are built; seven of them run whole, one runs nothing at
# all, and one is down to a single test:
#
#   * `tests/numeric_widths.rs` is `#![cfg(not(miri))]`, so it builds and runs zero tests. It is a
#     `syn` census over painty's source TEXT and walks none of painty's own paths, so there is
#     nothing in it for an interpreter to have an opinion about. It cost between 2h22m and 4h58m per
#     cell in run 31316096247, and on `i686-unknown-linux-gnu` it did not finish at all: Miri hands
#     every allocation a fresh address out of the target's four gigabytes, a census this long
#     exhausts them, and validation ICEs with `there are no more free addresses in the address
#     space`. No `MIRIFLAGS` entry raises that ceiling. That is the 4 GiB ceiling, on ALLOCATION;
#     the note above the invocation below is about a second, lower one at 2 GiB, on REFERENCE
#     VALIDITY, which is what a 32-bit cell reaches first.
#   * `tests/writer_discipline.rs` keeps one test and `#[cfg_attr(miri, ignore)]`s the rest. Its case
#     table is sized against a 65,536-byte budget, and the ignored tests render it between six and
#     roughly two thousand times each or build their own inputs of the same order. The only one CI
#     ever managed to measure took 7m37s on the quickest cell and 48m34s on the slowest, and no cell
#     got past the test after it before the ceiling.
#
# Neither is dropped from CI, only from the interpreter: `cargo hack test -p painty
# --feature-powerset` in `.github/workflows/ci.yml` runs both in full on three operating systems, as
# do the coverage and sanitizer jobs. Each file's header says what still covers its paths here.
#
# The exclusions live in the FILES and not in a `--test a --test b` list on this line, deliberately.
# A cargo target filter that stops matching warns and exits 0, so a list here would go quietly wrong
# the first time a target is renamed — and a reader of the test would have no way to learn that it
# never runs. In the file, the reason is where the reader already is.
#
# ── WHY `nextest run` AND NOT `test` ───────────────────────────────────────────────────────────
#
# `cargo miri test` runs a whole test binary inside ONE interpreter, and a Miri address only ever
# goes up. Every allocation is handed a fresh base address out of a generator that starts near zero
# and marches upward, and a stack allocation's address is never returned to the reuse pool at all
# (`ReusePool::add_addr` drops `MemoryKind::Stack` on the floor, because there are too many of
# them). So a binary's address consumption is CUMULATIVE over every test in it, and it tracks how
# much the suite interprets rather than how much it holds live at any moment.
#
# On a 32-bit target that hits a ceiling, and the ceiling is 2 GiB — half the address space, not
# all of it. A reference whose dereferenceability the compiler is not allowed to assume, which is
# what `MaybeDangling` marks and therefore what every `mem::forget` and `ManuallyDrop::new` goes
# through, is still validated for one thing: that `address + size of pointee` can be computed. That
# computation is `Size::checked_add`, bounded by `obj_size_bound()`, which is `1 << 31` on a 32-bit
# target. Past 2 GiB every such reference is rejected as
#
#     encountered a reference that is too close to the end of the address space
#     for a pointee of N bytes
#
# and the frame named is whichever std internal got there first — `Vec::dedup_by`'s `FillGapOnDrop`,
# `Box<[T]>::assume_init`, a float formatter's `Part`. It is never painty's frame, painty has no
# `unsafe` on any of those paths, and the same code passes when the test is run on its own.
#
# The numbers, measured on `i686-unknown-linux-gnu` with the `MIRIFLAGS` above. At `49aad26` the
# lib binary ran 101 tests and consumed 742 MiB of address space end to end, 36% of the ceiling.
# `9e75614` added four tests — 101 to 105 — and multiplied what the existing ones interpret; that
# binary now passes 2048 MiB partway through its 101st test, so cumulative consumption rose by at
# least 2.8x, and the CI cell's lib-binary wall clock rose 2.5x alongside it (1445s to 3679s).
# A probe crate holding no painty code brackets the boundary: `Vec::dedup_by` still succeeds at a
# watermark of `0x7c64b09c` and the next reference validated past `0x80688047` is rejected, with
# `1 << 31` = `0x80000000` between the two.
#
# NO `MIRIFLAGS` ENTRY RAISES IT, and the two that look like they should do not. Measured with the
# reuse rate forced to 1.0, a heap-churn loop consumed 2.3% less address space and a stack-churn
# loop 0.02% less. Miri does have a mitigation — `ReusePool::address_space_shortage` forces 100%
# reuse — but it is armed only once the generator passes half the address space, which on a 32-bit
# target is the exact point at which reference validation has already started failing. It cannot
# arrive in time here.
#
# So the only lever is HOW MANY TESTS SHARE ONE ADDRESS SPACE, and that is what nextest changes: one
# interpreter per test, each starting from a fresh watermark near zero. Nothing is skipped, nothing
# is narrowed, no cell is dropped and no flag is relaxed — the same tests run on the same targets
# under the same `MIRIFLAGS`. It is also faster rather than slower, because Miri is single-threaded
# per interpreter and nextest can run interpreters in parallel. On one developer machine, with the
# runs contending for it so the ratio is indicative rather than clean: this leg went from 34m43s
# spent reaching a failure at the 100th of the lib binary's 105 tests, to 16m20s for all 168 tests
# in every binary; the Stacked Borrows leg, from 21m35s to 9m34s.
#
# WHAT IT COSTS, so that it is not read as free. Two things.
#
#   * A one-process-per-test model cannot see a data race BETWEEN tests. painty has one `static`
#     that more than one test could reach — `HANDED` in `src/terminal/tests.rs` — and its own
#     comment records that a single test touches it; no test spawns a thread. So nothing is lost
#     today. That is a property of this suite and not a law: a second test on `HANDED`, or a test
#     that spawns threads onto shared state, makes this paragraph wrong.
#   * nextest does not run doctests. `--lib --tests` never selected them, so this changes nothing;
#     `.github/workflows/ci.yml` is where doctests run.
#
# AND WHAT IS LEFT, WHICH IS A NUMBER AND NOT A HOPE. The ceiling is now per test rather than per
# binary, which is a bigger budget and not an unbounded one — and the worst test's share of it is
# already half. Measured on this leg, the heaviest test in the suite is the one in
# `tests/resolution_invariants.rs` that tiles the span it resolved over generated sources: 963 MiB
# on its own, 47% of the ceiling. The heaviest lib test, the one that permutes the caller order of
# the rows under a line, is 664 MiB, 32%. So this buys roughly 2x for the worst case and not orders
# of magnitude: doubling what that one test interprets reddens the 32-bit cells again, and nextest
# will have nothing left to give. The answer then is upstream, because a 32-bit Miri target that
# hands out addresses across a range whose top half no reference can be validated in — while its own
# reuse mitigation arms only at the same halfway point — is a defect in the interpreter and not in
# the suite that reached it.
#
# `ci/miri_sb.sh` carries the same invocation and has to keep carrying it.
cargo miri nextest run -p painty --lib --tests --all-features --target "$TARGET"
