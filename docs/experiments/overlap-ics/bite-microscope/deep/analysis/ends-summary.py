#!/usr/bin/env python3
"""Compact classification of the 42 trigger attempts with explicit criteria, and the work spent after
the best-raw iteration. Reads analysis/ends-attempts.json and analysis/ends-threshold.json (written by
ends-attempts.py and ends-threshold.py) plus the live documents for cumulative evaluations.
Usage: python3 ends-summary.py"""
import json, os, glob, statistics as st
DEEP = '/var/lib/t3/tmp/astra/deep'
def med(xs): return st.median(xs) if xs else None
A = json.load(open(os.path.join(DEEP, 'analysis', 'ends-attempts.json')))
T = {(r['arm'], r['seed'], r['bite']): r for r in json.load(open(os.path.join(DEEP, 'analysis', 'ends-threshold.json')))}
# cumulative evaluations per attempt
cum = {}
for f in sorted(glob.glob(os.path.join(DEEP, 'deep-*-wall10s-s*.json'))):
    d = json.load(open(f)); arm = os.path.basename(f)[5]; seed = str(d['seed'])
    for b in d['biteMicroscope']['bites']:
        c = [0]
        for x in b['separations'][0]['sweeps']: c.append(c[-1] + x['evaluationsAllWorkers'])
        cum[(arm, seed, b['ordinal'])] = c
# successes' transition budget (raw<1 -> band), from ends-threshold.json
succ = [t for t in T.values() if t['band'] is not None]
need_it = {th: (med([t['band'] - t[f'raw<{th}'] for t in succ]), max(t['band'] - t[f'raw<{th}'] for t in succ)) for th in (10, 1)}
need_ev = {th: (med([t[f'raw<{th}_evAfter'] for t in succ]), max(t[f'raw<{th}_evAfter'] for t in succ)) for th in (10, 1)}
print(f"successes (n={len(succ)}): iterations from raw<10 to band median/max {need_it[10]}, evaluations {need_ev[10]}; from raw<1: iterations {need_it[1]}, evaluations {need_ev[1]}\n")
print('| arm | seed | bite | stop | iters | evals | best raw @it (frac) | work after best: iters / evals | last new min -> stop (iters) | best raw in last 10 % | crossed raw<1 (iters / evals left) | fewer than slowest success needed after raw<1 | last raw / best | class |')
print('|---|---|---|---|---|---|---|---|---|---|---|---|---|---|')
tot = {}
for r in sorted(A, key=lambda r: (r['bite'], r['arm'], r['bestRawFrac'])):
    k = (r['arm'], r['seed'], r['bite']); t = T[k]; c = cum[k]; n = r['iterations']
    after_it = n - r['bestRawIt']; after_ev = c[-1] - c[r['bestRawIt']]
    c1 = t['raw<1']; left = (t['raw<1_itAfter'], t['raw<1_evAfter']) if c1 is not None else None
    cut = (left is not None and left[0] < need_it[1][1] and left[1] < need_ev[1][1])
    if r['bandEntries'] > 0: cls = 'band'
    elif r['bestRawFrac'] >= 0.9: cls = 'improving at deadline (best raw in last 10 %)'
    elif r['bestRawFrac'] < 0.5: cls = 'plateau (best raw before the middle)'
    else: cls = 'best raw in [50 %, 90 %)'
    if r['stop'] == 'deadline':
        g = tot.setdefault((r['arm'], r['bite']), {'n': 0, 'afterIt': 0, 'afterEv': 0, 'it': 0, 'ev': 0, 'cut': 0, 'last10': 0, 'plateau': 0, 'mid': 0, 'degrade': 0, 'early': 0})
        g['n'] += 1; g['afterIt'] += after_it; g['afterEv'] += after_ev; g['it'] += n; g['ev'] += c[-1]; g['cut'] += cut
        g['last10'] += r['bestRawFrac'] >= 0.9; g['plateau'] += r['bestRawFrac'] < 0.5; g['mid'] += 0.5 <= r['bestRawFrac'] < 0.9
        g['degrade'] += r['lastRaw'] > 2 * r['bestRaw']; g['early'] += r['bestRawIt'] <= 11
    print(f"| {r['arm']} | {r['seed']} | {r['bite']} | {r['stop']} | {n} | {c[-1]} | {r['bestRaw']:.3f} @{r['bestRawIt']} ({r['bestRawFrac']:.2f}) | {after_it} / {after_ev} | {n - r['lastNewMinIt']} | {'yes' if r['bestRawFrac']>=0.9 else 'no'} | {f'{c1} ({left[0]} / {left[1]})' if c1 is not None else '-'} | {'yes' if cut else ('no' if c1 is not None else '-')} | {r['lastRaw']/r['bestRaw'] if r['bestRaw']>0 else 0:.1f} | {cls} |")
print()
for k, g in sorted(tot.items(), key=lambda kv: (kv[0][1], kv[0][0])):
    print(f"arm {k[0]} bite {k[1]} deadline stops n={g['n']}: iterations {g['it']} of which after the best-raw iteration {g['afterIt']} ({g['afterIt']/g['it']*100:.1f} %); evaluations {g['ev']} of which after the best {g['afterEv']} ({g['afterEv']/g['ev']*100:.1f} %); "
          f"best raw at iteration <= 11: {g['early']}; best in last 10 %: {g['last10']}; before middle: {g['plateau']}; in [50,90) %: {g['mid']}; degrade (last raw > 2x best): {g['degrade']}; crossed raw<1 with less work left than the slowest success needed: {g['cut']}")
