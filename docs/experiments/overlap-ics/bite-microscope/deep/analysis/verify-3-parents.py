"""verify-3-parents.py -- recompute section 3 (releases, parents near 160.6 mm, the origin x objective cross) of README-draft.md.
Usage: cd /var/lib/t3/tmp/astra/deep/analysis && python3 verify-3-parents.py"""
import json, os, statistics as st
from collections import Counter
from verify_common import *
docs = load_docs(); A = attempts(docs); R = load_replays()
def parent_pub(a):
    pubs = [p for p in a['doc']['outcome']['publications'] if abs(p['publishedRawDepthMm'] - a['b']['parentDepthMm']) < 1e-9]
    assert len(pubs) == 1, (a['seed'], a['arm'])
    return pubs[0]
rows = []
for a in A:
    if a['bite'] != 5: continue
    s = a['s']; b = a['b']; pp = parent_pub(a)
    entry = s['entryBlocking']; eids = [r for r, _ in entry]
    shrink = b['parentDepthMm'] - b['targetDepthMm']
    comps = components(eids)
    largest = len([v for v in comps[0] if isinstance(v, int)])
    pieces_in_rows = len(set().union(*[pieces_of(r) for r in eids]))
    rows.append(dict(arm=a['arm'], seed=a['seed'], pub=a['pub'], ptime=pp['wallSeconds'], left=s['wallAtEntry']['leftS'], moved=len(b['cutMoved']),
        rows=len(entry), pair=sum(1 for r in eids if r < PAIRS), edge=sum(1 for r in eids if r >= PAIRS), pieces=pieces_in_rows,
        resmed=st.median([v for _, v in entry]), big=sum(1 for _, v in entry if v >= 0.9 * shrink), ncomp=len(comps), largest=largest,
        bt=bt_span(eids), it=s['iterations'], minraw=s['minRaw'], strikes=s['strikes'], shrink=shrink, initpos=len(b['initialPositive'])))
def q3(v): return '%.2f / %.2f / %.2f' % (q(v, .25), q(v, .5), q(v, .75))
print('## table 3 (q1 / median / q3, statistics.quantiles inclusive)')
for g in [('A', True), ('A', False), ('B', True), ('B', False)]:
    sel = [r for r in rows if (r['arm'], r['pub']) == g]
    print(g, 'n', len(sel), '| ptime', q3([r['ptime'] for r in sel]), '| left', q3([r['left'] for r in sel]), '| moved', q3([r['moved'] for r in sel]), '| rows', q3([r['rows'] for r in sel]),
          '| pair/edge med', med([r['pair'] for r in sel]), '/', med([r['edge'] for r in sel]), '| pieces', q3([r['pieces'] for r in sel]), '| resmed', q3([r['resmed'] for r in sel]),
          '| >=0.9 shrink', q3([r['big'] for r in sel]), '| comps', q3([r['ncomp'] for r in sel]), '| largest', q3([r['largest'] for r in sel]), '| BT', sum(r['bt'] for r in sel), '/', len(sel),
          '| it', q3([r['it'] for r in sel]), '| minraw', q3([r['minraw'] for r in sel]), '| strikes>0', sum(1 for r in sel if r['strikes'] > 0))
print('exclusive-method quartiles for A failed iterations / rows:', st.quantiles([r['it'] for r in rows if r['arm']=='A' and not r['pub']], n=4), st.quantiles([r['rows'] for r in rows if r['arm']=='A' and not r['pub']], n=4))
print('## cut geometry')
print('shrink mm range %.3f-%.3f; cutMoved range %d-%d median %s; entry rows range %d-%d; initialPositive range %d-%d' % (min(r['shrink'] for r in rows), max(r['shrink'] for r in rows), min(r['moved'] for r in rows), max(r['moved'] for r in rows), med([r['moved'] for r in rows]), min(r['rows'] for r in rows), max(r['rows'] for r in rows), min(r['initpos'] for r in rows), max(r['initpos'] for r in rows)))
# translation of the cut: capsule poses vs parent publication poses
cross = 0; pairs_tot = 0; tedge_moved = 0; tedge = 0; rowfreq = Counter(); piecefreq = Counter(); movedfreq = Counter()
transl = []
for a in A:
    if a['bite'] != 5: continue
    s = a['s']; b = a['b']; pp = parent_pub(a); cap = a['doc']['biteMicroscope']['capsules'][b['capsule']]
    mv = set(b['cutMoved'])
    for p in range(61):
        pq = pp['poses'][p]; dx = cap['poses'][p][0] - pq['txMm']; dy = cap['poses'][p][1] - pq['tyMm']; dt = cap['poses'][p][2] - pq['thetaDeg']
        transl.append((p in mv, round(dx, 4), round(dy, 4), round(dt, 4)))
    for r, v in s['entryBlocking']:
        rowfreq[r] += 1
        for p in pieces_of(r): piecefreq[p] += 1
        if r < PAIRS:
            pairs_tot += 1; i, j = decode(r)[1:]
            if (i in mv) != (j in mv): cross += 1
        elif decode(r)[2] == 'T':
            tedge += 1
            if decode(r)[1] in mv: tedge_moved += 1
    for p in mv: movedfreq[p] += 1
