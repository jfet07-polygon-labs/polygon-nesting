#!/usr/bin/env python3
"""Convert a Sparrow solution JSON (its output format) into our placements JSON
(the benchmark's `placements_json` shape) for a diagnostic start.

    from-sparrow-solution.py <sparrow-solution.json> <out-placements.json>

Sparrow ring: p' = R(theta) p + t in (x_s, y_s); our frame: sheet_long = x_s, sheet_short = y_s.
Our placement: sheet_short = mx*cos - y*sin + t_short, sheet_long = mx*sin + y*cos + t_long,
mx = -x if mirrored else x. Fit (rotation, mirrored, t_short, t_long) by brute force over
mirrored, cyclic shifts and vertex order; verified to 1e-6 mm on every vertex.
"""
import json, math, sys
src, out = sys.argv[1:3]
s = json.load(open(src)); items = {it['id']: it for it in s['items']}
def ring(it):
    pts = it['shape']['data']
    if pts[0] == pts[-1]: pts = pts[:-1]
    return [(float(x), float(y)) for x, y in pts]
placements = []; worst = 0.0
for pl in s['solution']['layout']['placed_items']:
    it = items[pl['item_id']]; pts = ring(it)
    th = math.radians(pl['transformation']['rotation']); c, sn = math.cos(th), math.sin(th)
    tx, ty = pl['transformation']['translation']
    # target in OUR frame: (sheet_short, sheet_long) = (y_s, x_s)
    tgt = [((sn * x + c * y + ty), (c * x - sn * y + tx)) for x, y in pts]
    best = None
    for mirrored in (False, True):
        msrc = [((-x if mirrored else x), y) for x, y in pts]
        for order in (msrc, msrc[::-1]):
            for shift in range(len(order)):
                cand = order[shift:] + order[:shift]
                ax, ay = cand[1][0] - cand[0][0], cand[1][1] - cand[0][1]
                bx, by = tgt[1][0] - tgt[0][0], tgt[1][1] - tgt[0][1]
                if abs(math.hypot(ax, ay) - math.hypot(bx, by)) > 1e-6: continue
                phi = math.atan2(by, bx) - math.atan2(ay, ax); cs, si = math.cos(phi), math.sin(phi)
                # our transform: short = mx*cos - y*sin + ts ; long = mx*sin + y*cos + tl
                ts = tgt[0][0] - (cand[0][0] * cs - cand[0][1] * si)
                tl = tgt[0][1] - (cand[0][0] * si + cand[0][1] * cs)
                err = max(math.hypot(px * cs - py * si + ts - qs, px * si + py * cs + tl - ql) for (px, py), (qs, ql) in zip(cand, tgt))
                if err < 1e-6 and (best is None or err < best[0]): best = (err, math.degrees(phi) % 360.0, mirrored, ts, tl)
    if best is None: raise SystemExit(f'no fit for item {pl["item_id"]}')
    worst = max(worst, best[0])
    placements.append({'pieceId': it['dxf'] if 'dxf' in it else None, 'itemId': it['id'], 'rotationDeg': best[1], 'mirrored': best[2],
                       'translateShortAxis': best[3], 'translateLongAxis': best[4]})
json.dump({'source': src, 'stripWidth': s['solution']['strip_width'], 'placements': placements}, open(out, 'w'), indent=1)
print(f'{len(placements)} placements, strip width {s["solution"]["strip_width"]}, worst vertex error {worst:.2e}, mirrored {sum(1 for p in placements if p["mirrored"])}, missing pieceId {sum(1 for p in placements if p["pieceId"] is None)}')
