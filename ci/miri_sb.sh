#!/usr/bin/env bash
#
# Miri under STACKED BORROWS. The Tree Borrows twin is `ci/miri_tb.sh`, whose header carries the
# reasoning the two share: why Miri is installed before there is any `unsafe` to check, and why
# every cell can run on a Linux host regardless of the target it interprets.
#
# The two files differ by exactly one `MIRIFLAGS` entry — this one omits `-Zmiri-tree-borrows` —
# and are kept apart so a failure names which aliasing model rejected the program.
#
# Run by the `miri-sb` matrix in `.github/workflows/miri.yml`, one cell per target.
#
#     ci/miri_sb.sh x86_64-unknown-linux-gnu
set -euo pipefail

if [ -z "${1:-}" ]; then
  echo "Error: TARGET is not provided" >&2
  exit 1
fi

TARGET="$1"

# Only the targets this matrix actually names; see the note in `ci/miri_tb.sh`.
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

export MIRIFLAGS="-Zmiri-strict-provenance -Zmiri-disable-isolation -Zmiri-symbolic-alignment-check"

# `-p painty` names the package, and `--lib --tests` keeps bench targets out of the interpreter;
# see the note in `ci/miri_tb.sh`, which this line is the sibling of and must stay identical to.
#
# That note also lists the two integration targets which `--tests` selects here and which interpret
# nothing or almost nothing — `tests/numeric_widths.rs`, whose `syn` census ICEd the 32-bit cell on
# Miri's address space, and `tests/writer_discipline.rs`, whose case table is sized for a
# 65,536-byte budget rather than for an interpreter. Both opt out from inside the file, so this
# script is not what excludes them and editing this line will not bring them back.
cargo miri test -p painty --lib --tests --all-features --target "$TARGET"
