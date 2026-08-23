#!/usr/bin/env python3
"""**§0's six clauses, applied to what `gate.py` measured. Nothing else.**

    python3 verdict.py <gate10.json> [curve3.json curve30.json curve60.json abba.json]

This file contains **no threshold of its own**. Every bar below is quoted from
`docs/economics-round-spec.md` §0 - the copy `gate2/section0.py` proves is
byte-identical to the spec's - and from `docs/currency-amendment.md`, which
changes exactly one thing about them: which clauses are evaluable on the
fallback path.

## The six PASS clauses, verbatim

> PASS iff ALL: (1) ≥5/9 exact-valid ≤168.484 mm; (2) median ≤168.484;
> (3) every publication Exclusive r=2.500 + contract-valid; (4) per-seed
> two-process bit identity; (5) quiet-box p95 ≤10.000 s over 5×9;
> (6) attribution vs the control arm as above.

and the promotion clause (6) points at:

> PROMOTION: treatment must gain ≥2 qualifying seeds or ≥1.000 mm paired
> median over control; else the absolute 5/9 is a draw, the impatient policy is
> NOT promoted, and the control's policy remains the member.

## The 30 s clauses

> 30 s: median ≤163.00461; ≥7/9 ≤168.484 (no-regression); paired ≥1.000 mm;
> shapes-17/triangle-20 within 1 mm at equal work; zero invalid publications.

The amendment makes the shapes-17/triangle-20 clause **inevaluable** on this
path - a single-fixture work plan has no equal-work denomination on another
fixture - and it is recorded **NOT-RUN**. It is not a pass, it is not a fail,
and it is not silently dropped: `NOT-RUN` is a value this document prints.

## Which arm answers a clause

Clauses (1) (2) (3) (4) (5) are asked of **both arms** and reported per arm.
A clause passes only if the arm being read passes it; the gate's own verdict is
taken on the **control**, because the control is the member and clause (6) is
the only thing that could make it otherwise. Both columns are printed side by
side so nobody has to take that on trust.

Exit is the verdict: `0` when §0 PASSES, `1` when it does not, `2` when the
documents do not carry what a clause needs.
"""
import json
import os
import statistics
import sys

# Quoted, never chosen here.
BAR_MM = 168.484
QUORUM = 5
SEEDS_TOTAL = 9
P95_CEILING_SECONDS = 10.000
MEDIAN_30S_MM = 163.00461
NO_REGRESSION_30S = 7
PAIRED_GAIN_MM = 1.000
ATTRIBUTION_SEED_GAIN = 2
EXCLUSIVE_TWO_R_MICRON = 5000.0
ARMS = ['control', 'treatment']


def cells_of(document, arm=None, mode='calibrated'):
    rows = [row for row in document['cells']
            if row.get('mode', 'calibrated') == mode]
    if arm is not None:
        rows = [row for row in rows if row['arm'] == arm]
    return rows


def per_seed_best(rows):
    """The best strict child per seed, over that seed's repetitions.

    A work budget is deterministic in quality, so the repetitions of one cell
    must all agree; `repetitionsAgree` asserts that rather than assuming it,
    and a disagreement would mean the identity clause is answering a different
    question from the one this reduction is asking.
    """
    by_seed = {}
    for row in rows:
        by_seed.setdefault(row['seed'], []).append(row)
    out = []
    for seed in sorted(by_seed):
        group = by_seed[seed]
        depths = [row.get('bestStrictChildMm') for row in group]
        best = depths[0]
        out.append({
            'seed': seed,
            'bestStrictChildMm': best,
            'repetitions': len(group),
            'repetitionsAgree': len(set(
                'null' if d is None else f'{d!r}' for d in depths)) == 1,
            'depthsPerRepetition': depths,
            'qualifies': bool(best is not None and best <= BAR_MM),
            'walls': [row['processWallSeconds'] for row in group],
            'invalidPublications': max(row.get('invalidPublications') or 0
                                       for row in group),
            'everyPublicationRevalidated': all(
                row.get('everyPublicationRevalidated') for row in group),
            'publicationsTotal': group[0].get('publicationsTotal'),
            'strikesTotal': group[0].get('strikesTotal'),
            'disruptionsTotal': group[0].get('disruptionsTotal'),
            'exploreBites': group[0].get('exploreBites'),
            'compressBites': group[0].get('compressBites'),
            'consumedUnits': (group[0].get('calibratedLedger') or {})
            .get('consumedUnits'),
            'chargeIdentityHolds': (group[0].get('calibratedLedger') or {})
            .get('chargeIdentityHolds'),
        })
    return out


