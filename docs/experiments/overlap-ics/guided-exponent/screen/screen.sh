#!/usr/bin/env bash
# screen.sh <binary> <outdir> <prefix> "<armA flags>" "<armB flags>" [seeds] [reps] [profiles]
# The registered development screen (Astra review 5b, Q7): two arms, both profiles, nine seeds,
# three repetitions, arm order rotated per repetition (A,B on even repetitions, B,A on odd),
# one fresh eight-worker process per cell, bench lock held for the whole screen.
# Cells: <outdir>/<prefix>-<arm>-<profile>-r<rep>-s<seed>.json ; arm names are A and B.
B=$1; O=$2; P=$3; FA=$4; FB=$5; SEEDS=${6:-"18 19 20 21 22 23 24 25 26"}; REPS=${7:-3}; PROFS=${8:-"legacy wall10s"}
REQ=tests/fixtures/mixed-61/mixed61-request-exact-clearance.json
cd /var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a; mkdir -p $O
exec 9>/var/lib/t3/tmp/astra/bench.lock; flock 9
echo "screen $P: A=[$FA] B=[$FB] seeds=[$SEEDS] reps=$REPS profiles=[$PROFS] binary=$(sha256sum $B | cut -c1-12)"
for rep in $(seq 0 $((REPS-1))); do
  if [ $((rep % 2)) -eq 0 ]; then ORDER="A B"; else ORDER="B A"; fi
  for arm in $ORDER; do
    if [ $arm = A ]; then FL="$FA"; else FL="$FB"; fi
    for prof in $PROFS; do
      for s in $SEEDS; do
        out=$O/$P-$arm-$prof-r$rep-s$s.json
        if python3 -c "import json,sys; d=json.load(open('$out')); sys.exit(0 if 'outcome' in d else 1)" 2>/dev/null; then continue; fi
        $B --cell=cutclose --request=$REQ --edge=5 --pair=5 --mode=wall --wall=10.0 --orders=1 --workers=8 \
           --arm=control --profile=$prof $FL --seed=$s > $out 2>/dev/null
      done
    done
    echo "$P rep $rep arm $arm done $(date +%H:%M:%S)"
  done
done
echo "${P}_SCREEN_DONE"
