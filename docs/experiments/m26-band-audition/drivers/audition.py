#!/usr/bin/env python3
"""Kimi review 1's band audition: the never-gated m26 short ladder at 171-179.

    audition.py OUTDIR BINARY PARENTSJSON WORKUNITS [ARMS] [ALLOWANCE]

`ARMS` is a comma-separated subset of `m34,m26rung,m26drop1`; the default is
`m34,m26rung`, which is the gate. Every arm starts from the same pinned parent
at the same seed and carries `POLYGON_NESTING_PROFILE=1`, because the x-axis is
a counter.

# The control

`m34` is `docs/experiments/contact-block/drivers/matched.py`'s `run_m34`. What
is verbatim is everything that decides what gets measured: the
`past=1,rollback=0,work=W,lanes=1,pconfirm=0` spec string character for
character, the pinned positional tail through the shared `runlib.ARGS`, the
`POLYGON_NESTING_PROFILE=1`, the `target = parent - drop` construction and its
`.17g` formatting, the same `searchProfile` reader with
`processWorkUnits = candidateQueries + 5 * exactPairTests`, and the same
`rawSourceDepthMm` scoring with the parent as the floor.

What is *not* verbatim is the plumbing: the process launch is factored into
`run_mode` so the mode-26 arm can share it, the round's own environment names
are scrubbed, and the drop is `DROP_MM = 1.0` here against `matched.py`'s
`DEFAULT_DROP_MM = 0.3` - which the brief pre-committed and which `past=1`
makes a starting bound rather than a stop. A reader checking this against
`matched.py` should diff `run_m34` and `SPEC`, not the file.

# The arm, and how "capped at W" is honoured without touching the engine

Mode 26 has no work cap. It is not `POLYGON_NESTING_COMPRESSION_SCHEDULE`'s
`work=` - that spec is mode 34's alone - and Kimi's gate is explicitly "zero
engine surgery". So the cap is structural, and it is exact:

* `ladder_compression_bounds` (general_relaxed.rs:5994) sets
  `step_mm = max(span/8, parent*0.001)` and
  `steps = clamp(ceil(span/step_mm), 1, 8)`. At a 174 mm parent the
  `parent*0.001 = 0.174208` floor dominates for **every** drop from 0.175 mm to
  1.4 mm, so a drop of 0.3 mm and a drop of 1.0 mm produce the *same rung-1
  bound* and differ only in how many rungs follow (2 against 6). This is the
  plan's own reading of arm C: "the mode's own bounds function turns a 0.3 mm
  drop at a 174 mm parent into a 0.174208 mm step" and "**rung 1** publishes
  4.25 mm and 3.61 mm below its own requested bound"
  (`docs/next-generation-engine-plan.md:4380-4384`).
* One rung is 32,246,564 candidate queries + 5 x 233,445 exact pair tests =
  **33,413,789 work units**, the anatomy's own pinned equal-budget figure
  (`mode26-rung-anatomy/README.md` §3.4). W sits at that number, so "the
  drop-1.0 ladder truncated at W" *is* "the drop-1.0 ladder's first rung": the
  meter passes W inside rung 1 and a truncating cap would stop there.
* `m26rung` therefore asks for the largest drop that `ladder_compression_bounds`
  still turns into exactly one rung. `single_rung_target` below computes that
  drop in the engine's own f64 arithmetic, through the same 17-significant-digit
  decimal the CLI parses, and the driver **asserts** `stepsPlanned == 1` off the
  emitted document rather than trusting the derivation.
* `m26drop1` is the same ladder asked for the literal 1.0 mm drop and left
  uncapped. It is a secondary reading, priced per work unit, for the question
  "and if the cap were six times wider".

The arm's m31 tier needs no flag: `run_ladder_compression_arm` runs
`global_legalize` as repair tier four whenever tiers one to three produced
nothing (general_relaxed.rs:8539).

# The statistic

The anatomy measured 85.4% of mode-26 arms aborting on the rollback tracker's
0-6 f32-ulp disagreement and burning 75.5% of the wall (§1.4-1.5). Kimi prices
that failure *inside* the arm rather than around it: the published statistic is
**mm per coordinator work unit**, with mm per wall second secondary, and never
mm per arm. `aborted_by_rollback_disagreement` is a default-build field
(general_relaxed.rs:8354), so the abort census is counted explicitly here and
not inferred.
"""
import hashlib
import json
import math
import os
import statistics
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

