#!/usr/bin/env python3
"""Replays of every retained attempt's entry capsule under both objectives with the live horizon:
cost per iteration, band entry, evaluations beside iterations, and the trajectory's best-raw position.
Reads /var/lib/t3/tmp/astra/deep/replays/rp-*.json and the live documents. Usage: python3 ends-replays.py"""
import json, glob, os, re, statistics as st
DEEP = '/var/lib/t3/tmp/astra/deep'
def med(xs): xs = [x for x in xs if x is not None]; return st.median(xs) if xs else None
live = {(r['arm'], r['seed'], r['bite']): r for r in json.load(open(os.path.join(DEEP, 'analysis', 'ends-attempts.json')))}
rows = []
for f in sorted(glob.glob(os.path.join(DEEP, 'replays', 'rp-*.json'))):
    m = re.match(r'rp-deep-([AB])-wall10s-s(\d+)-b(\d)-c(\d)-p([12])\.json', os.path.basename(f))
    arm, seed, bite, cap, p = m.group(1), m.group(2), int(m.group(3)), int(m.group(4)), int(m.group(5))
    r = json.load(open(f))['replay']
    its = r['iterations']; n = len(its)
    raws = [r['entryRaw']] + [x['rawAfter'] for x in its]
    ev = [x['evaluationsAllWorkers'] for x in its]
    bi = raws.index(min(raws))
    L = live[(arm, seed, bite)]
    rows.append({'arm': arm, 'seed': seed, 'bite': bite, 'p': p, 'own': (arm == 'B') == (p == 1),
                 'horizon': r['horizon']['iterations'], 'ran': n, 'stop': r['stop'],
                 'evalTotal': r['evaluationsTotal'], 'evalPerIt': r['evaluationsTotal'] / n, 'evalSum': sum(ev),
                 'band': r['bandEnteredAtIteration'], 'evalToBand': r['evaluationsToBand'],
                 'certPublished': r['certification']['published'], 'certDepth': r['certification']['depthMm'],
                 'identityPass': r['identityPass'], 'rollbacks': r['rollbacks'],
                 'bestRaw': min(raws), 'bestRawIt': bi, 'bestRawFrac': bi / n, 'lastRaw': raws[-1], 'entryRaw': raws[0],
                 'liveIters': L['iterations'], 'liveEval': L['evalAll'], 'liveBand': L['bandEntries'], 'liveBestRaw': L['bestRaw'], 'liveBestRawIt': L['bestRawIt']})
json.dump(rows, open(os.path.join(DEEP, 'analysis', 'ends-replays.json'), 'w'), indent=1)
print(f'replays read: {len(rows)}')
own = [r for r in rows if r['own']]
print(f"own-objective replays: {len(own)}; identity pass = all iterations in {sum(r['identityPass']==r['ran'] for r in own)}; band iteration equals live in {sum((r['band'] or 0)==(L if (L:=live[(r['arm'],r['seed'],r['bite'])]['iterations']) and live[(r['arm'],r['seed'],r['bite'])]['bandEntries'] else 0) for r in own)}; evalTotal equals live evaluationsAllWorkers in {sum(r['evalTotal']==r['liveEval'] for r in own)}")

print('\n### Per capsule: both objectives under the live horizon (iterations, evaluations, band, certification)\n')
print('| arm | seed | bite | live iters / evals / band | p=1: ran / evals / evals-per-it / band@it (evals to band) / cert | p=2: ran / evals / evals-per-it / band@it (evals to band) / cert | p2/p1 evals-per-it | p=1 best raw @it (frac) | p=2 best raw @it (frac) |')
print('|---|---|---|---|---|---|---|---|---|')
caps = sorted(set((r['arm'], r['seed'], r['bite']) for r in rows), key=lambda k: (k[2], k[0], int(k[1])))
ratios = {'A': [], 'B': []}
for k in caps:
    p1 = next(r for r in rows if (r['arm'], r['seed'], r['bite']) == k and r['p'] == 1)
    p2 = next(r for r in rows if (r['arm'], r['seed'], r['bite']) == k and r['p'] == 2)
    ratios[k[0]].append(p2['evalPerIt'] / p1['evalPerIt'])
    def cell(r): return f"{r['ran']} / {r['evalTotal']} / {r['evalPerIt']:.0f} / {r['band']} ({r['evalToBand']}) / {r['certPublished']}"
    print(f"| {k[0]} | {k[1]} | {k[2]} | {p1['liveIters']} / {p1['liveEval']} / {p1['liveBand']} | {cell(p1)} | {cell(p2)} | {p2['evalPerIt']/p1['evalPerIt']:.2f} | {p1['bestRaw']:.3f} @{p1['bestRawIt']} ({p1['bestRawFrac']:.2f}) | {p2['bestRaw']:.3f} @{p2['bestRawIt']} ({p2['bestRawFrac']:.2f}) |")
print()
for arm in 'AB':
    print(f"capsules from arm {arm}: p2/p1 evaluations-per-iteration ratio min/med/max {min(ratios[arm]):.2f}/{med(ratios[arm]):.2f}/{max(ratios[arm]):.2f}")
print('\n### Band entries by capsule arm x replay objective (bite 5 capsules; horizon = the live attempt\'s iteration count)\n')
for bite in (5, 6):
    for arm in 'AB':
        for p in (1, 2):
            rs = [r for r in rows if r['arm'] == arm and r['bite'] == bite and r['p'] == p]
            if not rs: continue
            nb = [r for r in rs if r['band'] is not None]
            print(f"bite {bite} capsules of arm {arm} (n={len(rs)}) under p={p}: band entries {len(nb)} (certified published {sum(1 for r in rs if r['certPublished'])}); "
                  f"iterations run min/med/max {min(r['ran'] for r in rs)}/{med([r['ran'] for r in rs])}/{max(r['ran'] for r in rs)}; evals total med {med([r['evalTotal'] for r in rs]):.0f}; "
                  f"non-band best raw min/med/max {min((r['bestRaw'] for r in rs if r['band'] is None), default=None)}/{med([r['bestRaw'] for r in rs if r['band'] is None])}/{max((r['bestRaw'] for r in rs if r['band'] is None), default=None)}; "
                  f"non-band best-raw fraction med {med([r['bestRawFrac'] for r in rs if r['band'] is None])}; non-band with best raw in last 10% {sum(1 for r in rs if r['band'] is None and r['bestRawFrac']>=0.9)}; before middle {sum(1 for r in rs if r['band'] is None and r['bestRawFrac']<0.5)}")
            if nb: print('   band entries: ' + '; '.join(f"{r['seed']} @{r['band']} of {r['horizon']} ({r['evalToBand']} evals; live {r['liveIters']} it / {r['liveEval']} evals, live band {r['liveBand']})" for r in nb))
