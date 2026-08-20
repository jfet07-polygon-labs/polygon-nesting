#!/usr/bin/env python3
"""The README's tables, from the battery documents.

    summarize.py OUT BATTERY [BATTERY ...]

Prints, per request and per budget tier: best and worst published depth per
seed and arm, the paired per-round delta with its within-arm spread beside it,
the coordinator's own wall and its overruns, and the m34 slice accounting -
count, first-slice wall, sterile slices, probe aborts.

The within-arm spread is printed next to every between-arm delta because the
box is shared with another measurement agent: a between-arm number smaller than
the within-arm spread is not a finding.
"""
import json
import statistics
import sys


def main():
    out = sys.argv[1]
    documents = [json.load(open(path)) for path in sys.argv[2:]]
    report = {'requests': {}}
    for doc in documents:
        request = doc['request']
        rows = doc['rows']
        arms = [arm['label'] for arm in doc['arms']]
        tiers = sorted({arm['budget'] for arm in doc['arms']},
                       key=lambda b: int(b.split('=')[1]))
        entry = report['requests'].setdefault(request, {})
        for tier in tiers:
            labels = sorted(arm['label'] for arm in doc['arms']
                            if arm['budget'] == tier)
            if len(labels) < 2:
                continue
            # The baseline is the first label; the arm specs are named so that
            # sorting puts HEAD first (`ahead...`, `bnew...`, `cprobe...`).
            head = labels[0]
            by_key = {(r['arm'], r['seed'], r['round']): r for r in rows}
            seeds = sorted({r['seed'] for r in rows})
            rounds = sorted({r['round'] for r in rows})
            cell = {
                'arms': labels,
                'baseline': head,
                'perSeed': {
                    str(seed): {
                        label: sorted(
                            by_key[(label, seed, r)]['engineDepthMm']
                            for r in rounds if (label, seed, r) in by_key)
                        for label in labels}
                    for seed in seeds},
                'pairedDeltaMm': {},
                'withinArmSpreadMm': {},
            }
            for label in labels:
                spread = []
                for seed in seeds:
                    values = [by_key[(label, seed, r)]['engineDepthMm']
                              for r in rounds if (label, seed, r) in by_key
                              and by_key[(label, seed, r)]['engineDepthMm']
                              is not None]
                    if values:
                        spread.append(max(values) - min(values))
                cell['withinArmSpreadMm'][label] = max(spread) if spread \
                    else None
            for label in labels[1:]:
                deltas = []
                for seed in seeds:
                    for rnd in rounds:
                        a = by_key.get((head, seed, rnd))
                        b = by_key.get((label, seed, rnd))
                        if not a or not b:
                            continue
                        if a['engineDepthMm'] is None \
                                or b['engineDepthMm'] is None:
                            continue
                        deltas.append(b['engineDepthMm'] - a['engineDepthMm'])
                cell['pairedDeltaMm'][f'{label}-minus-{head}'] = {
                    'n': len(deltas),
                    'median': statistics.median(deltas) if deltas else None,
                    'min': min(deltas) if deltas else None,
                    'max': max(deltas) if deltas else None,
                    'armBetter': sum(1 for d in deltas if d < 0),
                    'armWorse': sum(1 for d in deltas if d > 0),
                    'equal': sum(1 for d in deltas if d == 0)}
            for label in labels:
                arm_rows = [r for r in rows if r['arm'] == label]
                walls = [r['coordinatorSeconds'] for r in arm_rows
                         if r.get('coordinatorSeconds') is not None]
                overs = [r['overrunSeconds'] for r in arm_rows
                         if r.get('overrunSeconds') is not None]
                slices = [s for r in arm_rows for s in r['m34']]
                firsts = [r['m34'][0]['seconds'] for r in arm_rows if r['m34']]
                cell[label] = {
                    'coordinatorSeconds': {
                        'median': statistics.median(walls),
                        'max': max(walls)} if walls else None,
                    'overruns': sum(1 for v in overs if v > 0),
                    'worstOverrunSeconds': max(overs) if overs else None,
                    'runs': len(arm_rows),
                    'm34Slices': len(slices),
                    'm34SlicesPublishing':
                        sum(1 for s in slices if s['published']),
                    'm34SecondsPerRun':
                        sum(s['seconds'] for s in slices) / len(arm_rows),
                    'm34FirstSliceSeconds': {
                        'median': statistics.median(firsts),
                        'min': min(firsts), 'max': max(firsts)}
                    if firsts else None,
                    'sterileSlicesOver1s':
                        sum(1 for s in slices
                            if not s['published'] and s['seconds'] > 1.0),
                    'probeAborts':
                        sum(1 for s in slices if s['abortedBarrenProbe']),
                    'mmPerSecond': None,
                }
            entry[tier] = cell
    json.dump(report, open(out, 'w'), indent=1)
    print(json.dumps(report, indent=1))


if __name__ == '__main__':
    main()
