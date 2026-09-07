"""verify-1-persists.py -- recompute section 1 (what persists) tables and text numbers of README-draft.md.
Usage: cd /var/lib/t3/tmp/astra/deep/analysis && python3 verify-1-persists.py"""
import json, math, statistics as st
from collections import Counter
from verify_common import *
docs = load_docs(); A = attempts(docs)
KS = [1, 2, 3, 5, 10, 20, 50, 100]

def kinds(rows):
    pair = sum(1 for r, _ in rows if r < PAIRS)
    B = sum(1 for r, _ in rows if r >= PAIRS and decode(r)[2] == 'B')
    T = sum(1 for r, _ in rows if r >= PAIRS and decode(r)[2] == 'T')
    return pair, B, T

rows = []
for a in A:
    s = a['s']; b = a['b']
    sweeps = s['sweeps']; assert [w['iteration'] for w in sweeps] == list(range(1, s['iterations'] + 1))
    entry = s['entryBlocking']; stop = s['stopBlocking']; last = sweeps[-1]['blocking']
    eids = {r for r, _ in entry}; sids = {r for r, _ in stop}; lids = {r for r, _ in last}
    rt = s['restoredToIteration'] if not a['pub'] else s['iterations']
    if a['pub']: assert s['restoredToIteration'] is None or s['restoredToIteration'] == s['iterations'], (a['seed'], s['restoredToIteration'])
    sets = [{r for r, _ in w['blocking']} for w in sweeps]
    # continuous
    cont = set(eids); cont_at = {}; cont_rt = None
    for it, sset in enumerate(sets, 1):
        cont &= sset
        if it in KS: cont_at[it] = len(cont)
        if it == rt: cont_rt = set(cont)
    if cont_rt is None: cont_rt = set(cont)  # rt == 0? never
    present = eids & sids; continuous = present & cont_rt
    # pieces in entry rows
    ep = set().union(*[pieces_of(r) for r in eids]); sp = set().union(*[pieces_of(r) for r in sids]) if sids else set(); lp = set().union(*[pieces_of(r) for r in lids]) if lids else set()
    # components
    ce = components(eids); cs = components(sids)
    le = {v for v in ce[0] if isinstance(v, int)} if ce else set(); ls = {v for v in cs[0] if isinstance(v, int)} if cs else set()
    # re-formation events (organic = not at a rollback restore iteration)
    rb_at = {at + 1 for at, to in s['rollbacks']}  # the sweep after the rollback shows the restored state
    prev = eids; seen = set(eids); ever_present = set(eids); released = set()
    events = 0; organic = 0; entry_reformed = set()
    for it, sset in enumerate(sets, 1):
        for r in sset - prev:
            if r in ever_present:
                events += 1
                if it not in rb_at:
                    organic += 1
                    if r in eids: entry_reformed.add(r)
        ever_present |= sset; prev = sset
    # persistence of rows: sweeps present
    pres = Counter()
    for sset in sets:
        for r in sset: pres[r] += 1
    top = pres.most_common(1)[0]
    rows.append(dict(arm=a['arm'], seed=a['seed'], bite=a['bite'], pub=a['pub'], it=s['iterations'], rt=rt,
        entry_n=len(entry), entry_k=kinds(entry), entry_max=max(r for _, r in entry) * 1000,
        stop_n=len(stop), stop_k=kinds(stop), stop_max=max([r for _, r in stop], default=0) * 1000,
        present=len(present), continuous=len(continuous), reformed=len(present) - len(continuous), created=len(sids - eids),
        bt_entry=bt_span(eids), bt_stop=bt_span(sids), bt_last=bt_span(lids),
        ep=len(ep), ep_stop=len(ep & sp), ep_last=len(ep & lp), largest_entry=len(le), largest_stop=len(ls), shared=len(le & ls),
        cont_at=cont_at, events=events, organic=organic, entry_reformed=len(entry_reformed),
        top_row=top[0], top_frac=top[1] / s['iterations'], top_kind=decode(top[0]), top_created=top[0] not in eids,
        stop_top3=sorted(stop, key=lambda x: -x[1])[:3], n_newmin=sum(1 for x in s['samples'][1:] if x[4]),
        initpos=len(b['initialPositive'])))

