#!/usr/bin/env python3
"""**The amended reject rule, applied to `U'` - and the path it selects.**

    python3 rejectgate2.py <currency.json> [<currency.json> ...]

docs/currency-amendment.md, the final text carrying three signatures:

> `U' = sample_evaluations + B·master_batches + E·exact_checkpoint_calls
> + P·published_bites + D·disruption_moves`
> - `R` is DROPPED absolutely …
> - Same derivation (timing-only, three fixtures, conservative rounding),
>   **same >10% reject rule verbatim, still a stop.**

`../gate/rejectgate.py` did this for the signed `U`. This does it for `U'`, and
the differences between the two files are exactly the differences between the
two currencies:

**1. The rule is applied over all three fixtures, and nothing is dropped.** The
wave's instruction is verbatim: *">10% wall-prediction error on ANY transfer
reading (3 runs × ordered pairs, all three fixtures) → `U'` is rejected. No
charity, no reweighing, no dropping fixtures. Record every reading."* So the
verdict here is `allThreeFixtures` and only that. The heavy-pair reading is
still computed and still printed - `U`'s stop rested on it and a reader is
entitled to the comparison - but it is labelled a **diagnostic** and it cannot
change the verdict in either direction.

**2. `P` gets the check `R` never survived.** `coefficientSpread` is not a
clause here either, but the amendment wrote a rule for a *future* currency that
wants `R` back - *"spread ≤1.5x across three runs AND support on ≥2 fixtures"* -
and the honest thing to do with a rule written for the next round is to apply it
to this round's new term and print the answer. `pSpreadUnderFutureRRule` is
that: it decides nothing today and it is the first thing the next proposal will
be asked for.

**3. Rider (ii) is reported, not re-decided.** The `E` and `P` design vectors
come out of the meter side by side, with the collinearity verdict the meter
reached at bars that are constants in `search::overlap_ics_meter::currency`.
This file prints them and checks the three runs agree; it does not re-derive
them, because a second implementation of a criterion is a second criterion.

**4. The consequence is a path, and the path is named.** `U` rejected meant
"no gate number". `U'` rejected means the **declared fallback**, which is final
per rider (iii): a mixed-61-only shelf-probed work budget labelled *"single-
fixture work plan, no transfer claim"*, with clauses (1)(2)(3)(4)(6) binding
unchanged and clause (5) binding as a claim. `BUDGET_PATH` is the field that
says which, and the gate battery reads it rather than a person deciding.

Exit status is the verdict, taken directly and never through a pipe:

* `0` - `U'` is ACCEPTED on every run. The calibrated-work plan is denominated
  in `U'` and the gate spends it.
* `1` - `U'` is REJECTED. The declared fallback runs. **This is not a stop:**
  the amendment's fallback is a gate that runs, on a budget that is honestly
  labelled, and clause (5) still binds as a claim.
* `2` - the reduction could not run.
"""
import hashlib
import importlib.util
import json
import os
import statistics
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
_spec = importlib.util.spec_from_file_location(
    'gate_rejectgate', f'{HERE}/../gate/rejectgate.py')
gate_rejectgate = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(gate_rejectgate)

TOLERANCE = gate_rejectgate.TOLERANCE
HEAVY = gate_rejectgate.HEAVY
FIXTURES = gate_rejectgate.FIXTURES
AMENDMENT = (
    'docs/currency-amendment.md: "U\' = sample_evaluations + B*master_batches '
    '+ E*exact_checkpoint_calls + P*published_bites + D*disruption_moves … '
    'Same derivation (timing-only, three fixtures, conservative rounding), '
    'same >10% reject rule verbatim, still a stop."')
# The amendment's rule for restoring `R` in a future funding, applied here to
# `P` as a report and never as a clause.
FUTURE_R_RULE_SPREAD = 1.5
FUTURE_R_RULE_FIXTURES = 2


def currency_row(check, coefficients, version):
    rows = gate_rejectgate.predictions(check)
    heavy = [p for p in rows
             if p['calibratedOn'] in HEAVY and p['transferFixture'] in HEAVY]
    return {
        'version': version,
        'coefficients': coefficients,
        # THE verdict: all three fixtures, every ordered pair, nothing dropped.
        'allThreeFixtures': gate_rejectgate.verdict_over(rows),
        # Diagnostic only. Printed because `U`'s stop rested on it.
        'restrictedToMixed61AndShapes17_DIAGNOSTIC':
            gate_rejectgate.verdict_over(heavy),
        'predictions': rows,
    }


