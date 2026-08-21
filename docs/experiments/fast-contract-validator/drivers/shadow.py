#!/usr/bin/env python3
"""Drives the release shadow corpus over several seeds and reports the totals.

    python3 shadow.py OUTDIR SHADOW_BINARY CASES SEED[,SEED...]

The binary is `examples/contract_validator_shadow.rs`, built `--release`. Each
seed is an independent corpus; the seeds are run as separate processes so a
crash localises and so the per-seed numbers can be read against each other
rather than only in aggregate.

Three numbers decide whether the zero means anything, and all three are carried
per seed rather than summed away:

  `provedClear`                 pairs the filter actually certified. A corpus
                                that certifies nothing has tested nothing.
  `tightestCertifiedExcessMm`   how close to the clearance the tightest
                                certificate came. A corpus whose tightest
                                certificate sits millimetres clear has not
                                probed the margin, whatever its pair count.
  `rejectedLayouts`             layouts the validator refused. If this is zero
                                the corpus is all legal geometry and the
                                verdict comparison is vacuous.

`verdictMismatches` counts disagreements between `validate_publication` and
`validate_publication_exact_reference` on the whole `Result` including the error
message; `auditMismatches` counts certificates that the two bypassed tests then
contradicted, per pair. Both must be zero and the harness exits non-zero if they
are not - so a green run here is an exit code, not a reading.
"""
import hashlib
import json
import os
import subprocess
import sys
import time


def main():
    outdir, binary, cases = sys.argv[1], sys.argv[2], sys.argv[3]
    seeds = sys.argv[4].split(',')
    os.makedirs(outdir, exist_ok=True)
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'casesPerSeed': int(cases),
        'seeds': seeds,
        'profile': 'release (debug_assertions off) - the assertions here are '
                   'explicit branches, not debug_assert',
        'perSeed': {},
    }
    for seed in seeds:
        path = f'{outdir}/shadow-seed{seed}.json'
        started = time.monotonic()
        with open(path, 'w') as handle:
            proc = subprocess.run([binary, cases, seed], stdout=handle,
                                  stderr=subprocess.PIPE, check=False)
        wall = time.monotonic() - started
        try:
            row = json.load(open(path))
        except json.JSONDecodeError:
            row = {'error': (proc.stderr or b'').decode()[-500:]}
        row['exitCode'] = proc.returncode
        row['processWallSeconds'] = wall
        row['stderr'] = (proc.stderr or b'').decode()[-400:]
        result['perSeed'][seed] = row
        print(f'seed {seed}: exit={proc.returncode} '
              f'pairs={row.get("pairs")} clear={row.get("provedClear")} '
              f'tightest={row.get("tightestCertifiedExcessMm")}',
              file=sys.stderr)
        json.dump(result, open(f'{outdir}/shadow.json', 'w'), indent=1)

    rows = [r for r in result['perSeed'].values() if 'pairs' in r]
    result['totals'] = {
        'seeds': len(rows),
        'cases': sum(r['cases'] for r in rows),
        'pairs': sum(r['pairs'] for r in rows),
        'provedClear': sum(r['provedClear'] for r in rows),
        'domainRefusals': sum(r['domainRefusals'] for r in rows),
        'acceptedLayouts': sum(r['acceptedLayouts'] for r in rows),
        'rejectedLayouts': sum(r['rejectedLayouts'] for r in rows),
        'nearThresholdCases': sum(r['nearThresholdCases'] for r in rows),
        'verdictMismatches': sum(r['verdictMismatches'] for r in rows),
        'auditMismatches': sum(r['auditMismatches'] for r in rows),
        'tightestCertifiedExcessMm': min(
            (r['tightestCertifiedExcessMm'] for r in rows), default=None),
        'allExitZero': all(r['exitCode'] == 0
                           for r in result['perSeed'].values()),
    }
    result['totals']['provedClearRate'] = (
        result['totals']['provedClear'] / result['totals']['pairs']
        if result['totals']['pairs'] else None)
    result['ZERO_MISMATCH'] = (
        result['totals']['verdictMismatches'] == 0
        and result['totals']['auditMismatches'] == 0
        and result['totals']['allExitZero'])
    json.dump(result, open(f'{outdir}/shadow.json', 'w'), indent=1)
    print(json.dumps(result['totals'] | {'ZERO_MISMATCH': result['ZERO_MISMATCH']},
                     indent=1))


if __name__ == '__main__':
    main()
