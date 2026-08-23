#!/usr/bin/env python3
"""**The declared fallback's budget: a single-fixture work plan, no transfer claim.**

    python3 budget.py [work-dir]

docs/currency-amendment.md, the fallback, verbatim:

> if `U'` fails, the **transferring** pacer is closed and the 10 s two-arm
> mixed-61 gate runs on a mixed-61-only shelf-probed work budget labeled
> "single-fixture work plan, no transfer claim"

`U'` failed (`gate2/rejectgate2.py`, exit 1, three runs, six of six ordered
pairs over the bar on every one). So this script runs, and it runs **before any
gate cell**. It produces one `icscal/v1` file and one document saying where
every number in it came from. **Nothing here is retuned afterwards** - not after
the 10 s cells, not after `p95`, not ever.

# What "shelf-probed" is, exactly

The spec's pre-named defect (3) is *probe-on-cheap-bites*: "calibrating on bites
1-21 overstates iters/s ~1.5x; **the probe is 400 iterations AT the 179 shelf**".
So the explore rate is measured on exactly that cell - `--cell=spawntax`, the
21 published 0.1 % bites to land the trajectory on the shelf, then **400 master
iterations on the 22nd bite**, which does not publish - and the units are the
**probe bite's own** `profile.sampleEvaluations` over the **probe's own**
`wall.searchSeconds`.

`sampleEvaluations` is a counter, populated in every build; `searchSeconds` is
the driver's bracket around the probe alone. Neither needs `ics-profile`, so the
probe runs on **the gate binary itself** and the plan is keyed to it.

## Why the writer's own plan is not used

`overlap_ics_benchmark --cell=spawntax --icscal=<path>` writes a plan, and on a
build **without** `ics-profile` that plan is wrong in the unsafe direction. Its
non-profile branch divides `outcome.trace.work.sample_evaluations` - which is
the engine's **cumulative** work vector, prefix included - by `search_seconds`,
which is the **probe alone**. On this box that reads 7,694,847 units over
2.4045 s = 3,200,171 units/s, against a true shelf rate of 6,605,800 / 2.4045 =
2,747,252 units/s: **16 % fast**, under a `derivation` string that says the
rate "includes the cheap prefix and is deliberately slower than the shelf's".

It is slower than the shelf on a profiling build, where the branch above it
takes the shelf's own barrier-to-barrier wall. On a plain build it is faster,
and a plan that over-promises a rate spends more work per second of budget than
the machine can do in it - which is an overrun, in the one direction the
conservative rounding rule exists to avoid. The census is unaffected (it
measured the profiling build and took the other branch). This wave may not edit
engine code, so it does not: it reads the two counters the document already
carries and composes the plan here, where the arithmetic is visible.

# The compress rate, and why it is not the shelf's

A trajectory runs both phases and the pacer refuses a plan that carries one
rate, so a compress rate has to come from somewhere. The shelf is an *explore*
stall - a 0.1 % cut that does not publish - and compress bites are a different
regime by construction (uniform-Y cut, compress range, decay by consumed
compress work). Pricing compress at the shelf's rate would be the
probe-on-the-wrong-window defect wearing the opposite hat. So the compress rate
is measured on **compress bites**, by one wall-mode calibration process on the
same fixture, the same seed and the same binary, and it is taken verbatim from
that process's own `icscal` file. Both derivations travel in the plan.

# The frame: why the budget is 10.000 s MINUS the constructor

Every §0 clause in this campaign is written in the **request-relative** frame.
The wall arm's `--wall=10` starts its clock at the decoded request; §0.1's
"a publication completed after 10.000 s cannot change that verdict" is
request-relative, and the evidence audit's `checkpoint-frame.py` exists because
one driver compared a loop-relative clock against it and the two are 2.3 s apart
on mixed-61. Clause (5)'s `p95 <= 10.000 s` is a statement about the same frame:
the wall of a ten-second run.

A `--mode=calibrated` trajectory has **no clock at all**, so it cannot subtract
its own constructor. The constructor is *charged and uncapped* by the spec, so
the search's share of a 10.000 s request is `10.000 - constructorSeconds`, and
that subtraction is done **here**, once, from a constructor time **pinned by the
probe before any gate cell runs**. It is the same number for both arms, all nine
seeds and all four budgets, and it is never recomputed. Getting this wrong in
the other direction - handing the pacer the whole 10.000 s - would spend ten
seconds of search on top of a 2.3 s constructor and fail clause (5) by
construction rather than by measurement.

# The label

Every document this budget touches carries
`"single-fixture work plan, no transfer claim"`, and the plan's own
`provenance` string carries it too, so a file that escapes this directory still
says what it is.

Exit is the verdict: `0` when the plan was written and re-read by the engine's
own reader as a **hit**, `2` when it could not be.
"""
import hashlib
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, '..', '..', '..', '..', '..'))
sys.path.insert(0, f'{ROOT}/docs/experiments/overlap-ics/drivers')
import lib  # noqa: E402

