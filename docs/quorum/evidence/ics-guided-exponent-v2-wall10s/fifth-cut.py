#!/usr/bin/env python3
"""fifth-cut.py <celldir> <seeds.txt> : per cell, the fifth explore cut (target <= 156.5 mm) of the archived Wall10s
documents: whether it published, its iterations and charged evaluations, the parent's publication time, the explore
allowance left at entry, attempts, strikes, moved pieces, band reached, and the attempt's best raw Phi (minRawPhi)."""
import json,sys,statistics as st
D,S=sys.argv[1],sys.argv[2]; seeds=[int(l) for l in open(S) if l.strip()]; rows=[]
for arm in 'AB':
    for s in seeds:
        for r in range(3):
            d=json.load(open(f'{D}/v2-{arm}-wall10s-r{r}-s{s}.json')); o=d['outcome']; b=[x for x in o['bites'] if x['phase']=='explore']
            pubs=[p for p in o['publications'] if p['phase']=='explore']; fifth=[x for x in b if x['widthAfterMm']<=156.5]
            if not fifth: continue
            f=fifth[0]; pp=[p for p in pubs if abs(p['publishedRawDepthMm']-f['widthBeforeMm'])<0.5]; pt=pp[-1]['wallSeconds'] if pp else None
            rows.append(dict(arm=arm,seed=s,rep=r,published=f['published'],iters=f['masterIterations'],ev=f['strikeMeter']['chargedWorkSampleEvaluations'],parentTime=pt,
                left=(d['wall']['loopEntrySeconds']+d['wall']['loopExploreSeconds']-pt) if pt else None,attempts=f['attempts'],strikes=f['strikes'],moved=f['movedPieces'],band=f['proxyBandReached'],minRawPhi=f['minRawPhi']))
for arm in 'AB':
    for ok in (True,False):
        sel=[x for x in rows if x['arm']==arm and x['published']==ok]
        if sel: print(f"arm {arm} {'published' if ok else 'failed'}: n {len(sel)} parentTime median {st.median(x['parentTime'] for x in sel):.2f} s, left {st.median(x['left'] for x in sel):.2f} s, iterations median {st.median(x['iters'] for x in sel):.0f}, minRawPhi median {st.median(x['minRawPhi'] for x in sel):.3g}, band {sum(1 for x in sel if x['band'])}")
print(json.dumps(rows) if '--json' in sys.argv else '')
