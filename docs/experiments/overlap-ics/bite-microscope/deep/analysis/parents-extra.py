"""Supplements: piece sizes of the always-blocking pieces, budget-matched cross statistics,
edge-side distribution, live evaluations per iteration, cut-moved overlap across seeds,
microscope-vs-v2 outcome agreement, deadline definitions.
Run: cd /var/lib/t3/tmp/astra/deep/analysis && python3 parents-extra.py
"""
import json, itertools, collections, os
from parents_common import *
docs = load_docs(); at = attempts(docs)
b5 = [a for a in at if a['ordinal']==5]
entry = json.load(open('/var/lib/t3/tmp/astra/deep/analysis/parents-entry.json'))
rep = json.load(open('/var/lib/t3/tmp/astra/deep/analysis/parents-replays.json'))

print('== (a) piece sizes (fixture order assumed = engine piece index; realBounds w x h, area rank 1 = largest)')
fx = json.load(open('/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json'))
pcs = fx['pieces']; assert len(pcs) == 61
area = [(i, p['realBounds']['width']*p['realBounds']['height'], p['realBounds']['width'], p['realBounds']['height']) for i, p in enumerate(pcs)]
rank = {i: r+1 for r, (i, *_ ) in enumerate(sorted(area, key=lambda x: -x[1]))}
lab = {i: next((s['label'] for s in fx['sourcePieces'] if s['id']==p['id']), '?') for i, p in enumerate(pcs)}
for i in [14, 20, 21, 22, 23, 24, 38, 40, 4]:
    print(f"  piece {i}: {area[i][2]}x{area[i][3]} area {area[i][1]} rank {rank[i]} label {lab[i]}")
print('  area ranks of the 13 largest pieces:', [(i, rank[i]) for i in sorted(range(61), key=lambda i: rank[i])[:13]])
freq = collections.Counter()
for a in b5:
    s=set()
    for r,_ in a['sep']['entryBlocking']: s.update(row_pieces(r))
    freq.update(s)
cut = collections.Counter()
for a in b5: cut.update(a['bite']['cutMoved'])
print('  blocking frequency (of 42) and cut-moved frequency (of 42) by area rank, 15 largest:',
      [(i, rank[i], freq[i], cut[i]) for i in sorted(range(61), key=lambda i: rank[i])[:15]])
print('  ... 10 smallest:', [(i, rank[i], freq[i], cut[i]) for i in sorted(range(61), key=lambda i: rank[i])[-10:]])

print('\n== (b) budget-matched cross statistics (bite 5)')
# p=1 objective: B parents (live) band-entry evaluations; A parents' p=1 replays with their evaluation budgets
Bp1 = [(r['seed'], r['live']['evals'], r['live']['published']) for r in rep if r['bite']==5 and r['arm']=='B']
Ap1 = [(r['seed'], r['p1']['evTotal'], r['p1']['band'] is not None) for r in rep if r['bite']==5 and r['arm']=='A']
exp = 0.0
for s, E, band in sorted(Ap1, key=lambda x: x[1]):
    frac = sum(1 for _, e, pub in Bp1 if pub and e <= E) / len(Bp1)
    exp += frac
print(f"  p=1 from A parents: budgets (M evals) {sorted(round(E/1e6,1) for _,E,_ in Ap1)}; observed band entries {sum(b for *_, b in Ap1)}/21; "
      f"expected if A parents entered the band at the B parents' evaluation-to-band distribution: {exp:.2f}/21")
print(f"  B parents' p=1 evaluations to band (M): {sorted(round(e/1e6,1) for _,e,pub in Bp1 if pub)}")
print(f"  A parents' p=1 budgets >= 14.6M (median B success cost): {sum(1 for _,E,_ in Ap1 if E>=14.6e6)} replays, band entries among them {sum(1 for _,E,b in Ap1 if E>=14.6e6 and b)}")
# p=2 objective: A parents live (band at 23.4M evals on 1 seed); B parents' p=2 replays with budgets
Ap2 = [(r['seed'], r['live']['evals'], r['live']['published']) for r in rep if r['bite']==5 and r['arm']=='A']
Bp2 = [(r['seed'], r['p2']['evTotal'], r['p2']['band'] is not None, r['p2']['evToBand']) for r in rep if r['bite']==5 and r['arm']=='B']
print(f"  p=2 from B parents: budgets (M evals) {sorted(round(E/1e6,1) for _,E,_,_ in Bp2)}; band entries {sum(b for _,_,b,_ in Bp2)}/21 at evals-to-band (M) {sorted(round(e/1e6,1) for _,_,b,e in Bp2 if b)}")
print(f"  p=2 from A parents (live): budgets (M evals) {sorted(round(E/1e6,1) for _,E,_ in Ap2)}; band entries {sum(b for *_, b in Ap2)}/21 (seed 10636268072709740349 at 23.4M)")
print(f"  B-parent p=2 replays whose band entry cost <= their own live p=1 evaluations: {[(s, round(e/1e6,1)) for s,E,b,e in Bp2 if b and e <= dict((x[0],x[1]) for x in Bp1)[s]]}")
print(f"  B-parent p=2 band entries within 33.4M evals (the largest A live p=2 budget): {sum(1 for s,E,b,e in Bp2 if b and e<=33.4e6)}/21; A-parent live p=2 band entries: 1/21")

print('\n== (c) edge rows at entry by side (sum over attempts), bite 5')
for arm in 'AB':
    for pub in (True, False):
        sel = [e for e in entry if e['bite']==5 and e['arm']==arm and e['published']==pub]
        c = collections.Counter()
        for e in sel: c.update(e['edgeSides'])
        print(f"  arm {arm} {'published' if pub else 'failed'} (n={len(sel)}): {dict(sorted(c.items()))}; edge rows on moved pieces {sum(e['edgeMoved'] for e in sel)}, on still pieces {sum(e['edgeStill'] for e in sel)}")

