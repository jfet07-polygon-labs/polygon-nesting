#!/usr/bin/env bash
# Copies the round's evidence out of the scratch output tree into the
# experiment directory, exactly as run.
#
# The per-run documents are deliberately NOT copied: the battery alone writes
# 216 of them at several megabytes each. What is kept is every reducer's output,
# which is what every number in the README is read from, plus the three suite
# logs.
set -eu
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_4111958b-3b3-2
E="$W/docs/experiments/sparse-rotation/evidence"
OUT="${V4_OUT:-/var/lib/t3/tmp/sparserot/out}"
G=/var/lib/t3/tmp/sparserot/gates
mkdir -p "$E"

cp "$OUT/curves-summary.json"                 "$E/curves-summary.json"
cp "$OUT/pool-10s.json"                       "$E/pool-10s.json"
cp "$OUT/pool-30s.json"                       "$E/pool-30s.json"
cp "$OUT/armgate/armgate.json"                "$E/armgate.json"
cp "$OUT/witnessprice/witnessprice.json"      "$E/witnessprice.json"
cp "$OUT/witnesscurve/witnesscurve-reduced.json" "$E/witnesscurve-reduced.json"
cp "$OUT/determinism-sparse/determinism.json" "$E/determinism-sparse.json"
cp "$OUT/reproduce-flagoff/reproduce.json"    "$E/reproduce-flagoff.json"
cp "$OUT/se2probe.json"                       "$E/se2probe.json"

for label in base-gate gate meas meas-se2; do
  cp "$G/$label/gates-$label.json" "$E/gates-$label.json"
done

# The battery rows, without the per-run documents they point at.
python3 - "$OUT" "$E" <<'PY'
import json
import sys

out, evidence = sys.argv[1], sys.argv[2]
for tag in ('mixed61', 'shapes17', 'triangle20', 'mixed61-seeds345'):
    try:
        battery = json.load(open(f'{out}/curve-{tag}/battery.json'))
    except FileNotFoundError:
        continue
    # `operatorCalls` and `archive` are the whole run's telemetry repeated per
    # row; the reducers read `m34` and the depths, and those stay.
    for row in battery['rows']:
        for key in ('operatorCalls', 'archive', 'phases', 'schedule'):
            row.pop(key, None)
    json.dump(battery, open(f'{evidence}/battery-{tag}.json', 'w'), indent=1)
    print(f'battery-{tag}.json')
PY

ls -la "$E"
