#!/usr/bin/env python3
"""**The profile census, and the executor go/no-go.**

    python3 spawntax.py [work-dir]

docs/economics-round-spec.md funds the persistent executor **behind a measured
gate**, and this driver is that measurement:

> Persistent executor, behind a measured gate: profile easy + bite-22 hard
> states, workers 1/2/4/8, identical fixed work (prep, dispatch/join, sweeps,
> merge+GLS, exact/repair separately). **Build iff prep+dispatch >= 10 % of
> hard-state wall.**

The threshold below is that sentence's, quoted rather than chosen, and it is
**pre-committed**: it is written into this file and into the census document
before either was run against a number. Nothing here may re-pick it, and the
verdict is a comparison rather than a judgement.

# The density, and the defect it is defending against

The spec's pre-named defect (3) is *probe-on-cheap-bites*: "calibrating on
bites 1-21 overstates iters/s ~1.5x; the probe is 400 iterations AT the 179
shelf". So every arm here runs the constructor, takes the 21 published 0.1 %
bites that land mixed-61 on the 179 shelf, and only then spends its 200 master
iterations - on the 22nd bite, the one that does not publish. Both windows are
reported: `cheapPrefix` is what the defect would have calibrated on, and the
ratio between them is printed, so the 1.5x is a measurement in this document
rather than a warning about one.

# What "identical fixed work" means here, exactly

**The prefix always runs at the frozen eight workers**, whatever the arm's own
worker count is. That is deliberate and it is the difference between measuring
the machinery and measuring four different layouts: all four rungs of the
ladder enter the shelf from the *same* state - the same constructor, the same
21 publications, the same exact-valid parent - and only the probe's worker
count differs. Every arm then spends the same **200 master iterations**.

It does not spend the same CPU: one master iteration at eight workers buys
eight sweeps and at one worker buys one, which is the whole point of the
ladder. The four probes' trajectories also diverge after their first iteration,
because the winner of a 2-worker tournament is not the winner of an 8-worker
one. So the ladder is a measurement of the *machinery* at a fixed density and a
fixed iteration count, and the per-window currency terms are reported beside
every arm so a reader can normalise by work instead of by iteration.

# Which seeds are at that density, and who decides

Not all nine are. The regime map in the spec's last paragraph says so, and the
measurement agrees: the fast cascade and the strike-starved seeds stall well
above 179 under fixed work, and a 181.5 mm state is a different density with
different economics. So this driver runs **all nine**, and derives the verdict
set from the measurement rather than from a list somebody typed: an arm counts
toward the verdict when its prefix published all 21 bites *and* its probe spent
every one of its 200 iterations at the shelf without publishing. Both
conditions are fields in the document, so the set can be recomputed by anyone
who disagrees with it.

# Two processes, and a quiet box

Every arm runs twice, in two processes. The **counters** must agree exactly
between the two - they are a function of the trajectory - and the
**nanoseconds** must not, because they are a measurement of a machine. Both
are reported; the spread between the two processes is the honest error bar on
every share below, and the verdict is rendered on the reading most favourable
to building, so a NO-GO cannot be an artefact of a slow process.
"""
import hashlib
import json
import os
import platform
import subprocess
import sys
import time

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                    '..', '..', '..', '..', '..'))
REQUEST = (f'{ROOT}/tests/fixtures/mixed-61/'
           'mixed61-request-exact-clearance.json')
# The census needs the phase timers, so it runs the `ics-profile` build. The
# default build is what every gate is measured on and it reports
# `measured: false` here, which is a refusal to answer rather than an answer of
# zero - see `identity.py` for the proof that the two take the same trajectory.
PROFILE_BIN = os.environ.get(
    'ICS_PROFILE_BIN',
    f'{ROOT}/target/profile-build/release/examples/overlap_ics_benchmark')

# --------------------------------------------------------- the pre-committed --
#
# docs/economics-round-spec.md, funded change 2: "Build iff prep+dispatch
# >= 10% of hard-state wall." One literal, quoted, and this file contains no
# other threshold.
EXECUTOR_GATE_SHARE = 0.10
EXECUTOR_GATE_QUOTE = ('docs/economics-round-spec.md, funded change 2: '
                       '"Build iff prep+dispatch >= 10% of hard-state wall."')

