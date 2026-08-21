#!/usr/bin/env python3
"""Every markdown table this round's README quotes, generated from the JSON.

    python3 tables.py docs/experiments/replan/evidence

`docs/experiments/calibrated-plan/drivers/summarize.py` did this for that
round's three tables; this round has more of them and one extra reason to
insist on it. The box was **not quiet** while these batteries ran (see §12), so
every table below carries the load it was measured under, and a load column
typed by hand is a load column that gets typed once and then copied.
"""
import json
import os
import statistics
import sys


def load(path):
    with open(path) as handle:
        return json.load(handle)


def maybe(path):
    return load(path) if os.path.exists(path) else None


def fmt(value, places=3):
    return 'n/a' if value is None else f'{value:.{places}f}'


def box(doc):
    b = (doc or {}).get('boxLoad') or {}
    if not b.get('n'):
        return 'load not recorded'
    return (f"load1 min {fmt(b['min'], 2)} / median {fmt(b['median'], 2)} / "
            f"max {fmt(b['max'], 2)} over {b['n']} runs")


# ---------------------------------------------------------------- the gates

def gates(outdir):
    print('## gates\n')
    print('| gate | reproduced scalar | ship | base | whole-document digest, '
          'ship / base |')
    print('|---|---|---|---|---|')
    ship = maybe(f'{outdir}/gates-ship.json')
    base = maybe(f'{outdir}/gates-base.json')
    if not ship:
        print('| (not run) | | | | | |')
        return
    for tag in sorted(ship['gates']):
        s = ship['gates'][tag]
        b = (base or {}).get('gates', {}).get(tag, {})
        # g1 reports a depth *list*; g2-g4 report the pinned scalar as `raw`.
        scalar = s.get('raw')
        if scalar is None:
            scalar = (s.get('depths') or [None])[0]
        sd, bd = (s.get('docDigest') or '')[:16], (b.get('docDigest') or '')[:16]
        same = '`' + sd + '`' + ('' if sd == bd else f' / **`{bd}`**')
        print(f"| {tag} | {scalar} | "
              f"{'hit' if s.get('hit') else '**MISS**'} | "
              f"{'hit' if b.get('hit') else '**MISS**'} | {same} |")
    print(f"\nALL_PASS ship={ship['ALL_PASS']} "
          f"base={(base or {}).get('ALL_PASS')}\n")


# ------------------------------------------------------- refactor / concat

def equiv(outdir, name, title):
    doc = maybe(f'{outdir}/{name}')
    print(f'### {title}\n')
    if not doc:
        print('(not run)\n')
        return
    print(f"`{doc['binaryA']}` vs `{doc['binaryB']}`, "
          f"work={doc['work']}, extraB=`{doc['extraB'] or '(none)'}`\n")
    print('| cell | document | step digest | m34 slices | batches | depth |')
    print('|---|---|---|---:|---:|---:|')
    for row in doc['rows']:
        digest = ('n/a' if not row.get('stepDigestsComparable')
                  else ('equal' if row['stepDigestsEqual'] else '**DIFFERS**'))
        print(f"| {row['tag']} | "
              f"{'equal' if row['documentEqual'] else '**DIFFERS**'} | "
              f"{digest} | {row['m34CallsB']} | {row['totalBatches']} | "
              f"{fmt(row['rawDepthMmB'], 4)} |")
    print(f"\nallEqual={doc['allEqual']} "
          f"allStepDigestsEqual={doc.get('allStepDigestsEqual')} "
          f"totalBatches={doc['totalBatches']} - {box(doc)}\n")


# ------------------------------------------------------------- determinism

def determinism(outdir, name, title):
    doc = maybe(f'{outdir}/{name}')
    print(f'### {title}\n')
    if not doc:
        print('(not run)\n')
        return
    print('| cell | plans agree | tranches agree | document equal | depth |')
    print('|---|---|---|---|---:|')
    for row in doc['rows']:
        print(f"| {row['tag']} | {row['planAgrees']} | "
              f"{row['tranchesAgree']} | {row['documentEqual']} | "
              f"{fmt(row['rawDepthMmA'], 4)} |")
    print(f"\nallEqual={doc['allEqual']} "
          f"allPlansAgree={doc.get('allPlansAgree')} "
          f"allTranchesAgree={doc.get('allTranchesAgree')} - {box(doc)}\n")


