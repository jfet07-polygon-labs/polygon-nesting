"""Shared loader for the parents-*.py analyses (stdlib only, read-only).

Row id scheme (biteMicroscope.rowIdScheme, pairCount 1830, 61 pieces):
  id < 1830          -> pair row (i, j), i<j, index = i*n - i*(i+1)/2 + (j-i-1)
  id >= 1830         -> (id-1830) = piece*4 + side, side in L,R,B,T
A blocking row is a row with residual > 0; the blocking graph has the pieces and
the four strip edges as vertices, the blocking rows as edges.
"""
import json, os, glob, statistics

DEEP = '/var/lib/t3/tmp/astra/deep'
REPLAYS = os.path.join(DEEP, 'replays')
N = 61
PAIRS = N * (N - 1) // 2
SIDES = ['L', 'R', 'B', 'T']
SUCCESS_B = {4872519857840070441, 5671471283886933426, 11151532166486038253,
             17316774662183274765, 18390115156762500293}
SUCCESS_A = {10636268072709740349}
V2_SEEDS = SUCCESS_B | SUCCESS_A | {12153648923418518200, 14782797682586776746,
            2586638353860241226, 6185102792240140886, 7797222088981460957,
            8755853977552987277}

_pair_lut = None
def decode(row_id):
    """-> ('pair', i, j) or ('edge', piece, side)"""
    global _pair_lut
    if row_id >= PAIRS:
        k = row_id - PAIRS
        return ('edge', k // 4, SIDES[k % 4])
    if _pair_lut is None:
        _pair_lut = {}
        for i in range(N):
            for j in range(i + 1, N):
                _pair_lut[i * N - i * (i + 1) // 2 + (j - i - 1)] = (i, j)
    i, j = _pair_lut[row_id]
    return ('pair', i, j)

def row_vertices(row_id):
    d = decode(row_id)
    if d[0] == 'pair':
        return d[1], d[2]
    return d[1], 'edge:' + d[2]

def row_pieces(row_id):
    d = decode(row_id)
    return [d[1], d[2]] if d[0] == 'pair' else [d[1]]

def components(rows):
    """rows: iterable of row ids. -> list of sets of vertices (pieces as ints,
    edges as 'edge:X'), sorted by size descending."""
    parent = {}
    def find(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x
    def union(a, b):
        parent.setdefault(a, a); parent.setdefault(b, b)
        ra, rb = find(a), find(b)
        if ra != rb: parent[ra] = rb
    for r in rows:
        a, b = row_vertices(r)
        union(a, b)
    comps = {}
    for v in parent:
        comps.setdefault(find(v), set()).add(v)
    return sorted(comps.values(), key=lambda s: -len(s))

def graph_summary(rows):
    comps = components(rows)
    def piece_count(c): return sum(1 for v in c if not isinstance(v, str))
    spans = []
    for c in comps:
        e = {v for v in c if isinstance(v, str)}
        if 'edge:B' in e and 'edge:T' in e: spans.append('B-T')
        if 'edge:L' in e and 'edge:R' in e: spans.append('L-R')
    return {
        'components': len(comps),
        'sizes': [piece_count(c) for c in comps],
        'largest_pieces': piece_count(comps[0]) if comps else 0,
        'largest_edges': sorted(v[5:] for v in comps[0] if isinstance(v, str)) if comps else [],
        'spanning': spans,
        'touching_edge_components': sum(1 for c in comps if any(isinstance(v, str) for v in c)),
    }

def load_docs():
    docs = {}
    for path in sorted(glob.glob(os.path.join(DEEP, 'deep-*-wall10s-s*.json'))):
        name = os.path.basename(path)
        arm = name.split('-')[1]
        seed = int(name.split('-s')[1].split('.')[0])
        docs[(arm, seed)] = json.load(open(path))
    return docs

def parent_publication(doc, parent_depth):
    """The explore publication whose published depth equals the bite's parentDepthMm."""
    hits = [p for p in doc['outcome']['publications']
            if p['phase'] == 'explore' and p['publishedRawDepthMm'] == parent_depth]
    assert len(hits) == 1, (parent_depth, len(hits))
    return hits[0]

def attempts(docs):
    """Yield one record per retained attempt (arm, seed, bite ordinal, attempt)."""
    out = []
    for (arm, seed), doc in sorted(docs.items()):
        m = doc['biteMicroscope']
        for bite in m['bites']:
            parent = parent_publication(doc, bite['parentDepthMm'])
            for sep in bite['separations']:
                out.append({
                    'arm': arm, 'seed': seed, 'doc': doc, 'bite': bite, 'sep': sep,
                    'parent': parent, 'ordinal': bite['ordinal'],
                    'capsule': sep['capsule'], 'attempt': sep['attempt'],
                })
    return out

def med(xs):
    xs = list(xs)
    return statistics.median(xs) if xs else float('nan')

def quart(xs):
    xs = sorted(xs)
    if not xs: return (float('nan'),)*3
    n = len(xs)
    def q(p):
        k = (n - 1) * p
        f = int(k); c = min(f + 1, n - 1)
        return xs[f] + (xs[c] - xs[f]) * (k - f)
    return q(0.25), q(0.5), q(0.75)
