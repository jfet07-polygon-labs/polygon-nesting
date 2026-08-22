#!/usr/bin/env python3
"""The v3-coordinator arm battery: the reachability A/B and the anytime table.

    coordarm.py OUTDIR BINARY REQUEST SEEDS ROUNDS KEY VALUES ARMS

`ARMS` is a comma-separated list of `label[:extra]`, where `extra` is appended
to the portfolio spec with `;` standing in for `,` (the argument separator is
already taken). `KEY` is `work`, `wall` or `plan`; `VALUES` is a comma-separated
ladder of budgets in that key's own unit.

A diffable copy of `docs/experiments/continuous-rotation/drivers/battery.py`'s
loop with the arm table taken from the command line, `coordlib.spec_for`'s
`v3=1`, the same salt sets and the same pinned positional tail. Arms are run
back to back within a round and the arm order rotates every round, because
another agent benchmarks on this box concurrently and an unpaired number here
would be worthless.

# The two questions this serves

* **reachability** - Grok review 7 §3's co-requirement, in the cheap form Sol
  review 12 §3.3 endorsed: re-run the *existing* rotation arms, which are
  measured negatives under the miter authority, with the round-envelope kernel
  armed. `crot` expressed 46 of Sparrow's 61 off-lattice poses and cost
  **+3.721 mm** at ten seconds on mixed-61, 0 of 9 rounds better. The question
  is whether that tax changes sign once the authority stops refusing off-lattice
  poses. This is a diagnostic; nothing here is a promotion.
* **anytime** - the binding user priority: mixed-61 from a bare request at
  3/10/30 s, kernel-armed against canonical, beside Sparrow's 150.165 @ 10 s.

# Off-lattice census

Every published placement's `rotationDeg` is checked against the 2.5 degree
lattice the default candidate stream can name, and against 1.0 degree, with the
same reduction Gate A used. A kernel-armed publication that uses no off-lattice
pose has not reached anything the miter authority could not have reached.
"""
import hashlib
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import coordlib  # noqa: E402

KERNEL_ENV = 'POLYGON_NESTING_ROUND_ENVELOPE_KERNEL'
ROUND_ENV = ('POLYGON_NESTING_CONTACT_BLOCK', 'POLYGON_NESTING_SE2_WITNESS',
             'POLYGON_NESTING_CONTINUOUS_ROTATION',
             'POLYGON_NESTING_SPARSE_ROTATION',
             'POLYGON_NESTING_COMPRESSION_SCHEDULE', KERNEL_ENV)
LATTICES = (2.5, 1.0)
# Gate A's own tolerance for "on the lattice": a published rotation is a printed
# f64 and the constructor's own grid values do not come back bit-exact.
LATTICE_TOLERANCE_DEG = 1e-9


def off_lattice(placements, step):
    """How many published poses are off the `step`-degree lattice."""
    count = 0
    worst = 0.0
    for placement in placements or []:
        angle = placement.get('rotationDeg')
        if angle is None:
            continue
        residue = angle % step
        distance = min(residue, step - residue)
        if distance > LATTICE_TOLERANCE_DEG:
            count += 1
            worst = max(worst, distance)
    return count, worst


def spec_for(seed, key, value, extra):
    spec = coordlib.spec_for(seed, key, value, True)
    return spec + (',' + extra if extra else '')


def run_cell(binary, request, seed, spec, out_path):
    doc, wall, err = coordlib.run(binary, request, seed, spec, out_path)
    portfolio = doc.get('portfolio') or {}
    incumbent = portfolio.get('incumbent') or {}
    placements = doc.get('placements')
    row = {
        'spec': spec,
        'processWallSeconds': wall,
        'rawDepthMm': incumbent.get('rawDepthMm'),
        'engineDepthMm': doc.get('independentUsedLongAxisDepthMm'),
        'usedLongAxisDepthMm': doc.get('usedLongAxisDepthMm'),
        'dualGateValid': incumbent.get('dualGateValid'),
        'incumbentSource': incumbent.get('source'),
        'publishedSeconds': incumbent.get('publishedSeconds'),
        'publishedWorkUnits': incumbent.get('publishedWorkUnits'),
        'coordinatorSeconds': portfolio.get('elapsedSeconds'),
        'workUnits': portfolio.get('workUnits'),
        'publications': portfolio.get('publications'),
        'placementCount': len(placements or []),
        'reportedKernelMode': (doc.get('roundEnvelopeKernel') or {}).get('mode'),
        'error': doc.get('_loadError') or (err[-400:] if err else None),
    }
    for step in LATTICES:
        count, worst = off_lattice(placements, step)
        row[f'offLattice{str(step).replace(".", "p")}Count'] = count
        row[f'offLattice{str(step).replace(".", "p")}WorstDeg'] = worst
    row['distinctRotations'] = len({p.get('rotationDeg')
                                    for p in (placements or [])})
    return row


