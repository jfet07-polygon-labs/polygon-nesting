#!/usr/bin/env python3
"""Per-attempt 'why the attempt ends and what the work buys' table for the 42 depth-microscope
documents (Wall10s, margin 8). Reads only /var/lib/t3/tmp/astra/deep/deep-*-wall10s-s*.json.
Writes analysis/ends-attempts.json (rows) and prints markdown tables + aggregates.
Usage: python3 ends-attempts.py
"""
import json, glob, os, sys, statistics as st

DEEP = '/var/lib/t3/tmp/astra/deep'
OUT = os.path.join(DEEP, 'analysis', 'ends-attempts.json')

def med(xs):
    xs = [x for x in xs if x is not None]
    return st.median(xs) if xs else None

rows = []
for f in sorted(glob.glob(os.path.join(DEEP, 'deep-*-wall10s-s*.json'))):
    d = json.load(open(f))
    arm = os.path.basename(f)[5]                      # deep-A-... / deep-B-...
    p_exp = d.get('guidedExponent', 2.0)              # A documents omit it (default p = 2); B carry 1.0
    assert (arm == 'A' and p_exp == 2.0) or (arm == 'B' and p_exp == 1.0), (f, p_exp)
    seed = str(d['seed'])
    m = d['biteMicroscope']
    for b in m['bites']:
        for s in b['separations']:
            sm = s['samples']            # [iteration, rawPhi, guidedPhi, maxViolationMm, newMinimum, bandEntry, exactCalls]
            sw = s['sweeps']
            n = s['iterations']
            assert n == len(sw) == len(sm) - 1
            raws = [x[1] for x in sm]; mxs = [x[3] for x in sm]
            best_raw = min(raws); best_raw_it = raws.index(best_raw)
            best_max = min(mxs); best_max_it = mxs.index(best_max)
            # monotonicity of the winner's rawAfter across master iterations
            ups = sum(1 for i in range(1, len(raws)) if raws[i] > raws[i-1])
            downs = sum(1 for i in range(1, len(raws)) if raws[i] < raws[i-1])
            newmins = [x[0] for x in sm[1:] if x[4]]
            last10_start = n - max(1, n // 10)         # iterations >= this are 'the last 10 %'
            newmin_last10 = sum(1 for it in newmins if it >= last10_start)
            newmin_last20 = sum(1 for it in newmins if it >= n - max(1, n // 5))
            # best raw before the last-10 % window versus inside it
            before = min(raws[:last10_start]) if last10_start > 0 else None
            inside = min(raws[last10_start:])
            # rollbacks: raw at the rollback iteration, raw at the restored iteration, raw right after
            rbs = []
            for at, to in s['rollbacks']:
                rbs.append({'at': at, 'to': to, 'rawAt': raws[at], 'rawRestored': raws[to],
                            'rawNext': raws[at+1] if at+1 < len(raws) else None,
                            'bestAfterRollback': min(raws[at+1:]) if at+1 < len(raws) else None,
                            'improvedAfterRollback': (min(raws[at+1:]) < raws[to]) if at+1 < len(raws) else None})
            ev_winner = sum(x['evaluationsWinner'] for x in sw)
            ev_all_sw = sum(x['evaluationsAllWorkers'] for x in sw)
            we, ws = s['wallAtEntry'], s['wallAtStop']
            dur = ws['elapsedS'] - we['elapsedS']
            rows.append({
                'file': os.path.basename(f), 'arm': arm, 'p': p_exp, 'seed': seed,
                'bite': b['ordinal'], 'capsule': s['capsule'], 'attempt': s['attempt'],
                'parentMm': b['parentDepthMm'], 'targetMm': b['targetDepthMm'],
                'stop': s['stop'], 'published': b['published'],
                'entryElapsedS': we['elapsedS'], 'entryLeftS': we['leftS'], 'stopElapsedS': ws['elapsedS'], 'stopLeftS': ws['leftS'],
                'phaseDeadlineS': we['phaseDeadlineS'], 'durationS': dur,
                'iterations': n, 'itPerS': n / dur if dur > 0 else None,
                'evalAll': s['evaluationsAllWorkers'], 'evalAllSweepSum': ev_all_sw, 'evalWinner': ev_winner,
                'evalPerIt': s['evaluationsAllWorkers'] / n, 'evalWinnerPerIt': ev_winner / n,
                'evalPerS': s['evaluationsAllWorkers'] / dur if dur > 0 else None,
                'strikes': s['strikes'], 'rollbacks': s['rollbacks'], 'nRollbacks': len(s['rollbacks']),
                'restoredTo': s['restoredToIteration'], 'bandEntries': s['bandEntries'], 'exactCalls': s['exactCheckpointCalls'],
                'minRaw': s['minRaw'],
                'entryRaw': raws[0], 'firstRaw': raws[1], 'bestRaw': best_raw, 'bestRawIt': best_raw_it, 'lastRaw': raws[-1],
                'entryMax': mxs[0], 'firstMax': mxs[1], 'bestMax': best_max, 'bestMaxIt': best_max_it, 'lastMax': mxs[-1],
                'ups': ups, 'downs': downs, 'nNewMin': len(newmins), 'lastNewMinIt': newmins[-1] if newmins else 0,
                'newMinLast10': newmin_last10, 'newMinLast20': newmin_last20,
                'bestRawFrac': best_raw_it / n, 'bestMaxFrac': best_max_it / n,
                'bestBeforeLast10': before, 'bestInLast10': inside,
                'rollbackDetail': rbs,
                'restoredRaw': raws[s['restoredToIteration']] if s['restoredToIteration'] is not None else None,
                'restoredMax': mxs[s['restoredToIteration']] if s['restoredToIteration'] is not None else None,
                'usefulMovesDiscarded': s['usefulMovesDiscarded'], 'discardedExpenditure': s['discardedExpenditure'],
                'stopBlockingRows': len(s['stopBlocking']), 'entryBlockingRows': len(s['entryBlocking']),
            })

json.dump(rows, open(OUT, 'w'), indent=1)

def fmt(x, nd=3):
    if x is None: return '-'
    if isinstance(x, float): return f'{x:.{nd}f}'
    return str(x)

def table(rs, title):
    print(f'\n### {title}\n')
    print('| arm | seed | bite | stop | wall entry left s | wall stop left s | dur s | iters | it/s | evals all | evals winner | evals/it | strikes | rollbacks [at->to] | restoredTo | band | exact | entry raw | first raw | best raw @it | last raw | entry max | best max @it | last max | ups/downs | newmin last10% |')
    print('|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|')
    for r in rs:
        rb = ','.join(f'{a}->{t}' for a, t in r['rollbacks']) or '-'
        print(f"| {r['arm']} p={r['p']:.0f} | {r['seed']} | {r['bite']} | {r['stop']} | {fmt(r['entryLeftS'])} | {fmt(r['stopLeftS'])} | {fmt(r['durationS'],2)} | {r['iterations']} | {fmt(r['itPerS'],1)} | {r['evalAll']} | {r['evalWinner']} | {fmt(r['evalPerIt'],0)} | {r['strikes']} | {rb} | {fmt(r['restoredTo'])} | {r['bandEntries']} | {r['exactCalls']} | {fmt(r['entryRaw'],1)} | {fmt(r['firstRaw'],1)} | {fmt(r['bestRaw'],3)} @{r['bestRawIt']} | {fmt(r['lastRaw'],1)} | {fmt(r['entryMax'],2)} | {fmt(r['bestMax'],4)} @{r['bestMaxIt']} | {fmt(r['lastMax'],2)} | {r['ups']}/{r['downs']} | {r['newMinLast10']} |")

b5 = [r for r in rows if r['bite'] == 5]
b6 = [r for r in rows if r['bite'] == 6]
table(sorted(b5, key=lambda r: (r['arm'], int(r['seed']))), 'Bite 5 (trigger) attempts: all 42')
table(sorted(b6, key=lambda r: (r['arm'], int(r['seed']))), 'Bite 6 attempts (documents whose bite 5 published)')

print('\n### Aggregates per arm (bite 5)\n')
for arm in 'AB':
    rs = [r for r in b5 if r['arm'] == arm]
    print(f"arm {arm} (n={len(rs)}): stop reasons {dict((k, sum(1 for r in rs if r['stop']==k)) for k in sorted(set(r['stop'] for r in rs)))}; "
          f"published {sum(r['published'] for r in rs)}; bandEntries>0 {sum(r['bandEntries']>0 for r in rs)}; exactCalls>0 {sum(r['exactCalls']>0 for r in rs)}")
    print(f"  iterations min/med/max {min(r['iterations'] for r in rs)}/{med([r['iterations'] for r in rs])}/{max(r['iterations'] for r in rs)}; "
          f"evals all min/med/max {min(r['evalAll'] for r in rs)}/{med([r['evalAll'] for r in rs])}/{max(r['evalAll'] for r in rs)}")
    print(f"  evals/iteration (all workers) min/med/max {fmt(min(r['evalPerIt'] for r in rs),0)}/{fmt(med([r['evalPerIt'] for r in rs]),0)}/{fmt(max(r['evalPerIt'] for r in rs),0)}; "
          f"winner evals/iteration med {fmt(med([r['evalWinnerPerIt'] for r in rs]),0)}; it/s min/med/max {fmt(min(r['itPerS'] for r in rs),1)}/{fmt(med([r['itPerS'] for r in rs]),1)}/{fmt(max(r['itPerS'] for r in rs),1)}; "
          f"evals/s med {fmt(med([r['evalPerS'] for r in rs]),0)}")
    print(f"  wall left at entry min/med/max {fmt(min(r['entryLeftS'] for r in rs))}/{fmt(med([r['entryLeftS'] for r in rs]))}/{fmt(max(r['entryLeftS'] for r in rs))}; "
          f"duration min/med/max {fmt(min(r['durationS'] for r in rs),2)}/{fmt(med([r['durationS'] for r in rs]),2)}/{fmt(max(r['durationS'] for r in rs),2)}; "
          f"wall left at stop (deadline stops) min/max {fmt(min(r['stopLeftS'] for r in rs if r['stop']=='deadline'))}/{fmt(max(r['stopLeftS'] for r in rs if r['stop']=='deadline'))}")
    print(f"  strikes: {sorted(r['strikes'] for r in rs)}; rollbacks per attempt: {sorted(r['nRollbacks'] for r in rs)}")
    fails = [r for r in rs if r['stop'] == 'deadline']
    print(f"  deadline-stopped n={len(fails)}: best raw min/med/max {fmt(min(r['bestRaw'] for r in fails))}/{fmt(med([r['bestRaw'] for r in fails]))}/{fmt(max(r['bestRaw'] for r in fails))}; "
          f"best max (mm) min/med/max {fmt(min(r['bestMax'] for r in fails),4)}/{fmt(med([r['bestMax'] for r in fails]),4)}/{fmt(max(r['bestMax'] for r in fails),4)}; "
          f"best-raw iteration fraction min/med/max {fmt(min(r['bestRawFrac'] for r in fails),2)}/{fmt(med([r['bestRawFrac'] for r in fails]),2)}/{fmt(max(r['bestRawFrac'] for r in fails),2)}")
    print(f"  deadline-stopped: last raw / best raw min/med/max {fmt(min(r['lastRaw']/r['bestRaw'] for r in fails),1)}/{fmt(med([r['lastRaw']/r['bestRaw'] for r in fails]),1)}/{fmt(max(r['lastRaw']/r['bestRaw'] for r in fails),1)}; "
          f"restored-state raw min/med/max {fmt(min(r['restoredRaw'] for r in fails))}/{fmt(med([r['restoredRaw'] for r in fails]))}/{fmt(max(r['restoredRaw'] for r in fails))}")

print('\n### Classification of the 42 trigger attempts (non-exclusive flags, then exclusive classes)\n')
def cls(r):
    if r['bandEntries'] > 0: return 'band'
    if r['bestRawFrac'] >= 0.9: return 'improving-at-deadline (best raw in last 10%)'
    if r['bestRawFrac'] < 0.5: return 'plateau (best raw before middle)'
    return 'best raw in 50-90%'
for arm in 'AB':
    rs = [r for r in b5 if r['arm'] == arm]
    print(f"arm {arm} (n={len(rs)}): reach band {sum(r['bandEntries']>0 for r in rs)}; "
          f"deadline & best raw in last 10% {sum(r['stop']=='deadline' and r['bestRawFrac']>=0.9 for r in rs)}; "
          f"deadline & best raw before middle {sum(r['stop']=='deadline' and r['bestRawFrac']<0.5 for r in rs)}; "
          f"deadline & best raw in [50%,90%) {sum(r['stop']=='deadline' and 0.5<=r['bestRawFrac']<0.9 for r in rs)}; "
          f"degrade (last raw > 2x best) {sum(r['lastRaw']>2*r['bestRaw'] for r in rs)}; "
          f"deadline & any new raw minimum in last 10% {sum(r['stop']=='deadline' and r['newMinLast10']>0 for r in rs)}; "
          f"deadline & any new raw minimum in last 20% {sum(r['stop']=='deadline' and r['newMinLast20']>0 for r in rs)}")
    from collections import Counter
    print('  exclusive classes:', dict(Counter(cls(r) for r in rs)))
    print('  per-seed (deadline stops): seed: bestRaw@it/iters (frac), lastNewMin it, last/best')
    for r in sorted(rs, key=lambda r: r['bestRawFrac']):
        if r['stop'] == 'deadline':
            print(f"    {r['seed']}: {r['bestRaw']:.3f}@{r['bestRawIt']}/{r['iterations']} ({r['bestRawFrac']:.2f}), lastNewMin {r['lastNewMinIt']}, last/best {r['lastRaw']/r['bestRaw']:.1f}, restoredTo {r['restoredTo']} raw {fmt(r['restoredRaw'])}")

print('\n### Rollbacks: what follows each\n')
for arm in 'AB':
    rs = [r for r in rows if r['arm'] == arm]
    allrb = [(r, rb) for r in rs for rb in r['rollbackDetail']]
    n_imp = sum(1 for r, rb in allrb if rb['improvedAfterRollback'])
    print(f"arm {arm}: rollbacks {len(allrb)} across {sum(1 for r in rs if r['nRollbacks'])} attempts; after-rollback best raw beats the restored raw in {n_imp} of {len(allrb)}; "
          f"gap at->to (iterations) min/med/max {min(rb['at']-rb['to'] for r,rb in allrb)}/{med([rb['at']-rb['to'] for r,rb in allrb])}/{max(rb['at']-rb['to'] for r,rb in allrb)}")
    for r, rb in allrb:
        print(f"   {r['seed']} b{r['bite']}: at {rb['at']} -> {rb['to']}: raw at rollback {rb['rawAt']:.2f}, restored raw {rb['rawRestored']:.3f}, next raw {fmt(rb['rawNext'],2)}, best after {fmt(rb['bestAfterRollback'],3)}, improved {rb['improvedAfterRollback']}")

print('\n### Bite 6 aggregates\n')
for arm in 'AB':
    rs = [r for r in b6 if r['arm'] == arm]
    if not rs: print(f'arm {arm}: none'); continue
    print(f"arm {arm} (n={len(rs)}): stops {[r['stop'] for r in rs]}; iterations {[r['iterations'] for r in rs]}; wall left at entry {[round(r['entryLeftS'],3) for r in rs]}; "
          f"best raw {[round(r['bestRaw'],2) for r in rs]} at it {[r['bestRawIt'] for r in rs]} (frac {[round(r['bestRawFrac'],2) for r in rs]}); last raw {[round(r['lastRaw'],1) for r in rs]}; band {[r['bandEntries'] for r in rs]}; "
          f"evals/it {[round(r['evalPerIt']) for r in rs]}; it/s {[round(r['itPerS'],1) for r in rs]}")