def run_row(path):
    with open(path) as handle:
        document = json.load(handle)
    meter = document.get('meter') or {}
    cells = {row['fixture']: row for row in document.get('cells') or []}
    prime_cells = {row['fixture']: row
                   for row in meter.get('cellsPrime') or []}
    calibration = meter.get('calibrationPrime') or {}
    coefficients = ((calibration.get('currency') or {})
                    .get('coefficients') or {})
    row = {
        'path': path,
        # RV3: a reduction names the bytes it reduced.
        'sourceSha256': hashlib.sha256(
            open(path, 'rb').read()).hexdigest(),
        'currencyExit': document.get('meterExit'),
        'exitMeans': meter.get('EXIT_MEANS'),
        'loadBefore': (document.get('machine') or {}).get('loadBefore'),
        'loadAfter': (document.get('machine') or {}).get('loadAfter'),
        'searchSeconds': {name: cells[name]['searchSeconds']
                          for name in FIXTURES if name in cells},
        # `U'`'s design matrix, including the counter rider (i) proved.
        'terms': {name: prime_cells[name]['terms'] for name in FIXTURES
                  if name in prime_cells},
        'coefficients': {key: value for key, value in coefficients.items()
                         if key not in ('measured', 'rounding')},
        'measuredCoefficients': coefficients.get('measured'),
        'collinearity': calibration.get('collinearity'),
        'residualSplit': calibration.get('residualSplit'),
        'notes': calibration.get('notes'),
        'currencies': {},
    }
    for name, version in (('u0', 'U0-sample-evaluations'),
                          ('u1', 'U1-weighted-vector'),
                          ('u2', 'U2-per-bite-vector')):
        check = meter.get(name)
        if not check:
            continue
        row['currencies'][name] = currency_row(
            check,
            row['coefficients'] if name == 'u2' else None,
            version if name != 'u1' else check['currency']['version'])
    return row


