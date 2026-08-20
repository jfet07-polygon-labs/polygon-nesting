#!/usr/bin/env python3
"""The direct, paired measurement of Sol review 6 §1 finding 4.

    stampdelta.py OUT BATTERY.json

`orderingcheck.py` can only bound the publication and archive stamps from
inside one document. This compares the *same* layout's stamps across the fixed
and unfixed arms of a paired cell, which turns the bound into an identity.

The two arms are bit-identical run prefixes until the fixed arm's honest budget
diverges from the unfixed arm's optimistic one - same actions, same
`meteredCost` to the unit - so for any layout fingerprint both arms produced,
the global counter read the same at the moment it was stamped. The only
difference in the stamp is the debit. Therefore:

    fixedStamp - unfixedStamp == cumulative debit *through this call inclusive*

is the corrected ordering, and

    fixedStamp - unfixedStamp == cumulative debit *strictly before this call*

is what the pre-fix ordering produced - Sol's "le pubblicazioni successive
includono quello precedente: curva temporalmente incoerente". The two differ by
exactly this call's own debit, so on any call with a non-zero debit the check
is a discriminator and not a plausibility argument.

Reported per fingerprint for both `publications[].workUnits` and the archived
basin's `birthWorkUnits`.

**Duplicates are excluded, and this is load-bearing.** A basin's
`birthWorkUnits` and a publication's `workUnits` are stamped by the call that
produced the layout *first*. A later call that lands on the same fingerprint is
archived `Duplicate` and publishes nothing, so it creates no new stamp: reading
the stamp under its fingerprint reads the earlier call's, whose cumulative
debit is by definition smaller. Including those rows makes the check report a
difference that looks like the pre-fix identity and is nothing of the kind -
which is exactly what the first version of this driver did, on 6 of 102 rows at
120M, all of them `archiveDisposition == "Duplicate"` on the same seed. They
are now counted under `skippedDuplicate` rather than scored.
"""
import json
import os
import sys


def stamps(doc):
    portfolio = doc.get('portfolio') or {}
    return (
        {p['fingerprint']: p['workUnits']
         for p in portfolio.get('publications') or []},
        {b['fingerprint']: b['birthWorkUnits']
         for b in (portfolio.get('archive') or {}).get('members') or []},
        sorted((portfolio.get('operatorCalls') or []),
               key=lambda c: c.get('startedSeconds') or 0.0),
    )


def cell(fixed_path, unfixed_path):
    fixed_pubs, fixed_births, fixed_calls = stamps(json.load(open(fixed_path)))
    unfixed_pubs, unfixed_births, _ = stamps(json.load(open(unfixed_path)))
    rows = []
    cumulative = 0
    produced_earlier = set()
    for call in fixed_calls:
        debit = call.get('debitedUnits') or 0
        before = cumulative
        cumulative += debit
        fingerprint = call.get('resultFingerprint')
        first_producer = fingerprint is not None \
            and fingerprint not in produced_earlier
        if fingerprint is not None:
            produced_earlier.add(fingerprint)
        if debit <= 0:
            continue
        row = {'operator': call['operator'], 'debitedUnits': debit,
               'cumulativeInclusive': cumulative,
               'cumulativeExclusive': before,
               'archiveDisposition': call.get('archiveDisposition'),
               'firstProducerOfFingerprint': first_producer}
        if not first_producer or call.get('archiveDisposition') == 'Duplicate':
            # No new stamp exists for this call: the archive entry and the
            # publication under this fingerprint belong to the call that got
            # there first. Nothing to compare - see the module docstring.
            row['skippedDuplicate'] = True
            rows.append(row)
            continue
        for name, left, right in (('publication', fixed_pubs, unfixed_pubs),
                                  ('birth', fixed_births, unfixed_births)):
            if fingerprint in left and fingerprint in right:
                delta = left[fingerprint] - right[fingerprint]
                row[f'{name}Delta'] = delta
                row[f'{name}FixedStamp'] = left[fingerprint]
                row[f'{name}UnfixedStamp'] = right[fingerprint]
                row[f'{name}MatchesCorrected'] = delta == cumulative
                row[f'{name}MatchesPreFix'] = delta == before
        rows.append(row)
    return rows


def main():
    out_path = sys.argv[1]
    battery = json.load(open(sys.argv[2]))
    run_dir = os.path.join(os.path.dirname(os.path.abspath(sys.argv[2])),
                           'runs')
    result = {'battery': battery['name'], 'spec': battery['spec'], 'cells': {}}
    totals = {'checked': 0, 'correctedIdentity': 0, 'preFixIdentity': 0,
              'comparable': 0, 'skippedDuplicate': 0}
    keys = sorted({(r['seed'], r['round']) for r in battery['rows']})
    for seed, rnd in keys:
        fixed = f'{run_dir}/fixed-s{seed}-r{rnd}.json'
        unfixed = f'{run_dir}/unfixed-s{seed}-r{rnd}.json'
        if not (os.path.exists(fixed) and os.path.exists(unfixed)):
            continue
        rows = cell(fixed, unfixed)
        result['cells'][f's{seed}-r{rnd}'] = rows
        for row in rows:
            if row.get('skippedDuplicate'):
                totals['skippedDuplicate'] += 1
                continue
            totals['checked'] += 1
            for name in ('publication', 'birth'):
                if f'{name}MatchesCorrected' not in row:
                    continue
                totals['comparable'] += 1
                totals['correctedIdentity'] += row[f'{name}MatchesCorrected']
                totals['preFixIdentity'] += row[f'{name}MatchesPreFix']
    result['totals'] = totals
    result['ALL_CORRECTED'] = (
        totals['comparable'] > 0
        and totals['correctedIdentity'] == totals['comparable']
        and totals['preFixIdentity'] == 0)
    print(json.dumps(totals, indent=1))
    print('ALL_CORRECTED', result['ALL_CORRECTED'])
    json.dump(result, open(out_path, 'w'), indent=1)


if __name__ == '__main__':
    main()
