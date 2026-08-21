#!/usr/bin/env python3
"""Turns the evidence JSON into the markdown tables the README quotes.

    python3 summarize.py battery PATH/planbattery.json
    python3 summarize.py anytime PATH/anytime.json
    python3 summarize.py determinism PATH/determinism.json

Written because a table typed by hand from a JSON file is a table that can
disagree with it. Every number in this round's README §8, §10 and §12 comes out
of here.
"""
import json
import statistics
import sys


def battery(path):
    d = json.load(open(path))
    seeds = d['seeds']
    target = d['targetSeconds']
    print(f"target {target} s, {d['rounds']} rounds x {len(seeds)} seeds, "
          f"request {d['request']}\n")
    print('| arm | n | wall p50 | wall p95 | wall max | runs over target |')
    print('|---|---:|---:|---:|---:|---:|')
    for arm, b in d['byArm'].items():
        print(f"| `{arm}` | {b['n']} | {b['wallP50']:.3f} s | "
              f"{b['wallP95']:.3f} s | {b['wallMax']:.3f} s | "
              f"**{b['overruns']} of {b['n']}** |")
    print()
    print('| arm | seed | distinct plans | distinct depths | distinct '
          'documents | depth |')
    print('|---|---:|---:|---:|---:|---:|')
    for arm, b in d['byArm'].items():
        for seed in seeds:
            v = b['perSeed'].get(str(seed))
            if not v:
                continue
            depths = v['distinctDepthsMm']
            shown = (f"**{depths[0]:.4f}**" if len(depths) == 1
                     else ' / '.join(f'{x:.4f}' for x in depths))
            plans = ('n/a' if v['distinctPlanUnits'] == [None]
                     else str(len(v['distinctPlanUnits'])))
            print(f"| `{arm}` | {seed} | {plans} | {len(depths)} | "
                  f"{len(v['distinctDigests'])} | {shown} |")
    print()
    for arm, b in d['byArm'].items():
        print(f"{arm}: allSeedsPlanStable={b['allSeedsPlanStable']} "
              f"allSeedsDocumentStable={b['allSeedsDocumentStable']} "
              f"seedMedianOfMedians={b['seedMedianOfMedians']:.4f}")
    print()
    # The per-seed wall detail, which the pooled percentiles hide.
    for arm, b in d['byArm'].items():
        for seed in seeds:
            v = b['perSeed'].get(str(seed))
            if v:
                print(f"{arm} s{seed}: wall p50={v['wallP50']:.3f} "
                      f"p95={v['wallP95']:.3f} max={v['wallMax']:.3f} "
                      f"min={v['wallMin']:.3f} over={v['overruns']}")
    print()
    # Distinct depths in full, for the arms that split.
    for arm, b in d['byArm'].items():
        for seed in seeds:
            v = b['perSeed'].get(str(seed))
            if v and len(v['distinctDepthsMm']) > 1:
                rows = [r for r in d['rows']
                        if r.get('arm') == arm and r.get('seed') == seed]
                counts = {}
                for r in rows:
                    counts[r['rawDepthMm']] = counts.get(r['rawDepthMm'], 0) + 1
                detail = ', '.join(f'{k} x{v2}'
                                   for k, v2 in sorted(counts.items()))
                print(f"SPLIT {arm} s{seed}: {detail}")


def anytime(path):
    d = json.load(open(path))
    print('| fixture | target | arm | seed medians | median | wall max | '
          'cells reproduced | over target |')
    print('|---|---:|---|---|---:|---:|---:|---:|')
    for request in d['requests']:
        for target in d['targets']:
            for arm in d['arms']:
                block = d['table'].get(f'{request}|{target}|{arm}')
                if not block:
                    continue
                per = ' / '.join(
                    f"{block['perSeedDepthMm'][str(s)]:.3f}"
                    for s in d['seeds']
                    if str(s) in block['perSeedDepthMm'])
                print(f"| {request} | {int(target)//1000} s | `{arm}` | {per} "
                      f"| **{block['medianDepthMm']:.3f}** | "
                      f"{block['wallMaxSeconds']:.2f} s | "
                      f"{block['reproducedCells']}/{block['n']} | "
                      f"{block['overrunCells']}/{block['n']} |")
    print()
    print(f"allPlanCellsReproduced={d['allPlanCellsReproduced']}")
    print(f"allWallCellsReproduced={d['allWallCellsReproduced']}")
    print()
    # The paired plan-minus-wall delta, per (fixture, target), which is the
    # price of reproducibility in millimetres.
    print('| fixture | target | plan median | wall median | plan - wall |')
    print('|---|---:|---:|---:|---:|')
    deltas = []
    for request in d['requests']:
        for target in d['targets']:
            p = d['table'].get(f'{request}|{target}|plan')
            w = d['table'].get(f'{request}|{target}|wall')
            if not p or not w:
                continue
            delta = p['medianDepthMm'] - w['medianDepthMm']
            deltas.append(delta)
            print(f"| {request} | {int(target)//1000} s | "
                  f"{p['medianDepthMm']:.3f} | {w['medianDepthMm']:.3f} | "
                  f"**{delta:+.3f}** |")
    if deltas:
        print(f"\nmedian delta = {statistics.median(deltas):+.3f} mm, "
              f"worst = {max(deltas):+.3f} mm, best = {min(deltas):+.3f} mm")


def determinism(path):
    d = json.load(open(path))
    print(f"budget {d['budgetKey']}={d['value']}  allEqual={d['allEqual']}")
    print('| cell | plans agree | document equal | depth |')
    print('|---|---|---|---:|')
    for row in d['rows']:
        print(f"| {row['tag']} | {row['planAgrees']} | "
              f"{row['documentEqual']} | {row['rawDepthMmA']} |")


if __name__ == '__main__':
    {'battery': battery, 'anytime': anytime,
     'determinism': determinism}[sys.argv[1]](sys.argv[2])
