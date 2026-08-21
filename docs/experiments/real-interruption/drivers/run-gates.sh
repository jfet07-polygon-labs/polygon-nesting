#!/usr/bin/env bash
# The gates, in the order the protocol runs them: build, gate, *then* measure.
#
#   run-gates.sh
#
# Exit statuses are captured **directly** and never through a pipe, because
# `cmd | tee log` reports `tee`'s status and that is how a red gate gets written
# up as green.
set -u
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_60cc1332-b41-1
D="$W/docs/experiments/real-interruption/drivers"
E="$W/docs/experiments/real-interruption/evidence"
T=/var/lib/t3/tmp/realint
B="$T/bin"
cd "$W" || exit 3
mkdir -p "$E" "$T/out"

echo "== the four pinned regression gates, on the gate binary"
python3 "$D/gates.py" ship "$B/gate-meas" "$T/gates/ship" > "$E/gates-ship.json"
G=$?
echo "gates exit=$G"
python3 -c "
import json,sys
d=json.load(open('$E/gates-ship.json'))
for tag, g in d['gates'].items():
    print(tag, 'hit=', g.get('hit'), g.get('rawDepthMm'), g.get('fingerprint','')[:20])
print('ALL_PASS', d['ALL_PASS'])
"

echo
echo "== the refactor equivalence: base binary vs this one, nothing armed"
python3 "$D/equiv.py" "$T/out/equiv-base" "$B/base-ship-meas" "$B/ship-meas" \
  mixed-61,shapes-17,triangle-20 0,1,2 30000000 > "$T/equiv-base.log" 2>&1
Q=$?
tail -6 "$T/equiv-base.log"
cp "$T/out/equiv-base/equiv.json" "$E/equiv-base.json"
echo "equiv-base exit=$Q"

echo
echo "== the concatenation gate: batched vs monolithic, same binary"
for BATCH in 25000 400000 2000000; do
  python3 "$D/equiv.py" "$T/out/concat-$BATCH" "$B/ship-meas" "$B/ship-meas" \
    mixed-61,shapes-17,triangle-20 0,1,2 30000000 '' "m34batch=$BATCH" \
    > "$T/concat-$BATCH.log" 2>&1
  echo "concat $BATCH exit=$?"
  tail -2 "$T/concat-$BATCH.log"
  cp "$T/out/concat-$BATCH/equiv.json" "$E/concat-$BATCH.json"
done

echo
echo "== determinism, two processes, work mode"
for ARM in '' 'm34past=1' 'm34past=1,m34yield=2' 'm34batch=400000'; do
  TAG=$(echo "${ARM:-base}" | tr -d ' =,')
  python3 "$D/determinism.py" "$T/out/det-$TAG" "$B/ship-meas" \
    mixed-61,shapes-17,triangle-20 0,1,2 work 30000000 "$ARM" \
    > "$T/det-$TAG.log" 2>&1
  echo "determinism $TAG exit=$?"
  tail -2 "$T/det-$TAG.log"
  cp "$T/out/det-$TAG/determinism.json" "$E/determinism-$TAG.json"
done
