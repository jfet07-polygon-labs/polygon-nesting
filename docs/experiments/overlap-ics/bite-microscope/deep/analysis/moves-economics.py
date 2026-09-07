#!/usr/bin/env python3
"""Where useful moves disappear: per-worker economics of every retained attempt
in the 42 depth-microscope documents (Wall10s, margin 8, arms A p=2 / B p=1).

Reads /var/lib/t3/tmp/astra/deep/deep-<A|B>-wall10s-s<seed>.json (read-only),
writes /var/lib/t3/tmp/astra/deep/analysis/moves-economics.json (one row per
attempt) and prints the per-attempt table.  stdlib only.

Definitions (engine: crates/polygon-nesting-core/src/search/overlap_ics/microscope.rs
WorkerSweepRecord, mod.rs merge): per master iteration eight workers sweep the
same master state; the tournament installs the worker with the lowest
workers[].guidedAfter (pre-GLS-update guided, strict '<', ties to the lowest
ordinal); 'contested' = at least two workers reached different guided totals;
a 'useful move' = a relocate that moved AND lowered the piece's incident guided
energy on the worker's own state; usefulMovesDiscarded = sum of the losers'
usefulMoves; discardedExpenditure = sum of the losers' sampleEvaluations.
"""
import json, glob, os, re, statistics as st

DEEP = '/var/lib/t3/tmp/astra/deep'
OUT = os.path.join(DEEP, 'analysis', 'moves-economics.json')
TREAT_SUCCESS = {4872519857840070441, 5671471283886933426, 11151532166486038253,
                 17316774662183274765, 18390115156762500293}
CONTROL_SUCCESS = {10636268072709740349}

def argmin(vals):
    """index of the strict minimum; ties broken to the lowest index (the engine's rule)."""
    best = 0
    for i in range(1, len(vals)):
        if vals[i] < vals[best]:
            best = i
    return best

