#!/usr/bin/env python3
"""An independent, out-of-engine check of a witness layout.

    python3 verify_witness.py <parent fixture> <certificate json> <program> <motion>

The certificate reports that `validate_publication` accepted its witness and
that the publication measure came back shallower than the parent. Both of those
calls live in the same crate as the thing under test, so this driver re-derives
them from the *request* with no engine code in the loop:

  * the transform is re-implemented from `transform_source_ring` (mirror, then
    rotate, then translate);
  * containment is `5 <= x <= 2000 - 5` and `5 <= y <= 2700 - 5`, the same
    strict-inequality test `validate_sheet` applies;
  * pair clearance is the true segment-to-segment minimum distance against the
    contract, computed by brute force;
  * the published depth is `max(y) + sheet edge clearance`.

The contract is the branch's **true 5.0/5.0 exact clearance**, not the
request document's `padding: 10`: the pinned CLI tail overrides it, which is why
`record-line-cascade/drivers/lib.py` carries `EDGE_CLEARANCE_MM = 5.0`. Two
independent facts confirm the 5.0 here rather than leaving it asserted — the
parent's own worst pair distance measures 5.004 mm, and the certificate's
`parentWorstResidualMm.MaterialPair`, computed by entirely separate code inside
the engine, is 0.004.

## What this does NOT check

`contractValid`. The engine's contract gate is the grid-quantized **collision
envelope**, and `validate_publication` never looks at one; neither does this
script. That matters here because the parent's `EnvelopePair` slack is exactly
`0.0` — the envelopes are already touching — and because a witness direction
labelled `modelObjective` was not required to satisfy the model's envelope rows
in the first place. So a layout that passes everything below may still be
refused by the contract gate, and nothing here may be read as a record claim.

It is calibrated before it is trusted: the same code is run on the **parent**
first, and its depth must reproduce the parent's pinned depth to the ULP. If
the transform did not match the engine's, that check fails and nothing below it
means anything.

All segments in this request are `line`, so no arc handling is needed; the
script asserts that rather than assuming it.
"""
import json
import math
import sys

REQUEST = ('/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/'
           'wf_b7992967-b13-3/tests/fixtures/mixed-61/'
           'mixed61-request-exact-clearance.json')
EDGE_CLEARANCE_MM = 5.0
PAIR_CLEARANCE_MM = 5.0


def rings():
    request = json.load(open(REQUEST))
    sheet = request['sheet']
    out = {}
    for piece in request['sourcePieces']:
        geometry = piece['geometry']
        points = []
        for segment in geometry['segments']:
            assert segment['kind'] == 'line', segment['kind']
            points.append((segment['x1'], segment['y1']))
        out[piece['id']] = points
    return out, sheet


def transform(points, rotation_deg, mirrored, tx, ty):
    radians = math.radians(rotation_deg)
    sin, cos = math.sin(radians), math.cos(radians)
    moved = []
    for x, y in points:
        mx = -x if mirrored else x
        moved.append((mx * cos - y * sin + tx, mx * sin + y * cos + ty))
    return moved


def edges(ring):
    return [(ring[i], ring[(i + 1) % len(ring)]) for i in range(len(ring))]


def segment_distance(a, b, c, d):
    def point_segment(p, q, r):
        qx, qy = q
        rx, ry = r
        dx, dy = rx - qx, ry - qy
        if dx == 0.0 and dy == 0.0:
            return math.hypot(p[0] - qx, p[1] - qy)
        t = ((p[0] - qx) * dx + (p[1] - qy) * dy) / (dx * dx + dy * dy)
        t = max(0.0, min(1.0, t))
        return math.hypot(p[0] - (qx + t * dx), p[1] - (qy + t * dy))
    return min(point_segment(a, c, d), point_segment(b, c, d),
               point_segment(c, a, b), point_segment(d, a, b))


def check(layout, sheet):
    """(published depth, worst containment margin, worst pair distance)."""
    deepest = -math.inf
    margin = math.inf
    for _piece_id, ring in layout:
        for x, y in ring:
            deepest = max(deepest, y)
            margin = min(margin, x - EDGE_CLEARANCE_MM,
                         y - EDGE_CLEARANCE_MM,
                         sheet['width'] - EDGE_CLEARANCE_MM - x,
                         sheet['height'] - EDGE_CLEARANCE_MM - y)
    worst_pair = math.inf
    for i in range(len(layout)):
        for j in range(i + 1, len(layout)):
            for a, b in edges(layout[i][1]):
                for c, d in edges(layout[j][1]):
                    worst_pair = min(worst_pair, segment_distance(a, b, c, d))
    return deepest + EDGE_CLEARANCE_MM, margin, worst_pair


if __name__ == '__main__':
    parent_path, certificate_path, program, motion = sys.argv[1:5]
    source, sheet = rings()
    parent = json.load(open(parent_path))
    certificate = json.load(open(certificate_path))

    def layout_of(vector_by_id):
        out = []
        for placement in parent['placements']:
            piece_id = placement['pieceId']
            dx, dy, dtheta_deg = vector_by_id.get(piece_id, (0.0, 0.0, 0.0))
            # The witness rotates about the piece's own transformed centroid,
            # so reproduce that: rotate the already-transformed ring about its
            # centroid, then translate.
            ring = transform(source[piece_id], placement['rotationDeg'],
                             placement['mirrored'],
                             placement['translateShortAxis'],
                             placement['translateLongAxis'])
            if dtheta_deg or dx or dy:
                cx = sum(p[0] for p in ring) / len(ring)
                cy = sum(p[1] for p in ring) / len(ring)
                radians = math.radians(dtheta_deg)
                sin, cos = math.sin(radians), math.cos(radians)
                ring = [((x - cx) * cos - (y - cy) * sin + cx + dx,
                         (x - cx) * sin + (y - cy) * cos + cy + dy)
                        for x, y in ring]
            out.append((piece_id, ring))
        return out

    parent_layout = layout_of({})
    parent_depth, parent_margin, parent_pair = check(parent_layout, sheet)
    pinned = certificate['publishedDepthMm']
    calibrated = abs(parent_depth - pinned) < 1e-9
    result = {'parentDepthMm': parent_depth, 'certificateParentDepthMm': pinned,
              'CALIBRATED': calibrated,
              'parentWorstContainmentMarginMm': parent_margin,
              'parentWorstPairDistanceMm': parent_pair}

    if calibrated:
        chosen = next(p for p in certificate['programs']
                      if p['program'] == program and p['motion'] == motion)
        vector = {pid: (dx, dy, dth)
                  for pid, dx, dy, dth in chosen['witness']['vector']}
        moved_depth, moved_margin, moved_pair = check(layout_of(vector), sheet)
        result.update({
            'program': program, 'motion': motion,
            'witnessScale': chosen['witness']['scale'],
            'movedDepthMm': moved_depth,
            'engineMovedDepthMm': chosen['witness']['publishedDepthMm'],
            'independentDeltaMm': parent_depth - moved_depth,
            'engineDeltaMm': chosen['witness']['deltaMm'],
            'movedWorstContainmentMarginMm': moved_margin,
            'movedWorstPairDistanceMm': moved_pair,
            'CONTAINMENT_OK': moved_margin >= 0.0,
            'PAIR_CLEARANCE_OK': moved_pair >= PAIR_CLEARANCE_MM,
        })
    print(json.dumps(result, indent=1))
