#!/usr/bin/env bash
# screen-arm.sh <binary> <outdir> <prefix> <arm> "<flags>" [seeds] [reps] [profiles]
# One arm of a development screen, cells <outdir>/<prefix>-<arm>-<profile>-r<rep>-s<seed>.json,
# one fresh eight-worker process per cell, bench lock held for the whole arm, existing cells skipped.
B=$1; O=$2; P=$3; ARM=$4; FL=$5; SEEDS=${6:-"18 19 20 21 22 23 24 25 26"}; REPS=${7:-3}; PROFS=${8:-"legacy wall10s"}
REQ=tests/fixtures/mixed-61/mixed61-request-exact-clearance.json
cd /var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a; mkdir -p $O
exec 9>/var/lib/t3/tmp/astra/bench.lock; flock 9
echo "screen-arm $P-$ARM: [$FL] seeds=[$SEEDS] reps=$REPS profiles=[$PROFS] binary=$(sha256sum $B | cut -c1-12) start $(date -u +%H:%M:%S)"
for rep in $(seq 0 $((REPS-1))); do
  for prof in $PROFS; do
    for s in $SEEDS; do
      out=$O/$P-$ARM-$prof-r$rep-s$s.json
      if python3 -c "import json,sys; d=json.load(open('$out')); sys.exit(0 if 'outcome' in d else 1)" 2>/dev/null; then continue; fi
      $B --cell=cutclose --request=$REQ --edge=5 --pair=5 --mode=wall --wall=10.0 --orders=1 --workers=8 \
         --arm=control --profile=$prof $FL --seed=$s > $out 2>/dev/null
    done
  done
  echo "$P-$ARM rep $rep done $(date -u +%H:%M:%S)"
done
echo "${P}_${ARM}_ARM_DONE"
