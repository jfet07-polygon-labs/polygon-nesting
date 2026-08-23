#!/usr/bin/env python3
"""**The two-arm gate, on the declared fallback's budget.**

    python3 gate.py gate10     # the 10 s battery: clauses (1)(2)(3)(4)(5)
    python3 gate.py curve30    # the 30 s clauses
    python3 gate.py curve60    # reported, never gated
    python3 gate.py curve3     # the curve, never gated
    python3 gate.py abba       # the interleaved old-wall-arm control

Bare mixed-61, seeds 0..=8, workers = 8, `--revalidate=1`, on a quiet box.
Both arms of the strike experiment on every cell of every battery: **control**
is the frozen `200/3/100/5/0.98`, **treatment** is the work-quanta impatient
policy at the frozen KNOB. Strike semantics are the only delta between them.

Every cell spends the **same work budget**, read from
`evidence/gate2.icscal.json` - the single-fixture shelf-probed plan
`budget.py` wrote before any of these cells existed. `--mode=calibrated`
constructs no `Instant` anywhere, so a cell cannot acquire a fresh rate, and
the plan's key pins the request, the currency, this binary's own sha256, the
features, `workers = 8` and the executor. **A key that does not match is a hard
error and never a fallback.**

# The frame, restated where the seconds are measured

The budget handed to the pacer is `budgetSeconds - pinnedConstructorSeconds`,
computed once in `budget.py` and read here rather than recomputed. Clause (5)
is checked against the **process wall** - the driver's own bracket around the
process, which is request-relative and strictly larger than anything the
document reports - so the p95 this file prints cannot be flattered by choosing
a frame.

# What each battery answers

`gate10` runs **five repetitions** of every (seed, arm) cell. The first two are
the two processes clause (4)'s bit identity is taken from; all five are wall
readings, and clause (5)'s `p95` is over the 5 x 9 of them per arm. One battery,
because the second process of a bit-identity pair is a wall reading whether or
not anyone reads it, and pretending otherwise would spend eighteen cells to
learn nothing.

`abba` is a **diagnostic and never a lane**. It interleaves the calibrated cell
with a `--mode=wall` cell of the same budget in both orders, A-B and B-A, on
every seed, so that a difference between the two can be told apart from the
box drifting during the battery. The old wall arm is not a member of anything
here; it is the thing the work plan is being compared against.

Exit is the verdict: `0` when every cell in the battery ran, `2` when one did
not. **Nothing in this file decides a clause.** `verdict.py` applies §0 to what
this measures, and it contains no threshold of its own either - it quotes.
"""
import hashlib
import json
import os
import statistics
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, '..', '..', '..', '..', '..'))
sys.path.insert(0, f'{ROOT}/docs/experiments/overlap-ics/drivers')
import lib  # noqa: E402

LABEL = 'single-fixture work plan, no transfer claim'
PLAN = os.environ.get('ICS_GATE2_PLAN', f'{HERE}/evidence/gate2.icscal.json')
BUDGET_DOC = os.environ.get('ICS_GATE2_BUDGET', f'{HERE}/evidence/budget.json')
FIXTURE = 'mixed-61'
SEEDS = list(range(9))
ARMS = ['control', 'treatment']
WORKERS = 8
# §0's own budgets. `10` is the gate; the other three are reported.
BUDGETS = {'3': 3.000, '10': 10.000, '30': 30.000, '60': 60.000}
# Clause (5) is a p95 over 5 x 9. The first two repetitions of each cell are
# also clause (4)'s two processes.
REPETITIONS = 5
IDENTITY_PROCESSES = 2


def plan_budget():
    """The search-relative budget for each of §0's four wall budgets.

    Read from `budget.py`'s document, never recomputed: the constructor time is
    pinned and the whole point of pinning it is that no later process gets to
    choose it.
    """
    with open(BUDGET_DOC) as handle:
        document = json.load(handle)
    table = document['budget']['searchBudgetSecondsByBudget']
    return {tag: table[f'{seconds:.1f}'] for tag, seconds in BUDGETS.items()}, document


def sha256_of(path):
    with open(path, 'rb') as handle:
        return hashlib.sha256(handle.read()).hexdigest()


