#!/usr/bin/env bash
# **The HEAVY tier at the amended wave's boundary.**
#
#     bash docs/experiments/overlap-ics/economics-round/gate2/heavy.sh
#
# The four pinned engine gates on both builds, the five release suites, and
# determinism in both of its forms - the required same-feature-set cell and the
# REAL two-binary comparison against a genuinely different executable. Wave 4's
# `../gate/heavy.sh` ran the same floor at the round's first boundary and found
# that the required cell was comparing a binary with itself; the cross-feature
# document is the one that carries the claim, and it is run here too.
#
# This wave *did* produce gate numbers, so the floor is not a consolation
# report: it is what says the engine-side change this wave carries - a purely
# additive `U'` section in the METER, which no trajectory can reach - cost the
# member nothing.
#
# Do NOT pipe this into `tee` or `tail`: you would read the pipe's status and
# not the script's. Every exit status below is captured on its own line
# immediately after its command.
#
# **THE TRAP, and what this script does about it.** `run-suites.sh` builds into
# the shared `target/`, so by the time it finishes,
# `target/release/examples/overlap_ics_benchmark` has been rebuilt under
# *another* feature set. `../gate/README.md` §4 records it. Two consequences
# are handled here rather than left to whoever runs this next:
#
#   1. every determinism cell runs BEFORE the suites, and
#   2. the canonical example's sha256 is recorded before and after the suite
#      step and compared. `SUITE_CLOBBERED_THE_BINARY` is a field of
#      `evidence/binary-trap.txt`, so the trap is a measurement rather than a
#      paragraph. The gate battery's own plan is keyed to that sha, and
#      `gate.py` records the binary's hash on both sides of every battery for
#      the same reason.

set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="${ICS_ROOT:-$(cd "$SCRIPT_DIR" && git rev-parse --show-toplevel)}"
export ICS_ROOT="$ROOT"
OUT="${ICS_OUT:-/var/lib/t3/tmp/overlapics/gate2heavy}"
DRIVERS="$ROOT/docs/experiments/overlap-ics/drivers"
EV="$SCRIPT_DIR/evidence"
LOG="$OUT/log"
CANONICAL="$ROOT/target/release/examples/overlap_ics_benchmark"
mkdir -p "$LOG" "$EV"
cd "$ROOT" || exit 3

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
FAILURES=0

note() { echo "[heavy] $1"; }
record() {
  local name="$1"; local status="$2"
  if [ "$status" -eq 0 ]; then note "$name EXIT=$status"
  else note "$name EXIT=$status  <-- NONZERO"; FAILURES=$((FAILURES + 1)); fi
}

# --------------------------------------------------------------- the FAST tier --
bash "$DRIVERS/fast.sh" > "$EV/fast-tier-stdout.txt" 2>&1
record "FAST union" $?

# ------------------------------------------------------------- the binaries --
cargo build --release --features jagua-experimental --example general_request_benchmark > "$LOG/build-base.log" 2>&1
record "base binary build (overlap-ics absent)" $?
cp "$ROOT/target/release/examples/general_request_benchmark" "$OUT/base-binary"

cargo build --release --features jagua-experimental,overlap-ics --example general_request_benchmark > "$LOG/build-meas.log" 2>&1
record "meas binary build (overlap-ics compiled, unarmed)" $?
cp "$ROOT/target/release/examples/general_request_benchmark" "$OUT/meas-binary"

sha256sum "$OUT/base-binary" "$OUT/meas-binary" > "$EV/binaries.txt" 2>&1
record "binary hashes" $?

# ----------------------------------------------------------------- the gates --
python3 "$DRIVERS/gates.py" base "$OUT/base-binary" "$OUT/gates/base" > "$LOG/gates-base.log" 2>&1
record "four pinned gates on the base binary" $?

python3 "$DRIVERS/gates.py" meas "$OUT/meas-binary" "$OUT/gates/meas" > "$LOG/gates-meas.log" 2>&1
record "four pinned gates on the meas binary" $?

python3 "$DRIVERS/gatecompare.py" "$OUT/gates/base" "$OUT/gates/meas" > "$EV/gates.json" 2>&1
record "gate comparison (whole-document identity)" $?

# --------------------------------------------------- two-binary determinism --
# The REQUIRED cell first: same source, same feature set, two target
# directories. On this toolchain that produces byte-identical executables, so
# it re-proves single-binary determinism and `binariesDiffer: false` says so.
CARGO_TARGET_DIR="$ROOT/target/g2-determinism" cargo build -p polygon-nesting-core \
  --release --features overlap-ics --example overlap_ics_benchmark \
  > "$LOG/build-determinism-b.log" 2>&1
record "second independent overlap-ics build" $?

ICS_OUT="$OUT" python3 "$DRIVERS/determinism.py" \
  "$CANONICAL" \
  "$ROOT/target/g2-determinism/release/examples/overlap_ics_benchmark" \
  > "$EV/determinism-two-binary.json" 2>&1
record "two-binary determinism, required cell (same feature set)" $?

