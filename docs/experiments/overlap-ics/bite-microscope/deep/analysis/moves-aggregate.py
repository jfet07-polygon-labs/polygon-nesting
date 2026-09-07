#!/usr/bin/env python3
"""Aggregate moves-economics.json by arm x outcome class; chance baselines;
quartile phases; paired A/B per seed on bite 5.  stdlib only."""
import json, statistics as st, collections
rows = json.load(open('/var/lib/t3/tmp/astra/deep/analysis/moves-economics.json'))

def cls(r):
    if r['bite'] == 5: return 'bite5-published' if r['published'] else 'bite5-failed'
    return 'bite6-failed'
def med(xs): return st.median(xs)
def fmt(x, p=1): return '-' if x is None else f'{x:.{p}f}'

groups = collections.OrderedDict()
for r in rows: groups.setdefault((r['arm'], cls(r)), []).append(r)
print('# per attempt, the median over attempts (and pooled where marked); n = attempts')
print('| arm | class | n | median sweeps | contested % | winner=min guided % | winner=min raw % | winner=min max % | winner=min raw&max % | some loser lower raw % | some loser lower max % | losers lower raw per sweep | losers lower max per sweep | median rel raw gap (best loser vs winner) | median rel max gap |')
print('|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|')
for (arm, c), g in groups.items():
    print(f"| {arm} | {c} | {len(g)} | {med([r['sweeps'] for r in g]):.0f} | {fmt(100*med([r['fracContested'] for r in g]))} | {fmt(100*med([r['fracWinMinGuided'] for r in g]))} | "
          f"{fmt(100*med([r['fracWinMinRaw'] for r in g]))} | {fmt(100*med([r['fracWinMinMax'] for r in g]))} | {fmt(100*med([r['fracWinMinRawAndMax'] for r in g]))} | "
          f"{fmt(100*med([r['fracLoserLowerRaw'] for r in g]))} | {fmt(100*med([r['fracLoserLowerMax'] for r in g]))} | "
          f"{fmt(med([r['meanLosersLowerRaw'] for r in g]),2)} | {fmt(med([r['meanLosersLowerMax'] for r in g]),2)} | "
          f"{fmt(100*med([r['medianRawGapRel'] for r in g]))} % | {fmt(100*med([r['medianMaxGapRel'] for r in g]))} % |")
print()
print('# pooled over sweeps (sum over attempts in the group)')
print('| arm | class | sweeps | evaluations all workers | winner share % | relocates all | moved all | useful all | useful/moved % | useful retained (winner) | useful discarded | discarded expenditure | discarded expenditure share % | container commits all | per sweep (all workers) | retained (winner) | per sweep (winner) | retained share % | sweeps with any container commit % | of those, winner had one % | winner has most useful moves % | mean winner rank by useful moves (1..8) |')
print('|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|')
for (arm, c), g in groups.items():
    S = lambda k: sum(r[k] for r in g)
    n = S('sweeps')
    print(f"| {arm} | {c} | {n} | {S('evalAll')} | {100*S('evalWin')/S('evalAll'):.2f} | {S('relAll')} | {S('movedAll')} | {S('usefulAll')} | {100*S('usefulAll')/S('movedAll'):.1f} | {S('usefulWin')} | {S('usefulMovesDiscarded')} | {S('discardedExpenditure')} | {100*S('discardedExpenditure')/S('evalAll'):.2f} | "
          f"{S('ccAll')} | {S('ccAll')/n:.2f} | {S('ccWin')} | {S('ccWin')/n:.2f} | {100*S('ccWin')/S('ccAll'):.1f} | {100*S('ccAnyWorker')/n:.1f} | {100*S('ccWinner')/S('ccAnyWorker'):.1f} | {100*S('winMostUseful')/n:.1f} | {sum(r['meanWinnerUsefulRank']*r['sweeps'] for r in g)/n:.2f} |")
