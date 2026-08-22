#!/usr/bin/env python3
"""Every markdown table in this round's README, generated from the evidence.

    tables.py EVIDENCEDIR

Written rather than typed for the reason the m26 audition wrote its own: a table
transcribed by hand is a second, unversioned copy of the measurement, and the
campaign has already had one round where the README and the JSON disagreed.
"""
import json
import os
import statistics
import sys


def load(path):
    return json.load(open(path)) if os.path.exists(path) else None


def matched_table(matched, verdict):
    print('### Per-seed, at equal work\n')
    print('| seed | parent | W | miter | union | diff | miter opwall | union opwall'
          ' | ratio | confirmations | same fingerprint |')
    print('|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|:--:|')
    for work in matched['works']:
        block = verdict['equalWork'].get(str(work), {}).get('rows') or []
        for row in block:
            print(f"| {row['seed']} | {row['parentMm']:.3f} | {work} | "
                  f"{row['controlDepthMm']:.4f} | {row['armDepthMm']:.4f} | "
                  f"{row['armMinusControlMm']:+.4f} | "
                  f"{row['controlOperatorWallSeconds']:.2f} s | "
                  f"{row['armOperatorWallSeconds']:.2f} s | "
                  f"{row['armOperatorWallSeconds'] / row['controlOperatorWallSeconds']:.3f} | "
                  f"{row['controlConfirmations']} | "
                  f"{'yes' if row['sameFingerprint'] else 'NO'} |")


def clause_table(verdict):
    print('\n### The pre-committed rule, per budget\n')
    print('| W | equal-work wins | equal-work median | equal-wall wins '
          '(interpolated) | equal-wall median | equal-wall wins (measured only)'
          ' | equal-wall median (measured only) |')
    print('|---:|---:|---:|---:|---:|---:|---:|')
    for work, block in verdict['preCommittedRule']['clausesPerBudget'].items():
        print(f"| {work} | {block['equalWorkWins']}/12 | "
              f"{block['equalWorkMedianImprovementMm']:+.4f} mm | "
              f"{block['equalWallWins']}/12 | "
              f"{block['equalWallMedianImprovementMm']:+.4f} mm | "
              f"{block['equalWallWinsNoInterpolation']}/12 | "
              f"{block['equalWallMedianImprovementNoInterpolationMm']:+.4f} mm |")


def overhead_table(verdict, wallratio):
    over = verdict['perConfirmationOverhead']
    print('\n### Cost\n')
    print('| quantity | value |')
    print('|---|---:|')
    print(f"| per-confirmation cost, arm / control, median over "
          f"{over['cells']} cells | **{over['median']:.4f}x** |")
    print(f"| per-confirmation cost, worst cell | {over['max']:.4f}x |")
    print(f"| per-confirmation cost, best cell | {over['min']:.4f}x |")
    if wallratio:
        summary = wallratio['summary']
        print(f"| whole-slice operator wall, arm / control, median of "
              f"{summary['cells']} paired-replica cells | "
              f"**{summary['medianPairedOperatorWallRatio']:.4f}x** |")
        print(f"| the same, range | "
              f"[{summary['range'][0]:.4f}, {summary['range'][1]:.4f}] |")


def crot_table(flip):
    print('\n### The crot tax, under each authority\n')
    print('| budget | crot tax under miter | crot tax under round | flipped? '
          '| round-armed off-2.5-degree poses |')
    print('|---|---:|---:|:--:|---|')
    for budget, block in flip['ANSWER'].items():
        miter = block['miterTaxMedianMm']
        rnd = block['roundTaxMedianMm']
        print(f"| `{budget}` | "
              f"{'-' if miter is None else f'{miter:+.4f} mm'} | "
              f"{'-' if rnd is None else f'{rnd:+.4f} mm'} | "
              f"{'**yes**' if block['flipped'] else 'no'} | "
              f"{block['roundArmedPublicationsUsingOffLatticePoses']} |")


def anytime_table(anytime, wall):
    print('\n### The anytime table\n')
    print('| budget | arm | seed medians (mm) | median | reproduced 2/2 | '
          'coordinator seconds (max) |')
    print('|---|---|---|---:|---:|---:|')
    for document, tag in ((anytime, 'plan'), (wall, 'wall')):
        if document is None:
            continue
        budgets = sorted({row['budget'] for row in document['rows']})
        for budget in budgets:
            for arm in [a['label'] for a in document['arms']]:
                rows = [r for r in document['rows']
                        if r['budget'] == budget and r['arm'] == arm]
                by_seed = {}
                for row in rows:
                    by_seed.setdefault(row['seed'], []).append(row['rawDepthMm'])
                medians = [statistics.median(v) for v in by_seed.values()
                           if v and v[0] is not None]
                reproduced = sum(1 for v in by_seed.values()
                                 if len(set(v)) == 1)
                seconds = [r['coordinatorSeconds'] for r in rows
                           if r['coordinatorSeconds'] is not None]
                print(f"| `{budget}` | {arm} | "
                      + ' / '.join(f'{m:.3f}' for m in medians)
                      + f" | **{statistics.median(medians):.3f}** | "
                      f"{reproduced}/{len(by_seed)} | "
                      f"{max(seconds):.2f} s |")


def publication_table(audit):
    print('\n### The publication audit\n')
    print('| quantity | value |')
    print('|---|---:|')
    for key in ('layouts', 'unionAcceptsAll', 'miterAcceptsAll',
                'roundAcceptsCount', 'newAdmissionCount', 'regressionCount'):
        print(f'| `{key}` | {audit[key]} |')


def sparrow_table(sparrow):
    print('\n### The Sparrow re-import, through the wire point\n')
    print('| allowance | expansion | contract | miter | round | **union** | '
          'kernel pair failures |')
    print('|---:|---:|:--:|:--:|:--:|:--:|---|')
    for row in sparrow['rows']:
        def mark(value):
            return 'accepts' if value else 'refuses'
        print(f"| {row['searchOffsetAllowanceMm']} mm | "
              f"{row['expansionMm']} mm | "
              f"{mark(row['contractOnlyAccepts'])} | "
              f"{mark(row['compositeMiterAccepts'])} | "
              f"{mark(row['compositeRoundAccepts'])} | "
              f"**{mark(row['compositeUnionAccepts'])}** | "
              f"{row['kernelPairFailureCount']} "
              f"{row['kernelRefusedPairIndices']} |")


def main():
    evidence = sys.argv[1]
    matched = load(f'{evidence}/matched.json')
    verdict = load(f'{evidence}/gate-verdict.json')
    if matched and verdict:
        matched_table(matched, verdict)
        clause_table(verdict)
        overhead_table(verdict, load(f'{evidence}/wallratio.json'))
    flip = load(f'{evidence}/crot-flip.json')
    if flip:
        crot_table(flip)
    anytime = load(f'{evidence}/anytime.json')
    if anytime:
        anytime_table(anytime, load(f'{evidence}/anytime-wall.json'))
    audit = load(f'{evidence}/publication-audit.json')
    if audit:
        publication_table(audit)
    sparrow = load(f'{evidence}/sparrow-republish.json')
    if sparrow:
        sparrow_table(sparrow)


if __name__ == '__main__':
    main()
