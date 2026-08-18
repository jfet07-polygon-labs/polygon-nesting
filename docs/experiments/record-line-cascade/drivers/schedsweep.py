#!/usr/bin/env python3
"""A grid of mode-34 schedule arms on one parent, run concurrently.

    python3 schedsweep.py LABEL PIN RAW DROP SPEC[,SPEC...] [SEEDS] [JOBS]

Concurrency is safe here and only here: mode 34's budget is the schedule's own
`work_cap_queries` in its own deterministic, load-independent currency, so an
arm stops at the same step whichever else is running. No wall-clock number from
a concurrent run is a measurement of anything, and none is claimed - the
`wallSeconds` column is reported so a reader can see the arms were concurrent.
"""
import concurrent.futures
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import drv  # noqa: E402
import lib  # noqa: E402
import sched  # noqa: E402

LABEL = sys.argv[1]
PIN, RAW, DROP = sys.argv[2], float(sys.argv[3]), float(sys.argv[4])
SPECS = [s for s in sys.argv[5].split(';') if s]
SEEDS = [int(s) for s in sys.argv[6].split(',')] if len(sys.argv) > 6 else [5]
JOBS = int(sys.argv[7]) if len(sys.argv) > 7 else 4
OUT = f'/var/lib/t3/tmp/recordline/{LABEL}'
LOG = f'{OUT}/sweep.log'
os.makedirs(OUT, exist_ok=True)


def one(index, spec, seed):
    tag = f'{LABEL}-a{index}-s{seed}'
    doc, wall = sched.sched_arm(tag, PIN, RAW - DROP, seed, spec, logfile=LOG,
                                outdir=f'{OUT}/runs')
    pop = lib.population(doc) or {}
    schedule = {k: v for k, v in (pop.get('compressionSchedule') or {}).items()
                if not isinstance(v, (dict, list))}
    return {'tag': tag, 'spec': spec, 'seed': seed,
            'published': drv.published_raw(doc),
            'publishedRepr': repr(drv.published_raw(doc)),
            'below': (drv.published_raw(doc) is not None
                      and drv.published_raw(doc) < RAW),
            'fingerprint': pop.get('finalPlacementFingerprint'),
            'wallSeconds': wall, 'schedule': schedule}


jobs = [(index, spec, seed) for seed in SEEDS
        for index, spec in enumerate(SPECS)]
rows = []
with concurrent.futures.ThreadPoolExecutor(max_workers=JOBS) as pool:
    for row in pool.map(lambda job: one(*job), jobs):
        rows.append(row)
        print(f"{row['tag']:32s} {row['spec']:52s} "
              f"-> {row['publishedRepr']:22s} below={row['below']} "
              f"({row['wallSeconds']:.0f}s)", flush=True)

rows.sort(key=lambda r: (r['published'] is None, r['published']))
json.dump({'label': LABEL, 'pin': PIN, 'declaredRaw': repr(RAW), 'drop': DROP,
           'rows': rows}, open(f'{OUT}/sweep.json', 'w'), indent=1)
best = rows[0]
print(json.dumps({'best': best['tag'], 'bestSpec': best['spec'],
                  'bestPublished': best['publishedRepr'],
                  'bestRun': f"{OUT}/runs/{best['tag']}.json"}, indent=1))
