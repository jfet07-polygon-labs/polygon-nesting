#!/usr/bin/env python3
"""Mode-23 crossover between one incumbent and a pool, both directions.

    python3 crosssweep.py LABEL PIN RAW POOL CUTS [SEEDS] [JOBS]

Mode 23 reads the target slot as a **scale-free cut fraction in (0, 1)** of
parent A's own measured short-axis span, and reads the warm-start slot (CLI
argument 46) as parent B. Both directions are run because the cut is taken in
A's frame, so (A, B) and (B, A) are different operators, not a symmetry.

The record line may draw parent B from anywhere. The **from-scratch** line may
not: its whole value is that it reached the depth without importing a
record-line placement, so a pool with a record co-state in it destroys exactly
the claim it is being run to support. This driver does not know which line it is
on - the caller passes the pool, and the caller is responsible for that.
"""
import concurrent.futures
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import drv  # noqa: E402
import lib  # noqa: E402

LABEL, PIN, RAW = sys.argv[1], sys.argv[2], float(sys.argv[3])
POOL = [p for p in sys.argv[4].split(':') if p]
CUTS = [float(c) for c in sys.argv[5].split(',')]
SEEDS = [int(s) for s in sys.argv[6].split(',')] if len(sys.argv) > 6 else [0]
JOBS = int(sys.argv[7]) if len(sys.argv) > 7 else 4
OUT = f'/var/lib/t3/tmp/wf87/run/{LABEL}'
LOG = f'{OUT}/cross.log'
RUNS = f'{OUT}/runs'
os.makedirs(RUNS, exist_ok=True)


def one(other, cut, seed, side):
    name = os.path.basename(other).replace('.json', '')[:26]
    tag = f'{LABEL}-x{side}-{name}-c{cut}-s{seed}'
    a, b = (PIN, other) if side == 'ab' else (other, PIN)
    started = time.time()
    out = lib.run(tag, 23, a, f'{cut:.6f}', seed, RUNS, warm=b)
    drv.log(LOG, f'[{time.time() - started:7.1f}s] ' + lib.line(tag, out))
    published = drv.published_raw(out)
    return {'tag': tag, 'side': side, 'parentA': a, 'parentB': b, 'cut': cut,
            'seed': seed, 'published': published,
            'publishedRepr': repr(published),
            'below': published is not None and published < RAW,
            'run': f'{RUNS}/{tag}.json'}


jobs = [(other, cut, seed, side)
        for other in POOL for cut in CUTS for seed in SEEDS
        for side in ('ab', 'ba')
        if os.path.abspath(other) != os.path.abspath(PIN)]
rows = []
with concurrent.futures.ThreadPoolExecutor(max_workers=JOBS) as pool:
    for row in pool.map(lambda job: one(*job), jobs):
        rows.append(row)
        print(f"{row['tag']:58s} -> {row['publishedRepr']:22s} "
              f"below={row['below']}", flush=True)

rows.sort(key=lambda r: (r['published'] is None, r['published']))
json.dump({'label': LABEL, 'pin': PIN, 'declaredRaw': repr(RAW), 'pool': POOL,
           'cuts': CUTS, 'seeds': SEEDS, 'arms': len(rows),
           'armsBelow': sum(1 for r in rows if r['below']), 'rows': rows},
          open(f'{OUT}/cross.json', 'w'), indent=1)
print(json.dumps({'arms': len(rows),
                  'armsBelow': sum(1 for r in rows if r['below']),
                  'best': rows[0]['tag'] if rows else None,
                  'bestPublished': rows[0]['publishedRepr'] if rows else None,
                  'bestRun': rows[0]['run'] if rows else None}, indent=1))
