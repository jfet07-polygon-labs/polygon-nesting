#!/usr/bin/env python3
"""From-scratch side probe: does the finer ladder unlock the 164.038568 basin?

24 arms max, all mode 33 with the finer-ladder binary: the frontier flatten
grid at a loose bound, then single-piece nudges on the frontier stack. The old
ladder moved this basin in none of 288 arms; the question is yes/no with counts.
"""
import sys, json, os, collections
sys.path.insert(0, '/var/lib/t3/tmp/orient-fine')
import drv, lib

LABEL = sys.argv[1] if len(sys.argv) > 1 else 'new'
LOG = f'/var/lib/t3/tmp/orient-fine/fs-{LABEL}.log'
RUNS = f'/var/lib/t3/tmp/orient-fine/fs-runs-{LABEL}'
FIX = '/var/lib/t3/tmp/orient-fine/fs-fix'
os.makedirs(RUNS, exist_ok=True)
os.makedirs(FIX, exist_ok=True)

PARENT, RAW = drv.SCRATCH, drv.SCRATCH_RAW
FLAT = (0.0005, 0.001, 0.002, 0.003, 0.004, 0.01)
SLACK = (0.05, 2.0)
NUDGE = (0.002, 0.006, 0.012, 0.02)

rows = []
rungs = collections.Counter()
drv.log(LOG, f'== FS {LABEL} bin={lib.BIN} parent={PARENT} raw={RAW!r}')


def arm(tag, fixture, target):
    out = drv.go(tag, 33, fixture, target, 0, LOG, outdir=RUNS)
    attr = dict(drv.attribution(out))
    angles = attr.pop('acceptedAngles', [])
    local = collections.Counter()
    for (_pid, _abs, dd, mirror) in angles:
        key = ('mirror' if mirror else 'rot') + f':{abs(dd):.7g}' if dd is not None \
            else ('mirror:?' if mirror else 'rot:?')
        local[key] += 1
        rungs[key] += 1
    pop = lib.population(out) or {}
    published = drv.published_raw(out)
    rows.append({'tag': tag, 'published': published,
                 'belowIncumbent': published is not None and published < RAW - 1e-12,
                 'exactValid': pop.get('exactValid'),
                 'contractValid': pop.get('contractValid'),
                 'raw': pop.get('rawSourceDepthMm'),
                 'attribution': attr, 'rungs': dict(local)})
    if local:
        drv.log(LOG, '   rungs ' + json.dumps(dict(local), sort_keys=True))
    return rows[-1]


ranked = drv.ranked_extents(PARENT)
for delta in FLAT:
    path, depth, moved = drv.flatten_fixture(delta, PARENT, 'fs', outdir=FIX)
    drv.log(LOG, f'-- flatten {delta}: depth {depth:.9f}, {len(moved)} moved')
    for slack in SLACK:
        arm(f'fs-flat{delta}-m33-p{slack}', path, RAW + slack)
for rank in (1, 2, 3):
    for delta in NUDGE:
        path, depth = drv.single_nudge_fixture(
            [ranked[rank - 1][1]], delta, PARENT, f'fs-r{rank}-d{delta}', outdir=FIX)
        arm(f'fs-nudge-r{rank}-d{delta}-m33', path, RAW + 2.0)

below = [r for r in rows if r['belowIncumbent']]
pubs = [r for r in rows if r['exactValid'] and r['contractValid']]
summary = {'label': LABEL, 'binary': lib.BIN, 'arms': len(rows),
           'publications': len(pubs), 'belowIncumbent': len(below),
           'rungs': dict(rungs), 'incumbent': RAW,
           'bestPublished': min([r['published'] for r in pubs], default=None),
           'rows': rows}
json.dump(summary, open(f'/var/lib/t3/tmp/orient-fine/fs-{LABEL}.json', 'w'), indent=1)
drv.log(LOG, f'== FS {LABEL}: {len(rows)} arms, {len(pubs)} publications, '
             f'{len(below)} below incumbent, best={summary["bestPublished"]!r}')
drv.log(LOG, '   rungs ' + json.dumps(dict(rungs), sort_keys=True))
