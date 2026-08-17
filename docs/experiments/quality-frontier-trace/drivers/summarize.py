#!/usr/bin/env python3
"""Assembles the curated summary.json from the measured artifacts.

Everything here is derived - no number is typed in. The inputs are
`summary-measured.json` (the per-run derivation), the A/B summaries under
/var/lib/t3/tmp/qft/ab, and the gate results under /var/lib/t3/tmp/qft/gates.
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.dirname(HERE)
AB = '/var/lib/t3/tmp/qft/ab'
GATES = '/var/lib/t3/tmp/qft/gates'


def maybe(path):
    try:
        return json.load(open(path))
    except (OSError, ValueError):
        return None


def ab_row(name):
    row = maybe(f'{AB}/{name}/summary.json')
    if not row:
        return None
    return {key: row[key] for key in (
        'aLabel', 'bLabel', 'rounds', 'aMedianSeconds', 'bMedianSeconds',
        'pairedRatioMedian', 'pairedRatioMin', 'pairedRatioMax',
        'roundsBelowParity', 'allOutcomesIdentical')}


def gate_row(label):
    row = maybe(f'{GATES}/{label}/gates-{label}.json')
    if not row:
        return None
    return {
        'binary': row['binary'], 'sink': row['sink'],
        'allPass': row['ALL_PASS'],
        'g1': {'depths': row['g1']['depths'],
               'fingerprints': row['g1']['fingerprints'],
               'hit': row['g1']['hit']},
        **{key: {'raw': row[key]['raw'], 'fingerprint': row[key]['fingerprint'],
                 'exactValid': row[key]['exactValid'],
                 'contractValid': row[key]['contractValid'],
                 'hit': row[key]['hit']}
           for key in ('g2', 'g3', 'g4')},
    }


def main():
    measured = json.load(open(f'{OUT}/summary-measured.json'))
    runs = measured['runs']

    def run(tag):
        return runs[tag]

    curve_rows = {}
    for tag, row in runs.items():
        milestone = row['milestones']
        marginal = row['marginal']
        curve_rows[tag] = {
            'config': row['config'], 'seed': row['seed'],
            'variant': row['variant'],
            'workOrdinalsArmed': row['workOrdinalsArmed'],
            'wallSeconds': row['wallSeconds'],
            'engineDepthMm': row['engineDepthMm'],
            'modeExactValid': row['modeExactValid'],
            'modeDepthMm': row['modeDepthMm'],
            'exactValidCandidates': row['exactValidCandidates'],
            'timeToFirstCompleteLayoutSeconds':
                milestone['timeToFirstCompleteLayoutSeconds'],
            'firstCompleteLayoutDepthMm':
                milestone['firstCompleteLayoutDepthMm'],
            'depthMilestones': milestone['depthMilestones'],
            'firstIncumbentDepthMm': marginal.get('firstIncumbentDepthMm'),
            'finalIncumbentDepthMm': marginal.get('finalIncumbentDepthMm'),
            'totalGainMm': marginal.get('totalGainMm'),
            'gainWindowSeconds': marginal.get('gainWindowSeconds'),
            'mmPerSecondInsideGainWindow': (
                marginal['totalGainMm'] / marginal['gainWindowSeconds']
                if marginal.get('gainWindowSeconds') else None),
            'mmPerSecondOverWholeRun':
                marginal.get('overallMmPerSecondOverWholeRun'),
            'zeroGainTailSeconds': marginal.get('idleTailSeconds'),
            'endWork': row['endWork'],
            'topScopesBySeconds': row['scopes'][:12],
            'publications': row['publications'],
        }

    stretch = {}
    for config in ('m0coupled', 'mode20'):
        for seed in (0, 1):
            work = run(f'{config}-seed{seed}-work')['traceEndSeconds']
            clock = run(f'{config}-seed{seed}-clock')['traceEndSeconds']
            stretch[f'{config}-seed{seed}'] = {
                'workVariantSeconds': work, 'clockVariantSeconds': clock,
                'ratio': work / clock,
            }

    summary = {
        'what': 'Quality frontier trace: the first depth-versus-time curve for '
                'this engine, measured from request only in one process.',
        'request': measured['request'],
        'areaLowerBoundDepthMm': measured['areaLowerBoundDepthMm'],
        'mode20ClampMultipleOfAreaLowerBound':
            measured['mode20ClampMultiple'],
        'mode20ClampMm': measured['mode20ClampMm'],
        'instrument': {
            'feature': 'quality-trace',
            'sinkEnv': 'POLYGON_NESTING_QUALITY_TRACE',
            'workOrdinalEnv': 'POLYGON_NESTING_QUALITY_TRACE_COUNTERS',
            'unpinnedParentEnv': 'POLYGON_NESTING_UNPINNED_VACANCY_PARENT',
            'chokePoint': 'search::general_fast::validate_and_measure_placements',
        },
        'overheadPairedAB': {
            'featureCompiledInButSinkClosed': ab_row('base-vs-traceoff'),
            'sinkOpenWithWorkOrdinals': ab_row('traceoff-vs-traceon'),
            'workOrdinalsAloneNoSink': ab_row('traceoff-vs-countersonly'),
        },
        'traceClockStretchWorkOverClock': stretch,
        'gates': {
            'baseBinary': gate_row('base'),
            'traceBinaryWithSinkArmedOnEveryGate': gate_row('trace-armed'),
        },
        'curves': curve_rows,
        'artifacts': {
            'plot': 'frontier.png',
            'perRunCurves': 'curves/curve-<tag>.json',
            'perRunDerivation': 'summary-measured.json',
            'rawEventStreams': '/var/lib/t3/tmp/qft/frontier/<tag>.jsonl',
        },
    }
    json.dump(summary, open(f'{OUT}/summary.json', 'w'), indent=1)
    print(json.dumps({
        'gatesBase': summary['gates']['baseBinary']['allPass'],
        'gatesTraced': summary['gates'][
            'traceBinaryWithSinkArmedOnEveryGate']['allPass'],
        'featureOffRatio': summary['overheadPairedAB'][
            'featureCompiledInButSinkClosed']['pairedRatioMedian'],
        'sinkOnRatio': summary['overheadPairedAB'][
            'sinkOpenWithWorkOrdinals']['pairedRatioMedian'],
        'countersOnlyRatio': summary['overheadPairedAB'][
            'workOrdinalsAloneNoSink']['pairedRatioMedian'],
    }, indent=1))


if __name__ == '__main__':
    sys.exit(main())
