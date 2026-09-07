"""verify_common.py -- shared read-only loader for the verify-*.py adversarial checks (stdlib only)."""
import glob, json, os, statistics as st
DEEP = '/var/lib/t3/tmp/astra/deep'
REPLAYS = os.path.join(DEEP, 'replays')
V2 = '/var/lib/t3/tmp/astra/v2'
N = 61; PAIRS = N * (N - 1) // 2; SIDES = 'LRBT'
_PAIR = {}
for i in range(N):
    for j in range(i + 1, N):
        _PAIR[i * N - i * (i + 1) // 2 + (j - i - 1)] = (i, j)
assert len(_PAIR) == PAIRS

def decode(rid):
    if rid < PAIRS: return ('pair',) + _PAIR[rid]
    k = rid - PAIRS; return ('edge', k // 4, SIDES[k % 4])

def verts(rid):
    d = decode(rid)
    return (d[1], d[2]) if d[0] == 'pair' else (d[1], 'E' + d[2])

def pieces_of(rid):
    d = decode(rid)
    return {d[1], d[2]} if d[0] == 'pair' else {d[1]}

def components(rowids):
    parent = {}
    def find(x):
        parent.setdefault(x, x)
        while parent[x] != x:
            parent[x] = parent[parent[x]]; x = parent[x]
        return x
    for rid in rowids:
        a, b = verts(rid)
        ra, rb = find(a), find(b)
        if ra != rb: parent[ra] = rb
    comps = {}
    for v in list(parent): comps.setdefault(find(v), set()).add(v)
    return sorted(comps.values(), key=lambda c: -len([v for v in c if isinstance(v, int)]))

def bt_span(rowids):
    for c in components(rowids):
        if 'EB' in c and 'ET' in c: return True
    return False

def med(v):
    v = [x for x in v if x is not None]
    return st.median(v) if v else None

def q(v, p):
    """quartile by the 'exclusive' median-of-halves rule is NOT used; use statistics.quantiles n=4 inclusive"""
    v = sorted(x for x in v if x is not None)
    if len(v) == 1: return v[0]
    return st.quantiles(v, n=4, method='inclusive')[{0.25: 0, 0.5: 1, 0.75: 2}[p]]

def load_docs():
    docs = []
    for path in sorted(glob.glob(os.path.join(DEEP, 'deep-*-wall10s-s*.json'))):
        d = json.load(open(path))
        arm = os.path.basename(path).split('-')[1]
        d['_arm'] = arm; d['_path'] = path
        docs.append(d)
    assert len(docs) == 42
    return docs

def attempts(docs):
    out = []
    for d in docs:
        for b in d['biteMicroscope']['bites']:
            assert len(b['separations']) == 1
            s = b['separations'][0]
            out.append(dict(arm=d['_arm'], seed=str(d['seed']), bite=b['ordinal'], pub=bool(b['published']), b=b, s=s, doc=d))
    return out

def load_replays():
    out = {}
    for path in sorted(glob.glob(os.path.join(REPLAYS, 'rp-deep-*.json'))):
        n = os.path.basename(path)[:-5].split('-')
        arm, seed, bite, cap, p = n[2], n[4][1:], int(n[5][1:]), int(n[6][1:]), int(n[7][1:])
        out[(arm, seed, bite, p)] = json.load(open(path))['replay']
    assert len(out) == 100
    return out