# matched.py's spec string, character for character.
SPEC = 'past=1,rollback=0,work={work},lanes=1,pconfirm=0'
DROP_MM = 1.0
CONTRACTION_RATIO = 0.001   # COUPLED_SEPARATOR_CONTRACTION_RATIO
LADDER_STEPS = 8            # LADDER_COMPRESSION_STEPS
DEFAULT_ARMS = 'm34,m26rung'
ROUND_ENV = ('POLYGON_NESTING_CONTACT_BLOCK', 'POLYGON_NESTING_SE2_WITNESS',
             'POLYGON_NESTING_CONTINUOUS_ROTATION',
             'POLYGON_NESTING_SPARSE_ROTATION',
             'POLYGON_NESTING_COMPRESSION_SCHEDULE')


def population(doc):
    return ((doc.get('relaxedDiagnostics') or {})
            .get('coupledDynamicSeparator') or {}).get(
                'persistentVacancyPopulation')


def ladder_bounds(parent_depth_mm, final_bound_mm):
    """`ladder_compression_bounds`, in the engine's arithmetic."""
    span = parent_depth_mm - final_bound_mm
    floor = parent_depth_mm * CONTRACTION_RATIO
    step = max(span / LADDER_STEPS, floor)
    steps = min(max(int(math.ceil(span / step)), 1), LADDER_STEPS)
    return step, steps


def single_rung_target(parent_depth_mm):
    """The largest drop the bounds function still turns into exactly one rung.

    Returned as the 17-significant-digit decimal the CLI will parse, so the
    span the engine computes is the span computed here.

    **`parent_depth_mm` here must be the parent's `independentDepthMm`, not its
    `rawDepthMm`.** The ladder measures its parent with
    `coupled_independent_source_depth` (general_relaxed.rs:6105) and its whole
    bounds arithmetic runs off that number, which is the grid-snapped one; on
    seed 4 the two differ by 0.00047 mm, which is enough to plan a second rung.
    Deriving from the wrong one is not a rounding quibble - the first pass of
    this audition did it and four of the twelve cells silently ran two rungs.
    That the right one is exact is checked rather than assumed: `stepMm` in the
    emitted document equals `independentDepthMm * 0.001` bit for bit on all
    twelve cells.

    The remaining correction is one rounding: `parent - fl(parent - drop)` comes
    back a few ULPs *above* `drop`, so `ceil(span/step)` is 2 and the ladder
    plans a second rung ~3e-14 mm below the first. Walking the target up by
    single ULPs until the count is 1 costs that same ~3e-14 mm of bound -
    eleven orders below the 1 um canonical grid and thirteen below the 0.002 mm
    search allowance - and buys the exact rung the plan credits.
    """
    target = parent_depth_mm - parent_depth_mm * CONTRACTION_RATIO
    for _ in range(64):
        text = f'{target:.17g}'
        _, steps = ladder_bounds(parent_depth_mm, float(text))
        if steps == 1:
            return text
        target = math.nextafter(target, math.inf)
    raise RuntimeError(f'no single-rung drop for parent {parent_depth_mm!r}')


def profile_row(doc, wall):
    profile = (doc.get('searchProfile') or {}).get('counters') or {}
    queries = profile.get('candidateQueries', 0)
    tests = profile.get('exactPairTests', 0)
    return {
        'processCandidateQueries': queries,
        'processExactPairTests': tests,
        'processWorkUnits': queries + 5 * tests,
        'processWallSeconds': wall,
    }


def run_mode(binary, seed, fixture, mode, target_text, out_path, allowance,
             schedule_spec=None):
    """One process: the pinned positional tail, one mode, one target."""
    args = [a.format(seed=seed) for a in runlib.ARGS]
    command = ([binary, runlib.REQUESTS['mixed-61']] + args
               + [str(mode), fixture, target_text, '', allowance])
    env = dict(os.environ)
    env['POLYGON_NESTING_PROFILE'] = '1'
    for name in ROUND_ENV:
        env.pop(name, None)
    if schedule_spec is not None:
        env['POLYGON_NESTING_COMPRESSION_SCHEDULE'] = schedule_spec
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    started = time.monotonic()
    with open(out_path, 'w') as handle:
        proc = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                              check=False, env=env)
    wall = time.monotonic() - started
    try:
        doc = json.load(open(out_path))
    except json.JSONDecodeError:
        return None, wall, (proc.stderr or b'').decode()[-800:], proc.returncode
    return doc, wall, (proc.stderr or b'').decode()[-800:], proc.returncode


