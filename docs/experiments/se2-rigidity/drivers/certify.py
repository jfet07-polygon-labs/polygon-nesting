#!/usr/bin/env python3
"""Run the SE(2) rigidity certificate over the four pinned parents.

    python3 certify.py <binary> [outdir]

The certificate is a read-only diagnostic: the armed binary prints its JSON in
place of the search and exits, so nothing here publishes a layout or touches a
record. Each cell is one parent at one trust radius, and each cell contains
four programs - {depth-only, strip-coupled} x {translation-only, SE(2)}.

Every reported bracket is a REAL-ARITHMETIC bound on the LINEARIZED program,
evaluated in f64 with an outward rounding allowance. The only exactly-validated
number in the document is `witness.deltaMm`, which is the publication measure
run on the moved placements after `validate_publication` accepted them - and
even that is not a record claim, because `validate_publication` does not gate on
the collision envelope and `contractValid` was never run. See
`verify_witness.py`, which re-derives the same numbers out of engine.

Importing this module must not run anything: `verify_all.py` imports it for
`PARENTS` and `TRUST`, so the body lives under `main()`.
"""
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

TRUE = lib.TRUE

# label, fixture, the parent's own published depth, the trailing allowance.
#
# The two record-line parents replay under the record lineage's `0.0005`; the
# 156.418 and 171.238 parents are from-request and take `0.002`. The allowance
# feeds the effective settings the fixture's own depth check runs against, so a
# wrong one is rejected at load rather than quietly measured.
PARENTS = [
    ('155.264', f'{TRUE}/orientation-floor/pinned-fs-155.26442950833.json',
     155.264, '0.0005'),
    ('155.422', f'{TRUE}/record-line-cascade/pinned-fs-155.4223.json',
     155.422, '0.0005'),
    ('156.418', f'{TRUE}/record-line-cascade/pinned-fs-156.418.json',
     156.418, '0.002'),
    ('171.238', f'{TRUE}/from-scratch-171.238/pinned-parent-171.238.json',
     171.238, '0.002'),
]

# 6 microns is record-line-cascade's own mode-31 certification step size; 1 mm
# is the radius Sol review 6 §3 quotes the old branch's brackets at, and the one
# the 0.422 mm question has to be answered at.
TRUST = (0.006, 0.025, 0.1, 0.25, 0.5, 1.0)
REFERENCE = 0.422
ITERS = 20000


def certify(binary, parent, trust, allowance, target):
    argv = ([binary, lib.REQ]
            + [a.format(clamp='0', seed='5') for a in lib.ARGS]
            + ['22', parent, target, '', allowance])
    env = dict(os.environ)
    env['POLYGON_NESTING_SE2_CERTIFICATE'] = (
        f'trust={trust},iters={ITERS},reference={REFERENCE}')
    start = time.time()
    proc = subprocess.run(argv, capture_output=True, check=False, env=env)
    wall = time.time() - start
    try:
        return json.loads(proc.stdout), wall, None
    except json.JSONDecodeError:
        return None, wall, (proc.stderr or b'').decode()[-2000:]


def summarize(doc, label, trust, wall):
    summary = {'parent': label, 'trustMm': trust,
               'wallSeconds': round(wall, 1),
               'pieceCount': doc['pieceCount'],
               'rotatablePieceCount': doc['rotatablePieceCount'],
               'publishedDepthMm': doc['publishedDepthMm'],
               'stripBoundMm': doc['stripBoundMm'],
               'stripExcessMm': doc['stripExcessMm'],
               'rows': doc['rows'],
               'rowsByFamily': doc['rowsByFamily'],
               'parentWorstResidualMm': doc['parentWorstResidualMm'],
               'thetaCapMaxDeg': doc['thetaCapMaxDeg'],
               'verdict': doc['verdict'],
               'programs': []}
    for program in doc['programs']:
        witness = program.get('witness') or {}
        summary['programs'].append({
            'program': program['program'], 'motion': program['motion'],
            'deltaRows': program['deltaRows'],
            'lowerMm': program['lp']['primalLowerMm'],
            'upperMm': program['lp']['dualUpperMm'],
            'gapMm': program['lp']['gapMm'],
            'primalFeasible': program['lp']['primalFeasible'],
            'exactValid': witness.get('exactValid'),
            'witnessDeltaMm': witness.get('deltaMm'),
            'witnessScale': witness.get('scale'),
            'witnessDirection': witness.get('direction'),
            'fullVectorExactValid': witness.get('fullVectorExactValid'),
            'fullVectorRejection': witness.get('fullVectorRejection'),
            'maxAbsDthetaDeg': witness.get('maxAbsDthetaDeg'),
            'maxAbsTranslationMm': witness.get('maxAbsTranslationMm'),
            'movedPieces': witness.get('movedPieces'),
            'verdict': program['verdict'],
        })
    return summary


def main():
    binary = sys.argv[1]
    outdir = sys.argv[2] if len(sys.argv) > 2 else '/var/lib/t3/tmp/se2cert'
    os.makedirs(outdir, exist_ok=True)
    rows = []
    for label, path, depth, allowance in PARENTS:
        target = f'{depth + 0.8:.6f}'
        for trust in TRUST:
            doc, wall, error = certify(binary, path, trust, allowance, target)
            if doc is None:
                rows.append({'parent': label, 'trustMm': trust,
                             'error': error, 'wallSeconds': wall})
                print(f'!!! {label} trust={trust}: {error}', flush=True)
                continue
            json.dump(doc, open(f'{outdir}/cert-{label}-t{trust}.json', 'w'),
                      indent=1)
            summary = summarize(doc, label, trust, wall)
            rows.append(summary)
            head = summary['programs'][1]  # depth-only / SE(2)
            print(f'{label} trust={trust:<6} '
                  f'verdict={summary["verdict"]:<26} '
                  f'[{head["lowerMm"]}, {head["upperMm"]}] '
                  f'witness={head["witnessDeltaMm"]} '
                  f'scale={head["witnessScale"]} '
                  f'fullValid={head["fullVectorExactValid"]} '
                  f'({wall:.0f}s)', flush=True)

    json.dump({'binary': binary, 'reference': REFERENCE, 'iterations': ITERS,
               'rows': rows}, open(f'{outdir}/certify.json', 'w'), indent=1)
    print(f'\nwrote {outdir}/certify.json')


if __name__ == '__main__':
    main()