def fm(v, d=1):
    return ('%.' + str(d) + 'f') % v if isinstance(v, float) and v != int(v) else str(int(v)) if isinstance(v, (int, float)) else str(v)
def mr(v, d=1): return f"{fm(med(v), d)} [{fm(min(v), d)}-{fm(max(v), d)}]"
groups = [('A', 5, False), ('A', 5, True), ('B', 5, False), ('B', 5, True), ('B', 6, False), ('A', 6, False)]
print('## table 1')
for g in groups:
    sel = [r for r in rows if (r['arm'], r['bite'], r['pub']) == g]
    if not sel: continue
    print(f"{g} n={len(sel)} | it {mr([r['it'] for r in sel])} | entry {med([r['entry_n'] for r in sel])} (initialPositive {med([r['initpos'] for r in sel])}) ({med([r['entry_k'][0] for r in sel])} / {med([r['entry_k'][1] for r in sel])} / {med([r['entry_k'][2] for r in sel])}; {med([r['entry_max'] for r in sel]):.0f}) | handed-back it {mr([r['rt'] for r in sel])} | rows there {mr([r['stop_n'] for r in sel])} ({med([r['stop_k'][0] for r in sel])} / {med([r['stop_k'][1] for r in sel])} / {med([r['stop_k'][2] for r in sel])}; {mr([r['stop_max'] for r in sel])}) | entry rows there {mr([r['present'] for r in sel])} = {mr([r['continuous'] for r in sel])} + {mr([r['reformed'] for r in sel])} | created {mr([r['created'] for r in sel])} | BT {sum(r['bt_entry'] for r in sel)}/{len(sel)} / {sum(r['bt_stop'] for r in sel)}/{len(sel)} / {sum(r['bt_last'] for r in sel)}/{len(sel)} | pieces {med([r['ep'] for r in sel])} -> {med([r['ep_stop'] for r in sel])} / {med([r['ep_last'] for r in sel])}")
print('## table 1b continuous up to sweep k (median; max over attempts in parens)')
for g in groups:
    sel = [r for r in rows if (r['arm'], r['bite'], r['pub']) == g]
    if not sel: continue
    print(g, ' '.join(f"k={k}: {fm(med([r['cont_at'].get(k) for r in sel]))} (max {max(r['cont_at'].get(k, 0) for r in sel)}, n {sum(1 for r in sel if k in r['cont_at'])})" for k in KS))
print('## text numbers')
for g in [('A', 5, False), ('B', 5, False), ('B', 6, False)]:
    sel = [r for r in rows if (r['arm'], r['bite'], r['pub']) == g]
    print(g, 'organic re-formation events per attempt med', med([r['organic'] for r in sel]), 'all events', med([r['events'] for r in sel]),
          '; entry rows re-formed organically med', med([r['entry_reformed'] for r in sel]), 'of entry', med([r['entry_n'] for r in sel]),
          '; largest comp handed-back med', med([r['largest_stop'] for r in sel]), 'shares with entry largest med', med([r['shared'] for r in sel]), 'entry largest med', med([r['largest_entry'] for r in sel]),
          '; newmin count med', med([r['n_newmin'] for r in sel]),
          '; top row frac max %.3f' % max(r['top_frac'] for r in sel), 'kinds', Counter((r['top_kind'][0], r['top_kind'][2] if r['top_kind'][0]=='edge' else '') for r in sel), 'created', sum(r['top_created'] for r in sel), '/', len(sel))
    print('   stop max um sorted:', sorted(round(r['stop_max']) for r in sel))
