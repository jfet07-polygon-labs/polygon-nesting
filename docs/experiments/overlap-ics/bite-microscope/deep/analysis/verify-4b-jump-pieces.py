"""verify-4b-jump-pieces.py -- the jump sweep's winner relocation magnitude per arm (section 4 text) and the
fixture piece sizes behind section 3's 'eight pieces among the twelve largest' sentence.
Usage: cd /var/lib/t3/tmp/astra/deep/analysis && python3 verify-4b-jump-pieces.py"""
import json, math, statistics as st
from collections import Counter
from verify_common import *
docs = load_docs(); A = attempts(docs)
for arm in 'AB':
    mags = []; before = []
    for a in A:
        if a['arm'] != arm or a['bite'] != 5: continue
        s = a['s']; raws = [x[1] for x in s['samples']]; runmin = raws[0]; jump = None
        for i in range(1, s['iterations'] + 1):
            if raws[i] > 2 * runmin: jump = i; break
            runmin = min(runmin, raws[i])
        w = s['sweeps'][jump - 1]
        mags.append(max([math.hypot(x['dxMm'], x['dyMm']) for x in w['relocates'] if x['moved']] or [0]))
        before.extend(max([math.hypot(x['dxMm'], x['dyMm']) for x in s['sweeps'][i-1]['relocates'] if x['moved']] or [0]) for i in range(1, jump))
    print(arm, 'bite 5 jump-sweep max relocation: median %.0f mm, sorted %s; sweeps before the jump: median %.1f mm, > 100 mm on %d of %d' % (st.median(mags), [round(m) for m in sorted(mags)], st.median(before), sum(1 for b in before if b > 100), len(before)))
fx = json.load(open('/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json'))
def poly_area(pc):
    segs = pc['geometry']['segments']; pts = [(sg['x1'], sg['y1']) for sg in segs]
    kinds = Counter(sg['kind'] for sg in segs)
    a = 0
    for i in range(len(pts)):
        x1, y1 = pts[i]; x2, y2 = pts[(i + 1) % len(pts)]; a += x1 * y2 - x2 * y1
    return abs(a) / 2, dict(kinds)
areas = []
for i, (pc, sp) in enumerate(zip(fx['pieces'], fx['sourcePieces'])):
    assert pc['id'] == sp['id']
    pa, kinds = poly_area(sp)
    areas.append((i, sp['label'], pc['paddedBounds']['area'], pc['realBounds']['width'] * pc['realBounds']['height'], round(pa), kinds))
for key, name in ((2, 'padded bbox'), (3, 'real bbox'), (4, 'polygon (line segments)')):
    top = sorted(areas, key=lambda x: -x[key])[:12]
    print('top-12 by', name, [(t[0], t[1], t[key]) for t in top])
print('piece 48:', areas[48], 'rank by padded', sorted(range(61), key=lambda i: -areas[i][2]).index(48) + 1, 'rank by polygon', sorted(range(61), key=lambda i: -areas[i][4]).index(48) + 1)
print('segment kinds', Counter(k for a in areas for k in a[5]))
