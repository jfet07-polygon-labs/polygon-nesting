"""verify-4-ends.py -- recompute section 4 (why the attempt ends) tables and text numbers of README-draft.md.
Usage: cd /var/lib/t3/tmp/astra/deep/analysis && python3 verify-4-ends.py"""
import json, math, statistics as st
from collections import Counter
from verify_common import *
docs = load_docs(); A = attempts(docs); R = load_replays()
def mmm(v, f='%.3f'): return (f + ' / ' + f + ' / ' + f) % (min(v), st.median(v), max(v))
rows = []
for a in A:
    s = a['s']; sm = s['samples']; raws = [x[1] for x in sm]; mxs = [x[3] for x in sm]; n = s['iterations']
    bi = raws.index(min(raws))
    ev_after = sum(w['evaluationsAllWorkers'] for w in s['sweeps'] if w['iteration'] > bi)
    # jump: first iteration with raw > 2x running minimum
    runmin = raws[0]; jump = None
    for i in range(1, n + 1):
        if raws[i] > 2 * runmin: jump = i; break
        runmin = min(runmin, raws[i])
    jinfo = None
    if jump:
        w = s['sweeps'][jump - 1]
        mag = max([math.hypot(x['dxMm'], x['dyMm']) for x in w['relocates'] if x['moved']] or [0])
        magb = [max([math.hypot(x['dxMm'], x['dyMm']) for x in s['sweeps'][i-1]['relocates'] if x['moved']] or [0]) for i in range(1, jump)]
        rm = min(raws[:jump])
        other = any(x['rawAfter'] <= 2 * rm for x in w['workers'] if x['worker'] != w['winner'])
        jinfo = (jump, mag, st.median(magb) if magb else None, other)
    # first raw < 1
    first1 = next((i for i in range(n + 1) if raws[i] < 1), None)
    rows.append(dict(arm=a['arm'], seed=a['seed'], bite=a['bite'], pub=a['pub'], stop=s['stop'], strikes=s['strikes'], rb=len(s['rollbacks']), it=n, ev=s['evaluationsAllWorkers'],
        left_entry=s['wallAtEntry']['leftS'], left_stop=s['wallAtStop']['leftS'], best=min(raws), bi=bi, bestmax=min(mxs), frac=bi / n, it_after=n - bi, ev_after=ev_after,
        last_over_best=(raws[-1] / min(raws)) if min(raws) > 0 else None, last=raws[-1], jump=jinfo, first1=first1, band=s['bandEntries'], exact=s['exactCheckpointCalls'], elapsed=s['wallAtStop']['elapsedS'] - s['wallAtEntry']['elapsedS']))
print('## table 4 (bite 5, 21 per arm)')
for arm in 'AB':
    sel = [r for r in rows if r['arm']==arm and r['bite']==5]; dl = [r for r in sel if r['stop']=='deadline']
    print(arm, 'stops', Counter(r['stop'] for r in sel), '| strikes', sorted(Counter(r['strikes'] for r in sel).items()), '| rollbacks', sorted(Counter(r['rb'] for r in sel).items()))
    print('  iterations', mmm([r['it'] for r in sel], '%d'), '| evaluations M', mmm([r['ev']/1e6 for r in sel], '%.2f'), '| wall left entry', mmm([r['left_entry'] for r in sel]), '| wall left stop (deadline)', '%.3f .. %.3f' % (min(r['left_stop'] for r in dl), max(r['left_stop'] for r in dl)))
    print('  deadline best raw', mmm([r['best'] for r in dl]), '| best max', mmm([r['bestmax'] for r in dl]), '| frac at best', mmm([r['frac'] for r in dl], '%.2f'), '| it after best %d of %d (%.1f %%) / ev after %.1f M of %.1f M (%.1f %%)' % (sum(r['it_after'] for r in dl), sum(r['it'] for r in dl), 100*sum(r['it_after'] for r in dl)/sum(r['it'] for r in dl), sum(r['ev_after'] for r in dl)/1e6, sum(r['ev'] for r in dl)/1e6, 100*sum(r['ev_after'] for r in dl)/sum(r['ev'] for r in dl)), '| last/best', mmm([r['last_over_best'] for r in dl if r['last_over_best'] is not None], '%.1f'))
    print('  deadline stops: band entries', sum(r['band'] for r in dl), 'exact calls', sum(r['exact'] for r in dl), '; last raw > 2x best:', sum(1 for r in dl if r['last_over_best'] > 2), 'of', len(dl))