def main():
    outdir, binary, request = sys.argv[1], sys.argv[2], sys.argv[3]
    seeds = [int(v) for v in sys.argv[4].split(',')]
    rounds = int(sys.argv[5])
    key = sys.argv[6]
    values = sys.argv[7].split(',')
    arms = []
    for item in sys.argv[8].split(','):
        label, _, extra = item.partition(':')
        arms.append((label, extra.replace(';', ',')))
    for name in ROUND_ENV:
        os.environ.pop(name, None)
    os.makedirs(outdir, exist_ok=True)
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'request': request,
        'seeds': seeds,
        'rounds': rounds,
        'budgetKey': key,
        'budgetValues': values,
        'arms': [{'label': a, 'extra': e} for a, e in arms],
        'rows': [],
    }
    for value in values:
        for index in range(rounds):
            for seed in seeds:
                ordered = arms[index % len(arms):] + arms[:index % len(arms)]
                for label, extra in ordered:
                    tag = f'{label}-{key}{value}-s{seed}-r{index}'
                    row = run_cell(binary, request, seed,
                                   spec_for(seed, key, value, extra),
                                   f'{outdir}/{tag}.json')
                    row.update({'arm': label, 'seed': seed, 'round': index,
                                'budget': f'{key}={value}', 'tag': tag})
                    result['rows'].append(row)
                    print(f"{tag}: raw={row['rawDepthMm']} "
                          f"dual={row['dualGateValid']} "
                          f"coord={row['coordinatorSeconds']} "
                          f"work={row['workUnits']} "
                          f"off2p5={row['offLattice2p5Count']} "
                          f"rek={row['reportedKernelMode']} "
                          f"wall={row['processWallSeconds']:.2f}s", flush=True)
                    json.dump(result, open(f'{outdir}/coordarm.json', 'w'),
                              indent=1)
    json.dump(result, open(f'{outdir}/coordarm.json', 'w'), indent=1)
    # Paired medians per budget, per arm, and every arm against the first.
    base = arms[0][0]
    summary = {}
    for value in values:
        cells = {}
        for row in result['rows']:
            if row['budget'] != f'{key}={value}':
                continue
            cells.setdefault(row['arm'], {})[(row['seed'], row['round'])] = row
        block = {}
        for label, _ in arms:
            rows = cells.get(label, {})
            depths = [r['rawDepthMm'] for r in rows.values()
                      if r['rawDepthMm'] is not None]
            block[label] = {
                'n': len(depths),
                'medianRawDepthMm': statistics.median(depths) if depths else None,
                'minRawDepthMm': min(depths) if depths else None,
                'allDualGateValid': all(r['dualGateValid'] for r in rows.values()),
                'medianOffLattice2p5': statistics.median(
                    [r['offLattice2p5Count'] for r in rows.values()]) if rows else None,
            }
            if label != base:
                paired = []
                for cellkey, row in rows.items():
                    other = cells.get(base, {}).get(cellkey)
                    if other and row['rawDepthMm'] is not None \
                            and other['rawDepthMm'] is not None:
                        paired.append(row['rawDepthMm'] - other['rawDepthMm'])
                block[label]['pairedVsBaseMm'] = {
                    'n': len(paired),
                    'median': statistics.median(paired) if paired else None,
                    'better': sum(1 for d in paired if d < 0),
                    'range': [min(paired), max(paired)] if paired else None,
                }
        summary[f'{key}={value}'] = block
    result['summary'] = summary
    json.dump(result, open(f'{outdir}/coordarm.json', 'w'), indent=1)
    print(json.dumps(summary, indent=1))


if __name__ == '__main__':
    main()
