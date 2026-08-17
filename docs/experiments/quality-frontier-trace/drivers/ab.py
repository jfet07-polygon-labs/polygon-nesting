#!/usr/bin/env python3
"""Paired interleaved A/B on the mode-22 gate stream.

Two other agents benchmark on this box, so an absolute second is not a
measurement here. The statistic is the per-round paired ratio B/A with the arm
order alternating every round, which is the convention every timing claim in
docs/next-generation-engine-plan.md uses.

Usage: ab.py ROUNDS A_LABEL A_BIN B_LABEL B_BIN [--sink-b DIR] [--profile-b]
"""
import json
import os
import statistics
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

PARENT = f'{lib.TRUE}/record-159.092/pinned-parent-159.092.json'
TARGET = '159.892624'


def one(binary, sink, profile, tag, out_dir):
    argv = ([binary, lib.REQ] + [a.format(clamp='0', seed='5') for a in lib.ARGS]
            + ['22', PARENT, TARGET, '', '0.0005'])
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_QUALITY_TRACE', None)
    env.pop('POLYGON_NESTING_PROFILE', None)
    if sink:
        os.makedirs(sink, exist_ok=True)
        env['POLYGON_NESTING_QUALITY_TRACE'] = f'{sink}/{tag}.jsonl'
    if profile:
        env['POLYGON_NESTING_PROFILE'] = '1'
    path = f'{out_dir}/{tag}.json'
    started = time.monotonic()
    with open(path, 'w') as handle:
        subprocess.run(argv, stdout=handle, stderr=subprocess.DEVNULL,
                       check=False, env=env)
    seconds = time.monotonic() - started
    doc = json.load(open(path))
    pop = doc['relaxedDiagnostics']['coupledDynamicSeparator'][
        'persistentVacancyPopulation']
    return seconds, pop.get('rawSourceDepthMm'), pop.get(
        'finalPlacementFingerprint')


def main():
    rounds = int(sys.argv[1])
    a_label, a_bin, b_label, b_bin = sys.argv[2:6]
    sink_b = None
    profile_b = '--profile-b' in sys.argv
    if '--sink-b' in sys.argv:
        sink_b = sys.argv[sys.argv.index('--sink-b') + 1]
    out_dir = f'/var/lib/t3/tmp/qft/ab/{a_label}-vs-{b_label}'
    os.makedirs(out_dir, exist_ok=True)
    rows = []
    for round_index in range(rounds):
        if round_index % 2 == 0:
            a = one(a_bin, None, False, f'r{round_index}-a', out_dir)
            b = one(b_bin, sink_b, profile_b, f'r{round_index}-b', out_dir)
        else:
            b = one(b_bin, sink_b, profile_b, f'r{round_index}-b', out_dir)
            a = one(a_bin, None, False, f'r{round_index}-a', out_dir)
        rows.append({
            'round': round_index,
            'firstArm': 'a' if round_index % 2 == 0 else 'b',
            'aSeconds': a[0], 'bSeconds': b[0], 'ratio': b[0] / a[0],
            'aRaw': a[1], 'bRaw': b[1],
            'sameRaw': a[1] == b[1], 'sameFingerprint': a[2] == b[2],
        })
        print(json.dumps(rows[-1]), flush=True)
    ratios = sorted(row['ratio'] for row in rows)
    summary = {
        'aLabel': a_label, 'bLabel': b_label,
        'aBinary': a_bin, 'bBinary': b_bin,
        'sinkB': sink_b, 'profileB': profile_b,
        'rounds': rounds,
        'aMedianSeconds': statistics.median(row['aSeconds'] for row in rows),
        'bMedianSeconds': statistics.median(row['bSeconds'] for row in rows),
        'pairedRatioMedian': statistics.median(ratios),
        'pairedRatioMin': ratios[0],
        'pairedRatioMax': ratios[-1],
        'roundsBelowParity': sum(1 for r in ratios if r < 1.0),
        'allOutcomesIdentical': all(row['sameRaw'] and row['sameFingerprint']
                                    for row in rows),
        'rows': rows,
    }
    print(json.dumps(summary, indent=1))
    json.dump(summary, open(f'{out_dir}/summary.json', 'w'), indent=1)


if __name__ == '__main__':
    main()
