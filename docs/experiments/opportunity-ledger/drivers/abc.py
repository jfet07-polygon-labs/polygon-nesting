#!/usr/bin/env python3
"""Part 2: the A/B/C probe at identical work, paired per seed.

    python3 abc.py TAG PROBE_WORK [SEEDS] [ARMS] [BINARY]

Every arm shares the same base schedule at the same work budget, so the state
each arm starts from is the *same saturated archive* by construction - the
probe is a sixth phase that runs after the drain, and the work-budget mode is
deterministic, which the ledger driver's `--twice` check pins.

The statistic is the raw depth of the best exact-valid publication, paired per
seed, plus the work the arm spent. There is no wall-clock claim here: the
seconds are reported because arm B and arm C are mispriced by the work meter in
opposite directions and the reader has to see both numbers.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

ARMS = {'A': 'A', 'B': 'B', 'C': 'C'}


def main():
    tag = sys.argv[1]
    probe_work = int(sys.argv[2])
    seeds = [int(s) for s in (sys.argv[3] if len(sys.argv) > 3
                              else '0,1,2').split(',')]
    arms = (sys.argv[4] if len(sys.argv) > 4 else 'A,B,C').split(',')
    binary = sys.argv[5] if len(sys.argv) > 5 else '/var/lib/t3/tmp/ledger-bin'
    base_work = runlib.WORK_30S
    result = {'tag': tag, 'binary': binary, 'baseWork': base_work,
              'probeWork': probe_work, 'seeds': seeds, 'arms': arms,
              'allowance': runlib.DEFAULT_ALLOWANCE, 'rows': []}
    for seed in seeds:
        for arm in arms:
            spec = runlib.spec_for(
                seed, base_work, f'probe={arm},probeWork={probe_work}')
            doc, wall, err = runlib.run(
                binary, 'mixed-61', seed, spec,
                f'{runlib.OUT}/abc/{tag}-seed{seed}-{arm}.json')
            if '_loadError' in doc:
                result['rows'].append({'seed': seed, 'arm': arm,
                                       'error': err[-600:]})
                continue
            portfolio = doc['portfolio']
            probe = portfolio.get('probe') or {}
            calls = [c for c in portfolio['operatorCalls']
                     if c['phase'] == 'probe']
            result['rows'].append({
                'seed': seed,
                'arm': arm,
                'spec': spec,
                'processWallSeconds': wall,
                'entryRawDepthMm': probe.get('entryRawDepthMm'),
                'exitRawDepthMm': probe.get('exitRawDepthMm'),
                'deltaRawMm': probe.get('deltaRawMm'),
                'exitDualGateValid': probe.get('exitDualGateValid'),
                'probeWorkUnitsSpent': probe.get('workUnitsSpent'),
                'probeSecondsSpent': probe.get('secondsSpent'),
                'probePublications': probe.get('publications'),
                'probeOperatorCalls': probe.get('operatorCalls'),
                'probeSteps': probe.get('steps'),
                'probeExitCause': probe.get('exitCause'),
                'incumbentRawDepthMm': portfolio['incumbent']['rawDepthMm'],
                'incumbentDualGateValid':
                    portfolio['incumbent']['dualGateValid'],
                'totalWorkUnits': portfolio['workUnits'],
                'probeCalls': [{
                    'operator': c['operator'],
                    'action': c['action'],
                    'workUnits': c['workUnits'],
                    'elapsedSeconds': c['elapsedSeconds'],
                    'exactValid': c['exactValid'],
                    'rawDepthMm': c['rawDepthMm'],
                    'archiveDisposition': c['archiveDisposition'],
                    'published': c['published'],
                    'failureReason': c['failureReason'],
                } for c in calls],
            })
    print(json.dumps(result, indent=1))


if __name__ == '__main__':
    main()
