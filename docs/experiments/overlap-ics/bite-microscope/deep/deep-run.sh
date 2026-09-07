#!/usr/bin/env bash
# deep-run.sh <binary> : the depth microscope (Astra review 7 Q18 / review 8 Q21-Q22) on CONSUMED seeds only, both arms,
# Wall10s: seeds 27-35 (v1) and the v2 seeds with a fifth-cut success in either arm plus six failing ones (deep-seeds.txt);
# --microscopetarget=156,151; then each retained trigger attempt replayed under both objectives with --horizon=live
# --certify=1; then deep-cut.py. Unscored, tripwired documents. Bench lock held for the whole run.
B=$1; T=/var/lib/t3/tmp/astra; W=/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a; O=$T/deep; mkdir -p $O
REQ=tests/fixtures/mixed-61/mixed61-request-exact-clearance.json
SEEDS="27 28 29 30 31 32 33 34 35 $(cat $T/v2/deep-seeds.txt | tr '\n' ' ')"
cd $W; exec 9>$T/bench.lock; flock 9
echo "deep-run binary=$(sha256sum $B | cut -c1-12) start $(date -u +%H:%M:%S) seeds: $SEEDS"
for s in $SEEDS; do for arm in A B; do
  if [ $arm = A ]; then FL="--proxymargin=8"; else FL="--proxymargin=8 --guidedexponent=1"; fi
  out=$O/deep-$arm-wall10s-s$s.json
  if python3 -c "import json,sys; d=json.load(open('$out')); sys.exit(0 if 'outcome' in d else 1)" 2>/dev/null; then continue; fi
  $B --cell=cutclose --request=$REQ --edge=5 --pair=5 --mode=wall --wall=10.0 --orders=1 --workers=8 --arm=control --profile=wall10s $FL --microscopetarget=156,151 --seed=$s > $out 2>/dev/null
  echo "$(date -u +%H:%M:%S) deep-$arm-s$s $(python3 -c "import json; d=json.load(open('$out')); m=d.get('biteMicroscope',{}); e=m.get('exposure',{}); print(e.get('status'), 'bites', len(m.get('bites',[])), 'depth', round(d['outcome']['depthMm'],2))" 2>/dev/null)"
done; done
echo "DEEP_CELLS_DONE $(date -u +%H:%M:%S)"
