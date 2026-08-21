#!/usr/bin/env python3
"""Regenerates this round's tables straight out of the evidence JSON.

    summarize.py {race|witness|attribution|gates|determinism} PATH [PATH...]

A table typed by hand from a JSON file is a table that can disagree with it.
Every table in `README.md` comes out of here.
"""
import json
import statistics
import sys


def race(path):
    d = json.load(open(path))
    print(f'### {path}')
    print(json.dumps(d['summary'], indent=1))
    print()
    print('| cell | plan units, off / on | off | on | delta | equal work | '
          'race work | race s | wall off / on | actions off / on | winner |')
    print('|---|---|---:|---:|---:|---|---:|---:|---|---|---|')
    for row in d['rows']:
        r = row['on'].get('race') or {}
        off, on = row['off'], row['on']
        plans = (f'{off["planUnits"]:,} / same' if row['equalWork']
                 else f'{off["planUnits"]:,} / {on["planUnits"]:,}')
        print(f'| {row["request"]} s{row["seed"]} | {plans} | '
              f'{off["depthMm"]:.4f} | {on["depthMm"]:.4f} | '
              f'{row["deltaMm"]:+.4f} | {"yes" if row["equalWork"] else "**no**"} | '
              f'{(r.get("workUnits") or 0):,} | {(r.get("seconds") or 0):.2f} | '
              f'{off["processSeconds"]:.2f} / {on["processSeconds"]:.2f} | '
              f'{off["scheduleIterations"]} / {on["scheduleIterations"]} | '
              f'slot {r.get("winnerSlot")} |')
    print()
    print('| cell | slot | kind | depth | yield | stability | infeasibility | '
          'confirmations | rank sum | eliminated |')
    print('|---|---:|---|---:|---:|---:|---:|---:|---:|---|')
    stabilities = []
    for row in d['rows']:
        for arm in ((row['on'].get('race') or {}).get('arms') or []):
            stabilities.append(arm['stability'])
            infeasible = arm['infeasibility']
            shown = 'inf' if infeasible is None or infeasible != infeasible \
                or infeasible == float('inf') else f'{infeasible:.3f}'
            print(f'| {row["request"]} s{row["seed"]} | {arm["slot"]} | '
                  f'{arm["kind"]} | {arm["depthMm"]:.4f} | '
                  f'{arm["yieldMm"]:.4f} | {arm["stability"]:.3f} | {shown} | '
                  f'{arm["batchConfirmations"]} | {arm["rankSum"]} | '
                  f'{arm["eliminatedRound"]} |')
    if stabilities:
        distinct = sorted(set(stabilities))
        print()
        print(f'stability: {len(stabilities)} arms, {len(distinct)} distinct '
              f'value(s): {distinct}')


def witness(path):
    d = json.load(open(path))
    print(f'### {path}')
    print(json.dumps(d['summary'], indent=1))
    print()
    print('| parent | off | publish | adopt | adopt − publish | '
          'witness accepted | adoptions | descendant |')
    print('|---|---:|---:|---:|---:|---:|---:|---|')
    same = 0
    for cell in d['cells']:
        pub = cell['arms']['publish'].get('slice') or {}
        ad = cell['arms']['adopt'].get('slice') or {}
        depths = cell['depths']
        if abs(depths['publish'] - depths['off']) < 1e-12:
            same += 1
        print(f'| s{cell["seed"]} {cell["parentRawDepthMm"]:.4f} | '
              f'{depths["off"]:.4f} | {depths["publish"]:.4f} | '
              f'{depths["adopt"]:.4f} | {cell["adoptMinusPublishMm"]:+.4f} | '
              f'{pub.get("se2WitnessAccepted")} | '
              f'{ad.get("se2WitnessAdoptions")} | '
              f'{"**yes**" if cell["descendantPublication"] else ""} |')
    print()
    print(f'publish == off on **{same} of {len(d["cells"])}** parents')
    deltas = [c['adoptMinusPublishMm'] for c in d['cells']]
    print(f'mean adopt − publish: {statistics.fmean(deltas):+.4f} mm')
    print()
    print('| parent | episodes, publish | episodes, adopt |')
    print('|---|---:|---:|')
    for cell in d['cells']:
        pub = cell['arms']['publish'].get('slice') or {}
        ad = cell['arms']['adopt'].get('slice') or {}
        if (ad.get('se2WitnessAdoptions') or 0) == 0:
            continue
        print(f'| s{cell["seed"]} | {pub.get("sparseRotationEpisodes"):,} | '
              f'{ad.get("sparseRotationEpisodes"):,} |')


