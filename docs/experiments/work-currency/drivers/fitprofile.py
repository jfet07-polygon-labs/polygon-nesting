#!/usr/bin/env python3
"""§2: fit the machine profile - one weight per count, per class.

What is being fitted, precisely: for each operator class `c` and each candidate
count `k`, the weight

    w[c][k] = WORK_CURRENCY_REFERENCE_RATE / (counts[k] per second, in class c)

so that `w[c][k] * n[k]` is the shipped-meter units the class *would* have
retired in the wall those `n[k]` counts took. That is the whole exchange rate:
a count is priced at the reference class's units-per-second times the seconds
the count costs.

The fit is deliberately **one count per class**, not a regression over all ten.
Two reasons, both measured rather than stylistic:

* the sample is small and correlated - a class's counts move together within a
  request, so a ten-parameter fit on seven mode-20 calls across three requests
  is fitting the request table, not the box;
* a single count is auditable. `w * n` is one multiplication a reader can do by
  hand against the table in §1, and a weight that is wrong is wrong visibly.

The selection statistic is the **residual**, not the rate spread: for each
candidate count the driver charges every call with the weight that count's
median rate implies, compares it against the honest target `wall *
REFERENCE_RATE`, and scores the candidate by the geometric RMS of the ratio.
That is the quantity a single constant actually has to survive, and it is not
the quantity the weight was fitted to minimise - the weight is a median of
rates, the score is a spread of charges - so the ranking is a check rather
than a restatement. Every candidate's score is reported, so the choice can be
argued with.

Only classes the shipped meter **under**-prices are fitted at all
(`MISPRICE_THRESHOLD`). A class the shipped meter already prices within a
factor of the reference rate is left at `DEFAULT_CLASS_PRICE`, where the
currency's `max` settlement is the identity on it. That is the fail-safe
direction and it is deliberate: a repricing that is not needed is a constant
that will be wrong on the next request.

Usage:
    python3 fitprofile.py RATES_JSON [OUT_JSON]
"""
import json
import math
import statistics
import sys

# Must match `work_currency::WORK_CURRENCY_REFERENCE_RATE` and
# `WORK_CURRENCY_SCALE`.
REFERENCE_RATE = 2_600_000
SCALE = 1_000

# A class whose shipped-meter rate is within this factor of the reference rate
# is already comparable and is not repriced. `3.0` is above every measured
# class but mode 20, whose ratio is 3.4e-05 - four and a half orders of
# magnitude away from the nearest other class, so the threshold is not a
# borderline call on this evidence and no class sits near it.
MISPRICE_THRESHOLD = 3.0

COUNT_KEYS = [
    'candidateQueries', 'exactPairTests', 'collisionBuilds', 'neighborTests',
    'fullRescores', 'positionSourceAttempts', 'returnedPositions',
    'pairVisits', 'operatorCollisionBuilds', 'confirmations',
]

# A call shorter than this is a call whose wall is mostly the process clock's
# resolution and the scheduler's; it cannot constrain a rate.
MIN_SECONDS = 0.05


def candidates_for(calls):
    """Every count that is non-zero on every call, scored by its residual."""
    usable = [c for c in calls if c['elapsedSeconds'] >= MIN_SECONDS]
    rows = []
    for key in COUNT_KEYS:
        rates = [c['counts'][key] / c['elapsedSeconds']
                 for c in usable if c['counts'].get(key)]
        if len(rates) != len(usable) or not rates:
            continue
        lo, hi = min(rates), max(rates)
        weight = REFERENCE_RATE / statistics.median(rates)
        ratios = [(call['counts'][key] * weight)
                  / (call['elapsedSeconds'] * REFERENCE_RATE)
                  for call in usable]
        logs = [math.log(ratio) for ratio in ratios]
        rows.append({
            'count': key,
            'calls': len(rates),
            'medianPerSecond': statistics.median(rates),
            'minPerSecond': lo,
            'maxPerSecond': hi,
            'weight': weight,
            'scaledWeight': round(weight * SCALE),
            # How far apart the slowest and fastest observation of this count
            # are - reported because it is the intuitive number, and not used
            # to rank.
            'spread': (hi / lo) if lo else float('inf'),
            # The ranking statistic: how far the *charge* lands from the
            # honest target, worst case in each direction and in geometric
            # RMS.
            'worstOvercharge': max(ratios),
            'worstUndercharge': min(ratios),
            'residualRms': math.exp(
                math.sqrt(sum(value * value for value in logs) / len(logs))),
        })
    rows.sort(key=lambda row: row['residualRms'])
    return rows, usable


