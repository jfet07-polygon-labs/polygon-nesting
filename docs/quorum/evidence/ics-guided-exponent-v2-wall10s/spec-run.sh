#!/usr/bin/env bash
# spec-run.sh <binary> <outdir> <prefix> "<armA flags>" "<armB flags>" [manifest]
# The prospective round ICS-guided-exponent-v1: adjacent control/treatment pairs per (repetition,
# profile, seed) in the committed manifest's order ((seed + rep) mod 2 == 0 -> A then B, else B then A),
# one fresh eight-worker process per cell, the bench lock held for the whole round, existing cells
# skipped (so a refused/contaminated pair can be re-run by deleting its two documents), no early stopping.
B=$1; O=$2; P=$3; FA=$4; FB=$5; M=${6:-$O/manifest-$P.txt}
REQ=tests/fixtures/mixed-61/mixed61-request-exact-clearance.json
cd /var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a; mkdir -p $O
[ -f "$M" ] || { echo "manifest $M missing"; exit 2; }
exec 9>/var/lib/t3/tmp/astra/bench.lock; flock 9
echo "spec-run $P: A=[$FA] B=[$FB] binary=$(sha256sum $B | cut -c1-12) manifest=$(sha256sum $M | cut -c1-12) start $(date -u +%H:%M:%S)"
while read -r rep prof seed first second; do
  [ -z "$rep" ] && continue; case "$rep" in \#*) continue;; esac
  for arm in $first $second; do
    if [ $arm = A ]; then FL="$FA"; else FL="$FB"; fi
    out=$O/$P-$arm-$prof-r$rep-s$seed.json
    if python3 -c "import json,sys; d=json.load(open('$out')); sys.exit(0 if 'outcome' in d else 1)" 2>/dev/null; then continue; fi
    $B --cell=cutclose --request=$REQ --edge=5 --pair=5 --mode=wall --wall=10.0 --orders=1 --workers=8 \
       --arm=control --profile=$prof $FL --seed=$seed > $out 2>/dev/null
    echo "$(date -u +%H:%M:%S) $P-$arm-$prof-r$rep-s$seed"
  done
done < "$M"
echo "${P}_ROUND_DONE $(date -u +%H:%M:%S)"