print('pair rows crossing the split', cross, 'of', pairs_tot, '; T-edge rows', tedge, 'on moved pieces', tedge_moved)
mvd = [t for t in transl if t[0]]; unm = [t for t in transl if not t[0]]
print('moved pieces: dx set', Counter(t[1] for t in mvd).most_common(3), 'dy set', Counter(t[2] for t in mvd).most_common(3), 'dtheta', Counter(t[3] for t in mvd).most_common(2), '; unmoved: dy', Counter(t[2] for t in unm).most_common(2))
print('distinct entry rows', len(rowfreq), 'max frequency', max(rowfreq.values()))
always = [p for p, c in piecefreq.items() if c == 42]
print('pieces in every entry (42/42):', sorted(always), 'moved by the cut in', {p: movedfreq[p] for p in sorted(always)})
fx = json.load(open('/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json'))
areas = [(pc['paddedBounds']['area'], i) for i, pc in enumerate(fx['pieces'])]
real = [(pc['realBounds']['width'] * pc['realBounds']['height'], i) for i, pc in enumerate(fx['pieces'])]
top12 = {i for _, i in sorted(areas, reverse=True)[:12]}; top12r = {i for _, i in sorted(real, reverse=True)[:12]}
print('top-12 by padded area', sorted(top12), '; always-present pieces among them', sorted(set(always) & top12), '; by real bbox area', sorted(set(always) & top12r))
print('ranks of always-present by padded area', {p: sorted(areas, reverse=True).index((areas[p][0], p)) + 1 for p in always})
# same-seed relatedness
eqfp = 0; eqpl = 0; identical_pose = 0; jac_same = []; jac_cross = []
byseed = {}
for a in A:
    if a['bite'] == 5: byseed.setdefault(a['seed'], {})[a['arm']] = a
for sd, d in byseed.items():
    pa = d['A']['doc']['outcome']['publications']; pb = d['B']['doc']['outcome']['publications']
    if pa[0]['parentFingerprint'] == pb[0]['parentFingerprint']: eqfp += 1
    if pa[0]['placementFingerprint'] == pb[0]['placementFingerprint']: eqpl += 1
    p4a = parent_pub(d['A']); p4b = parent_pub(d['B'])
    identical_pose += sum(1 for i in range(61) if (p4a['poses'][i]['txMm'], p4a['poses'][i]['tyMm'], p4a['poses'][i]['thetaDeg']) == (p4b['poses'][i]['txMm'], p4b['poses'][i]['tyMm'], p4b['poses'][i]['thetaDeg']))
    ea = {r for r, _ in d['A']['s']['entryBlocking']}; eb = {r for r, _ in d['B']['s']['entryBlocking']}
    jac_same.append(len(ea & eb) / len(ea | eb))
seeds = sorted(byseed)
for arm1, arm2 in (('A', 'A'), ('B', 'B'), ('A', 'B')):
    v = []
    for i, s1 in enumerate(seeds):
        for s2 in seeds:
            if s1 == s2: continue
            if arm1 == arm2 and s2 <= s1: continue
            e1 = {r for r, _ in byseed[s1][arm1]['s']['entryBlocking']}; e2 = {r for r, _ in byseed[s2][arm2]['s']['entryBlocking']}
            v.append(len(e1 & e2) / len(e1 | e2))
    jac_cross.append((arm1 + arm2, round(st.median(v), 3)))
print('bite-1 parentFingerprint equal', eqfp, '/21; placementFingerprint equal', eqpl, '/21; identical bite-4 poses', identical_pose, '; same-seed entry Jaccard median %.3f; cross-seed' % st.median(jac_same), jac_cross)
# 180 scored cells
v2 = json.load(open(os.path.join(V2, 'fifth-cut-rows.json')))
B = [r for r in v2 if r['arm'] == 'B']; succ = [r for r in B if r['fifth']]; fail = [r for r in B if not r['fifth']]
sseeds = {r['seed'] for r in succ}
for agg in (st.median, min, st.mean):
    byS = {sd: agg([r['ptime'] for r in B if r['seed'] == sd]) for sd in {r['seed'] for r in B}}
    order = sorted(byS, key=byS.get)
    print('v2 treatment success seeds rank by parent time (%s):' % agg.__name__, sorted(order.index(sd) + 1 for sd in sseeds), 'of', len(order))
