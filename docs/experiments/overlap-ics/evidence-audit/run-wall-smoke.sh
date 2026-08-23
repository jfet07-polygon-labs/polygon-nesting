#!/usr/bin/env bash
# One budget cell of the repaired `wall.py`, to prove the repair runs.
#
#     bash run-wall-smoke.sh <root> <out-dir> <budget>
#
# The 3 s budget is used because it is the cheapest of the three and because
# `wall.py` only drives the fixed-work replay when the 10 s cell is present, so
# this exercises `cell()` and the aggregation without spending the replay's wall.
set -u
ROOT="$1"
OUT="$2"
BUDGET="${3:-3}"
export ICS_ROOT="$ROOT"
export ICS_OUT="$OUT"
python3 "$ROOT/docs/experiments/overlap-ics/drivers/wall.py" "$BUDGET" > "$OUT/wall-smoke-stdout.json"
status=$?
echo "wall.py exit=$status"
exit "$status"
