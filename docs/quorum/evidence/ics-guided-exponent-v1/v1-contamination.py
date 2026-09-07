#!/usr/bin/env python3
"""v1-contamination.py <celldir> <prefix> <load.log> [threshold=10.0] - the registered contamination rule of
ICS-guided-exponent-v1: a PAIR (profile, seed, rep) is re-run whole if any 1-minute load sample inside either
cell's ten-second window exceeds the threshold. Prints the flagged pairs and the max load per cell."""
import sys,os,glob,collections
D,P,L=sys.argv[1],sys.argv[2],sys.argv[3]; TH=float(sys.argv[4]) if len(sys.argv)>4 else 10.0
samples=[(int(l.split()[0]),float(l.split()[1])) for l in open(L) if l.strip()]
cells={}
for f in sorted(glob.glob(f'{D}/{P}-[AB]-*-r*-s*.json')):
    b=os.path.basename(f)[len(P)+1:-5]; arm,prof,rest=b.split('-',2); rep,seed=rest.split('-'); e=os.path.getmtime(f); s=e-10.8
    mx=max([x for t,x in samples if s<=t<=e] or [0.0]); cells[(prof,seed,rep,arm)]=(mx,f)
pairs=collections.defaultdict(dict)
for (prof,seed,rep,arm),(mx,f) in cells.items(): pairs[(prof,seed,rep)][arm]=mx
flag=[(k,v) for k,v in sorted(pairs.items()) if max(v.values())>TH]
print(f'cells {len(cells)}, pairs {len(pairs)}, samples {len(samples)}, max load over all cell windows {max(v[0] for v in cells.values()):.2f}')
print('flagged pairs (re-run whole):', [(k, {a: round(x,2) for a,x in v.items()}) for k,v in flag] or 'none')