latest = max(succ, key=lambda r: r['ptime'])
print('latest success ptime %.3f left %.3f; failures with earlier parent AND more allowance:' % (latest['ptime'], latest['left']), sum(1 for r in fail if r['ptime'] < latest['ptime'] and r['left'] > latest['left']), 'of', len(fail))
# cross table from replays
live = {(a['arm'], a['seed'], a['bite']): a for a in A}
print('## cross table')
for arm, p_other in (('A', 1), ('B', 2)):
    ent = []; ratios = {True: [], False: []}
    for sd in seeds:
        a = live[(arm, sd, 5)]; r = R[(arm, sd, 5, p_other)]
        ratios[a['pub']].append(r['evaluationsTotal'] / a['s']['evaluationsAllWorkers'])
        if r['bandEnteredAtIteration'] is not None:
            ent.append((sd, r['bandEnteredAtIteration'], len(r['iterations']), a['s']['iterations'], round(r['evaluationsToBand']/1e6, 2), round(a['s']['evaluationsAllWorkers']/1e6, 2), 'live pub' if a['pub'] else 'live fail', r['certification']['published']))
        else:
            mx = min(x['maxAfterMm'] for x in r['iterations'])
            if mx < 0.02: print('   near miss', arm, sd, 'p', p_other, 'min max %.4f mm at %.1f M evaluations, min raw %.3f' % (mx, r['evaluationsTotal']/1e6, min(x['rawAfter'] for x in r['iterations'])))
        assert r['horizon'] == {'iterations': a['s']['iterations'], 'kind': 'live'}
    allr = ratios[True] + ratios[False]
    print(arm, 'parents under p=%d: band entries %d/21' % (p_other, len(ent)), ent)
    print('   evaluations ratio replay/live: all %.2f-%.2f median %.2f; failed capsules median %.2f; published median %.2f' % (min(allr), max(allr), st.median(allr), st.median(ratios[False]), st.median(ratios[True])))
    tot = [R[(arm, sd, 5, p_other)]['evaluationsTotal']/1e6 for sd in seeds]
    print('   evaluations total %.1f-%.1f M; failed capsules %.1f-%.1f M; published %.1f-%.1f M' % (min(tot), max(tot), min(R[(arm, sd, 5, p_other)]['evaluationsTotal']/1e6 for sd in seeds if not live[(arm, sd, 5)]['pub']), max(R[(arm, sd, 5, p_other)]['evaluationsTotal']/1e6 for sd in seeds if not live[(arm, sd, 5)]['pub']), min(R[(arm, sd, 5, p_other)]['evaluationsTotal']/1e6 for sd in seeds if live[(arm, sd, 5)]['pub']), max(R[(arm, sd, 5, p_other)]['evaluationsTotal']/1e6 for sd in seeds if live[(arm, sd, 5)]['pub'])))
# live
for arm in 'AB':
    pubs = sorted(round(live[(arm, sd, 5)]['s']['evaluationsAllWorkers']/1e6, 1) for sd in seeds if live[(arm, sd, 5)]['pub'])
    print(arm, 'live band entries', len(pubs), '/21 evaluations', pubs)
# own-objective replays: identity and band entry equal live
own_ok = all(R[(a['arm'], a['seed'], a['bite'], 2 if a['arm']=='A' else 1)]['identityFail'] == 0 for a in A)
print('own-objective replays identity pass on all 50:', own_ok)
# budget-matched
maxA = max(live[('A', sd, 5)]['s']['evaluationsAllWorkers'] for sd in seeds)
print('largest live control budget %.2f M; B entries under p=2 entering within it:' % (maxA/1e6), sum(1 for sd in seeds if R[('B', sd, 5, 2)]['bandEnteredAtIteration'] is not None and R[('B', sd, 5, 2)]['evaluationsToBand'] <= maxA))
bl = sorted(live[('B', sd, 5)]['s']['evaluationsAllWorkers'] for sd in seeds if live[('B', sd, 5)]['pub'])
pred = 0; n14 = 0
for sd in seeds:
    bud = R[('A', sd, 5, 1)]['evaluationsTotal']
    pred += sum(1 for x in bl if x <= bud) / 21
    if bud >= 14.6e6: n14 += 1
