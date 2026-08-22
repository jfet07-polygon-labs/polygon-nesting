#!/usr/bin/env python3
"""The pre-committed promotion rule, evaluated clause by clause.

    gateverdict.py MATCHEDJSON ARM CONTROL [OUT.json] [PUBLICATIONS.json]

Sol review 12 §3.2, as Grok review 7 §2 kept it unmodified:

> Promote only on **>=8/12 paired wins** and **>=1 mm median improvement** vs
> the miter control, at **equal operator wall**, with **<=1.25x** per-
> confirmation overhead, and **every publication passing the intact material
> validator**. Kill immediately on **any false accept**, or if new admissions
> stay around contact-block's 0.506 mm depth ceiling against m34's 1.104 mm.

Each clause is reported separately and none is allowed to stand in for another.

# The three readings, and why the wall one is interpolated

The engine is deterministic in the seed and the work cap, so a cell's depth is
exact and needs no replicas; only the wall varies between two runs of one cell.
That makes the *equal-work* reading immune to this box's pollution and makes it
the reading a reader should check first.

Sol asked for equal **wall**, and on a shared box the honest way to get there is
not to declare two runs equal-wall but to measure each arm's own
depth-against-wall curve and read both arms at the same point on it. So for each
reference budget, the control's own operator wall at that budget is the target,
and the arm's depth at that wall is interpolated from the arm's own ladder -
piecewise linear, clamped at the ends, with every clamped cell flagged. An arm
that is cheaper per confirmation buys more work for the same wall, and that is
exactly the advantage the equal-wall reading is supposed to credit it with.

`operatorWallSeconds` is the benchmark's own `medianElapsedMs`: the measured
stream, which excludes process start-up and request loading. `processWallSeconds`
is reported beside it and used for nothing.
"""
import json
import statistics
import sys

# The two numbers the kill's depth-class clause is written against.
CONTACT_BLOCK_CLASS_MM = 0.506
M34_CLASS_MM = 1.104
WIN_THRESHOLD = 8
MEDIAN_IMPROVEMENT_MM = 1.0
OVERHEAD_CEILING = 1.25
# A paired difference this small is the same layout, not a win: the corpus is
# quoted to 1e-4 mm and the engine's own canonical grid step is 1 micrometre.
TIE_EPSILON_MM = 1e-9


def interpolate(curve, wall):
    """Depth at `wall` on a (wall, depth) ladder. Returns (depth, clamped)."""
    points = sorted(curve)
    if not points:
        return None, True
    if wall <= points[0][0]:
        return points[0][1], wall < points[0][0]
    if wall >= points[-1][0]:
        return points[-1][1], wall > points[-1][0]
    for (x0, y0), (x1, y1) in zip(points, points[1:]):
        if x0 <= wall <= x1:
            if x1 == x0:
                return min(y0, y1), False
            share = (wall - x0) / (x1 - x0)
            return y0 + share * (y1 - y0), False
    return points[-1][1], True


