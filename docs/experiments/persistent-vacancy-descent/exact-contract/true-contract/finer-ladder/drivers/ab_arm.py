#!/usr/bin/env python3
"""A/B one named arm across the two ladder generations, field by field.

Usage: ab_arm.py <label> <mode> <fixture> <target> <seed>
"""
import sys, json, os, collections
sys.path.insert(0, '/var/lib/t3/tmp/orient-fine')
import drv, lib

LABEL, MODE, FIXTURE, TARGET, SEED = (sys.argv[1], int(sys.argv[2]), sys.argv[3],
                                      float(sys.argv[4]), int(sys.argv[5]))
LOG = f'/var/lib/t3/tmp/orient-fine/abarm-{LABEL}.log'
RUNS = f'/var/lib/t3/tmp/orient-fine/abarm-runs'
os.makedirs(RUNS, exist_ok=True)
BINS = (('base', '/var/lib/t3/tmp/orient-fine/bench-base'),
        ('new', '/var/lib/t3/tmp/orient-fine/bench-new'))

rows = {}
for ladder, binary in BINS:
    lib.BIN = binary
    out = drv.go(f'{LABEL}-{ladder}', MODE, FIXTURE, TARGET, SEED, LOG, outdir=RUNS)
    pop = lib.population(out) or {}
    attr = dict(drv.attribution(out))
    angles = attr.pop('acceptedAngles', [])
    rungs = collections.Counter()
    for (_pid, _abs, dd, mirror) in angles:
        rungs[('mirror' if mirror else 'rot') + (f':{abs(dd):.7g}' if dd is not None else ':?')] += 1
    rows[ladder] = {
        'ladder': ladder, 'published': drv.published_raw(out),
        'exactValid': pop.get('exactValid'), 'contractValid': pop.get('contractValid'),
        'raw': pop.get('rawSourceDepthMm'), 'fp': pop.get('finalPlacementFingerprint'),
        'failureReason': (pop.get('failureReason') or '')[:160],
        'attribution': attr, 'rungs': dict(rungs), 'acceptedAngles': angles,
    }
    drv.log(LOG, f'   [{ladder}] rungs ' + json.dumps(dict(rungs), sort_keys=True))

same = (rows['base']['published'] == rows['new']['published']
        and rows['base']['fp'] == rows['new']['fp'])
out = {'label': LABEL, 'mode': MODE, 'fixture': FIXTURE, 'target': TARGET,
       'seed': SEED, 'outcomeIdentical': same, 'base': rows['base'], 'new': rows['new']}
json.dump(out, open(f'/var/lib/t3/tmp/orient-fine/abarm-{LABEL}.json', 'w'), indent=1)
print(json.dumps(out, indent=1))
