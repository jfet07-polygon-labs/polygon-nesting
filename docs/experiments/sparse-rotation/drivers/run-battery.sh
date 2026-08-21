#!/usr/bin/env bash
# The §4 anytime battery: all three requests, 3/10/30 s, base against sparse.
#
# **Both arms carry the 168.484 configuration** - `fast-contract-validator`
# compiled in with no spec key, `m34lanes=1,m34pconfirm=1`, `m34wall` and
# `m34bit` at their v3 defaults - because that is the config the binding user
# priority names and the one docs/experiments/rotation-tax/ §4 measured the
# base arm at. The only keys that differ between the two arms of a pair are the
# sparse operator's own.
#
# The armed arm is `crot=1,sparserot=1,roteq=1`: design B on the equivariant
# construction, which is the composition §1 and §2 measure separately and this
# battery measures together. `rotbit` is left at its default (on), so the arm
# includes the request-adaptive disarm - that is the mechanism as it would
# ship, and §5 runs the `rotbit=0` control separately rather than folding it in.
set -u
W=/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_4111958b-3b3-2
D="$W/docs/experiments/sparse-rotation/drivers"
OUT="${V4_OUT:-/var/lib/t3/tmp/sparserot/out}"
OFF='m34lanes=1,m34pconfirm=1,crot=0'
ON='m34lanes=1,m34pconfirm=1,crot=1,sparserot=1,roteq=1'
ROUNDS="${ROUNDS:-3}"

mkdir -p "$OUT"
for R in mixed-61:mixed61 shapes-17:shapes17 triangle-20:triangle20; do
  REQ="${R%%:*}"
  TAG="${R##*:}"
  python3 "$D/battery.py" "curve-$TAG" "$ROUNDS" "$REQ" 0,1,2 \
      "baseat3:wall:3000:1:$OFF"   "sparseat3:wall:3000:1:$ON" \
      "baseat10:wall:10000:1:$OFF" "sparseat10:wall:10000:1:$ON" \
      "baseat30:wall:30000:1:$OFF" "sparseat30:wall:30000:1:$ON" \
      > "$OUT/battery-$TAG.log" 2>&1
  echo "DONE $REQ exit=$?"
done
