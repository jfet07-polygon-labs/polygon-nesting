#!/usr/bin/env bash
# The §4 compound battery, all three requests, both arms of `crot`.
#
# Both arms carry `fast-contract-validator` (compiled in, no spec key) and
# `m34pconfirm=1`; `m34wall` and `m34bit` are left at their v3 defaults. The
# only key that differs between the two arms of each pair is `crot`.
set -u
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_08a442a7-1aa-1
D="$W/docs/experiments/rotation-tax/drivers"
OUT="${V4_OUT:-/var/lib/t3/rt/out}"
OFF='m34lanes=1,m34pconfirm=1,crot=0'
ON='m34lanes=1,m34pconfirm=1,crot=1'
ROUNDS="${ROUNDS:-3}"

for R in mixed-61:mixed61 shapes-17:shapes17 triangle-20:triangle20; do
  REQ="${R%%:*}"
  TAG="${R##*:}"
  python3 "$D/battery.py" "curve-$TAG" "$ROUNDS" "$REQ" 0,1,2 \
      "baseat3:wall:3000:1:$OFF"   "crotat3:wall:3000:1:$ON" \
      "baseat10:wall:10000:1:$OFF" "crotat10:wall:10000:1:$ON" \
      "baseat30:wall:30000:1:$OFF" "crotat30:wall:30000:1:$ON" \
      > "$OUT/battery-$TAG.log" 2>&1
  echo "DONE $REQ exit=$?"
done
