"""synth-1-reconcile.py -- reconcile the four analysts' readings where they differ or look alike.
(a) rollback counts per (arm, bite, outcome) and per (arm, outcome) pooled over bites (analyst 1 pooled bites,
    analyst 4 tabulated bite 5 alone);
(b) how many control (A) failed fifth cuts hand back an iteration-5..10 state (analyst 1: 14 of 20; analyst 4: 13);
    the handed-back iteration is restoredToIteration and is checked against the last samples[].newMinimum;
(c) the max residual of the handed-back state (stopBlocking) against the best end-of-sweep max violation over the
    attempt (min over samples[].maxViolationMm): two different quantities;
(d) live evaluations per master iteration, arm A vs arm B, bite 5 (the p=2 / p=1 cost ratio);
(e) rollback restore -> next sweep max residual, bite 5 failed only.
Usage: cd /var/lib/t3/tmp/astra/deep/analysis && python3 synth-1-reconcile.py
"""
import glob, json, os, statistics as st
from collections import defaultdict

DEEP = "/var/lib/t3/tmp/astra/deep"
rows = []
for path in sorted(glob.glob(os.path.join(DEEP, "deep-*-wall10s-s*.json"))):
    d = json.load(open(path))
    arm = os.path.basename(path).split("-")[1]
    seed = d["seed"]
    for b in d["biteMicroscope"]["bites"]:
        assert len(b["separations"]) == 1
        s = b["separations"][0]
        sweeps = s["sweeps"]
        by_it = {w["iteration"]: w for w in sweeps}
        samples = s["samples"]
        last_min = max(x[0] for x in samples if x[4])
        best_max = min(x[3] for x in samples)
        handed = max((r[1] for r in s["stopBlocking"]), default=0.0)
        rb = []
        for at, to in s["rollbacks"]:
            nxt = by_it.get(at + 1)
            rb.append((at, to, by_it[to]["maxAfterMm"] if to in by_it else None, nxt["maxAfterMm"] if nxt else None))
        rows.append(dict(arm=arm, seed=str(seed), bite=b["ordinal"], pub=bool(b["published"]), it=s["iterations"],
                         restored=s["restoredToIteration"], last_min=last_min, best_max=best_max, handed=handed,
                         rollbacks=rb, evals=s["evaluationsAllWorkers"], stop=s["stop"]))

def med(v): return st.median(v) if v else float("nan")
def grp(f): return [r for r in rows if f(r)]

print("(a) rollbacks")
for arm in "AB":
    for bite in (5, 6):
        for pub in (False, True):
            g = grp(lambda r: r["arm"] == arm and r["bite"] == bite and r["pub"] == pub)
            if g: print(f"  {arm} bite {bite} {'published' if pub else 'failed'}: attempts {len(g)}, rollbacks {sum(len(r['rollbacks']) for r in g)}, per attempt {sorted(len(r['rollbacks']) for r in g)}")
    for pub in (False, True):
        g = grp(lambda r: r["arm"] == arm and r["pub"] == pub)
        n_with_next = sum(1 for r in g for x in r["rollbacks"] if x[3] is not None)
        print(f"  {arm} {'published' if pub else 'failed'} pooled over bites: attempts {len(g)}, rollbacks {sum(len(r['rollbacks']) for r in g)}, with a next sweep {n_with_next}")

print("(b) control failed fifth cuts: handed-back iteration")
g = grp(lambda r: r["arm"] == "A" and r["bite"] == 5 and not r["pub"])
print("  restoredTo == last newMinimum on", sum(1 for r in g if r["restored"] == r["last_min"]), "of", len(g),
      "(all groups:", sum(1 for r in rows if r["restored"] is not None and r["restored"] == r["last_min"]), "of", sum(1 for r in rows if r["restored"] is not None), ")")
its = sorted(r["restored"] for r in g)
print("  restoredTo sorted:", its, "median", med(its))
print("  in 5..10:", sum(1 for x in its if 5 <= x <= 10), "of", len(its), "; the others:", [x for x in its if not 5 <= x <= 10])

print("(c) handed-back max residual (stopBlocking) vs best end-of-sweep max violation (mm), medians [min-max]")
for arm in "AB":
    for bite in (5, 6):
        g = grp(lambda r: r["arm"] == arm and r["bite"] == bite and not r["pub"])
        if not g: continue
        h = [r["handed"] for r in g]; bm = [r["best_max"] for r in g]
        print(f"  {arm} bite {bite} failed n={len(g)}: handed-back {med(h):.3f} [{min(h):.3f}-{max(h):.3f}]; best max {med(bm):.3f} [{min(bm):.3f}-{max(bm):.3f}]; handed-back > best max on {sum(1 for r in g if r['handed'] > r['best_max'] + 1e-9)}; equal (1e-6) on {sum(1 for r in g if abs(r['handed']-r['best_max']) < 1e-6)}")
for r in g if False else grp(lambda r: r["seed"] in ("17316774662183274765", "28", "30") and not r["pub"] and r["bite"] == 5):
    print(f"    {r['arm']} s{r['seed']}: handed-back {r['handed']:.4f} mm at iteration {r['restored']}, best max {r['best_max']:.4f} mm")

print("(d) live evaluations per master iteration, bite 5, all attempts")
ev = {}
for arm in "AB":
    g = grp(lambda r: r["arm"] == arm and r["bite"] == 5)
    v = [r["evals"] / r["it"] for r in g]; ev[arm] = med(v)
    print(f"  {arm}: n={len(g)} median {med(v):,.0f} [{min(v):,.0f}-{max(v):,.0f}]")
print(f"  ratio A/B of medians {ev['A']/ev['B']:.2f}")

print("(e) rollbacks in failed fifth cuts: restored max -> next sweep max (mm)")
for arm in "AB":
    g = grp(lambda r: r["arm"] == arm and r["bite"] == 5 and not r["pub"])
    d = [x for r in g for x in r["rollbacks"] if x[2] is not None and x[3] is not None]
    ratio = [x[3] / x[2] for x in d]
    print(f"  {arm}: rollbacks {len(d)}; at iteration med {med([x[0] for x in d]):.0f}; restored max med {med([x[2] for x in d]):.3f}; next sweep max med {med([x[3] for x in d]):.3f}; next > 2x restored {sum(1 for q in ratio if q > 2)}/{len(d)}; next > 5 mm {sum(1 for x in d if x[3] > 5)}/{len(d)}")
