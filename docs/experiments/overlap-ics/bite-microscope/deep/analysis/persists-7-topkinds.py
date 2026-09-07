#!/usr/bin/env python3
"""persists-7-topkinds.py -- kind breakdown of the ten most persistent rows per attempt (from persists-census.json):
pair vs boundary (side), entry vs created, residence as a fraction of the attempt's iterations; and the pieces
that recur among them across attempts of the same seed (both arms).
Usage: python3 persists-7-topkinds.py
"""
import json, statistics as st
from collections import Counter
R = json.load(open("/var/lib/t3/tmp/astra/deep/analysis/persists-census.json"))
for g in sorted({(r["arm"], r["bite"], r["published"]) for r in R}):
    sel = [r for r in R if (r["arm"], r["bite"], r["published"]) == g]
    kinds = Counter(); entry = Counter(); fr = []
    for r in sel:
        for t in r["top10"]:
            k = t["kind"]; kinds["pair" if k[0] == "pair" else "boundary_" + k[2]] += 1; entry[t["entry"]] += 1; fr.append(t["sweeps"] / r["iterations"])
    n = sum(kinds.values())
    print(f"arm {g[0]} bite {g[1]} {'published' if g[2] else 'failed'} n={len(sel)}: top-10 rows {n}: " + ", ".join(f"{k} {v} ({v/n:.2f})" for k, v in sorted(kinds.items())) +
          f"; entry rows {entry[True]} ({entry[True]/n:.2f}), created {entry[False]}; residence fraction med {st.median(fr):.2f} max {max(fr):.2f}")
print("\n== pieces in the top-10 rows, per seed, both arms (bite 5): shared pieces between A's and B's top-10 rows")
seeds = sorted({r["seed"] for r in R}, key=str)
for sd in seeds:
    pcs = {}
    for r in R:
        if r["seed"] == sd and r["bite"] == 5:
            s = set()
            for t in r["top10"]:
                k = t["kind"]; s.add(k[1]); 
                if k[0] == "pair": s.add(k[2])
            pcs[r["arm"]] = (s, r["published"])
    if "A" in pcs and "B" in pcs:
        a, b = pcs["A"][0], pcs["B"][0]
        print(f"  seed {str(sd)[:8]:>8}: A({'P' if pcs['A'][1] else 'F'}) pieces {sorted(a)} ; B({'P' if pcs['B'][1] else 'F'}) pieces {sorted(b)} ; shared {sorted(a & b)}")
