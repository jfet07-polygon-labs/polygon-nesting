"""synth-2-budget.py -- the other objective's evaluation budget relative to the live attempt, per capsule class.
Analyst 3 wrote 1.25-2.88x for p=2 from B's bite-5 entries, analyst 4 wrote 1.3-3.2x. Recompute from the replays.
Usage: cd /var/lib/t3/tmp/astra/deep/analysis && python3 synth-2-budget.py
"""
import glob, json, os, statistics as st
DEEP = "/var/lib/t3/tmp/astra/deep"
live = {}
for path in glob.glob(os.path.join(DEEP, "deep-*-wall10s-s*.json")):
    d = json.load(open(path)); arm = os.path.basename(path).split("-")[1]
    for b in d["biteMicroscope"]["bites"]:
        live[(arm, str(d["seed"]), b["ordinal"])] = (b["separations"][0]["evaluationsAllWorkers"], bool(b["published"]))
out = {}
for path in glob.glob(os.path.join(DEEP, "replays", "rp-deep-*.json")):
    n = os.path.basename(path)[:-5].split("-")
    arm, seed, bite, p = n[2], n[4][1:], int(n[5][1:]), int(n[7][1:])
    r = json.load(open(path))["replay"]
    own = (arm == "A") == (p == 2)
    if own: continue
    lv, pub = live[(arm, seed, bite)]
    out.setdefault((arm, bite, pub), []).append((r["evaluationsTotal"] / lv, r["evaluationsTotal"], r.get("bandEnteredAtIteration")))
for k in sorted(out):
    v = out[k]; ratios = sorted(x[0] for x in v); tot = sorted(x[1] for x in v)
    print(f"capsules {k[0]} bite {k[1]} {'published' if k[2] else 'failed'} n={len(v)} under p={1 if k[0]=='A' else 2}: budget/live min {ratios[0]:.2f} med {st.median(ratios):.2f} max {ratios[-1]:.2f}; evaluations total {tot[0]/1e6:.1f}-{tot[-1]/1e6:.1f} M; band entries {sum(1 for x in v if x[2] is not None)}")
allB = [x for k in out for x in out[k] if k[0] == "B" and k[1] == 5]
print(f"all B bite-5 capsules under p=2 n={len(allB)}: budget/live {min(x[0] for x in allB):.2f}-{max(x[0] for x in allB):.2f}; evaluations {min(x[1] for x in allB)/1e6:.1f}-{max(x[1] for x in allB)/1e6:.1f} M")
allA = [x for k in out for x in out[k] if k[0] == "A" and k[1] == 5]
print(f"all A bite-5 capsules under p=1 n={len(allA)}: budget/live {min(x[0] for x in allA):.2f}-{max(x[0] for x in allA):.2f}; evaluations {min(x[1] for x in allA)/1e6:.1f}-{max(x[1] for x in allA)/1e6:.1f} M")
