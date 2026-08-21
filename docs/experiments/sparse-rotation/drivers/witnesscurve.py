#!/usr/bin/env python3
"""What the SE(2) certificate's witness is worth **as a function of what it is
allowed to spend**, on the parents a schedule slice actually walks.

    witnesscurve.py OUTDIR BINARY PARENTSJSON TRUST ITERS[,ITERS...]

docs/experiments/se2-rigidity/ ran the certificate at 20,000 iterations across
four programs and reported witnesses of 0.003-0.5 mm on four *record-line*
parents. Design C cannot spend that: a mode-34 slice is 0.78 s whole at a
ten-second wall. So the question this driver asks is the one that decides
whether design C can exist at all - **how much of the witness survives an
iteration budget a slice can afford** - and it asks it on the twelve 171-179 mm
parents the compression schedule is actually pointed at, not on the record line.

It runs the certificate as the read-only diagnostic it is
(`POLYGON_NESTING_SE2_CERTIFICATE`), so nothing here shares code with the
in-search invocation and the two are independent measurements of the same
object. The statistic per cell is the depth-only SE(2) program's
`witness.deltaMm` - the exactly-validated reduction, the only constructive
number the certificate produces - and the wall the whole call took.

The call this driver times runs **four** programs; design C runs one. The
per-program share is reported so the two are comparable, and it is a share
rather than a separate measurement because the row build is done once and
shared, so a quarter of the total is an over-estimate of one program's cost and
is used as one.
"""
import hashlib
import json
import os
import statistics
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402


def run_certificate(binary, seed, fixture, trust, iters, out_path, allowance):
    args = [a.format(seed=seed) for a in runlib.ARGS]
    # The certificate reads the pinned parent and returns before any search, so
    # the mode and target in the tail are inert; they are the pinned positional
    # contract and are passed unchanged.
    tail = ['34', fixture, '400.0', '', allowance]
    command = [binary, runlib.REQUESTS['mixed-61']] + args + tail
    env = dict(os.environ)
    env['POLYGON_NESTING_SE2_CERTIFICATE'] = f'trust={trust},iters={iters}'
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    started = time.monotonic()
    with open(out_path, 'w') as handle:
        proc = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                              check=False, env=env)
    wall = time.monotonic() - started
    try:
        return json.load(open(out_path)), wall, ''
    except json.JSONDecodeError:
        return None, wall, (proc.stderr or b'').decode()[-800:]


def headline(certificate):
    """The depth-only SE(2) program - the column design C actually solves.

    The names are compared case-insensitively because the certificate serializes
    its enums in camelCase (`depthOnly`, `se2`) while the Rust variants are
    `DepthOnly` and `Se2`; matching the variant spelling silently selects
    nothing and reports every cell as `None`, which is how the first run of this
    driver produced an empty summary that looked like a negative result.
    """
    for program in certificate.get('programs', []):
        if (program['program'].lower() == 'depthonly'
                and program['motion'].lower() == 'se2'):
            return program
    return None


def main():
    outdir, binary, parents_json = sys.argv[1], sys.argv[2], sys.argv[3]
    trust = sys.argv[4]
    iterations = [int(v) for v in sys.argv[5].split(',')]
    allowance = sys.argv[6] if len(sys.argv) > 6 else runlib.DEFAULT_ALLOWANCE
    parents = json.load(open(parents_json))['rows']
    os.makedirs(outdir, exist_ok=True)
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'trust': trust, 'iterations': iterations, 'allowance': allowance,
        'parents': parents_json, 'cells': [],
    }
    for iters in iterations:
        for parent in parents:
            seed = parent['seed']
            path = f'{outdir}/seed{seed}-i{iters}.json'
            doc, wall, err = run_certificate(binary, seed, parent['fixture'],
                                             trust, iters, path, allowance)
            cell = {'seed': seed, 'iterations': iters, 'wallSeconds': wall}
            if doc is None:
                cell['error'] = err
            else:
                program = headline(doc)
                witness = (program or {}).get('witness') or {}
                cell.update({
                    'publishedDepthMm': doc.get('publishedDepthMm'),
                    'rows': doc.get('rows'),
                    'deltaMm': witness.get('deltaMm'),
                    'scale': witness.get('scale'),
                    'fullVectorExactValid':
                        witness.get('fullVectorExactValid'),
                    'movedPieces': witness.get('movedPieces'),
                    'validations': witness.get('validations'),
                    'maxAbsDthetaDeg': witness.get('maxAbsDthetaDeg'),
                    'primalLowerMm': (program or {}).get('lp', {})
                    .get('primalLowerMm'),
                    'dualUpperMm': (program or {}).get('lp', {})
                    .get('dualUpperMm'),
                    'verdict': (program or {}).get('verdict'),
                })
            print(f"i{iters} seed{seed}: delta={cell.get('deltaMm')} "
                  f"scale={cell.get('scale')} wall={wall:.2f}s", flush=True)
            result['cells'].append(cell)
            json.dump(result, open(f'{outdir}/witnesscurve.json', 'w'),
                      indent=1)

    summary = {}
    for iters in iterations:
        rows = [c for c in result['cells']
                if c['iterations'] == iters and c.get('deltaMm') is not None]
        if not rows:
            continue
        deltas = [c['deltaMm'] for c in rows]
        walls = [c['wallSeconds'] for c in rows]
        summary[str(iters)] = {
            'cells': len(rows),
            'medianDeltaMm': statistics.median(deltas),
            'maxDeltaMm': max(deltas),
            'positiveCells': sum(1 for d in deltas if d > 0),
            'zeroCells': sum(1 for d in deltas if d == 0),
            'medianScale': statistics.median([c['scale'] for c in rows]),
            'medianWallSeconds': statistics.median(walls),
            # Four programs share one row build; a quarter is therefore an
            # over-estimate of the one program design C solves, used as one.
            'medianPerProgramSeconds': statistics.median(walls) / 4.0,
            'medianValidations': statistics.median(
                [c['validations'] for c in rows]),
        }
    result['summary'] = summary
    json.dump(result, open(f'{outdir}/witnesscurve.json', 'w'), indent=1)
    print(json.dumps(summary, indent=1))


if __name__ == '__main__':
    main()