rows = []
for path in sorted(glob.glob(os.path.join(DEEP, 'deep-*-wall10s-s*.json'))):
    m = re.match(r'deep-([AB])-wall10s-s(\d+)\.json', os.path.basename(path))
    arm, seed = m.group(1), int(m.group(2))
    doc = json.load(open(path))
    ms = doc['biteMicroscope']
    for bite in ms['bites']:
        for sep in bite['separations']:
            sweeps = sep['sweeps']
            n = len(sweeps)
            r = dict(arm=arm, seed=seed, bite=bite['ordinal'], attempt=sep['attempt'],
                     capsule=sep['capsule'], published=bool(bite['published']) and sep['stop'] == 'published',
                     stop=sep['stop'], iterations=sep['iterations'], sweeps=n,
                     seedClass=('treatment-success-seed' if seed in TREAT_SUCCESS else
                                'control-success-seed' if seed in CONTROL_SUCCESS else 'failed-both-arms'),
                     strikes=sep['strikes'], rollbacks=len(sep['rollbacks']), minRaw=sep['minRaw'],
                     usefulMovesDiscarded=sep['usefulMovesDiscarded'],
                     discardedExpenditure=sep['discardedExpenditure'],
                     evaluationsAllWorkers=sep['evaluationsAllWorkers'])
            # ---- pooled counters over sweeps
            tot = dict(evalAll=0, evalWin=0, relAll=0, relWin=0, movedAll=0, movedWin=0,
                       ccAll=0, ccWin=0, usefulAll=0, usefulWin=0, usefulEvalAll=0, usefulEvalWin=0,
                       discardedUseful=0, discardedSpend=0, winnerOriginContainer=0, winnerOriginFocused=0,
                       winnerOriginStayPut=0, winnerRelocatesMoved=0)
            c = dict(contested=0, winMinGuided=0, winMinRaw=0, winMinMax=0, winMinRawAndMax=0,
                     loserLowerRaw=0, loserLowerMax=0, loserLowerRawOrMax=0, loserLowerBoth=0,
                     winMostUseful=0, winMostCC=0, ccAnyWorker=0, ccWinner=0, ccAllLost=0,
                     nLosersLowerRaw=0, nLosersLowerMax=0, workersCount=0)
        # per-sweep magnitudes
            rawGap, maxGap, usefulRank, ccPerSweepAll, ccPerSweepWin = [], [], [], [], []
            phase = {}  # quartile -> counters
            for q in ('Q1', 'Q2', 'Q3', 'Q4'):
                phase[q] = dict(n=0, contested=0, loserLowerRaw=0, loserLowerMax=0, winMinRaw=0, winMinMax=0,
                                ccAll=0, ccWin=0, usefulAll=0, usefulWin=0)
            for k, sw in enumerate(sweeps):
                W = sw['workers']
                w = sw['winner']
                nW = len(W)
                c['workersCount'] += nW
                ev = [x['sampleEvaluations'] for x in W]
                tot['evalAll'] += sum(ev); tot['evalWin'] += W[w]['sampleEvaluations']
                assert sum(ev) == sw['evaluationsAllWorkers'], (path, k)
                assert W[w]['sampleEvaluations'] == sw['evaluationsWinner'], (path, k)
                tot['relAll'] += sum(x['relocates'] for x in W); tot['relWin'] += W[w]['relocates']
                tot['movedAll'] += sum(x['moved'] for x in W); tot['movedWin'] += W[w]['moved']
                ccs = [x['containerCommits'] for x in W]
                tot['ccAll'] += sum(ccs); tot['ccWin'] += ccs[w]
                ccPerSweepAll.append(sum(ccs)); ccPerSweepWin.append(ccs[w])
                if sum(ccs) > 0:
                    c['ccAnyWorker'] += 1
                    if ccs[w] > 0: c['ccWinner'] += 1
                    else: c['ccAllLost'] += 1
                us = [x['usefulMoves'] for x in W]
                tot['usefulAll'] += sum(us); tot['usefulWin'] += us[w]
                tot['usefulEvalAll'] += sum(x['usefulMoveEvaluations'] for x in W)
                tot['usefulEvalWin'] += W[w]['usefulMoveEvaluations']
                losersUseful = sum(us) - us[w]; losersSpend = sum(ev) - ev[w]
                assert losersUseful == sw['usefulMovesDiscarded'], (path, k)
                assert losersSpend == sw['discardedExpenditure'], (path, k)
                tot['discardedUseful'] += losersUseful; tot['discardedSpend'] += losersSpend
                # the winner's own relocates by origin (retained long-range moves)
                for rel in sw['relocates']:
                    if rel['moved']:
                        tot['winnerRelocatesMoved'] += 1
                        o = rel['origin']
                        if o == 'container': tot['winnerOriginContainer'] += 1
                        elif o == 'focused': tot['winnerOriginFocused'] += 1
                        elif o == 'stayPut': tot['winnerOriginStayPut'] += 1
                        else: raise SystemExit('origin ' + o)
                assert tot['winnerOriginContainer'] >= 0
                # tournament checks
                g = [x['guidedAfter'] for x in W]; ra = [x['rawAfter'] for x in W]; mx = [x['maxAfterMm'] for x in W]
                if sw['contested']: c['contested'] += 1
                assert sw['contested'] == any(x != g[0] for x in g), (path, k)
                if argmin(g) == w: c['winMinGuided'] += 1
                minRaw = argmin(ra) == w; minMax = argmin(mx) == w
                if minRaw: c['winMinRaw'] += 1
                if minMax: c['winMinMax'] += 1
                if minRaw and minMax: c['winMinRawAndMax'] += 1
                lr = [i for i in range(nW) if i != w and ra[i] < ra[w]]
                lm = [i for i in range(nW) if i != w and mx[i] < mx[w]]
                c['nLosersLowerRaw'] += len(lr); c['nLosersLowerMax'] += len(lm)
                if lr: c['loserLowerRaw'] += 1; rawGap.append((ra[w] - min(ra)) / ra[w] if ra[w] > 0 else 0.0)
                if lm: c['loserLowerMax'] += 1; maxGap.append((mx[w] - min(mx)) / mx[w] if mx[w] > 0 else 0.0)
                if lr or lm: c['loserLowerRawOrMax'] += 1
                if set(lr) & set(lm): c['loserLowerBoth'] += 1
                if us[w] == max(us): c['winMostUseful'] += 1
                if ccs[w] == max(ccs): c['winMostCC'] += 1
                usefulRank.append(sorted(us, reverse=True).index(us[w]) + 1)
                q = 'Q%d' % (min(3, (4 * k) // n) + 1)
                P = phase[q]; P['n'] += 1
                P['contested'] += int(sw['contested']); P['loserLowerRaw'] += int(bool(lr)); P['loserLowerMax'] += int(bool(lm))
                P['winMinRaw'] += int(minRaw); P['winMinMax'] += int(minMax)
                P['ccAll'] += sum(ccs); P['ccWin'] += ccs[w]; P['usefulAll'] += sum(us); P['usefulWin'] += us[w]
            assert tot['discardedUseful'] == sep['usefulMovesDiscarded'], path
            assert tot['discardedSpend'] == sep['discardedExpenditure'], path
            assert tot['evalAll'] == sep['evaluationsAllWorkers'], path
            assert tot['winnerOriginContainer'] == tot['ccWin'], (path, tot['winnerOriginContainer'], tot['ccWin'])
            r.update(tot); r.update(c)
            r['workersPerSweep'] = c['workersCount'] / n
            r['winnerEvalShare'] = tot['evalWin'] / tot['evalAll']
            r['usefulRetainedShare'] = tot['usefulWin'] / tot['usefulAll'] if tot['usefulAll'] else None
            r['ccRetainedShare'] = tot['ccWin'] / tot['ccAll'] if tot['ccAll'] else None
            r['usefulOverMovedAll'] = tot['usefulAll'] / tot['movedAll'] if tot['movedAll'] else None
            r['fracContested'] = c['contested'] / n
            r['fracWinMinGuided'] = c['winMinGuided'] / n
            r['fracWinMinRaw'] = c['winMinRaw'] / n
            r['fracWinMinMax'] = c['winMinMax'] / n
            r['fracWinMinRawAndMax'] = c['winMinRawAndMax'] / n
            r['fracLoserLowerRaw'] = c['loserLowerRaw'] / n
            r['fracLoserLowerMax'] = c['loserLowerMax'] / n
            r['fracLoserLowerRawOrMax'] = c['loserLowerRawOrMax'] / n
            r['fracLoserLowerBoth'] = c['loserLowerBoth'] / n
            r['meanLosersLowerRaw'] = c['nLosersLowerRaw'] / n
            r['meanLosersLowerMax'] = c['nLosersLowerMax'] / n
            r['medianRawGapRel'] = st.median(rawGap) if rawGap else None
            r['medianMaxGapRel'] = st.median(maxGap) if maxGap else None
            r['fracWinMostUseful'] = c['winMostUseful'] / n
            r['meanWinnerUsefulRank'] = st.mean(usefulRank)
            r['ccPerSweepAll'] = tot['ccAll'] / n
            r['ccPerSweepWin'] = tot['ccWin'] / n
            r['fracSweepsCCany'] = c['ccAnyWorker'] / n
            r['fracCCsweepsWinnerHasCC'] = c['ccWinner'] / c['ccAnyWorker'] if c['ccAnyWorker'] else None
            r['phase'] = phase
            rows.append(r)

json.dump(rows, open(OUT, 'w'), indent=1)
hdr = ('arm seed bite att pub stop iters contested% winMinRaw% winMinMax% loserLowRaw% loserLowMax% '
       'evalAll evalWin share% usefulAll usefulWin discarded discardSpend ccAll ccWin ccRet%')
print(hdr)
for r in rows:
    print(f"{r['arm']} {r['seed']} b{r['bite']} a{r['attempt']} {'P' if r['published'] else 'F'} {r['stop']:<9} {r['sweeps']:4d} "
          f"{100*r['fracContested']:6.1f} {100*r['fracWinMinRaw']:6.1f} {100*r['fracWinMinMax']:6.1f} "
          f"{100*r['fracLoserLowerRaw']:6.1f} {100*r['fracLoserLowerMax']:6.1f} "
          f"{r['evalAll']:9d} {r['evalWin']:8d} {100*r['winnerEvalShare']:5.2f} {r['usefulAll']:7d} {r['usefulWin']:6d} "
          f"{r['usefulMovesDiscarded']:7d} {r['discardedExpenditure']:9d} {r['ccAll']:6d} {r['ccWin']:5d} {100*r['ccRetainedShare']:5.1f}")
print('rows', len(rows), '->', OUT)
