#!/usr/bin/env python3
"""The 180 scored v2 cells' fifth cut (no microscope): iterations, evaluations per iteration per arm,
allowance, outcome; and the cross-check of the microscope run's bite-5 outcome against the scored
cells on the twelve 64-bit seeds. Reads /var/lib/t3/tmp/astra/v2/fifth-cut-rows.json and the
microscope rows. Usage: python3 ends-v2.py"""
import json, os, statistics as st
V2 = '/var/lib/t3/tmp/astra/v2/fifth-cut-rows.json'
def med(xs): return st.median(xs) if xs else None
rows = json.load(open(V2)); print('cells', len(rows), 'keys', sorted(rows[0].keys()))
for arm in 'AB':
    rs = [r for r in rows if r['arm'] == arm]
    pub = [r for r in rs if r['fifth']]; fail = [r for r in rs if not r['fifth']]
    print(f"arm {arm}: n={len(rs)}, fifth published {len(pub)}, band reached {sum(r['bandReached'] for r in rs)}, exact>0 {sum(r['exact']>0 for r in rs)}")
    for lab, g in (('published', pub), ('failed', fail)):
        if not g: continue
        epi = [r['ev']/r['iters'] for r in g]
        print(f"   {lab} n={len(g)}: iters min/med/max {min(r['iters'] for r in g)}/{med([r['iters'] for r in g])}/{max(r['iters'] for r in g)}; ev med {med([r['ev'] for r in g]):.0f}; "
              f"ev/iter min/med/max {min(epi):.0f}/{med(epi):.0f}/{max(epi):.0f}; parent published at (ptime) med {med([r['ptime'] for r in g]):.2f} s; left med {med([r['left'] for r in g]):.2f} s; "
              f"ev per left-second med {med([r['ev']/r['left'] for r in g]):.0f}; iters per left-second med {med([r['iters']/r['left'] for r in g]):.1f}; minraw med {med([r['minraw'] for r in g]):.3f}; strikes>0 {sum(r['strikes']>0 for r in g)}; attempts>1 {sum(r['attempts']>1 for r in g)}")
A = [r['ev']/r['iters'] for r in rows if r['arm']=='A']; B = [r['ev']/r['iters'] for r in rows if r['arm']=='B']
print(f"ev/iter medians A {med(A):.0f} B {med(B):.0f} ratio {med(A)/med(B):.2f}")
mic = json.load(open('/var/lib/t3/tmp/astra/deep/analysis/ends-attempts.json'))
print('\ncross-check, twelve 64-bit seeds: scored v2 fifth-cut published per rep vs the microscope run')
for arm in 'AB':
    for seed in sorted(set(r['seed'] for r in rows), key=int):
        v = sorted((r['rep'], r['fifth'], r['iters']) for r in rows if r['arm']==arm and r['seed']==int(seed))
        m = next((r for r in mic if r['arm']==arm and r['seed']==str(seed) and r['bite']==5), None)
        if m is None: continue
        print(f"  {arm} {seed}: v2 reps {[(rep, int(f), it) for rep,f,it in v]} | microscope published {m['published']} iters {m['iterations']} bestRaw {m['bestRaw']:.3f}")
