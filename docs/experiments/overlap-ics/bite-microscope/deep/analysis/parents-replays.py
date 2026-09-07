"""Cross of parent origin and continuation objective from the 100 replays.
Run: cd /var/lib/t3/tmp/astra/deep/analysis && python3 parents-replays.py
"""
import json, os, glob, collections
from parents_common import *

docs = load_docs()
at = attempts(docs)
recs = []
for a in at:
    arm, seed, b, c = a['arm'], a['seed'], a['ordinal'], a['capsule']
    live = {'published': a['bite']['published'], 'iterations': a['sep']['iterations'], 'evals': a['sep']['evaluationsAllWorkers'],
            'bandEntries': a['sep']['bandEntries']}
    own_p = 2 if arm == 'A' else 1
    r = {'arm': arm, 'seed': seed, 'bite': b, 'capsule': c, 'live': live}
    for p in (1, 2):
        path = os.path.join(REPLAYS, f'rp-deep-{arm}-wall10s-s{seed}-b{b}-c{c}-p{p}.json')
        d = json.load(open(path))['replay']
        assert d['horizon']['kind'] == 'live' and d['horizon']['iterations'] == live['iterations'], (path, d['horizon'])
        assert d['capsule']['capturedExponent'] == float(own_p)
        idp = sum(1 for x in d['identity'] if x['equal']); idn = len(d['identity'])
        r['p%d' % p] = {
            'band': d['bandEnteredAtIteration'], 'evToBand': d['evaluationsToBand'], 'evTotal': d['evaluationsTotal'],
            'ran': len(d['iterations']), 'stop': d.get('stop'), 'certAttempted': d['certification']['attempted'],
            'certPublished': d['certification']['published'], 'certDepth': d['certification']['depthMm'],
            'refusal': d['certification']['refusal'], 'identity': f'{idp}/{idn}', 'diverge': d['divergesFromTraceAtIteration'],
            'minRaw': min(x['rawAfter'] for x in d['iterations']), 'lastMax': d['iterations'][-1]['maxAfterMm'],
            'minMax': min(x['maxAfterMm'] for x in d['iterations']),
        }
    r['own'] = r['p%d' % own_p]; r['other'] = r['p%d' % (3 - own_p)]
    recs.append(r)

json.dump(recs, open('/var/lib/t3/tmp/astra/deep/analysis/parents-replays.json', 'w'), indent=1, default=str)

print('| arm | seed | bite | live out | live iters | live evals | own: band it / evals to band / cert pub / identity | other: band it / evals to band / evals total / cert pub / min raw / min max mm |')
print('|---|---|---|---|---:|---:|---|---|')
for r in sorted(recs, key=lambda r: (r['bite'], r['arm'], not r['live']['published'], r['seed'])):
    o, t, L = r['own'], r['other'], r['live']
    print(f"| {r['arm']} | {r['seed']} | {r['bite']} | {'PUB' if L['published'] else 'fail'} | {L['iterations']} | {L['evals']} | "
          f"{o['band']} / {o['evToBand']} / {o['certPublished']} / {o['identity']} | "
          f"{t['band']} / {t['evToBand']} / {t['evTotal']} / {t['certPublished']} / {t['minRaw']:.2f} / {t['minMax']:.3f} |")
print()
print('== own-objective replay reproduces the live outcome?')
for r in recs:
    o, L = r['own'], r['live']
    ok = (o['certPublished'] == L['published']) and ((o['band'] is not None) == (L['bandEntries'] > 0)) and o['identity'].split('/')[0] == o['identity'].split('/')[1]
    if not ok: print('  NOT reproduced:', r['arm'], r['seed'], r['bite'], o, L)
