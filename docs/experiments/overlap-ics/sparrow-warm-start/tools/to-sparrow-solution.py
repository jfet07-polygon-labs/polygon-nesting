#!/usr/bin/env python3
"""Convert one of our benchmark documents' incumbent placements into a Sparrow
solution JSON for warm starting.

    to-sparrow-solution.py <cell.json> <sparrow-input.json> <out.json>

Our placement: sheet_short = mx*cos - y*sin + t_short, sheet_long = mx*sin + y*cos + t_long,
mx = -x if mirrored else x (publish.rs::raw_source_depth_mm convention). Sparrow's frame:
x_s = sheet_long (strip width axis), y_s = sheet_short (strip height 2000). Sparrow's
transformation: p' = R(theta) p + t on the input item's shape data. The axis swap is a
reflection, so an unmirrored placement of ours is a mirrored ring in Sparrow's frame; every
mixed-61 polygon passed the converter's reflection-equivalence-by-rotation audit, so a proper
rotation exists. It is found by brute force over cyclic shifts and both vertex orders and
verified to 1e-6 mm on every vertex.
"""
import json, math, sys
cell, sp_in, out = sys.argv[1:4]
doc = json.load(open(cell)); inst = json.load(open(sp_in))
placements = doc['outcome']['incumbent']['placements']
depth = doc['outcome']['incumbent']['rawSourceDepthMm']
item_by_dxf = {it['dxf']: it for it in inst['items']}
def ring(it):
    pts = it['shape']['data']
    if pts[0] == pts[-1]: pts = pts[:-1]
    return [(float(x), float(y)) for x, y in pts]
placed = []; worst = 0.0
for pl in placements:
    it = item_by_dxf[pl['pieceId']]; src = ring(it)
    th = math.radians(pl['rotationDeg']); s, c = math.sin(th), math.cos(th)
    tgt = []
    for x, y in src:
        mx = -x if pl['mirrored'] else x
        sheet_short = mx * c - y * s + pl['translateShortAxis']
        sheet_long = mx * s + y * c + pl['translateLongAxis']
        tgt.append((sheet_long, sheet_short))  # Sparrow (x_s, y_s)
    n = len(src); best = None
    for order in (src, src[::-1]):
        for shift in range(n):
            cand = order[shift:] + order[:shift]
            ax, ay = cand[1][0] - cand[0][0], cand[1][1] - cand[0][1]
            bx, by = tgt[1][0] - tgt[0][0], tgt[1][1] - tgt[0][1]
            if abs(math.hypot(ax, ay) - math.hypot(bx, by)) > 1e-6: continue
            phi = math.atan2(by, bx) - math.atan2(ay, ax)
            cs, sn = math.cos(phi), math.sin(phi)
            tx = tgt[0][0] - (cs * cand[0][0] - sn * cand[0][1])
            ty = tgt[0][1] - (sn * cand[0][0] + cs * cand[0][1])
            err = max(math.hypot(cs * px - sn * py + tx - qx, sn * px + cs * py + ty - qy) for (px, py), (qx, qy) in zip(cand, tgt))
            if err < 1e-6 and (best is None or err < best[0]): best = (err, math.degrees(phi), tx, ty)
    if best is None: raise SystemExit(f'no rigid fit for {pl["pieceId"]} (mirrored={pl["mirrored"]})')
    worst = max(worst, best[0])
    placed.append({'item_id': it['id'], 'transformation': {'rotation': best[1], 'translation': [best[2], best[3]]}})
sol = {'name': inst['name'], 'items': inst['items'], 'strip_height': inst['strip_height'],
       'solution': {'strip_width': depth, 'layout': {'container_id': 1125526108, 'placed_items': placed},
                    'density': None, 'run_time_sec': 0}}
json.dump(sol, open(out, 'w'), indent=1)
print(f'{len(placed)} items, strip_width {depth}, worst vertex error {worst:.2e} mm, mirrored {sum(1 for p in placements if p["mirrored"])}')
