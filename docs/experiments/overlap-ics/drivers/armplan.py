#!/usr/bin/env python3
"""The economics round's integration stages: the two arms, and the plan.

    python3 armplan.py            # every stage
    python3 armplan.py plan       # one stage

Exits non-zero when any stage fails. Nothing here reads a clock inside a gated
trajectory: the `arms` and `batches` stages are fixed-work, and the `plan`
stage's spending runs are `--mode=calibrated`, which constructs no `Instant` at
all. The one stage that *does* need a clock is the calibration itself - a rate
is a statement about seconds - and it is a different process from every run
that spends its output, which is exactly the separation the spec asks for.

The stages, and which clause of the spec of record each one is:

  `arms`     Funded change 1: "CONTROL: the frozen literals 200/3/100/5/0.98 on
             the identical executor and pacer - strike semantics are the only
             delta between arms." Both arms are run on one fixed-work cell and
             the documents are compared field by field: everything outside the
             strike policy and the strike meter must be identical, because
             nothing outside the strike policy differs between the arms. The
             *default* invocation - no `--arm` at all - must be the control, or
             every committed cell in the campaign has silently changed arm.

             The cross-binary half of this claim, against the round's base
             binary, is `economics-round/integration/armgate.py`. This stage is
             the half that runs on every FAST tier.

  `plan`     Funded change 3: "read/write separate; no live probe on a gated
             trajectory; 80/20 by calibrated units; compress decay by consumed
             compress-work; stop only between master batches", and the FAST
             union's "calibrated-plan hit/miss/version/clock-poison".

             One wall process calibrates and writes an `icscal/v1` file. Two
             later processes spend it and must agree bit for bit. Four tampered
             copies of the same file - a different request, a different worker
             count, a different binary, a different currency - must each be
             REFUSED with the field named, and the refusal must be an exit
             status rather than a fallback: a plan that missed and then measured
             a fresh rate would be the live probe the clause forbids.

  `batches`  The FAST union's K = 1,024 identity, on the executor that exists.
             The spec asks for "K=1,024 ephemeral/persistent identity (incl.
             strike, pool restore, disruption)"; the persistent executor was
             refused by its own measured gate (census README: 5.082 % against a
             10.000 % bar), so there is no second executor to compare against
             and this is the half that remains: **at least 1,024 master batches,
             two processes, bit-identical, with strikes, pool restores and
             disruptions all having really happened in the cell.** The three
             preconditions are asserted, not hoped for - a K=1,024 identity on a
             cell that never struck would be 1,024 batches of the wrong thing.

             The batch-two-delta accounting the spec names beside it is the
             `chargeIdentityHolds` field of the `plan` stage: `charged +
             unchargedTail == trajectory`, term by term, on a cell that really
             spent a plan.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

# Fixed work, one attempt, a generous iteration cap: the two arms are compared
# on a cell whose *shape* neither of them can change.
ARM_CELL = dict(mode='fixed', bites=6, attempts=2, iters=300, compressbites=2,
                workers=8, seed=0)

# **The K >= 1,024 cell**, and every number in it is load-bearing.
#
#   `bites=22`    the 22nd bite IS the 179 shelf, and the shelf is the only
#                 place a separation stalls long enough to strike. A cell that
#                 stopped at 21 would be 1,024 batches of the cheap prefix.
#   `iters=800`   the control's patience is 200 no-improvement batches, so a
#                 cap below ~200 makes a strike unreachable by construction and
#                 the "incl. strike" half of the clause vacuous. 800 leaves
#                 room for the ladder to move twice.
#   `attempts=2`  a second attempt at one width is a pool restore followed by a
#                 disruption - `install_poses`, `restore_weights`, `disrupt` -
#                 which is the "incl. pool restore, disruption" half.
#   `seed=1`      one of the spec's strike-starved shelf seeds {0,1,4,5}. Seed
#                 5 is the watch seed and needs continued search rather than a
#                 strike, so it is the wrong seed for this cell.
#
# About eleven seconds per process on the census's box, and it runs twice.
BATCH_CELL = dict(mode='fixed', bites=22, attempts=2, iters=800,
                  compressbites=1, workers=8, seed=1)

# The calibration cell: wall mode, because a rate is a statement about seconds.
# Short, because FAST is minutes: the plan it writes is a real measurement of
# this binary on this box, which is all the spending runs need it to be.
CALIBRATE = dict(mode='wall', wall=3.0, workers=8, seed=0)
# And the spending cell. One second of calibrated work, 80/20 in units.
SPEND = dict(mode='calibrated', wall=1.0, workers=8, seed=0)

# Everything a strike arm is allowed to change. The `arms` stage requires the
# two documents to be identical outside these, which is the spec's "strike
# semantics are the only delta" as a diff rather than as a sentence.
ARM_KEYS = {
    'arm', 'armLabel', 'strikePolicy', 'strikeMeter', 'strikeArm',
    'exploreIterationsWithoutImprovement', 'exploreStrikes',
    'compressIterationsWithoutImprovement', 'compressStrikes',
}


def differences(left, right, path='', ignore=frozenset()):
    """Every leaf at which two documents disagree, ignoring `ignore` keys."""
    out = []
    if isinstance(left, dict):
        if not isinstance(right, dict):
            return [f'{path}: object vs {type(right).__name__}']
        for key in sorted(set(left) | set(right)):
            if key in ignore:
                continue
            here = f'{path}.{key}' if path else key
            if key not in left or key not in right:
                out.append(f'{here}: present in only one document')
                continue
            out.extend(differences(left[key], right[key], here, ignore))
        return out
    if isinstance(left, list):
        if not isinstance(right, list) or len(left) != len(right):
            return [f'{path}: {left!r} vs {right!r}'[:200]]
        for index, value in enumerate(left):
            out.extend(differences(value, right[index], f'{path}[{index}]', ignore))
        return out
    if repr(left) != repr(right):
        out.append(f'{path}: {left!r} vs {right!r}'[:200])
    return out


def arms(out):
    """Both arms on one cell, and everything outside the arm identical."""
    control, _, control_exit, control_err = lib.run(
        'cutclose', 'mixed-61', f'{out}/arm-default.json', **ARM_CELL)
    named, _, named_exit, named_err = lib.run(
        'cutclose', 'mixed-61', f'{out}/arm-control.json',
        arm='control', **ARM_CELL)
    treatment, _, treatment_exit, treatment_err = lib.run(
        'cutclose', 'mixed-61', f'{out}/arm-treatment.json',
        arm='treatment', **ARM_CELL)
    if control_exit or named_exit or treatment_exit:
        return {'stage': 'arms', 'pass': False,
                'exits': [control_exit, named_exit, treatment_exit],
                'stderr': (control_err or named_err or treatment_err)[-800:]}
    schedule = control.get('schedule', {})
    treatment_schedule = treatment.get('schedule', {})
    # The default invocation names no arm. If that is not the control, every
    # committed cell in the campaign changed arm without anyone saying so.
    default_is_control = schedule.get('armLabel') == 'control-iteration-strikes'
    naming_it_changes_nothing = not differences(
        lib.stripped(control), lib.stripped(named))
    # The arms may differ only where an arm is allowed to differ.
    #
    # **This is a cell-conditional check and the condition is asserted, not
    # assumed.** `ARM_CELL`'s 300-iteration cap is below the control's
    # 200-batch patience times its 3 strikes, and far below the treatment's
    # 1_630_000-evaluation quantum, so on this cell *neither* arm's patience is
    # ever spent - and the spec's "strike semantics are the only delta" then
    # says the two trajectories must coincide exactly. `strikesFired` below is
    # that precondition: if a future edit made either arm strike here, this
    # stage would go red for a legitimate reason, and the field says which.
    #
    # It follows that this stage does NOT prove the treatment changes anything.
    # It proves the *policy* is the work-denominated one and that nothing
    # outside it moved. That the arm reaches the trajectory at all is
    # `search::overlap_ics::tests::both_strike_arms_are_reachable_and_the_arm_
    # reaches_the_trajectory`, on a quantum sized to fire.
    outside_the_arm = differences(control, treatment,
                                 ignore=ARM_KEYS | {'wall'})
    strikes_fired = sum(
        row.get('strikes', 0)
        for document in (control, treatment)
        for row in (document.get('outcome', {}) or {}).get('bites', []))
    policy = treatment_schedule.get('strikePolicy', {})
    return {
        'stage': 'arms',
        'options': ARM_CELL,
        'defaultArmLabel': schedule.get('armLabel'),
        'defaultArmIsControl': default_is_control,
        'namingTheControlChangesNothing': naming_it_changes_nothing,
        'treatmentArmLabel': treatment_schedule.get('armLabel'),
        'treatmentExplorePatience': policy.get('explore', {}).get('patience'),
        'treatmentExploreQuantum':
            policy.get('explore', {}).get('workQuantumSampleEvaluations'),
        'treatmentCompressQuantum':
            policy.get('compress', {}).get('workQuantumSampleEvaluations'),
        'frozenLiteralsIntact':
            schedule.get('strikePolicy', {}).get('frozenLiteralsIntact'),
        'differencesOutsideTheArm': outside_the_arm[:20],
        'differenceCountOutsideTheArm': len(outside_the_arm),
        # The precondition of the comparison above, stated as a number.
        'strikesFiredOnEitherArm': strikes_fired,
        'controlStrikePolicy': schedule.get('strikePolicy'),
        'sourceSha256': lib.source_sha256(f'{out}/arm-treatment.json'),
        'pass': bool(
            default_is_control and naming_it_changes_nothing
            and not outside_the_arm and strikes_fired == 0
            and schedule.get('strikePolicy', {}).get('frozenLiteralsIntact')
            and treatment_schedule.get('armLabel') == 'treatment-work-strikes'
            and policy.get('explore', {}).get('patience') == 'work'
            and policy.get('explore', {})
            .get('workQuantumSampleEvaluations') == 1_630_000
            and policy.get('compress', {})
            .get('workQuantumSampleEvaluations') == 815_000
            # The iteration patience does not exist on the treatment arm and
            # the document must say so rather than reporting the control's
            # literal beside a policy that never counts to it.
            and treatment_schedule
            .get('exploreIterationsWithoutImprovement') is None
            and schedule.get('exploreIterationsWithoutImprovement') == 200),
    }


def tamper(source, target, mutate):
    with open(source) as handle:
        plan = json.load(handle)
    mutate(plan)
    with open(target, 'w') as handle:
        json.dump(plan, handle, indent=2)
    return target


def spend(out, tag, plan_path, **extra):
    """One spending process. Returns (document, exit, stderr)."""
    document, _, status, err = lib.run(
        'cutclose', 'mixed-61', f'{out}/{tag}.json',
        plan=plan_path, **SPEND, **extra)
    return document, status, err


def plan(out):
    """Calibrate in one process, spend in two more, refuse four tampered."""
    plan_path = f'{out}/armplan.icscal.json'
    calibration, _, calibration_exit, calibration_err = lib.run(
        'cutclose', 'mixed-61', f'{out}/plan-calibration.json',
        icscal=plan_path, **CALIBRATE)
    if calibration_exit or not os.path.exists(plan_path):
        return {'stage': 'plan', 'pass': False, 'exit': calibration_exit,
                'stderr': calibration_err[-800:]}

    first, first_exit, first_err = spend(out, 'plan-spend-a', plan_path)
    second, second_exit, second_err = spend(out, 'plan-spend-b', plan_path)
    bit_identical = (first_exit == 0 and second_exit == 0
                     and lib.stripped(first) == lib.stripped(second))
    outcome = first.get('outcome', {}) or {}
    ledger = outcome.get('calibrated') or {}
    # **The double-debit tripwire, read off the document by a third party.**
    # The ledger is what the pacer thinks it charged; `work` and `sweeps` are
    # what the engine counted, and they reach the document by a different
    # route. The trajectory's own five counters are the sum of the two halves
    # of the ledger, or work is being charged twice or to nobody.
    #
    # Four of the five terms are non-trivially non-zero on this cell -
    # `sampleEvaluations` in the millions, ~160 master batches, ~19 repair rows
    # and ~34 exact calls. `disruptionMoves` is `0 == 0`, and it is worth
    # saying so rather than letting a reader assume otherwise: a calibrated
    # explore phase ends when its units run out, and a separation that ends
    # that way never draws from the pool, so nothing disrupts. The disruption
    # is exercised by the `batches` stage instead, which is a fixed-work cell
    # deep enough to fail a separation and try again.
    work = outcome.get('work') or {}
    charged = ledger.get('charged') or {}
    tail = ledger.get('unchargedTail') or {}
    against_the_engine = {
        'sampleEvaluations': (
            charged.get('sampleEvaluations', 0) + tail.get('sampleEvaluations', 0)
            == work.get('sampleEvaluations')),
        'masterBatches': (
            charged.get('masterBatches', 0) + tail.get('masterBatches', 0)
            == outcome.get('sweeps')),
        'repairRows': (
            charged.get('repairRows', 0) + tail.get('repairRows', 0)
            == work.get('repairRows')),
        'actualPublicationAttemptCalls': (
            charged.get('actualPublicationAttemptCalls', 0)
            + tail.get('actualPublicationAttemptCalls', 0)
            == work.get('exactCheckpoints')),
        'disruptionMoves': (
            charged.get('disruptionMoves', 0) + tail.get('disruptionMoves', 0)
            == work.get('disruptionMoves')),
        'batchesChargedEqualsBatchesCounted': (
            (ledger.get('exploreBatches') or 0) + (ledger.get('compressBatches') or 0)
            == charged.get('masterBatches')),
    }

    # **The misses.** Every key field on its own, and each one must be an exit
    # status rather than a warning: a plan that missed and then measured would
    # be the live probe the clause forbids.
    misses = []
    for name, mutate, wanted in [
        ('requestSha256',
         lambda p: p['key'].__setitem__('requestSha256', '0' * 64), None),
        ('workers', lambda p: p['key'].__setitem__('workers', 4), None),
        ('binaryKey.executableSha256',
         lambda p: p['key']['binaryKey']
         .__setitem__('executableSha256', '1' * 64), None),
        ('binaryKey.features',
         lambda p: p['key']['binaryKey']
         .__setitem__('features', ['overlap-ics', 'ics-profile']), None),
        # Not a tamper: the *runner* asks for a currency the plan is not
        # denominated in. Same clause, other direction.
        ('currencyVersion', None, 'U1'),
        # And the schema version, which the reader refuses before serde sees
        # the rest of the file.
        ('schema', lambda p: p.__setitem__('schema', 'icscal/v2'), None),
    ]:
        tag = name.replace('.', '-')
        if mutate is None:
            path, extra = plan_path, {'currency': wanted}
        else:
            path = tamper(plan_path, f'{out}/miss-{tag}.icscal.json', mutate)
            extra = {}
        document, status, err = spend(out, f'plan-miss-{tag}', path, **extra)
        misses.append({
            'field': name,
            'exit': status,
            'refused': status != 0,
            'namedTheField': name.split('.')[-1] in err,
            'stderr': err.strip()[-300:],
        })

    # **A plan that carries only one phase cannot pace a trajectory that runs
    # two.** The census's committed plan is exactly that shape - Wave 1 had no
    # pacer to spend it and wrote an explore rate alone - but it is also keyed
    # to a different binary, so feeding it here would be refused for the wrong
    # reason and the vector would be green while proving nothing. So the phase
    # is dropped from *this* run's own plan, which matches on every key field
    # and can therefore fail on one thing only.
    one_phase = tamper(plan_path, f'{out}/miss-one-phase.icscal.json',
                       lambda p: p.__setitem__(
                           'phases', [row for row in p['phases']
                                      if row['phase'] != 'compress']))
    _, one_phase_exit, one_phase_err = spend(
        out, 'plan-miss-one-phase', one_phase)

    detail = {
        'stage': 'plan',
        'calibration': {
            'options': CALIBRATE,
            'planPath': plan_path,
            'planSha256': lib.source_sha256(plan_path),
            'summary': (calibration.get('icscal') or {}).get('summary'),
            'phases': [
                {k: row.get(k) for k in
                 ('phase', 'safeUnitsPerSecond', 'measuredUnitsPerSecond',
                  'observedUnits', 'observedSeconds')}
                for row in ((calibration.get('icscal') or {})
                            .get('plan') or {}).get('phases', [])
            ],
        },
        'spend': {
            'options': SPEND,
            'exitA': first_exit,
            'exitB': second_exit,
            'stderr': (first_err or second_err)[-400:],
            'bitIdentical': bit_identical,
            'ledger': ledger,
            'ledgerAgainstTheEngineCounters': against_the_engine,
            # No clock, structurally: a calibrated trajectory's loop reports
            # no seconds and no publication carries one.
            'loopSearchSeconds': first.get('wall', {}).get('loopSearchSeconds'),
            'noPublicationCarriesASecond': all(
                row.get('wallSeconds') is None
                for row in (first.get('outcome', {}) or {})
                .get('publications', [])),
            # **The overshoot clause, exactly.** A phase stops at the barrier
            # after its allocation ran out, so it overspends by at most the
            # units of the batch that crossed - which the ledger names, so
            # this is a comparison of two numbers rather than of a number and
            # a mean.
            'exploreOvershootUnits': (
                (ledger.get('exploreConsumedUnits') or 0)
                - (ledger.get('exploreAllocationUnits') or 0)),
            'compressOvershootUnits': (
                (ledger.get('compressConsumedUnits') or 0)
                - (ledger.get('compressAllocationUnits') or 0)),
            'exploreCrossingBatchUnits':
                ledger.get('exploreCrossingBatchUnits'),
            'compressCrossingBatchUnits':
                ledger.get('compressCrossingBatchUnits'),
        },
        'misses': misses,
        'onePhasePlanRefused': {
            'path': one_phase,
            'exit': one_phase_exit,
            'refused': one_phase_exit != 0,
            'namedTheMissingPhase': 'compress' in one_phase_err,
            'stderr': one_phase_err.strip()[-300:],
        },
    }
    detail['pass'] = bool(
        calibration_exit == 0
        and bit_identical
        and ledger.get('chargeIdentityHolds') is True
        and ledger.get('consumedUnitsMatchCharged') is True
        and all(against_the_engine.values())
        and ledger.get('currencyVersion') == 'U0-sample-evaluations'
        and ledger.get('exploreRatio') == 0.8
        and (ledger.get('charged') or {}).get('masterBatches')
        == (ledger.get('exploreBatches') or 0) + (ledger.get('compressBatches') or 0)
        and (ledger.get('exploreBatches') or 0) > 0
        and detail['spend']['loopSearchSeconds'] is None
        and detail['spend']['noPublicationCarriesASecond']
        # Overshoot >= 0 and <= the crossing batch, on both phases.
        and 0 <= detail['spend']['exploreOvershootUnits']
        <= (detail['spend']['exploreCrossingBatchUnits'] or 0)
        and 0 <= detail['spend']['compressOvershootUnits']
        <= (detail['spend']['compressCrossingBatchUnits'] or 0)
        and all(row['refused'] and row['namedTheField'] for row in misses)
        and one_phase_exit != 0
        and detail['onePhasePlanRefused']['namedTheMissingPhase'])
    return detail


def batches(out):
    """>= 1,024 master batches, two processes, with strikes and disruptions."""
    first, _, exit_a, err_a = lib.run(
        'cutclose', 'mixed-61', f'{out}/batches-a.json', **BATCH_CELL)
    second, _, exit_b, err_b = lib.run(
        'cutclose', 'mixed-61', f'{out}/batches-b.json', **BATCH_CELL)
    if exit_a or exit_b:
        return {'stage': 'batches', 'pass': False, 'exits': [exit_a, exit_b],
                'stderr': (err_a or err_b)[-800:]}
    outcome = first.get('outcome', {})
    rows = outcome.get('bites', [])
    master_batches = outcome.get('sweeps') or 0
    strikes = sum(row.get('strikes', 0) for row in rows)
    # **A disruption IS a pool restore**, and that is a fact about the loop
    # rather than an inference from a counter: `disrupt::disrupt` is called on
    # the line after `install_poses(&entry.poses)` and `entry.restore_weights`,
    # and on no other line. So a cell with a disruption is a cell in which a
    # pooled layout and its learned weights were reinstalled.
    disruptions = sum(row.get('disruptions', 0) for row in rows)
    failed_separations = sum(row.get('attempts', 0) for row in rows)
    detail = {
        'stage': 'batches',
        'options': BATCH_CELL,
        'masterBatches': master_batches,
        'strikes': strikes,
        'disruptions': disruptions,
        'poolRestoresWithADisruption': disruptions,
        'failedSeparations': failed_separations,
        'exitA': exit_a,
        'exitB': exit_b,
        'digestA': lib.digest(first),
        'digestB': lib.digest(second),
        'sourceShaA': lib.source_sha256(f'{out}/batches-a.json'),
        'sourceShaB': lib.source_sha256(f'{out}/batches-b.json'),
        'bitIdentical': lib.stripped(first) == lib.stripped(second),
    }
    detail['pass'] = bool(
        detail['bitIdentical'] and master_batches >= 1024
        and strikes > 0 and disruptions > 0 and failed_separations > 0)
    return detail


STAGES = {'arms': arms, 'plan': plan, 'batches': batches}
ORDER = ['arms', 'plan', 'batches']


def main():
    out = os.environ.get('ICS_OUT', lib.OUT)
    os.makedirs(out, exist_ok=True)
    wanted = sys.argv[1:] or ORDER
    results = []
    for name in wanted:
        if name not in STAGES:
            raise SystemExit(f'unknown stage {name}')
        results.append(STAGES[name](out))
    failures = [row['stage'] for row in results if not row['pass']]
    document = {
        'experiment': 'overlap-ics',
        'battery': 'economics-round-integration',
        'cellSources': lib.MANIFEST,
        'binary': lib.BIN,
        'stagesRun': [row['stage'] for row in results],
        'stages': results,
        'failures': failures,
        'ARMPLAN_PASS': not failures,
    }
    print(json.dumps(document, indent=1))
    with open(f'{out}/armplan-fast.json', 'w') as handle:
        json.dump(document, handle, indent=1)
    return 0 if not failures else 1


if __name__ == '__main__':
    sys.exit(main())
