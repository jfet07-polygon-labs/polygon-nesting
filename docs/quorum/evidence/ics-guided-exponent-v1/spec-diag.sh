#!/usr/bin/env bash
# spec-diag.sh <binary> <outdir> <prefix> "<control flags>"  -- the unscored diagnostic cells of ICS-guided-exponent-v1:
# one --bitemicroscope=1 cell per (profile, seed 27-35) for the CONTROL arm only, after the 108 scored cells,
# refused by every scorer through the biteMicroscope tripwire; absence of a trigger bite is reported, never substituted.
B=$1; O=$2; P=$3; FL=$4
REQ=tests/fixtures/mixed-61/mixed61-request-exact-clearance.json
cd /var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a; mkdir -p $O
exec 9>/var/lib/t3/tmp/astra/bench.lock; flock 9
echo "spec-diag $P: [$FL] binary=$(sha256sum $B | cut -c1-12) start $(date -u +%H:%M:%S)"
for prof in legacy wall10s; do for s in 27 28 29 30 31 32 33 34 35; do
  out=$O/$P-diag-$prof-s$s.json
  if python3 -c "import json,sys; d=json.load(open('$out')); sys.exit(0 if 'outcome' in d else 1)" 2>/dev/null; then continue; fi
  $B --cell=cutclose --request=$REQ --edge=5 --pair=5 --mode=wall --wall=10.0 --orders=1 --workers=8 \
     --arm=control --profile=$prof $FL --bitemicroscope=1 --seed=$s > $out 2>/dev/null
  echo "$(date -u +%H:%M:%S) $P-diag-$prof-s$s $(python3 -c "import json; d=json.load(open('$out')); m=d.get('biteMicroscope',{}); print('exposure', m.get('exposure',{}).get('status'), 'bites', len(m.get('bites',[])))" 2>/dev/null)"
done; done
echo "${P}_DIAG_DONE $(date -u +%H:%M:%S)"