print('\n== (d) live evaluations per master iteration (bite 5)')
for arm in 'AB':
    v = [e['evals']/e['iterations'] for e in entry if e['bite']==5 and e['arm']==arm]
    print(f"  arm {arm}: q1/med/q3 {quart(v)[0]:.0f}/{quart(v)[1]:.0f}/{quart(v)[2]:.0f} evaluations per iteration")
ra = med([e['evals']/e['iterations'] for e in entry if e['bite']==5 and e['arm']=='A']); rb = med([e['evals']/e['iterations'] for e in entry if e['bite']==5 and e['arm']=='B'])
print(f"  ratio of medians A/B: {ra/rb:.2f}")

print('\n== (e) cut-moved overlap across seeds within an arm (Jaccard), bite 5')
for arm in 'AB':
    sel = [a for a in b5 if a['arm']==arm]; js=[]
    for x, y in itertools.combinations(sel, 2):
        cx, cy = set(x['bite']['cutMoved']), set(y['bite']['cutMoved']); js.append(len(cx&cy)/len(cx|cy))
    print(f"  arm {arm}: q1/med/q3 {quart(js)[0]:.2f}/{quart(js)[1]:.2f}/{quart(js)[2]:.2f}")
print('  split y / parent depth (bite 5):', sorted(set(round(a['bite']['splitYMm']/a['bite']['parentDepthMm'],3) for a in b5)))

print('\n== (f) microscope outcome vs v2 scored outcome on the twelve v2 seeds; and the nine v1 seeds')
v2 = json.load(open('/var/lib/t3/tmp/astra/v2/fifth-cut-rows.json'))
v2o = {}
for r in v2: v2o.setdefault((r['arm'], r['seed']), []).append(r['fifth'])
agree = 0; n = 0
for e in entry:
    if e['bite']!=5: continue
    k = (e['arm'], e['seed'])
    if k in v2o:
        n += 1; agree += (all(v2o[k]) == e['published'])
    else:
        print(f"  v1-era seed {e['seed']} arm {e['arm']}: microscope bite 5 {'PUB' if e['published'] else 'fail'} (not a v2 seed)")
print(f"  v2 seeds: microscope outcome agrees with the scored cells' (unanimous) outcome in {agree}/{n} (arm, seed) pairs")
# iteration counts microscope vs v2 (same seed, arm)
v2it = {}
for r in v2: v2it.setdefault((r['arm'], r['seed']), []).append(r['iters'])
d = [(e['arm'], e['seed'], e['iterations'], v2it[(e['arm'], e['seed'])]) for e in entry if e['bite']==5 and (e['arm'], e['seed']) in v2it]
print('  microscope iterations vs the three scored reps:', [(a, str(s)[:6], it, reps) for a, s, it, reps in d])

print('\n== (g) deadline definitions')
e = [x for x in entry if x['bite']==5 and x['arm']=='A' and x['seed']==10636268072709740349][0]
print(f"  microscope wallAtEntry: elapsed {e['entryElapsedS']:.3f}, left {e['leftS']:.3f}, phaseDeadline {e['phaseDeadlineS']:.3f} (= elapsed + left)")
r0 = [r for r in v2 if r['arm']=='A' and r['seed']==10636268072709740349 and r['rep']==0][0]
print(f"  v2 fifth-cut-rows: ptime {r0['ptime']:.3f}, left {r0['left']:.3f} (= loopEntrySeconds + loopExploreSeconds - ptime, i.e. measured explore end {r0['ptime']+r0['left']:.3f} s)")

print('\n== (h) within-arm ranking of the 21 microscope parents by publication time and by allowance, with outcome')
for arm in 'AB':
    sel = sorted([e for e in entry if e['bite']==5 and e['arm']==arm], key=lambda e: e['parentPubS'])
    print(f"  arm {arm} by parent pub s: " + ' '.join(f"{e['parentPubS']:.2f}{'*' if e['published'] else ''}" for e in sel) + '   (* = published)')
    sel = sorted([e for e in entry if e['bite']==5 and e['arm']==arm], key=lambda e: -e['leftS'])
    print(f"  arm {arm} by left s (desc): " + ' '.join(f"{e['leftS']:.2f}{'*' if e['published'] else ''}" for e in sel))
    sel = sorted([e for e in entry if e['bite']==5 and e['arm']==arm], key=lambda e: e['entryRows'])
    print(f"  arm {arm} by entry rows: " + ' '.join(f"{e['entryRows']}{'*' if e['published'] else ''}" for e in sel))
    sel = sorted([e for e in entry if e['bite']==5 and e['arm']==arm], key=lambda e: e['largest'])
    print(f"  arm {arm} by largest component (pieces): " + ' '.join(f"{e['largest']}{'*' if e['published'] else ''}" for e in sel))
    sel = sorted([e for e in entry if e['bite']==5 and e['arm']==arm], key=lambda e: e['components'])
    print(f"  arm {arm} by components: " + ' '.join(f"{e['components']}{'*' if e['published'] else ''}" for e in sel))
    sel = sorted([e for e in entry if e['bite']==5 and e['arm']==arm], key=lambda e: e['rowsGE0.9shrink'])
    print(f"  arm {arm} by rows >= 0.9 shrink: " + ' '.join(f"{e['rowsGE0.9shrink']}{'*' if e['published'] else ''}" for e in sel))
