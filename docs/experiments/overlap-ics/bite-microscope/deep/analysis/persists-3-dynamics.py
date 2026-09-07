#!/usr/bin/env python3
"""persists-3-dynamics.py -- sweep-by-sweep dynamics of the blocking set in every retained attempt.
For each attempt (42 microscope documents, read-only):
  * decay curve: fraction of entry rows still blocking at sweep k (k = 1,2,3,5,10,20,50,100,200,400),
    and the fraction of sweep-k rows that are entry rows;
  * B-T / L-R spanning: fraction of sweeps whose blocking graph has a component joining the bottom and
    top edges (and left/right), the first sweep without a B-T span, the longest run of sweeps without one;
  * turnover: Jaccard similarity of consecutive sweeps' blocking sets (median), and of each sweep vs entry;
  * presence fractions: rows present in > 50 % / > 25 % / > 10 % of sweeps, entry rows among them;
  * windowed medians (per 100 iterations) of maxAfterMm, blocking count, rawAfter;
  * new-minimum iterations (samples column newMinimum) count and last;
  * bands: fraction of sweeps with maxAfter < 2 mm, 2-5, 5-10, > 10 mm.
Usage: python3 persists-3-dynamics.py
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
def spans(rows):
    parent = {}
    def find(x):
        parent.setdefault(x, x)
        while parent[x] != x:
            parent[x] = parent[parent[x]]; x = parent[x]
        return x
    for rid, _ in rows:
        a, b = endpoints(rid); ra, rb = find(a), find(b)
        if ra != rb: parent[ra] = rb
    def same(u, v): return u in parent and v in parent and find(u) == find(v)
    return same("EB", "ET"), same("EL", "ER")
def jac(a, b):
    if not a and not b: return 1.0
    return len(a & b) / len(a | b)
KS = [1, 2, 3, 5, 10, 20, 50, 100, 200, 400]
out = []
for path in sorted(glob.glob(os.path.join(DEEP, "deep-*-wall10s-s*.json"))):
    doc = json.load(open(path)); m = doc["biteMicroscope"]
    arm = "A" if doc.get("guidedExponent", 2) == 2 else "B"
    for bite in m["bites"]:
        for sep in bite["separations"]:
            sweeps = sorted(sep["sweeps"], key=lambda s: s["iteration"])
            eids = {r for r, _ in sep["entryBlocking"]}
            sets = [{r for r, _ in s["blocking"]} for s in sweeps]
            iters = [s["iteration"] for s in sweeps]
            byit = dict(zip(iters, sets))
            decay = {k: (len(eids & byit[k]) / len(eids), (len(eids & byit[k]) / len(byit[k]) if byit[k] else None)) for k in KS if k in byit}
            sp = [spans(s["blocking"]) for s in sweeps]
            bt = [a for a, _ in sp]; lr = [b for _, b in sp]
            first_no_bt = next((iters[k] for k, v in enumerate(bt) if not v), None)
            run = best = 0
            for v in bt:
                run = run + 1 if not v else 0; best = max(best, run)
            cons = [jac(sets[k], sets[k + 1]) for k in range(len(sets) - 1)]
            vs_entry = [jac(eids, s) for s in sets]
            cnt = {}
            for s in sets:
                for r in s: cnt[r] = cnt.get(r, 0) + 1
            n = len(sets)
            pres = lambda f: [r for r, c in cnt.items() if c > f * n]
            mx = [s["maxAfterMm"] * 1000 for s in sweeps]; nb = [len(s) for s in sets]; raw = [s["rawAfter"] for s in sweeps]
            win = []
            for w0 in range(0, iters[-1], 100):
                idx = [k for k, i in enumerate(iters) if w0 < i <= w0 + 100]
                if idx: win.append({"window": f"{w0+1}-{w0+100}", "max_med": st.median(mx[k] for k in idx), "nblk_med": st.median(nb[k] for k in idx), "raw_med": st.median(raw[k] for k in idx), "bt_frac": sum(bt[k] for k in idx) / len(idx)})
            samples = sep.get("samples", [])
            newmin = [s[0] for s in samples if s[4]]
            bands = {"lt2": sum(v < 2000 for v in mx) / n, "2to5": sum(2000 <= v < 5000 for v in mx) / n, "5to10": sum(5000 <= v < 10000 for v in mx) / n, "gt10": sum(v >= 10000 for v in mx) / n}
            out.append({"arm": arm, "seed": doc["seed"], "bite": bite["ordinal"], "published": bite["published"], "iterations": sep["iterations"],
                        "decay": decay, "bt_frac": sum(bt) / n, "lr_frac": sum(lr) / n, "bt_entry": spans(sep["entryBlocking"])[0], "first_no_bt": first_no_bt, "longest_no_bt_run": best,
                        "bt_last": bt[-1], "consec_jaccard_med": st.median(cons) if cons else None, "vs_entry_jaccard_last": vs_entry[-1], "vs_entry_jaccard_med": st.median(vs_entry),
                        "rows_gt50": len(pres(.5)), "rows_gt25": len(pres(.25)), "rows_gt10": len(pres(.1)), "entry_rows_gt25": sum(1 for r in pres(.25) if r in eids), "entry_rows_gt10": sum(1 for r in pres(.1) if r in eids),
                        "windows": win, "newmin_count": len(newmin), "newmin_last": max(newmin) if newmin else None, "bands": bands})
json.dump(out, open(os.path.join(DEEP, "analysis", "persists-dynamics.json"), "w"), indent=1)
def med(v): v = [x for x in v if x is not None]; return f"{st.median(v):.2f}" if v else "-"
def rng(v): v = [x for x in v if x is not None]; return f"[{min(v):.2f}-{max(v):.2f}]" if v else ""
groups = sorted({(r["arm"], r["bite"], r["published"]) for r in out})
for g in groups:
    sel = [r for r in out if (r["arm"], r["bite"], r["published"]) == g]
    print(f"\n-- arm {g[0]} bite {g[1]} {'published' if g[2] else 'failed'} n={len(sel)}")
    print("  entry-row survival fraction at sweep k (median over attempts): " + ", ".join(f"k={k}: {med([r['decay'][k][0] for r in sel if k in r['decay']])} (n={sum(1 for r in sel if k in r['decay'])})" for k in KS))
    print("  fraction of sweep-k rows that are entry rows (median):        " + ", ".join(f"k={k}: {med([r['decay'][k][1] for r in sel if k in r['decay']])}" for k in KS))
    print(f"  B-T span at entry: {sum(r['bt_entry'] for r in sel)}/{len(sel)}; fraction of sweeps with a B-T span: med {med([r['bt_frac'] for r in sel])} {rng([r['bt_frac'] for r in sel])}; L-R: med {med([r['lr_frac'] for r in sel])}; B-T at last sweep: {sum(r['bt_last'] for r in sel)}/{len(sel)}")
    print(f"  first sweep without B-T span: med {med([r['first_no_bt'] for r in sel])} (none in {sum(1 for r in sel if r['first_no_bt'] is None)}); longest run without B-T: med {med([r['longest_no_bt_run'] for r in sel])} {rng([r['longest_no_bt_run'] for r in sel])}")
    print(f"  consecutive-sweep Jaccard med {med([r['consec_jaccard_med'] for r in sel])} {rng([r['consec_jaccard_med'] for r in sel])}; vs entry: med over sweeps {med([r['vs_entry_jaccard_med'] for r in sel])}, last {med([r['vs_entry_jaccard_last'] for r in sel])}")
    print(f"  rows present in >50% of sweeps: med {med([r['rows_gt50'] for r in sel])} {rng([r['rows_gt50'] for r in sel])}; >25%: {med([r['rows_gt25'] for r in sel])} {rng([r['rows_gt25'] for r in sel])} (entry rows among them {med([r['entry_rows_gt25'] for r in sel])}); >10%: {med([r['rows_gt10'] for r in sel])} (entry {med([r['entry_rows_gt10'] for r in sel])})")
    print(f"  new-minimum events: med {med([r['newmin_count'] for r in sel])} {rng([r['newmin_count'] for r in sel])}, last new minimum at iteration med {med([r['newmin_last'] for r in sel])} {rng([r['newmin_last'] for r in sel])} of {med([r['iterations'] for r in sel])} iterations")
    print("  maxAfter bands (fraction of sweeps): <2mm " + med([r['bands']['lt2'] for r in sel]) + ", 2-5 " + med([r['bands']['2to5'] for r in sel]) + ", 5-10 " + med([r['bands']['5to10'] for r in sel]) + ", >10 " + med([r['bands']['gt10'] for r in sel]))
    for wname in ["1-100", "101-200", "201-300", "301-400", "401-500", "501-600", "601-700", "701-800", "801-900"]:
        ws = [w for r in sel for w in r["windows"] if w["window"] == wname]
        if ws: print(f"  window {wname:>8}: n={len(ws)} max med {st.median(w['max_med'] for w in ws):8.0f} um, nblk med {st.median(w['nblk_med'] for w in ws):5.1f}, raw med {st.median(w['raw_med'] for w in ws):7.1f}, B-T span frac med {st.median(w['bt_frac'] for w in ws):.2f}")
print("\n== per attempt: B-T span fraction / first no-BT / longest no-BT run / rows>25% (entry) / newmin count,last / bands <2,2-5,5-10,>10")
for r in sorted(out, key=lambda r: (r["arm"], r["bite"], str(r["seed"]))):
    b = r["bands"]
    print(f"  {r['arm']} {str(r['seed'])[:6]:>6} b{r['bite']} {'P' if r['published'] else 'F'} it={r['iterations']:>3}: BT@entry {int(r['bt_entry'])} frac {r['bt_frac']:.2f} first-no {r['first_no_bt']} longest-no {r['longest_no_bt_run']:>3} | rows>25% {r['rows_gt25']:>2} ({r['entry_rows_gt25']}) >50% {r['rows_gt50']} | newmin {r['newmin_count']:>3} last {r['newmin_last']} | {b['lt2']:.2f} {b['2to5']:.2f} {b['5to10']:.2f} {b['gt10']:.2f}")
