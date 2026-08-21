#!/usr/bin/env python3
"""Every table in the README, regenerated from the JSON that produced it.

A table typed by hand from a JSON file is a table that can disagree with it.

    python3 summarize.py <section> PATH [PATH ...]

Sections: rates, profile, equiv, determinism, race, plan, countertax, ledger.
"""
import json
import statistics
import sys


def load(path):
    with open(path) as handle:
        return json.load(handle)


def rates(paths):
    document = load(paths[0])
    print('## corpus rates - every operator call already in the repository')
    print(f"corpus: {document['corpusCalls']} calls from "
          f"{', '.join(document['corpusRoots'])}")
    print(f"| operator | calls | wall (s) | shipped units | units/s |")
    print('|---|---:|---:|---:|---:|')
    for row in document['corpusRates']:
        print(f"| {row['operator']} | {row['calls']} | "
              f"{row['totalSeconds']:.2f} | {row['totalUnits']:,} | "
              f"{row['pooledUnitsPerSecond']:,.1f} |")
    print()
    print('## this session, `cur2=2`')
    print(f"| operator | calls | wall (s) | shipped units | units/s |")
    print('|---|---:|---:|---:|---:|')
    for row in document['sessionRates']:
        rate = row['pooledUnitsPerSecond']
        print(f"| {row['operator']} | {row['calls']} | "
              f"{row['totalSeconds']:.3f} | {row['totalUnits']:,} | "
              f"{rate:,.1f} |" if rate else
              f"| {row['operator']} | {row['calls']} | "
              f"{row['totalSeconds']:.3f} | {row['totalUnits']:,} | - |")
    print()
    print('## per-class count vectors, summed over the observed calls')
    by = {}
    for run in document['observed']:
        for call in run['calls']:
            entry = by.setdefault(call['operator'],
                                  {'seconds': 0.0, 'global': 0, 'counts': {}})
            entry['seconds'] += call['elapsedSeconds']
            entry['global'] += call['globalUnits']
            for key, value in call['counts'].items():
                entry['counts'][key] = entry['counts'].get(key, 0) + value
    for operator in sorted(by):
        entry = by[operator]
        nonzero = {k: v for k, v in entry['counts'].items() if v}
        print(f"* **{operator}** - {entry['seconds']:.3f} s, "
              f"{entry['global']:,} shipped units: "
              + (', '.join(f'{k} {v:,}' for k, v in nonzero.items())
                 if nonzero else '**every count zero**'))


def profile(paths):
    document = load(paths[0])
    print(f"reference rate {document['referenceRate']:,} units/s, "
          f"scale {document['scale']}")
    print('| class | shipped units/s | x reference | verdict | fitted count | '
          'scaled weight |')
    print('|---|---:|---:|---|---|---:|')
    for operator, entry in document['classes'].items():
        rate = entry['shippedUnitsPerSecond']
        if rate is None:
            print(f"| {operator} | - | - | too short to price | - | - |")
            continue
        fit = entry.get('fit')
        print(f"| {operator} | {rate:,.0f} | "
              f"{entry['shippedMispricing']:.4g} | "
              f"{'**under-priced**' if entry['underpriced'] else 'comparable'} "
              f"| {fit['count'] if fit else '-'} | "
              f"{fit['scaledWeight']:,} |" if fit else
              f"| {operator} | {rate:,.0f} | "
              f"{entry['shippedMispricing']:.4g} | comparable | - | - |")
    print()
    for operator, entry in document['classes'].items():
        if not entry.get('fit'):
            continue
        print(f"### {operator}: candidates, ranked by residual")
        print('| count | residual RMS | worst under | worst over | '
              'scaled weight |')
        print('|---|---:|---:|---:|---:|')
        for row in entry['candidates']:
            print(f"| {row['count']} | {row['residualRms']:.3f} | "
                  f"{row['worstUndercharge']:.3f} | "
                  f"{row['worstOvercharge']:.3f} | {row['scaledWeight']:,} |")
        print()
        print(f"### {operator}: residuals of the chosen weight")
        print('| request | wall (s) | shipped | target | fitted | ratio |')
        print('|---|---:|---:|---:|---:|---:|')
        for row in entry['residuals']:
            print(f"| {row['request']} | {row['seconds']:.3f} | "
                  f"{row['globalUnits']:,} | {row['targetUnits']:,.0f} | "
                  f"{row['fittedUnits']:,} | {row['ratio']:.3f} |")


def equiv(paths):
    document = load(paths[0])
    print(f"base `{document['baseSha256'][:16]}` vs ship "
          f"`{document['shipSha256'][:16]}` at work={document['workUnits']:,}")
    print('| cell | base == ship | observe is pure | leaves | differing | '
          'clock-only leaves | depth |')
    print('|---|---|---|---:|---:|---:|---:|')
    for row in document['rows']:
        diff = row['baseShipLeafDiff']
        print(f"| {row['request']} s{row['seed']} | "
              f"{'**yes**' if row['baseEqualsShip'] else 'NO'} | "
              f"{'**yes**' if row['observeIsPureObserver'] else 'NO'} | "
              f"{diff['leaves']:,} | {diff['differing']} | "
              f"{diff['differingBeforeWallStrip']} | {row['shipDepthMm']} |")
    print()
    print(json.dumps(document['summary'], indent=1))


