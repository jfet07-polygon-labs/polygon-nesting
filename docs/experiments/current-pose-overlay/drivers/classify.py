#!/usr/bin/env python3
"""Classify the entry collision pairs the overlay adds, per Sol review 6 §2.

    python3 classify.py BINARY OUTDIR

Sol review 6, "Interpretazione dei numeri":

    Il `+9` coppie su 14/15 non invalida la fedeltà alla posa esatta, ma non è
    neppure "il prezzo atteso" finché non è classificato. Dice che il surrogate
    continuo è più conservativo, o più inaccurato, proprio alle rotazioni dei
    parent. Per ogni coppia nuova misurerei: collisione esatta
    material/envelope; risultato proxy grid; risultato proxy continuous;
    margine dal confine.

That is exactly the four columns `parentPairClassification` carries, and this
driver reduces them to a verdict per pair.

The decision rule, for a pair the continuous proxy calls colliding and the grid
proxy does not:

  * `envelopeOverlap == true`  -> the exact tier agrees the pair conflicts.
    The grid proxy was *missing a real conflict*; the overlay is more
    conservative **and more accurate**. Call this `catches-real-conflict`.
  * `envelopeOverlap == false` -> the exact tier says the pair is clear, so the
    continuous proxy over-reported. **How far from clear** decides what that
    means, which is exactly why Sol asked for the margin:
      - within the margin band the grid proxy's *own* flagged pairs occupy
        (`<= bothMarginMaxMm`, the largest margin among pairs both proxies
        already call colliding) the continuous proxy is making the same kind of
        call the shipping proxy already makes, on a pair sitting on the
        clearance contract. Call this `conservative-at-boundary`.
      - beyond that band it is flagging a pair with more slack than any pair
        the grid proxy flags. Call this `inaccurate`.

The mirror image applies to a pair the grid proxy calls colliding and the
continuous one does not (`removed-by-overlay`): `envelopeOverlap == true` is
the overlay **missing a real conflict**; otherwise the grid proxy was the one
over-reporting, and the same band splits "both were being conservative on a
contract-tight pair" from "the grid proxy flagged a pair with real slack and
the overlay correctly did not".

The band is derived per parent from the data rather than fixed, so it is not a
threshold chosen to make an answer come out.

Both proxy verdicts come from a single run, computed on the two resolutions of
the *same* parent translations - which is precisely how the two campaign arms
differ, because `initialize_complete_state` changes only the rotation half of
the pose. The driver cross-checks that reconstruction against the two arms'
own `parentCollisionPairs` counters before trusting any of it.

The classification run carries an exact-tier bisection per pair, so it is never
a timing measurement; it is run at a one-unit work budget purely to reach the
parent's own entry state, which the schedule computes before it steps.
"""
import hashlib
import json
import os
import statistics
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import campaign  # noqa: E402


def run(binary, parent, outdir, overlay, classify):
    args = [a.format(pressure='structured') for a in campaign.ARGS]
    target = parent['depthMm'] - campaign.DEFAULT_DROP_MM
    allowance = parent.get('allowance', campaign.DEFAULT_ALLOWANCE)
    tail = ['34', parent['fixture'], f'{target:.17g}', '', allowance]
    env = dict(os.environ)
    # One work unit: the parent's entry state and its classification are both
    # computed before the schedule takes a step, so no budget is needed.
    env['POLYGON_NESTING_COMPRESSION_SCHEDULE'] = f'{campaign.SCHEDULE_V4},work=1'
    env.pop('POLYGON_NESTING_CURRENT_POSE_OVERLAY', None)
    env.pop('POLYGON_NESTING_CURRENT_POSE_OVERLAY_CLASSIFY', None)
    env.pop('POLYGON_NESTING_PROFILE', None)
    if overlay:
        env['POLYGON_NESTING_CURRENT_POSE_OVERLAY'] = '1'
    if classify:
        env['POLYGON_NESTING_CURRENT_POSE_OVERLAY_CLASSIFY'] = '1'
    tag = ('overlay-classify' if classify else ('overlay' if overlay else 'grid'))
    path = f"{outdir}/{parent['name']}-{tag}.json"
    os.makedirs(outdir, exist_ok=True)
    with open(path, 'w') as handle:
        subprocess.run([binary, campaign.REQUEST] + args + tail, stdout=handle,
                       stderr=subprocess.DEVNULL, check=False, env=env)
    try:
        return json.load(open(path))
    except json.JSONDecodeError:
        return None


def schedule_of(doc):
    if doc is None:
        return None
    pop = ((doc.get('relaxedDiagnostics') or {})
           .get('coupledDynamicSeparator') or {}).get(
               'persistentVacancyPopulation')
    return (pop or {}).get('compressionSchedule')


def group_of(row):
    continuous = row['continuousProxyPenalty'] > 0.0
    grid = row['gridProxyPenalty'] > 0.0
    if continuous and not grid:
        return 'added-by-overlay'
    if grid and not continuous:
        return 'removed-by-overlay'
    return 'both'


