#!/usr/bin/env bash
# The four `cutclose.py` FAST stages on the repaired driver, to prove the
# `CANARY_PASS` repair did not change any stage's verdict.
#
#     bash run-cutclose-stages.sh <root> <out-dir>
set -u
ROOT="$1"
OUT="$2"
export ICS_ROOT="$ROOT"
export ICS_OUT="$OUT"
mkdir -p "$OUT"
python3 "$ROOT/docs/experiments/overlap-ics/drivers/cutclose.py" > "$OUT/cutclose-stdout.json"
status=$?
echo "cutclose.py exit=$status"
exit "$status"
