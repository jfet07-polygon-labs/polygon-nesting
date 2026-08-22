#!/usr/bin/env bash
# The HEAVY tier at the round boundary: the four pinned gates on a fresh
# default build and on the feature build, the five suites, and the suite/gate
# summaries.
#
#     bash docs/experiments/overlap-ics/gate0-rerun/drivers/heavy.sh
#
# Do NOT pipe this into `tee` or `tail`. Every exit status is captured on its
# own line immediately after its command.
#
# `run-suites.sh` writes its five logs into the PREVIOUS round's committed
# `evidence/` directory. This script copies them into the re-run's evidence and
# then restores the previous round's with `git checkout`, exactly as the
# verification round did, so both rounds' logs survive and neither overwrites
# the other.

set -u

ROOT="${ICS_ROOT:-/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_72c0d771-eb6-1}"
OUT="${ICS_OUT:-/var/lib/t3/tmp/overlapics-rerun}"
DRIVERS="$ROOT/docs/experiments/overlap-ics/drivers"
EV="$ROOT/docs/experiments/overlap-ics/gate0-rerun/evidence"
LOG="$OUT/heavy"
mkdir -p "$LOG" "$EV"
cd "$ROOT"
FAILURES=0

note() { echo "[heavy] $1"; }
record() {
  local name="$1"; local status="$2"
  if [ "$status" -eq 0 ]; then note "$name EXIT=$status"
  else note "$name EXIT=$status  <-- NONZERO"; FAILURES=$((FAILURES + 1)); fi
}

# ------------------------------------------------------------- the binaries --
# `base` is the gate binary's own feature set with this round's feature ABSENT.
# `meas` has it COMPILED and unarmed - nothing outside the example can reach it.
cargo build --release --features jagua-experimental --example general_request_benchmark > "$LOG/build-base.log" 2>&1
record "base binary build (overlap-ics absent)" $?
cp "$ROOT/target/release/examples/general_request_benchmark" "$OUT/base-binary"

cargo build --release --features jagua-experimental,overlap-ics --example general_request_benchmark > "$LOG/build-meas.log" 2>&1
record "meas binary build (overlap-ics compiled)" $?
cp "$ROOT/target/release/examples/general_request_benchmark" "$OUT/meas-binary"

sha256sum "$OUT/base-binary" "$OUT/meas-binary" > "$EV/binaries.txt" 2>&1
record "binary hashes" $?

# ----------------------------------------------------------------- the gates --
ICS_ROOT="$ROOT" python3 "$DRIVERS/gates.py" base "$OUT/base-binary" "$OUT/gates/base" > "$LOG/gates-base.log" 2>&1
record "four pinned gates on the base binary" $?

ICS_ROOT="$ROOT" python3 "$DRIVERS/gates.py" meas "$OUT/meas-binary" "$OUT/gates/meas" > "$LOG/gates-meas.log" 2>&1
record "four pinned gates on the meas binary" $?

ICS_ROOT="$ROOT" python3 "$DRIVERS/gatecompare.py" "$OUT/gates/base" "$OUT/gates/meas" > "$EV/gates.json" 2>&1
record "gate comparison (whole-document identity)" $?

# ---------------------------------------------------------------- the suites --
ICS_ROOT="$ROOT" bash "$DRIVERS/run-suites.sh" > "$EV/suites-stdout.txt" 2>&1
record "five release suites" $?

# `suitetotals.py` reads the previous round's evidence directory by
# construction, so it runs while this round's logs are still sitting there.
ICS_ROOT="$ROOT" python3 "$DRIVERS/suitetotals.py" > "$EV/suites.json" 2>&1
record "suite totals" $?

for name in jagua combo example overlap-ics overlap-ics-stacked; do
  cp "$ROOT/docs/experiments/overlap-ics/evidence/suite-$name.log" "$EV/suite-$name.log" 2>/dev/null
done
git -C "$ROOT" checkout -- docs/experiments/overlap-ics/evidence
record "previous round's suite logs restored" $?

note "logs in $LOG"
note "evidence in $EV"
note "FAILURES=$FAILURES"
exit "$FAILURES"
