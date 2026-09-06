#!/usr/bin/env python3
"""Give-ups and exact checkpoints per publication, A vs B.

    giveups.py <outdir> <labelA> <labelB> [profile] [seeds]

Counts, per cell and in total, the exact checkpoints, the publications, the
checkpoints per publication, and the give-ups (refusal 'a failing row is
outside the 4 um band or has no sheet slack'), plus the refusal categories.
"""
import json, glob, collections, sys
D, A_, B_ = sys.argv[1], sys.argv[2], sys.argv[3]
profiles = [sys.argv[4]] if len(sys.argv) > 4 else ['legacy', 'wall10s']
seeds = [int(s) for s in sys.argv[5].split()] if len(sys.argv) > 5 else list(range(18, 27))
def cells(label, profile):
    out = {}
    for s in seeds:
        fs = glob.glob(f'{D}/{label}-{profile}-r0-s{s}.json')
        if fs: out[s] = json.load(open(fs[0]))
    return out
def cat(r):
    if r is None: return 'PUBLISHED'
    if 'outside the' in r and 'band' in r: return 'give-up (band/slack)'
    if r.startswith('piece ') and 'crosses' in r: return 'piece crosses the strip'
    if r.startswith('pieces '): return 'pair refused by validator/kernel'
    if 'enlarged the locked strip' in r: return 'repair would enlarge the strip'
    return r[:50]
def stats(doc):
    o = doc['outcome']; cps = o['exactCheckpoints']
    pubs = o.get('publicationCount', len(o.get('publications', [])))
    ref = collections.Counter(cat(c.get('refusal')) for c in cps)
    return dict(cps=len(cps), pubs=pubs, giveup=ref['give-up (band/slack)'], ref=ref,
                depth=o['depthMm'], invalid=o.get('invalidPublications', 0))
for profile in profiles:
    A = cells(A_, profile); B = cells(B_, profile)
    print(f'== {profile}: {A_} vs {B_} ==')
    tot = {'A': collections.Counter(), 'B': collections.Counter()}
    for s in seeds:
        if s not in A or s not in B: continue
        a = stats(A[s]); b = stats(B[s])
        print(f'  s{s}: cps {a["cps"]:4d}->{b["cps"]:4d}  pubs {a["pubs"]:3d}->{b["pubs"]:3d}  cps/pub {a["cps"]/max(a["pubs"],1):5.2f}->{b["cps"]/max(b["pubs"],1):5.2f}  give-ups {a["giveup"]:4d}->{b["giveup"]:4d}  depth {a["depth"]:.3f}->{b["depth"]:.3f}')
        for side, x in (('A', a), ('B', b)):
            for k in ('cps', 'pubs', 'giveup', 'invalid'): tot[side][k] += x[k]
            for k, v in x['ref'].items(): tot[side]['ref:' + k] += v
    for side, lab in (('A', A_), ('B', B_)):
        t = tot[side]
        print(f'  TOTAL {lab}: checkpoints {t["cps"]} publications {t["pubs"]} cps/pub {t["cps"]/max(t["pubs"],1):.2f} give-ups {t["giveup"]} invalid {t["invalid"]}')
        for k, v in sorted(t.items()):
            if k.startswith('ref:'): print(f'      {k[4:]}: {v}')
