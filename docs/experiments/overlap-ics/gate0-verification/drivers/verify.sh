#!/usr/bin/env bash
# The independent verification of Gate 0's STOP: re-run the fatal cells from the
# previous round's own committed drivers, on a build made from scratch in a
# second worktree, and compare every field of every document.
#
#     bash docs/experiments/overlap-ics/gate0-verification/drivers/verify.sh
#
# Do NOT pipe this into `tee` or `tail`. Every exit status below is read on the
# line immediately after its command, exactly as `drivers/fast.sh` does.
#
# The point of a second worktree is that the reproduction shares nothing with
# the run it is checking except the committed source: a different directory, a
# `target/` that did not exist, a separately compiled binary. What it may NOT
# share is the absolute path baked into the evidence documents, which is why the
# comparison is `docdiff.py` (field by field, paths neutralised and named) and
# not `lib.digest` (one hash over the whole document, paths included).

set -u

ROOT="${ICS_ROOT:-/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_7f77514b-f9a-2}"
OUT="${ICS_VERIFY_OUT:-/var/lib/t3/tmp/overlapics-v2}"
PRIOR="$ROOT/docs/experiments/overlap-ics/evidence"
DRIVERS="$ROOT/docs/experiments/overlap-ics/drivers"
MINE="$ROOT/docs/experiments/overlap-ics/gate0-verification/drivers"
mkdir -p "$OUT/cells"
FAILURES=0

note() { echo "[verify] $1"; }

record() {
  if [ "$2" -eq 0 ]; then
    note "$1 EXIT=$2"
  else
    note "$1 EXIT=$2  <-- FAILED"
    FAILURES=$((FAILURES + 1))
  fi
}

# ------------------------------------------------------------- 1. FAST tier --
# Expected to exit 1: its one red stage is the two-process smoke's S1 mechanism
# clause, which is the round's STOP. `record` is not used here for that reason;
# the interesting number is which stage is red, and `fast.sh` prints it.
ICS_ROOT="$ROOT" ICS_OUT="$OUT/fast" bash "$DRIVERS/fast.sh" > "$OUT/fast-stdout.txt" 2>&1
note "fast.sh EXIT=$?  (1 is this round's expected value: S1's mechanism clause)"

# ---------------------------------------------------------- 2. the six fatal --
ICS_ROOT="$ROOT" ICS_OUT="$OUT/cells" python3 "$DRIVERS/cells.py" s0 s1 c175 triangle \
  > "$OUT/cells-fatal.json" 2> "$OUT/cells-fatal.err"
record "cells.py s0 s1 c175 triangle" $?

ICS_ROOT="$ROOT" ICS_OUT="$OUT/cells" python3 "$DRIVERS/cells.py" corpus throughput \
  > "$OUT/cells-sound.json" 2> "$OUT/cells-sound.err"
record "cells.py corpus throughput" $?

# ------------------------------------------------- 3. document-by-document ----
# Six documents, every scalar leaf, against the previous round's committed
# evidence. A single differing number here would falsify the reproduction.
for pair in "cell-s0.json:s0.json" "cell-s1.json:s1.json" \
            "cell-triangle20.json:triangle20.json" \
            "cell-c175-seed0.json:c175-seed0.json" \
            "cell-c175-seed1.json:c175-seed1.json" \
            "cell-c175-seed2.json:c175-seed2.json"; do
  before="${pair%%:*}"
  after="${pair##*:}"
  python3 "$MINE/docdiff.py" "$PRIOR/$before" "$OUT/cells/$after" \
    > "$OUT/docdiff-$after" 2>&1
  record "docdiff $before" $?
done

note "FAILURES=$FAILURES"
exit "$FAILURES"
