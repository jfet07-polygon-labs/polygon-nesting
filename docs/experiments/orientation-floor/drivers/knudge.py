#!/usr/bin/env python3
"""The k-deepest-pieces perturbation, swept over k, d and the repair mode.

    python3 knudge.py LABEL PIN RAW KS DS MODES [JOBS] [SLACK]

`lib.nudge` moves the k deepest pieces by TRUE transformed max-Y in by `d` mm
along the depth axis; the perturbed state is handed to the repair mode as the
PARENT fixture, which is the entry law modes 26-33 read.

The band is chosen from the frontier stack rather than carried over: the
displacement-cap law was measured at 164 mm, where k=2-3 pieces and d=1-2 mm was
productive, and this incumbent's stack has *seven* pieces inside 0.040 mm, so
the same law asks for a larger k and a much smaller d.
"""
import concurrent.futures
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import drv  # noqa: E402
import lib  # noqa: E402

LABEL, PIN, RAW = sys.argv[1], sys.argv[2], float(sys.argv[3])
KS = [int(k) for k in sys.argv[4].split(',')]
DS = [float(d) for d in sys.argv[5].split(',')]
MODES = [int(m) for m in sys.argv[6].split(',')]
JOBS = int(sys.argv[7]) if len(sys.argv) > 7 else 3
SLACK = float(sys.argv[8]) if len(sys.argv) > 8 else 2.0
OUT = f'/var/lib/t3/tmp/wf87/run/{LABEL}'
LOG = f'{OUT}/sweep.log'
os.makedirs(f'{OUT}/fix', exist_ok=True)

placements = json.load(open(PIN))['placements']
FIXTURES = {}
for k in KS:
    for d in DS:
        path = f'{OUT}/fix/knudge-k{k}-d{d}.json'
        depth = lib.write_fixture(
            path, f'{k} deepest pieces moved in by {d} mm from {PIN}',
            lib.nudge(placements, k, d))
        FIXTURES[(k, d)] = (path, depth)
        drv.log(LOG, f'-- knudge k={k} d={d}: entry depth {depth:.9f}')


def one(k, d, mode):
    path, depth = FIXTURES[(k, d)]
    tag = f'{LABEL}-k{k}-d{d}-m{mode}'
    # Modes 26 and 31 read the target as a bound to walk down to, so they are
    # given one below the incumbent; the re-insertion modes are given a slack
    # above it, which is the alternation fixpoint's own contract.
    target = RAW - 0.3 if mode in (26, 31) else RAW + SLACK
    out = drv.go(tag, mode, path, target, 0, LOG, outdir=f'{OUT}/runs')
    pop = lib.population(out) or {}
    published = drv.published_raw(out)
    return {'tag': tag, 'k': k, 'd': d, 'mode': mode, 'entryDepth': depth,
            'published': published, 'publishedRepr': repr(published),
            'rawAny': repr(pop.get('rawSourceDepthMm')),
            'exactValid': pop.get('exactValid'),
            'contractValid': pop.get('contractValid'),
            'attribution': drv.attribution(out),
            'below': published is not None and published < RAW,
            'run': f'{OUT}/runs/{tag}.json'}


rows = []
with concurrent.futures.ThreadPoolExecutor(max_workers=JOBS) as pool:
    for row in pool.map(lambda job: one(*job),
                        [(k, d, m) for k in KS for d in DS for m in MODES]):
        rows.append(row)
        print(f"{row['tag']:44s} -> {row['publishedRepr']:22s} "
              f"(any {row['rawAny']:22s} ev={row['exactValid']}) "
              f"below={row['below']}", flush=True)

rows.sort(key=lambda r: (r['published'] is None, r['published']))
json.dump({'label': LABEL, 'pin': PIN, 'declaredRaw': repr(RAW),
           'binary': lib.BIN, 'ks': KS, 'ds': DS, 'modes': MODES,
           'arms': len(rows), 'armsBelow': sum(1 for r in rows if r['below']),
           'rows': rows}, open(f'{OUT}/sweep.json', 'w'), indent=1)
print(json.dumps({'best': rows[0]['tag'], 'bestPublished': rows[0]['publishedRepr'],
                  'below': rows[0]['below'], 'bestRun': rows[0]['run'],
                  'armsBelow': sum(1 for r in rows if r['below']),
                  'arms': len(rows)}, indent=1))
