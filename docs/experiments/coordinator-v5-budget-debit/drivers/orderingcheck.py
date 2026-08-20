#!/usr/bin/env python3
"""Sol review 6 §1 finding 4, checked on real run documents.

    orderingcheck.py OUT RUN.json [RUN.json ...]

The claim under test is that the debit a self-metered action incurs is on the
meter *before* that action's own archive entry, publication and call report are
stamped - not one action later.

Two kinds of check, and the difference matters:

* **Exact.** `OperatorCallReport.workUnits == globalUnits + debitedUnits`. The
  pre-fix ordering computed `work_units` from the meter *before* the debit, so
  it could only ever produce `workUnits == globalUnits`. Any call with a
  non-zero debit that satisfies the identity is a call whose report was written
  after its own charge settled. This is a discriminator: the old code cannot
  produce it.
* **Bound.** The publication and the archived basin the same call produced
  carry a cumulative meter reading, so the strongest statement available from
  the document alone is that the reading is at least the cumulative debit
  charged through this call inclusive. The pre-fix ordering can also satisfy
  that bound when the global counter happens to be large, so this is a
  consistency check, not a proof - it is reported as one.
"""
import json
import sys


def check(path):
    doc = json.load(open(path))
    portfolio = doc.get('portfolio') or {}
    calls = sorted((portfolio.get('operatorCalls') or []),
                   key=lambda c: c.get('startedSeconds') or 0.0)
    publications = {p['fingerprint']: p
                    for p in portfolio.get('publications') or []}
    basins = {b['fingerprint']: b
              for b in (portfolio.get('archive') or {}).get('members') or []}
    cumulative = 0
    rows = []
    for call in calls:
        debit = call.get('debitedUnits') or 0
        cumulative += debit
        if debit <= 0:
            continue
        fingerprint = call.get('resultFingerprint')
        row = {
            'operator': call['operator'],
            'phase': call['phase'],
            'globalUnits': call['globalUnits'],
            'selfMeteredUnits': call['selfMeteredUnits'],
            'debitedUnits': debit,
            'workUnits': call['workUnits'],
            # Exact: the identity the pre-fix ordering could not produce.
            'reportExact':
                call['workUnits'] == call['globalUnits'] + debit,
            'reportWouldBePreFix': call['workUnits'] == call['globalUnits'],
            'cumulativeDebitInclusive': cumulative,
        }
        publication = publications.get(fingerprint)
        if publication:
            row['publicationWorkUnits'] = publication['workUnits']
            row['publicationAtLeastCumulative'] = \
                publication['workUnits'] >= cumulative
        basin = basins.get(fingerprint)
        if basin:
            row['birthWorkUnits'] = basin['birthWorkUnits']
            row['birthAtLeastCumulative'] = \
                basin['birthWorkUnits'] >= cumulative
        rows.append(row)
    return rows


def main():
    out_path = sys.argv[1]
    result = {'runs': {}, 'totals': {}}
    totals = {'debitedCalls': 0, 'reportExact': 0, 'reportWouldBePreFix': 0,
              'publicationsChecked': 0, 'publicationAtLeastCumulative': 0,
              'birthsChecked': 0, 'birthAtLeastCumulative': 0}
    for path in sys.argv[2:]:
        rows = check(path)
        result['runs'][path.split('/')[-1]] = rows
        for row in rows:
            totals['debitedCalls'] += 1
            totals['reportExact'] += bool(row['reportExact'])
            totals['reportWouldBePreFix'] += bool(row['reportWouldBePreFix'])
            if 'publicationAtLeastCumulative' in row:
                totals['publicationsChecked'] += 1
                totals['publicationAtLeastCumulative'] += \
                    bool(row['publicationAtLeastCumulative'])
            if 'birthAtLeastCumulative' in row:
                totals['birthsChecked'] += 1
                totals['birthAtLeastCumulative'] += \
                    bool(row['birthAtLeastCumulative'])
    result['totals'] = totals
    result['ALL_EXACT'] = (totals['debitedCalls'] > 0
                           and totals['reportExact'] == totals['debitedCalls']
                           and totals['reportWouldBePreFix'] == 0)
    print(json.dumps(totals, indent=1))
    print('ALL_EXACT', result['ALL_EXACT'])
    json.dump(result, open(out_path, 'w'), indent=1)


if __name__ == '__main__':
    main()
