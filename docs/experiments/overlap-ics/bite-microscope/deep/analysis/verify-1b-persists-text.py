"""verify-1b-persists-text.py -- section 1 text claims that verify-1 flagged: the B-T span at new-minimum states,
the 10 um / 10 mm threshold, container relocation displacement, top-3 chain at the control's handed-back states,
the treatment's large handed-back rows, re-formation variants.
Usage: cd /var/lib/t3/tmp/astra/deep/analysis && python3 verify-1b-persists-text.py"""
import math, statistics as st
from collections import Counter
from verify_common import *
docs = load_docs(); A = attempts(docs)
# (1) B-T span at every new-minimum state: which A attempts fail, and at which iterations
for a in A:
    if a['pub']: continue
    s = a['s']; bad = []
    for x in s['samples'][1:]:
        if x[4] and not bt_span({r for r, _ in s['sweeps'][x[0]-1]['blocking']}): bad.append(x[0])
    if bad: print('no B-T span at new-minimum iterations', a['arm'], a['seed'], 'bite', a['bite'], bad, 'of newmins', [x[0] for x in s['samples'][1:] if x[4]], 'restoredTo', s['restoredToIteration'])
# (2) threshold 10 um vs 10 mm
tot = big_um = big_mm = 0
for a in A:
    for w in a['s']['sweeps']:
        tot += 1
        m = max([r for _, r in w['blocking']], default=0)
        if m > 0.010: big_um += 1
        if m > 10: big_mm += 1
print('sweeps', tot, '> 10 um', big_um, '> 10 mm', big_mm)
# (3) container relocations: displacement median and fraction of sweeps with >= 1 container relocate (median over attempts)
for arm in 'AB':
    sel = [a for a in A if a['arm']==arm]
    d = [math.hypot(x['dxMm'], x['dyMm']) for a in sel for w in a['s']['sweeps'] for x in w['relocates'] if x['origin']=='container']
    fr = [sum(1 for w in a['s']['sweeps'] if any(x['origin']=='container' for x in w['relocates'])) / len(a['s']['sweeps']) for a in sel]
    print(arm, 'winner container relocates n %d median displacement %.1f mm; fraction of sweeps with >= 1 winner container relocate: median over attempts %.2f [%.2f-%.2f], pooled %.3f' % (len(d), st.median(d), st.median(fr), min(fr), max(fr), sum(1 for a in sel for w in a['s']['sweeps'] if any(x['origin']=='container' for x in w['relocates'])) / sum(len(a['s']['sweeps']) for a in sel)))
    for g in [(5, False), (5, True), (6, False)]:
        s2 = [a for a in sel if (a['bite'], a['pub']) == g]
        if not s2: continue
        d = [math.hypot(x['dxMm'], x['dyMm']) for a in s2 for w in a['s']['sweeps'] for x in w['relocates'] if x['origin']=='container']
        fr = [sum(1 for w in a['s']['sweeps'] if any(x['origin']=='container' for x in w['relocates'])) / len(a['s']['sweeps']) for a in s2]
        print('   ', g, 'container disp median %.1f; frac sweeps med %.2f' % (st.median(d), st.median(fr)))
# (4) top-3 residual rows at the handed-back state of the A failed fifth cuts: do they form a B-piece-piece-T chain?
chain = 0
for a in A:
    if a['arm']=='A' and a['bite']==5 and not a['pub']:
        top = sorted(a['s']['stopBlocking'], key=lambda x: -x[1])[:3]
        ids = {r for r, _ in top}; ok = bt_span(ids)
        chain += ok
        print('  A', a['seed'], [(decode(r), round(v*1000)) for r, v in top], 'B-T chain', ok)
print('A failed: top-3 rows form a B..T chain on', chain, 'of 20')
# (5) B failed handed-back states: rows above 3 mm, and the max
for a in A:
    if a['arm']=='B' and a['bite']==5 and not a['pub']:
        big = sorted([(round(v*1000), decode(r)) for r, v in a['s']['stopBlocking'] if v > 3.0], reverse=True)
        print('  B', a['seed'], 'rows > 3 mm:', big, 'max um', round(max(v for _, v in a['s']['stopBlocking'])*1000), 'n rows', len(a['s']['stopBlocking']))
# (6) most persistent row per attempt: fraction of the attempt, kind, created
for g in [('A', 5, False), ('B', 5, False)]:
    out = []
    for a in [a for a in A if (a['arm'], a['bite'], a['pub']) == g]:
        pres = Counter()
        for w in a['s']['sweeps']:
            for r, _ in w['blocking']: pres[r] += 1
        rid, n = pres.most_common(1)[0]
        eids = {r for r, _ in a['s']['entryBlocking']}
        out.append((round(n / a['s']['iterations'], 3), decode(rid), rid not in eids, a['seed']))
    print(g, sorted(out, reverse=True))
# (7) re-formation variants
for g in [('A', 5, False), ('B', 5, False)]:
    sel = [a for a in A if (a['arm'], a['bite'], a['pub']) == g]
    for variant in ('exclude at+1', 'exclude at', 'exclude both', 'none'):
        ev = []; er = []
        for a in sel:
            s = a['s']; eids = {r for r, _ in s['entryBlocking']}
            excl = set()
            for at, to in s['rollbacks']:
                if variant in ('exclude at+1', 'exclude both'): excl.add(at + 1)
                if variant in ('exclude at', 'exclude both'): excl.add(at)
            prev = eids; ever = set(eids); n = 0; ent = set()
            for it, w in enumerate(s['sweeps'], 1):
                sset = {r for r, _ in w['blocking']}
                for r in sset - prev:
                    if r in ever and it not in excl:
                        n += 1
                        if r in eids: ent.add(r)
                ever |= sset; prev = sset
            ev.append(n); er.append(len(ent))
        print(g, variant, 'events med', st.median(ev), 'entry rows re-formed med', st.median(er))