def attribution(path):
    d = json.load(open(path))
    print(f'### {path}')
    print('| arm | rungs proposed | `rotationAcceptedMoves` | '
          '`sparseRotationCommittedMoves` |')
    print('|---|---:|---:|---:|')
    for label, row in d['summary'].items():
        print(f'| {label} | {row["rotationRungsProposed"]:,} | '
              f'{row["rotationAcceptedMoves"]:,} | '
              f'{row["sparseRotationCommittedMoves"]:,} |')
    b = d['summary']['designB']
    print()
    print('| | count | as a fraction |')
    print('|---|---:|---|')
    print(f'| episodes | {b["sparseRotationEpisodes"]:,} | |')
    print(f'| sparse rungs proposed | {b["sparseRotationRungsProposed"]:,} | '
          f'{b["sparseRotationRungsProposed"] / b["sparseRotationEpisodes"]:.1f}'
          ' per episode |')
    print(f'| rung winners | {b["sparseRotationRungWinners"]:,} | '
          f'{100 * b["sparseRotationRungWinners"] / b["sparseRotationRungsProposed"]:.2f}%'
          ' of proposals |')
    print(f'| committed moves | {b["sparseRotationCommittedMoves"]:,} | '
          f'{100 * b["sparseRotationCommittedMoves"] / b["sparseRotationRungWinners"]:.2f}%'
          ' of winners |')
    print(f'| committed episodes | {b["sparseRotationCommittedEpisodes"]:,} | '
          f'{100 * b["committedEpisodeFraction"]:.2f}% of episodes |')
    print(f'| `rotationAcceptedMoves` | {b["rotationAcceptedMoves"]:,} | '
          f'{b["acceptedOverCommitted"]:.3f}x the committed moves |')


def gates(*paths):
    docs = {json.load(open(p))['label']: json.load(open(p)) for p in paths}
    labels = list(docs)
    print('| gate | ' + ' | '.join(labels) + ' | digests agree |')
    print('|---|' + '---|' * (len(labels) + 1))
    for gate in ('g1', 'g2', 'g3', 'g4'):
        cells, digests = [], set()
        for label in labels:
            row = docs[label]['gates'][gate]
            cells.append(f'{"hit" if row["hit"] else "MISS"}, '
                         f'{row["wallSeconds"]:.2f} s')
            digests.add(row['docDigest'])
        agree = ('**identical** `' + list(digests)[0][:8] + '...`'
                 if len(digests) == 1 else '**DIFFER**')
        print(f'| {gate} | ' + ' | '.join(cells) + f' | {agree} |')
    for label in labels:
        print(f'{label}: ALL_PASS={docs[label]["ALL_PASS"]}')


def determinism(*paths):
    print('| file | cells | equal | plan agrees | misses |')
    print('|---|---:|---:|---:|---|')
    for path in paths:
        d = json.load(open(path))
        rows = d['rows']
        misses = [r['tag'] for r in rows if not r['equal']]
        print(f'| {path.rsplit("/", 1)[-1]} | {len(rows)} | '
              f'{sum(1 for r in rows if r["equal"])} | '
              f'{sum(1 for r in rows if r["planAgrees"])} | '
              f'{", ".join(misses) or "-"} |')


def price(path):
    """The race phase's own operator log, as wall share against work share.

    Shares and a units-per-second rate rather than seconds, because those are
    the two forms that survive a contended box: a uniform slowdown moves every
    row's seconds and leaves both columns where they were.
    """
    d = json.load(open(path))
    calls = [c for c in d['portfolio']['operatorCalls'] if c['phase'] == 'race']
    if not calls:
        print(f'{path}: no race phase')
        return
    wall = sum(c['elapsedSeconds'] for c in calls)
    work = sum(c['workUnits'] for c in calls)
    print(f'### {path}')
    print('| call | wall | % of race wall | work units | % of race work | units/s |')
    print('|---|---:|---:|---:|---:|---:|')
    for c in calls:
        rate = c['workUnits'] / c['elapsedSeconds'] if c['elapsedSeconds'] else 0
        print(f'| `{c.get("action")}` | {c["elapsedSeconds"]:.3f} s | '
              f'{100 * c["elapsedSeconds"] / wall:.1f}% | {c["workUnits"]:,} | '
              f'{100 * c["workUnits"] / work:.4f}% | {rate:,.0f} |')
    for operator in ('mode20', 'mode22', 'mode34'):
        rows = [c for c in calls if c['operator'] == operator]
        if not rows:
            continue
        seconds = sum(c['elapsedSeconds'] for c in rows)
        units = sum(c['workUnits'] for c in rows)
        print(f'{operator}: {seconds:.3f} s ({100 * seconds / wall:.1f}% of race '
              f'wall) for {units:,} units ({100 * units / work:.4f}% of race '
              f'work) = {units / seconds:,.1f} units/s')


MODES = {'race': race, 'witness': witness, 'attribution': attribution,
         'gates': gates, 'determinism': determinism, 'price': price}
MODES[sys.argv[1]](*sys.argv[2:])
