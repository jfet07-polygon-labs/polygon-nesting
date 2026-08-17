#!/usr/bin/env python3
"""A/B one mechanism change (the orientation ladder) on mode 33.

Arm = frontier flatten of the incumbent -> mode 33 with a LOOSE bound, run
under BENCH_BIN. Reports the published raw depth, the orientation stream's
attribution counters, and the accepted-rung distribution, so the two ladder
generations can be compared field by field.
"""
import sys, json, os, collections
sys.path.insert(0, '/var/lib/t3/tmp/orient-fine')
import drv, lib

LABEL = sys.argv[1]
LOG = f'/var/lib/t3/tmp/orient-fine/ab33-{LABEL}.log'
OUT = f'/var/lib/t3/tmp/orient-fine/ab33-{LABEL}.json'
RUNS = f'/var/lib/t3/tmp/orient-fine/ab33-runs-{LABEL}'
FIX = f'/var/lib/t3/tmp/orient-fine/ab33-fix'
os.makedirs(RUNS, exist_ok=True)
os.makedirs(FIX, exist_ok=True)

PARENT, RAW = drv.RECORD, drv.RECORD_RAW
DELTAS = [float(x) for x in (sys.argv[2].split(',') if len(sys.argv) > 2
                             else ['0.003'])]
SLACK = 0.05

rows = []
drv.log(LOG, f'== AB33 {LABEL} bin={lib.BIN} parent={PARENT} raw={RAW!r}')
for delta in DELTAS:
    path, depth, moved = drv.flatten_fixture(delta, PARENT, f'ab-{delta}', outdir=FIX)
    drv.log(LOG, f'-- flatten {delta}: depth {depth:.9f}, {len(moved)} moved')
    tag = f'{LABEL}-flat{delta}-m33'
    out = drv.go(tag, 33, path, RAW + SLACK, 0, LOG, outdir=RUNS)
    attr = drv.attribution(out)
    angles = attr.pop('acceptedAngles', [])
    rungs = collections.Counter()
    for (_pid, _abs, dd, mirror) in angles:
        rungs[('mirror' if mirror else 'rot', dd)] += 1
    pop = lib.population(out) or {}
    rows.append({
        'label': LABEL, 'delta': delta, 'mode': 33,
        'published': drv.published_raw(out),
        'exactValid': pop.get('exactValid'),
        'contractValid': pop.get('contractValid'),
        'raw': pop.get('rawSourceDepthMm'),
        'fp': pop.get('finalPlacementFingerprint'),
        'attribution': attr,
        'rungs': {f'{k[0]}{k[1]}': v for k, v in sorted(rungs.items(), key=str)},
        'acceptedAngles': angles,
        'failure': (pop.get('failureReason') or '')[:200],
    })
    drv.log(LOG, '   rungs ' + json.dumps(rows[-1]['rungs']))
json.dump(rows, open(OUT, 'w'), indent=1)
drv.log(LOG, f'== wrote {OUT}')
