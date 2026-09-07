"""Per retained attempt: parent, allowance, cut, entry blocking, graph, outcome.
Run: cd /var/lib/t3/tmp/astra/deep/analysis && python3 parents-entry.py
"""
import json, sys, collections
from parents_common import *

docs = load_docs()
rows_out = []
for a in attempts(docs):
    sep, bite, par = a['sep'], a['bite'], a['parent']
    eb = sep['entryBlocking']
    ids = [r for r, _ in eb]; res = [x for _, x in eb]
    kinds = collections.Counter(decode(r)[0] for r in ids)
    edge_sides = collections.Counter(decode(r)[2] for r in ids if decode(r)[0] == 'edge')
    pcs = set()
    for r in ids: pcs.update(row_pieces(r))
    g = graph_summary(ids)
    shrink = bite['parentDepthMm'] - bite['targetDepthMm']
    q = quart(res)
    cm = set(bite['cutMoved'])
    # rows crossing the cut: pair rows with one endpoint moved and one not; edge rows of moved pieces
    cross = sum(1 for r in ids if decode(r)[0]=='pair' and ((decode(r)[1] in cm) != (decode(r)[2] in cm)))
    both_moved = sum(1 for r in ids if decode(r)[0]=='pair' and (decode(r)[1] in cm) and (decode(r)[2] in cm))
    none_moved = sum(1 for r in ids if decode(r)[0]=='pair' and (decode(r)[1] not in cm) and (decode(r)[2] not in cm))
    edge_moved = sum(1 for r in ids if decode(r)[0]=='edge' and decode(r)[1] in cm)
    edge_still = sum(1 for r in ids if decode(r)[0]=='edge' and decode(r)[1] not in cm)
    rec = {
        'arm': a['arm'], 'seed': a['seed'], 'bite': a['ordinal'],
        'parentDepthMm': bite['parentDepthMm'], 'targetDepthMm': bite['targetDepthMm'], 'shrinkMm': shrink,
        'parentBite': par['ordinal']['bite'], 'parentIter': par['ordinal']['iteration'],
        'parentPubS': par['wallSeconds'],
        'entryElapsedS': sep['wallAtEntry']['elapsedS'], 'leftS': sep['wallAtEntry']['leftS'],
        'phaseDeadlineS': sep['wallAtEntry']['phaseDeadlineS'],
        'cutMoved': len(cm), 'cutMovedList': sorted(cm),
        'initialPositivePieces': len(bite['initialPositive']),
        'entryRows': len(ids), 'pairRows': kinds['pair'], 'edgeRows': kinds['edge'],
        'edgeSides': dict(edge_sides),
        'resMin': min(res), 'resQ1': q[0], 'resMed': q[1], 'resQ3': q[2], 'resMax': max(res),
        'rowsGE0.9shrink': sum(1 for x in res if x >= 0.9*shrink),
        'rowsLT1mm': sum(1 for x in res if x < 1.0),
        'pairCross': cross, 'pairBothMoved': both_moved, 'pairNoneMoved': none_moved,
        'edgeMoved': edge_moved, 'edgeStill': edge_still,
        'piecesTouched': len(pcs),
        'components': g['components'], 'compSizes': g['sizes'], 'largest': g['largest_pieces'],
        'largestEdges': g['largestEdges'] if 'largestEdges' in g else g['largest_edges'],
        'spanning': g['spanning'], 'edgeComps': g['touching_edge_components'],
        'published': bite['published'], 'iterations': sep['iterations'], 'stop': sep['stop'],
        'strikes': sep['strikes'], 'rollbacks': len(sep['rollbacks']), 'bandEntries': sep['bandEntries'],
        'minRaw': sep['minRaw'], 'evals': sep['evaluationsAllWorkers'],
        'wallAtStopLeftS': sep['wallAtStop']['leftS'],
    }
    rows_out.append(rec)

json.dump(rows_out, open('/var/lib/t3/tmp/astra/deep/analysis/parents-entry.json', 'w'), indent=1)

def fmt(r):
    return (f"| {r['arm']} | {r['seed']} | {r['bite']} | {r['parentDepthMm']:.3f} | b{r['parentBite']}/i{r['parentIter']} | {r['parentPubS']:.2f} | {r['leftS']:.2f} | "
            f"{r['cutMoved']} | {r['entryRows']} ({r['pairRows']}p/{r['edgeRows']}e) | {r['piecesTouched']} | "
            f"{r['resMin']:.2f}/{r['resMed']:.2f}/{r['resMax']:.2f} | {r['rowsGE0.9shrink']} | {r['pairCross']}/{r['pairBothMoved']}/{r['pairNoneMoved']} | "
            f"{r['components']} | {r['compSizes'][:4]} | {','.join(r['spanning']) or '-'} | "
            f"{'PUB' if r['published'] else 'fail'} | {r['iterations']} | {r['strikes']}/{r['rollbacks']} | {r['minRaw']:.2f} |")

hdr = ("| arm | seed | bite | parent mm | parent pub (bite/iter) | pub s | left s | cut moved | entry rows (pair/edge) | pieces | res min/med/max mm | rows>=0.9 shrink | pair rows cross/both/none moved | comps | comp sizes (pieces) | span | out | iters | strikes/rollbacks | min raw |\n"
       "|---|---|---|---:|---|---:|---:|---:|---|---:|---|---:|---|---:|---|---|---|---:|---|---:|")
print(hdr)
for r in sorted(rows_out, key=lambda r: (r['bite'], r['arm'], not r['published'], r['seed'])):
    print(fmt(r))

# group summaries
def summ(label, rs):
    if not rs: return
    def q3(k): 
        a,b,c = quart([r[k] for r in rs]); return f"{a:.2f}/{b:.2f}/{c:.2f}"
    print(f"{label}: n={len(rs)} parentPubS q1/med/q3 {q3('parentPubS')}; leftS {q3('leftS')}; cutMoved {q3('cutMoved')}; "
          f"entryRows {q3('entryRows')}; pairRows {q3('pairRows')}; edgeRows {q3('edgeRows')}; pieces {q3('piecesTouched')}; "
          f"resMed {q3('resMed')}; rows>=0.9shrink {q3('rowsGE0.9shrink')}; rows<1mm {q3('rowsLT1mm')}; "
          f"pairCross {q3('pairCross')}; comps {q3('components')}; largest {q3('largest')}; "
          f"spanning B-T {sum(1 for r in rs if 'B-T' in r['spanning'])}, L-R {sum(1 for r in rs if 'L-R' in r['spanning'])}; "
          f"iters {q3('iterations')}; minRaw {q3('minRaw')}; strikes>0 {sum(1 for r in rs if r['strikes']>0)}")
print()
for bite in (5, 6):
    for arm in ('A', 'B'):
        for pub in (True, False):
            summ(f"bite {bite} arm {arm} {'published' if pub else 'failed'}", [r for r in rows_out if r['bite']==bite and r['arm']==arm and r['published']==pub])
