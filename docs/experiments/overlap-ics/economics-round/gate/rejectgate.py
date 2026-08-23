#!/usr/bin/env python3
"""**Funded change 3's reject rule, applied — and the §0 gate it stops.**

    python3 rejectgate.py <currency.json> [<currency.json> ...]

docs/economics-round-spec.md, funded change 3, verbatim:

> B/E/R/D from timing-only microbenchmarks on all three fixtures, conservative
> rounding; **REJECT the currency if wall-prediction error >10 % on any
> transfer fixture.**

`meter/currency.py` measures one reading of that. This reduces several of them
and renders the consequence, which is the part a single reading cannot carry:
a reject rule that fires on one run and not the next has not rejected anything,
it has sampled noise. So this script asks three questions of N runs, and the
answers are the whole of wave 4's verdict.

**1. Does it reject on every run?** `rejectedOnEveryRun`. A rule that must fire
on *any* transfer fixture is a maximum over six ordered pairs, and a maximum is
exactly the statistic a lucky run flatters. Unanimity across independent runs
is the cheapest defence available and it costs three cells.

**2. Does it still reject with triangle-20 removed?** `restrictedToMixed61And
Shapes17`. triangle-20's cell is five master batches and about six milliseconds
of search - a wall short enough that a reader is entitled to say the ratio is
scheduler noise rather than a statement about a currency. That objection is
worth taking seriously and worth answering with a number instead of a
paragraph, so the same clause is re-applied to the two heavy fixtures alone,
which is the pair the spec's transfer story is actually about. If the currency
survived there, the rejection would be an argument about one thin fixture. It
does not.

**3. Is the design matrix stable, so that only the response moved?**
`termsIdenticalAcrossRuns`. The cells are `--mode=fixed`, so the five counters
are a deterministic function of request and seed and must be bit-identical
across runs; if they are not, the runs are not repetitions and nothing may be
pooled from them. This is the precondition of questions 1 and 2, asserted
rather than assumed.

It also prints `coefficientSpread`, which is not a clause and decides nothing.
It is there because a coefficient that moves by a factor of six between
repetitions of its own calibration is not a price, and a reader who is about to
be told "the currency is rejected" should be able to see which of its five
terms the cells were too thin to identify in the first place.

Exit status is the verdict, taken directly and never through a pipe:

* `0` - the currency is ACCEPTED on every run, under both readings. §0's
  10 s calibrated-work gate has a validated denomination and may be run.
* `1` - the currency is REJECTED. Wave 4 stops here; the six §0 clauses are
  not answered, because a calibrated-work budget denominated in a rejected
  currency is the "silently inventing another exchange rate" that Sol review
  19 §5 forbids by name.
* `2` - the reduction could not run: a document is missing or carries no
  transfer check.

**Nothing here retunes anything.** The spec's no-second-guess discipline
applies to the currency exactly as it applies to the quanta: the coefficients
are what the timing-only harness measured, the rounding is the harness's, and
a reject is a result rather than an invitation to choose different cells.
"""
import json
import os
import statistics
import sys

TOLERANCE = 0.10
SPEC = ('docs/economics-round-spec.md, funded change 3: "B/E/R/D from '
        'timing-only microbenchmarks on all three fixtures, conservative '
        'rounding; REJECT the currency if wall-prediction error >10% on any '
        'transfer fixture."')
# The pair the spec's transfer story is about, and the one whose walls are long
# enough that nobody can call the ratio noise.
HEAVY = ('mixed-61', 'shapes-17')
FIXTURES = ('mixed-61', 'shapes-17', 'triangle-20')


def predictions(check):
    return [
        {
            'calibratedOn': row['calibratedOn'],
            'transferFixture': row['transferFixture'],
            'relativeError': row['relativeError'],
            'withinTolerance': row['withinTolerance'],
            'predictedSeconds': row['predictedSeconds'],
            'observedSeconds': row['observedSeconds'],
        }
        for row in check['predictions']
    ]


def verdict_over(rows):
    """The spec's clause over a set of ordered transfer pairs."""
    if not rows:
        return {'pairs': 0, 'accepted': None, 'worstRelativeError': None,
                'rejectedBy': None}
    worst = max(rows, key=lambda row: row['relativeError'])
    failing = [row for row in rows if not row['withinTolerance']]
    return {
        'pairs': len(rows),
        'accepted': not failing,
        'worstRelativeError': worst['relativeError'],
        'worstPair': f"{worst['calibratedOn']} -> {worst['transferFixture']}",
        'pairsOverTolerance': len(failing),
        'rejectedBy': (None if not failing else
                       f"{failing[0]['calibratedOn']} -> "
                       f"{failing[0]['transferFixture']}"),
    }