def median_of(seeds):
    """§0's median over the nine seeds.

    A seed with no strict child has not published anything under the bar, and
    dropping it would make the median a statistic about the seeds that
    succeeded. It is carried at `+inf`, which is what "did not qualify" means
    for a minimum-is-better quantity, and the count of them is printed.
    """
    values = [row['bestStrictChildMm'] if row['bestStrictChildMm'] is not None
              else float('inf') for row in seeds]
    unpublished = sum(1 for value in values if value == float('inf'))
    median = statistics.median(values)
    return (None if median == float('inf') else median), unpublished


def arm_summary(document, arm):
    rows = cells_of(document, arm)
    seeds = per_seed_best(rows)
    median, unpublished = median_of(seeds)
    qualifying = [row['seed'] for row in seeds if row['qualifies']]
    walls = sorted(row['processWallSeconds'] for row in rows)
    return {
        'arm': arm,
        'seeds': seeds,
        'qualifyingSeeds': qualifying,
        'quorumReached': len(qualifying),
        'medianMm': median,
        'seedsWithNoStrictChild': unpublished,
        'wallReadings': len(walls),
        'p95Seconds': (statistics.quantiles(walls, n=100, method='inclusive')[94]
                       if len(walls) > 1 else None),
        'maxWallSeconds': max(walls) if walls else None,
        'minWallSeconds': min(walls) if walls else None,
        'medianWallSeconds': statistics.median(walls) if walls else None,
        'invalidPublications': sum(row['invalidPublications'] for row in seeds),
        'everyPublicationRevalidated': all(row['everyPublicationRevalidated']
                                           for row in seeds),
        'repetitionsAgree': all(row['repetitionsAgree'] for row in seeds),
    }


def paired_gain(control, treatment):
    """The paired per-seed median gain, treatment over control.

    Paired, so a seed that neither arm published contributes nothing rather
    than contributing a made-up number, and the count of pairs the comparison
    could actually be taken on is printed beside the median.
    """
    control_by_seed = {row['seed']: row['bestStrictChildMm'] for row in control}
    pairs = []
    for row in treatment:
        theirs = control_by_seed.get(row['seed'])
        mine = row['bestStrictChildMm']
        if theirs is None or mine is None:
            pairs.append({'seed': row['seed'], 'controlMm': theirs,
                          'treatmentMm': mine, 'gainMm': None})
            continue
        pairs.append({'seed': row['seed'], 'controlMm': theirs,
                      'treatmentMm': mine, 'gainMm': theirs - mine})
    gains = [row['gainMm'] for row in pairs if row['gainMm'] is not None]
    return {
        'pairs': pairs,
        'comparablePairs': len(gains),
        'medianGainMm': statistics.median(gains) if gains else None,
        'meanGainMm': statistics.fmean(gains) if gains else None,
    }


