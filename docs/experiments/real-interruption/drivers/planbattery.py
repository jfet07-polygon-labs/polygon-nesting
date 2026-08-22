#!/usr/bin/env python3
"""The plan battery: does a wall target, spent as work, land and reproduce?

    python3 planbattery.py OUTDIR BINARY REQUEST TARGET_MS SEEDS ROUNDS \\
        [ARMS] [EXTRA]

Three arms, all on one binary in one window, interleaved by round so no arm
always runs first into a cold cache:

    plan        `plan=<ms>` with quantisation on  - the shipping mode
    planraw     `plan=<ms>,planq=1`               - quantisation off
    wall        `wall=<ms>`                       - the incumbent baseline

Two questions, and they are not the same question:

  **Does it land?** Process wall p50/p95/max against the target, and the count
  of runs over it. The `wall` arm answers this trivially by construction and is
  here for the depth comparison, not for the wall one.

  **Does it reproduce?** Per seed, across the rounds: how many distinct plans
  (`portfolio.plan.units`) were chosen, how many distinct depths came out, and
  how many distinct whole-document digests. The digest strips the clock -
  including `planCalibration`, which is *supposed* to differ every run - so a
  digest split is a split in the search and not in the instrument. `plan.units`
  stays in the digest deliberately: two processes that chose different plans
  ran different searches and the digest must say so.

The `planraw` arm exists because the honest form of this round's claim is a
trade and not a win. Quantisation is what makes two processes agree on a plan;
it is also what throws budget away. Running both ends measures the price.
"""
import hashlib
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

# Stripped from both sides of every digest comparison. `planCalibration` is the
# plan's clock half and differs every run by construction (that is what the
# split in `portfolio_report_json` is for); the rest is the inherited list.
VOLATILE = {
    'executableSha256', 'engineWorktreeStatus', 'engineWorktreeDirty',
    'engineCommit', 'relevantSourceTreeSha256', 'rustflags', 'buildProfile',
    'scheduleSlice', 'cpuModel', 'actualThreads', 'requestedThreads',
    'occupancyOverTime', 'seconds', 'planCalibration',
    # The tranche's clock half, for exactly the reason `planCalibration` is
    # stripped: `tranches` - the units each re-plan installed - stays in the
    # digest, because two processes that took different tranches ran different
    # searches and the digest must say so.
    'trancheCalibration',
}
VOLATILE_SUFFIXES = ('Seconds', 'Ms')


def volatile(key):
    return key in VOLATILE or key.endswith(VOLATILE_SUFFIXES)


def strip(node):
    """Drops the volatile keys, and **also drops any key whose value is null**.

    The second rule is what lets a binary that emits `"plan": null` on a
    non-plan run be compared against one that does not emit the key at all. A
    document with a null field and a document without the field are the same
    document in every sense this campaign compares them for; without this rule
    every cross-binary comparison in `armgate.py` would fail on the one key
    guaranteed to differ between a build that has the plan mode and one that
    predates it, which is exactly the failure mode `gatelib.VOLATILE`'s comment
    records for `executableSha256`.
    """
    if isinstance(node, dict):
        return {k: strip(v) for k, v in node.items()
                if not volatile(k) and v is not None}
    if isinstance(node, list):
        return [strip(v) for v in node]
    return node


def digest(doc):
    return hashlib.sha256(
        json.dumps(strip(doc), sort_keys=True).encode()).hexdigest()[:16]


def percentile(values, q):
    ordered = sorted(values)
    rank = max(1, min(len(ordered), int(q * len(ordered) + 0.9999999)))
    return ordered[rank - 1]