# ----------------------------------------------------------- the battery

def battery(outdir, name='battery-10s.json'):
    doc = maybe(f'{outdir}/{name}')
    print('## the twenty-round battery\n')
    if not doc:
        print('(not run)\n')
        return
    print(f"{doc['request']}, target {doc['targetSeconds']} s, "
          f"{doc['rounds']} rounds x {len(doc['seeds'])} seeds - {box(doc)}\n")
    print('| arm | n | wall p50 | wall p95 | wall max | over target |')
    print('|---|---:|---:|---:|---:|---:|')
    for arm, b in doc['byArm'].items():
        print(f"| `{arm}` | {b['n']} | {fmt(b['wallP50'])} s | "
              f"{fmt(b['wallP95'])} s | {fmt(b['wallMax'])} s | "
              f"**{b['overruns']} of {b['n']}** |")
    print()
    print('| arm | seed | distinct plans | tranches | distinct depths | '
          'distinct documents | depth |')
    print('|---|---:|---:|---|---:|---:|---|')
    for arm, b in doc['byArm'].items():
        for seed in doc['seeds']:
            v = b['perSeed'].get(str(seed))
            if not v:
                continue
            depths = v['distinctDepthsMm']
            shown = ' / '.join(f'{x:.4f}' for x in depths)
            units = v.get('distinctFinalUnits') or []
            plans = 'n/a' if units == [None] else str(len(units))
            counts = v.get('distinctTrancheCounts') or []
            print(f"| `{arm}` | {seed} | {plans} | "
                  f"{'/'.join(str(c) for c in counts) or 'n/a'} | "
                  f"{len(depths)} | {len(v['distinctDigests'])} | {shown} |")
    print()
    for arm, b in doc['byArm'].items():
        print(f"- `{arm}`: allSeedsPlanStable={b['allSeedsPlanStable']} "
              f"allSeedsDocumentStable={b['allSeedsDocumentStable']} "
              f"seedMedianOfMedians={fmt(b['seedMedianOfMedians'], 4)}")
    print()


# ------------------------------------------------------------- the anytime

def anytime(outdir, name='anytime.json', title='the anytime table'):
    doc = maybe(f'{outdir}/{name}')
    print(f'## {title}\n')
    if not doc:
        print('(not run)\n')
        return
    print(f"{box(doc)}\n")
    print('| fixture | target | arm | seed medians (mm) | median | wall max | '
          'reproduced | over target | tranches |')
    print('|---|---:|---|---|---:|---:|---:|---:|---|')
    for request in doc['requests']:
        for target in doc['targets']:
            for arm in doc['arms']:
                cell = doc['table'].get(f'{request}|{target}|{arm}')
                if not cell:
                    continue
                per = cell['perSeedDepthMm']
                seeds = ' / '.join(fmt(per[str(s)], 3) for s in doc['seeds']
                                   if str(s) in per)
                counts = cell.get('trancheCounts') or []
                print(f"| {request} | {int(target)//1000} s | `{arm}` | "
                      f"{seeds} | **{fmt(cell['medianDepthMm'], 3)}** | "
                      f"{fmt(cell['wallMaxSeconds'], 2)} s | "
                      f"{cell['reproducedCells']}/{cell['n']} | "
                      f"{cell['overrunCells']}/{cell['n']} | "
                      f"{'/'.join(str(c) for c in counts) or 'n/a'} |")
    print()
    # The price table: every arm against `wall`, per (fixture, budget).
    print('| fixture | target | `plan` | `replan` | `wall` | plan-wall | '
          'replan-wall | **replan-plan** |')
    print('|---|---:|---:|---:|---:|---:|---:|---:|')
    deltas = []
    for request in doc['requests']:
        for target in doc['targets']:
            def med(arm):
                cell = doc['table'].get(f'{request}|{target}|{arm}')
                return cell['medianDepthMm'] if cell else None
            p, r, w = med('plan'), med('replan'), med('wall')
            if p is None or w is None:
                continue
            row = (f"| {request} | {int(target)//1000} s | {fmt(p, 3)} | "
                   f"{fmt(r, 3)} | {fmt(w, 3)} | {fmt(p - w, 3)} | ")
            row += (f"{fmt(r - w, 3)} | **{fmt(r - p, 3)}** |"
                    if r is not None else 'n/a | n/a |')
            print(row)
            if r is not None:
                deltas.append(r - p)
    if deltas:
        print(f"\nmedian `replan` - `plan` over "
              f"{len(deltas)} rows: **{fmt(statistics.median(deltas), 3)} mm**")
    print(f"\nallPlanCellsReproduced={doc.get('allPlanCellsReproduced')} "
          f"allReplanCellsReproduced={doc.get('allReplanCellsReproduced')} "
          f"allWallCellsReproduced={doc.get('allWallCellsReproduced')}\n")


