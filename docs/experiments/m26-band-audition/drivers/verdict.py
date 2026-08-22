#!/usr/bin/env python3
"""The kill rule, applied.

    verdict.py OUTFILE REPLAYJSON ARMJSON LADDERJSON CONTROLJSON [CONTROLJSON ...]

Kimi review 1's rule, pre-committed before the battery ran and quoted here so
the code and the sentence can be diffed:

> **Kill**: se il braccio ladder non batte la mediana del controllo di >=1mm
> **o** non scende sotto il controllo su >=8/12 parent, m26 e tagliato dalla
> banda 10s con evidenza.

Both readings of the connective are evaluated and both are reported, because
the campaign brief transcribed it into English with `AND` between the two
negations, which is the *weaker* kill. `survivesStrict` is the governing
document's `o`: the arm survives only if it clears both clauses.
`survivesWeak` is the brief's `AND`: the arm survives if it clears either.

# The equal-work axis

Three columns are reported per arm and the verdict names which one it used:

* **`processWorkUnits`** - `candidateQueries + 5 * exactPairTests` off the
  process, matched.py's declared axis ("measured on the process rather than
  declared").
* **`operatorWorkUnits`** - the same meter minus the *harness floor*: what a
  mode-34 process burns when it refuses the mode outright and runs no search at
  all, measured per seed by `replay.py` (6.84M - 11.91M, median 8.97M). Both
  arms pay that floor for the same reason - phase 0 constructs before the deep
  operator is handed the pinned parent - so it is common mode, and leaving it in
  compresses a 10x work difference into a 1.4x one.
* **wall seconds** - the currency the ten-second contract is written in.

The control budget the kill rule is read at is chosen by one pre-registered
rule and no other: **the budget whose median operator work units is nearest the
arm's**. Every other budget's verdict is reported beside it precisely so the
choice can be checked for tuning.
"""
import json
import statistics
import sys


def load(path):
    return json.load(open(path))


def baseline_from_replay(replay):
    """The per-seed harness floor, from the refused-replay documents."""
    return {c['seed']: c['harnessFloorWorkUnits'] for c in replay['cells']}


def arm_rows(doc, label, baseline):
    rows = {}
    for cell in doc['cells']:
        arm = cell['arms'].get(label)
        if arm is None or 'deltaVsParentMm' not in arm:
            continue
        seed = cell['seed']
        process = arm.get('processWorkUnits') or 0
        rows[seed] = {
            'seed': seed,
            'parentRawDepthMm': cell['parentRawDepthMm'],
            'publishedRawDepthMm': arm['rawSourceDepthMm'],
            'deltaMm': arm['deltaVsParentMm'],
            'processWorkUnits': process,
            'operatorWorkUnits': process - baseline[seed],
            'wallSeconds': arm.get('processWallSeconds') or 0.0,
            'exactValid': arm.get('exactValid'),
            'contractValid': arm.get('contractValid'),
            'stepsRun': arm.get('stepsRun'),
            'armsRun': arm.get('armsRun'),
            'armsAbortedByRollbackDisagreement': arm.get(
                'armsAbortedByRollbackDisagreement'),
            'armsProducingNoState': arm.get('armsProducingNoState'),
            'armsExactValid': arm.get('armsExactValid'),
            'armsLegalizedByTier': arm.get('armsLegalizedByTier'),
        }
    return rows


def stats(rows):
    deltas = [r['deltaMm'] for r in rows.values()]
    process = [r['processWorkUnits'] for r in rows.values()]
    operator = [r['operatorWorkUnits'] for r in rows.values()]
    walls = [r['wallSeconds'] for r in rows.values()]
    entry = {
        'cells': len(rows),
        'medianDeltaMm': statistics.median(deltas),
        'meanDeltaMm': statistics.fmean(deltas),
        'minDeltaMm': min(deltas),
        'maxDeltaMm': max(deltas),
        'cellsMoved': sum(1 for d in deltas if d > 0),
        'medianProcessWorkUnits': statistics.median(process),
        'medianOperatorWorkUnits': statistics.median(operator),
        'totalOperatorWorkUnits': sum(operator),
        'medianWallSeconds': statistics.median(walls),
        'totalWallSeconds': sum(walls),
        # The published statistic Kimi names: mm per coordinator work unit,
        # aggregated over the battery so a cell that published nothing is
        # charged for the work it spent publishing nothing.
        'aggregateMmPerMegaOperatorWork': (sum(deltas) / sum(operator) * 1e6
                                           if sum(operator) else None),
        'aggregateMmPerMegaProcessWork': (sum(deltas) / sum(process) * 1e6
                                          if sum(process) else None),
        'aggregateMmPerWallSecond': (sum(deltas) / sum(walls)
                                     if sum(walls) else None),
        'medianMmPerMegaOperatorWork': statistics.median(
            [d / w * 1e6 for d, w in zip(deltas, operator) if w > 0]),
    }
    arms = [r['armsRun'] for r in rows.values() if r.get('armsRun') is not None]
    if arms:
        aborts = [r['armsAbortedByRollbackDisagreement'] or 0
                  for r in rows.values()]
        entry['armsRun'] = sum(arms)
        entry['armsAbortedByRollbackDisagreement'] = sum(aborts)
        entry['abortShare'] = sum(aborts) / sum(arms) if sum(arms) else None
        entry['armsProducingNoState'] = sum(
            r['armsProducingNoState'] or 0 for r in rows.values())
        entry['armsExactValid'] = sum(
            r['armsExactValid'] or 0 for r in rows.values())
        entry['rungsRun'] = sum(r['stepsRun'] or 0 for r in rows.values())
    return entry


