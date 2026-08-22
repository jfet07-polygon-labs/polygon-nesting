#!/usr/bin/env python3
"""The wall ratio between the two arms, measured with replicas rather than once.

    wallratio.py OUTDIR BINARY PARENTSJSON WORK SEEDS REPLICAS OUT.json

The matched gate's equal-wall reading rests entirely on one number - how much
less operator wall the armed run spends for the same work - because the two arms
produce *identical* depths at equal work. A quantity a whole verdict rests on
should not be measured once on a shared box.

So: the same cell, both arms, interleaved back to back, N times, with the arm
order alternating every replica. The depths are asserted identical across every
replica of a cell (the engine is deterministic in seed and work cap), which
makes this purely a timing measurement and lets the wall be reported as a
distribution instead of a point.
"""
import hashlib
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import matchedgate  # noqa: E402


def main():
    outdir, binary, parents_json = sys.argv[1:4]
    work = int(sys.argv[4])
    seeds = {int(v) for v in sys.argv[5].split(',')}
    replicas = int(sys.argv[6])
    out_path = sys.argv[7]
    parents = [p for p in json.load(open(parents_json))['rows']
               if p['seed'] in seeds]
    os.makedirs(outdir, exist_ok=True)
    result = {'binary': binary,
              'binarySha256': hashlib.sha256(open(binary, 'rb').read())
              .hexdigest(),
              'workCap': work, 'replicas': replicas, 'cells': []}
    for parent in parents:
        seed = parent['seed']
        target = parent['rawDepthMm'] - matchedgate.DEFAULT_DROP_MM
        cell = {'seed': seed, 'runs': []}
        for index in range(replicas):
            arms = ['miter', 'union'] if index % 2 == 0 else ['union', 'miter']
            for arm in arms:
                row = matchedgate.run_cell(
                    binary, arm, seed, parent['fixture'], target, work,
                    f'{outdir}/seed{seed}-{arm}-r{index}.json', '0.002')
                cell['runs'].append({'replica': index, 'arm': arm,
                                     'operatorWallSeconds':
                                         row.get('operatorWallSeconds'),
                                     'processWallSeconds':
                                         row.get('processWallSeconds'),
                                     'rawSourceDepthMm':
                                         row.get('rawSourceDepthMm'),
                                     'fingerprint': row.get('fingerprint'),
                                     'msPerConfirmation':
                                         row.get('msPerConfirmation'),
                                     'armReportedCorrectly':
                                         row.get('armReportedCorrectly')})
                print(f"seed{seed} r{index} {arm} "
                      f"opwall={row.get('operatorWallSeconds'):.3f}s "
                      f"depth={row.get('rawSourceDepthMm')}", flush=True)
        walls = {arm: [r['operatorWallSeconds'] for r in cell['runs']
                       if r['arm'] == arm] for arm in ('miter', 'union')}
        depths = {arm: {r['rawSourceDepthMm'] for r in cell['runs']
                        if r['arm'] == arm} for arm in ('miter', 'union')}
        cell['depthsIdenticalWithinArm'] = all(len(v) == 1
                                               for v in depths.values())
        cell['armsAgreeOnDepth'] = depths['miter'] == depths['union']
        cell['medianOperatorWallSeconds'] = {a: statistics.median(v)
                                             for a, v in walls.items()}
        cell['operatorWallRatio'] = (statistics.median(walls['union'])
                                     / statistics.median(walls['miter']))
        # Paired within a replica: the two arms ran back to back, so a transient
        # is common mode and the ratio of the pair is the cleaner statistic.
        pairs = []
        for index in range(replicas):
            m = [r['operatorWallSeconds'] for r in cell['runs']
                 if r['arm'] == 'miter' and r['replica'] == index]
            u = [r['operatorWallSeconds'] for r in cell['runs']
                 if r['arm'] == 'union' and r['replica'] == index]
            if m and u and m[0]:
                pairs.append(u[0] / m[0])
        cell['pairedRatios'] = pairs
        cell['medianPairedRatio'] = statistics.median(pairs) if pairs else None
        result['cells'].append(cell)
        json.dump(result, open(out_path, 'w'), indent=1)
    ratios = [c['medianPairedRatio'] for c in result['cells']
              if c['medianPairedRatio'] is not None]
    result['summary'] = {
        'cells': len(result['cells']),
        'medianPairedOperatorWallRatio': statistics.median(ratios)
        if ratios else None,
        'range': [min(ratios), max(ratios)] if ratios else None,
        'allDepthsIdenticalWithinArm': all(c['depthsIdenticalWithinArm']
                                           for c in result['cells']),
        'allArmsAgreeOnDepth': all(c['armsAgreeOnDepth']
                                   for c in result['cells']),
    }
    json.dump(result, open(out_path, 'w'), indent=1)
    print(json.dumps(result['summary'], indent=1))


if __name__ == '__main__':
    main()
