#!/usr/bin/env python3
"""The geometric diff between two pinned layouts.

    python3 geodiff.py PIN_A PIN_B

Reports which pieces rotated, translated or mirror-flipped, and by how much.
The finer-ladder round's argument that a rung the old ladder could not express
is what produced its record rests on exactly this diff, so it is a driver rather
than a one-off.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

A, B = sys.argv[1], sys.argv[2]
old = {p['pieceId']: p for p in json.load(open(A))['placements']}
new = {p['pieceId']: p for p in json.load(open(B))['placements']}
rotated = translated = flipped = 0
rows = []
for pid in sorted(old):
    a, b = old[pid], new[pid]
    d_rot = b['rotationDeg'] - a['rotationDeg']
    d_long = b['translateLongAxis'] - a['translateLongAxis']
    d_short = b['translateShortAxis'] - a['translateShortAxis']
    flip = a['mirrored'] != b['mirrored']
    if d_rot or d_long or d_short or flip:
        rows.append({'pieceId': pid, 'dRotDeg': d_rot, 'dLongMm': d_long,
                     'dShortMm': d_short, 'mirrorFlipped': flip})
        print(f'{pid:44s} dRot={d_rot:+.9g} dLong={d_long:+.9g} '
              f'dShort={d_short:+.9g} flip={flip}')
    rotated += bool(d_rot)
    translated += bool(d_long or d_short)
    flipped += flip
summary = {'a': A, 'b': B, 'rotated': rotated, 'translated': translated,
           'mirrorFlips': flipped, 'depthA': repr(lib.depth_mm(list(old.values()))),
           'depthB': repr(lib.depth_mm(list(new.values()))), 'rows': rows}
print(json.dumps({k: v for k, v in summary.items() if k != 'rows'}, indent=1))
if len(sys.argv) > 3:
    json.dump(summary, open(sys.argv[3], 'w'), indent=1)
