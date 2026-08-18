#!/usr/bin/env python3
"""Reduces curve.json to the numbers the report quotes.

Everything here is paired: each round contributes one baseline row and one
coordinator row at the same seed, run back to back with the arm order
alternating, so the statistic is a per-round difference rather than two
independently-sampled medians.

Usage: summarize.py [CURVE_JSON]
"""
import json
import statistics
import sys

PATH = sys.argv[1] if len(sys.argv) > 1 else '/var/lib/t3/tmp/pr7/curve/curve.json'
MILESTONES = (200.0, 190.0, 185.0, 182.0, 181.6, 180.0, 179.69, 179.0, 178.0,
              177.0, 175.0)


def first_at_or_below(series, threshold):
    for point in series:
        depth = point.get('rawDepthMm')
        if depth is None:
            depth = point['depthMm']
        if depth <= threshold:
            return point['t']
    return None


def main():
    data = json.load(open(PATH))
    rows = data['rows']
    by_key = {(row['arm'], row['seed'], row['round']): row for row in rows}
    seeds = sorted({row['seed'] for row in rows})
    rounds = sorted({row['round'] for row in rows})

    arms = [label for label in data['arms'] if label != 'base']
    summary = {
        'binary': data['binary'],
        'allowance': data['allowance'],
        'arms': data['arms'],
        'saltSets': data['saltSets'],
        'pairs': [],
        'milestones': {},
        'perSeed': {},
    }
    deltas = {arm: [] for arm in arms}
    for seed in seeds:
        per_arm = {arm: [] for arm in ('base', *arms)}
        for round_index in rounds:
            base = by_key.get(('base', seed, round_index))
            if not base:
                continue
            per_arm['base'].append(base['engineDepthMm'])
            for arm in arms:
                row = by_key.get((arm, seed, round_index))
                if not row:
                    continue
                per_arm[arm].append(row['engineDepthMm'])
                delta = row['engineDepthMm'] - base['engineDepthMm']
                deltas[arm].append(delta)
                summary['pairs'].append({
                    'arm': arm,
                    'seed': seed,
                    'round': round_index,
                    'baseEngineDepthMm': base['engineDepthMm'],
                    'armEngineDepthMm': row['engineDepthMm'],
                    'armRawDepthMm': row.get('rawDepthMm'),
                    'armDualGateValid': row.get('dualGateValid'),
                    'deltaMm': delta,
                    'baseProcessSeconds': base['processSeconds'],
                    'armProcessSeconds': row['processSeconds'],
                    'armPublishedSeconds': row.get('publishedSeconds'),
                })
        summary['perSeed'][str(seed)] = {
            arm: {'median': statistics.median(values), 'depths': values}
            for arm, values in per_arm.items() if values
        }
    summary['pairedDeltaMm'] = {
        arm: {
            'median': statistics.median(values),
            'min': min(values),
            'max': max(values),
            'roundsBetterThanBaseline': sum(1 for d in values if d < 0),
            'rounds': len(values),
        }
        for arm, values in deltas.items() if values
    }

    # Milestone times, median over rounds, per arm and seed.
    for arm in data['arms']:
        summary['milestones'][arm] = {}
        for seed in seeds:
            per_threshold = {}
            for threshold in MILESTONES:
                times = []
                for round_index in rounds:
                    row = by_key.get((arm, seed, round_index))
                    if not row:
                        continue
                    reached = first_at_or_below(row['incumbentSeries'], threshold)
                    times.append(reached)
                hit = [value for value in times if value is not None]
                per_threshold[str(threshold)] = (
                    round(statistics.median(hit), 3) if len(hit) == len(times)
                    and hit else None)
            summary['milestones'][arm][str(seed)] = per_threshold

    # Which phase produced each coordinator improvement, and what the archive
    # was doing while it did.
    phase_credits = {}
    archive_curves = {}
    operator_cost = {}
    for row in rows:
        if row['arm'] == 'base':
            continue
        for event in row.get('publications', []):
            key = f"{row['arm']}/{event['phase']}/{event['source']}"
            credit = phase_credits.setdefault(key, {'count': 0, 'gainMm': 0.0})
            credit['count'] += 1
            previous = event.get('previousRawDepthMm')
            if previous is not None:
                credit['gainMm'] += previous - event['rawDepthMm']
        for call in row.get('operatorCalls', []):
            cost = operator_cost.setdefault(
                f"{row['arm']}/{call['phase']}/{call['operator']}",
                {'calls': 0, 'seconds': 0.0, 'published': 0, 'exactValid': 0})
            cost['calls'] += 1
            cost['seconds'] += call['elapsedSeconds']
            cost['published'] += int(call['published'])
            cost['exactValid'] += int(call['exactValid'])
        archive = row.get('archive')
        if archive:
            archive_curves[row['tag']] = {
                'occupancy': archive['occupancy'],
                'capacity': archive['capacity'],
                'byOperator': archive['byOperator'],
                'evicted': archive['evicted'],
                'duplicates': archive['duplicates'],
                'refusedArchiveFullAllDistinct':
                    archive['refusedArchiveFullAllDistinct'],
                'occupancyOverTime': archive['occupancyOverTime'],
            }
    summary['phaseCredits'] = phase_credits
    summary['operatorCost'] = {
        key: {**value, 'meanSeconds': value['seconds'] / value['calls']}
        for key, value in sorted(operator_cost.items())
    }
    summary['archives'] = archive_curves

    # Phase wall shares, median over the coordinator runs.
    phase_seconds = {}
    for row in rows:
        if row['arm'] == 'base':
            continue
        for phase in row.get('phases', []):
            phase_seconds.setdefault(f"{row['arm']}/{phase['name']}", []).append(
                phase['elapsedSeconds'])
    summary['phaseSeconds'] = {
        name: {'median': statistics.median(values), 'min': min(values),
               'max': max(values)}
        for name, values in phase_seconds.items()
    }
    print(json.dumps(summary, indent=1))
    json.dump(summary, open(PATH.replace('curve.json', 'summary.json'), 'w'),
              indent=1)


if __name__ == '__main__':
    main()
