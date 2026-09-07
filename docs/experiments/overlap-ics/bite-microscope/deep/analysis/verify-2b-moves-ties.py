"""verify-2b-moves-ties.py -- section 2: 'winner = min max %' under the engine tie rule (argmin, ties to the lowest
worker ordinal) and the min-raw worker's raw gap / guided excess pooled over sweeps.
Usage: cd /var/lib/t3/tmp/astra/deep/analysis && python3 verify-2b-moves-ties.py"""
import statistics as st
from verify_common import *
docs = load_docs(); A = attempts(docs)
def argmin_first(ws, key):
    b = ws[0]
    for x in ws[1:]:
        if key(x) < key(b): b = x
    return b['worker']
groups = [('A', 5, True), ('A', 5, False), ('B', 5, True), ('B', 5, False), ('B', 6, False)]
gap = {'A': [], 'B': []}; gex = {'A': [], 'B': []}
for g in groups:
    sel = [a for a in A if (a['arm'], a['bite'], a['pub']) == g]
    wmm = []; wmr = []
    for a in sel:
        n = len(a['s']['sweeps']); m = 0; r = 0
        for w in a['s']['sweeps']:
            ws = sorted(w['workers'], key=lambda x: x['worker'])
            if argmin_first(ws, lambda x: x['maxAfterMm']) == w['winner']: m += 1
            if argmin_first(ws, lambda x: x['rawAfter']) == w['winner']: r += 1
            wk = next(x for x in ws if x['worker'] == w['winner']); mr = min(ws, key=lambda x: x['rawAfter'])
            if mr['rawAfter'] < wk['rawAfter']:
                gap[a['arm']].append((wk['rawAfter'] - mr['rawAfter']) / wk['rawAfter']); gex[a['arm']].append((mr['guidedAfter'] - wk['guidedAfter']) / wk['guidedAfter'])
        wmm.append(m / n * 100); wmr.append(r / n * 100)
    print(g, 'winner = min max %% (tie to lowest ordinal) median %.1f; winner = min raw %.1f' % (st.median(wmm), st.median(wmr)))
for arm in 'AB':
    print(arm, 'pooled over sweeps where winner != min raw: min-raw raw below winner %.1f %%; guided excess %.1f %% (n %d)' % (st.median(gap[arm])*100, st.median(gex[arm])*100, len(gap[arm])))
