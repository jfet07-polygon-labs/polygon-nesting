#!/usr/bin/env python3
"""Locate the invocation the pinned mode-20 anchor 206.869 / 8a773738 came from.

Run on BOTH binaries so any match is simultaneously a parity check.
"""

import json
import os
import subprocess
import sys

sys.path.insert(0, '/var/lib/t3/tmp/mode31')
from run import ARGS, ROOT  # noqa: E402

OUT = '/var/lib/t3/tmp/mode31/anchor20'
ANCHOR = '/var/lib/t3/tmp/ex5-seed-native.json'
REQUESTS = {
    'exact': f'{ROOT}/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json',
    'plain': f'{ROOT}/tests/fixtures/mixed-61/mixed61-request.json',
}
ALLOWANCES = ['0.0005', '0.002', None]


def run(binary, tag, request, allowance, seed=0):
    os.makedirs(OUT, exist_ok=True)
    argv = [binary, request] + [a.format(clamp='0', seed=seed) for a in ARGS] + [
        '20', ANCHOR, '320.000', '']
    if allowance is not None:
        argv.append(allowance)
    path = f'{OUT}/{tag}.json'
    with open(path, 'w') as out:
        subprocess.run(argv, stdout=out, stderr=subprocess.DEVNULL, check=False)
    try:
        with open(path) as handle:
            data = json.load(handle)
    except Exception:
        return 'crashed'
    pop = (data['relaxedDiagnostics']['coupledDynamicSeparator']
           .get('persistentVacancyPopulation'))
    if pop is None:
        return 'no population'
    return (f"depth={pop.get('independentDepthMm')} "
            f"fp={(pop.get('finalPlacementFingerprint') or '')[:16]} "
            f"exactValid={pop.get('exactValid')}")


if __name__ == '__main__':
    for name, request in REQUESTS.items():
        for allowance in ALLOWANCES:
            label = f'{name}-{allowance or "default"}'
            base = run('/var/lib/t3/tmp/mode31-baseline', f'{label}-base',
                       request, allowance)
            treat = run('/var/lib/t3/tmp/mode31-bench', f'{label}-treat',
                        request, allowance)
            same = 'SAME' if base == treat else 'DIFFERS'
            print(f'{label}: {same}\n    base : {base}\n    treat: {treat}',
                  flush=True)
