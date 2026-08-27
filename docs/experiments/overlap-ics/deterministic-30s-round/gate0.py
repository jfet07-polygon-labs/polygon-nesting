#!/usr/bin/env python3
"""Gate 0 for the deterministic 30-second round.

Five AB/BA pairs are run on each fixture. A is the frozen four-order
constructor and B is the one-order candidate. Each cell is a fresh process;
the search budget is empty, so the measured wall field is the constructor
alone. `construct_short_side_first` validates every returned arm through the
independent material-contract validator before this example can emit it.

The aggregate is written beside the raw cell documents. Exit 0 is PASS, exit 1
is a measured gate miss, and exit 2 is an invalid measurement.
"""

import hashlib
import json
import os
import statistics
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, '..', '..', '..', '..'))
sys.path.insert(0, f'{ROOT}/docs/experiments/overlap-ics/drivers')
import lib  # noqa: E402

FIXTURES = ('mixed-61', 'shapes-17', 'triangle-20')
ORDERS = {'A': 4, 'B': 1}
PAIR_COUNT = 5
WORKERS = 8


def sha256_of(path):
    with open(path, 'rb') as handle:
        return hashlib.sha256(handle.read()).hexdigest()


def loadavg():
    with open('/proc/loadavg') as handle:
        return [float(value) for value in handle.read().split()[:3]]


def percentile(values, percentile_number):
    ordered = sorted(values)
    if len(ordered) == 1:
        return ordered[0]
    return statistics.quantiles(
        ordered, n=100, method='inclusive')[percentile_number - 1]


def constructor_cell(out, fixture, pair, sequence, position, arm):
    orders = ORDERS[arm]
    tag = f'{fixture}-p{pair}-{sequence}{position}-{arm}-o{orders}'
    path = f'{out}/cells/{tag}.json'
    document, process_wall, status, stderr = lib.run(
        'cutclose', fixture, path, orders=orders, mode='fixed', bites=0,
        compressbites=0, attempts=1, iters=1, workers=WORKERS, seed=0,
        revalidate=1)
    request = document.get('request') or {}
    constructor = document.get('constructor') or {}
    wall = document.get('wall') or {}
    outcome = document.get('outcome') or {}
    placement_count = constructor.get('placementCount')
    complete = placement_count == request.get('pieceCount')
    # A successful ShortSideFirst.layout call is a validation result, not an
    # assumption: general_fast::run_constructor_arm calls validate_result,
    # which runs both the exact search envelope and material contract checks;
    # the adapter then refuses any unplaced piece.
    dual_valid = bool(status == 0 and complete)
    return {
        'fixture': fixture,
        'pair': pair,
        'sequence': sequence,
        'position': position,
        'arm': arm,
        'orders': orders,
        'exit': status,
        'stderr': stderr[-800:],
        'processWallSeconds': process_wall,
        'constructorSeconds': wall.get('constructorSeconds'),
        'totalSeconds': wall.get('totalSeconds'),
        'rawSourceDepthMm': constructor.get('rawSourceDepthMm'),
        'placementFingerprint': constructor.get('placementFingerprint'),
        'placementCount': placement_count,
        'requestPieceCount': request.get('pieceCount'),
        'complete': complete,
        'dualValid': dual_valid,
        'incumbentIsConstructor': (outcome.get('incumbent') or {})
        .get('fromConstructor'),
        'invalidPublications': outcome.get('invalidPublications'),
        'sourcePath': path,
        'sourceSha256': lib.source_sha256(path),
    }


def summarize_fixture(rows, fixture):
    fixture_rows = [row for row in rows if row['fixture'] == fixture]
    arms = {}
    for arm in ORDERS:
        selected = [row for row in fixture_rows if row['arm'] == arm]
        seconds = [row['constructorSeconds'] for row in selected]
        arms[arm] = {
            'orders': ORDERS[arm],
            'readings': len(selected),
            'constructorSeconds': seconds,
            'minSeconds': min(seconds),
            'medianSeconds': statistics.median(seconds),
            'p95Seconds': percentile(seconds, 95),
            'maxSeconds': max(seconds),
            'rangeSeconds': max(seconds) - min(seconds),
            'depthsMm': sorted(set(row['rawSourceDepthMm'] for row in selected)),
            'fingerprints': sorted(set(row['placementFingerprint'] for row in selected)),
            'allDualValid': all(row['dualValid'] for row in selected),
            'allComplete': all(row['complete'] for row in selected),
        }
    paired = []
    for pair in range(PAIR_COUNT):
        pair_rows = [row for row in fixture_rows if row['pair'] == pair]
        for sequence in ('AB', 'BA'):
            seq_rows = [row for row in pair_rows if row['sequence'] == sequence]
            by_arm = {row['arm']: row for row in seq_rows}
            paired.append({
                'pair': pair,
                'sequence': sequence,
                'ASeconds': by_arm['A']['constructorSeconds'],
                'BSeconds': by_arm['B']['constructorSeconds'],
                'savingSeconds': (by_arm['A']['constructorSeconds']
                                  - by_arm['B']['constructorSeconds']),
            })
    pair_averages = []
    for pair in range(PAIR_COUNT):
        entries = [row for row in paired if row['pair'] == pair]
        pair_averages.append({
            'pair': pair,
            'savingSeconds': statistics.fmean(
                row['savingSeconds'] for row in entries),
        })
    return {
        'fixture': fixture,
        'arms': arms,
        'pairedReadings': paired,
        'pairedMedianSavingSeconds': statistics.median(
            row['savingSeconds'] for row in paired),
        'pairAverageMedianSavingSeconds': statistics.median(
            row['savingSeconds'] for row in pair_averages),
        'pairAverages': pair_averages,
    }


