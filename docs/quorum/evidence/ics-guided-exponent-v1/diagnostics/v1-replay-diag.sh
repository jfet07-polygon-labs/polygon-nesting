#!/usr/bin/env bash
# For each unscored microscope cell of ICS-guided-exponent-v1 (control arm, margin 8), replay its trigger bite's
# entry capsule at p = 2 (identity + column) and at p = 1 with --certify=1 (the treatment on the SAME bite),
# 100 iterations maximum; diagnostic only (replay tripwire). Under the bench lock, one at a time.
T=/var/lib/t3/tmp/astra; W=/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a; B=$T/frozen-1e913f4
R=$W/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json; O=$T/v1/replay; mkdir -p $O
exec 9>$T/bench.lock; flock 9
for prof in legacy wall10s; do for s in 27 28 29 30 31 32 33 34 35; do
  doc=$T/v1/v1-diag-$prof-s$s.json
  bite=$(python3 -c "import json; m=json.load(open('$doc'))['biteMicroscope']; b=[x for x in m['bites'] if x.get('retainedAs')=='trigger']; print(b[0]['ordinal'] if b else '')")
  [ -z "$bite" ] && { echo "$prof s$s: no trigger bite"; continue; }
  for probe in exponent:2 exponent:1; do
    name=$O/rp-$prof-s$s-$probe
    cd $W && $B --cell=replay --request=$R --edge=5 --pair=5 --capsule=$doc --bite=$bite --probe=$probe --workers=8 --proxymargin=8 --maxiters=100 --certify=1 > $name.json 2> $name.err
    echo "$prof s$s bite $bite $probe: $(grep -E '^replay identity \(' $name.err | cut -c1-60) | $(grep -E '^replay column \(' $name.err | sed -e 's/replay column (//' | cut -c1-150) | $(grep -E '^replay \(' $name.err | grep -oE 'stop [a-z-]+ after [0-9]+ iterations; bandEnteredAtIteration [A-Za-z0-9()]+; evaluationsToBand [A-Za-z0-9()]+') | $(grep -E '^replay certification' $name.err | grep -oE 'published [a-z]+')"
  done
done; done
echo V1_REPLAY_DIAG_DONE
