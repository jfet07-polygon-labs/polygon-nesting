#!/usr/bin/env bash
# demo-geometry-all.sh <binary> : the replay-geometry instrument's demonstration (Astra 5b Q6-Q8)
# on the bite-15 capsule of s20-m8-legacy.json, then bite 17 (margin 8) and bite 17 of the
# margin-0 document, all four exponents, 50 iterations maximum for the treatments (Astra's
# registered horizon), the sweep-24 fork at p = 2, --certify=1 at p = 1 / 0.75 / 0.5.
# Under the bench lock, one at a time. Outputs geom-*.json / .err beside this script.
set -u
B=$1
W=/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a
R=$W/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json
OUT=/var/lib/t3/tmp/astra/microscope
echo "binary $(sha256sum $B | cut -c1-12)"
run() {
  local name=$1 capsule=$2 bite=$3 probe=$4 maxiters=$5 margin=$6 extra=${7:-}
  echo "== $name: --bite=$bite --probe=$probe --maxiters=$maxiters --proxymargin=$margin $extra ($capsule)"
  flock /var/lib/t3/tmp/astra/bench.lock bash -c "cd $W && $B --cell=replay --request=$R --edge=5 --pair=5 --capsule=$OUT/$capsule --bite=$bite --probe=$probe --workers=8 --proxymargin=$margin --maxiters=$maxiters $extra > $OUT/$name.json 2> $OUT/$name.err"
  echo "rc=$?"
  grep -E "^replay identity \(|^replay column|^replay fork|^replay certification|^replay \(" "$OUT/$name.err" | grep -v "persistent row"
}
run geom-b15-p2         s20-m8-legacy.json 15 exponent:2    43 8
run geom-b15-p1         s20-m8-legacy.json 15 exponent:1    50 8
run geom-b15-p075       s20-m8-legacy.json 15 exponent:0.75 50 8
run geom-b15-p05        s20-m8-legacy.json 15 exponent:0.5  50 8
run geom-b15-fork24     s20-m8-legacy.json 15 exponent:2    43 8 --fork=24
run geom-b15-p1-certify   s20-m8-legacy.json 15 exponent:1    50 8 --certify=1
run geom-b15-p075-certify s20-m8-legacy.json 15 exponent:0.75 50 8 --certify=1
run geom-b15-p05-certify  s20-m8-legacy.json 15 exponent:0.5  50 8 --certify=1
run geom-b17-p2   s20-m8-legacy.json 17 exponent:2    26 8
run geom-b17-p1   s20-m8-legacy.json 17 exponent:1    50 8
run geom-b17-p075 s20-m8-legacy.json 17 exponent:0.75 50 8
run geom-b17-p05  s20-m8-legacy.json 17 exponent:0.5  50 8
run geom-m0b17-p2   s20-m0-legacy.json 17 exponent:2    37 0
run geom-m0b17-p1   s20-m0-legacy.json 17 exponent:1    50 0
run geom-m0b17-p075 s20-m0-legacy.json 17 exponent:0.75 50 0
run geom-m0b17-p05  s20-m0-legacy.json 17 exponent:0.5  50 0
echo "== column-break.py"
python3 /var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_9a73dad9-b3c-1/docs/experiments/overlap-ics/bite-microscope/column-break.py $OUT/geom-*.json
echo "GEOMETRY_DEMO_DONE"