def loadavg():
    try:
        with open('/proc/loadavg') as handle:
            return handle.read().split()[:3]
    except OSError:
        return None


def cell(out, tag, seed, arm, budget, search_seconds, mode='calibrated'):
    """One process. One JSON document. One row."""
    path = f'{out}/{tag}.json'
    options = dict(workers=WORKERS, seed=seed, arm=arm, revalidate=1)
    if mode == 'calibrated':
        options.update(mode='calibrated', plan=PLAN, currency='U0',
                       wall=search_seconds)
    else:
        # The old wall arm, at the same §0 budget in its own frame.
        options.update(mode='wall', wall=BUDGETS[budget])
    document, wall, status, stderr = lib.run(
        'cutclose', FIXTURE, path, **options)
    row = {
        'tag': tag,
        'seed': seed,
        'arm': arm,
        'budget': budget,
        'budgetSeconds': BUDGETS[budget],
        'mode': mode,
        'searchBudgetSeconds': search_seconds if mode == 'calibrated' else None,
        'exit': status,
        'processWallSeconds': wall,
        'sourcePath': path,
        'sourceSha256': lib.source_sha256(path),
    }
    if status != 0:
        row.update(valid=False, qualifies=False, stderr=stderr[-800:])
        return row
    outcome = document.get('outcome') or {}
    constructor = document.get('constructor') or {}
    wall_object = document.get('wall') or {}
    publications = outcome.get('publications') or []
    limit = BUDGETS[budget]
    within, late, undecided = lib.within_budget(publications, document, limit)
    strict = [p for p in within
              if p['placementFingerprint']
              != constructor.get('placementFingerprint')]
    best = min((p['publishedRawDepthMm'] for p in strict), default=None)
    lower_offset, _ = lib.checkpoint_frame(document)
    # Clause (3), per cell: every publication revalidated by the untouched
    # contract validator, bitwise, and none invalid.
    revalidations = [p.get('revalidation') for p in publications]
    all_revalidated = bool(revalidations) and all(
        row_ and row_.get('depthMatchesBitwise') and row_.get('fingerprintMatches')
        for row_ in revalidations)
    ledger = outcome.get('calibrated') or {}
    strikes = [(b.get('strikeMeter') or {}) for b in (outcome.get('bites') or [])]
    row.update({
        'valid': True,
        'constructorDepthMm': constructor.get('rawSourceDepthMm'),
        'constructorSeconds': wall_object.get('constructorSeconds'),
        'searchSeconds': wall_object.get('searchSeconds'),
        'totalSeconds': wall_object.get('totalSeconds'),
        'bestStrictChildMm': best,
        'incumbentMm': (outcome.get('incumbent') or {}).get('rawSourceDepthMm'),
        'incumbentIsConstructor': (outcome.get('incumbent') or {})
        .get('fromConstructor'),
        'depthMm': outcome.get('depthMm'),
        'finalWidthMm': outcome.get('finalWidthMm'),
        'exploreBites': outcome.get('exploreBites'),
        'compressBites': outcome.get('compressBites'),
        'publicationsTotal': len(publications),
        'publicationsWithinBudget': len(within),
        'strictChildren': len(strict),
        'invalidPublications': outcome.get('invalidPublications'),
        'everyPublicationRevalidated': all_revalidated,
        'repairMaxUm': (outcome.get('repairMaxDisplacementMm') or 0.0) * 1000.0,
        'repairMaxGivebackMm': outcome.get('repairMaxGivebackMm'),
        'funnel': outcome.get('funnel'),
        'strikeArm': outcome.get('strikeArm'),
        # **The corrected clock frame.** On a calibrated cell no publication
        # carries a clock reading at all - the trajectory constructs no
        # `Instant` - so the filter is a no-op by construction and says so
        # rather than looking like a filter that decided something.
        'checkpointFrame': {
            'publicationsExcludedAsLate': len(late),
            'publicationsUndecidedByFrame': len(undecided),
            'loopRelativeMaxSeconds': max(
                (p['wallSeconds'] for p in publications
                 if p.get('wallSeconds') is not None), default=None),
            'bestStrictChildRequestSecondsLower': min(
                (lib.request_seconds(p, lower_offset) for p in strict
                 if p['publishedRawDepthMm'] == best
                 and lib.request_seconds(p, lower_offset) is not None),
                default=None),
            'note': ('a calibrated trajectory reads no clock, so publications '
                     'carry no wallSeconds and nothing can be excluded as '
                     'late; the filter is reported so its no-op is visible'
                     if mode == 'calibrated' else
                     'the wall arm carries per-publication clock readings and '
                     'the request-relative filter applies'),
        },
        'calibratedLedger': {
            key: ledger.get(key) for key in
            ('budgetSeconds', 'currencyVersion', 'exploreAllocationUnits',
             'compressAllocationUnits', 'exploreConsumedUnits',
             'compressConsumedUnits', 'consumedUnits', 'exploreBatches',
             'compressBatches', 'exploreCrossingBatchUnits',
             'compressCrossingBatchUnits', 'chargeIdentityHolds',
             'consumedUnitsMatchCharged')} if ledger else None,
        'strikeMeter': {
            'batches': sum(row_.get('batches') or 0 for row_ in strikes),
            'none': sum(row_.get('none') or 0 for row_ in strikes),
            'marginal': sum(row_.get('marginal') or 0 for row_ in strikes),
            'substantial': sum(row_.get('substantial') or 0 for row_ in strikes),
            'strikeAccumulated': sum(row_.get('strikeAccumulated') or 0
                                     for row_ in strikes),
            'strikeOvershoot': sum(row_.get('strikeOvershoot') or 0
                                   for row_ in strikes),
        },
        'strikesTotal': sum(b.get('strikes') or 0
                            for b in (outcome.get('bites') or [])),
        'disruptionsTotal': sum(b.get('disruptions') or 0
                                for b in (outcome.get('bites') or [])),
        'finalPoseDigest': document.get('finalPoseDigest'),
        # RV2: poses travel with publications. The raw documents carry them per
        # publication; the reduction carries the digest and the count so the
        # binding is a field rather than a re-derivation.
        'publicationsCarryPoses': all('poses' in p for p in publications),
        'bites': outcome.get('bites'),
    })
    return row