def run_m34(binary, seed, fixture, parent_depth, work, out_path, allowance):
    """matched.py's control arm, one line changed: the drop is a parameter."""
    target = parent_depth - DROP_MM
    doc, wall, err, code = run_mode(
        binary, seed, fixture, 34, f'{target:.17g}', out_path, allowance,
        schedule_spec=SPEC.format(work=int(work)))
    if doc is None:
        return {'error': err, 'exitCode': code}
    row = profile_row(doc, wall)
    row['exitCode'] = code
    row['targetDepthMm'] = target
    pop = population(doc)
    if pop is None:
        row['error'] = 'no population'
        return row
    row['exactValid'] = pop.get('exactValid')
    row['contractValid'] = pop.get('contractValid')
    row['rawSourceDepthMm'] = pop.get('rawSourceDepthMm')
    row['independentDepthMm'] = pop.get('independentDepthMm')
    row['fingerprint'] = pop.get('finalPlacementFingerprint')
    row['failureReason'] = pop.get('failureReason')
    schedule = pop.get('compressionSchedule') or {}
    row['scheduleWorkUnits'] = schedule.get('workUnits')
    row['confirmationsAttempted'] = schedule.get('confirmationsAttempted')
    row['confirmationsAccepted'] = schedule.get('confirmationsAccepted')
    row['stepsTaken'] = schedule.get('stepsTaken')
    return row


def run_m26(binary, seed, fixture, parent_depth, target_text, out_path,
            allowance, expect_steps=None):
    doc, wall, err, code = run_mode(
        binary, seed, fixture, 26, target_text, out_path, allowance)
    if doc is None:
        return {'error': err, 'exitCode': code}
    row = profile_row(doc, wall)
    row['exitCode'] = code
    row['targetDepthMm'] = float(target_text)
    pop = population(doc)
    if pop is None:
        row['error'] = 'no population'
        return row
    row['exactValid'] = pop.get('exactValid')
    row['contractValid'] = pop.get('contractValid')
    row['rawSourceDepthMm'] = pop.get('rawSourceDepthMm')
    row['independentDepthMm'] = pop.get('independentDepthMm')
    row['fingerprint'] = pop.get('finalPlacementFingerprint')
    row['failureReason'] = pop.get('failureReason')
    row['engineParentDepthMm'] = pop.get('parentIndependentDepthMm')
    ladder = pop.get('ladderCompression') or {}
    row['ladderStepMm'] = ladder.get('stepMm')
    row['stepsPlanned'] = ladder.get('stepsPlanned')
    row['stepsRun'] = ladder.get('stepsRun')
    row['publishedStep'] = ladder.get('publishedStep')
    row['publishedBoundMm'] = ladder.get('publishedBoundMm')
    row['finalBoundMm'] = ladder.get('finalBoundMm')
    steps = ladder.get('steps') or []
    row['rungBoundsMm'] = [s.get('boundMm') for s in steps]
    arms = [arm for step in steps for arm in (step.get('arms') or [])]
    row['armsRun'] = len(arms)
    row['armsAbortedByRollbackDisagreement'] = sum(
        1 for a in arms if a.get('abortedByRollbackDisagreement'))
    row['armsProducingNoState'] = sum(
        1 for a in arms if a.get('stateFingerprint') is None)
    row['armsExactValid'] = sum(1 for a in arms if a.get('exactValid'))
    row['armsLegalizedByTier'] = {}
    for arm in arms:
        for key, tier in (('microLegalizedDepthMm', 'microLegalization'),
                          ('replacementRepairedDepthMm', 'replacement'),
                          ('jointReplacedDepthMm', 'jointReplacement'),
                          ('globalLegalizedDepthMm', 'globalLegalization')):
            if arm.get(key) is not None:
                row['armsLegalizedByTier'][tier] = \
                    row['armsLegalizedByTier'].get(tier, 0) + 1
    row['publishedRepairTier'] = next(
        (s.get('publishedRepairTier') for s in steps
         if s.get('improvedPublication')), None)
    row['rollbackDisagreementsTolerated'] = sum(
        s.get('rollbackDisagreementsTolerated') or 0 for s in steps)
    if expect_steps is not None:
        row['stepsPlannedAsDerived'] = (row['stepsPlanned'] == expect_steps)
    return row


