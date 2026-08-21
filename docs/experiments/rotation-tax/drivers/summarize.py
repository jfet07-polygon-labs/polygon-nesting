#!/usr/bin/env python3
"""The round's tables, out of the three anytime batteries.

    summarize.py OUT BATTERY [BATTERY ...]

One table of paired deltas per request per budget - the statistic is the
per-round paired difference in published depth, `crot` minus `base`, so a
negative number is the operator winning - and one table of the operator's own
attribution: what it proposed, what was accepted, what it bought and what it
cost. The two are printed side by side deliberately: "the arm was better" and
"rotation bought millimetres" are different claims and this round has to be able
to make them separately.
"""
import json
import statistics
import sys

BUDGETS = ('3', '10', '30')


def paired(rows, base_label, arm_label):
    by_key = {(row['arm'], row['seed'], row['round']): row for row in rows}
    deltas = []
    for (arm, seed, rnd) in list(by_key):
        if arm != base_label:
            continue
        left = by_key.get((base_label, seed, rnd))
        right = by_key.get((arm_label, seed, rnd))
        if not left or not right:
            continue
        if left['engineDepthMm'] is None or right['engineDepthMm'] is None:
            continue
        deltas.append((seed, rnd, left['engineDepthMm'],
                       right['engineDepthMm'],
                       right['engineDepthMm'] - left['engineDepthMm']))
    return sorted(deltas)


def spread(rows, label):
    values = [row['engineDepthMm'] for row in rows
              if row['arm'] == label and row['engineDepthMm'] is not None]
    if not values:
        return None
    return {'n': len(values), 'min': min(values), 'max': max(values),
            'median': statistics.median(values),
            'spreadMm': max(values) - min(values)}


def rotation_totals(rows, label):
    keys = ('rotationRungsProposed', 'rotationRungsImproved',
            'mirrorTogglesProposed', 'mirrorTogglesImproved',
            'rotationAcceptedMoves', 'acceptedMoves', 'rotationLossBoughtMm',
            'translationLossBoughtMm', 'rotationSurrogateBuilds',
            'rotationSurrogateHits', 'rotationSurrogateEvictions',
            'rotationSurrogateBuildMs', 'rotationSurrogateCells',
            'rotationBuildsRefused')
    slices = [entry for row in rows if row['arm'] == label
              for entry in row['m34']]
    out = {key: sum(entry.get(key) or 0 for entry in slices) for key in keys}
    out['m34Slices'] = len(slices)
    out['m34Published'] = sum(1 for entry in slices if entry.get('published'))
    out['m34Seconds'] = sum(entry.get('seconds') or 0 for entry in slices)
    proposals = out['rotationRungsProposed'] + out['mirrorTogglesProposed']
    iterations = proposals / 2 if proposals else 0
    out['rotationIterations'] = iterations
    improved = out['rotationRungsImproved'] + out['mirrorTogglesImproved']
    out['rotationImproved'] = improved
    out['rungAcceptance'] = improved / iterations if iterations else None
    out['rotationAcceptedMoveShare'] = (
        out['rotationAcceptedMoves'] / out['acceptedMoves']
        if out['acceptedMoves'] else None)
    total_loss = out['rotationLossBoughtMm'] + out['translationLossBoughtMm']
    out['rotationLossShare'] = (out['rotationLossBoughtMm'] / total_loss
                                if total_loss else None)
    out['msPerRotationIteration'] = (out['rotationSurrogateBuildMs']
                                     / iterations if iterations else None)
    out['buildsPerRotationIteration'] = (out['rotationSurrogateBuilds']
                                         / iterations if iterations else None)
    out['cacheHitRate'] = (
        out['rotationSurrogateHits']
        / (out['rotationSurrogateHits'] + out['rotationSurrogateBuilds'])
        if (out['rotationSurrogateHits'] + out['rotationSurrogateBuilds'])
        else None)
    return out


def main():
    out_path = sys.argv[1]
    report = {'requests': {}}
    for path in sys.argv[2:]:
        battery = json.load(open(path))
        request = battery['request']
        rows = battery['rows']
        entry = {'binary': battery['binary'], 'budgets': {}}
        for budget in BUDGETS:
            base, arm = f'baseat{budget}', f'crotat{budget}'
            deltas = paired(rows, base, arm)
            budget_rows = [row for row in rows if row['arm'] in (base, arm)]
            values = [d[4] for d in deltas]
            entry['budgets'][budget] = {
                'pairs': len(deltas),
                'medianDeltaMm': statistics.median(values) if values else None,
                'minDeltaMm': min(values) if values else None,
                'maxDeltaMm': max(values) if values else None,
                'crotBetter': sum(1 for v in values if v < 0),
                'baseBetter': sum(1 for v in values if v > 0),
                'equal': sum(1 for v in values if v == 0),
                'baseSpread': spread(budget_rows, base),
                'crotSpread': spread(budget_rows, arm),
                'baseRotation': rotation_totals(budget_rows, base),
                'crotRotation': rotation_totals(budget_rows, arm),
                'perPair': [{'seed': s, 'round': r, 'baseMm': b, 'crotMm': c,
                             'deltaMm': d} for s, r, b, c, d in deltas],
            }
        report['requests'][request] = entry
    json.dump(report, open(out_path, 'w'), indent=1)

    for request, entry in report['requests'].items():
        print(f'\n=== {request}')
        for budget, cell in entry['budgets'].items():
            median = cell['medianDeltaMm']
            print(f"  {budget:>2}s  median {median:+.3f} mm  "
                  f"crot better {cell['crotBetter']}/{cell['pairs']}  "
                  f"range [{cell['minDeltaMm']:+.3f}, {cell['maxDeltaMm']:+.3f}]"
                  f"  base spread {cell['baseSpread']['spreadMm']:.3f}"
                  f"  crot spread {cell['crotSpread']['spreadMm']:.3f}")
            rot = cell['crotRotation']
            if rot['rotationIterations']:
                print(f"       rotation: {rot['rotationIterations']:.0f} iters, "
                      f"acceptance {rot['rungAcceptance']:.3f}, "
                      f"loss share {rot['rotationLossShare']:.3f}, "
                      f"accepted-move share "
                      f"{rot['rotationAcceptedMoveShare']:.3f}, "
                      f"{rot['msPerRotationIteration']*1000:.1f} us/iter build, "
                      f"cache hit {rot['cacheHitRate']:.3f}, "
                      f"m34 {rot['m34Published']}/{rot['m34Slices']} published")
            else:
                print(f"       rotation: no m34 slice ran in the armed arm "
                      f"(base ran {cell['baseRotation']['m34Slices']})")


if __name__ == '__main__':
    main()
