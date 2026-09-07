#!/usr/bin/env python3
"""The 100 replays: every retained attempt's entry capsule under both objectives
with the live horizon and --certify=1.  Per objective: iterations run,
evaluations, contested fraction, winner evaluation share, the winner's
container-origin moved relocates per iteration and per million evaluations,
band entry and certification.  Per-worker records do not exist in the replays,
so nothing about losers can be read here.  stdlib only."""
import json, glob, os, re, collections, statistics as st
R = '/var/lib/t3/tmp/astra/deep/replays'
rows = []
for path in sorted(glob.glob(os.path.join(R, 'rp-deep-*-wall10s-s*-b*-c*-p*.json'))):
    m = re.match(r'rp-deep-([AB])-wall10s-s(\d+)-b(\d)-c(\d)-p(\d)\.json', os.path.basename(path))
    arm, seed, bite, cap, p = m.group(1), int(m.group(2)), int(m.group(3)), int(m.group(4)), int(m.group(5))
    rp = json.load(open(path))['replay']
    its = rp['iterations']
    n = len(its)
    contested = sum(1 for it in its if it['contested'])
    evAll = sum(it['evaluationsAllWorkers'] for it in its); evWin = sum(it['evaluationsWinner'] for it in its)
    cc = sum(1 for it in its for rel in it['relocates'] if rel['moved'] and rel['origin'] == 'container')
    moved = sum(1 for it in its for rel in it['relocates'] if rel['moved'])
    useful = sum(1 for it in its for rel in it['relocates'] if rel['moved'] and rel['guidedAfter'] < rel['guidedBefore'])
    assert evAll == rp['evaluationsTotal'], path
    own = (arm == 'A' and p == 2) or (arm == 'B' and p == 1)
    rows.append(dict(arm=arm, seed=seed, bite=bite, cap=cap, p=p, own=own, iterations=n, horizon=rp['horizon']['iterations'],
                     stop=rp['stop'], evAll=evAll, evWin=evWin, contested=contested, ccWin=cc, movedWin=moved, usefulWin=useful,
                     band=rp['bandEnteredAtIteration'], evToBand=rp['evaluationsToBand'],
                     certified=rp['certification']['published'], identityPass=rp['identityPass'], identityFail=rp['identityFail'],
                     control=rp['control']))
json.dump(rows, open('/var/lib/t3/tmp/astra/deep/analysis/moves-replays.json', 'w'), indent=1)
print('replays', len(rows))
own = [r for r in rows if r['own']]
print('own-objective identity: pass', sum(r['identityPass'] for r in own), 'fail', sum(r['identityFail'] for r in own), 'over', len(own), 'replays;',
      'other-objective identity pass', sum(r['identityPass'] for r in rows if not r['own']), 'fail', sum(r['identityFail'] for r in rows if not r['own']))
print()
print('# per entry class and objective, pooled over iterations: contested %, winner evaluation share, winner container-origin moved relocates per iteration and per million evaluations, evaluations per iteration; band entries / certified')
print('| entry arm | bite | class | objective | replays | iterations | evaluations | evals/iteration | contested % | winner eval share % | winner container moves | per iteration | per M evaluations | winner moved | useful/moved % | band entries | certified |')
print('|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|')
def cls(r):
    c = r['control']
    return 'live-published' if c['tracedPublished'] else 'live-failed'
G = collections.defaultdict(list)
for r in rows: G[(r['arm'], r['bite'], cls(r), r['p'])].append(r)
for k in sorted(G):
    g = G[k]; S = lambda f: sum(r[f] for r in g)
    n = S('iterations')
    print(f"| {k[0]} | {k[1]} | {k[2]} | p={k[3]}{' (own)' if (k[0]=='A' and k[3]==2) or (k[0]=='B' and k[3]==1) else ' (other)'} | {len(g)} | {n} | {S('evAll')} | {S('evAll')/n:.0f} | {100*S('contested')/n:.1f} | {100*S('evWin')/S('evAll'):.2f} | {S('ccWin')} | {S('ccWin')/n:.2f} | {1e6*S('ccWin')/S('evAll'):.1f} | {S('movedWin')} | {100*S('usefulWin')/S('movedWin'):.1f} | {sum(1 for r in g if r['band'] is not None)} | {sum(1 for r in g if r['certified'])} |")
print()
print('# paired per capsule: the other objective vs the own objective on the identical entry (live horizon = own iteration count)')
print('| arm | seed | bite | own p | own iters | own evals | own band | other iters run | other evals | other band iter | other certified | own cc/iter | other cc/iter | own contested % | other contested % |')
print('|---|---|---|---|---:|---:|---|---:|---:|---|---|---:|---:|---:|---:|')
by = {(r['arm'], r['seed'], r['bite'], r['p']): r for r in rows}
pairs = sorted({(r['arm'], r['seed'], r['bite']) for r in rows}, key=lambda k: (k[0], len(str(k[1])), k[1], k[2]))
otherBand = collections.Counter(); ownBand = collections.Counter()
for arm, seed, bite in pairs:
    po = 2 if arm == 'A' else 1; pt = 3 - po
    a, b = by[(arm, seed, bite, po)], by[(arm, seed, bite, pt)]
    key = (arm, bite, cls(a)); ownBand[key] += a['band'] is not None; otherBand[key] += b['band'] is not None
    print(f"| {arm} | {seed} | {bite} | {po} | {a['iterations']} | {a['evAll']} | {a['band']} | {b['iterations']} | {b['evAll']} | {b['band']} | {b['certified']} | {a['ccWin']/a['iterations']:.2f} | {b['ccWin']/b['iterations']:.2f} | {100*a['contested']/a['iterations']:.1f} | {100*b['contested']/b['iterations']:.1f} |")
print()
print('# band entries per class: own objective vs other objective under the live horizon')
for k in sorted(ownBand): print(' ', k, 'own', ownBand[k], 'other', otherBand[k], 'of', sum(1 for r in own if (r['arm'], r['bite'], cls(r)) == k))
