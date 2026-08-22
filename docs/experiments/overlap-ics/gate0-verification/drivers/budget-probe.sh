#!/usr/bin/env bash
# Is the stall hard, or is the quota just short?
#
#     bash docs/experiments/overlap-ics/gate0-verification/drivers/budget-probe.sh
#
# The three fatal cells run at 200,000 piece proposals, which is the work quota
# `cells.py` derives from two solver seconds at the measured rate. If ten times
# that quota moved `max_g` at all, the verdict would be "the budget is short",
# which is a schedule question and not a kill. This asks the question directly,
# on the two cells whose residual is small enough for the answer to matter.
#
# Everything except `--budget` is the fatal cell's own invocation.

set -u

ROOT="${ICS_ROOT:-/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_7f77514b-f9a-2}"
BIN="${ICS_BIN:-$ROOT/target/release/examples/overlap_ics_benchmark}"
OUT="${ICS_VERIFY_OUT:-/var/lib/t3/tmp/overlapics-v2}/budget"
BUDGET="${ICS_PROBE_BUDGET:-2000000}"
mkdir -p "$OUT"

MIXED="$ROOT/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json"
TRI="$ROOT/tests/fixtures/triangle-20/2000x2700-compact/request.json"
POSES="$ROOT/docs/experiments/gate-a-sparrow-import/fixture/sparrow-10s-x86-poses.json"

"$BIN" --cell=s1 --request="$MIXED" --edge=5 --pair=5 --poses="$POSES" \
  --target=150.16547 --budget="$BUDGET" --seed=0 --perturbmm=0.5 --perturbdeg=2.0 \
  --checkpointevery=1 > "$OUT/s1-budget.json" 2> "$OUT/s1-budget.err"
echo "S1_BUDGET_EXIT=$?"

"$BIN" --cell=triangle --request="$TRI" --edge=5 --pair=5 \
  --target=70.742 --budget="$BUDGET" --seed=0 --checkpointevery=1 \
  > "$OUT/triangle20-budget.json" 2> "$OUT/triangle20-budget.err"
echo "T20_BUDGET_EXIT=$?"
