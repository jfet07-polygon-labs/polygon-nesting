#!/usr/bin/env python3
"""Prints the five ledger tables from an evidence document.

    python3 summarize.py evidence/ledger-120M-mixed61.json
"""
import json
import statistics as st
import sys
from collections import OrderedDict


def main():
    document = json.load(open(sys.argv[1]))
    print('budget %s work units, allowance %s, binary %s' % (
        format(document['workBudget'], ','), document['allowance'],
        document['binary']))
    for seed, row in document['runs'].items():
        portfolio = row['portfolio']
        ledger = portfolio['ledger']
        incumbent = portfolio['incumbent']
        print('\n' + '=' * 78)
        print('seed %s  raw %r  dualGateValid=%s  work %s  wall %.2fs  det=%s'
              % (seed, incumbent['rawDepthMm'], incumbent['dualGateValid'],
                 format(portfolio['workUnits'], ','),
                 row['processWallSeconds'], row.get('determinism')))

        print('\n-- 3. phase exit causes --')
        for phase in portfolio['phases']:
            print('   %-12s work=%11s calls=%d pub=%d exit=%s' % (
                phase['name'], format(phase['workUnits'], ','),
                phase['operatorCalls'], phase['publications'],
                phase['exitCause']))

        print('\n-- 1. untried crossover actions --')
        actions = ledger['frontierActions']
        print('   whole archive: %d ordered pairs, %d actions, %d untried, '
              '%d untried and non-degenerate' % (
                  ledger['archiveOrderedPairs'],
                  ledger['archiveActionsTotal'],
                  ledger['archiveActionsUntried'],
                  ledger['archiveActionsUntriedNondegenerate']))
        groups = OrderedDict()
        for action in actions:
            key = (min(action['leftRank'], action['rightRank']),
                   max(action['leftRank'], action['rightRank']),
                   action['reciprocal'])
            groups.setdefault(key, []).append(action)
        print('   crossover frontier (top-%d):' % len({
            r for a in actions for r in (a['leftRank'], a['rightRank'])}))
        print('     pair  direction   actions distinct degenerate attempted')
        for key, members in groups.items():
            print('     (%d,%d) %-11s %7d %8d %10d %9d' % (
                key[0], key[1], 'reciprocal' if key[2] else 'forward',
                len(members),
                len({m['hybridFingerprint'] for m in members}),
                sum(1 for m in members if m['degenerate']),
                sum(1 for m in members if m['attempted'])))
        gaps = sorted(a['bandGapMm'] for a in actions)
        print('   interface band gap, mm: min %.4f  p50 %.4f  p95 %.4f  '
              'max %.4f' % (gaps[0], st.median(gaps),
                            gaps[int(0.95 * len(gaps))], gaps[-1]))
        print('   bands whose lower edge holds no differing piece: %d of %d'
              % (sum(1 for a in actions
                     if a['differingPiecesAtBand'] == 0), len(actions)))
        nxt = ledger['nextAction']
        if nxt:
            print('   NEXT untried action: %s rank%d->rank%d cut=%.9f '
                  'gap=%.4fmm midpointBand=%s fromLeft=%d fromRight=%d'
                  % ('reciprocal' if nxt['reciprocal'] else 'forward',
                     nxt['leftRank'], nxt['rightRank'], nxt['cutFraction'],
                     nxt['bandGapMm'], nxt['isMidpointBand'],
                     nxt['piecesFromLeft'], nxt['piecesFromRight']))

        ranks = {r['fingerprint']: r['depthRank']
                 for r in ledger['archiveRows']}
        print('   the mode-23 calls the schedule actually made:')
        for call in portfolio['operatorCalls']:
            if call['operator'] != 'mode23':
                continue
            left, right = (call['parentFingerprint'],
                           call['secondaryParentFingerprint'])
            print('     A=%s(finalRank %s) B=%s(finalRank %s) work=%s '
                  'published=%s raw=%s' % (
                      left[:8], ranks.get(left, 'evicted/absent'),
                      (right or '-')[:8], ranks.get(right, 'evicted/absent'),
                      format(call['workUnits'], ','), call['published'],
                      call['rawDepthMm']))

        print('\n-- 2 + 4. archive rows, selection and deferred credit --')
        print('   rk fingerprint operator      raw  ev dF xF excludedBy   '
              'acts desc dpub  bestDesc  gens')
        for r in ledger['archiveRows']:
            best = ('%.3f' % r['bestDescendantRawDepthMm']) \
                if r['bestDescendantRawDepthMm'] is not None else '-'
            print('   %2d %-11s %-9s %8.3f  %s  %s  %s %-12s %4d %4d %4d '
                  '%9s %5s' % (
                      r['depthRank'], r['fingerprint'][:10], r['operator'],
                      r['rawDepthMm'], str(r['exactValid'])[0],
                      str(r['inDescentFrontier'])[0],
                      str(r['inCrossoverFrontier'])[0],
                      str(r['excludedBy']), r['actionsReceived'],
                      r['descents'], r['descendantPublications'], best,
                      str(r['generationsToIncumbent'])))
        print('   members with no action at all: %d ; excluded by top-K: %d ; '
              'excluded by the bit-exact similarity rule: %d'
              % (ledger['membersWithoutAction'], ledger['excludedByTopK'],
                 ledger['excludedBySimilarity']))

        print('\n-- 5. cost and yield per action class --')
        print('   phase        operator  n pub     workTotal        p50'
              '        p95   s50   s95    dRaw  dRaw/Meval')
        for c in ledger['actionClasses']:
            print('   %-12s %-8s %2d %3d %13s %10s %10s %5.2f %5.2f %7.3f '
                  '%11.4f' % (
                      c['phase'], c['operator'], c['calls'], c['published'],
                      format(c['workUnitsTotal'], ','),
                      format(c['workUnitsP50'], ','),
                      format(c['workUnitsP95'], ','),
                      c['secondsP50'], c['secondsP95'], c['deltaRawMm'],
                      c['deltaRawPerMegaUnit']))

        print('\n-- 4. the incumbent lineage, birth order --')
        for step in ledger['incumbentLineage']:
            print('   %s %-9s raw=%.4f  born at %s units' % (
                step['fingerprint'][:10], step['operator'],
                step['rawDepthMm'], format(step['birthWorkUnits'], ',')))

        if portfolio.get('probe'):
            probe = portfolio['probe']
            print('\n-- probe --')
            print('   ' + json.dumps(probe))


if __name__ == '__main__':
    main()
