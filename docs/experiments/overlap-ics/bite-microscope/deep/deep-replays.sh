#!/usr/bin/env bash
# deep-replays.sh <binary> : for every retained bite attempt of every /var/lib/t3/tmp/astra/deep/deep-*.json, replay its
# entry capsule under both objectives with the live horizon and the detached certification. Bench lock held.
B=$1; T=/var/lib/t3/tmp/astra; W=/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a; O=$T/deep/replays; mkdir -p $O
REQ=$W/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json
cd $W; exec 9>$T/bench.lock; flock 9
echo "deep-replays binary=$(sha256sum $B | cut -c1-12) start $(date -u +%H:%M:%S)"
for doc in $T/deep/deep-[AB]-wall10s-s*.json; do
  name=$(basename $doc .json); arm=${name:5:1}
  python3 - "$doc" <<'PY' | while read -r bite idx; do
import json,sys
d=json.load(open(sys.argv[1])); m=d.get('biteMicroscope') or {}
retained={b['ordinal'] for b in m.get('bites',[])}
for i,c in enumerate(m.get('capsules',[])):
    if c.get('bite') in retained: print(c['bite'], i)
PY
    for p in 2 1; do
      out=$O/rp-$name-b$bite-c$idx-p$p.json
      [ -s $out ] && continue
      EXTRA=""; [ $arm = B ] && EXTRA="--guidedexponent=1"
      $B --cell=replay --request=$REQ --edge=5 --pair=5 --capsule=$doc --bite=$bite --capsuleindex=$idx --probe=exponent:$p --workers=8 --proxymargin=8 --profile=wall10s --horizon=live --certify=1 $EXTRA > $out 2> ${out%.json}.err
      echo "$(date -u +%H:%M:%S) $name b$bite c$idx p$p rc=$? $(grep -oE 'stop [a-z-]+ after [0-9]+ iterations; bandEnteredAtIteration [A-Za-z0-9()]+' ${out%.json}.err | head -1)"
    done
  done
done
echo "DEEP_REPLAYS_DONE $(date -u +%H:%M:%S)"