print('## classification of deadline stops, bite 5')
dl = [r for r in rows if r['bite']==5 and r['stop']=='deadline']
for r in sorted(dl, key=lambda r: (r['arm'], r['frac'])):
    print(' ', r['arm'], r['seed'], 'best %.3f at %d of %d (frac %.2f)' % (r['best'], r['bi'], r['it'], r['frac']), 'last10' if r['bi'] >= r['it'] - r['it'] // 10 else ('50-90' if r['frac'] >= 0.5 else 'early'), '| first raw<1 at', r['first1'], 'left', (r['it'] - r['first1']) if r['first1'] is not None else None, 'ev left %.2f M' % (sum(w['evaluationsAllWorkers'] for w in [a for a in A if a['arm']==r['arm'] and a['seed']==r['seed'] and a['bite']==5][0]['s']['sweeps'] if w['iteration'] > r['first1'])/1e6 if r['first1'] is not None else 0))
for arm in 'AB':
    d = [r for r in dl if r['arm']==arm]
    print(arm, 'last 10 %% (bi >= n - n//10):', sum(1 for r in d if r['bi'] >= r['it'] - r['it']//10), '; last 10 %% (frac >= 0.9):', sum(1 for r in d if r['frac'] >= 0.9), '; [50,90):', sum(1 for r in d if 0.5 <= r['frac'] < 0.9), '; < 50 %:', sum(1 for r in d if r['frac'] < 0.5))
    e = [r for r in d if 5 <= r['bi'] <= 10]
    if e: print(arm, 'best at iteration 5-10:', len(e), 'raw range of those %.1f-%.1f, best max range %.2f-%.2f' % (min(r['best'] for r in e), max(r['best'] for r in e), min(r['bestmax'] for r in e), max(r['bestmax'] for r in e)))
# successes: iterations / evaluations from first raw < 1 to band entry
print('## successes: work from first raw < 1 to band entry')
need = []
for a in A:
    if a['pub']:
        s = a['s']; raws = [x[1] for x in s['samples']]; f1 = next(i for i in range(len(raws)) if raws[i] < 1)
        it_need = s['iterations'] - f1; ev_need = sum(w['evaluationsAllWorkers'] for w in s['sweeps'] if w['iteration'] > f1)
        need.append((it_need, ev_need/1e6, a['arm'], a['seed'])); 
print(sorted(need), 'median iterations', st.median([x[0] for x in need]), 'median M', st.median([x[1] for x in need]))
print('## jump (bite 5 attempts and all)')
for label, sel in (('bite5', [r for r in rows if r['bite']==5]), ('all', rows), ('failed', [r for r in rows if not r['pub']])):
    js = [r['jump'] for r in sel if r['jump']]
    print(label, 'n', len(sel), 'with jump', len(js), 'jump it range %d-%d' % (min(j[0] for j in js), max(j[0] for j in js)), 'mag at jump med %.0f mm [%.0f-%.0f]' % (st.median([j[1] for j in js]), min(j[1] for j in js), max(j[1] for j in js)), 'median mag before med %.1f' % st.median([j[2] for j in js if j[2] is not None]), 'other worker <= 2x runmin:', sum(1 for j in js if j[3]))
print('  A failed 5-10 raw range (deadline, best at 5..10):', sorted(round(r['best'], 1) for r in dl if r['arm']=='A' and 5 <= r['bi'] <= 10))
print('  B deadline best raw sorted:', sorted(round(r['best'], 3) for r in dl if r['arm']=='B'), 'best max sorted', sorted(round(r['bestmax'], 2) for r in dl if r['arm']=='B'))
print('  A deadline best raw sorted:', sorted(round(r['best'], 3) for r in dl if r['arm']=='A'))
print('## replays for section 4')
for arm, p in (('B', 2), ('A', 1)):
    for pub in (False, True):
        sel = [a for a in A if a['arm']==arm and a['bite']==5 and a['pub']==pub]
        ev = [R[(arm, a['seed'], 5, p)]['evaluationsTotal']/1e6 for a in sel]; rat = [R[(arm, a['seed'], 5, p)]['evaluationsTotal']/a['s']['evaluationsAllWorkers'] for a in sel]
        ent = sum(1 for a in sel if R[(arm, a['seed'], 5, p)]['bandEnteredAtIteration'] is not None)
        print(arm, 'capsules under p=%d, live %s: n %d, evaluations %.1f-%.1f M, ratio %.2f-%.2f, band entries %d' % (p, 'published' if pub else 'failed', len(sel), min(ev), max(ev), min(rat), max(rat), ent))
json.dump(rows, open('verify-4-rows.json', 'w'), default=str)
