"""verify-3c-misc.py -- corrected per-attempt piece frequency (section 3), the v2 iteration-agreement statistic
(preamble), bite-6 stops and strikes (section 4), and the control attempt with entry rows continuous to sweep 50.
Usage: cd /var/lib/t3/tmp/astra/deep/analysis && python3 verify-3c-misc.py"""
import json, os, statistics as st
from collections import Counter
from verify_common import *
docs = load_docs(); A = attempts(docs)
pf = Counter(); mv = Counter()
for a in A:
    if a['bite'] != 5: continue
    pcs = set().union(*[pieces_of(r) for r, _ in a['s']['entryBlocking']])
    pf.update(pcs); mv.update(a['b']['cutMoved'])
always = sorted(p for p, c in pf.items() if c == 42)
print('pieces in all 42 entries:', always, 'moved by the cut in', {p: mv[p] for p in always}, 'max', max(mv[p] for p in always))
fx = json.load(open('/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json'))
def poly_area(sp):
    pts = [(sg['x1'], sg['y1']) for sg in sp['geometry']['segments']]
    return abs(sum(pts[i][0] * pts[(i+1) % len(pts)][1] - pts[(i+1) % len(pts)][0] * pts[i][1] for i in range(len(pts)))) / 2
bb = sorted(range(61), key=lambda i: -fx['pieces'][i]['paddedBounds']['area']); pa = sorted(range(61), key=lambda i: -poly_area(fx['sourcePieces'][i]))
print('ranks of the always-present pieces: by padded bbox', {p: bb.index(p) + 1 for p in always}, '; by polygon area', {p: pa.index(p) + 1 for p in always})
v2 = json.load(open(os.path.join(V2, 'fifth-cut-rows.json')))
seeds12 = sorted({a['seed'] for a in A if len(a['seed']) > 3})
cell = []; pairmed = []; signed = []
for arm in 'AB':
    for sd in seeds12:
        mic = [a for a in A if a['arm']==arm and a['seed']==sd and a['bite']==5][0]['s']['iterations']
        its = [r['iters'] for r in v2 if r['arm']==arm and str(r['seed'])==sd]
        cell += [abs(i - mic) / mic for i in its]; pairmed.append(abs(st.median(its) - mic) / mic); signed += [(i - mic) / mic for i in its]
        cell_rel_v2 = [abs(i - mic) / i for i in its]
print('v2 vs microscope iterations: per-cell |diff|/mic median %.4f, max %.4f; per pair median-of-reps median %.4f max %.4f; per-cell |diff|/v2 median %.4f; signed median %.4f; mean |diff| %.4f' % (st.median(cell), max(cell), st.median(pairmed), max(pairmed), st.median([abs(i) for i in cell]), st.median(signed), st.mean(cell)))
print('bite 6:', [(a['arm'], a['seed'], a['s']['stop'], 'strikes', a['s']['strikes'], 'band', a['s']['bandEntries'], 'exact', a['s']['exactCheckpointCalls'], 'left %.3f' % a['s']['wallAtStop']['leftS']) for a in A if a['bite']==6])
print('all failed attempts max strikes', max(a['s']['strikes'] for a in A if not a['pub']), 'min wall left at stop %.3f' % min(a['s']['wallAtStop']['leftS'] for a in A if not a['pub']))
for a in A:
    if a['arm'] != 'A' or a['s']['iterations'] < 50: continue
    eids = {r for r, _ in a['s']['entryBlocking']}; cont = set(eids)
    for w in a['s']['sweeps'][:50]: cont &= {r for r, _ in w['blocking']}
    if cont: print('A attempt with entry rows continuous to sweep 50:', a['seed'], 'bite', a['bite'], [decode(r) for r in cont], 'still at sweep', max(k for k in range(1, a['s']['iterations'] + 1) if all(r in {x for x, _ in w['blocking']} for w in a['s']['sweeps'][:k] for r in cont)))