print()
print('# chance baselines with 8 workers ranked by guided only: P(winner = min raw) = 1/8 = 12.5 %; P(some loser lower raw) = 7/8 = 87.5 %; winner evaluation share and container-commit retained share = 12.5 % if independent of winning')
print()
print('# quartile phases of each attempt (pooled over sweeps in the group): does the raw/guided disagreement move over the attempt?')
print('| arm | class | quartile | sweeps | contested % | winner=min raw % | winner=min max % | some loser lower raw % | some loser lower max % | container commits/sweep all | container commits/sweep winner | cc retained % | useful/sweep all | useful/sweep winner |')
print('|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|')
for (arm, c), g in groups.items():
    for q in ('Q1','Q2','Q3','Q4'):
        P = collections.Counter()
        for r in g:
            for k, v in r['phase'][q].items(): P[k] += v
        n = P['n']
        print(f"| {arm} | {c} | {q} | {n} | {100*P['contested']/n:.1f} | {100*P['winMinRaw']/n:.1f} | {100*P['winMinMax']/n:.1f} | {100*P['loserLowerRaw']/n:.1f} | {100*P['loserLowerMax']/n:.1f} | {P['ccAll']/n:.2f} | {P['ccWin']/n:.2f} | {100*P['ccWin']/P['ccAll'] if P['ccAll'] else 0:.1f} | {P['usefulAll']/n:.1f} | {P['usefulWin']/n:.1f} |")
print()
print('# paired per seed, bite 5, A (p=2) vs B (p=1): the same parent geometry class, the two objectives')
print('| seed | seed class | A pub | B pub | A sweeps | B sweeps | A win=min raw % | B win=min raw % | A win=min max % | B win=min max % | A loser lower raw % | B loser lower raw % | A cc/sweep all | B cc/sweep all | A cc retained % | B cc retained % | A discarded useful | B discarded useful | A discarded spend | B discarded spend |')
print('|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|')
by = {(r['arm'], r['seed']): r for r in rows if r['bite'] == 5}
seeds = sorted({r['seed'] for r in rows}, key=lambda s: (len(str(s)), s))
diffs = collections.defaultdict(list)
for s in seeds:
    a, b = by[('A', s)], by[('B', s)]
    print(f"| {s} | {a['seedClass']} | {'P' if a['published'] else 'F'} | {'P' if b['published'] else 'F'} | {a['sweeps']} | {b['sweeps']} | {100*a['fracWinMinRaw']:.1f} | {100*b['fracWinMinRaw']:.1f} | {100*a['fracWinMinMax']:.1f} | {100*b['fracWinMinMax']:.1f} | {100*a['fracLoserLowerRaw']:.1f} | {100*b['fracLoserLowerRaw']:.1f} | {a['ccPerSweepAll']:.2f} | {b['ccPerSweepAll']:.2f} | {100*a['ccRetainedShare']:.1f} | {100*b['ccRetainedShare']:.1f} | {a['usefulMovesDiscarded']} | {b['usefulMovesDiscarded']} | {a['discardedExpenditure']} | {b['discardedExpenditure']} |")
    for k in ('fracWinMinRaw','fracWinMinMax','fracLoserLowerRaw','fracLoserLowerMax','ccPerSweepAll','ccRetainedShare','winnerEvalShare','usefulOverMovedAll'):
        diffs[k].append(b[k] - a[k])
print()
print('# paired differences B - A on bite 5 (21 seeds): median, count B>A / B<A')
for k, d in diffs.items():
    print(f"  {k}: median {st.median(d):+.4f}, B>A on {sum(1 for x in d if x>0)}, B<A on {sum(1 for x in d if x<0)} of {len(d)}")
print()
print('# bite 5 published vs failed inside each arm (attempt-level medians): winner=min raw %, some loser lower raw %, cc retained %, useful/moved %')
for arm in 'AB':
    for c in ('bite5-published','bite5-failed'):
        g = groups.get((arm, c), [])
        if not g: continue
        print(f"  {arm} {c} n={len(g)}: win=min raw {100*med([r['fracWinMinRaw'] for r in g]):.1f} %, loser lower raw {100*med([r['fracLoserLowerRaw'] for r in g]):.1f} %, loser lower max {100*med([r['fracLoserLowerMax'] for r in g]):.1f} %, cc retained {100*med([r['ccRetainedShare'] for r in g]):.1f} %, cc/sweep all {med([r['ccPerSweepAll'] for r in g]):.2f}, useful/moved {100*med([r['usefulOverMovedAll'] for r in g]):.1f} %, winner eval share {100*med([r['winnerEvalShare'] for r in g]):.2f} %, useful retained share {100*med([r['usefulRetainedShare'] for r in g]):.2f} %")
print()
print('# ranges over all 50 attempts')
for k in ('fracContested','fracWinMinGuided','winnerEvalShare','usefulRetainedShare','ccRetainedShare','fracWinMinRaw','fracWinMinMax','fracLoserLowerRaw','fracLoserLowerMax','fracLoserLowerBoth','usefulOverMovedAll','workersPerSweep'):
    xs = [r[k] for r in rows]
    print(f"  {k}: min {min(xs):.4f} median {st.median(xs):.4f} max {max(xs):.4f}")
