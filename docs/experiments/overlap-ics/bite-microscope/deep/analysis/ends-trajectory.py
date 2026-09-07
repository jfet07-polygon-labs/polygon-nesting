#!/usr/bin/env python3
"""Trajectory shape of every retained attempt: raw / guided / max at checkpoint iterations, running
minimum, per-segment (between rollbacks) behaviour, and the winner-selection rule check.
Reads only the 42 deep-*-wall10s-s*.json documents. Usage: python3 ends-trajectory.py"""
import json, glob, os, statistics as st
DEEP = '/var/lib/t3/tmp/astra/deep'
CK = [0, 1, 2, 5, 10, 20, 50, 100, 200, 300, 400, 600, 800]

def med(xs): return st.median(xs) if xs else None

win_g = win_r = win_m = win_n = 0
segrows = []
print('### Raw Φ (mm²) at checkpoint iterations; G = guided Φ, M = max violation (mm); running-min raw in brackets\n')
print('| arm | seed | bite | iters | ' + ' | '.join(f'it{c}' for c in CK) + ' | best raw @it | best guided @it | best max @it | last raw / guided / max |')
print('|' + '---|' * (5 + len(CK) + 4))
for f in sorted(glob.glob(os.path.join(DEEP, 'deep-*-wall10s-s*.json'))):
    d = json.load(open(f)); arm = os.path.basename(f)[5]; seed = str(d['seed'])
    for b in d['biteMicroscope']['bites']:
        for s in b['separations']:
            sm = s['samples']; sw = s['sweeps']; n = s['iterations']
            raws = [x[1] for x in sm]; gu = [x[2] for x in sm]; mx = [x[3] for x in sm]
            # winner-selection rule: which worker statistic the winner minimises
            for x in sw:
                ws = x['workers']
                if not ws: continue
                win_n += 1
                w = next(ww for ww in ws if ww['worker'] == x['winner'])
                if w['guidedAfter'] <= min(ww['guidedAfter'] for ww in ws): win_g += 1
                if w['rawAfter'] <= min(ww['rawAfter'] for ww in ws): win_r += 1
                if w['maxAfterMm'] <= min(ww['maxAfterMm'] for ww in ws): win_m += 1
            cells = []
            for c in CK:
                if c <= n: cells.append(f'{raws[c]:.1f} [{min(raws[:c+1]):.1f}]')
                else: cells.append('-')
            bi = raws.index(min(raws)); gi = gu.index(min(gu)); mi = mx.index(min(mx))
            print(f"| {arm} | {seed} | {b['ordinal']} | {n} | " + ' | '.join(cells) + f" | {min(raws):.3f} @{bi} | {min(gu):.3f} @{gi} | {min(mx):.4f} @{mi} | {raws[-1]:.1f} / {gu[-1]:.1f} / {mx[-1]:.2f} |")
            # segments between rollbacks: [start, end] with start = iteration after the rollback (or 0)
            bounds = [0] + [at for at, to in s['rollbacks']] + [n]
            restored = [None] + [to for at, to in s['rollbacks']]
            for k in range(len(bounds) - 1):
                a, e = bounds[k], bounds[k + 1]
                base = raws[restored[k]] if restored[k] is not None else raws[0]
                seg = raws[a + 1:e + 1] if k == 0 else raws[a + 1:e + 1]
                if not seg: continue
                bmin = min(seg); bit = a + 1 + seg.index(bmin)
                segrows.append({'arm': arm, 'seed': seed, 'bite': b['ordinal'], 'seg': k, 'from': a, 'to': e, 'len': e - a,
                                'base': base, 'segBest': bmin, 'segBestAt': bit, 'improved': bmin < base,
                                'final': k == len(bounds) - 2, 'stop': s['stop'],
                                'newMinInSeg': sum(1 for x in sm[a + 1:e + 1] if x[4])})
print(f'\nwinner-selection check over {win_n} contested sweeps: winner has the minimum guided Φ among workers in {win_g}, minimum raw Φ in {win_r}, minimum max violation in {win_m}')

print('\n### Segments between rollbacks (a segment = the iterations after a rollback or the start, up to the next rollback or the stop)\n')
for arm in 'AB':
    rs = [r for r in segrows if r['arm'] == arm and r['bite'] == 5]
    fin = [r for r in rs if r['final'] and r['stop'] == 'deadline']
    print(f"arm {arm} bite 5: segments {len(rs)}; improved on their base state {sum(r['improved'] for r in rs)}; "
          f"final segments of deadline stops {len(fin)}: improved on the restored state {sum(r['improved'] for r in fin)}, "
          f"length min/med/max {min(r['len'] for r in fin)}/{med([r['len'] for r in fin])}/{max(r['len'] for r in fin)}, "
          f"iterations from the segment's best to the stop min/med/max {min(r['to']-r['segBestAt'] for r in fin)}/{med([r['to']-r['segBestAt'] for r in fin])}/{max(r['to']-r['segBestAt'] for r in fin)}")
    print('  final segments of deadline stops (seed: from->to, base raw, seg best @it, improved, new minima in segment, iterations since last minimum):')
    for r in sorted(fin, key=lambda r: r['to'] - r['segBestAt']):
        print(f"    {r['seed']}: {r['from']}->{r['to']} (len {r['len']}), base {r['base']:.3f}, seg best {r['segBest']:.3f} @{r['segBestAt']}, improved {r['improved']}, newmins {r['newMinInSeg']}, since-last-min {r['to']-r['segBestAt'] if r['improved'] else r['len']}")
    rs6 = [r for r in segrows if r['arm'] == arm and r['bite'] == 6]
    if rs6:
        fin6 = [r for r in rs6 if r['final']]
        print(f"  bite 6: segments {len(rs6)}, improved {sum(r['improved'] for r in rs6)}; final segments {len(fin6)} improved {sum(r['improved'] for r in fin6)}: " +
              '; '.join(f"{r['seed']} {r['from']}->{r['to']} base {r['base']:.1f} best {r['segBest']:.1f}@{r['segBestAt']} since-last-min {r['to']-r['segBestAt'] if r['improved'] else r['len']}" for r in fin6))

print('\n### The first ten iterations: raw at iteration 1..10 (winner state) and the iteration where raw first exceeds the running min by 2x\n')
for f in sorted(glob.glob(os.path.join(DEEP, 'deep-*-wall10s-s*.json'))):
    d = json.load(open(f)); arm = os.path.basename(f)[5]; seed = str(d['seed'])
    for b in d['biteMicroscope']['bites']:
        if b['ordinal'] != 5: continue
        for s in b['separations']:
            raws = [x[1] for x in s['samples']]; mx = [x[3] for x in s['samples']]
            runmin = raws[0]; blow = None
            for i, v in enumerate(raws):
                runmin = min(runmin, v)
                if blow is None and v > 2 * runmin and i > 0: blow = i
            print(f"{arm} {seed}: raw it1..10 " + ' '.join(f'{v:.1f}' for v in raws[1:11]) + f" | max it1..10 " + ' '.join(f'{v:.2f}' for v in mx[1:11]) + f" | first raw > 2x running-min at it {blow}")
