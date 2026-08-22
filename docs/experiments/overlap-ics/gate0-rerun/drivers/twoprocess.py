#!/usr/bin/env python3
"""Two-process determinism, on **every** cell document of the re-run.

    python3 twoprocess.py

The previous round compared two processes on S0 and S1 only. The re-run's
instruction is every cell, so every cell is here: S0, S1, S2, C175 x3, C168,
random-T, triangle-20, the 1,000- and 10,000-state corpora, and throughput.

Each cell is run in two separate processes with identical arguments and the
**entire** JSON document is compared after stripping one named field list,
`lib.WALL_FIELDS` = `['wall']`. Every clock reading the binary takes is inside
that object precisely so this comparison cannot be weakened by adding a field
somewhere else.

**Throughput is the one cell that cannot be bit-identical and is not claimed
to be.** Every number in it is a timing. It is run twice anyway and its four
verdict booleans are compared, which is the strongest claim the cell admits;
the row says so rather than quietly excluding it.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402
import cells  # noqa: E402

S1_W = cells.S1_LOCKED_W_MM


def pair(name, cell, request, out, **options):
    first, _, status_a, err_a = lib.run(
        cell, request, f'{out}/twoprocess/{name}-a.json', **options)
    second, _, status_b, err_b = lib.run(
        cell, request, f'{out}/twoprocess/{name}-b.json', **options)
    identical = (status_a == 0 and status_b == 0
                 and lib.stripped(first) == lib.stripped(second))
    return {
        'cell': name,
        'exitA': status_a,
        'exitB': status_b,
        'stderrA': err_a,
        'stderrB': err_b,
        'digestA': lib.digest(first),
        'digestB': lib.digest(second),
        'strippedFields': lib.WALL_FIELDS,
        'comparison': 'whole document minus the wall object',
        'bitIdentical': bool(identical),
    }


def throughput_pair(out):
    first, _, status_a, err_a = lib.run(
        'throughput', 'mixed-61', f'{out}/twoprocess/throughput-a.json',
        repeats=300, proposals=20_000, seed=0)
    second, _, status_b, err_b = lib.run(
        'throughput', 'mixed-61', f'{out}/twoprocess/throughput-b.json',
        repeats=300, proposals=20_000, seed=0)
    keys = ['coldPhiUnder200us', 'rowRebuildUnder20us',
            'cellGapAtLeast1MPerSecond', 'projectedAtLeast100K', 'pass']
    a = {key: first.get('throughput', {}).get(key) for key in keys}
    b = {key: second.get('throughput', {}).get(key) for key in keys}
    return {
        'cell': 'throughput',
        'exitA': status_a,
        'exitB': status_b,
        'stderrA': err_a,
        'stderrB': err_b,
        'comparison': 'the four verdict booleans only - every number in this '
                      'cell is a timing, so bit-identity is not claimed',
        'verdictsA': a,
        'verdictsB': b,
        'bitIdentical': None,
        'verdictsIdentical': bool(status_a == 0 and status_b == 0 and a == b),
    }


def main():
    out = os.environ.get('ICS_OUT', lib.OUT)
    os.makedirs(f'{out}/twoprocess', exist_ok=True)
    rows = [
        pair('s0', 's0', 'mixed-61', out, poses=lib.SPARROW_POSES,
             target=S1_W, budget=0, seed=0),
        pair('s1', 's1', 'mixed-61', out, poses=lib.SPARROW_POSES,
             target=S1_W, budget=cells.CELL_BUDGET, seed=0,
             perturbmm=0.5, perturbdeg=2.0, checkpointevery=1),
        pair('s2', 's2', 'mixed-61', out, poses=lib.SPARROW_POSES,
             target=S1_W, budget=cells.CELL_BUDGET, seed=0,
             perturbmm=2.0, perturbdeg=10.0, checkpointevery=1),
        pair('c175-seed0', 'c175', 'mixed-61', out, seed=0,
             budget=cells.C175_BUDGET, checkpointevery=1),
        pair('c175-seed1', 'c175', 'mixed-61', out, seed=1,
             budget=cells.C175_BUDGET, checkpointevery=1),
        pair('c175-seed2', 'c175', 'mixed-61', out, seed=2,
             budget=cells.C175_BUDGET, checkpointevery=1),
        pair('triangle20', 'triangle', 'triangle-20', out,
             target=cells.TRIANGLE_W_MM, budget=cells.CELL_BUDGET, seed=0,
             checkpointevery=1),
        pair('c168', 'c168', 'mixed-61', out, target=cells.C168_W_MM,
             budget=cells.CELL_BUDGET, seed=0, checkpointevery=1),
        pair('randomt', 'randomt', 'mixed-61', out, target=cells.C168_W_MM,
             budget=cells.CELL_BUDGET, seed=0, jumps=8, checkpointevery=1),
        pair('corpus-1000', 'corpus', 'mixed-61', out,
             states=cells.CORPUS_STATES, seed=0),
        pair('corpus-10000', 'corpus', 'mixed-61', out,
             states=cells.HEAVY_CORPUS_STATES, seed=0),
        throughput_pair(out),
    ]
    document = {
        'experiment': 'overlap-ics',
        'battery': 'two-process-determinism-every-cell',
        'binary': lib.BIN,
        'rows': rows,
        'ALL_BIT_IDENTICAL': all(row['bitIdentical'] for row in rows
                                 if row['bitIdentical'] is not None),
        'THROUGHPUT_VERDICTS_IDENTICAL': rows[-1]['verdictsIdentical'],
    }
    print(json.dumps(document, indent=1))
    with open(f'{out}/determinism-two-process.json', 'w') as handle:
        json.dump(document, handle, indent=1)
    return 0 if document['ALL_BIT_IDENTICAL'] else 1


if __name__ == '__main__':
    sys.exit(main())
