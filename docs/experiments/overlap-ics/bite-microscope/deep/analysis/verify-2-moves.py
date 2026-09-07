"""verify-2-moves.py -- recompute section 2 (where useful moves disappear) table and text numbers of README-draft.md.
Usage: cd /var/lib/t3/tmp/astra/deep/analysis && python3 verify-2-moves.py"""
import json, statistics as st
from collections import Counter
from verify_common import *
docs = load_docs(); A = attempts(docs)
BAND = 0.004
def argmin_first(vals):
    b = 0
    for i in range(1, len(vals)):
        if vals[i] < vals[b]: b = i
    return b
rows = []; tot_sweeps = 0; win_guided = 0
per_seed_cc = {}
for a in A:
    s = a['s']; sw = s['sweeps']
    n = len(sw); contested = 0; wmr = 0; wmm = 0; slr = 0; slm = 0
    useful = moved = 0; cc_all = cc_win = 0; ev_all = ev_win = disc = 0
    best_w = None; best_l = None; loser50_not_winner = 0; loser4 = 0; anyw50 = 0; anyw20 = 0; anyw4 = 0
    minraw_gap = []; minraw_gex = []
    quart = [dict(n=0, cc_all=0, cc_win=0, wmr=0) for _ in range(4)]
    for w in sw:
        tot_sweeps += 1
        ws = w['workers']; win = w['winner']
        wk = next(x for x in ws if x['worker'] == win)
        g = [x['guidedAfter'] for x in ws]
        if wk['guidedAfter'] <= min(g): win_guided += 1
        if w['contested']: contested += 1
        raws = [x['rawAfter'] for x in ws]; mxs = [x['maxAfterMm'] for x in ws]
        if wk['rawAfter'] <= min(raws): wmr += 1
        else:
            mr = min(ws, key=lambda x: x['rawAfter'])
            minraw_gap.append((wk['rawAfter'] - mr['rawAfter']) / wk['rawAfter'])
            minraw_gex.append((mr['guidedAfter'] - wk['guidedAfter']) / wk['guidedAfter'])
        if wk['maxAfterMm'] <= min(mxs): wmm += 1
        losers = [x for x in ws if x['worker'] != win]
        if any(x['rawAfter'] < wk['rawAfter'] for x in losers): slr += 1
        if any(x['maxAfterMm'] < wk['maxAfterMm'] for x in losers): slm += 1
        useful += sum(x['usefulMoves'] for x in ws); moved += sum(x['moved'] for x in ws)
        cc_all += sum(x['containerCommits'] for x in ws); cc_win += wk['containerCommits']
        ev_all += sum(x['sampleEvaluations'] for x in ws); ev_win += wk['sampleEvaluations']; disc += w['discardedExpenditure']
        assert ev_all - ev_win == disc or True
        best_w = wk['maxAfterMm'] if best_w is None else min(best_w, wk['maxAfterMm'])
        lm = min(x['maxAfterMm'] for x in losers)
        best_l = lm if best_l is None else min(best_l, lm)
        if lm < 0.050 and wk['maxAfterMm'] >= 0.050: loser50_not_winner += 1
        if lm < BAND and wk['maxAfterMm'] >= BAND: loser4 += 1
        if min(mxs) < 0.050: anyw50 += 1
        if min(mxs) < 0.020: anyw20 += 1
        if min(mxs) < BAND: anyw4 += 1
        qi = min(3, (w['iteration'] - 1) * 4 // n)
        quart[qi]['n'] += 1; quart[qi]['cc_all'] += sum(x['containerCommits'] for x in ws); quart[qi]['cc_win'] += wk['containerCommits']
    rows.append(dict(arm=a['arm'], seed=a['seed'], bite=a['bite'], pub=a['pub'], n=n, contested=contested/n*100, wmr=wmr/n*100, wmm=wmm/n*100,
        slr=slr/n*100, slm=slm/n*100, useful=useful, moved=moved, cc_all=cc_all, cc_win=cc_win, ev_all=ev_all, ev_win=ev_win, disc=disc,
        best_w=best_w, best_l=best_l, l50=loser50_not_winner, l4=loser4, anyw50=anyw50, anyw20=anyw20, anyw4=anyw4,
        gap=st.median(minraw_gap), gex=st.median(minraw_gex), quart=quart, evA=s['evaluationsAllWorkers'], discS=s['discardedExpenditure'], umd=s['usefulMovesDiscarded']))
print('total sweeps', tot_sweeps, 'winner = argmin guided', win_guided)
print('## table 2')
groups = [('A', 5, True), ('A', 5, False), ('B', 5, True), ('B', 5, False), ('B', 6, False), ('A', 6, False)]
for g in groups:
    sel = [r for r in rows if (r['arm'], r['bite'], r['pub']) == g]
    if not sel: continue
    U = sum(r['useful'] for r in sel); M = sum(r['moved'] for r in sel); CA = sum(r['cc_all'] for r in sel); CW = sum(r['cc_win'] for r in sel); NS = sum(r['n'] for r in sel)
    D = sum(r['disc'] for r in sel); E = sum(r['ev_all'] for r in sel)
    print(f"{g} n={len(sel)} | sweeps {med([r['n'] for r in sel])} | contested {med([r['contested'] for r in sel]):.1f} | w=min raw {med([r['wmr'] for r in sel]):.1f} | w=min max {med([r['wmm'] for r in sel]):.1f} | loser lower raw {med([r['slr'] for r in sel]):.1f} | loser lower max {med([r['slm'] for r in sel]):.1f} | useful/moved pooled {U/M*100:.1f} (med {med([r['useful']/r['moved']*100 for r in sel]):.1f}) | discarded exp pooled {D/E*100:.2f} (med {med([r['disc']/r['ev_all']*100 for r in sel]):.2f}) | cc/sweep {CA/NS:.2f} / {CW/NS:.2f} ({CW/CA*100:.1f}) | best w max / best l max {med([r['best_w'] for r in sel]):.4f} / {med([r['best_l'] for r in sel]):.4f} (ratio of medians {med([r['best_l'] for r in sel])/med([r['best_w'] for r in sel]):.3f}; med of ratios {med([r['best_l']/r['best_w'] for r in sel if r['best_w']>0]):.3f}) | loser<50um & winner not {sum(r['l50'] for r in sel)} | loser<4um & winner not {sum(r['l4'] for r in sel)}")
print('## text numbers')
print('retained share of evaluations per attempt: min %.2f max %.2f %%' % (min(r['ev_win']/r['ev_all']*100 for r in rows), max(r['ev_win']/r['ev_all']*100 for r in rows)))
print('discarded expenditure per attempt: min %.2f max %.2f %%' % (min(r['disc']/r['ev_all']*100 for r in rows), max(r['disc']/r['ev_all']*100 for r in rows)))
print('useful/moved per attempt: min %.2f max %.2f %%' % (min(r['useful']/r['moved']*100 for r in rows), max(r['useful']/r['moved']*100 for r in rows)))
print('check discardedExpenditure == sum losers evaluations:', all(r['disc'] == r['ev_all'] - r['ev_win'] for r in rows), '; sep.discardedExpenditure == sum sweeps:', all(r['disc'] == r['discS'] for r in rows), '; evaluationsAllWorkers == sum:', all(r['ev_all'] == r['evA'] for r in rows))
for arm in 'AB':
    sel = [r for r in rows if r['arm']==arm]
    print(arm, 'when winner != min raw: min-raw raw below winner %% median-of-attempt-medians %.1f; guided excess %.1f' % (med([r['gap'] for r in sel])*100, med([r['gex'] for r in sel])*100))
    fails = [r for r in sel if not r['pub']]
    print(arm, 'failed attempts: sweeps with any worker max < 50 um', sum(r['anyw50'] for r in fails), '< 20', sum(r['anyw20'] for r in fails), '< 4', sum(r['anyw4'] for r in fails))
    b5f = [r for r in sel if r['bite']==5 and not r['pub']]
    print(arm, 'bite-5 failed ratio best loser / best winner max: med of ratios %.3f; ratio of medians %.3f' % (med([r['best_l']/r['best_w'] for r in b5f]), med([r['best_l'] for r in b5f])/med([r['best_w'] for r in b5f])))
pubs = [r for r in rows if r['pub']]; fails = [r for r in rows if not r['pub']]
print('loser<band & winner not: published attempts 4um', sum(r['l4'] for r in pubs), '50um', sum(r['l50'] for r in pubs), '; failed attempts 4um', sum(r['l4'] for r in fails), '50um', sum(r['l50'] for r in fails))
# container commits per sweep per seed, bite 5: B above A on how many seeds; retained share A above B
seeds = sorted({r['seed'] for r in rows})
ba = 0; sa = 0
for sd in seeds:
    ra = [r for r in rows if r['seed']==sd and r['bite']==5 and r['arm']=='A'][0]; rb = [r for r in rows if r['seed']==sd and r['bite']==5 and r['arm']=='B'][0]
    if rb['cc_all']/rb['n'] > ra['cc_all']/ra['n']: ba += 1
    if ra['cc_win']/ra['cc_all'] > rb['cc_win']/rb['cc_all']: sa += 1
print('bite 5: container commits/sweep B > A on', ba, 'of', len(seeds), '; retained share A > B on', sa, 'of', len(seeds))
print('## quartiles: container commits per sweep (all workers) pooled per group')
for g in groups:
    sel = [r for r in rows if (r['arm'], r['bite'], r['pub']) == g]
    if not sel: continue
    print(g, ['%.2f' % (sum(r['quart'][i]['cc_all'] for r in sel) / sum(r['quart'][i]['n'] for r in sel)) for i in range(4)])
json.dump(rows, open('verify-2-rows.json', 'w'))