print('A failed handed-back iteration in 5..10:', sum(1 for r in rows if r['arm']=='A' and r['bite']==5 and not r['pub'] and 5 <= r['rt'] <= 10), 'of 20; rts', sorted(r['rt'] for r in rows if r['arm']=='A' and r['bite']==5 and not r['pub']))
r = [r for r in rows if r['arm']=='A' and r['seed']=='11151532166486038253' and r['bite']==5][0]
print('A 1115.. stop top3:', [(decode(x[0]), round(x[1]*1000)) for x in r['stop_top3']])
# all-attempt claims: no entry row continuous at sweep 50 (A) / 20 (B)
for arm, k in (('A', 50), ('B', 20)):
    sel = [r for r in rows if r['arm']==arm and k in r['cont_at']]
    print(arm, f'max continuous at k={k} over {len(sel)} attempts:', max(r['cont_at'][k] for r in sel), '; at k=20 (A) / k=10 (B):', max(r['cont_at'].get(20 if arm=='A' else 10, 0) for r in rows if r['arm']==arm))
# BT span at every new-minimum state of every failed attempt
for g in [('A', 5, False), ('B', 5, False), ('B', 6, False), ('A', 6, False)]:
    cnt = 0; sel = [a for a in A if (a['arm'], a['bite'], a['pub']) == g]
    for a in sel:
        s = a['s']; ok = True
        for x in s['samples'][1:]:
            if x[4]:
                if not bt_span({r for r, _ in s['sweeps'][x[0]-1]['blocking']}): ok = False
        if ok: cnt += 1
    print(g, 'BT span at every new-minimum state:', cnt, '/', len(sel))
# published stops: BT absent
print('published stops BT:', [(a['arm'], a['seed'], bt_span({r for r,_ in a['s']['stopBlocking']})) for a in A if a['pub']])
# end-of-sweep states with a row > 10 mm; winner relocation
tot = 0; big = 0; wdisp = {'A': [], 'B': []}; wfrac = {'A': [], 'B': []}
for a in A:
    for w in a['s']['sweeps']:
        tot += 1
        if any(r > 10 for _, r in w['blocking']): big += 1
        d = [math.hypot(x['dxMm'], x['dyMm']) for x in w['relocates'] if x['moved']]
        if d: wdisp[a['arm']].append(max(d))
    fr = sum(1 for w in a['s']['sweeps'] if any(math.hypot(x['dxMm'], x['dyMm']) > 100 for x in w['relocates'] if x['moved'])) / len(a['s']['sweeps'])
    wfrac[a['arm']].append(fr)
print('sweeps', tot, 'with a row > 10 mm', big)
for arm in 'AB':
    v = wdisp[arm]; print(arm, 'winner max displacement per sweep: median %.1f mm; frac of sweeps with max disp > 100 mm med %.2f; frac > 10 mm computed next' % (st.median(v), st.median(wfrac[arm])))
for arm in 'AB':
    allsw = [w for a in A if a['arm']==arm for w in a['s']['sweeps']]
    d = [math.hypot(x['dxMm'], x['dyMm']) for w in allsw for x in w['relocates'] if x['moved']]
    print(arm, 'all winner moved relocates median disp %.2f mm, n %d; frac of sweeps with any moved relocate > 10 mm %.3f, > 100 mm %.3f, > 500 mm %.3f' % (st.median(d), len(d),
        sum(1 for w in allsw if any(math.hypot(x['dxMm'], x['dyMm']) > 10 for x in w['relocates'] if x['moved'])) / len(allsw),
        sum(1 for w in allsw if any(math.hypot(x['dxMm'], x['dyMm']) > 100 for x in w['relocates'] if x['moved'])) / len(allsw),
        sum(1 for w in allsw if any(math.hypot(x['dxMm'], x['dyMm']) > 500 for x in w['relocates'] if x['moved'])) / len(allsw)))
    # median over sweeps of the winner's max displacement, and fraction of sweeps whose max displacement > 10 mm
    mx = [max([math.hypot(x['dxMm'], x['dyMm']) for x in w['relocates'] if x['moved']] or [0]) for w in allsw]
    print('   median of per-sweep max displacement %.1f mm; sweeps with max disp > 10 mm %.3f; > 1 m %.3f' % (st.median(mx), sum(1 for m in mx if m > 10)/len(mx), sum(1 for m in mx if m > 1000)/len(mx)))
json.dump(rows, open('verify-1-rows.json', 'w'), default=str)