print('  checked', len(recs), 'own replays; identity pass counts:', collections.Counter(r['own']['identity'].split('/')[0]==r['own']['identity'].split('/')[1] for r in recs))
print('  other-objective identity pass counts (expected 0):', collections.Counter(r['other']['identity'] for r in recs).most_common(3))
print()
print('== cross table by (arm, live outcome), bite 5 and bite 6')
for bite in (5, 6):
    for arm in 'AB':
        for pub in (True, False):
            sel = [r for r in recs if r['bite']==bite and r['arm']==arm and r['live']['published']==pub]
            if not sel: continue
            ob = sum(1 for r in sel if r['other']['band'] is not None); oc = sum(1 for r in sel if r['other']['certPublished'])
            wb = sum(1 for r in sel if r['own']['band'] is not None); wc = sum(1 for r in sel if r['own']['certPublished'])
            evr = [r['other']['evTotal']/r['live']['evals'] for r in sel]
            print(f"bite {bite} arm {arm} live {'PUB' if pub else 'fail'} (n={len(sel)}): own replay band {wb}/{len(sel)} cert {wc}/{len(sel)}; "
                  f"OTHER objective (p={'1' if arm=='A' else '2'}) band {ob}/{len(sel)} cert {oc}/{len(sel)}; "
                  f"other/live evaluation ratio q1/med/q3 {quart(evr)[0]:.2f}/{quart(evr)[1]:.2f}/{quart(evr)[2]:.2f}; "
                  f"other min raw q1/med/q3 {quart([r['other']['minRaw'] for r in sel])[0]:.1f}/{quart([r['other']['minRaw'] for r in sel])[1]:.1f}/{quart([r['other']['minRaw'] for r in sel])[2]:.1f}; "
                  f"other min max-violation mm med {quart([r['other']['minMax'] for r in sel])[1]:.3f}")
            for r in sel:
                if r['other']['band'] is not None:
                    print(f"     other reached band: seed {r['seed']} at iteration {r['other']['band']} of {r['live']['iterations']}, evals to band {r['other']['evToBand']} (live own evals {r['live']['evals']}), cert published {r['other']['certPublished']} depth {r['other']['certDepth']} refusal {r['other']['refusal']}")
print()
print('== 2x2 at bite 5: parent origin (arm) x continuation objective -> band within live horizon')
tab = {}
for r in recs:
    if r['bite'] != 5: continue
    for p in (1, 2):
        tab.setdefault((r['arm'], p), [0, 0])
        tab[(r['arm'], p)][0] += 1 if r['p%d'%p]['band'] is not None else 0
        tab[(r['arm'], p)][1] += 1
print('| parent from | continuation p=1 | continuation p=2 |'); print('|---|---|---|')
for arm in 'AB':
    print(f"| arm {arm} ({'p=2' if arm=='A' else 'p=1'} parents) | {tab[(arm,1)][0]}/{tab[(arm,1)][1]} | {tab[(arm,2)][0]}/{tab[(arm,2)][1]} |")
print('same for seeds where the OTHER arm published live (bite 5):')
pubB = {r['seed'] for r in recs if r['bite']==5 and r['arm']=='B' and r['live']['published']}
pubA = {r['seed'] for r in recs if r['bite']==5 and r['arm']=='A' and r['live']['published']}
for r in recs:
    if r['bite']==5 and r['arm']=='A' and r['seed'] in pubB:
        print(f"  A parent of B-success seed {r['seed']}: p=1 band {r['p1']['band']} (evals {r['p1']['evToBand']}, total {r['p1']['evTotal']}, min raw {r['p1']['minRaw']:.1f}), p=2 band {r['p2']['band']}, live iters {r['live']['iterations']}")
    if r['bite']==5 and r['arm']=='B' and r['seed'] in pubA:
        print(f"  B parent of A-success seed {r['seed']}: p=2 band {r['p2']['band']} (evals {r['p2']['evToBand']}, total {r['p2']['evTotal']}, min raw {r['p2']['minRaw']:.1f}), p=1 band {r['p1']['band']}, live iters {r['live']['iterations']}")
print()
print('== horizon asymmetry: p=2 evaluations per iteration / p=1 evaluations per iteration on the same capsule')
ratios = [r['p2']['evTotal']/r['p2']['ran'] / (r['p1']['evTotal']/r['p1']['ran']) for r in recs]
print(f"  q1/med/q3 {quart(ratios)[0]:.2f}/{quart(ratios)[1]:.2f}/{quart(ratios)[2]:.2f} over {len(ratios)} capsules; min {min(ratios):.2f} max {max(ratios):.2f}")
