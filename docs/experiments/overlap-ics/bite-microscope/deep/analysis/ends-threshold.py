#!/usr/bin/env python3
"""'Insufficient remaining work' test. For every retained attempt: the iteration at which the running
minimum of raw Φ first falls below 10 / 5 / 1 / 0.1 mm² and of max violation below 0.5 / 0.1 mm; for
band entries the iterations and evaluations from that crossing to the band; for deadline stops the
iterations and evaluations that remained after the crossing. Also: improvement gained in the last
10 % / 20 % of iterations, the microscope-vs-v2 agreement, the winner's move magnitudes around the
early raw jump, and the near-zero replay raws. Usage: python3 ends-threshold.py"""
import json, glob, os, re, statistics as st
DEEP = '/var/lib/t3/tmp/astra/deep'
def med(xs): xs = [x for x in xs if x is not None]; return st.median(xs) if xs else None
RAW_T = [10, 5, 1, 0.1]; MAX_T = [0.5, 0.1]
att = []
for f in sorted(glob.glob(os.path.join(DEEP, 'deep-*-wall10s-s*.json'))):
    d = json.load(open(f)); arm = os.path.basename(f)[5]; seed = str(d['seed'])
    for b in d['biteMicroscope']['bites']:
        for s in b['separations']:
            sm = s['samples']; sw = s['sweeps']; n = s['iterations']
            raws = [x[1] for x in sm]; mxs = [x[3] for x in sm]
            cum = [0]
            for x in sw: cum.append(cum[-1] + x['evaluationsAllWorkers'])
            band_it = next((x[0] for x in sm if x[5]), None)
            def cross(series, t):
                m = float('inf')
                for i, v in enumerate(series):
                    m = min(m, v)
                    if m < t: return i
                return None
            r = {'arm': arm, 'seed': seed, 'bite': b['ordinal'], 'n': n, 'evals': cum[-1], 'stop': s['stop'], 'band': band_it,
                 'bestRaw': min(raws), 'bestRawIt': raws.index(min(raws)), 'bestMax': min(mxs)}
            for t in RAW_T:
                c = cross(raws, t); r[f'raw<{t}'] = c
                r[f'raw<{t}_itAfter'] = (n - c) if c is not None else None
                r[f'raw<{t}_evAfter'] = (cum[-1] - cum[c]) if c is not None else None
            for t in MAX_T:
                c = cross(mxs, t); r[f'max<{t}'] = c
                r[f'max<{t}_itAfter'] = (n - c) if c is not None else None
                r[f'max<{t}_evAfter'] = (cum[-1] - cum[c]) if c is not None else None
            # improvement gained in the last 10 % / 20 %
            for frac, lab in ((0.1, 'last10'), (0.2, 'last20')):
                k = n - max(1, int(n * frac))
                before = min(raws[:k + 1]); after = min(raws)
                r[lab + '_relGain'] = (before - after) / before if before > 0 else 0.0
                r[lab + '_absGain'] = before - after
            # winner's move magnitudes for iterations 1..14 and the jump iteration (first raw > 2x running-min)
            runmin = raws[0]; jump = None
            for i, v in enumerate(raws):
                runmin = min(runmin, v)
                if jump is None and i > 0 and v > 2 * runmin: jump = i
            r['jump'] = jump
            def mag(it):
                rel = sw[it - 1]['relocates']
                if not rel: return (0, 0.0, 0.0)
                return (sum(1 for x in rel if x['moved']), max(abs(x['dxMm']) + abs(x['dyMm']) for x in rel), max(abs(x['dthetaDeg']) for x in rel))
            r['magBefore'] = [mag(i) for i in range(1, jump)] if jump else []
            r['magJump'] = mag(jump) if jump and jump <= n else None
            r['magAfter'] = [mag(i) for i in range(jump + 1, min(jump + 4, n + 1))] if jump else []
            r['workersAtJump'] = [(w['worker'], round(w['rawAfter'], 1), round(w['maxAfterMm'], 2), round(w['guidedAfter'], 1)) for w in sw[jump - 1]['workers']] if jump and jump <= n else None
            r['winnerAtJump'] = sw[jump - 1]['winner'] if jump and jump <= n else None
            att.append(r)
json.dump(att, open(os.path.join(DEEP, 'analysis', 'ends-threshold.json'), 'w'), indent=1)