# --------------------------------------------------------- the lever sweeps

def sweep(outdir, name, title, label):
    doc = maybe(f'{outdir}/{name}')
    print(f'### {title}\n')
    if not doc:
        print('(not run)\n')
        return
    print(f"{doc['request']}, {doc['rounds']} rounds x "
          f"{len(doc['seeds'])} seeds - {box(doc)}\n")
    print(f'| target | {label} | n | wall p50 | wall max | worst / target | '
          'over target | depth median | per-seed depth | tranches |')
    print('|---:|---|---:|---:|---:|---:|---:|---:|---|---|')
    for key, cell in doc['cells'].items():
        target, value = key.split('/')
        per = cell['perSeedDepthMedianMm']
        seeds = ' / '.join(fmt(per[str(s)], 3) for s in doc['seeds']
                           if str(s) in per)
        counts = cell.get('trancheCounts') or []
        print(f"| {int(target)//1000} s | `{value}` | {cell['n']} | "
              f"{fmt(cell['wallP50'], 2)} s | {fmt(cell['wallMax'], 2)} s | "
              f"{fmt(cell['worstOverrunRatio'], 3)} | "
              f"**{cell['overruns']} of {cell['n']}** | "
              f"{fmt(cell['depthMedianMm'], 3)} | {seeds} | "
              f"{'/'.join(str(c) for c in counts)} |")
    print()


def main():
    outdir = sys.argv[1]
    gates(outdir)
    print('## the equivalence gates\n')
    equiv(outdir, 'refactor-equivalence.json',
          'the refactor: base binary vs this one')
    for tag, size in (('400k', '400000'), ('100k', '100000'),
                      ('25k', '25000')):
        equiv(outdir, f'concat-{tag}.json',
              f'the concatenation gate, `m34batch={size}`')
    equiv(outdir, 'concat-120M.json',
          'the concatenation gate at a 120 M budget')
    print('## determinism across two processes\n')
    determinism(outdir, 'determinism-work.json', 'work mode')
    determinism(outdir, 'determinism-plan.json', 'plan mode')
    determinism(outdir, 'determinism-replan.json', 'plan mode, re-planning')
    determinism(outdir, 'determinism-replan-planfirst06.json',
                'plan mode, re-planning, `planfirst=0.6`')
    determinism(outdir, 'determinism-replan-stranded.json',
                'plan mode, re-planning, before the stranding fix')
    battery(outdir)
    anytime(outdir)
    anytime(outdir, 'anytime30.json', 'the thirty-second cell')
    print('## the two levers\n')
    sweep(outdir, 'cal-first-tranche.json', 'the first tranche', 'planfirst')
    sweep(outdir, 'cap-30s.json', 'the slice cap at thirty seconds', 'm34cap')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