def main():
    matched = json.load(open(sys.argv[1]))
    arm_name, control_name = sys.argv[2], sys.argv[3]
    out_path = sys.argv[4] if len(sys.argv) > 4 else None
    publications = json.load(open(sys.argv[5])) if len(sys.argv) > 5 else None
    works = matched['works']

    equal_work = {}
    per_seed_curves = {}
    overheads = []
    aborts = {arm_name: 0, control_name: 0}
    contract_failures = []
    exact_failures = []
    arm_label_failures = []
    for cell in matched['cells']:
        seed = cell['seed']
        curves = {arm_name: [], control_name: []}
        for work in works:
            rows = {}
            for name in (arm_name, control_name):
                row = cell['arms'].get(f'{name}:{work}')
                if row is None:
                    continue
                rows[name] = row
                if not row.get('armReportedCorrectly'):
                    arm_label_failures.append(f'seed{seed} {name}:{work}')
                if row.get('contractValid') is not True:
                    contract_failures.append(f'seed{seed} {name}:{work}')
                if row.get('exactValid') is not True:
                    exact_failures.append(f'seed{seed} {name}:{work}')
                if not (row.get('schedule_confirmationsAttempted') or 0):
                    aborts[name] += 1
                wall = row.get('operatorWallSeconds')
                depth = row.get('rawSourceDepthMm')
                if wall is not None and depth is not None:
                    curves[name].append((wall, depth))
            if len(rows) == 2:
                arm, control = rows[arm_name], rows[control_name]
                diff = arm['rawSourceDepthMm'] - control['rawSourceDepthMm']
                equal_work.setdefault(work, []).append({
                    'seed': seed,
                    'parentMm': cell['parentRawDepthMm'],
                    'armDepthMm': arm['rawSourceDepthMm'],
                    'controlDepthMm': control['rawSourceDepthMm'],
                    'armMinusControlMm': diff,
                    'armDeltaVsParentMm': arm['deltaVsParentMm'],
                    'controlDeltaVsParentMm': control['deltaVsParentMm'],
                    'armOperatorWallSeconds': arm.get('operatorWallSeconds'),
                    'controlOperatorWallSeconds':
                        control.get('operatorWallSeconds'),
                    'armProcessWorkUnits': arm.get('processWorkUnits'),
                    'controlProcessWorkUnits': control.get('processWorkUnits'),
                    'armConfirmations':
                        arm.get('schedule_confirmationsAttempted'),
                    'controlConfirmations':
                        control.get('schedule_confirmationsAttempted'),
                    'sameFingerprint':
                        arm.get('fingerprint') == control.get('fingerprint'),
                    'sameStepDigest':
                        arm.get('schedule_stepDigest')
                        == control.get('schedule_stepDigest'),
                })
                a_ms, c_ms = (arm.get('msPerConfirmation'),
                              control.get('msPerConfirmation'))
                if a_ms and c_ms:
                    overheads.append({'seed': seed, 'work': work,
                                      'armMsPerConfirmation': a_ms,
                                      'controlMsPerConfirmation': c_ms,
                                      'ratio': a_ms / c_ms})
        per_seed_curves[seed] = curves

    equal_wall = {}
    for work in works:
        rows = []
        for cell in matched['cells']:
            seed = cell['seed']
            control = cell['arms'].get(f'{control_name}:{work}')
            if control is None or control.get('operatorWallSeconds') is None:
                continue
            target = control['operatorWallSeconds']
            depth, clamped = interpolate(per_seed_curves[seed][arm_name],
                                         target)
            if depth is None:
                continue
            # The same reading with no interpolation in it: the best depth the
            # arm was *measured* reaching inside the control's wall. A step
            # function read at a rung it actually ran, so it can only understate
            # the arm - which is the direction a promotion gate should err in.
            within = [d for w, d in per_seed_curves[seed][arm_name]
                      if w <= target]
            achieved = min(within) if within else None
            rows.append({
                'seed': seed,
                'targetOperatorWallSeconds': target,
                'controlDepthMm': control['rawSourceDepthMm'],
                'armDepthAtControlWallMm': depth,
                'armMinusControlMm': depth - control['rawSourceDepthMm'],
                'armAchievedDepthWithinControlWallMm': achieved,
                'armAchievedMinusControlMm':
                    (achieved - control['rawSourceDepthMm'])
                    if achieved is not None else None,
                'interpolationClamped': clamped,
                'armLadder': sorted(per_seed_curves[seed][arm_name]),
            })
        equal_wall[work] = rows

    def stats(rows, key='armMinusControlMm'):
        diffs = [r[key] for r in rows]
        if not diffs:
            return None
        return {
            'n': len(diffs),
            # A win is the arm ending strictly shallower than the control.
            'armWins': sum(1 for d in diffs if d < -TIE_EPSILON_MM),
            'controlWins': sum(1 for d in diffs if d > TIE_EPSILON_MM),
            'ties': sum(1 for d in diffs if abs(d) <= TIE_EPSILON_MM),
            'medianArmMinusControlMm': statistics.median(diffs),
            'medianImprovementMm': -statistics.median(diffs),
            'rangeMm': [min(diffs), max(diffs)],
        }

    ratios = [o['ratio'] for o in overheads]
    result = {
        'arm': arm_name,
        'control': control_name,
        'works': works,
        'parents': len(matched['cells']),
        'equalWork': {str(w): {'rows': equal_work.get(w, []),
                               'stats': stats(equal_work.get(w, []))}
                      for w in works},
        'equalOperatorWall': {
            str(w): {'rows': equal_wall.get(w, []),
                     'stats': stats(equal_wall.get(w, [])),
                     'statsNoInterpolation': stats(
                         [r for r in equal_wall.get(w, [])
                          if r['armAchievedMinusControlMm'] is not None],
                         'armAchievedMinusControlMm')}
            for w in works},
        'perConfirmationOverhead': {
            'cells': len(ratios),
            'median': statistics.median(ratios) if ratios else None,
            'max': max(ratios) if ratios else None,
            'min': min(ratios) if ratios else None,
            'rows': overheads,
        },
        'aborts': aborts,
        'contractValidFailures': contract_failures,
        'exactValidFailures': exact_failures,
        'armLabelFailures': arm_label_failures,
    }

    if publications is not None:
        rows = publications.get('rows') or []
        new_admissions = [r for r in rows
                          if r.get('unionAccepts') and not r.get('miterAccepts')]
        result['publicationAudit'] = {
            'layouts': len(rows),
            'unionAcceptsAll': all(r.get('unionAccepts') for r in rows),
            'miterRefusesUnionAccepts': len(new_admissions),
            'contractRefusals': [r['label'] for r in rows
                                 if not r.get('unionAccepts')],
            'newAdmissions': new_admissions,
        }

    # The clauses. Read at the reference budget whose control wall is nearest
    # the ten-second contract, and at every budget, because a rule quoted at one
    # cell is a cell, not a rule.
    clauses = {}
    for work in works:
        wall_stats = result['equalOperatorWall'][str(work)]['stats']
        work_stats = result['equalWork'][str(work)]['stats']
        if wall_stats is None:
            continue
        no_interp = result['equalOperatorWall'][str(work)][
            'statsNoInterpolation']
        clauses[str(work)] = {
            'equalWallWins': wall_stats['armWins'],
            'equalWallWinsRequired': WIN_THRESHOLD,
            'equalWallWinClausePasses':
                wall_stats['armWins'] >= WIN_THRESHOLD,
            'equalWallMedianImprovementMm': wall_stats['medianImprovementMm'],
            'medianImprovementClausePasses':
                wall_stats['medianImprovementMm'] >= MEDIAN_IMPROVEMENT_MM,
            'equalWallWinsNoInterpolation':
                no_interp['armWins'] if no_interp else None,
            'equalWallMedianImprovementNoInterpolationMm':
                no_interp['medianImprovementMm'] if no_interp else None,
            'equalWorkWins': work_stats['armWins'] if work_stats else None,
            'equalWorkMedianImprovementMm':
                work_stats['medianImprovementMm'] if work_stats else None,
        }
    overhead_pass = (ratios and max(ratios) <= OVERHEAD_CEILING)
    contract_pass = not contract_failures
    label_pass = not arm_label_failures
    any_budget_passes = any(
        c['equalWallWinClausePasses'] and c['medianImprovementClausePasses']
        for c in clauses.values())
    result['preCommittedRule'] = {
        'clausesPerBudget': clauses,
        'perConfirmationOverheadClausePasses': bool(overhead_pass),
        'perConfirmationOverheadMax': max(ratios) if ratios else None,
        'everyPublicationPassesUntouchedContractValidator': contract_pass,
        'everyArmRunReportedItsArm': label_pass,
        'anyBudgetSatisfiesBothQualityClauses': any_budget_passes,
        'VERDICT': ('PROMOTE' if (any_budget_passes and overhead_pass
                                  and contract_pass and label_pass)
                    else 'DO-NOT-PROMOTE'),
        'failingClauses': [
            name for name, ok in (
                ('>=8/12 paired wins at equal operator wall AND >=1 mm median '
                 'improvement (at any measured budget)', any_budget_passes),
                ('per-confirmation overhead <=1.25x', bool(overhead_pass)),
                ('every publication passes the untouched material contract '
                 'validator', contract_pass),
                ('every armed run reported its own arm', label_pass),
            ) if not ok],
        'depthClassNote': {
            'contactBlockClassMm': CONTACT_BLOCK_CLASS_MM,
            'm34ClassMm': M34_CLASS_MM,
        },
    }
    text = json.dumps(result, indent=1)
    if out_path:
        open(out_path, 'w').write(text)
    print(json.dumps({
        'preCommittedRule': result['preCommittedRule'],
        'perConfirmationOverhead': {
            k: v for k, v in result['perConfirmationOverhead'].items()
            if k != 'rows'},
        'aborts': result['aborts'],
        'publicationAudit': {
            k: v for k, v in (result.get('publicationAudit') or {}).items()
            if k != 'newAdmissions'},
    }, indent=1))


if __name__ == '__main__':
    main()
