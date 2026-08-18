#!/usr/bin/env python3
"""Frontier-flatten entry grid handed to mode 33 (or 32), run concurrently.

    python3 flatsweep.py LABEL PIN RAW DELTAS SLACKS [MODE] [JOBS] [SEEDS]

The entry grid is a lever in its own right (the finer-ladder round's
from-scratch adoption came from adding a single flatten delta, not from the
rungs), so this driver sweeps it wide rather than reusing the cascade's six
fixed deltas. `BENCH_BIN` selects the binary, so the same grid can be run under
two ladder generations as a paired A/B.

Concurrency changes wall time only: every arm is seeded and budgeted in the
engine's own deterministic counters and no wall-clock claim is made here.
"""
import concurrent.futures
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import drv  # noqa: E402
import lib  # noqa: E402

LABEL, PIN, RAW = sys.argv[1], sys.argv[2], float(sys.argv[3])
DELTAS = [float(d) for d in sys.argv[4].split(',')]
SLACKS = [float(s) for s in sys.argv[5].split(',')]
MODE = int(sys.argv[6]) if len(sys.argv) > 6 else 33
JOBS = int(sys.argv[7]) if len(sys.argv) > 7 else 4
SEEDS = [int(s) for s in sys.argv[8].split(',')] if len(sys.argv) > 8 else [0]
OUT = f'/var/lib/t3/tmp/wf87/run/{LABEL}'
LOG = f'{OUT}/sweep.log'
os.makedirs(f'{OUT}/fix', exist_ok=True)

FIXTURES = {}
for delta in DELTAS:
    path, depth, moved = drv.flatten_fixture(delta, PIN, f'{LABEL}-{delta}',
                                             outdir=f'{OUT}/fix')
    FIXTURES[delta] = (path, depth, len(moved))
    drv.log(LOG, f'-- flatten {delta}: depth {depth:.9f}, {len(moved)} moved')


def one(delta, slack, seed):
    path, depth, moved = FIXTURES[delta]
    tag = f'{LABEL}-flat{delta}-m{MODE}-p{slack}-s{seed}'
    out = drv.go(tag, MODE, path, RAW + slack, seed, LOG, outdir=f'{OUT}/runs')
    pop = lib.population(out) or {}
    published = drv.published_raw(out)
    return {'tag': tag, 'mode': MODE, 'delta': delta, 'slack': slack,
            'seed': seed, 'movedPieces': moved, 'entryDepth': depth,
            'published': published, 'publishedRepr': repr(published),
            'rawAny': repr(pop.get('rawSourceDepthMm')),
            'exactValid': pop.get('exactValid'),
            'contractValid': pop.get('contractValid'),
            'below': published is not None and published < RAW,
            'run': f'{OUT}/runs/{tag}.json'}


jobs = [(d, s, sd) for d in DELTAS for s in SLACKS for sd in SEEDS]
rows = []
with concurrent.futures.ThreadPoolExecutor(max_workers=JOBS) as pool:
    for row in pool.map(lambda job: one(*job), jobs):
        rows.append(row)
        print(f"{row['tag']:52s} -> {row['publishedRepr']:22s} "
              f"(any {row['rawAny']:22s} ev={row['exactValid']}) "
              f"below={row['below']}", flush=True)

rows.sort(key=lambda r: (r['published'] is None, r['published']))
json.dump({'label': LABEL, 'mode': MODE, 'pin': PIN, 'declaredRaw': repr(RAW),
           'binary': lib.BIN, 'deltas': DELTAS, 'slacks': SLACKS,
           'seeds': SEEDS, 'arms': len(rows),
           'armsBelow': sum(1 for r in rows if r['below']), 'rows': rows},
          open(f'{OUT}/sweep.json', 'w'), indent=1)
print(json.dumps({'best': rows[0]['tag'], 'bestPublished': rows[0]['publishedRepr'],
                  'below': rows[0]['below'], 'bestRun': rows[0]['run'],
                  'armsBelow': sum(1 for r in rows if r['below']),
                  'arms': len(rows)}, indent=1))
