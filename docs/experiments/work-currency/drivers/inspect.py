#!/usr/bin/env python3
"""One run's operator calls, in both currencies. The debugging instrument.

    python3 inspect.py RUN_JSON [--all]

Prints every operator call with its shipped-meter delta, its own meter when it
carries one, the class price the parallel currency computed, and what the
settlement actually charged - which is the four-column comparison
`OperatorCharge` exists to make readable.
"""
import json
import sys

COUNTS = ['candidateQueries', 'exactPairTests', 'collisionBuilds',
          'neighborTests', 'fullRescores', 'positionSourceAttempts',
          'returnedPositions', 'pairVisits', 'operatorCollisionBuilds',
          'confirmations']


def main():
    doc = json.load(open(sys.argv[1]))
    show_all = '--all' in sys.argv
    portfolio = doc.get('portfolio') or {}
    print(json.dumps(portfolio.get('workCurrency'), indent=1))
    header = (f"{'op':>8} {'phase':>12} {'wall':>7} {'global':>13} "
              f"{'self':>13} {'class':>13} {'charged':>13} {'extra':>11}")
    print(header)
    for call in portfolio.get('operatorCalls', []):
        currency = call.get('workCurrency') or {}
        if not show_all and not currency.get('chargedExtraUnits'):
            continue
        print(f"{call['operator']:>8} {call['phase'][:12]:>12} "
              f"{call['elapsedSeconds']:>7.3f} {call['globalUnits']:>13,} "
              f"{(call.get('selfMeteredUnits') or 0):>13,} "
              f"{currency.get('classUnits', 0):>13,} "
              f"{call['workUnits']:>13,} "
              f"{currency.get('chargedExtraUnits', 0):>11,}")
        nonzero = {k: currency[k] for k in COUNTS if currency.get(k)}
        if nonzero:
            print('          ' + '  '.join(f'{k}={v:,}'
                                           for k, v in nonzero.items()))
        if call.get('action'):
            print(f"          action: {call['action']}")


if __name__ == '__main__':
    main()