ARM_SPECS = {
    'plan': ('plan', '{target}', ''),
    'planraw': ('plan', '{target}', 'planq=1'),
    'wall': ('wall', '{target}', ''),
    # This round's arm: the same wall target, aimed at in tranches priced from
    # the rate this run measured rather than from a constant.
    'replan': ('plan', '{target}', 'replan=1'),
    # `docs/experiments/robust-plan/`'s three candidates for the load fragility,
    # all against the same `plan` incumbent above.
    #
    #   probe  the max-of-k bucket estimate: still a clock reading, but the
    #          least-loaded one this run can see
    #   cal    a persisted calibration keyed on `probe_work_units`, which is a
    #          counter - the clock leaves the decision entirely
    #   calprobe  both, which is the shipping shape: the file decides, the
    #          bucket estimate is what a calibration pass writes into it and
    #          what a file miss falls back to
    'probe': ('plan', '{target}', 'planprobe=8'),
    'callive': ('plan', '{target}', 'plancal={live}'),
    'calprobe': ('plan', '{target}', 'planprobe=8,plancal={probe}'),
    # The same file with the headroom budgeted for the load rather than for a
    # quiet box. A work-denominated budget converts box load into *wall* exactly
    # as a wall budget converts it into *depth*; a fixed plan therefore cannot
    # honour a wall on a box whose speed it does not know, and no calibration
    # changes that. What the file does change is that the choice can be made
    # once, deterministically, instead of being made by whichever clock reading
    # the probe happened to catch. `planhead` is the dial and this arm is its
    # price.
    'calhead': ('plan', '{target}', 'plancal={live},planhead=0.85'),
    # The two calibration passes. Never measured; they write the files.
    #
    # They are two files and not one because the min rule stores whatever
    # estimate the pass itself made, and the two passes make different ones:
    # without `planprobe` a pass stores the least-loaded *whole-phase* reading,
    # which is the same quantity the shipping `plan` arm divides by - so the
    # `callive` arm is the incumbent's own budget, made deterministic and
    # nothing else. With `planprobe` it stores the least-loaded *bucket*
    # estimate, which is systematically shorter, so the `calprobe` arm buys a
    # rung more. Separating them is what lets the round say which half of any
    # difference is reproducibility and which half is budget.
    'calwritelive': ('plan', '{target}', 'plancal={live},plancalwrite=1'),
    'calwriteprobe': ('plan', '{target}',
                      'planprobe=8,plancal={probe},plancalwrite=1'),
}

# The two calibration files every `{live}` / `{probe}` above is substituted
# with. Paths and not defaults, because a battery that silently calibrated
# itself would be measuring the order its own rounds happened to run in.
CAL_LIVE = os.environ.get('PLAN_CAL_LIVE',
                          '/var/lib/t3/tmp/realint/cal/live.json')
CAL_PROBE = os.environ.get('PLAN_CAL_PROBE',
                           '/var/lib/t3/tmp/realint/cal/probe.json')