def main():
    out = (sys.argv[1] if len(sys.argv) > 1 else
           f'{HERE}/evidence/gate0')
    os.makedirs(f'{out}/cells', exist_ok=True)
    before = loadavg()
    document = {
        'experiment': 'overlap-ics',
        'battery': 'deterministic-30s-round-gate0',
        'spec': 'docs/deterministic-30s-round-spec.md',
        'fixtures': list(FIXTURES),
        'orders': ORDERS,
        'pairs': PAIR_COUNT,
        'workers': WORKERS,
        'binary': lib.BIN,
        'binarySha256': sha256_of(lib.BIN),
        'machine': {'cpus': os.cpu_count(), 'loadBefore': before},
        'quietBoxAtStart': before[0] < 1.0,
        'constructorValidityAuthority': (
            'construct_short_side_first -> run_constructor_arm -> '
            'validate_result -> validate_and_measure_placements; the adapter '
            'also refuses any unplaced piece'),
    }
    if not document['quietBoxAtStart']:
        document['error'] = 'one-minute load was not below 1.00 at Gate-0 start'
        with open(f'{out}/gate0.json', 'w') as handle:
            json.dump(document, handle, indent=1)
        print(json.dumps(document, indent=1))
        return 2

    rows = []
    started = time.monotonic()
    for fixture in FIXTURES:
        for pair in range(PAIR_COUNT):
            for sequence in ('AB', 'BA'):
                for position, arm in enumerate(sequence):
                    row = constructor_cell(
                        out, fixture, pair, sequence, position, arm)
                    rows.append(row)
                    print(
                        f'[gate0] {fixture} p{pair} {sequence}{position} '
                        f'o{row["orders"]} depth={row["rawSourceDepthMm"]} '
                        f'constructor={row["constructorSeconds"]:.6f}s',
                        file=sys.stderr, flush=True)
    document['batterySeconds'] = time.monotonic() - started
    document['cells'] = rows
    document['summaries'] = {
        fixture: summarize_fixture(rows, fixture) for fixture in FIXTURES
    }
    mixed = document['summaries']['mixed-61']
    a = mixed['arms']['A']
    b = mixed['arms']['B']
    other_start_ok = all(
        summary['arms']['B']['allDualValid']
        and summary['arms']['A']['allDualValid']
        and min(summary['arms']['B']['depthsMm'])
        <= min(summary['arms']['A']['depthsMm']) + 1.000
        for name, summary in document['summaries'].items()
        if name != 'mixed-61')
    clauses = {
        'quietBoxAtStart': document['quietBoxAtStart'],
        'allCellsExitedZero': all(row['exit'] == 0 for row in rows),
        'mixedFingerprintIdentity': a['fingerprints'] == b['fingerprints']
        and len(a['fingerprints']) == 1,
        'mixedDepthIdentity': a['depthsMm'] == b['depthsMm']
        and len(a['depthsMm']) == 1,
        'mixedBothDualValid': a['allDualValid'] and b['allDualValid'],
        'orders1P95AtMost0800': b['p95Seconds'] <= 0.800,
        'pairedMedianSavingAtLeast1500':
            mixed['pairedMedianSavingSeconds'] >= 1.500,
        'otherFixtureStartsWithin1mmAndDualValid': other_start_ok,
    }
    document['clauses'] = clauses
    document['GATE0_PASS'] = all(clauses.values())
    document['machine']['loadAfter'] = loadavg()
    document['cellSources'] = lib.MANIFEST
    document['binarySha256After'] = sha256_of(lib.BIN)
    document['binaryUnchangedDuringBattery'] = (
        document['binarySha256'] == document['binarySha256After'])
    document['GATE0_PASS'] &= document['binaryUnchangedDuringBattery']
    with open(f'{out}/gate0.json', 'w') as handle:
        json.dump(document, handle, indent=1)
    print(json.dumps({
        'GATE0_PASS': document['GATE0_PASS'],
        'clauses': clauses,
        'mixed61': {
            'orders4P95Seconds': a['p95Seconds'],
            'orders1P95Seconds': b['p95Seconds'],
            'pairedMedianSavingSeconds':
                mixed['pairedMedianSavingSeconds'],
            'orders4DepthsMm': a['depthsMm'],
            'orders1DepthsMm': b['depthsMm'],
            'fingerprintIdentity': clauses['mixedFingerprintIdentity'],
        },
    }, indent=1))
    return 0 if document['GATE0_PASS'] else 1


if __name__ == '__main__':
    sys.exit(main())
