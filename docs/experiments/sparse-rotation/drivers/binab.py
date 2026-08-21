#!/usr/bin/env python3
"""Two binaries, one spec, paired and interleaved from the bare request.

    binab.py NAME ROUNDS REQUEST SEEDS BUDGETKEY BUDGETVALUE EXTRA OLD NEW

`battery.py` compares two *arms* of one binary; this compares two *binaries* at
one arm, which is the question the tax fixes have to answer before the compound
battery is worth running: with the operator armed on both sides, does the fixed
binary buy more mode-34 slices in the same ten seconds?

Paired and interleaved with the binary order reversed on odd rounds, because
the box is shared. Per-slice wall and slices per run are reported beside the
depth, since the depth is the outcome and the slice count is the mechanism.
"""
import hashlib
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402


def slices_of(doc):
    portfolio = doc.get('portfolio') or {}
    return [call for call in portfolio.get('operatorCalls', [])
            if call.get('operator') == 'mode34']


def row_for(doc, seconds):
    portfolio = doc.get('portfolio') or {}
    incumbent = portfolio.get('incumbent') or {}
    m34 = slices_of(doc)
    m22 = [call for call in portfolio.get('operatorCalls', [])
           if call.get('operator') == 'mode22']
    m34_wall = sum((call.get('elapsedSeconds') or 0.0) for call in m34)
    m22_wall = sum((call.get('elapsedSeconds') or 0.0) for call in m22)
    slice_totals = {}
    for key in ('rotationRungsProposed', 'rotationRungsImproved',
                'mirrorTogglesProposed', 'mirrorTogglesImproved',
                'rotationSurrogateBuilds', 'rotationSurrogateHits',
                'rotationSurrogateBuildMs', 'rotationBuildsRefused'):
        slice_totals[key] = sum(((call.get('scheduleSlice') or {}).get(key)
                                 or 0) for call in m34)
    return {
        'processSeconds': seconds,
        'rawDepthMm': incumbent.get('rawDepthMm'),
        'engineDepthMm': doc.get('independentUsedLongAxisDepthMm'),
        'incumbentSource': incumbent.get('source'),
        'coordinatorSeconds': portfolio.get('elapsedSeconds'),
        'operatorCalls': len(portfolio.get('operatorCalls', [])),
        'm34Slices': len(m34),
        'm34Published': sum(1 for call in m34 if call.get('published')),
        'm34WallSeconds': m34_wall,
        'm34PerSliceSeconds': (m34_wall / len(m34)) if m34 else None,
        'm22Calls': len(m22),
        'm22WallSeconds': m22_wall,
        'm22PerCallSeconds': (m22_wall / len(m22)) if m22 else None,
        'sliceTotals': slice_totals,
        'loadError': doc.get('_loadError'),
    }


def main():
    name, rounds, request, seeds = sys.argv[1], int(sys.argv[2]), sys.argv[3], \
        [int(s) for s in sys.argv[4].split(',')]
    budget_key, budget_value, extra = sys.argv[5], sys.argv[6], sys.argv[7]
    binaries = [('old', sys.argv[8]), ('new', sys.argv[9])]
    out_dir = f'{runlib.OUT}/{name}'
    result = {'name': name, 'request': request, 'rounds': rounds,
              'seeds': seeds, 'budget': f'{budget_key}={budget_value}',
              'extra': extra,
              'binaries': {label: {'path': path, 'sha256': hashlib.sha256(
                  open(path, 'rb').read()).hexdigest()}
                  for label, path in binaries},
              'rows': []}
    for round_index in range(rounds):
        ordered = binaries if round_index % 2 == 0 else list(reversed(binaries))
        for seed in seeds:
            for label, binary in ordered:
                spec = runlib.spec_for(seed, budget_key, budget_value, True,
                                       extra)
                tag = f'{label}-s{seed}-r{round_index}'
                doc, seconds, err = runlib.run(
                    binary, request, seed, spec, f'{out_dir}/runs/{tag}.json')
                row = row_for(doc, seconds)
                row.update({'binary': label, 'seed': seed,
                            'round': round_index, 'spec': spec})
                result['rows'].append(row)
                print(f"{tag}: raw={row['rawDepthMm']} "
                      f"m34={row['m34Slices']}/{row['m34Published']} "
                      f"perSlice={row['m34PerSliceSeconds']} "
                      f"m22={row['m22Calls']}@{row['m22PerCallSeconds']}",
                      flush=True)
                json.dump(result, open(f'{out_dir}/binab.json', 'w'), indent=1)

    summary = {}
    for label, _ in binaries:
        rows = [r for r in result['rows'] if r['binary'] == label]
        for field in ('rawDepthMm', 'm34Slices', 'm34PerSliceSeconds',
                      'm22PerCallSeconds', 'operatorCalls', 'm34Published'):
            values = [r[field] for r in rows if r.get(field) is not None]
            summary[f'{label}.{field}.median'] = statistics.median(values) \
                if values else None
    # Paired per (seed, round): the only comparison a shared box supports.
    paired = []
    for seed in seeds:
        for round_index in range(rounds):
            old = next((r for r in result['rows'] if r['binary'] == 'old'
                        and r['seed'] == seed and r['round'] == round_index),
                       None)
            new = next((r for r in result['rows'] if r['binary'] == 'new'
                        and r['seed'] == seed and r['round'] == round_index),
                       None)
            if old and new and old['rawDepthMm'] and new['rawDepthMm']:
                paired.append({
                    'seed': seed, 'round': round_index,
                    'depthDeltaMm': new['rawDepthMm'] - old['rawDepthMm'],
                    'sliceDelta': new['m34Slices'] - old['m34Slices'],
                    'perSliceRatio': ((old['m34PerSliceSeconds']
                                       / new['m34PerSliceSeconds'])
                                      if old['m34PerSliceSeconds']
                                      and new['m34PerSliceSeconds'] else None),
                    'perM22Ratio': ((old['m22PerCallSeconds']
                                     / new['m22PerCallSeconds'])
                                    if old['m22PerCallSeconds']
                                    and new['m22PerCallSeconds'] else None),
                })
    summary['pairedCells'] = len(paired)
    if paired:
        deltas = [p['depthDeltaMm'] for p in paired]
        summary['pairedDepthMedianMm'] = statistics.median(deltas)
        summary['newBetterCells'] = sum(1 for d in deltas if d < 0)
        summary['oldBetterCells'] = sum(1 for d in deltas if d > 0)
        summary['pairedSliceDeltaMedian'] = statistics.median(
            [p['sliceDelta'] for p in paired])
        ratios = [p['perSliceRatio'] for p in paired if p['perSliceRatio']]
        summary['perSliceSpeedupMedian'] = statistics.median(ratios) \
            if ratios else None
        m22 = [p['perM22Ratio'] for p in paired if p['perM22Ratio']]
        summary['perM22SpeedupMedian'] = statistics.median(m22) if m22 else None
    result['paired'] = paired
    result['summary'] = summary
    json.dump(result, open(f'{out_dir}/binab.json', 'w'), indent=1)
    print(json.dumps(summary, indent=1))


if __name__ == '__main__':
    main()
