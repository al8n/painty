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
cargo miri test -p painty --all-targets --all-features --target "$TARGET"