def identity_digest(path):
    """The document with only the clock stripped, as a digest.

    `--mode=calibrated` constructs no `Instant` inside the trajectory, so two
    processes of one cell may differ in `wall` and nowhere else.
    """
    with open(path) as handle:
        document = json.load(handle)
    for field in lib.WALL_FIELDS:
        document.pop(field, None)
    return hashlib.sha256(
        json.dumps(document, sort_keys=True, separators=(',', ':')).encode()
    ).hexdigest()


def battery(stage, out, budgets, budget_document):
    rows = []
    started = time.monotonic()
    if stage == 'gate10':
        for repetition in range(REPETITIONS):
            for seed in SEEDS:
                for arm in ARMS:
                    tag = f'gate10-{arm}-seed{seed}-r{repetition}'
                    rows.append(cell(out, tag, seed, arm, '10',
                                     budgets['10']))
                    print(f'[gate10] r{repetition} seed{seed} {arm} '
                          f'depth={rows[-1].get("bestStrictChildMm")} '
                          f'wall={rows[-1]["processWallSeconds"]:.3f}',
                          file=sys.stderr)
    elif stage in ('curve3', 'curve30', 'curve60'):
        budget = stage.replace('curve', '')
        for seed in SEEDS:
            for arm in ARMS:
                tag = f'curve{budget}-{arm}-seed{seed}'
                rows.append(cell(out, tag, seed, arm, budget, budgets[budget]))
                print(f'[{stage}] seed{seed} {arm} '
                      f'depth={rows[-1].get("bestStrictChildMm")} '
                      f'wall={rows[-1]["processWallSeconds"]:.3f}',
                      file=sys.stderr)
    elif stage == 'abba':
        # Interleaved, both orders, one seed at a time: A is the work plan, B
        # is the old wall arm, and a drift in the box shows up as an AB/BA
        # asymmetry rather than as a difference between the arms.
        for seed in SEEDS:
            for order in ('AB', 'BA'):
                sequence = (('calibrated', 'wall') if order == 'AB'
                            else ('wall', 'calibrated'))
                for position, mode in enumerate(sequence):
                    tag = f'abba-{order}{position}-{mode}-seed{seed}'
                    row = cell(out, tag, seed, 'control', '10', budgets['10'],
                               mode=mode)
                    row['abbaOrder'] = order
                    row['abbaPosition'] = position
                    rows.append(row)
                print(f'[abba] seed{seed} {order} '
                      f'{[r.get("bestStrictChildMm") for r in rows[-2:]]}',
                      file=sys.stderr)
    else:
        raise SystemExit(f'unknown stage `{stage}`')
    return rows, time.monotonic() - started


