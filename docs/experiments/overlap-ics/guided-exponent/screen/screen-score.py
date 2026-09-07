#!/usr/bin/env python3
"""Score a development screen against Astra's six gates (review 4 Q4, review 5 Q3, review 5b Q7).

    screen-score.py <outdir> <prefix> [seeds] [profiles]

Per-seed repetition medians of the published depth; paired gain = A - B (positive = treatment
better); wins > 0.001 mm; per-seed-median regression = B - A > 1.000 mm on any seed fails the
tail gate; work = sample evaluations per explore bite and explore bites per cell; invalid
publications and refusals of the tripwires.
"""
import json, glob, sys, statistics as st, os
O, P = sys.argv[1], sys.argv[2]
seeds = [int(x) for x in sys.argv[3].split()] if len(sys.argv) > 3 else list(range(18, 27))
profs = sys.argv[4].split() if len(sys.argv) > 4 else ['legacy', 'wall10s']
def load(arm, prof):
    out = {}
    for s in seeds:
        cells = []
        for f in sorted(glob.glob(f'{O}/{P}-{arm}-{prof}-r*-s{s}.json')):
            try: d = json.load(open(f))
            except Exception: continue
            if 'startedFrom' in d or 'biteMicroscope' in d or 'replay' in d: raise SystemExit(f'REFUSED: {f} carries a diagnostic tripwire')
            o = d['outcome']; b = [x for x in o['bites'] if x['phase'] == 'explore']
            ev = o['relocateEconomics']['sampleEvaluations'] if isinstance(o.get('relocateEconomics'), dict) else o['work']['sampleEvaluations']
            cells.append(dict(depth=o['depthMm'], invalid=o.get('invalidPublications', 0), bites=len(b), published=sum(1 for x in b if x['published']),
                              evals=ev, iters=sum(x['masterIterations'] for x in o['bites']), pubs=o.get('publicationCount', len(o['publications'])),
                              giveups=sum(1 for c in o['exactCheckpoints'] if c.get('refusal') and 'band' in c['refusal'])))
        out[s] = cells
    return out
overall_pass = True
for prof in profs:
    A = load('A', prof); B = load('B', prof)
    n = sum(len(v) for v in A.values()); m = sum(len(v) for v in B.values())
    print(f'== {P} {prof}: A cells {n}, B cells {m}')
    ma = {s: st.median(c['depth'] for c in A[s]) for s in seeds if A[s]}; mb = {s: st.median(c['depth'] for c in B[s]) for s in seeds if B[s]}
    common = [s for s in seeds if s in ma and s in mb]
    g = [ma[s] - mb[s] for s in common]; reg = [mb[s] - ma[s] for s in common]
    def agg(cells, k): xs = [c[k] for s in seeds for c in cells[s]]; return sum(xs), (st.mean(xs) if xs else 0)
    evA, evB = agg(A, 'evals')[0], agg(B, 'evals')[0]; biA, biB = agg(A, 'bites')[0], agg(B, 'bites')[0]
    print(f'   depth: A median-of-medians {st.median(ma.values()):.3f} mean {st.mean(ma.values()):.3f} | B {st.median(mb.values()):.3f} mean {st.mean(mb.values()):.3f}')
    print(f'   paired gain (A-B) median {st.median(g):+.3f} mm, B wins {sum(1 for x in g if x > 0.001)}/{len(g)}, worst {min(g):+.3f}, per seed: ' + ' '.join(f'{s}:{x:+.2f}' for s, x in zip(common, g)))
    print(f'   evaluations per explore bite: A {evA/max(biA,1):.0f}  B {evB/max(biB,1):.0f}  ({100*(evB/max(biB,1))/(evA/max(biA,1))-100:+.0f} %); explore bites per cell: A {biA/max(n,1):.1f}  B {biB/max(m,1):.1f}; master iterations per cell: A {agg(A,"iters")[1]:.0f} B {agg(B,"iters")[1]:.0f}')
    print(f'   invalid publications: A {agg(A,"invalid")[0]} B {agg(B,"invalid")[0]}; give-ups: A {agg(A,"giveups")[0]} B {agg(B,"giveups")[0]}; publications per cell: A {agg(A,"pubs")[1]:.1f} B {agg(B,"pubs")[1]:.1f}')
    gates = {
        'depth: positive paired median': st.median(g) > 0,
        'tail: no per-seed-median regression > 1.000 mm': max(reg) <= 1.000,
        'integrity: zero invalid publications in B': agg(B, 'invalid')[0] == 0,
        'engineering: fewer evaluations per explore bite': (evB/max(biB,1)) < (evA/max(biA,1)),
        'engineering: explore bites per cell not lower': (biB/max(m,1)) >= (biA/max(n,1)),
    }
    for k, v in gates.items(): print(f'   [{"PASS" if v else "FAIL"}] {k}'); overall_pass &= v
print('SCREEN', 'PASS' if overall_pass else 'FAIL', '(mechanism and halving gates are scored from the traces, not here)')