# Then the REAL one: two genuinely different executables. More than the
# converged spec claims - it fixes the feature set - so it is supplementary
# rather than required, and it is the evidence the required cell cannot give.
CARGO_TARGET_DIR="$ROOT/target/g2-crossfeature" cargo build -p polygon-nesting-core \
  --release --features jagua-experimental,overlap-ics --example overlap_ics_benchmark \
  > "$LOG/build-crossfeature.log" 2>&1
record "cross-feature-set build (jagua-experimental,overlap-ics)" $?

ICS_OUT="$OUT" python3 "$DRIVERS/determinism.py" \
  "$CANONICAL" \
  "$ROOT/target/g2-crossfeature/release/examples/overlap_ics_benchmark" \
  > "$EV/determinism-cross-featureset.json" 2>&1
record "cross-feature-set determinism (genuinely different executables)" $?

# -------------------------------------------------- the trajectory did not move --
# The claim this wave most needs is that its engine-side change cannot reach a
# trajectory. It is a **source** claim before it is a measurement one: the only
# edit is an additive `U'` section in `search/overlap_ics_meter/currency.rs` and
# its reader in `examples/ics_meter.rs`, and `search/overlap_ics/` - the whole
# trajectory, `run_cutclose`, the pacer wiring and the document schema - is
# byte-for-byte identical to the wave's base commit. `git diff` over that
# directory must be empty, and an empty diff is a fact rather than an argument.
#
# The measured half is the four pinned gates on both builds and the two
# determinism documents above. The one thing NOT re-measured here is
# `integration/armgate.py`'s cross-binary arm comparison against the round's
# base binary: it needs a second checkout of `6e9c2e5`, and this run is
# isolated to one worktree. `../gate/README.md` carries that measurement from
# the wave that made it, and the source claim below is what this wave adds.
BASE_REF="${ICS_BASE_REF:-e4da8c5}"
{
  echo "# git diff $BASE_REF..HEAD over the trajectory. Empty is the claim."
  git -C "$ROOT" diff --stat "$BASE_REF" -- \
    crates/polygon-nesting-core/src/search/overlap_ics/
  echo "# names only:"
  git -C "$ROOT" diff --name-only "$BASE_REF" -- \
    crates/polygon-nesting-core/src/search/overlap_ics/
  echo "# and the whole engine-side diff of this wave, for contrast:"
  git -C "$ROOT" diff --stat "$BASE_REF" -- crates/
} > "$EV/trajectory-unchanged.txt" 2>&1
record "trajectory diff recorded" $?

changed=$(git -C "$ROOT" diff --name-only "$BASE_REF" -- \
  crates/polygon-nesting-core/src/search/overlap_ics/ | wc -l)
if [ "$changed" -eq 0 ]; then
  echo "TRAJECTORY_UNCHANGED: true" >> "$EV/trajectory-unchanged.txt"
else
  echo "TRAJECTORY_UNCHANGED: false ($changed files)" >> "$EV/trajectory-unchanged.txt"
fi
record "search/overlap_ics/ byte-for-byte unchanged since $BASE_REF" \
  "$([ "$changed" -eq 0 ] && echo 0 || echo 1)"

# ---------------------------------------------------------------- the suites --
sha256sum "$CANONICAL" > "$OUT/canonical-before.txt"
bash "$DRIVERS/run-suites.sh" > "$EV/suites-stdout.txt" 2>&1
record "five release suites" $?
sha256sum "$CANONICAL" > "$OUT/canonical-after.txt"

python3 "$DRIVERS/suitetotals.py" > "$EV/suites.json" 2>&1
record "suite totals" $?

for name in jagua combo example overlap-ics overlap-ics-stacked; do
  cp "$ROOT/docs/experiments/overlap-ics/evidence/suite-$name.log" "$EV/suite-$name.log" 2>/dev/null
done
git -C "$ROOT" checkout -- docs/experiments/overlap-ics/evidence
record "campaign suite logs restored" $?

# ------------------------------------------------------------------ the trap --
{
  echo "# §4 trap: run-suites.sh rebuilds the shared target/, so any driver that"
  echo "# takes lib.BIN's default after a suite run measures a binary nobody named."
  echo "before: $(cat "$OUT/canonical-before.txt")"
  echo "after:  $(cat "$OUT/canonical-after.txt")"
  if cmp -s "$OUT/canonical-before.txt" "$OUT/canonical-after.txt"; then
    echo "SUITE_CLOBBERED_THE_BINARY: false"
  else
    echo "SUITE_CLOBBERED_THE_BINARY: true"
  fi
  echo "# The gate battery ran BEFORE this script, against the 'before' sha, and"
  echo "# gate.py records the binary's hash on both sides of every battery."
} > "$EV/binary-trap.txt"
record "binary trap recorded" $?

# The canonical example is rebuilt so the tree is left with the binary its
# plan is keyed to, rather than with whatever the last suite happened to build.
cargo build -p polygon-nesting-core --release --features overlap-ics \
  --example overlap_ics_benchmark --example ics_meter > "$LOG/rebuild-canonical.log" 2>&1
record "canonical example rebuilt after the suites" $?
sha256sum "$CANONICAL" >> "$EV/binary-trap.txt"

note "logs in $LOG"
note "evidence in $EV"
note "FAILURES=$FAILURES"
exit "$FAILURES"
