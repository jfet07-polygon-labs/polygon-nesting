"""Which pieces recur in entry blocking rows across seeds and arms.
Run: cd /var/lib/t3/tmp/astra/deep/analysis && python3 parents-pieces.py
"""
import collections, json
from parents_common import *

docs = load_docs()
at = [a for a in attempts(docs)]
def piece_freq(sel):
    c = collections.Counter()
    for a in sel:
        pcs = set()
        for r, _ in a['sep']['entryBlocking']: pcs.update(row_pieces(r))
        c.update(pcs)
    return c
def row_freq(sel):
    c = collections.Counter()
    for a in sel:
        c.update(r for r, _ in a['sep']['entryBlocking'])
    return c
def cut_freq(sel):
    c = collections.Counter()
    for a in sel: c.update(a['bite']['cutMoved'])
    return c

groups = {
 'bite5 all (42)': [a for a in at if a['ordinal']==5],
 'bite5 A (21)': [a for a in at if a['ordinal']==5 and a['arm']=='A'],
 'bite5 B (21)': [a for a in at if a['ordinal']==5 and a['arm']=='B'],
 'bite5 published (8)': [a for a in at if a['ordinal']==5 and a['bite']['published']],
 'bite5 failed (34)': [a for a in at if a['ordinal']==5 and not a['bite']['published']],
 'bite6 all (8)': [a for a in at if a['ordinal']==6],
}
print('== piece frequency in entry blocking rows (piece: attempts in which it has a positive row)')
for g, sel in groups.items():
    c = piece_freq(sel); n = len(sel)
    top = c.most_common(15)
    print(f"{g}: pieces ever blocking {len(c)}/61; in all attempts {sum(1 for p,k in c.items() if k==n)}; "
          f"never blocking {sorted(set(range(61))-set(c))}; top {[(p,k) for p,k in top]}")
print()
print('== pieces in all 42 bite-5 entries:', sorted(p for p,k in piece_freq(groups['bite5 all (42)']).items() if k==42))
print('== pieces blocking in <= 5 of 42 bite-5 entries:', sorted((p,k) for p,k in piece_freq(groups['bite5 all (42)']).items() if k<=5))
c5 = piece_freq(groups['bite5 all (42)'])
print('== distribution of per-piece frequency over 42 bite-5 entries:', sorted(collections.Counter(c5.values()).items()))
print()
print('== cut-moved recurrence (bite 5): pieces moved by the cut in k of 42 attempts')
cm = cut_freq(groups['bite5 all (42)'])
print('moved in all 42:', sorted(p for p,k in cm.items() if k==42))
print('never moved:', sorted(set(range(61))-set(cm)))
print('sometimes moved:', sorted((p,k) for p,k in cm.items() if 0<k<42))
print()
print('== published vs failed (bite 5): pieces whose blocking frequency differs most')
cp = piece_freq(groups['bite5 published (8)']); cf = piece_freq(groups['bite5 failed (34)'])
diff = sorted(((cp[p]/8 - cf[p]/34), p, cp[p], cf[p]) for p in range(61))
print('more often in failures (rate pub - rate fail, piece, n pub/8, n fail/34):', [(round(d,2),p,a,b) for d,p,a,b in diff[:8]])
print('more often in successes:', [(round(d,2),p,a,b) for d,p,a,b in diff[-8:]])
print()
print('== row (pair/edge) recurrence in bite-5 entries')
rf = row_freq(groups['bite5 all (42)'])
print('distinct rows', len(rf), 'rows in >= 30 of 42:', [(r, decode(r), k) for r,k in rf.most_common() if k>=30])
print('rows in all 42:', [(r, decode(r)) for r,k in rf.items() if k==42])
print('distribution of row frequency:', sorted(collections.Counter(rf.values()).items()))
# same-seed row overlap between arms
print()
print('== same-seed A vs B overlap of entry rows / pieces (bite 5): Jaccard')
by = {(a['arm'], a['seed']): a for a in groups['bite5 all (42)']}
for seed in sorted({s for _, s in by}):
    ra = {r for r,_ in by[('A',seed)]['sep']['entryBlocking']}; rb = {r for r,_ in by[('B',seed)]['sep']['entryBlocking']}
    pa = set(); pb = set()
    for r in ra: pa.update(row_pieces(r))
    for r in rb: pb.update(row_pieces(r))
    ca = set(by[('A',seed)]['bite']['cutMoved']); cb = set(by[('B',seed)]['bite']['cutMoved'])
    print(f"seed {seed}: rows A {len(ra)} B {len(rb)} common {len(ra&rb)} J={len(ra&rb)/len(ra|rb):.2f}; pieces common {len(pa&pb)} J={len(pa&pb)/len(pa|pb):.2f}; cutMoved J={len(ca&cb)/len(ca|cb):.2f}; "
          f"A {'PUB' if by[('A',seed)]['bite']['published'] else 'fail'} B {'PUB' if by[('B',seed)]['bite']['published'] else 'fail'}")
# cross-seed row overlap within an arm
import itertools
for arm in 'AB':
    js = []
    sel = [a for a in groups['bite5 all (42)'] if a['arm']==arm]
    for x, y in itertools.combinations(sel, 2):
        rx = {r for r,_ in x['sep']['entryBlocking']}; ry = {r for r,_ in y['sep']['entryBlocking']}
        js.append(len(rx&ry)/len(rx|ry))
    print(f"arm {arm}: cross-seed row Jaccard q1/med/q3 {quart(js)[0]:.2f}/{quart(js)[1]:.2f}/{quart(js)[2]:.2f} over {len(js)} pairs")
js=[]
for seed in sorted({s for _, s in by}):
    ra = {r for r,_ in by[('A',seed)]['sep']['entryBlocking']}; rb = {r for r,_ in by[('B',seed)]['sep']['entryBlocking']}
    js.append(len(ra&rb)/len(ra|rb))
print(f"same-seed A-B row Jaccard q1/med/q3 {quart(js)[0]:.2f}/{quart(js)[1]:.2f}/{quart(js)[2]:.2f} over {len(js)} seeds")
