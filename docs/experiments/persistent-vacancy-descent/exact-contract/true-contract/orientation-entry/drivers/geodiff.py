#!/usr/bin/env python3
"""Geometric diff of two pinned fixtures: rotation, mirror and translation.

The pose-entry negative's headline measurement was maxdRot 0.000000 and
mirrorFlips 0 across every state modes 28/29 produced. This is the same
measurement, run against the states modes 32/33 produced.
"""
import sys, json


def load(path):
    return {p['pieceId']: p for p in json.load(open(path))['placements']}


def angle_delta(a, b):
    d = (b - a) % 360.0
    return d - 360.0 if d > 180.0 else d


first, second = load(sys.argv[1]), load(sys.argv[2])
rot, mirror, moved = [], 0, 0
for pid, a in first.items():
    b = second[pid]
    delta = angle_delta(a['rotationDeg'], b['rotationDeg'])
    if abs(delta) > 1e-9:
        rot.append((pid, a['rotationDeg'], b['rotationDeg'], delta))
    if a['mirrored'] != b['mirrored']:
        mirror += 1
    if (abs(a['translateShortAxis'] - b['translateShortAxis']) > 1e-9
            or abs(a['translateLongAxis'] - b['translateLongAxis']) > 1e-9):
        moved += 1
print(json.dumps({
    'pieces': len(first),
    'rotationChanges': len(rot),
    'maxAbsRotationDeltaDeg': max((abs(r[3]) for r in rot), default=0.0),
    'mirrorFlips': mirror,
    'translatedPieces': moved,
    'rotated': [{'pieceId': p, 'fromDeg': a, 'toDeg': b, 'deltaDeg': d} for p, a, b, d in rot],
}, indent=1))
