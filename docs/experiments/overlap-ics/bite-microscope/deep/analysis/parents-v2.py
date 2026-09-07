"""180 v2 cells: is fifth-cut success predicted by parent publication time or allowance left?
Run: cd /var/lib/t3/tmp/astra/deep/analysis && python3 parents-v2.py
"""
import json, collections, itertools
from parents_common import quart, med
rows = json.load(open('/var/lib/t3/tmp/astra/v2/fifth-cut-rows.json'))
assert len(rows) == 180
def auc(pos, neg):
    """Probability a random success has a larger value than a random failure (ties 0.5)."""
    n = 0.0
    for a in pos:
        for b in neg:
            n += 1.0 if a > b else (0.5 if a == b else 0.0)
    return n / (len(pos) * len(neg)) if pos and neg else float('nan')
for arm in 'AB':
    sel = [r for r in rows if r['arm'] == arm]
    pos = [r for r in sel if r['fifth']]; neg = [r for r in sel if not r['fifth']]
    print(f"arm {arm}: cells {len(sel)}, fifth published {len(pos)} on seeds {sorted({r['seed'] for r in pos})}")
    for k in ('ptime', 'left', 'iters', 'ev', 'moved', 'minraw'):
        qp, qn = quart([r[k] for r in pos]), quart([r[k] for r in neg])
        print(f"   {k:7s} success q1/med/q3 {qp[0]:.3f}/{qp[1]:.3f}/{qp[2]:.3f}  failure {qn[0]:.3f}/{qn[1]:.3f}/{qn[2]:.3f}  AUC(success>failure) {auc([r[k] for r in pos],[r[k] for r in neg]):.3f}")
    # thresholds: how many failures have ptime <= max success ptime, left >= min success left
    if pos:
        mp = max(r['ptime'] for r in pos); ml = min(r['left'] for r in pos)
        print(f"   failures with ptime <= max success ptime ({mp:.3f}): {sum(1 for r in neg if r['ptime']<=mp)}/{len(neg)}; failures with left >= min success left ({ml:.3f}): {sum(1 for r in neg if r['left']>=ml)}/{len(neg)}")
        print(f"   failures with ptime <= median success ptime ({med([r['ptime'] for r in pos]):.3f}): {sum(1 for r in neg if r['ptime']<=med([r['ptime'] for r in pos]))}/{len(neg)}")
    # per-seed: seed-level ptime medians, success seeds vs failure seeds
    byseed = collections.defaultdict(list)
    for r in sel: byseed[r['seed']].append(r)
    ss = [(s, med([r['ptime'] for r in v]), med([r['left'] for r in v]), sum(r['fifth'] for r in v)) for s, v in byseed.items()]
    succ = [x for x in ss if x[3] > 0]; fail = [x for x in ss if x[3] == 0]
    print(f"   seed level: success seeds {len(succ)} (all reps published? {[x[3] for x in succ]}), failure seeds {len(fail)}; "
          f"seed-median ptime success {sorted(round(x[1],2) for x in succ)} vs failure q1/med/q3 {quart([x[1] for x in fail])[0]:.2f}/{quart([x[1] for x in fail])[1]:.2f}/{quart([x[1] for x in fail])[2]:.2f}; "
          f"AUC {auc([x[1] for x in succ],[x[1] for x in fail]):.3f}; seed-median left AUC {auc([x[2] for x in succ],[x[2] for x in fail]):.3f}")
    print(f"   rank of success seeds by seed-median ptime (1 = earliest of 30): {[sorted(ss, key=lambda x: x[1]).index(x)+1 for x in succ]}; by left (1 = most): {[sorted(ss, key=lambda x: -x[2]).index(x)+1 for x in succ]}")
    # within-seed consistency: for every seed, are the three reps' outcomes identical?
    print(f"   seeds with mixed outcomes across reps: {[s for s,v in byseed.items() if 0 < sum(r['fifth'] for r in v) < len(v)]}")
    # ptime and left in the 5 treatment-success seeds' cells vs the same seeds' cells in the other arm
# cross arm on the same seeds
byk = {(r['arm'], r['seed'], r['rep']): r for r in rows}
succB = sorted({r['seed'] for r in rows if r['arm']=='B' and r['fifth']}); succA = sorted({r['seed'] for r in rows if r['arm']=='A' and r['fifth']})
print('\nsame-seed cross-arm (rep-matched) parent publication time and allowance:')
for s in succB + succA:
    for rep in range(3):
        a, b = byk[('A', s, rep)], byk[('B', s, rep)]
        print(f"  seed {s} rep {rep}: A ptime {a['ptime']:.2f} left {a['left']:.2f} fifth {a['fifth']} iters {a['iters']} minraw {a['minraw']:.1f} | B ptime {b['ptime']:.2f} left {b['left']:.2f} fifth {b['fifth']} iters {b['iters']} minraw {b['minraw']:.1f}")
# paired ptime B - A over all 90 pairs
d = [byk[('B',s,rep)]['ptime'] - byk[('A',s,rep)]['ptime'] for (arm,s,rep) in byk if arm=='A']
print(f"\nB - A parent publication time over 90 rep-matched pairs: q1/med/q3 {quart(d)[0]:.3f}/{quared(d)[1] if False else quart(d)[1]:.3f}/{quart(d)[2]:.3f}; B earlier in {sum(1 for x in d if x<0)}/90")
# where the failures' minraw is small (near band) vs ptime
for arm in 'AB':
    neg = [r for r in rows if r['arm']==arm and not r['fifth']]
    print(f"arm {arm} failures: minraw <= 5: {sum(1 for r in neg if r['minraw']<=5)}, <= 10: {sum(1 for r in neg if r['minraw']<=10)}, of {len(neg)}; those with minraw<=5: {[(r['seed'], r['rep'], round(r['minraw'],2), round(r['ptime'],2), r['iters']) for r in neg if r['minraw']<=5]}")
