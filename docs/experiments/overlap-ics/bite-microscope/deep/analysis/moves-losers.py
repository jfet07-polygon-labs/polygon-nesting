#!/usr/bin/env python3
"""What the discarded workers' states looked like: per attempt, the best
post-sweep max residual / raw any worker reached vs the best the tournament
retained (winners only); sweeps where a loser's max residual was below a
threshold while the winner's was not; the winner-ordinal distribution.
Reads the 42 deep documents read-only.  stdlib only."""
import json, glob, os, re, collections, statistics as st
DEEP = '/var/lib/t3/tmp/astra/deep'
THR = [0.050, 0.020, 0.004]  # mm: the summariser's 50 / 20 / 4 um marks
rows = []
ordinal = collections.Counter(); ordinalByArm = {'A': collections.Counter(), 'B': collections.Counter()}
for path in sorted(glob.glob(os.path.join(DEEP, 'deep-*-wall10s-s*.json'))):
    m = re.match(r'deep-([AB])-wall10s-s(\d+)\.json', os.path.basename(path))
    arm, seed = m.group(1), int(m.group(2))
    doc = json.load(open(path))
    for bite in doc['biteMicroscope']['bites']:
        for sep in bite['separations']:
            sweeps = sep['sweeps']
            bestWinMax = min(sw['workers'][sw['winner']]['maxAfterMm'] for sw in sweeps)
            bestAnyMax = min(w['maxAfterMm'] for sw in sweeps for w in sw['workers'])
            bestWinRaw = min(sw['workers'][sw['winner']]['rawAfter'] for sw in sweeps)
            bestAnyRaw = min(w['rawAfter'] for sw in sweeps for w in sw['workers'])
            bestLoserMax = min(w['maxAfterMm'] for sw in sweeps for i, w in enumerate(sw['workers']) if i != sw['winner'])
            bestLoserRaw = min(w['rawAfter'] for sw in sweeps for i, w in enumerate(sw['workers']) if i != sw['winner'])
            below = {t: dict(loserOnly=0, winner=0, any=0) for t in THR}
            for sw in sweeps:
                w = sw['winner']; W = sw['workers']
                ordinal[w] += 1; ordinalByArm[arm][w] += 1
                for t in THR:
                    wm = W[w]['maxAfterMm'] < t
                    lm = any(x['maxAfterMm'] < t for i, x in enumerate(W) if i != w)
                    if wm: below[t]['winner'] += 1
                    if lm and not wm: below[t]['loserOnly'] += 1
                    if wm or lm: below[t]['any'] += 1
            rows.append(dict(arm=arm, seed=seed, bite=bite['ordinal'], published=sep['stop'] == 'published',
                             sweeps=len(sweeps), minRaw=sep['minRaw'],
                             bestWinMax=bestWinMax, bestLoserMax=bestLoserMax, bestAnyMax=bestAnyMax,
                             bestWinRaw=bestWinRaw, bestLoserRaw=bestLoserRaw, bestAnyRaw=bestAnyRaw,
                             below={str(t): v for t, v in below.items()}))
json.dump(rows, open(os.path.join(DEEP, 'analysis', 'moves-losers.json'), 'w'), indent=1)
print('| arm | seed | bite | pub | sweeps | best winner max (mm) | best loser max (mm) | best loser max < best winner max | best winner raw | best loser raw | sweeps winner max<50um | loser-only <50um | winner <20um | loser-only <20um | winner <4um | loser-only <4um |')
print('|---|---|---|---|---:|---:|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|')
for r in rows:
    b = r['below']
    print(f"| {r['arm']} | {r['seed']} | {r['bite']} | {'P' if r['published'] else 'F'} | {r['sweeps']} | {r['bestWinMax']:.4f} | {r['bestLoserMax']:.4f} | {'yes' if r['bestLoserMax'] < r['bestWinMax'] else 'no'} | {r['bestWinRaw']:.3f} | {r['bestLoserRaw']:.3f} | "
          f"{b['0.05']['winner']} | {b['0.05']['loserOnly']} | {b['0.02']['winner']} | {b['0.02']['loserOnly']} | {b['0.004']['winner']} | {b['0.004']['loserOnly']} |")
print()
def grp(r): return (r['arm'], 'b6' if r['bite'] == 6 else ('b5-pub' if r['published'] else 'b5-fail'))
G = collections.defaultdict(list)
for r in rows: G[grp(r)].append(r)
print('# per group: attempts where the best loser max over the attempt < best winner max; median best winner max, median best loser max, median ratio loser/winner; sums of loser-only sub-threshold sweeps')
for k in sorted(G):
    g = G[k]
    n = sum(1 for r in g if r['bestLoserMax'] < r['bestWinMax'])
    print(f"  {k}: n={len(g)}, bestLoserMax<bestWinMax on {n}; median bestWinMax {st.median([r['bestWinMax'] for r in g]):.4f} mm, median bestLoserMax {st.median([r['bestLoserMax'] for r in g]):.4f} mm, median ratio {st.median([r['bestLoserMax']/r['bestWinMax'] for r in g if r['bestWinMax'] > 0]) if any(r['bestWinMax'] > 0 for r in g) else float('nan'):.3f} (attempts with winner max 0 excluded); "
          f"median bestWinRaw {st.median([r['bestWinRaw'] for r in g]):.3f}, median bestLoserRaw {st.median([r['bestLoserRaw'] for r in g]):.3f}; "
          f"loser-only <50um sweeps {sum(r['below']['0.05']['loserOnly'] for r in g)}, <20um {sum(r['below']['0.02']['loserOnly'] for r in g)}, <4um {sum(r['below']['0.004']['loserOnly'] for r in g)}; winner <50um sweeps {sum(r['below']['0.05']['winner'] for r in g)}")
print()
print('# failed attempts only: the best max residual any worker (winner or loser) ever reached, min / median / max over attempts (mm)')
for arm in 'AB':
    g = [r for r in rows if r['arm'] == arm and not r['published']]
    xs = [r['bestAnyMax'] for r in g]
    print(f"  {arm}: n={len(g)} min {min(xs):.4f} median {st.median(xs):.4f} max {max(xs):.4f}; winner-only best: min {min(r['bestWinMax'] for r in g):.4f} median {st.median([r['bestWinMax'] for r in g]):.4f}")
print()
tot = sum(ordinal.values())
print('# winner ordinal distribution over all sweeps (ties to the lowest ordinal): ', {k: f'{100*v/tot:.1f}%' for k, v in sorted(ordinal.items())}, 'n =', tot)
for arm in 'AB':
    t = sum(ordinalByArm[arm].values())
    print(f'  {arm}:', {k: f'{100*v/t:.1f}%' for k, v in sorted(ordinalByArm[arm].items())}, 'n =', t)
