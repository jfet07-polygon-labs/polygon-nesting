#!/usr/bin/env python3
"""persists-9-beststate.py -- the handed-back (best raw) state of every attempt: which row carries the maximum
residual (kind, pieces), how many rows exceed 1 mm / 100 um, how many distinct pieces those rows touch;
and per seed the entry B-T span / best-state B-T span in both arms next to the outcome.
Usage: python3 persists-9-beststate.py
"""
import glob, json, os, statistics as st
from collections import Counter
DEEP = "/var/lib/t3/tmp/astra/deep"; N = 61; PAIRS = 1830; SIDES = "LRBT"
PAIR = {}
for i in range(N):
    for j in range(i + 1, N):
        PAIR[i * N - i * (i + 1) // 2 + (j - i - 1)] = (i, j)
def kind(rid): return ("pair", PAIR[rid]) if rid < PAIRS else ("boundary", ((rid - PAIRS) // 4, SIDES[(rid - PAIRS) % 4]))
def pieces_of(rid): return set(PAIR[rid]) if rid < PAIRS else {(rid - PAIRS) // 4}
G = json.load(open(os.path.join(DEEP, "analysis", "persists-census.json")))
gmap = {(str(r["seed"]), r["arm"], r["bite"]): r for r in G}
rows = []
for path in sorted(glob.glob(os.path.join(DEEP, "deep-*-wall10s-s*.json"))):
    doc = json.load(open(path)); arm = "A" if doc.get("guidedExponent", 2) == 2 else "B"
    for bite in doc["biteMicroscope"]["bites"]:
        for sep in bite["separations"]:
            stop = sorted(sep["stopBlocking"], key=lambda r: -r[1])
            if not stop:
                rows.append({"arm": arm, "seed": doc["seed"], "bite": bite["ordinal"], "published": bite["published"], "n": 0}); continue
            top = stop[:3]
            gt1 = [r for r in stop if r[1] > 1.0]; gt01 = [r for r in stop if r[1] > 0.1]
            rows.append({"arm": arm, "seed": doc["seed"], "bite": bite["ordinal"], "published": bite["published"], "n": len(stop),
                         "max_kind": kind(top[0][0])[0], "max_row": kind(top[0][0]), "max_um": top[0][1] * 1000, "top3": [(kind(r), round(x * 1000)) for r, x in top],
                         "gt1mm": len(gt1), "gt1mm_pieces": len(set().union(*[pieces_of(r) for r, _ in gt1])) if gt1 else 0, "gt100um": len(gt01), "gt100um_pieces": len(set().union(*[pieces_of(r) for r, _ in gt01])) if gt01 else 0,
                         "gt1mm_kinds": dict(Counter(kind(r)[0] for r, _ in gt1)), "sum_um": sum(x for _, x in stop) * 1000})
def med(v): v = [x for x in v if x is not None]; return f"{st.median(v):.1f}" if v else "-"
def rng(v): v = [x for x in v if x is not None]; return f"[{min(v):.0f}-{max(v):.0f}]" if v else ""
for g in sorted({(r["arm"], r["bite"], r["published"]) for r in rows}):
    sel = [r for r in rows if (r["arm"], r["bite"], r["published"]) == g and r["n"] > 0]
    if not sel: continue
    print(f"-- arm {g[0]} bite {g[1]} {'published' if g[2] else 'failed'} n={len(sel)}: max row kind {dict(Counter(r['max_kind'] for r in sel))}; rows > 1 mm med {med([r['gt1mm'] for r in sel])} {rng([r['gt1mm'] for r in sel])} touching pieces med {med([r['gt1mm_pieces'] for r in sel])}; rows > 100 um med {med([r['gt100um'] for r in sel])} {rng([r['gt100um'] for r in sel])} touching pieces med {med([r['gt100um_pieces'] for r in sel])} {rng([r['gt100um_pieces'] for r in sel])}; sum of residuals um med {med([r['sum_um'] for r in sel])}; >1mm kinds pooled {dict(sum((Counter(r['gt1mm_kinds']) for r in sel), Counter()))}")
print("\n== per attempt: handed-back state top-3 residual rows")
for r in sorted(rows, key=lambda r: (r["arm"], r["bite"], str(r["seed"]))):
    if r["n"]: print(f"  {r['arm']} {str(r['seed'])[:6]:>6} b{r['bite']} {'P' if r['published'] else 'F'}: n {r['n']:>3} >1mm {r['gt1mm']:>2} ({r['gt1mm_pieces']} pieces) >100um {r['gt100um']:>3} ({r['gt100um_pieces']} pieces) top3 {r['top3']}")
print("\n== per seed, bite 5: entry B-T span / best-state B-T / largest entry component / outcome, both arms")
for sd in sorted({str(r["seed"]) for r in rows}):
    a = gmap.get((sd, "A", 5)); b = gmap.get((sd, "B", 5))
    fmt = lambda r: f"entryBT {int(r['graph_entry']['any_bt'])} entryLargest {r['graph_entry']['largest_pieces']:>2} entryRows {r['entry']['n']} bestBT {int(r['graph_stop']['any_bt'])} {'PUB' if r['published'] else 'fail'} it {r['iterations']:>3}"
    print(f"  seed {sd[:8]:>8}: A {fmt(a)} | B {fmt(b)}")
