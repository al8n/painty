#!/usr/bin/env bash
#
# Sanitizer legs. Nightly only (`-Z sanitizer`, `-Zbuild-std`).
#
# Run by the `sanitizer` matrix in `.github/workflows/ci.yml`, one cell per sanitizer:
#
#     ci/sanitizer.sh                               # the default sanitizer on the CI target
#     ci/sanitizer.sh aarch64-apple-darwin address  # one leg, one target
#
# ## The target is an argument, not a constant
#
# It used to be hard-coded to `x86_64-unknown-linux-gnu`, which is right for the CI runner and
# wrong everywhere else. Which sanitizers exist is a property of the target, so the target belongs
# where the matrix can set it:
#
#     rustc +nightly --print target-spec-json -Z unstable-options --target <t> | grep supported-sanitizers
#
# ## Why the default is `address` alone
#
# The script used to run address, leak, memory and thread in one job. Three of those are dropped,
# for two different reasons, and both are worth stating so the list is not silently re-grown.
#
# `leak` and `memory` are unsupported on some targets. An unsupported leg reports as a failure
# rather than as the platform fact it is, and pinning it as an expected failure inverts silently
# the day support lands. They stay available as an explicit second argument.
#
# `thread` is dropped on the crate's own terms rather than the platform's. `painty` renders: it
# spawns nothing, shares nothing across threads, and the design rules out owning any terminal
# state. ThreadSanitizer additionally needs `-Zbuild-std` for an instrumented `std`, so the leg
# would rebuild the standard library on every push in order to observe a program that by contract
# cannot race. If a future output ever does hold shared state, add `thread` back to the matrix in
# `ci.yml` — this script already handles it, and the `-Zbuild-std` branch below is the reason it
# still can.
#
# ## `-Zbuild-std` is not optional for thread/memory, and dropping it looks like a finding
#
# The shipped `std` is uninstrumented. ThreadSanitizer and MemorySanitizer need an instrumented
# one, so those legs rebuild it. Without the flag the build fails with an ABI/interceptor mismatch
# that READS LIKE a sanitizer report — the shape that gets misdiagnosed as a real defect, so it is
# written down here rather than rediscovered.
set -eu

TARGET="${1:-x86_64-unknown-linux-gnu}"
SANITIZERS="${2:-address}"

export ASAN_OPTIONS="detect_odr_violation=0 detect_leaks=0"

for san in $SANITIZERS; do
  echo "=== sanitizer: ${san} on ${TARGET} ==="
  case "$san" in
    memory | thread)
      # instrumented std — see the note above before removing `-Zbuild-std`
      RUSTFLAGS="-Z sanitizer=${san}" \
        cargo -Zbuild-std test -p painty --tests --target "$TARGET" --all-features
      ;;
    *)
      RUSTFLAGS="-Z sanitizer=${san}" \
        cargo test -p painty --tests --target "$TARGET" --all-features
      ;;
  esac
done
