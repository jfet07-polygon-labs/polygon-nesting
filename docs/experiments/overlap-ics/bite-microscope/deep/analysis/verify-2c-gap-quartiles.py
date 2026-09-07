"""verify-2c-gap-quartiles.py -- section 2: the min-raw worker's raw deficit / guided excess per class (analysts'
formula: sweeps where argmin raw != winner, pooled per class), and the winner = min raw rate per quartile.
Usage: cd /var/lib/t3/tmp/astra/deep/analysis && python3 verify-2c-gap-quartiles.py"""
import statistics as st
from collections import defaultdict
from verify_common import *
docs = load_docs(); A = attempts(docs)
def argmin(v):
    b = 0
    for i in range(1, len(v)):
        if v[i] < v[b]: b = i
    return b
G = defaultdict(lambda: dict(r=[], g=[])); Q = defaultdict(lambda: [dict(n=0, wmr=0, slr=0) for _ in range(4)]); ARM = defaultdict(lambda: dict(r=[], g=[]))
for a in A:
    cls = (a['arm'], a['bite'], a['pub']); n = a['s']['iterations']
    for sw in a['s']['sweeps']:
        W = sw['workers']; w = sw['winner']; gu = [x['guidedAfter'] for x in W]; ra = [x['rawAfter'] for x in W]
        ir = argmin(ra)
        if ir != w and gu[w] > 0:
            G[cls]['r'].append((ra[w] - ra[ir]) / ra[w] if ra[w] > 0 else 0); G[cls]['g'].append((gu[ir] - gu[w]) / gu[w])
            ARM[a['arm']]['r'].append((ra[w] - ra[ir]) / ra[w] if ra[w] > 0 else 0); ARM[a['arm']]['g'].append((gu[ir] - gu[w]) / gu[w])
        qi = min(3, (sw['iteration'] - 1) * 4 // n); Q[cls][qi]['n'] += 1
        if ir == w: Q[cls][qi]['wmr'] += 1
        if any(x['rawAfter'] < ra[w] for x in W if x['worker'] != w): Q[cls][qi]['slr'] += 1
for cls in sorted(G):
    print(cls, 'raw deficit median %.1f %%, guided excess median %.1f %% (n %d)' % (st.median(G[cls]['r'])*100, st.median(G[cls]['g'])*100, len(G[cls]['r'])), '| winner = min raw by quartile', ['%.1f' % (Q[cls][i]['wmr']/Q[cls][i]['n']*100) for i in range(4)])
for arm in 'AB': print(arm, 'pooled: raw deficit %.1f %%, guided excess %.1f %%' % (st.median(ARM[arm]['r'])*100, st.median(ARM[arm]['g'])*100))
