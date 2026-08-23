#!/usr/bin/env bash
# **The HEAVY tier at the economics round's boundary.**
#
#     bash docs/experiments/overlap-ics/economics-round/gate/heavy.sh
#
# The four pinned engine gates on both builds, the five release suites, and
# two-binary determinism on this round's own member. It is the mechanical floor
# ox-alpha review 1 §Q3 calls "mandatory, stop-not-report", and it is run here
# for a reason worth stating plainly: **wave 4 stopped at the currency reject
# rule and produced no gate number.** A round that stops still has to say what
# it did to the tree on the way, and three waves of engine work sit between
# `6e9c2e5` and this commit. This is the evidence that they cost the member
# nothing.
#
# Do NOT pipe this into `tee` or `tail`: you would read the pipe's status and
# not the script's. Every exit status below is captured on its own line
# immediately after its command.
#
# `ROOT` resolves to the repository containing THIS FILE. The pivot round's
# copy of this script hard-coded a worktree, which Sol review 17 Round 2 §2
# names a round-validity defect rather than a knob - the four pinned gates are
# exactly the regression floor a wrong-tree run would silently launder, and on
# this box the wrong tree still exists, so the failure would be silent.
# `ICS_ROOT` still overrides explicitly.
#
# **The suite logs.** `run-suites.sh` writes its five logs into the campaign's
# committed `overlap-ics/evidence/` directory by construction. This script
# copies them into the round's own evidence and then restores the committed
# ones with `git checkout`, exactly as the verification and pivot rounds did,
# so both rounds' logs survive and neither silently overwrites the other.

set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="${ICS_ROOT:-$(cd "$SCRIPT_DIR" && git rev-parse --show-toplevel)}"
export ICS_ROOT="$ROOT"
OUT="${ICS_OUT:-/var/lib/t3/tmp/overlapics/w4heavy}"
DRIVERS="$ROOT/docs/experiments/overlap-ics/drivers"
EV="$SCRIPT_DIR/evidence"
LOG="$OUT/log"
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

# ------------------------------------------------------------- the binaries --
# `base` is the pinned gate binary's own feature set with this round's feature
# ABSENT. `meas` has it COMPILED and unarmed: nothing outside
# `overlap_ics_benchmark` can reach `search::overlap_ics`, so the four gates
# must not move by one bit of their documents between the two. That is the
# claim, and `gatecompare.py` checks it on whole documents rather than on the
# four pinned scalars alone.
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
# The other half of the determinism claim. `smoke.py`'s two-process comparison
# catches ordering and allocation nondeterminism inside ONE binary; this
# catches a build that differs. The second binary is an independent build of
# the SAME feature set in its own target directory, so everything the
# trajectory can see is identical and only the executable's own sha differs -
# which is why `determinism.py` strips `executableSha256` and would otherwise
# be trivially false.
CARGO_TARGET_DIR="$ROOT/target/w4-determinism" cargo build -p polygon-nesting-core \
  --release --features overlap-ics --example overlap_ics_benchmark \
  > "$LOG/build-determinism-b.log" 2>&1
record "second independent overlap-ics build" $?

ICS_OUT="$OUT" python3 "$DRIVERS/determinism.py" \
  "$ROOT/target/release/examples/overlap_ics_benchmark" \
  "$ROOT/target/w4-determinism/release/examples/overlap_ics_benchmark" \
  > "$EV/determinism-two-binary.json" 2>&1
record "two-binary determinism (five cells, incl. this round's member)" $?

# ---------------------------------------------------------------- the suites --
bash "$DRIVERS/run-suites.sh" > "$EV/suites-stdout.txt" 2>&1
record "five release suites" $?

# `suitetotals.py` reads the campaign evidence directory by construction, so it
# runs while this round's logs are still sitting there.
python3 "$DRIVERS/suitetotals.py" > "$EV/suites.json" 2>&1
record "suite totals" $?

for name in jagua combo example overlap-ics overlap-ics-stacked; do
  cp "$ROOT/docs/experiments/overlap-ics/evidence/suite-$name.log" "$EV/suite-$name.log" 2>/dev/null
done
git -C "$ROOT" checkout -- docs/experiments/overlap-ics/evidence
record "campaign suite logs restored" $?

note "logs in $LOG"
note "evidence in $EV"
note "FAILURES=$FAILURES"
exit "$FAILURES"
