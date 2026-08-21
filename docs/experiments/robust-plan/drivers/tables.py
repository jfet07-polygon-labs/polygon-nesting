#!/usr/bin/env python3
"""Every table in this round's README, rendered from the evidence.

    python3 tables.py docs/experiments/robust-plan/evidence

One function per table and no numbers typed by hand anywhere: a README figure
that cannot be regenerated from `evidence/` is a figure this campaign has
already been caught getting wrong once.
"""
import json
import os
import statistics
import sys


def load(root, name):
    path = os.path.join(root, name)
    if not os.path.exists(path):
        return None
    with open(path) as handle:
        return json.load(handle)


def fmt(value, places=3):
    if value is None:
        return '-'
    if isinstance(value, float):
        return f'{value:.{places}f}'
    return str(value)


def box(doc):
    load_block = (doc or {}).get('boxLoad') or {}
    if not load_block.get('n'):
        return ''
    return (f"load1 min {fmt(load_block['min'], 2)} / median "
            f"{fmt(load_block['median'], 2)} / max {fmt(load_block['max'], 2)}"
            f" over {load_block['n']} runs")


def battery(doc, title):
    if not doc:
        return
    print(f'\n### {title}\n')
    print(f"mixed-61, target {doc['targetSeconds']} s, {doc['rounds']} rounds x "
          f"{len(doc['seeds'])} seeds - {box(doc)}\n")
    print('| arm | n | wall p50 | wall p95 | wall max | over target |')
    print('|---|---:|---:|---:|---:|---:|')
    for arm, block in doc['byArm'].items():
        print(f"| `{arm}` | {block['n']} | {fmt(block['wallP50'])} s | "
              f"{fmt(block['wallP95'])} s | {fmt(block['wallMax'])} s | "
              f"**{block['overruns']} of {block['n']}** |")
    print('\n| arm | seed | distinct plans | distinct depths | distinct '
          'documents | modal share | depth | probe source |')
    print('|---|---:|---:|---:|---:|---:|---|---|')
    for arm, block in doc['byArm'].items():
        for seed, cell in block['perSeed'].items():
            depths = ' / '.join(fmt(d, 4) for d in cell['distinctDepthsMm'])
            sources = ' '.join(f'{k}:{v}' for k, v in
                               (cell.get('calibrationSources') or {}).items())
            print(f"| `{arm}` | {seed} | {len(cell['distinctFinalUnits'])} | "
                  f"{len(cell['distinctDepthsMm'])} | "
                  f"{len(cell['distinctDigests'])} | "
                  f"{fmt(cell['modalDepthShare'], 2)} | {depths} | {sources} |")
    print()
    for arm, block in doc['byArm'].items():
        print(f"- `{arm}`: allSeedsPlanStable={block['allSeedsPlanStable']} "
              f"allSeedsDocumentStable={block['allSeedsDocumentStable']} "
              f"seedMedianOfMedians={fmt(block['seedMedianOfMedians'], 4)}")
    # The probe spread the arms had to survive, from the raw rows: this is the
    # size of the disturbance, and without it a determinism result is not
    # interpretable.
    live = [r['probeSeconds'] for r in doc['rows']
            if r.get('probeSeconds') is not None]
    if live:
        print(f"\nlive probe wall over all {len(live)} runs: "
              f"min {fmt(min(live), 4)} / median "
              f"{fmt(statistics.median(live), 4)} / max {fmt(max(live), 4)} s "
              f"- a spread of {fmt(max(live) / min(live), 3)}x")


def density(doc, title):
    if not doc:
        return
    print(f'\n### {title}\n')
    print(f"{doc['fixture']}, {doc['mode']}={doc['value']}, {doc['rounds']} "
          f"rounds x {len(doc['seeds'])} seeds - {box(doc)}\n")
    print('| step_grid | confirm_every | depth (seed median of medians) | '
          'delta vs 1/4 | first-slice drop | steps | confirms acc/att | '
          'slice units | mm per 1k units | slices | wall p50 |')
    print('|---:|---:|---:|---:|---:|---:|---|---:|---:|---:|---:|')
    for key, block in doc['byCell'].items():
        print(f"| {block['grid']} | {block['confirm']} | "
              f"**{fmt(block['seedMedianOfMedians'], 4)}** | "
              f"{fmt(block.get('deltaVsBaselineMm'), 4)} | "
              f"{fmt(block['firstSliceDropMm'], 4)} | "
              f"{fmt(block['firstSliceStepsMedian'], 0)} | "
              f"{fmt(block['confirmAcceptedMedian'], 0)}/"
              f"{fmt(block['confirmAttemptedMedian'], 0)} | "
              f"{fmt(block['firstSliceWorkUnitsMedian'], 0)} | "
              f"{fmt(block['mmPerKiloUnit'], 5)} | "
              f"{fmt(block['slicesMedian'], 1)} | "
              f"{fmt(block['wallP50'])} s |")


