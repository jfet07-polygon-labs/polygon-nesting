#!/usr/bin/env python3
"""Phase A: the perturbation x mechanism x line grid for modes 32/33.

Protocol from the pose-entry negative: frontier flatten in
{0.002, 0.004, 0.01, 0.02} plus single-piece nudges targeting the frontier
stack, handed to modes 28/29 (control) and 32/33 (treatment) with a LOOSE
bound. The bound gates acceptance, it does not drive descent.
"""
import sys, json, os
sys.path.insert(0, '/var/lib/t3/tmp/orient')
import drv

LOG = '/var/lib/t3/tmp/orient/grid.log'
LINE = sys.argv[1] if len(sys.argv) > 1 else 'rec'
PARENT, RAW = (drv.RECORD, drv.RECORD_RAW) if LINE == 'rec' else (drv.SCRATCH, drv.SCRATCH_RAW)
BOUND = RAW + 2.0
results = []


def arm(tag, fixture, modes=(28, 29, 32, 33), seed=5):
    for mode in modes:
        out = drv.go(f'{LINE}-{tag}-m{mode}', mode, fixture, BOUND, seed, LOG)
        published = drv.published_raw(out)
        attr = drv.attribution(out)
        results.append({
            'line': LINE, 'perturbation': tag, 'mode': mode,
            'published': published,
            'record': published is not None and published < RAW - 1e-12,
            'attribution': attr,
            'failure': (drv.blk(out) or {}).get('skippedReason')
                       or (drv.blk(out) or {}).get('rejectionReason')
                       or (drv.population(out) if False else None)
                       or ((drv.lib.population(out) or {}).get('failureReason')),
        })


ranked = drv.ranked_extents(PARENT)
drv.log(LOG, f'== {LINE} parent raw {RAW} bound {BOUND}')
drv.log(LOG, '   frontier: ' + ', '.join(f'{pid[:12]}@{high:.6f}' for high, pid in ranked[:6]))

# Arm 1: frontier flatten at four deltas.
for delta in (0.002, 0.004, 0.01, 0.02):
    path, depth, moved = drv.flatten_fixture(delta, PARENT, LINE)
    drv.log(LOG, f'-- flatten {delta}: depth {depth:.6f}, {len(moved)} pieces moved')
    arm(f'flat{delta}', path)

# Arm 2: single-piece nudges on the frontier stack. Rank 1 is the
# depth-setting piece; the deeper ranks are the runners-up it is tied with.
for rank in (1, 2, 3):
    for delta in (0.002, 0.006, 0.02):
        pid = ranked[rank - 1][1]
        path, depth = drv.single_nudge_fixture([pid], delta, PARENT, f'{LINE}-r{rank}-d{delta}')
        drv.log(LOG, f'-- nudge rank {rank} ({pid[:12]}) by {delta}: depth {depth:.6f}')
        arm(f'nudge-r{rank}-d{delta}', path)

# Arm 3: pair nudges that put the depth-setting piece INTO the violating pair.
# The vertex cover ejects the endpoint carrying more violation mass with the
# lower pieceId breaking ties, so moving the depth setter *and* a neighbour
# together is what gets the depth setter itself ejected.
for delta in (0.002, 0.006, 0.02):
    ids = [ranked[0][1], ranked[1][1]]
    path, depth = drv.single_nudge_fixture(ids, delta, PARENT, f'{LINE}-pair12-d{delta}')
    drv.log(LOG, f'-- nudge ranks 1+2 by {delta}: depth {depth:.6f}')
    arm(f'nudge-pair12-d{delta}', path)

json.dump(results, open(f'/var/lib/t3/tmp/orient/grid-{LINE}.json', 'w'), indent=1)
published = [r for r in results if r['record']]
drv.log(LOG, f'== {LINE}: {len(results)} arms, {len(published)} sub-parent publications')
for entry in published:
    drv.log(LOG, '   RECORD ' + json.dumps(entry))
