"""Same seed, A vs B: how different are the two arms' parents near 160.6 mm, and where do the arms diverge?
Run: cd /var/lib/t3/tmp/astra/deep/analysis && python3 parents-crossarm.py
"""
import json, math
from parents_common import *
docs = load_docs()
seeds = sorted({s for _, s in docs})
def pubs(doc): return [p for p in doc['outcome']['publications'] if p['phase']=='explore']
def pose_diff(pa, pb):
    """pa, pb: poses lists. -> (pieces with identical pose, max translation mm, max rotation deg)"""
    same = 0; mt = 0.0; mr = 0.0
    for x, y in zip(pa, pb):
        dx = x['txMm']-y['txMm'] if 'txMm' in x else x[0]-y[0]
        dy = x['tyMm']-y['tyMm'] if 'tyMm' in x else x[1]-y[1]
        dth = x['thetaDeg']-y['thetaDeg'] if 'thetaDeg' in x else x[2]-y[2]
        if dx == 0 and dy == 0 and dth == 0: same += 1
        mt = max(mt, math.hypot(dx, dy)); mr = max(mr, abs(dth))
    return same, mt, mr
print('| seed | first explore bite where A and B publications differ (by placementFingerprint) | bite-4 parent: identical pieces / max translation mm / max rotation deg | A b4 pub s | B b4 pub s | A5 | B5 |')
print('|---|---|---|---:|---:|---|---|')
for s in seeds:
    A, B = docs[('A', s)], docs[('B', s)]
    pa, pb = pubs(A), pubs(B)
    first_diff = None
    for x, y in zip(pa, pb):
        if x['placementFingerprint'] != y['placementFingerprint']:
            first_diff = x['ordinal']['bite']; break
    a4 = [p for p in pa if p['ordinal']['bite']==4][0]; b4 = [p for p in pb if p['ordinal']['bite']==4][0]
    same, mt, mr = pose_diff(a4['poses'], b4['poses'])
    m5a = [b for b in A['biteMicroscope']['bites'] if b['ordinal']==5][0]; m5b = [b for b in B['biteMicroscope']['bites'] if b['ordinal']==5][0]
    print(f"| {s} | {first_diff} | {same} / {mt:.2f} / {mr:.2f} | {a4['wallSeconds']:.2f} | {b4['wallSeconds']:.2f} | {'PUB' if m5a['published'] else 'fail'} | {'PUB' if m5b['published'] else 'fail'} |")
print()
print('pose sample:', docs[('A', seeds[0])]['outcome']['publications'][0]['poses'][0])
# constructor identical? compare bite-1 parent fingerprints
print('bite-1 parentFingerprint equal across arms on all seeds:', all(pubs(docs[('A',s)])[0]['parentFingerprint']==pubs(docs[('B',s)])[0]['parentFingerprint'] for s in seeds))
print('bite-1 placementFingerprint equal across arms per seed:', sum(1 for s in seeds if pubs(docs[('A',s)])[0]['placementFingerprint']==pubs(docs[('B',s)])[0]['placementFingerprint']), '/', len(seeds))