LABEL = 'single-fixture work plan, no transfer claim'
FIXTURE = 'mixed-61'
SEED = 0
WORKERS = 8
# The spec's own probe shape: 21 published 0.1 % bites to reach the shelf, then
# 400 master iterations AT it.
SHELF_BITES = 21
PREFIX_ITERS = 400
PROBE_ITERS = 400
# The campaign's safety factor, unchanged. `PhasePlan::from_measurement` applies
# it in the engine; it is applied here for the composed explore phase and the
# measured value is kept beside it so the discount is visible rather than baked
# in.
SAFETY = 0.80
# The wall calibration that supplies the compress rate. Long enough that the
# compress phase is a phase and not a tail.
CALIBRATION_WALL_SECONDS = 30.0
# §0's budget, in the frame §0 is written in.
GATE_BUDGET_SECONDS = 10.000


def sha256_of(path):
    with open(path, 'rb') as handle:
        return hashlib.sha256(handle.read()).hexdigest()


def run(command, out_path):
    with open(out_path, 'w') as handle:
        result = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                                check=False)
    try:
        with open(out_path) as handle:
            document = json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        document = {'_loadError': f'{error}'}
    return document, result.returncode, (result.stderr or b'').decode()[-400:]


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else '/var/lib/t3/tmp/overlapics/gate2/budget'
    os.makedirs(out, exist_ok=True)
    request = lib.REQUESTS[FIXTURE]
    document = {
        'experiment': 'overlap-ics',
        'battery': 'economics-round-gate2-fallback-budget',
        'label': LABEL,
        'why': ('docs/currency-amendment.md: U\' failed its own reject rule, so '
                'the transferring pacer is closed and the gate runs on a '
                'mixed-61-only shelf-probed work budget. Final per rider (iii).'),
        'fixture': FIXTURE,
        'requestSha256': None,
        'binary': lib.BIN,
        'binarySha256': sha256_of(lib.BIN),
        'gateBudgetSeconds': GATE_BUDGET_SECONDS,
        'safetyFactor': SAFETY,
    }

    # ---- 1. the shelf probe: 400 master iterations AT the 179 shelf ----
    probe_path = f'{out}/shelf-probe.json'
    probe_command = [
        lib.BIN, '--cell=spawntax', f'--request={request}',
        f'--edge={lib.EDGE_MM}', f'--pair={lib.PAIR_MM}',
        f'--workers={WORKERS}', f'--prefixworkers={WORKERS}', f'--seed={SEED}',
        f'--shelfbites={SHELF_BITES}', f'--prefixiters={PREFIX_ITERS}',
        f'--probeiters={PROBE_ITERS}']
    probe, status, stderr = run(probe_command, probe_path)
    if status != 0:
        document['error'] = f'the shelf probe exited {status}: {stderr}'
        print(json.dumps(document, indent=1))
        return 2
    spawn = probe.get('spawnTax') or {}
    probe_bites = (probe.get('outcome') or {}).get('bites') or []
    if not probe_bites:
        document['error'] = 'the shelf probe produced no bite to price'
        print(json.dumps(document, indent=1))
        return 2
    probe_units = (probe_bites[0].get('profile') or {}).get('sampleEvaluations')
    probe_seconds = (probe.get('wall') or {}).get('searchSeconds')
    constructor_seconds = (probe.get('wall') or {}).get('constructorSeconds')
    document['shelfProbe'] = {
        'path': probe_path,
        'sourceSha256': sha256_of(probe_path),
        'command': probe_command,
        # The three preconditions of a shelf probe, asserted rather than hoped
        # for: the prefix published every bite, the probe bite did NOT publish,
        # and it really ran the iterations it was asked for.
        'prefixAllPublished': spawn.get('prefixAllPublished'),
        'prefixBites': spawn.get('prefixBites'),
        'prefixDepthMm': spawn.get('prefixDepthMm'),
        'shelfPublished': spawn.get('shelfPublished'),
        'shelfIterations': spawn.get('shelfIterations'),
        'shelfDepthMm': spawn.get('shelfDepthMm'),
        'probeSampleEvaluations': probe_units,
        'probeSeconds': probe_seconds,
        'constructorSeconds': constructor_seconds,
        'measuredUnitsPerSecond': (None if not probe_seconds else
                                   probe_units / probe_seconds),
        'writersOwnPlanNotUsed': (probe.get('icscal') or {}).get('plan'),
        'writersOwnPlanRejectedBecause': (
            'shelf_work_plan\'s non-profile branch divides the CUMULATIVE work '
            'vector by the probe\'s own wall, which reads fast rather than '
            'slow. See this file\'s module docs.'),
    }
    if not (spawn.get('prefixAllPublished')
            and spawn.get('shelfPublished') is False
            and spawn.get('shelfIterations') == PROBE_ITERS
            and probe_units and probe_seconds and constructor_seconds):
        document['error'] = ('the shelf probe did not meet its own '
                             'preconditions; no budget is derivable from it')
        print(json.dumps(document, indent=1))
        return 2
    explore_measured = probe_units / probe_seconds

    # ---- 2. the compress rate, from compress bites ----
    calibration_path = f'{out}/compress-calibration.json'
    calibration_plan_path = f'{out}/compress-calibration.icscal.json'
    calibration_command = [
        lib.BIN, '--cell=cutclose', f'--request={request}',
        f'--edge={lib.EDGE_MM}', f'--pair={lib.PAIR_MM}', '--mode=wall',
        f'--wall={CALIBRATION_WALL_SECONDS}', f'--workers={WORKERS}',
        f'--seed={SEED}', f'--icscal={calibration_plan_path}']
    calibration, status, stderr = run(calibration_command, calibration_path)
    if status != 0:
        document['error'] = f'the compress calibration exited {status}: {stderr}'
        print(json.dumps(document, indent=1))
        return 2
    written = (calibration.get('icscal') or {}).get('plan') or {}
    compress = next((row for row in written.get('phases') or []
                     if row.get('phase') == 'compress'), None)
    blended_explore = next((row for row in written.get('phases') or []
                            if row.get('phase') == 'explore'), None)
    if compress is None or blended_explore is None:
        document['error'] = 'the calibration wrote no compress phase'
        print(json.dumps(document, indent=1))
        return 2
    document['compressCalibration'] = {
        'path': calibration_path,
        'sourceSha256': sha256_of(calibration_path),
        'command': calibration_command,
        'compressBites': (calibration.get('outcome') or {}).get('compressBites'),
        'exploreBites': (calibration.get('outcome') or {}).get('exploreBites'),
        'phase': compress,
        # Not used, reported: the blended explore rate the same process wrote,
        # which is what a plan calibrated on a whole wall trajectory rather
        # than on the shelf would have spent.
        'blendedExplorePhase_NOT_USED': blended_explore,
        'shelfOverBlendedExploreRate': (
            explore_measured / blended_explore['measuredUnitsPerSecond']),
    }

    # ---- 3. the plan ----
    request_sha = (calibration.get('requestSha256')
                   or written.get('key', {}).get('requestSha256'))
    document['requestSha256'] = request_sha
    plan = {
        'schema': 'icscal/v1',
        'key': {
            'requestSha256': request_sha,
            'currencyVersion': 'U0-sample-evaluations',
            'binaryKey': {
                'executableSha256': document['binarySha256'],
                'features': ['overlap-ics'],
            },
            'workers': WORKERS,
            'executor': 'ephemeral-scope',
        },
        'phases': [
            {
                'phase': 'explore',
                'safeUnitsPerSecond': explore_measured * SAFETY,
                'measuredUnitsPerSecond': explore_measured,
                'safetyFactor': SAFETY,
                'observedUnits': probe_units,
                'observedSeconds': probe_seconds,
                'derivation': (
                    f'{LABEL}. THE SHELF PROBE: bite {SHELF_BITES + 1} (the 179 '
                    f'shelf) alone, {PROBE_ITERS} master iterations, the probe '
                    f'bite\'s own sampleEvaluations over the probe\'s own '
                    f'searchSeconds, measured on this binary. NOT the cheap '
                    f'0.1 % prefix: spec defect (3). NOT the writer\'s own '
                    f'non-profile plan, which divides the cumulative work '
                    f'vector by the probe\'s wall.'),
            },
            compress | {
                'derivation': f'{LABEL}. {compress["derivation"]}',
            },
        ],
        'provenance': (
            f'docs/experiments/overlap-ics/economics-round/gate2/budget.py - '
            f'the declared fallback of docs/currency-amendment.md. {LABEL}: '
            f'both rates are mixed-61\'s own and this plan makes no claim '
            f'about any other fixture. Explore from the shelf probe '
            f'({probe_path}), compress from one wall calibration '
            f'({calibration_path}).'),
    }
    plan_path = f'{out}/gate2.icscal.json'
    with open(plan_path, 'w') as handle:
        json.dump(plan, handle, indent=1)
    document['plan'] = plan
    document['planPath'] = plan_path
    document['planSha256'] = sha256_of(plan_path)

    # ---- 4. the budget, in the frame §0 is written in ----
    search_budget = GATE_BUDGET_SECONDS - constructor_seconds
    document['budget'] = {
        'frame': ('request-relative, like every §0 clause: the budget is the '
                  'search\'s share of a 10.000 s request, and the constructor '
                  'is charged and uncapped by the spec.'),
        'gateBudgetSeconds': GATE_BUDGET_SECONDS,
        'pinnedConstructorSeconds': constructor_seconds,
        'searchBudgetSeconds': search_budget,
        'exploreRatio': 0.80,
        'exploreAllocationUnits': int(
            explore_measured * SAFETY * search_budget * 0.80),
        'compressAllocationUnits': int(
            compress['safeUnitsPerSecond'] * (search_budget - search_budget * 0.80)),
        'retuned': False,
        'rule': ('set before any gate cell ran; NOT retuned after seeing p95, '
                 'per docs/currency-amendment.md'),
        # The same subtraction at the other three budgets, computed here so
        # that no later process has to choose one.
        'searchBudgetSecondsByBudget': {
            str(seconds): seconds - constructor_seconds
            for seconds in (3.0, 10.0, 30.0, 60.0)},
    }

    # ---- 5. the engine's own reader must call it a hit ----
    #
    # A composed plan that the reader refuses is worse than no plan, and the
    # refusal is a hard error by design, so this is a cheap and total check: one
    # tiny calibrated cell, and the engine says `match: hit` or the process
    # fails.
    check_path = f'{out}/plan-hit-check.json'
    check_command = [
        lib.BIN, '--cell=cutclose', f'--request={request}',
        f'--edge={lib.EDGE_MM}', f'--pair={lib.PAIR_MM}', '--mode=calibrated',
        f'--plan={plan_path}', '--currency=U0', '--wall=0.05',
        f'--workers={WORKERS}', f'--seed={SEED}']
    check, status, stderr = run(check_command, check_path)
    spent = (check.get('schedule') or {}).get('calibratedPlan') or {}
    document['planHitCheck'] = {
        'path': check_path,
        'sourceSha256': sha256_of(check_path),
        'command': check_command,
        'exit': status,
        'stderr': stderr,
        'match': spent.get('match'),
        'summary': spent.get('summary'),
        'planSha256AsRead': spent.get('sourceSha256'),
    }
    document['PLAN_WRITTEN'] = bool(status == 0 and spent.get('match') == 'hit'
                                    and spent.get('sourceSha256')
                                    == document['planSha256'])
    print(json.dumps(document, indent=1))
    env_out = os.environ.get('ICS_OUT')
    if env_out:
        os.makedirs(env_out, exist_ok=True)
        with open(f'{env_out}/budget.json', 'w') as handle:
            json.dump(document, handle, indent=1)
    return 0 if document['PLAN_WRITTEN'] else 2


if __name__ == '__main__':
    sys.exit(main())