def kill_rule(arm_rows_, control_rows, arm_stats, control_stats):
    seeds = sorted(set(arm_rows_) & set(control_rows))
    per_seed = []
    for seed in seeds:
        a, c = arm_rows_[seed], control_rows[seed]
        per_seed.append({
            'seed': seed,
            'armDeltaMm': a['deltaMm'],
            'controlDeltaMm': c['deltaMm'],
            'armPublishedMm': a['publishedRawDepthMm'],
            'controlPublishedMm': c['publishedRawDepthMm'],
            # "below the control" = a shallower published depth = a deeper cut.
            'armBelowControl': (a['publishedRawDepthMm']
                                < c['publishedRawDepthMm']),
            'armMinusControlMm': (a['publishedRawDepthMm']
                                  - c['publishedRawDepthMm']),
        })
    below = sum(1 for r in per_seed if r['armBelowControl'])
    margin = arm_stats['medianDeltaMm'] - control_stats['medianDeltaMm']
    clause_a = margin >= 1.0
    clause_b = below >= 8
    return {
        'cells': len(per_seed),
        'armMedianDeltaMm': arm_stats['medianDeltaMm'],
        'controlMedianDeltaMm': control_stats['medianDeltaMm'],
        'armMedianMinusControlMedianMm': margin,
        'clauseA_beatsControlMedianBy1mm': clause_a,
        'clauseB_belowControlOnAtLeast8of12': clause_b,
        'armBelowControlCells': below,
        'survivesStrict': bool(clause_a and clause_b),
        'survivesWeak': bool(clause_a or clause_b),
        'verdict': ('SURVIVES' if (clause_a and clause_b) else 'CUT'),
        'perSeed': per_seed,
    }


def main():
    outfile, replay_json, arm_json, ladder_json = sys.argv[1:5]
    control_jsons = sys.argv[5:]
    replay = load(replay_json)
    baseline = baseline_from_replay(replay)

    arm_doc = load(arm_json)
    arm = arm_rows(arm_doc, 'm26:1rung', baseline)
    arm_stats = stats(arm)

    ladder_doc = load(ladder_json)
    ladder = arm_rows(ladder_doc, 'm26:drop1.0', baseline)
    ladder_stats = stats(ladder)

    controls = {}
    for path in control_jsons:
        doc = load(path)
        for label in (doc['cells'][0]['arms'] if doc['cells'] else []):
            if not label.startswith('m34:'):
                continue
            rows = arm_rows(doc, label, baseline)
            controls[label] = {'rows': rows, 'stats': stats(rows),
                               'source': path}

    # The pre-registered choice: nearest median operator work to the arm's.
    target = arm_stats['medianOperatorWorkUnits']
    chosen = min(controls,
                 key=lambda k: abs(controls[k]['stats']
                                   ['medianOperatorWorkUnits'] - target))

    result = {
        'armLabel': 'm26:1rung',
        'ladderLabel': 'm26:drop1.0',
        'harnessFloorWorkUnits': baseline,
        'arm': arm_stats,
        'ladder': ladder_stats,
        'controls': {k: v['stats'] for k, v in controls.items()},
        'controlSources': {k: v['source'] for k, v in controls.items()},
        'workMatchTargetOperatorUnits': target,
        'chosenControl': chosen,
        'chosenControlRule': ('the control budget whose median operator work '
                              'units is nearest the arm\'s'),
        'killRule': kill_rule(arm, controls[chosen]['rows'],
                              arm_stats, controls[chosen]['stats']),
        'killRuleAtEveryControlBudget': {
            k: {kk: vv for kk, vv in
                kill_rule(arm, v['rows'], arm_stats, v['stats']).items()
                if kk != 'perSeed'}
            for k, v in controls.items()},
        'ladderKillRuleAtEveryControlBudget': {
            k: {kk: vv for kk, vv in
                kill_rule(ladder, v['rows'], ladder_stats, v['stats']).items()
                if kk != 'perSeed'}
            for k, v in controls.items()},
        'armPerSeed': list(arm.values()),
        'ladderPerSeed': list(ladder.values()),
        'controlPerSeed': {k: list(v['rows'].values())
                           for k, v in controls.items()},
    }
    result['VERDICT'] = result['killRule']['verdict']
    json.dump(result, open(outfile, 'w'), indent=1)
    print(json.dumps({
        'arm': {k: v for k, v in arm_stats.items()},
        'ladder': {k: v for k, v in ladder_stats.items()},
        'controls': result['controls'],
        'chosenControl': chosen,
        'killRule': {k: v for k, v in result['killRule'].items()
                     if k != 'perSeed'},
        'killRuleAtEveryControlBudget': result['killRuleAtEveryControlBudget'],
        'VERDICT': result['VERDICT'],
    }, indent=1))


if __name__ == '__main__':
    main()
