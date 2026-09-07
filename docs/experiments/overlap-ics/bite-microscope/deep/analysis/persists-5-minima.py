#!/usr/bin/env python3
"""persists-5-minima.py -- the blocking set along the chain of NEW-MINIMUM states (samples[].newMinimum),
i.e. the states the search keeps and restores to, as opposed to every sweep's end state.
Per attempt: number of new minima, their iterations; at each new minimum the blocking count, max residual,
B-T / L-R span, entry rows still present; between consecutive minima the rows released / created / kept and
the shared pieces of the largest component; the census of the LAST new minimum (the best state) by kind.
Usage: python3 persists-5-minima.py
"""
import glob, json, os, statistics as st
DEEP = "/var/lib/t3/tmp/astra/deep"
N = 61; PAIRS = 1830; SIDES = "LRBT"
PAIR = {}
for i in range(N):
    for j in range(i + 1, N):
        PAIR[i * N - i * (i + 1) // 2 + (j - i - 1)] = (i, j)
def endpoints(rid):
    if rid < PAIRS: return PAIR[rid]
    k = rid - PAIRS; return (k // 4, "E" + SIDES[k % 4])
def comps(rows):
    parent = {}
    def find(x):
        parent.setdefault(x, x)
        while parent[x] != x:
            parent[x] = parent[parent[x]]; x = parent[x]
        return x
    for rid, _ in rows:
        a, b = endpoints(rid); ra, rb = find(a), find(b)
        if ra != rb: parent[ra] = rb
    groups = {}
    for v in list(parent): groups.setdefault(find(v), set()).add(v)
    cs = sorted(groups.values(), key=lambda c: -sum(1 for v in c if isinstance(v, int)))
    largest = {v for v in cs[0] if isinstance(v, int)} if cs else set()
    bt = any("EB" in c and "ET" in c for c in cs); lr = any("EL" in c and "ER" in c for c in cs)
    return largest, bt, lr, len(cs)
out = []
for path in sorted(glob.glob(os.path.join(DEEP, "deep-*-wall10s-s*.json"))):
    doc = json.load(open(path)); m = doc["biteMicroscope"]
    arm = "A" if doc.get("guidedExponent", 2) == 2 else "B"
    for bite in m["bites"]:
        for sep in bite["separations"]:
            byit = {s["iteration"]: s for s in sep["sweeps"]}
            entry = sep["entryBlocking"]; eids = {r for r, _ in entry}
            mins = [s[0] for s in sep.get("samples", []) if s[4] and s[0] in byit]
            chain = [("entry", entry)] + [(i, byit[i]["blocking"]) for i in mins]
            steps = []; prev = None
            for label, rows in chain:
                ids = {r for r, _ in rows}; largest, bt, lr, nc = comps(rows)
                rec = {"at": label, "n": len(ids), "pair": sum(1 for r in ids if r < PAIRS), "boundary_B": sum(1 for r in ids if r >= PAIRS and (r - PAIRS) % 4 == 2),
                       "boundary_T": sum(1 for r in ids if r >= PAIRS and (r - PAIRS) % 4 == 3), "max_um": max([x * 1000 for _, x in rows], default=0), "bt": bt, "lr": lr, "ncomp": nc,
                       "largest": len(largest), "entry_present": len(ids & eids)}
                if prev is not None:
                    pids, plargest = prev
                    rec.update({"kept_from_prev": len(ids & pids), "released_from_prev": len(pids - ids), "created_from_prev": len(ids - pids),
                                "largest_shared_prev": len(largest & plargest)})
                steps.append(rec); prev = (ids, largest)
            best = steps[-1]
            out.append({"arm": arm, "seed": doc["seed"], "bite": bite["ordinal"], "published": bite["published"], "iterations": sep["iterations"], "n_minima": len(mins), "minima_iters": mins, "steps": steps,
                        "bt_at_minima_frac": (sum(s["bt"] for s in steps[1:]) / len(mins)) if mins else None, "bt_at_best": best["bt"], "lr_at_best": best["lr"],
                        "entry_present_at_best": best["entry_present"], "best_n": best["n"], "best_max_um": best["max_um"], "best_largest": best["largest"], "best_pair": best["pair"], "best_B": best["boundary_B"], "best_T": best["boundary_T"],
                        "kept_med": st.median([s["kept_from_prev"] for s in steps[1:]]) if mins else None,
                        "kept_frac_med": st.median([s["kept_from_prev"] / max(1, s["n"] - s["created_from_prev"] + s["released_from_prev"]) for s in steps[1:]]) if mins else None,
                        "largest_shared_prev_med": st.median([s["largest_shared_prev"] for s in steps[1:]]) if mins else None,
                        "bt_run_at_minima": [int(s["bt"]) for s in steps]})
json.dump(out, open(os.path.join(DEEP, "analysis", "persists-minima.json"), "w"), indent=1)
def med(v): v = [x for x in v if x is not None]; return f"{st.median(v):.1f}" if v else "-"
def rng(v): v = [x for x in v if x is not None]; return f"[{min(v):.0f}-{max(v):.0f}]" if v else ""
for g in sorted({(r["arm"], r["bite"], r["published"]) for r in out}):
    sel = [r for r in out if (r["arm"], r["bite"], r["published"]) == g]
    print(f"\n-- arm {g[0]} bite {g[1]} {'published' if g[2] else 'failed'} n={len(sel)}")
    print(f"  new minima per attempt med {med([r['n_minima'] for r in sel])} {rng([r['n_minima'] for r in sel])}; last minimum at iteration med {med([r['minima_iters'][-1] if r['minima_iters'] else None for r in sel])} of {med([r['iterations'] for r in sel])}")
    print(f"  B-T span at entry {sum(r['steps'][0]['bt'] for r in sel)}/{len(sel)}; fraction of new minima with a B-T span med {med([r['bt_at_minima_frac'] for r in sel])} {rng([r['bt_at_minima_frac'] for r in sel])}; B-T at the best state {sum(r['bt_at_best'] for r in sel)}/{len(sel)}; L-R at best {sum(r['lr_at_best'] for r in sel)}/{len(sel)}")
    print(f"  best state: rows med {med([r['best_n'] for r in sel])} {rng([r['best_n'] for r in sel])} (pair {med([r['best_pair'] for r in sel])}, B {med([r['best_B'] for r in sel])}, T {med([r['best_T'] for r in sel])}), max um med {med([r['best_max_um'] for r in sel])} {rng([r['best_max_um'] for r in sel])}, largest component pieces med {med([r['best_largest'] for r in sel])} {rng([r['best_largest'] for r in sel])}, entry rows present med {med([r['entry_present_at_best'] for r in sel])} {rng([r['entry_present_at_best'] for r in sel])}")
    print(f"  between consecutive minima: rows kept med {med([r['kept_med'] for r in sel])}, kept fraction of previous med {med([r['kept_frac_med'] for r in sel])}, largest-component pieces shared med {med([r['largest_shared_prev_med'] for r in sel])}")
print("\n== per attempt: minima iterations; per minimum n/max_um/BT/largest/entry_present")
for r in sorted(out, key=lambda r: (r["arm"], r["bite"], str(r["seed"]))):
    print(f"  {r['arm']} {str(r['seed'])[:6]:>6} b{r['bite']} {'P' if r['published'] else 'F'} it={r['iterations']}: " + " | ".join(f"{s['at']}: {s['n']}/{s['max_um']:.0f}/{'BT' if s['bt'] else '--'}/{s['largest']}/{s['entry_present']}" for s in r["steps"]))
