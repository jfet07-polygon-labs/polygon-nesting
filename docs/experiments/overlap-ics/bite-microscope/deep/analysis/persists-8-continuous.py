#!/usr/bin/env python3
"""persists-8-continuous.py -- (a) entry rows present at the handed-back state (stopBlocking): how many were
present in EVERY sweep from entry to the restored iteration (continuous) versus released and re-formed;
(b) entry rows continuously present up to sweep k; (c) for every end-of-sweep state, the pieces carrying a
row > 10 mm and whether the winner relocated them in that sweep with a 'container' or 'focused' origin, or by
more than 10 mm; (d) per sweep the winner's container relocations (count).
Usage: python3 persists-8-continuous.py
"""
import glob, json, os, statistics as st
DEEP = "/var/lib/t3/tmp/astra/deep"; N = 61; PAIRS = 1830; SIDES = "LRBT"
PAIR = {}
for i in range(N):
    for j in range(i + 1, N):
        PAIR[i * N - i * (i + 1) // 2 + (j - i - 1)] = (i, j)
def pieces_of(rid):
    return set(PAIR[rid]) if rid < PAIRS else {(rid - PAIRS) // 4}
KS = [1, 2, 3, 5, 10, 20, 50, 100, 200]
out = []
for path in sorted(glob.glob(os.path.join(DEEP, "deep-*-wall10s-s*.json"))):
    doc = json.load(open(path)); arm = "A" if doc.get("guidedExponent", 2) == 2 else "B"
    for bite in doc["biteMicroscope"]["bites"]:
        for sep in bite["separations"]:
            sweeps = sorted(sep["sweeps"], key=lambda s: s["iteration"]); iters = [s["iteration"] for s in sweeps]
            eids = {r for r, _ in sep["entryBlocking"]}; sids = {r for r, _ in sep["stopBlocking"]}
            sets = [{r for r, _ in s["blocking"]} for s in sweeps]
            rt = sep.get("restoredToIteration") or iters[-1]
            cont = set(eids)
            cont_at = {}
            for k, (it, s) in enumerate(zip(iters, sets)):
                cont &= s
                if it in KS: cont_at[it] = len(cont)
                if it == rt: cont_rt = set(cont)
            present_stop = eids & sids
            continuous_stop = present_stop & cont_rt
            # big rows vs relocations
            big_pieces_total = 0; big_relocated = 0; big_relocated_far = 0; cont_per_sweep = []; big_rows_involving_relocated = 0; big_rows = 0
            for s in sweeps:
                big = [rid for rid, r in s["blocking"] if r > 0.010]
                bp = set().union(*[pieces_of(r) for r in big]) if big else set()
                rel = {rl["piece"]: rl for rl in s["relocates"]}
                relocated = {p for p, rl in rel.items() if rl["moved"] and rl["origin"] in ("container", "focused")}
                far = {p for p, rl in rel.items() if rl["moved"] and (rl["dxMm"] ** 2 + rl["dyMm"] ** 2) ** 0.5 > 10}
                big_pieces_total += len(bp); big_relocated += len(bp & relocated); big_relocated_far += len(bp & (relocated | far))
                big_rows += len(big); big_rows_involving_relocated += sum(1 for r in big if pieces_of(r) & (relocated | far))
                cont_per_sweep.append(sum(1 for rl in s["relocates"] if rl["origin"] == "container"))
            out.append({"arm": arm, "seed": doc["seed"], "bite": bite["ordinal"], "published": bite["published"], "restoredTo": rt, "entry_n": len(eids),
                        "present_stop": len(present_stop), "continuous_stop": len(continuous_stop), "reformed_stop": len(present_stop) - len(continuous_stop),
                        "cont_at": cont_at, "big_pieces": big_pieces_total, "big_relocated": big_relocated, "big_relocated_far": big_relocated_far, "big_rows": big_rows, "big_rows_reloc": big_rows_involving_relocated,
                        "container_per_sweep_med": st.median(cont_per_sweep), "container_sweeps_frac": sum(1 for c in cont_per_sweep if c > 0) / len(cont_per_sweep)})
json.dump(out, open(os.path.join(DEEP, "analysis", "persists-continuous.json"), "w"), indent=1)
def med(v): v = [x for x in v if x is not None]; return f"{st.median(v):.1f}" if v else "-"
def rng(v): v = [x for x in v if x is not None]; return f"[{min(v):.0f}-{max(v):.0f}]" if v else ""
for g in sorted({(r["arm"], r["bite"], r["published"]) for r in out}):
    sel = [r for r in out if (r["arm"], r["bite"], r["published"]) == g]
    print(f"\n-- arm {g[0]} bite {g[1]} {'published' if g[2] else 'failed'} n={len(sel)}: handed-back state at iteration med {med([r['restoredTo'] for r in sel])} {rng([r['restoredTo'] for r in sel])}")
    print(f"  entry rows {med([r['entry_n'] for r in sel])}: present at handed-back state med {med([r['present_stop'] for r in sel])} {rng([r['present_stop'] for r in sel])}; of these CONTINUOUSLY present since entry med {med([r['continuous_stop'] for r in sel])} {rng([r['continuous_stop'] for r in sel])}, released-and-re-formed med {med([r['reformed_stop'] for r in sel])} {rng([r['reformed_stop'] for r in sel])}")
    print("  entry rows continuously present up to sweep k (median): " + ", ".join(f"k={k}: {med([r['cont_at'].get(k) for r in sel])}" for k in KS if any(k in r['cont_at'] for r in sel)))
    bp = sum(r["big_pieces"] for r in sel); br = sum(r["big_relocated"] for r in sel); bf = sum(r["big_relocated_far"] for r in sel); rows = sum(r["big_rows"] for r in sel); rr = sum(r["big_rows_reloc"] for r in sel)
    print(f"  pieces carrying a > 10 mm row at a sweep's end: {bp}; relocated in that sweep by the winner with container/focused origin: {br} ({br/bp:.2f}); container/focused or > 10 mm displacement: {bf} ({bf/bp:.2f}); > 10 mm rows {rows}, involving such a piece {rr} ({rr/rows:.2f})")
    print(f"  winner container relocations per sweep med {med([r['container_per_sweep_med'] for r in sel])}; fraction of sweeps with >= 1 container relocation med {med([r['container_sweeps_frac'] for r in sel])} {rng([r['container_sweeps_frac'] for r in sel])}")
print("\n== per attempt: entry rows present at handed-back state = continuous + re-formed")
for r in sorted(out, key=lambda r: (r["arm"], r["bite"], str(r["seed"]))):
    print(f"  {r['arm']} {str(r['seed'])[:6]:>6} b{r['bite']} {'P' if r['published'] else 'F'} restoredTo {r['restoredTo']:>4}: entry {r['entry_n']} present {r['present_stop']:>2} = continuous {r['continuous_stop']:>2} + re-formed {r['reformed_stop']:>2}; cont@k " + " ".join(f"{k}:{v}" for k, v in sorted(r['cont_at'].items())))
