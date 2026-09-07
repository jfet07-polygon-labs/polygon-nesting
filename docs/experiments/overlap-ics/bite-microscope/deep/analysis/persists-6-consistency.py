#!/usr/bin/env python3
"""persists-6-consistency.py -- (a) does each sweep's maxAfterMm equal the max residual of its blocking rows;
(b) which row kinds carry the > 10 mm residuals in end-of-sweep states; (c) the winner's relocate origins
(stayPut / focused / container) and displacement sizes, overall and in sweeps whose end state exceeds 10 mm;
(d) fraction of end-of-sweep states with any piece-vs-B/T boundary residual > 10 mm.
Usage: python3 persists-6-consistency.py
"""
import glob, json, os, statistics as st
from collections import Counter
DEEP = "/var/lib/t3/tmp/astra/deep"; PAIRS = 1830; SIDES = "LRBT"
def kind(rid):
    if rid < PAIRS: return "pair"
    return "boundary_" + SIDES[(rid - PAIRS) % 4]
tot = Counter(); mismatch = 0; nsweeps = 0; big_kind = Counter(); big_states = 0; big_boundary_states = 0
origins = {"A": Counter(), "B": Counter()}; big_origins = {"A": Counter(), "B": Counter()}
disp = {"A": {"stayPut": [], "focused": [], "container": []}, "B": {"stayPut": [], "focused": [], "container": []}}
moved = {"A": Counter(), "B": Counter()}
for path in sorted(glob.glob(os.path.join(DEEP, "deep-*-wall10s-s*.json"))):
    doc = json.load(open(path)); arm = "A" if doc.get("guidedExponent", 2) == 2 else "B"
    for bite in doc["biteMicroscope"]["bites"]:
        for sep in bite["separations"]:
            for s in sep["sweeps"]:
                nsweeps += 1
                mx = max([r for _, r in s["blocking"]], default=0.0)
                if abs(mx - s["maxAfterMm"]) > 1e-9: mismatch += 1
                big = [(rid, r) for rid, r in s["blocking"] if r > 0.010]
                if big:
                    big_states += 1
                    for rid, r in big: big_kind[kind(rid)] += 1
                    if any(kind(rid).startswith("boundary") for rid, _ in big): big_boundary_states += 1
                for rl in s["relocates"]:
                    origins[arm][rl["origin"]] += 1
                    moved[arm][(rl["origin"], rl["moved"])] += 1
                    d = (rl["dxMm"] ** 2 + rl["dyMm"] ** 2) ** 0.5
                    disp[arm][rl["origin"]].append(d)
                    if s["maxAfterMm"] > 0.010: big_origins[arm][rl["origin"]] += 1
print(f"sweeps {nsweeps}; maxAfterMm != max blocking residual in {mismatch} sweeps")
print(f"end-of-sweep states with any row residual > 10 mm: {big_states}/{nsweeps}; of which with a boundary row > 10 mm: {big_boundary_states}")
print("rows with residual > 10 mm by kind:", dict(big_kind))
for arm in "AB":
    o = origins[arm]; n = sum(o.values())
    print(f"arm {arm}: winner relocates {n}; origins " + ", ".join(f"{k} {v} ({v/n:.2f})" for k, v in o.items()) + "; moved by origin " + str({f'{k[0]}/{k[1]}': v for k, v in moved[arm].items()}))
    for k, v in disp[arm].items():
        if v: print(f"   displacement mm {k}: n {len(v)} median {st.median(v):.3f} q90 {sorted(v)[int(0.9*len(v))]:.2f} max {max(v):.1f}; > 50 mm: {sum(1 for x in v if x > 50)}")
    bo = big_origins[arm]; nb = sum(bo.values())
    print(f"   relocates in sweeps whose end state max > 10 mm: {nb}; origins " + ", ".join(f"{k} {v} ({v/nb:.2f})" for k, v in bo.items()))