print('A entries under p=1 budgets %.1f-%.1f M; predicted entries from B live evaluation-to-band ECDF (over 21) %.2f; A budgets >= 14.6 M: %d, entered: %d' % (min(R[('A', sd, 5, 1)]['evaluationsTotal'] for sd in seeds)/1e6, max(R[('A', sd, 5, 1)]['evaluationsTotal'] for sd in seeds)/1e6, pred, n14, sum(1 for sd in seeds if R[('A', sd, 5, 1)]['evaluationsTotal'] >= 14.6e6 and R[('A', sd, 5, 1)]['bandEnteredAtIteration'] is not None)))
# interaction pattern on B parents
for sd in seeds:
    a = live[('B', sd, 5)]; r2 = R[('B', sd, 5, 2)]['bandEnteredAtIteration'] is not None
    print('  B', sd, 'live p1', a['pub'], 'p2 replay', r2, '| A', 'live p2', live[('A', sd, 5)]['pub'], 'p1 replay', R[('A', sd, 5, 1)]['bandEnteredAtIteration'] is not None)
# control's exceptional seed
sd = '10636268072709740349'
for arm in 'AB':
    a = live[(arm, sd, 5)]; pp = parent_pub(a); eids = [r for r, _ in a['s']['entryBlocking']]; cs = components(eids)
    other = R[(arm, sd, 5, 1 if arm == 'A' else 2)]
    print(arm, sd, 'parent %.2f s, left %.2f s, rows %d, BT %s, edge sides %s, largest comp pieces %d, it %d, pub %s; other-objective replay: ratio %.2f, min raw %.1f, best max %.3f, band %s' % (pp['wallSeconds'], a['s']['wallAtEntry']['leftS'], len(eids), bt_span(eids), Counter(decode(r)[2] for r in eids if r >= PAIRS), len([v for v in cs[0] if isinstance(v, int)]), a['s']['iterations'], a['pub'], other['evaluationsTotal']/a['s']['evaluationsAllWorkers'], min(x['rawAfter'] for x in other['iterations']), min(x['maxAfterMm'] for x in other['iterations']), other['bandEnteredAtIteration']))
# chain of minima and rollbacks
print('## rollbacks (bite 5 failed)')
for arm in 'AB':
    sel = [a for a in A if a['arm']==arm and a['bite']==5 and not a['pub']]
    restored = []; nxt = []; above2 = 0; above5 = 0; n = 0; nwith = 0; all_to_argmin = 0; nrb = 0; patience_ok = 0
    for a in sel:
        s = a['s']; by = {w['iteration']: w for w in s['sweeps']}; raws = [x[1] for x in s['samples']]
        for at, to in s['rollbacks']:
            nrb += 1
            if to == min(range(at + 1), key=lambda i: raws[i]): all_to_argmin += 1
            newmins = [x[0] for x in s['samples'] if x[4] and x[0] <= at]
            if at - max(newmins) == 200: patience_ok += 1
            if at + 1 in by:
                nwith += 1; rv = by[to]['maxAfterMm'] if to in by else s['samples'][to][3]; nv = by[at + 1]['maxAfterMm']
                restored.append(rv); nxt.append(nv)
                if nv > 2 * rv: above2 += 1
                if nv > 5: above5 += 1
    print(arm, 'rollbacks', nrb, 'with a next sweep', nwith, '| restored max med %.2f, next-sweep max med %.2f mm; next > 2x restored on %d; next > 5 mm on %d; restore == argmin raw %d/%d; at - last newmin == 200: %d/%d' % (st.median(restored), st.median(nxt), above2, above5, all_to_argmin, nrb, patience_ok, nrb))
    print('   new minima per attempt med', med([sum(1 for x in a['s']['samples'][1:] if x[4]) for a in sel]), '; chain stops at', med([a['s']['restoredToIteration'] for a in sel]), '[%d-%d]' % (min(a['s']['restoredToIteration'] for a in sel), max(a['s']['restoredToIteration'] for a in sel)))
allrb = sum(len(a['s']['rollbacks']) for a in A)
print('all rollbacks over 50 attempts:', allrb, 'restore to argmin raw:', sum(1 for a in A for at, to in a['s']['rollbacks'] if to == min(range(at+1), key=lambda i: a['s']['samples'][i][1])), '; at - last newmin == 200:', sum(1 for a in A for at, to in a['s']['rollbacks'] if at - max(x[0] for x in a['s']['samples'] if x[4] and x[0] <= at) == 200))
print('## sixth cuts')
for a in A:
    if a['bite'] == 6:
        raws = [x[1] for x in a['s']['samples']]; mxs = [x[3] for x in a['s']['samples']]
        print(a['arm'], a['seed'], 'it', a['s']['iterations'], 'best max %.3f mm at %d, best raw %.2f at %d, stop %s' % (min(mxs), mxs.index(min(mxs)), min(raws), raws.index(min(raws)), a['s']['stop']))