def summarize(cells, labels):
    summary = {}
    for label in labels:
        rows = [c['arms'][label] for c in cells
                if 'deltaVsParentMm' in c['arms'].get(label, {})]
        if not rows:
            continue
        deltas = [r['deltaVsParentMm'] for r in rows]
        works = [r.get('processWorkUnits') or 0 for r in rows]
        walls = [r.get('processWallSeconds') or 0 for r in rows]
        per_work = [d / w * 1e6 for d, w in zip(deltas, works) if w > 0]
        per_second = [d / w for d, w in zip(deltas, walls) if w > 0]
        entry = {
            'cells': len(rows),
            'medianDeltaMm': statistics.median(deltas),
            'meanDeltaMm': statistics.fmean(deltas),
            'minDeltaMm': min(deltas),
            'maxDeltaMm': max(deltas),
            'cellsMoved': sum(1 for d in deltas if d > 0),
            'medianWorkUnits': statistics.median(works),
            'totalWorkUnits': sum(works),
            'medianWallSeconds': statistics.median(walls),
            'totalWallSeconds': sum(walls),
            'medianMmPerMegaWork': (statistics.median(per_work)
                                    if per_work else None),
            'aggregateMmPerMegaWork': ((sum(deltas) / sum(works) * 1e6)
                                       if sum(works) else None),
            'medianMmPerSecond': (statistics.median(per_second)
                                  if per_second else None),
            'aggregateMmPerSecond': ((sum(deltas) / sum(walls))
                                     if sum(walls) else None),
            'deltasMm': deltas,
        }
        if any('armsRun' in r for r in rows):
            arms = sum(r.get('armsRun') or 0 for r in rows)
            aborts = sum(r.get('armsAbortedByRollbackDisagreement') or 0
                         for r in rows)
            entry['armsRun'] = arms
            entry['armsAbortedByRollbackDisagreement'] = aborts
            entry['abortShare'] = aborts / arms if arms else None
            entry['armsProducingNoState'] = sum(
                r.get('armsProducingNoState') or 0 for r in rows)
            entry['rungsRun'] = sum(r.get('stepsRun') or 0 for r in rows)
            entry['cellsPublishingBelowParent'] = sum(1 for d in deltas
                                                      if d > 0)
        summary[label] = entry
    return summary


def main():
    outdir, binary, parents_json, work_arg = sys.argv[1:5]
    # matched.py's own shape: several control budgets, so the comparison is
    # read off a curve rather than off one cell that happened to land where
    # the author wanted it.
    works = [int(w) for w in work_arg.split(',')]
    work = works[0]
    arms_wanted = (sys.argv[5] if len(sys.argv) > 5 else DEFAULT_ARMS).split(',')
    allowance = sys.argv[6] if len(sys.argv) > 6 else runlib.DEFAULT_ALLOWANCE
    parents = json.load(open(parents_json))['rows']
    os.makedirs(outdir, exist_ok=True)
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'parents': parents_json,
        'workUnits': works,
        'dropMm': DROP_MM,
        'arms': arms_wanted,
        'allowance': allowance,
        'controlSpecs': [SPEC.format(work=w) for w in works],
        'cells': [],
    }
    for parent in parents:
        seed = parent['seed']
        depth = parent['rawDepthMm']
        # The ladder's own parent measure; see `single_rung_target`.
        ladder_depth = parent['independentDepthMm']
        fixture = parent['fixture']
        rung_target = single_rung_target(ladder_depth)
        drop1_target = f'{depth - DROP_MM:.17g}'
        cell = {
            'seed': seed,
            'parentRawDepthMm': depth,
            'parentIndependentDepthMm': ladder_depth,
            'fixture': fixture,
            'singleRungTargetMm': float(rung_target),
            'singleRungDropMm': ladder_depth - float(rung_target),
            'drop1LadderStepsDerived': ladder_bounds(
                ladder_depth, float(drop1_target))[1],
            'drop1LadderStepMmDerived': ladder_bounds(
                ladder_depth, float(drop1_target))[0],
            'arms': {},
        }
        if 'm34' in arms_wanted:
            for budget in works:
                cell['arms'][f'm34:{budget}'] = run_m34(
                    binary, seed, fixture, depth, budget,
                    f'{outdir}/seed{seed}-m34-{budget}.json', allowance)
        if 'm26rung' in arms_wanted:
            cell['arms']['m26:1rung'] = run_m26(
                binary, seed, fixture, depth, rung_target,
                f'{outdir}/seed{seed}-m26-1rung.json', allowance,
                expect_steps=1)
        if 'm26drop1' in arms_wanted:
            cell['arms']['m26:drop1.0'] = run_m26(
                binary, seed, fixture, depth, drop1_target,
                f'{outdir}/seed{seed}-m26-drop1.json', allowance,
                expect_steps=cell['drop1LadderStepsDerived'])
        for row in cell['arms'].values():
            if row.get('rawSourceDepthMm') is None:
                row['rawSourceDepthMm'] = depth
            row['deltaVsParentMm'] = depth - row['rawSourceDepthMm']
        print(f'seed{seed} parent={depth:.4f} ' + ' '.join(
            f"{label}=({row.get('deltaVsParentMm', 0):.4f}mm,"
            f"{(row.get('processWorkUnits') or 0) / 1e6:.2f}Mw,"
            f"{row.get('processWallSeconds', 0):.1f}s)"
            for label, row in cell['arms'].items()), flush=True)
        result['cells'].append(cell)
        json.dump(result, open(f'{outdir}/audition.json', 'w'), indent=1)

    labels = list(result['cells'][0]['arms']) if result['cells'] else []
    result['summary'] = summarize(result['cells'], labels)
    json.dump(result, open(f'{outdir}/audition.json', 'w'), indent=1)
    print(json.dumps(result['summary'], indent=1))


if __name__ == '__main__':
    main()
