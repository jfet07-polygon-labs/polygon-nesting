#!/usr/bin/env python3
"""Gate 0 for the T-row repair (`docs/t-row-repair-spec.md` §3).

Three arms - Control, TRepair, ComputeIgnore - on the nine mixed-61 seeds, at
the fixed-work residual that reproduces the specification's pre-declared
partition. One fresh process per arm and seed, and a second process per arm and
seed for the two-process bit-identity clause.

Exit 0 is PASS, 1 is a measured miss, 2 is an invalid instrument.
"""
import hashlib
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, '..', '..', '..', '..', '..'))
BIN = os.environ.get(
    'ICS_BIN', f'{ROOT}/target/release/examples/overlap_ics_benchmark')
REQUEST = (f'{ROOT}/tests/fixtures/mixed-61/'
           'mixed61-request-exact-clearance.json')
OUT = os.environ.get('T_ROW_OUT', '/var/lib/t3/tmp/bite22/gate0')

SEEDS = tuple(range(9))
CLOSED = (0, 2, 3, 6)
FROZEN = (1, 4, 5, 7, 8)
TAIL = (7, 8)
# The residual: the round's own 10-second composed cells spend one retry
# attempt and 821-1,424 master iterations on bite 22. Calibrated on the CONTROL
# alone, before any treatment cell ran, against the pre-declared partition.
ATTEMPTS = 2
ITERS = 500
ARMS = ('off', 'repair', 'computeignore')
# The frozen caps this gate checks publications against.
MAX_DISPLACEMENT_MM = 0.016
BAND_MM = 0.004


def run(arm, seed, repetition):
    path = f'{OUT}/{arm}-seed{seed}-r{repetition}.json'
    os.makedirs(OUT, exist_ok=True)
    argv = [
        BIN, '--cell=cutclose', f'--request={REQUEST}', '--edge=5', '--pair=5',
        '--mode=fixed', '--bites=22', f'--attempts={ATTEMPTS}',
        f'--iters={ITERS}', '--compressbites=0', '--orders=1', '--workers=8',
        '--arm=control', f'--trow={arm}', f'--seed={seed}', '--revalidate=1',
    ]
    with open(path, 'w') as handle:
        proc = subprocess.run(argv, stdout=handle, stderr=subprocess.PIPE,
                              check=False)
    with open(path) as handle:
        document = json.load(handle)
    return document, proc.returncode, (proc.stderr or b'').decode()[-400:]


# The wall is volatile; `tRowCensus` and `publishCensus` are the shadow
# diagnostics the specification's clause 7 says to strip before comparing
# `ComputeIgnore` against `Control` - the shadow arm records what it paid for
# and discarded, and that record is the point of it.
SHADOW = ('wall', 'tRowCensus', 'publishCensus')


def strip_wall(document, shadow=False):
    drop = SHADOW if shadow else ('wall',)
    copy = {k: v for k, v in document.items() if k not in drop}
    return json.dumps(copy, sort_keys=True)


def bite(document, ordinal):
    for row in document['outcome']['bites']:
        if row['ordinal'] == ordinal:
            return row
    return None


def main():
    cells = {}
    for arm in ARMS:
        for seed in SEEDS:
            for repetition in (0, 1):
                document, status, stderr = run(arm, seed, repetition)
                cells[(arm, seed, repetition)] = (document, status, stderr)

    doc = {}
    for key, (document, status, stderr) in cells.items():
        doc[f'{key[0]}-{key[1]}-r{key[2]}'] = {
            'exit': status, 'stderr': stderr,
            'digestWithoutWall': hashlib.sha256(
                strip_wall(document).encode()).hexdigest(),
        }

    def cell(arm, seed):
        return cells[(arm, seed, 0)][0]

    clauses = {}
    # ---- instrument validity: the control must reproduce the partition ----
    control_closed = [s for s in SEEDS if (bite(cell('off', s), 22) or {}).get('published')]
    control_open = [s for s in SEEDS if s not in control_closed]
    clauses['instrumentPartition'] = (
        tuple(control_closed) == CLOSED and tuple(control_open) == FROZEN)
    doc['controlClosed'] = control_closed
    doc['controlOpen'] = control_open

    # ---- clause 1: per-bite census integrity ----
    # bites 1..21 publish with proxy <= T, so the T-row census is bite 22's.
    integrity = True
    for seed in SEEDS:
        census = cell('repair', seed).get('tRowCensus') or {}
        total = (census.get('published', 0) + census.get('refused', 0))
        integrity &= census.get('eligible', 0) >= total
    clauses['censusIntegrity'] = integrity

    # ---- clause 2: unique install ----
    unique = True
    for seed in SEEDS:
        census = cell('repair', seed).get('tRowCensus') or {}
        if census.get('eligible', 0) > 0:
            unique &= census['eligible'] == census.get('eligibleWithTRow')
    clauses['uniqueInstall'] = unique

    # ---- clause 3: tail-relevant conversion ----
    converted = [s for s in FROZEN if (bite(cell('repair', s), 22) or {}).get('published')]
    doc['converted'] = converted
    clauses['conversion'] = (all(s in converted for s in TAIL)
                             and len([s for s in converted if s in (1, 4, 5)]) >= 1)

    # ---- clause 4: causal witness ----
    causal = True
    for seed in converted:
        census = cell('repair', seed).get('tRowCensus') or {}
        causal &= census.get('published', 0) >= 1 and census.get('eligibleWithTRow', 0) >= 1
    clauses['causalWitness'] = causal

    # ---- clause 5: no reverse ----
    reverse = []
    for seed in CLOSED:
        control_row = bite(cell('off', seed), 22) or {}
        repair_row = bite(cell('repair', seed), 22) or {}
        if control_row.get('published') and not repair_row.get('published'):
            reverse.append(seed)
        elif cell('repair', seed)['outcome']['depthMm'] > cell('off', seed)['outcome']['depthMm']:
            reverse.append(seed)
    doc['reverse'] = reverse
    clauses['noReverse'] = not reverse

    # ---- clause 6: authority and caps ----
    authority = True
    for seed in SEEDS:
        outcome = cell('repair', seed)['outcome']
        authority &= outcome.get('invalidPublications', 0) == 0
        authority &= (outcome.get('repairMaxDisplacementMm') or 0.0) <= MAX_DISPLACEMENT_MM
        row = bite(cell('repair', seed), 22)
        if row and row.get('published'):
            authority &= row['widthAfterMm'] >= outcome['depthMm'] - 1e-9
    clauses['authorityAndCaps'] = authority

    # ---- clause 7: isolation, cost, determinism ----
    two_process = all(
        doc[f'{arm}-{seed}-r0']['digestWithoutWall']
        == doc[f'{arm}-{seed}-r1']['digestWithoutWall']
        for arm in ARMS for seed in SEEDS)
    isolation = all(
        strip_wall(cell('computeignore', seed), shadow=True)
        == strip_wall(cell('off', seed), shadow=True)
        for seed in SEEDS)
    clauses['twoProcessIdentity'] = two_process
    clauses['computeIgnoreIsolation'] = isolation

    doc['clauses'] = clauses
    doc['residual'] = {'attempts': ATTEMPTS, 'iterationsPerSeparation': ITERS}
    doc['GATE0_PASS'] = bool(clauses['instrumentPartition']) and all(clauses.values())
    print(json.dumps(doc, indent=1, sort_keys=True))
    if not clauses['instrumentPartition']:
        return 2
    return 0 if doc['GATE0_PASS'] else 1


if __name__ == '__main__':
    sys.exit(main())