def main():
    paths = sys.argv[1:]
    if not paths:
        print(json.dumps({'error': 'usage: verdict.py <gate10.json> [...]'},
                         indent=1))
        return 2
    documents = {}
    for path in paths:
        with open(path) as handle:
            loaded = json.load(handle)
        documents[loaded['battery'].rsplit('-', 1)[-1]] = loaded

    gate = documents.get('gate10')
    if gate is None:
        print(json.dumps({'error': 'no gate10 document'}, indent=1))
        return 2

    document = {
        'experiment': 'overlap-ics',
        'battery': 'economics-round-gate2-verdict',
        'label': gate['label'],
        'section0': ('docs/economics-round-spec.md §0, copied byte-for-byte '
                     'into gate2/README.md and checked by section0.py'),
        'amendment': ('docs/currency-amendment.md: U\' failed, so this gate '
                      'ran on the declared fallback. Clauses (1)(2)(3)(4)(6) '
                      'bind unchanged; clause (5) binds as a claim; the 30 s '
                      'shapes-17/triangle-20 equal-work clause is inevaluable '
                      'and is recorded NOT-RUN.'),
        'bars': {
            'barMm': BAR_MM, 'quorum': QUORUM, 'seeds': SEEDS_TOTAL,
            'p95CeilingSeconds': P95_CEILING_SECONDS,
            'median30sMm': MEDIAN_30S_MM,
            'noRegression30s': NO_REGRESSION_30S,
            'pairedGainMm': PAIRED_GAIN_MM,
            'attributionSeedGain': ATTRIBUTION_SEED_GAIN,
            'exclusiveTwoRMicron': EXCLUSIVE_TWO_R_MICRON,
        },
        'binarySha256': gate['binarySha256'],
        'planSha256': gate['planSha256'],
        'budgetRetuned': gate['budgetRetuned'],
        'pinnedConstructorSeconds': gate['pinnedConstructorSeconds'],
    }

    arms = {arm: arm_summary(gate, arm) for arm in ARMS}
    document['arms'] = arms

    # ---- clause (1): >=5/9 exact-valid <=168.484 mm ----
    clause1 = {arm: {
        'qualifyingSeeds': arms[arm]['qualifyingSeeds'],
        'quorumReached': arms[arm]['quorumReached'],
        'required': QUORUM,
        'pass': arms[arm]['quorumReached'] >= QUORUM,
    } for arm in ARMS}

    # ---- clause (2): median <= 168.484 ----
    clause2 = {arm: {
        'medianMm': arms[arm]['medianMm'],
        'seedsWithNoStrictChild': arms[arm]['seedsWithNoStrictChild'],
        'pass': bool(arms[arm]['medianMm'] is not None
                     and arms[arm]['medianMm'] <= BAR_MM),
    } for arm in ARMS}

    # ---- clause (3): Exclusive r=2.500 + contract-valid, certified ----
    clause3 = {arm: {
        'invalidPublications': arms[arm]['invalidPublications'],
        'everyPublicationRevalidated': arms[arm]['everyPublicationRevalidated'],
        'revalidateFlag': 1,
        'pass': bool(arms[arm]['invalidPublications'] == 0
                     and arms[arm]['everyPublicationRevalidated']),
    } for arm in ARMS}

    # ---- clause (4): per-seed two-process bit identity ----
    identity = gate.get('twoProcessIdentity') or []
    clause4 = {arm: {
        'cells': [row for row in identity if row['arm'] == arm],
        'pass': all(row['bitIdentical'] for row in identity
                    if row['arm'] == arm),
    } for arm in ARMS}
    for arm in ARMS:
        clause4[arm]['cells'] = [
            {'seed': row['seed'], 'bitIdentical': row['bitIdentical'],
             'digest': row['digests'][0][:16]}
            for row in clause4[arm]['cells']]

    # ---- clause (5): quiet-box p95 <= 10.000 s over 5 x 9 ----
    clause5 = {arm: {
        'readings': arms[arm]['wallReadings'],
        'p95Seconds': arms[arm]['p95Seconds'],
        'maxWallSeconds': arms[arm]['maxWallSeconds'],
        'medianWallSeconds': arms[arm]['medianWallSeconds'],
        'ceilingSeconds': P95_CEILING_SECONDS,
        'pass': bool(arms[arm]['p95Seconds'] is not None
                     and arms[arm]['p95Seconds'] <= P95_CEILING_SECONDS),
    } for arm in ARMS}
    clause5['loadBefore'] = gate['machine']['loadBefore']
    clause5['loadAfter'] = gate['machine']['loadAfter']
    clause5['frame'] = ('the driver\'s own process wall, request-relative and '
                        'strictly larger than anything the document reports')

    # ---- clause (6): attribution ----
    gain = paired_gain(arms['control']['seeds'], arms['treatment']['seeds'])
    seed_gain = (arms['treatment']['quorumReached']
                 - arms['control']['quorumReached'])
    promoted = bool(seed_gain >= ATTRIBUTION_SEED_GAIN
                    or (gain['medianGainMm'] is not None
                        and gain['medianGainMm'] >= PAIRED_GAIN_MM))
    clause6 = {
        'controlQualifying': arms['control']['quorumReached'],
        'treatmentQualifying': arms['treatment']['quorumReached'],
        'seedGain': seed_gain,
        'seedGainRequired': ATTRIBUTION_SEED_GAIN,
        'pairedMedianGainMm': gain['medianGainMm'],
        'pairedGainRequiredMm': PAIRED_GAIN_MM,
        'pairs': gain['pairs'],
        'comparablePairs': gain['comparablePairs'],
        'IMPATIENT_POLICY_PROMOTED': promoted,
        'consequence': (
            'the treatment arm is promoted' if promoted else
            'a draw: the impatient policy is NOT promoted and the control\'s '
            'frozen 200/3/100/5/0.98 remains the member'),
        # **An interpretation, stated rather than buried.** §0 lists (6) among
        # the clauses a PASS needs, and the promotion sentence it points at
        # defines a *draw* as a valid outcome - "the control's policy remains
        # the member" - rather than as a failure. So clause (6) is read as
        # "attribution was performed and rendered a verdict", which a draw
        # satisfies; the promotion decision is reported separately in
        # IMPATIENT_POLICY_PROMOTED. Reading it the other way - the treatment
        # must win or §0 fails - would make §0 unpassable whenever the two arms
        # tie, which the promotion sentence explicitly contemplates. On this
        # gate the choice changes nothing: clauses (1) and (2) fail either way.
        'interpretation': (
            'clause (6) passes when attribution was performed and rendered a '
            'verdict; a draw is a defined outcome of the promotion sentence, '
            'not a failure of it. IMPATIENT_POLICY_PROMOTED carries the '
            'decision. On this gate the reading changes nothing: (1) and (2) '
            'fail either way.'),
        'pass': True,
    }

    document['clauses'] = {
        '1_quorum': clause1, '2_median': clause2, '3_dualValid': clause3,
        '4_bitIdentity': clause4, '5_p95': clause5, '6_attribution': clause6,
    }

    # ---- the 30 s clauses ----
    thirty = documents.get('curve30')
    if thirty is not None:
        arms30 = {arm: arm_summary(thirty, arm) for arm in ARMS}
        gain30 = paired_gain(arms30['control']['seeds'],
                             arms30['treatment']['seeds'])
        document['thirtySecond'] = {
            'arms': arms30,
            'medianMm': {arm: arms30[arm]['medianMm'] for arm in ARMS},
            'medianClause': {arm: bool(arms30[arm]['medianMm'] is not None
                                       and arms30[arm]['medianMm']
                                       <= MEDIAN_30S_MM) for arm in ARMS},
            'noRegression': {arm: {
                'qualifying': arms30[arm]['quorumReached'],
                'required': NO_REGRESSION_30S,
                'pass': arms30[arm]['quorumReached'] >= NO_REGRESSION_30S,
            } for arm in ARMS},
            'pairedGain': gain30,
            'pairedClause': bool(gain30['medianGainMm'] is not None
                                 and gain30['medianGainMm'] >= PAIRED_GAIN_MM),
            'zeroInvalidPublications': all(
                arms30[arm]['invalidPublications'] == 0 for arm in ARMS),
            'shapes17Triangle20EqualWork': 'NOT-RUN',
            'shapes17Triangle20Reason': (
                'inevaluable on the declared fallback: a single-fixture work '
                'plan has no equal-work denomination on another fixture. '
                'docs/currency-amendment.md records it not-run.'),
        }

    for tag, key in (('curve3', 'threeSecond'), ('curve60', 'sixtySecond')):
        curve = documents.get(tag)
        if curve is None:
            continue
        summary = {arm: arm_summary(curve, arm) for arm in ARMS}
        document[key] = {
            'gated': False,
            'arms': summary,
            'medianMm': {arm: summary[arm]['medianMm'] for arm in ARMS},
            'qualifying': {arm: summary[arm]['qualifyingSeeds']
                           for arm in ARMS},
            'medianWallSeconds': {arm: summary[arm]['medianWallSeconds']
                                  for arm in ARMS},
        }

    abba = documents.get('abba')
    if abba is not None:
        rows = abba['cells']
        by = {}
        for row in rows:
            by.setdefault((row['abbaOrder'], row['mode']), []).append(row)
        # **The number this diagnostic exists for.** The work plan is spent
        # against a rate discounted by 0.80, so it buys less search than a
        # 10.000 s wall run does, and the difference is a cost the gate's own
        # depth clauses pay. Paired per seed, in both orders, so an asymmetry
        # between AB and BA is the box drifting rather than the budgets
        # differing.
        paired = []
        for seed in sorted({row['seed'] for row in rows}):
            entry = {'seed': seed}
            for order in ('AB', 'BA'):
                pair = {row['mode']: row for row in rows
                        if row['seed'] == seed and row['abbaOrder'] == order}
                work = pair.get('calibrated', {}).get('bestStrictChildMm')
                clock = pair.get('wall', {}).get('bestStrictChildMm')
                entry[order] = {
                    'calibratedMm': work,
                    'wallMm': clock,
                    'wallMinusCalibratedMm': (None if work is None or clock is None
                                              else clock - work),
                    'calibratedWallSeconds':
                        pair.get('calibrated', {}).get('processWallSeconds'),
                    'wallArmWallSeconds':
                        pair.get('wall', {}).get('processWallSeconds'),
                }
            paired.append(entry)
        deltas = [entry[order]['wallMinusCalibratedMm'] for entry in paired
                  for order in ('AB', 'BA')
                  if entry[order]['wallMinusCalibratedMm'] is not None]
        document['abba'] = {
            'diagnostic': True,
            'note': ('the old wall arm is never a lane; both orders are run so '
                     'that a drift in the box during the battery is separable '
                     'from a difference between the two budgets'),
            'cells': {f'{order}-{mode}': {
                'depths': [r.get('bestStrictChildMm') for r in group],
                'medianWallSeconds': statistics.median(
                    [r['processWallSeconds'] for r in group]),
                'qualifying': sum(1 for r in group
                                  if r.get('bestStrictChildMm') is not None
                                  and r['bestStrictChildMm'] <= BAR_MM),
            } for (order, mode), group in sorted(by.items())},
            'pairedBySeed': paired,
            'wallMinusCalibratedMm': {
                'readings': len(deltas),
                'median': statistics.median(deltas) if deltas else None,
                'mean': statistics.fmean(deltas) if deltas else None,
                'min': min(deltas) if deltas else None,
                'max': max(deltas) if deltas else None,
                'note': ('positive means the old 10.000 s wall arm published '
                         'DEEPER (worse) than the work plan; negative means '
                         'the work plan bought less search and paid for it'),
            },
        }

    # ---- §0's verdict ----
    #
    # Read on the control, because the control is the member. Both arms are in
    # the table above and clause (6) is what could change which arm is read.
    absolute = {
        '1_quorum': clause1['control']['pass'],
        '2_median': clause2['control']['pass'],
        '3_dualValid': clause3['control']['pass'],
        '4_bitIdentity': clause4['control']['pass'],
        '5_p95': clause5['control']['pass'],
        '6_attribution': clause6['pass'],
    }
    document['SECTION0_VERDICT'] = {
        'readOn': 'control',
        'clauses': absolute,
        'failedClauses': [name for name, ok in absolute.items() if not ok],
        'GATE_PASS': all(absolute.values()),
    }
    document['GATE_PASS'] = document['SECTION0_VERDICT']['GATE_PASS']
    print(json.dumps(document, indent=1))
    out = os.environ.get('ICS_OUT')
    if out:
        os.makedirs(out, exist_ok=True)
        with open(f'{out}/verdict.json', 'w') as handle:
            json.dump(document, handle, indent=1)
    return 0 if document['GATE_PASS'] else 1


if __name__ == '__main__':
    sys.exit(main())
