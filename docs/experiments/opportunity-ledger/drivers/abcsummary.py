#!/usr/bin/env python3
"""Prints the A/B/C table from an `abc.py` evidence document.

    python3 abcsummary.py evidence/abc-equalwork-mixed61.json
"""
import json
import sys


def main():
    document = json.load(open(sys.argv[1]))
    print('base work %s, probe allowance %s, allowance-mm %s, binary %s' % (
        format(document['baseWork'], ','), format(document['probeWork'], ','),
        document['allowance'], document['binary']))
    print('\n seed arm  entryRaw      exitRaw     delta  valid  probeWork'
          '   probeSec pubs calls exit')
    for row in document['rows']:
        if 'error' in row:
            print(' %4s %3s  ERROR %s' % (row['seed'], row['arm'],
                                          row['error'][:200]))
            continue
        print(' %4s %3s %10.4f %12.4f %9.4f %6s %10s %10.2f %4s %5s %s' % (
            row['seed'], row['arm'], row['entryRawDepthMm'],
            row['exitRawDepthMm'], row['deltaRawMm'],
            row['exitDualGateValid'],
            format(row['probeWorkUnitsSpent'], ','),
            row['probeSecondsSpent'], row['probePublications'],
            row['probeOperatorCalls'], row['probeExitCause']))
    print('\nper-call detail')
    for row in document['rows']:
        if 'error' in row:
            continue
        print(' seed %s arm %s  (%s)' % (row['seed'], row['arm'],
                                         '; '.join(row['probeSteps'])))
        for call in row['probeCalls']:
            print('    %-7s work=%12s sec=%6.2f exact=%-5s raw=%-20s '
                  'disp=%-22s published=%s%s' % (
                      call['operator'], format(call['workUnits'], ','),
                      call['elapsedSeconds'], call['exactValid'],
                      call['rawDepthMm'], call['archiveDisposition'],
                      call['published'],
                      '  fail=' + call['failureReason'][:70]
                      if call['failureReason'] else ''))
    print('\npaired per seed, delta raw of the best exact-valid publication')
    seeds = sorted({r['seed'] for r in document['rows'] if 'error' not in r})
    arms = document['arms']
    print(' seed  ' + '  '.join('%12s' % ('arm ' + a) for a in arms))
    for seed in seeds:
        cells = []
        for arm in arms:
            hit = [r for r in document['rows']
                   if r['seed'] == seed and r['arm'] == arm
                   and 'error' not in r]
            cells.append('%12.4f' % hit[0]['deltaRawMm'] if hit else
                         '%12s' % '-')
        print(' %4s  ' % seed + '  '.join(cells))


if __name__ == '__main__':
    main()