def main():
    paths = sys.argv[1:]
    document = {
        'experiment': 'overlap-ics',
        'battery': 'economics-round-amended-currency-reject-gate',
        'amendment': AMENDMENT,
        'rule': ('>10% wall-prediction error on ANY transfer reading (3 runs x '
                 'ordered pairs, all three fixtures) rejects U\'. No charity, '
                 'no reweighing, no dropping fixtures.'),
        'tolerance': TOLERANCE,
        'runs': [],
    }
    if not paths:
        document['error'] = 'usage: rejectgate2.py <currency.json> [...]'
        print(json.dumps(document, indent=1))
        return 2
    try:
        document['runs'] = [run_row(path) for path in paths]
    except (OSError, json.JSONDecodeError, KeyError) as error:
        document['error'] = f'{error}'
        print(json.dumps(document, indent=1))
        return 2

    runs = document['runs']
    if not all(row['currencies'].get('u2') for row in runs):
        document['error'] = 'a run carries no U\' transfer check'
        print(json.dumps(document, indent=1))
        return 2

    # The precondition: fixed work, so the counters must not move - and that
    # now includes `publishedBites`, which is the whole of rider (i) restated
    # over the runs the coefficient was actually fitted from.
    first_terms = runs[0]['terms']
    document['termsIdenticalAcrossRuns'] = all(row['terms'] == first_terms
                                               for row in runs)
    document['terms'] = first_terms
    document['publishedBitesPerRun'] = [
        {name: row['terms'][name]['publishedBites'] for name in row['terms']}
        for row in runs]
    document['publishedBitesIdenticalAcrossRuns'] = all(
        vector == document['publishedBitesPerRun'][0]
        for vector in document['publishedBitesPerRun'])

    summary = {}
    for name in ('u0', 'u1', 'u2'):
        present = [row['currencies'][name] for row in runs
                   if name in row['currencies']]
        if not present:
            continue
        summary[name] = {
            'version': present[0]['version'],
            'runs': len(present),
            'acceptedOnEveryRun': all(
                row['allThreeFixtures']['accepted'] for row in present),
            'rejectedOnEveryRun': all(
                row['allThreeFixtures']['accepted'] is False
                for row in present),
            'worstRelativeErrorPerRun': [
                row['allThreeFixtures']['worstRelativeError']
                for row in present],
            'worstPairPerRun': [row['allThreeFixtures']['worstPair']
                                for row in present],
            'pairsOverTolerancePerRun': [
                row['allThreeFixtures']['pairsOverTolerance']
                for row in present],
            # Every reading, every run, every ordered pair. The instruction is
            # "record every reading" and this is where they all are.
            'everyReadingPerRun': [
                {f"{p['calibratedOn']} -> {p['transferFixture']}":
                 p['relativeError'] for p in row['predictions']}
                for row in present],
            'restrictedToMixed61AndShapes17_DIAGNOSTIC': {
                'note': ('a diagnostic, never the verdict: the amended rule '
                         'drops no fixture'),
                'worstRelativeErrorPerRun': [
                    row['restrictedToMixed61AndShapes17_DIAGNOSTIC'][
                        'worstRelativeError'] for row in present],
                'errorsPerRun': [
                    {f"{p['calibratedOn']} -> {p['transferFixture']}":
                     p['relativeError']
                     for p in row['predictions']
                     if p['calibratedOn'] in HEAVY
                     and p['transferFixture'] in HEAVY}
                    for row in present],
            },
        }
    document['currencies'] = summary

    # Rider (ii): reported, not re-decided, and required to be stable.
    collinearities = [row['collinearity'] for row in runs]
    document['riderTwo'] = {
        'criterion': ('collinear iff per-fixture ratio max/min <= ratioBar AND '
                      'cosine >= cosineBar; both bars are constants in '
                      'search::overlap_ics_meter::currency'),
        'designVectors': {
            'fixtures': collinearities[0]['fixtures'],
            'E_exactCheckpointCalls': collinearities[0]['exactCheckpointCalls'],
            'P_publishedBites': collinearities[0]['publishedBites'],
        },
        'ratios': collinearities[0]['ratios'],
        'ratioMaxOverMin': collinearities[0]['ratioMaxOverMin'],
        'ratioBar': collinearities[0]['ratioBar'],
        'cosine': collinearities[0]['cosine'],
        'cosineBar': collinearities[0]['cosineBar'],
        'collinear': collinearities[0]['collinear'],
        'identicalAcrossRuns': all(row == collinearities[0]
                                   for row in collinearities),
        'consequence': ('ONE combined E,P term was fitted'
                        if collinearities[0]['collinear']
                        else 'two separate prices were fitted'),
    }

    # Not a clause. `R` moved 6.89x between repetitions of its own calibration
    # and was never a price; this is the same question asked of `U'`'s terms.
    spread = {}
    for key in sorted({k for row in runs for k in row['coefficients']}):
        values = [row['coefficients'][key] for row in runs
                  if key in row['coefficients']]
        # `bool` is an `int` in Python, and `combinedEAndP` is a verdict rather
        # than a price: a min/max/ratio over it would be arithmetic on a
        # decision. Rider (ii)'s verdict is in `riderTwo`, where it belongs.
        if not values or not all(isinstance(v, (int, float))
                                 and not isinstance(v, bool) for v in values):
            continue
        spread[key] = {
            'values': values,
            'min': min(values),
            'max': max(values),
            'ratio': (None if min(values) == 0 else max(values) / min(values)),
            'median': statistics.median(values),
        }
    document['coefficientSpread'] = spread

    p_support = sum(1 for name in first_terms
                    if first_terms[name]['publishedBites'] > 0)
    p_spread = (spread.get('pPublishedBite') or {}).get('ratio')
    document['pSpreadUnderFutureRRule'] = {
        'note': ('the amendment\'s rule for restoring R in a FUTURE funding, '
                 'applied to P as a report. It decides nothing today.'),
        'rule': f'spread <= {FUTURE_R_RULE_SPREAD}x across three runs AND '
                f'support on >= {FUTURE_R_RULE_FIXTURES} fixtures',
        'spreadRatio': p_spread,
        'fixturesWithSupport': p_support,
        'wouldPass': bool(p_spread is not None
                          and p_spread <= FUTURE_R_RULE_SPREAD
                          and p_support >= FUTURE_R_RULE_FIXTURES),
    }

    accepted = bool(
        document['termsIdenticalAcrossRuns']
        and summary['u2']['acceptedOnEveryRun'])
    document['CURRENCY_PRIME_ACCEPTED'] = accepted
    document['BUDGET_PATH'] = (
        'calibrated-work plan denominated in U\', spent through the icscal '
        'read path' if accepted else
        'DECLARED FALLBACK: mixed-61-only shelf-probed work budget, labelled '
        '"single-fixture work plan, no transfer claim". Final per rider (iii).')
    document['SECTION0_CLAUSES'] = (
        'all six bind on U\'' if accepted else
        'clauses (1)(2)(3)(4)(6) bind unchanged; clause (5) p95<=10.000s '
        'STILL BINDS as a claim and the budget is not retuned after seeing '
        'p95; the 30 s shapes-17/triangle-20 equal-work clause is inevaluable '
        'and recorded NOT-RUN')
    print(json.dumps(document, indent=1))
    out = os.environ.get('ICS_OUT')
    if out:
        os.makedirs(out, exist_ok=True)
        with open(f'{out}/rejectgate2.json', 'w') as handle:
            json.dump(document, handle, indent=1)
    return 0 if accepted else 1


if __name__ == '__main__':
    sys.exit(main())