def main():
    stage = sys.argv[1] if len(sys.argv) > 1 else 'gate10'
    out = (sys.argv[2] if len(sys.argv) > 2
           else f'/var/lib/t3/tmp/overlapics/gate2/{stage}')
    os.makedirs(out, exist_ok=True)
    budgets, budget_document = plan_budget()
    document = {
        'experiment': 'overlap-ics',
        'battery': f'economics-round-gate2-{stage}',
        'label': LABEL,
        'fixture': FIXTURE,
        'seeds': SEEDS,
        'arms': ARMS,
        'workers': WORKERS,
        'binary': lib.BIN,
        'binarySha256': sha256_of(lib.BIN),
        'plan': PLAN,
        'planSha256': sha256_of(PLAN),
        'planSummary': budget_document['plan']['phases'],
        'searchBudgetSeconds': budgets,
        'pinnedConstructorSeconds':
            budget_document['budget']['pinnedConstructorSeconds'],
        'budgetRetuned': budget_document['budget']['retuned'],
        'machine': {'cpus': os.cpu_count(), 'loadBefore': loadavg()},
    }
    rows, seconds = battery(stage, out, budgets, budget_document)
    document['cells'] = rows
    document['batterySeconds'] = seconds
    document['machine']['loadAfter'] = loadavg()
    document['cellSources'] = lib.MANIFEST
    document['binarySha256After'] = sha256_of(lib.BIN)
    document['binaryUnchangedDuringBattery'] = (
        document['binarySha256'] == document['binarySha256After'])

    # Clause (4): the first two repetitions of each 10 s cell, bit for bit.
    if stage == 'gate10':
        identity = []
        for seed in SEEDS:
            for arm in ARMS:
                digests = [identity_digest(f'{out}/gate10-{arm}-seed{seed}-r{r}.json')
                           for r in range(IDENTITY_PROCESSES)]
                identity.append({
                    'seed': seed, 'arm': arm, 'processes': IDENTITY_PROCESSES,
                    'digests': digests,
                    'bitIdentical': len(set(digests)) == 1,
                })
        document['twoProcessIdentity'] = identity
        document['ALL_BIT_IDENTICAL'] = all(row['bitIdentical']
                                            for row in identity)
        walls = {arm: sorted(row['processWallSeconds'] for row in rows
                             if row['arm'] == arm) for arm in ARMS}
        document['wallReadings'] = walls
        document['p95'] = {
            arm: statistics.quantiles(values, n=100, method='inclusive')[94]
            for arm, values in walls.items() if len(values) > 1}
        pooled = sorted(row['processWallSeconds'] for row in rows)
        document['p95']['pooled'] = statistics.quantiles(
            pooled, n=100, method='inclusive')[94]
        document['maxWallSeconds'] = {arm: max(values)
                                      for arm, values in walls.items()}

    print(json.dumps({
        'battery': document['battery'],
        'cells': len(rows),
        'exits': sorted({row['exit'] for row in rows}),
        'batterySeconds': round(seconds, 1),
        'p95': document.get('p95'),
        'ALL_BIT_IDENTICAL': document.get('ALL_BIT_IDENTICAL'),
        'binaryUnchangedDuringBattery': document['binaryUnchangedDuringBattery'],
    }, indent=1))
    env_out = os.environ.get('ICS_OUT')
    if env_out:
        os.makedirs(env_out, exist_ok=True)
        with open(f'{env_out}/{stage}.json', 'w') as handle:
            json.dump(document, handle, indent=1)
    return 0 if all(row['exit'] == 0 for row in rows) else 2


if __name__ == '__main__':
    sys.exit(main())
