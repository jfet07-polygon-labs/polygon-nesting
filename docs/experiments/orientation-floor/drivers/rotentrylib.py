#!/usr/bin/env python3
"""The rotation-entry fixture generator, shared by `rotentry.py` and the cascade.

The rotation is applied *in place*, about each piece's own transformed
bounding-box centre, by the same construction the engine's orientation stream
uses: for a placement `R(r)·s + T` whose footprint centre is `C = R(r)·c + T`,
the rotated placement is `R(r+d)·s + T'` with `T' = C - R(r+d)·c`. Without the
re-centring a rung on a piece whose material sits far from the source origin is
a translation, which is a different pocket rather than a different orientation.
"""
import json
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402


def source_centre(placement):
    """The source-frame bounding-box centre, in the placement's mirror sense."""
    points = lib.SRC[lib.PIECE_SRC[placement['pieceId']]]
    xs = [(-x if placement['mirrored'] else x) for (x, _) in points]
    ys = [y for (_, y) in points]
    return (min(xs) + max(xs)) / 2.0, (min(ys) + max(ys)) / 2.0


def rotate_in_place(placement, degrees):
    centre_x_src, centre_y_src = source_centre(placement)
    old = math.radians(placement['rotationDeg'])
    new = math.radians(placement['rotationDeg'] + degrees)
    centre_x = (centre_x_src * math.cos(old) - centre_y_src * math.sin(old)
                + placement['translateShortAxis'])
    centre_y = (centre_x_src * math.sin(old) + centre_y_src * math.cos(old)
                + placement['translateLongAxis'])
    short = centre_x - (centre_x_src * math.cos(new)
                        - centre_y_src * math.sin(new))
    long_axis = centre_y - (centre_x_src * math.sin(new)
                            + centre_y_src * math.cos(new))
    return dict(placement,
                rotationDeg=round(placement['rotationDeg'] + degrees, 6),
                translateShortAxis=round(short, 6),
                translateLongAxis=round(long_axis, 6))


def rotation_fixture(pin, k, degrees, path):
    """Rotate the k deepest pieces by `degrees` in place; return (path, depth)."""
    placements = json.load(open(pin))['placements']
    extent = lib.extents(placements)
    ranked = sorted(placements,
                    key=lambda p: (-extent[p['pieceId']][1], p['pieceId']))
    ids = {p['pieceId'] for p in ranked[:k]}
    out = [rotate_in_place(p, degrees) if p['pieceId'] in ids else dict(p)
           for p in placements]
    os.makedirs(os.path.dirname(path), exist_ok=True)
    depth = lib.write_fixture(
        path, f'{k} deepest pieces rotated in place by {degrees} deg from {pin}',
        out)
    return path, depth
