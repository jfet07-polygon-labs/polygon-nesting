#!/usr/bin/env python3
"""Re-entry probe: hand the schedule a layout its own lane can represent.

    python3 regrid.py LABEL PIN RAW

Mode 34 is productive on a parent that arrives *proxy-feasible* and inert on one
that does not, and which of the two a state is decided upstream of the mode:
`initialize_complete_state` maps every warm-start rotation through
`canonical_angle`, which snaps it onto the structured surrogate's 2.5-degree
grid. A state the schedule itself produced survives that snap (entry loss
0.019 mm on the 159.668 state); a state modes 22/33 produced does not (entry
loss 0.647 mm and 28 colliding pairs on the 156.919 state), and the schedule
then confirms nothing at any budget.

This probe asks whether the barrier can be walked around from outside: round
every pose onto the 2.5-degree grid *first*, legalize the result with the
translation-tier repairs that do not snap (modes 30, 31, 33), and hand whatever
comes back to the schedule. The pre-snapped fixture is deliberately not
required to be valid - it is an input to a repair mode, which is what modes
30/33 are for.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import drv  # noqa: E402
import lib  # noqa: E402
import sched  # noqa: E402

SURROGATE_ANGLE_STEP_DEG = 2.5
LABEL, PIN, RAW = sys.argv[1], sys.argv[2], float(sys.argv[3])
OUT = f'/var/lib/t3/tmp/recordline/{LABEL}'
LOG = f'{OUT}/regrid.log'
RUNS = f'{OUT}/runs'
FIX = f'{OUT}/fix'
for directory in (OUT, RUNS, FIX):
    os.makedirs(directory, exist_ok=True)


def snapped(placements):
    out = []
    for placement in placements:
        angle = placement['rotationDeg'] % 360.0
        snap = round(angle / SURROGATE_ANGLE_STEP_DEG) * SURROGATE_ANGLE_STEP_DEG
        out.append(dict(placement, rotationDeg=snap % 360.0))
    return out


placements = json.load(open(PIN))['placements']
grid = snapped(placements)
moved = sum(1 for a, b in zip(placements, grid)
            if a['rotationDeg'] != b['rotationDeg'])
path = f'{FIX}/{LABEL}-onGrid.json'
depth = lib.write_fixture(path, f'2.5-degree pose snap of {PIN}', grid)
drv.log(LOG, f'=== REGRID {PIN} raw={RAW!r}: {moved}/{len(grid)} poses moved, '
             f'snapped depth {depth:.9f} (+{depth - RAW:.6f})')

rows = []
for tag, mode, target in (('m30', 30, RAW), ('m27', 27, RAW),
                          ('m33-p2', 33, RAW + 2.0),
                          ('m33-p0.05', 33, RAW + 0.05),
                          ('m31-b', 31, depth + 0.5)):
    out = drv.go(f'{LABEL}-{tag}', mode, path, target, 0, LOG, outdir=RUNS)
    published = drv.published_raw(out)
    rows.append({'tag': tag, 'mode': mode, 'published': repr(published),
                 'belowIncumbent': published is not None and published < RAW})
    if published is None:
        continue
    # Whatever legalized, hand it to the schedule and see whether the lane can
    # now represent it.
    repaired = f'{FIX}/{LABEL}-{tag}-repaired.json'
    lib.pin(out, repaired, f'{LABEL} regrid {tag} repair of {PIN}')
    for spec in ('past=1,work=20000000,step=0.25',
                 'past=1,work=60000000,step=0.25'):
        doc, _ = sched.sched_arm(f'{LABEL}-{tag}-m34-{spec[-4:]}', repaired,
                                 published - 0.3, 5, spec, logfile=LOG,
                                 outdir=RUNS)
        pop = lib.population(doc) or {}
        schedule = pop.get('compressionSchedule') or {}
        rows.append({'tag': f'{tag}+m34', 'spec': spec,
                     'parentProxyFeasible': schedule.get('parentProxyFeasible'),
                     'parentCollisionPairs': schedule.get('parentCollisionPairs'),
                     'startDepthMm': schedule.get('startDepthMm'),
                     'floorDepthMm': schedule.get('floorDepthMm'),
                     'confirmationsAccepted':
                     schedule.get('confirmationsAccepted'),
                     'published': repr(drv.published_raw(doc)),
                     'belowIncumbent': (drv.published_raw(doc) is not None
                                        and drv.published_raw(doc) < RAW)})

result = {'label': LABEL, 'pin': PIN, 'declaredRaw': repr(RAW),
          'posesMoved': moved, 'snappedDepthMm': depth,
          'snapEntryLossMm': depth - RAW, 'rows': rows,
          'anyBelow': any(r['belowIncumbent'] for r in rows)}
print(json.dumps(result, indent=1))
json.dump(result, open(f'{OUT}/regrid.json', 'w'), indent=1)
