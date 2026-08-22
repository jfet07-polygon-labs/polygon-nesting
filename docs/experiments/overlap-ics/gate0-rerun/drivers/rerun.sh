#!/usr/bin/env bash
# The complete Gate-0 re-run: every cell, fatal and diagnostic, plus the basin
# sweep and two-process determinism on every cell document.
#
#     bash docs/experiments/overlap-ics/gate0-rerun/drivers/rerun.sh
#
# Do NOT pipe this into `tee` or `tail`: you would read the pipe's status
# instead of the script's. Every exit status below is captured on its own line
# immediately after the command.

set -u

ROOT="${ICS_ROOT:-/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_72c0d771-eb6-1}"
OUT="${ICS_OUT:-/var/lib/t3/tmp/overlapics-rerun}"
DRIVERS="$ROOT/docs/experiments/overlap-ics/gate0-rerun/drivers"
LOG="$OUT/log"
mkdir -p "$LOG"
cd "$ROOT"
FAILURES=0

note() { echo "[rerun] $1"; }

record() {
  local name="$1"
  local status="$2"
  if [ "$status" -eq 0 ]; then
    note "$name EXIT=$status"
  else
    note "$name EXIT=$status  <-- NONZERO"
    FAILURES=$((FAILURES + 1))
  fi
}

cargo build -p polygon-nesting-core --release --features overlap-ics --example overlap_ics_benchmark > "$LOG/build.log" 2>&1
record "release example build" $?

ICS_ROOT="$ROOT" ICS_OUT="$OUT" python3 "$DRIVERS/cells.py" > "$LOG/cells.log" 2>&1
record "every Gate-0 cell (fatal + diagnostic)" $?

ICS_ROOT="$ROOT" ICS_OUT="$OUT" python3 "$DRIVERS/basin.py" 200000 > "$LOG/basin-default.log" 2>&1
record "basin sweep, derived commit rule" $?

ICS_ROOT="$ROOT" ICS_OUT="$OUT" python3 "$DRIVERS/basin.py" 200000 guided > "$LOG/basin-guided.log" 2>&1
record "basin sweep, guided commit rule (the A/B arm)" $?

ICS_ROOT="$ROOT" ICS_OUT="$OUT" python3 "$DRIVERS/twoprocess.py" > "$LOG/twoprocess.log" 2>&1
record "two-process determinism, every cell" $?

note "documents in $OUT"
note "logs in $LOG"
note "FAILURES=$FAILURES"
exit "$FAILURES"
