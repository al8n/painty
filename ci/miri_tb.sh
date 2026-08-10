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
#     space`. No `MIRIFLAGS` entry raises that ceiling.
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
cargo miri test -p painty --lib --tests --all-features --target "$TARGET"
