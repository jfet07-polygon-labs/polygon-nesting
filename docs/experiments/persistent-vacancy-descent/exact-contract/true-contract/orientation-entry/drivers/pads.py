#!/usr/bin/env python3
"""Phase B2: the 28 from-scratch launch pads as parents for modes 32/33.

Each pad is flattened in the measured productive band (0.002-0.004) and handed
to modes 33/32 with a loose bound; the controls 29/28 run on the same fixtures.
"""
import sys, json, os, glob
sys.path.insert(0, '/var/lib/t3/tmp/orient')
import drv

LOG = '/var/lib/t3/tmp/orient/pads.log'
PADS = sorted(glob.glob(f'{drv.lib.TRUE}/from-scratch-164.038/pads/pad-*.json'))
INCUMBENT = drv.SCRATCH_RAW
results = []

drv.log(LOG, f'== {len(PADS)} pads, incumbent raw {INCUMBENT}')
for pad in PADS:
    name = os.path.basename(pad).replace('.json', '').replace('pad-', '')
    doc = json.load(open(pad))
    pad_raw = drv.lib.depth_mm(doc['placements'])
    for delta in (0.002, 0.004):
        path, depth, moved = drv.flatten_fixture(delta, pad, f'pad-{name}')
        for mode in (33, 32, 29, 28):
            tag = f'pad{name}-f{delta}-m{mode}'
            out = drv.go(tag, mode, path, pad_raw + 2.0, 0, LOG)
            published = drv.published_raw(out)
            results.append({
                'pad': name, 'padRaw': pad_raw, 'flatten': delta, 'mode': mode,
                'published': published,
                'belowIncumbent': published is not None and published < INCUMBENT - 1e-12,
                'belowPad': published is not None and published < pad_raw - 1e-12,
                'attribution': drv.attribution(out),
            })

json.dump(results, open('/var/lib/t3/tmp/orient/pads.json', 'w'), indent=1)
wins = [r for r in results if r['belowIncumbent']]
drv.log(LOG, f'== {len(results)} arms, {len(wins)} below the 164.038568 incumbent')
for entry in wins:
    drv.log(LOG, '   WIN ' + json.dumps(entry))