def determinism(paths):
    for path in paths:
        document = load(path)
        print(f"### `{document['mode']}={document['value']}` "
              f"`{document['extra'] or '(no extra)'}`")
        print('| cell | plans agree | document equal | depth | charged extra |')
        print('|---|---|---|---:|---:|')
        for row in document['rows']:
            extra = row['chargedExtraUnits'][0]
            print(f"| {row['request']}-s{row['seed']} | "
                  f"{row['plansAgree']} | {row['equal']} | "
                  f"{row['depths'][0]} | "
                  f"{extra:,} |" if extra is not None else
                  f"| {row['request']}-s{row['seed']} | {row['plansAgree']} | "
                  f"{row['equal']} | {row['depths'][0]} | - |")
        loads = [entry['before'] for entry in document['boxLoad']
                 if entry['before'] is not None]
        print(f"\nallEqual={document['allEqual']} "
              f"{document['equalCells']}/{document['cells']}"
              + (f" - load1 min {min(loads):.2f} / median "
                 f"{statistics.median(loads):.2f} / max {max(loads):.2f} "
                 f"over {len(loads)} runs" if loads else ''))
        print()


def race(paths):
    document = load(paths[0])
    print(f"`plan={document['targetMillis']}`, race `{document['raceSpec']}`, "
          f"binary `{document['binarySha256'][:16]}`")
    print('| cell | plan units off/on/on2 | off | on | on2 | on-off | '
          'on2-off | equal on/on2 | race s on/on2 | draw s on/on2 | '
          'draw charged on/on2 |')
    print('|---|---|---:|---:|---:|---:|---:|---|---|---|---|')
    for row in document['rows']:
        off, on, on2 = row['off'], row['on'], row['on2']
        print(f"| {row['request']} s{row['seed']} | "
              f"{off['planUnits']:,} / {on['planUnits']:,} / "
              f"{on2['planUnits']:,} | "
              f"{off['depthMm']:.4f} | {on['depthMm']:.4f} | "
              f"{on2['depthMm']:.4f} | "
              f"{row.get('deltaOnMm', float('nan')):+.4f} | "
              f"{row.get('deltaOn2Mm', float('nan')):+.4f} | "
              f"{row['equalWorkOn']}/{row['equalWorkOn2']} | "
              f"{(on.get('race') or {}).get('seconds', 0):.2f} / "
              f"{(on2.get('race') or {}).get('seconds', 0):.2f} | "
              f"{on['draws']['seconds']:.2f} / "
              f"{on2['draws']['seconds']:.2f} | "
              f"{on['draws']['chargedUnits']:,} / "
              f"{on2['draws']['chargedUnits']:,} |")
    print()
    print(json.dumps(document['summary'], indent=1))


def plan(paths):
    document = load(paths[0])
    print(f"`plan={document['targetMillis']}`, "
          f"{document['rounds']} rounds x {len(document['seeds'])} seeds")
    print('| cell | round | plan units off/on | off | on | delta | equal | '
          'wall off/on | charged extra |')
    print('|---|---:|---|---:|---:|---:|---|---|---:|')
    for row in document['rows']:
        off, on = row['off'], row['on']
        print(f"| {row['request']} s{row['seed']} | {row['round']} | "
              f"{off['planUnits']:,} / {on['planUnits']:,} | "
              f"{off['depthMm']:.4f} | {on['depthMm']:.4f} | "
              f"{row.get('deltaMm', float('nan')):+.4f} | "
              f"{row['equalWork']} | "
              f"{off['processSeconds']:.2f} / {on['processSeconds']:.2f} | "
              f"{(on.get('workCurrency') or {}).get('chargedExtraUnits', 0):,} |")
    print()
    print(json.dumps(document['summary'], indent=1))


def countertax(paths):
    document = load(paths[0])
    print(f"`wall={document['targetMillis']}` on {document['request']}, "
          f"{document['rounds']} rounds x {len(document['seeds'])} seeds")
    print('| seed | counters off | counters on | counters on + `cur2=1` | '
          'counter tax | currency tax |')
    print('|---|---:|---:|---:|---:|---:|')
    for row in document['perSeed']:
        print(f"| {row['seed']} | {row['countersOff']:.4f} | "
              f"{row['countersOn']:.4f} | {row['countersOnCur2']:.4f} | "
              f"{row['counterTaxMm']:+.4f} | **{row['currencyTaxMm']:+.4f}** |")
    print()
    print(json.dumps(document['summary'], indent=1))


SECTIONS = {
    'rates': rates, 'profile': profile, 'equiv': equiv,
    'determinism': determinism, 'race': race, 'plan': plan,
    'countertax': countertax,
}

if __name__ == '__main__':
    SECTIONS[sys.argv[1]](sys.argv[2:])
