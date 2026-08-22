#!/usr/bin/env python3
"""Every table in this round's README, rendered from `evidence/verdict.json`.

    tables.py VERDICTJSON

Nothing here computes: `verdict.py` computed, this prints. A number in the
README that this does not print is a number the README made up.
"""
import json
import sys

ORDER = ['m34:1670689', 'm34:3341379', 'm34:6682758', 'm34:15000000',
         'm34:33413789']


def row(cells):
    return '| ' + ' | '.join(cells) + ' |'


def main():
    doc = json.load(open(sys.argv[1]))
    arm, ladder, controls = doc['arm'], doc['ladder'], doc['controls']
    chosen = doc['chosenControl']

    print('### The battery, by arm\n')
    print(row(['arm', 'median delta', 'moved', 'median operator work',
               'median wall', 'mm / M operator unit (aggregate)',
               'mm / wall s (aggregate)']))
    print(row(['---', '---:', '---:', '---:', '---:', '---:', '---:']))

    def line(label, s, mark=''):
        return row([f'{label}{mark}',
                    f"{s['medianDeltaMm']:.4f} mm",
                    f"{s['cellsMoved']}/12",
                    f"{s['medianOperatorWorkUnits'] / 1e6:.3f} M",
                    f"{s['medianWallSeconds']:.2f} s",
                    f"**{s['aggregateMmPerMegaOperatorWork']:.4f}**",
                    f"{s['aggregateMmPerWallSecond']:.4f}"])

    print(line('`m26:1rung` (the arm)', arm))
    print(line('`m26:drop1.0` (uncapped, 6 rungs)', ladder))
    for label in ORDER:
        if label in controls:
            print(line(f'`{label}`', controls[label],
                       ' **<- work-matched**' if label == chosen else ''))

    print('\n### The kill rule, at every control budget\n')
    print(row(['control', 'control median', 'arm median', 'arm - control',
               'clause A (>= +1 mm)', 'clause B (arm below on >= 8/12)',
               'verdict']))
    print(row(['---', '---:', '---:', '---:', '---', '---', '---']))
    for label in ORDER:
        k = doc['killRuleAtEveryControlBudget'].get(label)
        if k is None:
            continue
        print(row([f'`{label}`' + (' **<- chosen**' if label == chosen else ''),
                   f"{k['controlMedianDeltaMm']:.4f} mm",
                   f"{k['armMedianDeltaMm']:.4f} mm",
                   f"{k['armMedianMinusControlMedianMm']:+.4f} mm",
                   'pass' if k['clauseA_beatsControlMedianBy1mm'] else 'FAIL',
                   ('pass' if k['clauseB_belowControlOnAtLeast8of12']
                    else f"FAIL ({k['armBelowControlCells']}/12)"),
                   f"**{k['verdict']}**"]))

    print('\n### Per parent, arm against the work-matched control\n')
    print(row(['seed', 'parent raw', 'arm published', 'arm delta',
               'control published', 'control delta', 'arm below control?']))
    print(row(['---:', '---:', '---:', '---:', '---:', '---:', '---']))
    per = {r['seed']: r for r in doc['killRule']['perSeed']}
    for cell in doc['armPerSeed']:
        seed = cell['seed']
        k = per[seed]
        print(row([str(seed),
                   f"{cell['parentRawDepthMm']:.4f}",
                   f"{k['armPublishedMm']:.4f}",
                   f"{k['armDeltaMm']:.4f}",
                   f"{k['controlPublishedMm']:.4f}",
                   f"{k['controlDeltaMm']:.4f}",
                   'yes' if k['armBelowControl'] else 'no']))

    print('\n### The abort census, counted rather than assumed\n')
    print(row(['ladder', 'rungs run', 'rung arms run',
               'aborted on the rollback disagreement',
               'produced no state at all', 'produced an exact-valid state']))
    print(row(['---', '---:', '---:', '---:', '---:', '---:']))
    for label, s in (('`m26:1rung`', arm), ('`m26:drop1.0`', ladder)):
        print(row([label, str(s['rungsRun']), str(s['armsRun']),
                   f"{s['armsAbortedByRollbackDisagreement']} "
                   f"({s['abortShare'] * 100:.1f}%)",
                   str(s['armsProducingNoState']),
                   str(s['armsExactValid'])]))

    print('\n### The harness floor, per seed (work units a refused mode-34 '
          'process burns before any operator runs)\n')
    floor = doc['harnessFloorWorkUnits']
    print(row(['seed'] + [str(s) for s in sorted(floor, key=int)]))
    print(row(['---'] + ['---:'] * len(floor)))
    print(row(['floor'] + [f'{floor[s] / 1e6:.2f} M'
                           for s in sorted(floor, key=int)]))


if __name__ == '__main__':
    main()
