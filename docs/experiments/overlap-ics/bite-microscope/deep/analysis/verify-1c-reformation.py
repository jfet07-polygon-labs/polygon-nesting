"""verify-1c-reformation.py -- re-formation under the analysts' episode definition (episodes counted over sweeps
1..n only; the entry state is not an episode; 'organic' = an episode not starting on the sweep after a rollback),
and variants of the v2-vs-microscope iteration agreement statistic.
Usage: cd /var/lib/t3/tmp/astra/deep/analysis && python3 verify-1c-reformation.py"""
import json, os, statistics as st
from verify_common import *
docs = load_docs(); A = attempts(docs)
for g in [('A', 5, False), ('B', 5, False)]:
    ev_o = []; ev_t = []; er_o = []; er_mine = []
    for a in [a for a in A if (a['arm'], a['bite'], a['pub']) == g]:
        s = a['s']; eids = {r for r, _ in s['entryBlocking']}; restore_starts = {at + 1 for at, _ in s['rollbacks']}
        present = {}
        for w in s['sweeps']:
            for rid, _ in w['blocking']: present.setdefault(rid, set()).add(w['iteration'])
        tot = org = 0; ent = set(); ent_mine = set()
        for rid, its in present.items():
            starts = [i for i in range(1, s['iterations'] + 1) if i in its and (i - 1) not in its]
            reform = starts[1:]
            tot += len(reform); o = [e for e in reform if e not in restore_starts]; org += len(o)
            if o and rid in eids: ent.add(rid)
            # with the entry state as the first episode: an entry row absent at sweep 1 and back later re-formed
            if rid in eids:
                starts2 = [i for i in range(1, s['iterations'] + 1) if i in its and (i - 1) not in its and not (i == 1)]
                if [e for e in starts2 if e not in restore_starts]: ent_mine.add(rid)
        ev_t.append(tot); ev_o.append(org); er_o.append(len(ent)); er_mine.append(len(ent_mine))
    print(g, 'analyst definition: events total med', st.median(ev_t), 'organic med', st.median(ev_o), 'entry rows re-formed organically med', st.median(er_o), '| counting the entry state as the first episode:', st.median(er_mine))
v2 = json.load(open(os.path.join(V2, 'fifth-cut-rows.json')))
seeds12 = sorted({a['seed'] for a in A if len(a['seed']) > 3})
pairmax = []; evrel = []; med_of_reps_signed = []
for arm in 'AB':
    for sd in seeds12:
        m = [a for a in A if a['arm']==arm and a['seed']==sd and a['bite']==5][0]
        cells = [r for r in v2 if r['arm']==arm and str(r['seed'])==sd]
        pairmax.append(max(abs(r['iters'] - m['s']['iterations']) / m['s']['iterations'] for r in cells))
        evrel += [abs(r['ev'] - m['s']['evaluationsAllWorkers']) / m['s']['evaluationsAllWorkers'] for r in cells]
        med_of_reps_signed.append(abs(st.median([r['iters'] for r in cells]) - m['s']['iterations']) / st.median([r['iters'] for r in cells]))
print('v2 agreement variants: per-pair max |diff| median %.4f; per-cell evaluations |diff| median %.4f; |median reps - mic|/median reps median %.4f; per-cell mean %.4f' % (st.median(pairmax), st.median(evrel), st.median(med_of_reps_signed), st.mean(pairmax)))