def anytime(doc, title):
    if not doc:
        return
    print(f'\n### {title}\n')
    print(box(doc) + '\n')
    print('| fixture | target | arm | seed depths (mm) | median | wall max | '
          'reproduced | over target |')
    print('|---|---:|---|---|---:|---:|---:|---:|')
    for key, row in doc.get('table', {}).items():
        request, target, arm = key.split('|')
        depths = ' / '.join(fmt(row['perSeedDepthMm'][s], 3)
                            for s in sorted(row['perSeedDepthMm'], key=int))
        print(f"| {request} | {int(target) // 1000} s | "
              f"`{arm}` | {depths} | **{fmt(row['medianDepthMm'], 3)}** | "
              f"{fmt(row['wallMaxSeconds'], 2)} s | {row['reproducedCells']}/"
              f"{row['n']} | {row['overrunCells']}/{row['n']} |")
    # The paired difference every arm against the first one named, which is the
    # only form of this table that is a comparison rather than a list.
    arms = doc.get('arms') or []
    if len(arms) > 1:
        print(f"\n| fixture | target | "
              + ' | '.join(f'`{a}`' for a in arms)
              + ' | ' + ' | '.join(f'{a}-{arms[0]}' for a in arms[1:]) + ' |')
        print('|---|---:|' + '---:|' * (2 * len(arms) - 1))
        for request in doc['requests']:
            for target in doc['targets']:
                cells = {a: doc['table'].get(f'{request}|{target}|{a}')
                         for a in arms}
                if not cells.get(arms[0]):
                    continue
                base = cells[arms[0]]['medianDepthMm']
                values = [fmt((cells[a] or {}).get('medianDepthMm'), 3)
                          for a in arms]
                deltas = [fmt((cells[a] or {}).get('medianDepthMm', base)
                              - base, 3) if cells.get(a) else '-'
                          for a in arms[1:]]
                print(f"| {request} | {int(target) // 1000} s | "
                      + ' | '.join(values) + ' | ' + ' | '.join(deltas) + ' |')


def calpass(doc):
    if not doc:
        return
    print('\n### The calibration pass\n')
    print(f"{doc['rounds']} rounds x {len(doc['fixtures'])} fixtures x "
          f"{len(doc['seeds'])} seeds - {box(doc)}, "
          f"convergedOnLastRound={doc['convergedOnLastRound']}\n")
    print('| probeWorkUnits (key) | live entry (s) | probe entry (s) | '
          'probe / live |')
    print('|---:|---:|---:|---:|')
    for key in sorted(doc['live'], key=int):
        live = doc['live'][key]
        probe = doc['probe'].get(key)
        ratio = doc['probeOverLive'].get(key)
        print(f"| {key} | {fmt(live, 4)} | {fmt(probe, 4)} | "
              f"{fmt(ratio, 3)} |")


def gates(ship, base):
    if not ship:
        return
    print('\n### The four pinned gates\n')
    print('| gate | reproduced | ship digest | base digest | equal |')
    print('|---|---|---|---|---|')
    for name, block in ship['gates'].items():
        theirs = ((base or {}).get('gates') or {}).get(name) or {}
        print(f"| {name} | {block.get('hit')} | {block['docDigest'][:16]} | "
              f"{(theirs.get('docDigest') or '')[:16]} | "
              f"{block['docDigest'] == theirs.get('docDigest')} |")
    print(f"\nALL_PASS ship={ship.get('ALL_PASS')} "
          f"base={(base or {}).get('ALL_PASS')}")


def determinism(doc, title):
    if not doc:
        return
    print(f'\n### {title}\n')
    print(f"`{doc['budgetKey']}={doc['value']}`"
          + (f" `{doc['extra']}`" if doc.get('extra') else '')
          + f" - {box(doc)}\n")
    print('| cell | plans agree | tranches agree | document equal | depth |')
    print('|---|---|---|---|---:|')
    for row in doc['rows']:
        print(f"| {row['tag']} | {row.get('planAgrees')} | "
              f"{row.get('tranchesAgree')} | {row.get('documentEqual')} | "
              f"{fmt(row.get('rawDepthMmA'), 4)} |")
    print(f"\nallEqual={doc.get('allEqual')} "
          f"allPlansAgree={doc.get('allPlansAgree')} "
          f"allTranchesAgree={doc.get('allTranchesAgree')}")


def main():
    root = sys.argv[1]
    calpass(load(root, 'calpass.json'))
    battery(load(root, 'battery-loaded.json'),
            'The loaded battery, under a competing load this round owns')
    battery(load(root, 'battery-quiet.json'),
            'The unstressed battery - the box as this round found it')
    battery(load(root, 'battery-head.json'),
            'The headroom battery - the dial, under the same load')
    density(load(root, 'density-plancal.json'),
            'The confirmation-density sweep, plan mode, on a pinned plan')
    density(load(root, 'density-plan.json'),
            'The same sweep without the calibration, as a control')
    density(load(root, 'density-work.json'),
            "The confirmation-density sweep, Grok's equal-work gate")
    anytime(load(root, 'anytime.json'), 'The anytime table')
    anytime(load(root, 'anytime30.json'), 'The thirty-second cell')
    gates(load(root, 'gates-ship.json'), load(root, 'gates-base.json'))
    for name, title in (('determinism-work.json', 'work mode'),
                        ('determinism-plan.json', 'plan mode'),
                        ('determinism-callive.json',
                         'plan mode, persisted calibration'),
                        ('determinism-plan-loaded.json',
                         'plan mode, under the controlled load'),
                        ('determinism-callive-loaded.json',
                         'plan mode, persisted calibration, under load')):
        determinism(load(root, name), f'Determinism: {title}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
