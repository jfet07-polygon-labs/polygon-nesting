#!/usr/bin/env bash
# The FAST tier, exactly per Sol review 14 Round 2 §3. Minutes, every iteration.
#
#     bash docs/experiments/overlap-ics/drivers/fast.sh
#
# Do NOT pipe this into `tee` or `tail`: you would read the pipe's status
# instead of the script's. Every exit status below is captured on its own line
# immediately after the command, and nothing here uses a pipeline to decide
# anything.
#
# Stages, in the order the spec lists them:
#
#   1. compile-only default-build isolation  (`--no-default-features --lib`)
#   2. dependency hygiene                    (no `jagua-rs` in the tree)
#   3. one release feature combo             (`--features overlap-ics`)
#   4. the module's unit vectors + the three pinned vector suites
#   5. the 1,000-state deterministic contact corpus
#   6. the two-process fixed-work smoke, S0 canary and S1 locked strip
#
# Stage 6 is red in this round and that is the round's finding, not a broken
# script: S1's *mechanism* clause (republish inside the locked strip) fails at
# max_g = 12.6 um against a 4 um attempt band. Its *invariant* clauses - no
# invalid publication, repair <= 16 um, giveback <= 0.050 mm, two-process
# bit-identity - all hold, and `INVARIANTS_PASS` in the smoke document is the
# field that says so. See docs/experiments/overlap-ics/README.md.

set -u

ROOT="${ICS_ROOT:-/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_7f77514b-f9a-1}"
OUT="${ICS_OUT:-/var/lib/t3/tmp/overlapics/fast}"
LOG="$OUT/log"
mkdir -p "$LOG"
cd "$ROOT"
FAILURES=0

note() {
  echo "[fast] $1"
}

record() {
  local name="$1"
  local status="$2"
  if [ "$status" -eq 0 ]; then
    note "$name EXIT=$status"
  else
    note "$name EXIT=$status  <-- FAILED"
    FAILURES=$((FAILURES + 1))
  fi
}

# ---------------------------------------------------------------- 1. default --
# Catches accidental unconditional imports, module visibility changes and
# feature leakage. It does not prove semantic identity; the heavy gates do that.
cargo check -p polygon-nesting-core --no-default-features --lib > "$LOG/default-check.log" 2>&1
record "default-build compile check" $?

# --------------------------------------------------------------- 2. hygiene --
# The Chinese wall, in the dependency graph. `overlap-ics` implies
# `round-envelope-kernel` and `fast-contract-validator` and nothing else, so
# `jagua-rs` must not appear at any depth of the resolved feature tree.
cargo tree -p polygon-nesting-core --features overlap-ics -e features > "$LOG/tree.log" 2>&1
record "cargo tree --features overlap-ics" $?
grep -q "jagua-rs" "$LOG/tree.log"
GREP_STATUS=$?
if [ "$GREP_STATUS" -eq 1 ]; then
  note "dependency hygiene: jagua-rs ABSENT EXIT=0"
else
  note "dependency hygiene: jagua-rs PRESENT (grep exit $GREP_STATUS)  <-- FAILED"
  FAILURES=$((FAILURES + 1))
fi

# ------------------------------------------------------------------ 3/4. tests --
cargo test -p polygon-nesting-core --release --features overlap-ics --lib search::overlap_ics:: > "$LOG/unit.log" 2>&1
record "module unit vectors" $?

cargo test -p polygon-nesting-core --release --features overlap-ics --test validation_vectors sat_penetration_matches_ts_oracle > "$LOG/sat-oracle.log" 2>&1
record "validation_vectors::sat_penetration_matches_ts_oracle" $?

cargo test -p polygon-nesting-core --release --features overlap-ics --test canonical_grid_vectors > "$LOG/grid-vectors.log" 2>&1
record "canonical_grid_vectors" $?

cargo test -p polygon-nesting-core --release --features overlap-ics --test collision_builder_vectors > "$LOG/collision-vectors.log" 2>&1
record "collision_builder_vectors" $?

# -------------------------------------------------------- 5. the 1,000 corpus --
cargo build -p polygon-nesting-core --release --features overlap-ics --example overlap_ics_benchmark > "$LOG/build-example.log" 2>&1
record "release example build" $?

ICS_OUT="$OUT" python3 "$ROOT/docs/experiments/overlap-ics/drivers/corpus_gate.py" 1000 > "$LOG/corpus.log" 2>&1
record "1,000-state contact corpus" $?

# ------------------------------------------------------------------ 6. smoke --
ICS_OUT="$OUT" python3 "$ROOT/docs/experiments/overlap-ics/drivers/smoke.py" 200000 > "$LOG/smoke.log" 2>&1
record "two-process fixed-work smoke (S0 canary, S1 locked strip)" $?

note "logs in $LOG"
note "FAILURES=$FAILURES"
exit "$FAILURES"
