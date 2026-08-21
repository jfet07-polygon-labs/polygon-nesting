#!/usr/bin/env python3
"""How many of a published layout's rotations are off the 2.5-degree grid.

    offgrid.py RUN.json [RUN.json ...]

The publication-tier half of the operator's decomposition. The proxy-tier
counters say how many rungs were proposed and how many were accepted *into the
lane's state*; this says how many survived all the way onto the sheet, which is
the only tier whose opinion is authoritative. A run whose published layout is
entirely grid-native has had every one of its rungs rejected or overwritten
downstream, and that is a different finding from "the operator proposed
nothing".

`canonical_angle`'s grid is 2.5 degrees and the engine's angle-key quantum is
1e-6 degrees, so "off grid" here is exactly the predicate
`angle_key(rotation) != angle_key(canonical_angle(rotation))` the engine itself
uses.
"""
import json
import sys

STEP_DEG = 2.5
KEY_SCALE = 1_000_000.0


def angle_key(deg):
    return round((deg % 360.0) * KEY_SCALE)


def canonical_key(deg):
    normalized = deg % 360.0
    return angle_key(round(normalized / STEP_DEG) * STEP_DEG)


def main():
    rows = []
    for path in sys.argv[1:]:
        try:
            doc = json.load(open(path))
        except (json.JSONDecodeError, FileNotFoundError):
            rows.append({'run': path, 'error': 'unreadable'})
            continue
        placements = doc.get('placements') or []
        off = [p for p in placements
               if angle_key(p['rotationDeg']) != canonical_key(p['rotationDeg'])]
        rows.append({
            'run': path,
            'pieces': len(placements),
            'offGridPieces': len(off),
            'mirroredPieces': sum(1 for p in placements if p.get('mirrored')),
            'depthMm': doc.get('independentUsedLongAxisDepthMm'),
            'maxOffGridDeltaDeg': max(
                (abs(((p['rotationDeg'] % 360.0)
                      - round((p['rotationDeg'] % 360.0) / STEP_DEG)
                      * STEP_DEG)) for p in off), default=0.0),
        })
    print(json.dumps({'runs': rows}, indent=1))


if __name__ == '__main__':
    main()