WORKER_LADDER = [1, 2, 4, 8]
# 8 is frozen by the spec ("workers=8"), so it is the only worker count the
# verdict may be rendered on. The other three are the shape of the tax.
VERDICT_WORKERS = 8
SEEDS = list(range(9))
SHELF_BITES = 21
# The audit's committed fixed-work replay shape, unchanged: 21 bites, one
# attempt, 400 iterations. The prefix is that replay.
PREFIX_ITERATIONS = 400
PROBE_ITERATIONS = 200
PROCESSES = ['a', 'b']
# The depth 21 published 0.1 % bites reach on mixed-61 seed 0, reproduced bit
# for bit by the evidence audit's committed fixed-work replay on three
# machines. If seed 0 does not land here, the prefix is not the replay and
# nothing below is a measurement of the shelf.
SHELF_DEPTH_SEED0_MM = 179.16566573285345


def sha256_of(path):
    try:
        with open(path, 'rb') as handle:
            return hashlib.sha256(handle.read()).hexdigest()
    except OSError:
        return None


def loadavg():
    try:
        with open('/proc/loadavg') as handle:
            return handle.read().split()[:3]
    except OSError:
        return None


def arm(out, workers, seed, process, icscal=None):
    """One process: one worker count, one seed, 200 iterations at the shelf."""
    tag = f'spawntax-w{workers}-seed{seed}-{process}'
    path = f'{out}/{tag}.json'
    os.makedirs(out, exist_ok=True)
    command = [PROFILE_BIN, '--cell=spawntax', f'--request={REQUEST}',
               '--edge=5', '--pair=5', f'--workers={workers}', f'--seed={seed}',
               f'--prefixworkers={VERDICT_WORKERS}',
               f'--shelfbites={SHELF_BITES}',
               f'--prefixiters={PREFIX_ITERATIONS}',
               f'--probeiters={PROBE_ITERATIONS}']
    if icscal:
        command.append(f'--icscal={icscal}')
    before = loadavg()
    started = time.monotonic()
    with open(path, 'w') as handle:
        result = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                                check=False)
    wall = time.monotonic() - started
    status = result.returncode
    stderr = (result.stderr or b'').decode()[-500:]
    try:
        with open(path) as handle:
            document = json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        document = {'_loadError': f'{error}'}
    census = document.get('spawnTax', {})
    # **The double-debit clause, on a window that actually repairs.**
    #
    # `cutclose.py`'s FAST tripwire checks the same identity, but its K=8 cell
    # publishes every bite on its first separation, so its repair-row and
    # disruption-move clauses are vacuously satisfied. The 21-bite prefix here
    # does repair, so the sum is a real one. The spec ranks "persistent-slot
    # leakage / double-debit" first among this round's pre-named defects, and
    # the whole point of charging the currency per bite is that the per-bite
    # deltas have to add back up to the trajectory's own work vector.
    prefix_rows = [row.get('census') or {} for row in census.get('prefixPerBite', [])]
    prefix_work = census.get('prefixWork') or {}

    def summed(key):
        return sum((row.get('currencyTerms') or {}).get(key, 0)
                   for row in prefix_rows)

    reconciliation = {
        'prefixSampleEvaluations': summed('sampleEvaluations'),
        'prefixWorkSampleEvaluations': prefix_work.get('sampleEvaluations'),
        'prefixRepairRows': summed('repairRows'),
        'prefixWorkRepairRows': prefix_work.get('repairRows'),
        'prefixDisruptionMoves': summed('disruptionMoves'),
        'prefixWorkDisruptionMoves': prefix_work.get('disruptionMoves'),
        'prefixActualPublicationAttemptCalls':
            summed('actualPublicationAttemptCalls'),
        'prefixWorkExactCheckpoints': prefix_work.get('exactCheckpoints'),
    }
    reconciliation['pass'] = bool(
        prefix_rows
        and reconciliation['prefixSampleEvaluations']
        == reconciliation['prefixWorkSampleEvaluations']
        and reconciliation['prefixRepairRows']
        == reconciliation['prefixWorkRepairRows']
        and reconciliation['prefixDisruptionMoves']
        == reconciliation['prefixWorkDisruptionMoves']
        and reconciliation['prefixActualPublicationAttemptCalls']
        == reconciliation['prefixWorkExactCheckpoints'])
    return {
        'workers': workers,
        'seed': seed,
        'process': process,
        'exit': status,
        'stderr': stderr,
        'processWallSeconds': wall,
        'loadAvgBefore': before,
        'loadAvgAfter': loadavg(),
        # RV3: the reduction names the bytes it reduced.
        'sourcePath': path,
        'sourceSha256': sha256_of(path),
        'executableSha256': document.get('executableSha256'),
        'buildFeatures': document.get('buildFeatures'),
        'profileFeature': census.get('profileFeature'),
        'prefixWorkers': census.get('prefixWorkers'),
        'prefixAllPublished': census.get('prefixAllPublished'),
        'prefixDepthMm': census.get('prefixDepthMm'),
        'prefixFingerprint': census.get('prefixFingerprint'),
        'shelfDepthMm': census.get('shelfDepthMm'),
        'shelfEntryWidthMm': census.get('shelfEntryWidthMm'),
        'shelfPublished': census.get('shelfPublished'),
        'shelfIterations': census.get('shelfIterations'),
        'shelfStrikes': census.get('shelfStrikes'),
        'shelfBandEntries': census.get('shelfBandEntries'),
        'shelfCheckpointCalls': census.get('shelfCheckpointCalls'),
        'hardState': census.get('hardState'),
        'cheapPrefix': census.get('cheapPrefix'),
        'work': census.get('work'),
        'prefixWork': prefix_work,
        'perBiteCurrencyReconciliation': reconciliation,
        'funnel': (document.get('outcome') or {}).get('funnel'),
        'icscal': document.get('icscal'),
    }


