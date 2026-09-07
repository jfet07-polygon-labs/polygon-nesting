"""verify-3b-rollbacks.py -- per-attempt rollback counts (section 4 table row), the patience rule under
'200 iterations since the later of the last new minimum and the last rollback', and the B-T spanning component
of the treatment's parent on the control's exceptional seed.
Usage: cd /var/lib/t3/tmp/astra/deep/analysis && python3 verify-3b-rollbacks.py"""
from collections import Counter
from verify_common import *
docs = load_docs(); A = attempts(docs)
for arm in 'AB':
    for bite in (5, 6):
        sel = [a for a in A if a['arm']==arm and a['bite']==bite]
        print(arm, 'bite', bite, 'rollbacks per attempt', sorted(Counter(len(a['s']['rollbacks']) for a in sel).items()), 'total', sum(len(a['s']['rollbacks']) for a in sel), '; failed only total', sum(len(a['s']['rollbacks']) for a in sel if not a['pub']))
        for a in sel:
            if a['arm']=='A' and a['bite']==5: print('   ', a['seed'], 'pub' if a['pub'] else 'fail', a['s']['rollbacks'])
ok = 0; tot = 0; bad = []
for a in A:
    s = a['s']; prev_rb = 0
    for at, to in s['rollbacks']:
        tot += 1
        last_nm = max(x[0] for x in s['samples'] if x[4] and x[0] <= at)
        base = max(last_nm, prev_rb)
        if at - base == 200: ok += 1
        else: bad.append((a['arm'], a['seed'], a['bite'], at, to, last_nm, prev_rb))
        prev_rb = at
print('patience: at - max(last new minimum, last rollback) == 200 on', ok, 'of', tot, bad)
a = [a for a in A if a['arm']=='B' and a['seed']=='10636268072709740349' and a['bite']==5][0]
cs = components([r for r, _ in a['s']['entryBlocking']])
for c in cs:
    if 'EB' in c and 'ET' in c: print('B 1063.. B-T spanning component pieces', len([v for v in c if isinstance(v, int)]), '; largest component pieces', len([v for v in cs[0] if isinstance(v, int)]))
