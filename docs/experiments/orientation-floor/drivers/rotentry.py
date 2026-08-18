#!/usr/bin/env python3
"""Rotation entries: perturb the frontier pieces' *poses*, not their positions.

    python3 rotentry.py LABEL PIN RAW KS DEGS MODES [JOBS] [SLACK]

Every entry family this line has used is a translation: the frontier flatten
moves pieces in along the depth axis, and the k-deepest nudge does the same to a
chosen few. The orientation degree of freedom has only ever been reachable from
*inside* modes 32 and 33, as a candidate stream. This driver puts it in the
entry instead, so it can be handed to the legalization tiers - which is the
composition §5 found to matter and which the re-insertion modes cannot express,
because their own orientation ladder only perturbs the pieces *they* ejected.

The rotation is applied in place, about each piece's own transformed bounding-box
centre, by the same construction the engine's orientation stream uses: for a
placement `R(r)·s + T` whose footprint centre is `C = R(r)·c + T`, the rotated
placement is `R(r+d)·s + T'` with `T' = C - R(r+d)·c`, so the piece turns without
translating. `d` is drawn from the ladder itself, including the new 0.00128 rung.
"""
import concurrent.futures
import json
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import drv  # noqa: E402
import lib  # noqa: E402

LABEL, PIN, RAW = sys.argv[1], sys.argv[2], float(sys.argv[3])
KS = [int(k) for k in sys.argv[4].split(',')]
DEGS = [float(d) for d in sys.argv[5].split(',')]
MODES = [int(m) for m in sys.argv[6].split(',')]
JOBS = int(sys.argv[7]) if len(sys.argv) > 7 else 4
SLACK = float(sys.argv[8]) if len(sys.argv) > 8 else 2.0
OUT = f'/var/lib/t3/tmp/wf87/run/{LABEL}'
LOG = f'{OUT}/sweep.log'
os.makedirs(f'{OUT}/fix', exist_ok=True)

PLACEMENTS = json.load(open(PIN))['placements']


def source_centre(placement):
    """The source-frame bounding-box centre, in the placement's mirror sense."""
    points = lib.SRC[lib.PIECE_SRC[placement['pieceId']]]
    xs = [(-x if placement['mirrored'] else x) for (x, _) in points]
    ys = [y for (_, y) in points]
    return (min(xs) + max(xs)) / 2.0, (min(ys) + max(ys)) / 2.0


def rotate_in_place(placement, degrees):
    cx, cy = source_centre(placement)
    old = math.radians(placement['rotationDeg'])
    new = math.radians(placement['rotationDeg'] + degrees)
    # C = R(old)*c + T ; T' = C - R(new)*c
    centre_x = cx * math.cos(old) - cy * math.sin(old) + placement['translateShortAxis']
    centre_y = cx * math.sin(old) + cy * math.cos(old) + placement['translateLongAxis']
    short = centre_x - (cx * math.cos(new) - cy * math.sin(new))
    long_axis = centre_y - (cx * math.sin(new) + cy * math.cos(new))
    return dict(placement,
                rotationDeg=round(placement['rotationDeg'] + degrees, 6),
                translateShortAxis=round(short, 6),
                translateLongAxis=round(long_axis, 6))


FIXTURES = {}
for k in KS:
    extent = lib.extents(PLACEMENTS)
    ranked = sorted(PLACEMENTS, key=lambda p: (-extent[p['pieceId']][1],
                                               p['pieceId']))
    ids = {p['pieceId'] for p in ranked[:k]}
    for degrees in DEGS:
        out = [rotate_in_place(p, degrees) if p['pieceId'] in ids else dict(p)
               for p in PLACEMENTS]
        path = f'{OUT}/fix/rot-k{k}-d{degrees}.json'
        depth = lib.write_fixture(
            path, f'{k} deepest pieces rotated in place by {degrees} deg '
                  f'from {PIN}', out)
        FIXTURES[(k, degrees)] = (path, depth)
        drv.log(LOG, f'-- rotentry k={k} d={degrees}: entry depth {depth:.9f} '
                     f'({depth - RAW:+.6f})')


def one(k, degrees, mode):
    path, depth = FIXTURES[(k, degrees)]
    tag = f'{LABEL}-k{k}-d{degrees}-m{mode}'
    target = RAW - 0.3 if mode in (26, 31) else RAW + SLACK
    out = drv.go(tag, mode, path, target, 0, LOG, outdir=f'{OUT}/runs')
    pop = lib.population(out) or {}
    published = drv.published_raw(out)
    return {'tag': tag, 'k': k, 'deg': degrees, 'mode': mode,
            'entryDepth': depth, 'published': published,
            'publishedRepr': repr(published),
            'rawAny': repr(pop.get('rawSourceDepthMm')),
            'exactValid': pop.get('exactValid'),
            'contractValid': pop.get('contractValid'),
            'attribution': drv.attribution(out),
            'below': published is not None and published < RAW,
            'run': f'{OUT}/runs/{tag}.json'}


rows = []
with concurrent.futures.ThreadPoolExecutor(max_workers=JOBS) as pool:
    for row in pool.map(lambda job: one(*job),
                        [(k, d, m) for k in KS for d in DEGS for m in MODES]):
        rows.append(row)
        print(f"{row['tag']:44s} -> {row['publishedRepr']:22s} "
              f"(any {row['rawAny']:22s} ev={row['exactValid']}) "
              f"below={row['below']}", flush=True)

rows.sort(key=lambda r: (r['published'] is None, r['published']))
json.dump({'label': LABEL, 'pin': PIN, 'declaredRaw': repr(RAW),
           'binary': lib.BIN, 'ks': KS, 'degrees': DEGS, 'modes': MODES,
           'arms': len(rows), 'armsBelow': sum(1 for r in rows if r['below']),
           'rows': rows}, open(f'{OUT}/sweep.json', 'w'), indent=1)
print(json.dumps({'best': rows[0]['tag'],
                  'bestPublished': rows[0]['publishedRepr'],
                  'below': rows[0]['below'], 'bestRun': rows[0]['run'],
                  'armsBelow': sum(1 for r in rows if r['below']),
                  'arms': len(rows)}, indent=1))