def counters_of(row):
    """The fields two processes of one arm must agree on exactly.

    Everything here is a function of the trajectory. Nothing here is a clock.
    """
    hard = row.get('hardState') or {}
    cheap = row.get('cheapPrefix') or {}
    return {
        'prefixDepthMm': row.get('prefixDepthMm'),
        'prefixFingerprint': row.get('prefixFingerprint'),
        'shelfDepthMm': row.get('shelfDepthMm'),
        'shelfEntryWidthMm': row.get('shelfEntryWidthMm'),
        'shelfIterations': row.get('shelfIterations'),
        'shelfStrikes': row.get('shelfStrikes'),
        'shelfBandEntries': row.get('shelfBandEntries'),
        'shelfCheckpointCalls': row.get('shelfCheckpointCalls'),
        'hardIterations': hard.get('iterations'),
        'hardCurrency': hard.get('currencyTerms'),
        'cheapIterations': cheap.get('iterations'),
        'cheapCurrency': cheap.get('currencyTerms'),
        'work': row.get('work'),
        'funnel': row.get('funnel'),
    }


def pair(rows):
    """One arm's two processes, reduced."""
    first, second = rows
    hard_a = first.get('hardState') or {}
    hard_b = second.get('hardState') or {}
    shares = [row.get('prepPlusDispatchShare') for row in (hard_a, hard_b)]
    shares = [value for value in shares if value is not None]
    per_iteration = [row.get('barrierToBarrierNsPerIteration')
                     for row in (hard_a, hard_b)]
    per_iteration = [value for value in per_iteration if value is not None]
    cheap_per_iteration = [
        (row.get('cheapPrefix') or {}).get('barrierToBarrierNsPerIteration')
        for row in (first, second)]
    cheap_per_iteration = [value for value in cheap_per_iteration
                           if value is not None]
    return {
        'workers': first['workers'],
        'seed': first['seed'],
        'exits': [row['exit'] for row in rows],
        'sourceSha256': [row['sourceSha256'] for row in rows],
        'perBiteCurrencyReconciles': all(
            row['perBiteCurrencyReconciliation']['pass'] for row in rows),
        # The trajectory half of the two-process claim: identical.
        'countersAgree': counters_of(first) == counters_of(second),
        'processes': rows,
        # The measurement half: a spread, reported rather than averaged away.
        'prepPlusDispatchShareMin': min(shares) if shares else None,
        'prepPlusDispatchShareMax': max(shares) if shares else None,
        'hardNsPerIterationMin': min(per_iteration) if per_iteration else None,
        'hardNsPerIterationMax': max(per_iteration) if per_iteration else None,
        'cheapNsPerIterationMin': (min(cheap_per_iteration)
                                   if cheap_per_iteration else None),
        # Derived from the measurement, not from a list: this arm entered the
        # 179 shelf with all 21 bites published and spent every one of its 200
        # iterations there without publishing. Both halves are fields above.
        'atShelfDensity': bool(
            first.get('prefixAllPublished')
            and first.get('shelfPublished') is False
            and first.get('shelfIterations') == PROBE_ITERATIONS),
        # **Pre-named defect (3), measured, in both currencies.**
        #
        # `shelfOverCheapIterationCost` is how much more one master iteration
        # costs at the shelf than in the cheap prefix a naive probe would have
        # calibrated on. `shelfOverCheapEvalRate` is the same comparison
        # denominated in the member's own work unit. The two do not agree, and
        # which one a pacer is written in is therefore a decision and not a
        # detail: a plan denominated in iterations inherits the defect and a
        # plan denominated in sample evaluations largely does not.
        'shelfOverCheapIterationCost': (
            None if not per_iteration or not cheap_per_iteration
            else min(per_iteration) / min(cheap_per_iteration)),
        'shelfEvalsPerSecond': hard_a.get('sampleEvaluationsPerSecond'),
        'cheapEvalsPerSecond':
            (first.get('cheapPrefix') or {}).get('sampleEvaluationsPerSecond'),
        'shelfOverCheapEvalRate': (
            None
            if not hard_a.get('sampleEvaluationsPerSecond')
            or not (first.get('cheapPrefix') or {}).get('sampleEvaluationsPerSecond')
            else hard_a['sampleEvaluationsPerSecond']
            / first['cheapPrefix']['sampleEvaluationsPerSecond']),
        'sweepParallelEfficiency': (
            None if not hard_a.get('ns') or not hard_a['ns'].get('sweepCritical')
            else hard_a['ns']['sweepTotal'] / hard_a['ns']['sweepCritical']),
        # **The tax in nanoseconds, not as a share.**
        #
        # The share is a ratio, and a ratio moves when either end moves. These
        # are the numerator and the denominator separately, per master
        # iteration, so a reader can see which one a difference between two
        # arms actually came from. It is the single most useful pair of numbers
        # in this document: the tax is close to constant across every arm and
        # the sweep is not, so an arm with a high share has a cheap sweep
        # rather than an expensive dispatch.
        'prepPlusDispatchNsPerIteration': (
            None if not hard_a.get('iterations')
            else hard_a['prepPlusDispatchNs'] / hard_a['iterations']),
        'prepNsPerIteration': (
            None if not hard_a.get('iterations')
            else hard_a['ns']['prep'] / hard_a['iterations']),
        'dispatchNsPerIteration': (
            None if not hard_a.get('iterations')
            else hard_a['ns']['dispatch'] / hard_a['iterations']),
        'sweepCriticalNsPerIteration': (
            None if not hard_a.get('iterations')
            else hard_a['ns']['sweepCritical'] / hard_a['iterations']),
    }


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else '/var/lib/t3/tmp/census-wave1/spawntax'
    os.makedirs(out, exist_ok=True)
    icscal_path = f'{out}/mixed61-w8-seed0.icscal.json'
    arms = []
    for workers in WORKER_LADDER:
        for seed in SEEDS:
            rows = []
            for process in PROCESSES:
                # One icscal file, written by one arm: the frozen worker count,
                # the gate seed, the first process. A plan is calibrated once.
                write = (icscal_path if (workers == VERDICT_WORKERS
                                         and seed == 0 and process == 'a')
                         else None)
                rows.append(arm(out, workers, seed, process, icscal=write))
            arms.append(pair(rows))

    # The verdict is rendered on the FROZEN worker count, at the shelf density,
    # and on nothing else. Both conditions are read off the measurement.
    verdict_arms = [row for row in arms
                    if row['workers'] == VERDICT_WORKERS and row['atShelfDensity']]
    # **The verdict is rendered on the reading most favourable to BUILDING.**
    # If even the largest observed prep+dispatch share is under the bar, a
    # NO-GO cannot be an artefact of one slow process or one unlucky seed.
    observed = [row['prepPlusDispatchShareMax'] for row in verdict_arms
                if row['prepPlusDispatchShareMax'] is not None]
    best = max(observed) if observed else None
    measured = bool(verdict_arms) and all(
        (row['processes'][0].get('hardState') or {}).get('measured')
        for row in verdict_arms)
    seed0 = [row for row in arms if row['seed'] == 0]
    depth_ok = bool(seed0) and all(
        row['processes'][0]['prefixDepthMm'] == SHELF_DEPTH_SEED0_MM
        for row in seed0)
    build = bool(measured and best is not None and best >= EXECUTOR_GATE_SHARE)
    verdict = {
        'clause': EXECUTOR_GATE_QUOTE,
        'thresholdShare': EXECUTOR_GATE_SHARE,
        'workers': VERDICT_WORKERS,
        'state': f'bite {SHELF_BITES + 1}, the 179 shelf, {PROBE_ITERATIONS} '
                 'master iterations',
        'rendering': 'the largest prep+dispatch share observed on the frozen '
                     'worker count, over every seed that reached the shelf '
                     'density and both of its processes - the reading most '
                     'favourable to building',
        'verdictSeeds': sorted(row['seed'] for row in verdict_arms),
        'seedsAtShelfDensity': sorted(
            {row['seed'] for row in arms if row['atShelfDensity']}),
        'seedsNotAtShelfDensity': sorted(
            {row['seed'] for row in arms if not row['atShelfDensity']}),
        'observedShares': observed,
        'observedShareMax': best,
        'observedShareMin': min(observed) if observed else None,
        # The share on every arm at the frozen worker count, shelf or not, so
        # a reader can see what the clause would have said if it had not named
        # a state - and the two numbers that explain the difference.
        'allWorkerEightShares': {
            str(row['seed']): {
                'atShelfDensity': row['atShelfDensity'],
                'iterations': (row['processes'][0].get('hardState') or {})
                .get('iterations'),
                'share': row['prepPlusDispatchShareMax'],
                'prepPlusDispatchNsPerIteration':
                    row['prepPlusDispatchNsPerIteration'],
                'sweepCriticalNsPerIteration':
                    row['sweepCriticalNsPerIteration'],
            }
            for row in arms if row['workers'] == VERDICT_WORKERS
        },
        'profileTimersPresent': measured,
        'seed0PrefixDepthMatchesCommittedReplay': depth_ok,
        'BUILD_PERSISTENT_EXECUTOR': build,
        'verdict': 'BUILD' if build else 'DO NOT BUILD',
    }
    document = {
        'experiment': 'overlap-ics',
        'battery': 'economics-round-census-spawn-tax',
        'binary': PROFILE_BIN,
        'binarySha256': sha256_of(PROFILE_BIN),
        'requestSha256': sha256_of(REQUEST),
        'request': REQUEST,
        'box': {
            'platform': platform.platform(),
            'machine': platform.machine(),
            'cpuCount': os.cpu_count(),
            'loadAvgAtStart': loadavg(),
        },
        'shelfBites': SHELF_BITES,
        'prefixIterations': PREFIX_ITERATIONS,
        'prefixWorkers': VERDICT_WORKERS,
        'probeIterations': PROBE_ITERATIONS,
        'workerLadder': WORKER_LADDER,
        'seeds': SEEDS,
        'arms': arms,
        'allExitsZero': all(status == 0 for row in arms
                            for status in row['exits']),
        'allCountersAgreeAcrossProcesses': all(row['countersAgree']
                                               for row in arms),
        # The spec's ranked defect (1): per-bite currency deltas that do not
        # add back up to the trajectory's own work vector are work charged
        # twice or charged to nobody.
        'allPerBiteCurrencyReconciles': all(row['perBiteCurrencyReconciles']
                                            for row in arms),
        'icscalPath': icscal_path,
        'icscalSha256': sha256_of(icscal_path),
        'verdict': verdict,
    }
    print(json.dumps(document, indent=1))
    with open(f'{out}/spawntax.json', 'w') as handle:
        json.dump(document, handle, indent=1)
    # A census that could not measure, could not reach the shelf, or whose two
    # processes disagreed on a counter has not produced a verdict at all, and
    # exits non-zero so nothing downstream reads one.
    healthy = (document['allExitsZero']
               and document['allCountersAgreeAcrossProcesses']
               and document['allPerBiteCurrencyReconciles']
               and verdict['profileTimersPresent']
               and verdict['seed0PrefixDepthMatchesCommittedReplay'])
    return 0 if healthy else 1


if __name__ == '__main__':
    sys.exit(main())