print('### Band entries: iterations / evaluations from the threshold crossing to band entry\n')
print('| arm | seed | bite | band @it | evals to band | raw<10 @it (it/ev to band) | raw<5 | raw<1 | raw<0.1 | max<0.5 mm | max<0.1 mm |')
print('|---|---|---|---|---|---|---|---|---|---|---|')
for r in att:
    if r['band'] is None: continue
    def c(t, kind='raw'):
        k = f'{kind}<{t}'; i = r[k]
        return f"{i} ({r['band']-i} / {r[k+'_evAfter']})" if i is not None else '-'
    print(f"| {r['arm']} | {r['seed']} | {r['bite']} | {r['band']} | {r['evals']} | {c(10)} | {c(5)} | {c(1)} | {c(0.1)} | {c(0.5,'max')} | {c(0.1,'max')} |")
succ = [r for r in att if r['band'] is not None]
for t in RAW_T:
    xs = [r['band'] - r[f'raw<{t}'] for r in succ if r[f'raw<{t}'] is not None]
    ev = [r[f'raw<{t}_evAfter'] for r in succ if r[f'raw<{t}'] is not None]
    print(f"successes (n={len(succ)}): iterations from raw<{t} to band min/med/max {min(xs)}/{med(xs)}/{max(xs)}; evaluations min/med/max {min(ev)}/{med(ev):.0f}/{max(ev)}")
for t in MAX_T:
    xs = [r['band'] - r[f'max<{t}'] for r in succ if r[f'max<{t}'] is not None]
    print(f"successes: iterations from max<{t} mm to band min/med/max {min(xs)}/{med(xs)}/{max(xs)} (n crossing {len(xs)})")

print('\n### Deadline stops: thresholds crossed and the work that remained after the crossing\n')
print('| arm | seed | bite | iters | best raw @it | best max mm | raw<10 @it (it / ev left) | raw<5 | raw<1 | raw<0.1 | max<0.5 mm | max<0.1 mm | rel gain last 10% | rel gain last 20% |')
print('|---|---|---|---|---|---|---|---|---|---|---|---|---|---|')
for r in sorted(att, key=lambda r: (r['bite'], r['arm'], r['bestRaw'])):
    if r['band'] is not None: continue
    def c(t, kind='raw'):
        k = f'{kind}<{t}'; i = r[k]
        return f"{i} ({r[k+'_itAfter']} / {r[k+'_evAfter']})" if i is not None else '-'
    print(f"| {r['arm']} | {r['seed']} | {r['bite']} | {r['n']} | {r['bestRaw']:.3f} @{r['bestRawIt']} | {r['bestMax']:.4f} | {c(10)} | {c(5)} | {c(1)} | {c(0.1)} | {c(0.5,'max')} | {c(0.1,'max')} | {r['last10_relGain']*100:.1f}% | {r['last20_relGain']*100:.1f}% |")
fails = [r for r in att if r['band'] is None and r['bite'] == 5]
for arm in 'AB':
    fs = [r for r in fails if r['arm'] == arm]
    print(f"\narm {arm} deadline-stopped bite 5 (n={len(fs)}): crossed raw<10 {sum(r['raw<10'] is not None for r in fs)}, raw<5 {sum(r['raw<5'] is not None for r in fs)}, raw<1 {sum(r['raw<1'] is not None for r in fs)}, raw<0.1 {sum(r['raw<0.1'] is not None for r in fs)}; max<0.5 mm {sum(r['max<0.5'] is not None for r in fs)}, max<0.1 mm {sum(r['max<0.1'] is not None for r in fs)}")
    for t in (10, 5, 1):
        need = max(x['band'] - x[f'raw<{t}'] for x in succ if x[f'raw<{t}'] is not None)
        needev = max(x[f'raw<{t}_evAfter'] for x in succ if x[f'raw<{t}'] is not None)
        got = [(r['seed'], r[f'raw<{t}_itAfter'], r[f'raw<{t}_evAfter']) for r in fs if r[f'raw<{t}'] is not None]
        print(f"   raw<{t}: successes needed at most {need} iterations / {needev} evaluations after crossing; failures that crossed had left (seed, iterations, evaluations): {got}; "
              f"fewer iterations than the slowest success {sum(1 for _, i, _ in got if i < need)} of {len(got)}; fewer evaluations {sum(1 for _, _, e in got if e < needev)} of {len(got)}")
    print(f"   relative gain of the running-min raw in the last 10 %: >0 in {sum(r['last10_relGain']>0 for r in fs)} of {len(fs)}, >10 % in {sum(r['last10_relGain']>0.1 for r in fs)}, >50 % in {sum(r['last10_relGain']>0.5 for r in fs)}; "
          f"last 20 %: >0 in {sum(r['last20_relGain']>0 for r in fs)}, >10 % in {sum(r['last20_relGain']>0.1 for r in fs)}, >50 % in {sum(r['last20_relGain']>0.5 for r in fs)}")