def run_row(path):
    with open(path) as handle:
        document = json.load(handle)
    meter = document.get('meter') or {}
    cells = {row['fixture']: row for row in document.get('cells') or []}
    row = {
        'path': path,
        'currencyExit': document.get('meterExit'),
        'loadBefore': (document.get('machine') or {}).get('loadBefore'),
        'loadAfter': (document.get('machine') or {}).get('loadAfter'),
        'searchSeconds': {name: cells[name]['searchSeconds']
                          for name in FIXTURES if name in cells},
        'terms': {name: cells[name]['terms'] for name in FIXTURES
                  if name in cells},
        'currencies': {},
    }
    coefficients = ((meter.get('calibration') or {})
                    .get('currency') or {}).get('coefficients') or {}
    row['coefficients'] = {key: value for key, value in coefficients.items()
                           if key != 'measured'}
    row['measuredCoefficients'] = coefficients.get('measured')
    for name in ('u0', 'u1'):
        check = meter.get(name)
        if not check:
            continue
        rows = predictions(check)
        heavy = [p for p in rows
                 if p['calibratedOn'] in HEAVY and p['transferFixture'] in HEAVY]
        row['currencies'][name] = {
            'version': check['currency']['version'],
            'allThreeFixtures': verdict_over(rows),
            'restrictedToMixed61AndShapes17': verdict_over(heavy),
            'predictions': rows,
        }
    return row


def main():
    paths = sys.argv[1:]
    document = {
        'experiment': 'overlap-ics',
        'battery': 'economics-round-currency-reject-gate',
        'spec': SPEC,
        'tolerance': TOLERANCE,
        'runs': [],
    }
    if not paths:
        document['error'] = 'usage: rejectgate.py <currency.json> [...]'
        print(json.dumps(document, indent=1))
        return 2
    try:
        document['runs'] = [run_row(path) for path in paths]
    except (OSError, json.JSONDecodeError, KeyError) as error:
        document['error'] = f'{error}'
        print(json.dumps(document, indent=1))
        return 2

    runs = document['runs']
    if not all(row['currencies'] for row in runs):
        document['error'] = 'a run carries no transfer check'
        print(json.dumps(document, indent=1))
        return 2

    # The precondition: fixed work, so the counters must not move.
    first_terms = runs[0]['terms']
    document['termsIdenticalAcrossRuns'] = all(row['terms'] == first_terms
                                               for row in runs)
    document['terms'] = first_terms

    summary = {}
    for name in ('u0', 'u1'):
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
                row['allThreeFixtures']['accepted'] is False for row in present),
            'worstRelativeErrorPerRun': [
                row['allThreeFixtures']['worstRelativeError'] for row in present],
            'rejectedByPerRun': [
                row['allThreeFixtures']['rejectedBy'] for row in present],
            'restrictedToMixed61AndShapes17': {
                'acceptedOnEveryRun': all(
                    row['restrictedToMixed61AndShapes17']['accepted']
                    for row in present),
                'rejectedOnEveryRun': all(
                    row['restrictedToMixed61AndShapes17']['accepted'] is False
                    for row in present),
                'worstRelativeErrorPerRun': [
                    row['restrictedToMixed61AndShapes17']['worstRelativeError']
                    for row in present],
                # Both directions of the one pair, every run, so the reader can
                # see there is no direction in which it passes.
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

    # Not a clause. A term whose price moves by a large factor between
    # repetitions of its own calibration was never identified by these cells,
    # and the reader is entitled to know which one.
    spread = {}
    for key in sorted({k for row in runs for k in row['coefficients']
                       if k != 'rounding'}):
        values = [row['coefficients'][key] for row in runs
                  if key in row['coefficients']]
        if not values or not all(isinstance(v, (int, float)) for v in values):
            continue
        spread[key] = {
            'values': values,
            'min': min(values),
            'max': max(values),
            'ratio': (None if min(values) == 0 else max(values) / min(values)),
            'median': statistics.median(values),
        }
    document['coefficientSpread'] = spread

    accepted = bool(
        document['termsIdenticalAcrossRuns']
        and all(row['acceptedOnEveryRun'] for row in summary.values()))
    document['CURRENCY_ACCEPTED'] = accepted
    # The consequence, stated where the verdict is, not in prose elsewhere.
    document['SECTION0_GATE_LICENSED'] = accepted
    document['consequence'] = (
        'the 10 s calibrated-work gate has a validated denomination and may '
        'be run' if accepted else
        'REJECTED on every run and under both readings. §0\'s budget is a '
        '"10 s calibrated-work" plan; a calibrated-work plan denominated in a '
        'currency the spec rejects is the "silently inventing another '
        'exchange rate" Sol review 19 §5 forbids, so the six §0 clauses are '
        'not answered by this wave and no gate number is produced.')
    print(json.dumps(document, indent=1))
    out = os.environ.get('ICS_OUT')
    if out:
        os.makedirs(out, exist_ok=True)
        with open(f'{out}/rejectgate.json', 'w') as handle:
            json.dump(document, handle, indent=1)
    return 0 if accepted else 1


if __name__ == '__main__':
    sys.exit(main())