def classify_rows(rows):
    """Group every classified pair and give it a verdict.

    Two passes: the band the verdicts are measured against is the largest
    envelope margin among the pairs *both* proxies call colliding, which is a
    property of this parent's own layout rather than a constant chosen here.
    A parent with no `both` pairs gets a band of `0.0`, which makes every
    over-report `inaccurate` - the strict reading, not the flattering one.
    """
    grouped = [{**row, 'group': group_of(row)} for row in rows]
    both_margins = [r['envelopeMarginMm'] for r in grouped
                    if r['group'] == 'both']
    band = max(both_margins) if both_margins else 0.0
    out = []
    for row in grouped:
        at_boundary = row['envelopeMarginMm'] <= band
        if row['group'] == 'added-by-overlay':
            if row['envelopeOverlap']:
                verdict = 'catches-real-conflict'
            elif at_boundary:
                verdict = 'conservative-at-boundary'
            else:
                verdict = 'inaccurate'
        elif row['group'] == 'removed-by-overlay':
            if row['envelopeOverlap']:
                verdict = 'misses-real-conflict'
            elif at_boundary:
                verdict = 'optimistic-at-boundary'
            else:
                verdict = 'drops-a-grid-false-positive'
        else:
            verdict = 'agree'
        out.append({**row, 'verdict': verdict, 'atBoundary': at_boundary})
    return out, band


def tally(rows):
    out = {}
    for row in rows:
        out[row['verdict']] = out.get(row['verdict'], 0) + 1
    return dict(sorted(out.items()))


def span(rows):
    """min / median / max envelope margin, in millimetres."""
    margins = [r['envelopeMarginMm'] for r in rows]
    if not margins:
        return None
    return {'min': min(margins), 'median': statistics.median(margins),
            'max': max(margins)}


def main():
    binary = sys.argv[1]
    outdir = sys.argv[2]
    os.makedirs(outdir, exist_ok=True)
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'schedule': f'{campaign.SCHEDULE_V4},work=1',
        'note': 'diagnostic runs; wall and work are not comparable with a measured arm',
        'perParent': [],
    }
    for parent in campaign.PORT_PARENTS:
        grid = schedule_of(run(binary, parent, outdir, False, False))
        overlay = schedule_of(run(binary, parent, outdir, True, True))
        if grid is None or overlay is None:
            result['perParent'].append({'parent': parent['name'],
                                        'error': 'no compressionSchedule'})
            print(json.dumps(result['perParent'][-1]))
            continue
        rows, band = classify_rows(
            overlay.get('parentPairClassification') or [])
        added = [r for r in rows if r['group'] == 'added-by-overlay']
        removed = [r for r in rows if r['group'] == 'removed-by-overlay']
        both = [r for r in rows if r['group'] == 'both']
        # Self-validation: the reconstruction must reproduce both arms' own
        # entry counters. If it does not, nothing below is trustworthy.
        reconstructed_grid = sum(1 for r in rows if r['gridProxyPenalty'] > 0.0)
        reconstructed_overlay = sum(
            1 for r in rows if r['continuousProxyPenalty'] > 0.0)
        entry = {
            'parent': parent['name'],
            'gridCollisionPairs': grid.get('parentCollisionPairs'),
            'overlayCollisionPairs': overlay.get('parentCollisionPairs'),
            'delta': (overlay.get('parentCollisionPairs', 0)
                      - grid.get('parentCollisionPairs', 0)),
            'reconstructedGridPairs': reconstructed_grid,
            'reconstructedOverlayPairs': reconstructed_overlay,
            'reconstructionMatches':
                reconstructed_grid == grid.get('parentCollisionPairs')
                and reconstructed_overlay == overlay.get('parentCollisionPairs'),
            'overlayEntries': overlay.get('currentPoseOverlayEntries'),
            'overlayOffGridPieces': overlay.get('currentPoseOverlayOffGridPieces'),
            'bothPairs': len(both),
            'bothMarginMaxMm': band,
            'addedPairs': len(added),
            'removedPairs': len(removed),
            'addedVerdicts': tally(added),
            'removedVerdicts': tally(removed),
            'addedMarginMmSpan': span(added),
            'removedMarginMmSpan': span(removed),
            'bothMarginMmSpan': span(both),
            'rows': rows,
        }
        result['perParent'].append(entry)
        print(json.dumps({k: v for k, v in entry.items() if k != 'rows'}))
        json.dump(result, open(f'{outdir}/classification.json', 'w'), indent=1)

    ok = [p for p in result['perParent'] if 'error' not in p]
    added_all = [r for p in ok for r in p['rows'] if r['group'] == 'added-by-overlay']
    removed_all = [r for p in ok for r in p['rows'] if r['group'] == 'removed-by-overlay']
    both_all = [r for p in ok for r in p['rows'] if r['group'] == 'both']
    result['summary'] = {
        'parents': len(ok),
        # If this is not `true`, nothing below is trustworthy: it says the
        # per-pair reconstruction reproduced *both* campaign arms' own
        # `parentCollisionPairs` counters exactly, on every parent.
        'reconstructionMatchesAll': all(p['reconstructionMatches'] for p in ok),
        'bothPairsTotal': len(both_all),
        'addedPairsTotal': len(added_all),
        'removedPairsTotal': len(removed_all),
        'addedVerdicts': tally(added_all),
        'removedVerdicts': tally(removed_all),
        # The exact tier's own verdicts. `materialOverlap` must be 0 on an
        # exact-valid parent; `envelopeOverlap` is the question "did either
        # proxy flag a pair that really does conflict".
        'addedMaterialOverlap': sum(1 for r in added_all if r['materialOverlap']),
        'addedEnvelopeOverlap': sum(1 for r in added_all if r['envelopeOverlap']),
        'removedEnvelopeOverlap':
            sum(1 for r in removed_all if r['envelopeOverlap']),
        'addedMarginMm': span(added_all),
        'removedMarginMm': span(removed_all),
        'bothMarginMm': span(both_all),
        'deltaMedian': statistics.median([p['delta'] for p in ok]) if ok else None,
    }
    json.dump(result, open(f'{outdir}/classification.json', 'w'), indent=1)
    print(json.dumps(result['summary'], indent=1))


if __name__ == '__main__':
    main()
