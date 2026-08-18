#!/usr/bin/env python3
"""A grid of one mode's arms on one parent, run concurrently.

    python3 armsweep.py LABEL MODE PIN RAW DROPS SEEDS [JOBS]

`DROPS` is a comma list subtracted from the declared raw to form each arm's
bound (mode 26 reads it as the sheet long axis its ladder walks down to; mode 31
as a hard containment bound; mode 22 wants a *slack* and should be given
negative drops). Used when a tier is known to be productive and the cascade's
adopt-and-restart order keeps starving it: the 156.091 certification found six
of six mode-26 arms below the incumbent while 555 cascade arms never reached
that tier, because the 3 s tiers kept adopting first.

Concurrency changes wall time only. Every arm is seeded and budgeted in the
engine's own deterministic counters, and no wall-clock claim is made from a run
of this driver.
"""
import concurrent.futures
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import drv  # noqa: E402
import lib  # noqa: E402

LABEL, MODE, PIN, RAW = sys.argv[1], int(sys.argv[2]), sys.argv[3], float(sys.argv[4])
DROPS = [float(d) for d in sys.argv[5].split(',')]
SEEDS = [int(s) for s in sys.argv[6].split(',')]
JOBS = int(sys.argv[7]) if len(sys.argv) > 7 else 4
OUT = f'/var/lib/t3/tmp/wf87/run/{LABEL}'
LOG = f'{OUT}/sweep.log'
os.makedirs(OUT, exist_ok=True)


def one(drop, seed):
    tag = f'{LABEL}-m{MODE}-d{drop}-s{seed}'
    out = drv.go(tag, MODE, PIN, RAW - drop, seed, LOG, outdir=f'{OUT}/runs')
    published = drv.published_raw(out)
    return {'tag': tag, 'mode': MODE, 'drop': drop, 'seed': seed,
            'published': published, 'publishedRepr': repr(published),
            'below': published is not None and published < RAW,
            'run': f'{OUT}/runs/{tag}.json'}


rows = []
with concurrent.futures.ThreadPoolExecutor(max_workers=JOBS) as pool:
    for row in pool.map(lambda job: one(*job),
                        [(d, s) for d in DROPS for s in SEEDS]):
        rows.append(row)
        print(f"{row['tag']:44s} -> {row['publishedRepr']:22s} "
              f"below={row['below']}", flush=True)

rows.sort(key=lambda r: (r['published'] is None, r['published']))
json.dump({'label': LABEL, 'mode': MODE, 'pin': PIN, 'declaredRaw': repr(RAW),
           'rows': rows}, open(f'{OUT}/sweep.json', 'w'), indent=1)
best = rows[0]
print(json.dumps({'best': best['tag'], 'bestPublished': best['publishedRepr'],
                  'below': best['below'], 'bestRun': best['run'],
                  'armsBelow': sum(1 for r in rows if r['below']),
                  'arms': len(rows)}, indent=1))
