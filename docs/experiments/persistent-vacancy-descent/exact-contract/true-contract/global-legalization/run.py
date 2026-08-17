#!/usr/bin/env python3
"""Driver for the global pressure-balanced legalization (modes 30/31) experiment.

Runs the frozen binary against the true-contract exact-clearance request with
the pinned CLI tail, and reports the fields the experiment is judged on.
"""

import json
import os
import subprocess
import sys

ROOT = '/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/agent-aaf6da94f4d0383f4'
BIN = os.environ.get('BENCH_BIN', '/var/lib/t3/tmp/mode31-bench')
REQ = f'{ROOT}/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json'
TRUE = (f'{ROOT}/docs/experiments/persistent-vacancy-descent/exact-contract/'
        'true-contract')
RECORD = f'{TRUE}/record-164.042/pinned-parent-164.042.json'
SCRATCH = f'{TRUE}/from-scratch-164.932/pinned-parent-164.932.json'
ARGS = ('1 4 0 0 0 0 1 0 0 1 1 0 16 4 8 {clamp} 0 5 5 24 8 40 10 10 5 {seed} '
        '0.005 0.001 1 6 0 0 0 structured 0 10 1 0 0 0 0').split()


def run(tag, mode, parent, target, seed, outdir, clamp='0', warm='',
        request=REQ, allowance='0.0005'):
    os.makedirs(outdir, exist_ok=True)
    argv = [BIN, request] + [a.format(clamp=clamp, seed=seed) for a in ARGS] + [
        str(mode), parent, str(target), warm, allowance]
    path = f'{outdir}/{tag}.json'
    with open(path, 'w') as out:
        subprocess.run(argv, stdout=out, stderr=subprocess.DEVNULL, check=False)
    with open(path) as handle:
        return json.load(handle)


def population(run_json):
    coupled = run_json['relaxedDiagnostics']['coupledDynamicSeparator']
    return coupled.get('persistentVacancyPopulation')


def line(tag, run_json):
    pop = population(run_json)
    if pop is None:
        arm = run_json['relaxedDiagnostics']['coupledDynamicSeparator'][
            'boundaryProjectionTreatment']
        return (f'{tag}: no population; separator accepted='
                f"{arm['targetsAccepted']} depth="
                f"{arm.get('independentlyMeasuredFinalDepthMm')}")
    return (f"{tag}: mode={pop['mode']} exactValid={pop['exactValid']} "
            f"contractValid={pop.get('contractValid')} "
            f"depth={pop.get('independentDepthMm')} "
            f"raw={pop.get('rawSourceDepthMm')} "
            f"parentDepth={pop.get('parentIndependentDepthMm')} "
            f"fp={(pop.get('finalPlacementFingerprint') or '')[:16]} "
            f"{(pop.get('failureReason') or '')[:80]}")


if __name__ == '__main__':
    print(line(sys.argv[1], run(*sys.argv[1:])))