def main():
    outdir, binary, request, target_ms = sys.argv[1:5]
    seeds = [int(v) for v in sys.argv[5].split(',')]
    rounds = int(sys.argv[6])
    arms = (sys.argv[7].split(',') if len(sys.argv) > 7
            else ['plan', 'planraw', 'wall'])
    extra = sys.argv[8] if len(sys.argv) > 8 else ''
    target_s = int(target_ms) / 1000.0
    os.makedirs(outdir, exist_ok=True)
    rows = []
    for rnd in range(rounds):
        # Arm order rotated by round, for the reason every battery in this
        # campaign rotates it.
        order = arms[rnd % len(arms):] + arms[:rnd % len(arms)]
        for arm in order:
            key, value, arm_extra = ARM_SPECS[arm]
            arm_extra = arm_extra.format(live=CAL_LIVE, probe=CAL_PROBE)
            pieces = [p for p in (arm_extra, extra) if p]
            for seed in seeds:
                spec = runlib.spec_for(seed, key,
                                       value.format(target=target_ms), True,
                                       ','.join(pieces))
                tag = f'{arm}-s{seed}-r{rnd}'
                doc, wall, err = runlib.run(binary, request, seed, spec,
                                            f'{outdir}/{tag}.json')
                portfolio = doc.get('portfolio') or {}
                if not portfolio:
                    rows.append({'tag': tag, 'arm': arm, 'seed': seed,
                                 'round': rnd, 'error': err[-300:]})
                    print(f'{tag}: FAILED {err[-200:]}', flush=True)
                    continue
                plan = portfolio.get('plan') or {}
                row = {
                    'tag': tag, 'arm': arm, 'seed': seed, 'round': rnd,
                    'spec': spec,
                    'processWallSeconds': wall,
                    'overran': wall > target_s,
                    'rawDepthMm': portfolio['incumbent']['rawDepthMm'],
                    'planUnits': plan.get('units'),
                    'planRung': plan.get('rung'),
                    'coordinatorWorkUnits': portfolio['workUnits'],
                    'digest': digest(doc),
                }
                tranches = portfolio.get('tranches') or []
                row['tranches'] = len(tranches)
                row['trancheUnits'] = [t['units'] for t in tranches]
                # The budget actually in force at the end: the last tranche's
                # units, or the plan's when no tranche was taken. This is the
                # number a caller replays with `work=`.
                row['finalUnits'] = (tranches[-1]['units'] if tranches
                                     else plan.get('units'))
                cal = portfolio.get('planCalibration') or {}
                row['probeSeconds'] = cal.get('probeSeconds')
                row['rawUnits'] = cal.get('rawUnits')
                # This round's two: what the arithmetic actually divided by, and
                # where it came from. `calibrationSource` is in the deterministic
                # half of the document on purpose, so a run that fell back is a
                # run whose digest says it fell back.
                row['probeEffectiveSeconds'] = cal.get('probeEffectiveSeconds')
                row['probeSamples'] = cal.get('probeSamples')
                row['calibrationSource'] = plan.get('calibrationSource') \
                    or 'live'
                rows.append(row)
                print(f'{tag}: wall={wall:6.3f} depth={row["rawDepthMm"]:.3f} '
                      f'units={row["planUnits"]}->{row["finalUnits"]} '
                      f'tr={row["tranches"]} dg={row["digest"]}', flush=True)

    summary = {'binary': binary, 'request': request, 'targetMs': target_ms,
               'targetSeconds': target_s, 'seeds': seeds, 'rounds': rounds,
               'arms': arms, 'extra': extra, 'rows': rows, 'byArm': {}}
    for arm in arms:
        good = [r for r in rows if r.get('arm') == arm and 'error' not in r]
        if not good:
            continue
        walls = [r['processWallSeconds'] for r in good]
        block = {
            'n': len(good),
            'wallP50': percentile(walls, 0.50),
            'wallP95': percentile(walls, 0.95),
            'wallMax': max(walls), 'wallMin': min(walls),
            'overruns': sum(1 for r in good if r['overran']),
            'perSeed': {},
        }
        for seed in seeds:
            cell = [r for r in good if r['seed'] == seed]
            if not cell:
                continue
            cw = [r['processWallSeconds'] for r in cell]
            units = sorted({r['finalUnits'] for r in cell})
            first_units = sorted({r['planUnits'] for r in cell})
            tranche_counts = sorted({r['tranches'] for r in cell})
            depths = sorted({r['rawDepthMm'] for r in cell})
            digests = sorted({r['digest'] for r in cell})
            block['perSeed'][str(seed)] = {
                'n': len(cell),
                'wallP50': percentile(cw, 0.50), 'wallP95': percentile(cw, 0.95),
                'wallMax': max(cw), 'wallMin': min(cw),
                'overruns': sum(1 for r in cell if r['overran']),
                'distinctFinalUnits': units,
                'distinctFirstPlanUnits': first_units,
                'distinctTrancheCounts': tranche_counts,
                'distinctDepthsMm': depths,
                'distinctDigests': digests,
                'planStable': len(units) == 1,
                'depthStable': len(depths) == 1,
                'documentStable': len(digests) == 1,
                'depthMedianMm': statistics.median(
                    r['rawDepthMm'] for r in cell),
                # The modal depth's share: "identical per seed" is a yes/no,
                # and when it is no the interesting number is how often.
                'modalDepthShare': max(
                    sum(1 for r in cell if r['rawDepthMm'] == d)
                    for d in depths) / len(cell),
                # Where each run's probe wall came from. A cell that is not
                # entirely `file` is a cell whose determinism claim has a hole in
                # it, and the hole has to be countable rather than argued about.
                'calibrationSources': {
                    source: sum(1 for r in cell
                                if r.get('calibrationSource') == source)
                    for source in sorted({r.get('calibrationSource')
                                          for r in cell})
                },
                'probeSecondsMin': min(
                    (r['probeSeconds'] for r in cell
                     if r.get('probeSeconds') is not None), default=None),
                'probeSecondsMax': max(
                    (r['probeSeconds'] for r in cell
                     if r.get('probeSeconds') is not None), default=None),
                'probeEffectiveSecondsDistinct': sorted({
                    r.get('probeEffectiveSeconds') for r in cell}),
            }
        block['allSeedsPlanStable'] = all(
            v['planStable'] for v in block['perSeed'].values())
        block['allSeedsDocumentStable'] = all(
            v['documentStable'] for v in block['perSeed'].values())
        block['seedMedianOfMedians'] = statistics.median(
            v['depthMedianMm'] for v in block['perSeed'].values())
        summary['byArm'][arm] = block
    loads = [row['before'] for row in runlib.LOAD
             if row['before'] is not None]
    summary['boxLoad'] = {
        'n': len(loads),
        'min': min(loads) if loads else None,
        'median': statistics.median(loads) if loads else None,
        'max': max(loads) if loads else None,
    }
    json.dump(summary, open(f'{outdir}/planbattery.json', 'w'), indent=1)
    print(json.dumps(summary['byArm'], indent=1))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
