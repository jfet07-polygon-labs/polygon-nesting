"""verify-0-preamble.py -- recompute the preamble numbers of README-draft.md.
Usage: cd /var/lib/t3/tmp/astra/deep/analysis && python3 verify-0-preamble.py"""
import json, glob, os, statistics as st
from verify_common import *
docs = load_docs(); A = attempts(docs); R = load_replays()
print('documents', len(docs), 'replays', len(R), 'attempts', len(A))
from collections import Counter
c = Counter((a['arm'], a['bite'], a['pub']) for a in A)
print('groups', dict(c))
print('bite-5 published A:', [a['seed'] for a in A if a['arm']=='A' and a['bite']==5 and a['pub']])
print('bite-5 published B:', sorted(a['seed'] for a in A if a['arm']=='B' and a['bite']==5 and a['pub']))
print('sixth cuts fail:', all(not a['pub'] for a in A if a['bite']==6), 'n', sum(1 for a in A if a['bite']==6))
# every attempt single: separations len 1 asserted in loader; attempt index
print('attempt indices', Counter(a['s']['attempt'] for a in A))
# restoredToIteration == last newMinimum on failed
fails = [a for a in A if not a['pub']]
eq = sum(1 for a in fails if a['s']['restoredToIteration'] == max(x[0] for x in a['s']['samples'] if x[4]))
print('restoredTo == last newMinimum:', eq, 'of', len(fails))
# argmin raw?
eq2 = sum(1 for a in fails if a['s']['restoredToIteration'] == min(range(len(a['s']['samples'])), key=lambda i: a['s']['samples'][i][1]))
print('restoredTo == argmin raw over samples:', eq2, 'of', len(fails))
# v2 agreement on twelve seeds
v2 = json.load(open(os.path.join(V2, 'fifth-cut-rows.json')))
seeds12 = sorted({a['seed'] for a in A if len(a['seed']) > 3})
print('64-bit seeds', len(seeds12))
agree = 0; pairs = 0; ratios = []
for arm in 'AB':
    for sd in seeds12:
        mic = [a for a in A if a['arm']==arm and a['seed']==sd and a['bite']==5][0]
        cells = [r for r in v2 if r['arm']==arm and str(r['seed'])==sd]
        assert len(cells) == 3
        pairs += 1
        if all(r['fifth'] == mic['pub'] for r in cells): agree += 1
        for r in cells: ratios.append(abs(r['iters'] - mic['s']['iterations']) / mic['s']['iterations'])
print('outcome agreement', agree, 'of', pairs, '; iteration count |diff|/mic median %.4f max %.4f' % (st.median(ratios), max(ratios)))
# evaluation cost per iteration, bite 5
ev = {}
for arm in 'AB':
    g = [a for a in A if a['arm']==arm and a['bite']==5]
    v = [a['s']['evaluationsAllWorkers'] / a['s']['iterations'] for a in g]
    ev[arm] = st.median(v)
    print(arm, 'evals/iteration bite 5 median %.0f' % ev[arm], 'n', len(g))
print('ratio A/B %.3f' % (ev['A']/ev['B']))
# same-capsule replays: p2 evals/iter divided by p1 evals/iter for the same capsule
rat = []
for (arm, seed, bite, p), r in R.items():
    if p != 2: continue
    r1 = R[(arm, seed, bite, 1)]
    e2 = r['evaluationsTotal'] / len(r['iterations']); e1 = r1['evaluationsTotal'] / len(r1['iterations'])
    rat.append((e2 / e1, arm, seed, bite))
rat.sort()
print('same-capsule replay cost ratio p2/p1: n %d median %.3f min %.3f max %.3f' % (len(rat), st.median([x[0] for x in rat]), rat[0][0], rat[-1][0]))
for arm in 'AB':
    v = [x[0] for x in rat if x[1]==arm]; print('  capsules from', arm, 'median %.3f [%.2f-%.2f]' % (st.median(v), min(v), max(v)))
for bite in (5, 6):
    v = [x[0] for x in rat if x[3]==bite]; print('  bite', bite, 'median %.3f [%.2f-%.2f]' % (st.median(v), min(v), max(v)))
# 180 scored cells evals per iteration of the fifth cut
for arm in 'AB':
    v = [r['ev'] / r['iters'] for r in v2 if r['arm']==arm]
    print('v2', arm, 'evals/iter fifth cut median %.0f n %d' % (st.median(v), len(v)))
# evaluations per second (bite 5): evaluations / (wall elapsed at stop - elapsed at entry)
for arm in 'AB':
    g = [a for a in A if a['arm']==arm and a['bite']==5]
    rate = [a['s']['evaluationsAllWorkers'] / (a['s']['wallAtStop']['elapsedS'] - a['s']['wallAtEntry']['elapsedS']) for a in g]
    its = [a['s']['iterations'] / (a['s']['wallAtStop']['elapsedS'] - a['s']['wallAtEntry']['elapsedS']) for a in g]
    print(arm, 'evals/s median %.3f M' % (st.median(rate)/1e6), 'iterations/s median %.1f' % st.median(its))