def main():
    rates = json.load(open(sys.argv[1]))
    out = sys.argv[2] if len(sys.argv) > 2 else None
    by_class = {}
    for row in rates['observed']:
        for call in row['calls']:
            by_class.setdefault(call['operator'], []).append(
                dict(call, request=row['request']))

    document = {'referenceRate': REFERENCE_RATE, 'scale': SCALE, 'classes': {}}
    for operator in sorted(by_class):
        calls = by_class[operator]
        ranked, usable = candidates_for(calls)
        seconds = sum(c['elapsedSeconds'] for c in usable)
        units = sum(c['globalUnits'] for c in usable)
        entry = {
            'calls': len(calls),
            'usableCalls': len(usable),
            'wallSeconds': seconds,
            'globalUnits': units,
            'shippedUnitsPerSecond': (units / seconds) if seconds else None,
            # How far the shipped meter is from pricing this class at the
            # reference rate: 1.0 is "already comparable".
            'shippedMispricing': (
                (units / seconds) / REFERENCE_RATE if seconds else None),
            'candidates': ranked,
        }
        underpriced = (entry['shippedMispricing'] is not None
                       and entry['shippedMispricing']
                       < 1.0 / MISPRICE_THRESHOLD)
        entry['underpriced'] = underpriced
        if ranked and underpriced:
            best = ranked[0]
            weight = best['weight']
            entry['fit'] = {
                'count': best['count'],
                'perSecond': best['medianPerSecond'],
                'spread': best['spread'],
                'residualRms': best['residualRms'],
                'weight': weight,
                'scaledWeight': best['scaledWeight'],
                'runnerUp': ranked[1]['count'] if len(ranked) > 1 else None,
                'runnerUpResidualRms': (ranked[1]['residualRms']
                                        if len(ranked) > 1 else None),
            }
            # What the fitted weight would have charged each call, against the
            # honest target `wall * REFERENCE_RATE`. This is the residual table
            # §2 prints, and it is the only check on the fit that does not
            # reuse the statistic the fit minimised.
            entry['residuals'] = [{
                'request': call['request'],
                'seconds': call['elapsedSeconds'],
                'globalUnits': call['globalUnits'],
                'targetUnits': call['elapsedSeconds'] * REFERENCE_RATE,
                'fittedUnits': (call['counts'][best['count']]
                                * round(weight * SCALE)) // SCALE,
            } for call in usable]
            for row in entry['residuals']:
                row['ratio'] = (row['fittedUnits'] / row['targetUnits']
                                if row['targetUnits'] else None)
        document['classes'][operator] = entry

    text = json.dumps(document, indent=1, sort_keys=True)
    if out:
        with open(out, 'w') as handle:
            handle.write(text)
    for operator, entry in document['classes'].items():
        rate = entry['shippedUnitsPerSecond']
        if rate is None:
            print(f"{operator}: no call long enough to price "
                  f"({entry['calls']} calls, all under {MIN_SECONDS}s)")
            continue
        verdict = 'UNDER-PRICED' if entry['underpriced'] else 'comparable'
        print(f"{operator}: shipped {rate:,.0f} u/s "
              f"({entry['shippedMispricing']:.4g}x the reference rate) "
              f"- {verdict}")
        for row in entry['candidates']:
            print(f"     {row['count']:>24} rms {row['residualRms']:>6.3f} "
                  f"range {row['worstUndercharge']:>6.3f}-"
                  f"{row['worstOvercharge']:<6.3f} weight "
                  f"{row['scaledWeight']:>14,}")
        fit = entry.get('fit')
        if fit:
            print(f"   FIT: {fit['count']} at {fit['perSecond']:,.0f}/s, "
                  f"scaled weight {fit['scaledWeight']}")
            for row in entry['residuals']:
                print(f"      {row['request']:<12} {row['seconds']:>6.3f}s  "
                      f"target {row['targetUnits']:>12,.0f}  fitted "
                      f"{row['fittedUnits']:>12,}  ratio {row['ratio']:.3f}")
    return 0


if __name__ == '__main__':
    sys.exit(main())