print('\n### The early raw jump (first iteration with raw > 2x the running minimum): winner\'s moves before / at / after\n')
print('(moved pieces, max |dx|+|dy| mm, max |dtheta| deg) per iteration; workers at the jump iteration: (worker, raw, max mm, guided)')
for r in att:
    if r['bite'] != 5: continue
    print(f"{r['arm']} {r['seed']}: jump at it {r['jump']}; before {[(m, round(a,2), round(t,2)) for m,a,t in r['magBefore']]}; at {(r['magJump'][0], round(r['magJump'][1],2), round(r['magJump'][2],2)) if r['magJump'] else None}; after {[(m, round(a,2), round(t,2)) for m,a,t in r['magAfter']]}; winner {r['winnerAtJump']}; workers {r['workersAtJump']}")
b5 = [r for r in att if r['bite'] == 5]
print(f"\njump iteration over the 42 trigger attempts: min/med/max {min(r['jump'] for r in b5)}/{med([r['jump'] for r in b5])}/{max(r['jump'] for r in b5)}; A {sorted(r['jump'] for r in b5 if r['arm']=='A')}; B {sorted(r['jump'] for r in b5 if r['arm']=='B')}")
for arm in 'AB':
    rs = [r for r in b5 if r['arm'] == arm and r['magJump']]
    print(f"arm {arm}: max displacement (|dx|+|dy| mm) med over attempts: before jump {med([med([a for _,a,_ in r['magBefore']]) for r in rs]):.3f}, at jump {med([r['magJump'][1] for r in rs]):.3f}, after {med([med([a for _,a,_ in r['magAfter']]) for r in rs if r['magAfter']]):.3f}; "
          f"max |dtheta| deg med: before {med([med([t for _,_,t in r['magBefore']]) for r in rs]):.3f}, at jump {med([r['magJump'][2] for r in rs]):.3f}; moved pieces med: before {med([med([m for m,_,_ in r['magBefore']]) for r in rs])}, at jump {med([r['magJump'][0] for r in rs])}")
    # at the jump, was every worker's raw above 2x the running min (i.e. no worker offered a non-jumping state)?
    allup = 0; anylow = 0
    for r in rs:
        runmin = min(x for x in [json.load(open(os.path.join(DEEP, f"deep-{arm}-wall10s-s{r['seed']}.json")))['biteMicroscope']['bites'][0]['separations'][0]['samples'][i][1] for i in range(r['jump'])])
        ws = r['workersAtJump']
        if all(w[1] > 2 * runmin for w in ws): allup += 1
        else: anylow += 1
    print(f"   at the jump iteration every worker's raw exceeded 2x the running minimum in {allup} of {len(rs)} attempts; at least one worker stayed below in {anylow}")

print('\n### microscope run vs the scored v2 cells (twelve 64-bit seeds x 2 arms)\n')
v2 = json.load(open('/var/lib/t3/tmp/astra/v2/fifth-cut-rows.json'))
agree = 0; tot = 0; itdiff = []
for r in b5:
    reps = [x for x in v2 if x['arm'] == r['arm'] and str(x['seed']) == r['seed']]
    if not reps: continue
    tot += 1
    pub = (r['band'] is not None)
    if all(bool(x['fifth']) == pub for x in reps): agree += 1
    itdiff.append(max(abs(x['iters'] - r['n']) / r['n'] for x in reps))
print(f"arm-seed pairs with scored cells: {tot}; fifth-cut published/failed agrees with all three reps in {agree}; max relative iteration difference to any rep: med {med(itdiff)*100:.1f}%, max {max(itdiff)*100:.1f}%")

print('\n### replays whose best raw is below 1 mm² without band entry (6 decimals)\n')
for f in sorted(glob.glob(os.path.join(DEEP, 'replays', 'rp-*.json'))):
    rp = json.load(open(f))['replay']
    raws = [rp['entryRaw']] + [x['rawAfter'] for x in rp['iterations']]; mxs = [rp['entryMaxMm']] + [x['maxAfterMm'] for x in rp['iterations']]
    if rp['bandEnteredAtIteration'] is None and min(raws) < 1:
        i = raws.index(min(raws))
        print(f"{os.path.basename(f)}: ran {len(rp['iterations'])} of horizon {rp['horizon']['iterations']}, stop {rp['stop']}, best raw {min(raws):.6f} @{i}, max there {mxs[i]:.6f} mm, best max {min(mxs):.6f} mm @{mxs.index(min(mxs))}, evaluations {rp['evaluationsTotal']}")
