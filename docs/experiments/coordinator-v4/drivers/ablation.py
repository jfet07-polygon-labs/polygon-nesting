#!/usr/bin/env python3
"""Which of the three changes bought which millimetres, one key at a time.

Three changes landed together, so the headline is a joint number and the
attribution has to be measured rather than apportioned. Work-budget mode is
deterministic and load-independent, so one run per cell is the whole
measurement and a shared box cannot move it.

    ablation.py NAME REQUEST SEEDS WORK

Arms, all from one binary and one budget:

    v3          the merged-HEAD reference: sched=0, barren=0, divq=0
    sched       the mode-34 class alone
    barren      the global barren patience alone
    divq        the ranked-and-auditioned diversify class alone
    v4          all three, which is the shipping configuration
"""
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

ARMS = (
    ('v3', 'sched=0,barren=0,divq=0'),
    ('sched', 'sched=1,barren=0,divq=0'),
    ('barren', 'sched=0,divq=0'),
    ('divq', 'sched=0,barren=0'),
    ('v4', ''),
)


def main():
    name = sys.argv[1]
    request = sys.argv[2]
    seeds = [int(value) for value in sys.argv[3].split(',')]
    work = int(sys.argv[4])
    out_dir = f'{runlib.OUT}/{name}'
    result = {'name': name, 'request': request, 'work': work,
              'binary': runlib.BIN,
              'arms': [{'label': a, 'keys': k} for a, k in ARMS],
              'rows': []}
    for seed in seeds:
        for label, keys in ARMS:
            spec = runlib.spec_for(seed, 'work', work, True, keys)
            tag = f'{label}-s{seed}'
            doc, wall, err = runlib.run(runlib.BIN, request, seed, spec,
                                        f'{out_dir}/runs/{tag}.json')
            row = runlib.summarize(tag, doc, wall)
            row.update({'arm': label, 'seed': seed, 'spec': spec})
            result['rows'].append(row)
            print(f"{tag}: raw={row.get('rawDepthMm')} "
                  f"dual={row.get('dualGateValid')} "
                  f"spent={row.get('workUnits')} "
                  f"exit={(row.get('schedule') or {}).get('exitCause')} "
                  f"iters={(row.get('schedule') or {}).get('iterations')}",
                  flush=True)
    base = {row['seed']: row for row in result['rows'] if row['arm'] == 'v3'}
    summary = {}
    for label, _ in ARMS:
        deltas = [row['rawDepthMm'] - base[row['seed']]['rawDepthMm']
                  for row in result['rows'] if row['arm'] == label
                  and row.get('rawDepthMm') is not None
                  and base.get(row['seed'], {}).get('rawDepthMm') is not None]
        summary[label] = {
            'perSeed': {str(row['seed']): row.get('rawDepthMm')
                        for row in result['rows'] if row['arm'] == label},
            'deltaVsV3': {'median': statistics.median(deltas) if deltas else None,
                          'min': min(deltas) if deltas else None,
                          'max': max(deltas) if deltas else None},
            'coordinatorSeconds': {
                str(row['seed']): row.get('coordinatorSeconds')
                for row in result['rows'] if row['arm'] == label},
            'scheduleIterations': {
                str(row['seed']): (row.get('schedule') or {}).get('iterations')
                for row in result['rows'] if row['arm'] == label},
            'scheduleExit': {
                str(row['seed']): (row.get('schedule') or {}).get('exitCause')
                for row in result['rows'] if row['arm'] == label},
        }
    result['summary'] = summary
    os.makedirs(out_dir, exist_ok=True)
    json.dump(result, open(f'{out_dir}/ablation.json', 'w'), indent=1)
    print(json.dumps(summary, indent=1))


if __name__ == '__main__':
    main()
