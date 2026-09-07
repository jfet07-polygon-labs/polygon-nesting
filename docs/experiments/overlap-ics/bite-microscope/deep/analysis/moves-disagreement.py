#!/usr/bin/env python3
"""Magnitude of the raw-vs-guided disagreement per sweep: in sweeps where the
winner (min guided) is not the min-raw worker, how far the min-raw worker's
guided sits above the winner's (relative), and how far its raw sits below the
winner's raw; same for the min-max worker.  Reads the deep documents read-only."""
import json, glob, os, re, statistics as st, collections
DEEP = '/var/lib/t3/tmp/astra/deep'
def argmin(v):
    b = 0
    for i in range(1, len(v)):
        if v[i] < v[b]: b = i
    return b
G = collections.defaultdict(lambda: dict(n=0, disRaw=0, disMax=0, gRaw=[], rRaw=[], gMax=[], mMax=[], guidedEqRaw=0))
for path in sorted(glob.glob(os.path.join(DEEP, 'deep-*-wall10s-s*.json'))):
    m = re.match(r'deep-([AB])-wall10s-s(\d+)\.json', os.path.basename(path)); arm = m.group(1)
    doc = json.load(open(path))
    for bite in doc['biteMicroscope']['bites']:
        for sep in bite['separations']:
            key = (arm, 'b6' if bite['ordinal'] == 6 else ('b5-pub' if sep['stop'] == 'published' else 'b5-fail'))
            g = G[key]
            for sw in sep['sweeps']:
                W = sw['workers']; w = sw['winner']
                gu = [x['guidedAfter'] for x in W]; ra = [x['rawAfter'] for x in W]; mx = [x['maxAfterMm'] for x in W]
                g['n'] += 1
                if all(x['guidedAfter'] == x['rawAfter'] for x in W): g['guidedEqRaw'] += 1
                ir = argmin(ra)
                if ir != w and gu[w] > 0:
                    g['disRaw'] += 1; g['gRaw'].append((gu[ir] - gu[w]) / gu[w]); g['rRaw'].append((ra[w] - ra[ir]) / ra[w] if ra[w] > 0 else 0)
                im = argmin(mx)
                if im != w and gu[w] > 0:
                    g['disMax'] += 1; g['gMax'].append((gu[im] - gu[w]) / gu[w]); g['mMax'].append((mx[w] - mx[im]) / mx[w] if mx[w] > 0 else 0)
print('| arm | class | sweeps | sweeps with guided == raw for all workers (unit weights) | winner != min-raw worker | median rel guided excess of min-raw worker | median rel raw deficit of winner vs min-raw worker | winner != min-max worker | median rel guided excess of min-max worker | median rel max deficit of winner vs min-max worker |')
print('|---|---|---:|---:|---:|---:|---:|---:|---:|---:|')
for k in sorted(G):
    g = G[k]
    print(f"| {k[0]} | {k[1]} | {g['n']} | {g['guidedEqRaw']} | {g['disRaw']} | {100*st.median(g['gRaw']):.1f} % | {100*st.median(g['rRaw']):.1f} % | {g['disMax']} | {100*st.median(g['gMax']):.1f} % | {100*st.median(g['mMax']):.1f} % |")
