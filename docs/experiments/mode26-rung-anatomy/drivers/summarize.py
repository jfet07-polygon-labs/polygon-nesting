#!/usr/bin/env python3
"""Turns the profiled mode-26 ladder documents into the experiment's evidence.

    python3 summarize.py <rungdir> <evidence.json>

Three tables come out of this:

* the **rung table** - one row per rung, its wall time, how many arms it ran,
  and how each arm ended;
* the **arm-component table** - where one arm's wall time went, split into the
  clamped separator and the four repair tiers plus the three exact-tier
  confirmations;
* the **phase table** - the profiling leaf phases summed over every arm, in
  calls and nanoseconds per call.

Phase milliseconds are **thread** time (the separator runs eight workers), and
arm/rung/ladder milliseconds are **wall** time. The two are reported separately
and never added; `threadParallelism` is their ratio and is the only place they
are related.
"""
import json
import os
import re
import statistics
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

# The phases that enclose other phases. A share-of-leaf table must exclude
# them or the percentages sum past 100. This is `Phase::is_enclosing` in
# crates/polygon-nesting-core/src/profiling.rs.
ENCLOSING = {
    'publicationConfirm', 'scorePlacement', 'fullRescore', 'moveSweep',
    'auditorScore', 'vacancyProposals', 'vacancyExactRows',
}

COMPONENTS = [
    ('separatorMs', 'clamped mode-0 pipeline (relaxed epochs + coupled separator)'),
    ('depthMeasureMs', 'coupled_independent_source_depth on the arm state'),
    ('overlapCountMs', 'count_exact_overlap_pairs on the arm state'),
    ('exactValidateMs', 'validate_and_measure_placements on the arm state'),
    ('microLegalizationMs', 'repair tier 1: micro_legalize'),
    ('replacementRepairMs', 'repair tier 2: single-piece re-placement'),
    ('jointReplacementMs', 'repair tier 3: joint multi-piece re-placement'),
    ('globalLegalizationMs', 'repair tier 4: global program (mode 31)'),
]


def ladder_of(doc):
    pop = (doc.get('relaxedDiagnostics', {})
              .get('coupledDynamicSeparator', {})
              .get('persistentVacancyPopulation'))
    if pop is None:
        return None, None
    return pop, pop.get('ladderCompression')


def arm_fate(arm):
    if arm.get('exactValid'):
        return 'exactValid'
    if arm.get('abortedByRollbackDisagreement'):
        return 'rollbackAbort'
    if arm.get('failureReason') == \
            'clamped separator arm produced no usable complete state':
        return 'noCompleteState'
    if arm.get('convergedDepthMm') is not None:
        return 'terminalResidue'
    return 'other'


def f32_ulp_distance(first, second):
    """The gap between two readings in `f32` units in the last place.

    This is `pressure_ulp_distance` in general_relaxed.rs, reimplemented here
    so the abort census can say how far outside its budget each refused
    comparison actually was without adding a diagnostic to the engine.
    """
    bits = lambda value: struct.unpack('<I', struct.pack('<f', value))[0]
    return abs(bits(first) - bits(second))


ABORT = 'rollback tracker disagrees with a complete rescore'


def abort_row(arm):
    """The refused rollback comparison of one aborted arm, parsed."""
    reason = arm.get('separatorSkippedReason') or ''
    if ABORT not in reason:
        return None
    kind = ('incidentLoss' if 'incident loss' in reason
            else 'boundaryLoss' if 'boundary loss' in reason else 'other')
    match = re.search(r': ([0-9.eE+-]+) != ([0-9.eE+-]+)', reason)
    if match is None:
        return {'kind': kind}
    first, second = float(match.group(1)), float(match.group(2))
    return {
        'kind': kind, 'first': first, 'second': second,
        'f32UlpGap': f32_ulp_distance(first, second),
        'relativeGap': abs(first - second) / max(abs(first), 1e-30),
    }


def add_phases(into, phases):
    for name, row in (phases or {}).items():
        slot = into.setdefault(name, {'milliseconds': 0.0, 'calls': 0})
        slot['milliseconds'] += row['milliseconds']
        slot['calls'] += row['calls']


def add_counters(into, counters):
    for name, value in (counters or {}).items():
        into[name] = into.get(name, 0) + value


def main():
    rungdir, out = sys.argv[1], sys.argv[2]
    rows = json.load(open(f'{rungdir}/rows.json'))
    ladders, rungs, arms, aborts = [], [], [], []
    arm_phases, arm_counters = {}, {}
    for row in rows:
        doc = json.load(open(f"{rungdir}/{row['tag']}.json"))
        pop, ladder = ladder_of(doc)
        if ladder is None:
            continue
        anatomy = ladder['anatomy']
        wall_s = anatomy['wallMs'] / 1000.0
        counters = anatomy['counters'] or {}
        ladders.append({
            'tag': row['tag'], 'parent': row['parent'], 'dropMm': row['dropMm'],
            'seed': row['seed'],
            'processWallSeconds': row['processWallSeconds'],
            'engineSeconds': row['engineSeconds'],
            'ladderWallMs': anatomy['wallMs'],
            'ladderOrchestrationMs': anatomy['orchestrationMs'],
            'stepsPlanned': ladder['stepsPlanned'],
            'stepsRun': ladder['stepsRun'],
            'stepMm': ladder['stepMm'],
            'publishedStep': ladder.get('publishedStep'),
            'publishedRaw': pop.get('rawSourceDepthMm'),
            'parentRaw': row['parentRaw'],
            'armCount': sum(len(step['arms']) for step in ladder['steps']),
            # The inner-loop rate the ladder actually sustains, in the units
            # `docs/next-generation-engine-plan.md` quotes for the m22 stream:
            # candidate queries/s and effective piece moves/s, against the
            # ladder's own wall time.
            'candidateQueries': counters.get('candidateQueries'),
            'effectivePieceMoves': counters.get('effectivePieceMoves'),
            'fullRescores': counters.get('fullRescores'),
            'exactPairTests': counters.get('exactPairTests', 0),
            'collisionPolygonBuilds': counters.get('collisionPolygonBuilds', 0),
            'candidateQueriesPerSecond':
                (counters.get('candidateQueries', 0) / wall_s if wall_s else None),
            'effectiveMovesPerSecond':
                (counters.get('effectivePieceMoves', 0) / wall_s if wall_s else None),
            'phases': anatomy['phases'],
            'counters': counters,
        })
        for step in ladder['steps']:
            anatomy = step['anatomy']
            fates = {}
            for arm in step['arms']:
                fates[arm_fate(arm)] = fates.get(arm_fate(arm), 0) + 1
            rungs.append({
                'tag': row['tag'], 'step': step['step'],
                'boundMm': step['boundMm'],
                'wallMs': anatomy['wallMs'],
                'orchestrationMs': anatomy['orchestrationMs'],
                'arms': len(step['arms']),
                'attemptsRun': step['attemptsRun'],
                'roles': sorted({arm['role'] for arm in step['arms']}),
                'fates': fates,
                'improvedPublication': step['improvedPublication'],
                'chainAdvanced': step['chainAdvanced'],
                'rollbackDisagreementsTolerated':
                    step.get('rollbackDisagreementsTolerated', 0),
                'rollbackDisagreementMaxPressureUlps':
                    step.get('rollbackDisagreementMaxPressureUlps', 0),
            })
            for arm in step['arms']:
                anatomy = arm['anatomy']
                thread_ms = sum(
                    value['milliseconds']
                    for name, value in (anatomy['phases'] or {}).items()
                    if name not in ENCLOSING)
                arms.append({
                    'tag': row['tag'], 'step': step['step'],
                    'role': arm['role'], 'attempt': arm['attempt'],
                    'fate': arm_fate(arm),
                    'wallMs': anatomy['wallMs'],
                    'leafThreadMs': thread_ms,
                    'threadParallelism': (thread_ms / anatomy['wallMs']
                                          if anatomy['wallMs'] else None),
                    'targetsAttempted': arm['armTargetsAttempted'],
                    'targetsAccepted': arm['armTargetsAccepted'],
                    'epochsImproved': arm['epochsImproved'],
                    'rollbackDisagreementsTolerated':
                        arm.get('rollbackDisagreementsTolerated', 0),
                    **{key: anatomy[key] for key, _ in COMPONENTS},
                })
                add_phases(arm_phases, anatomy['phases'])
                add_counters(arm_counters, anatomy['counters'])
                abort = abort_row(arm)
                if abort is not None:
                    abort.update({'tag': ladders[-1]['tag'],
                                  'step': step['step'], 'role': arm['role'],
                                  'attempt': arm['attempt'],
                                  'wallMs': anatomy['wallMs']})
                    aborts.append(abort)

    def stat(values):
        values = [v for v in values if v is not None]
        if not values:
            return None
        return {'n': len(values), 'min': min(values), 'median':
                statistics.median(values), 'mean': statistics.fmean(values),
                'max': max(values), 'sum': sum(values)}

    by_fate = {}
    for arm in arms:
        by_fate.setdefault(arm['fate'], []).append(arm)

    leaf_total_ms = sum(row['milliseconds'] for name, row in arm_phases.items()
                        if name not in ENCLOSING)
    phase_table = []
    for name, row in sorted(arm_phases.items(),
                            key=lambda item: -item[1]['milliseconds']):
        phase_table.append({
            'phase': name,
            'enclosing': name in ENCLOSING,
            'threadMs': row['milliseconds'],
            'calls': row['calls'],
            'nsPerCall': (row['milliseconds'] * 1.0e6 / row['calls']
                          if row['calls'] else None),
            'leafSharePercent': (None if name in ENCLOSING else
                                 100.0 * row['milliseconds'] / leaf_total_ms),
        })

    component_table = []
    for key, description in COMPONENTS:
        values = [arm[key] for arm in arms]
        nonzero = [value for value in values if value > 0.0]
        component_table.append({
            'component': key, 'description': description,
            'totalMs': sum(values), 'armsWithAny': len(nonzero),
            'perArmWhenRun': stat(nonzero),
            'shareOfArmWallPercent':
                100.0 * sum(values) / sum(arm['wallMs'] for arm in arms),
        })

    evidence = {
        'ladders': ladders,
        'rungs': rungs,
        'arms': arms,
        'armWallMsByFate': {fate: stat([a['wallMs'] for a in group])
                            for fate, group in sorted(by_fate.items())},
        'armCountByFate': {fate: len(group)
                           for fate, group in sorted(by_fate.items())},
        'rungWallMs': stat([rung['wallMs'] for rung in rungs]),
        'rungOrchestrationMs': stat([rung['orchestrationMs'] for rung in rungs]),
        'armWallMs': stat([arm['wallMs'] for arm in arms]),
        'separatorShareOfArmPercent':
            100.0 * sum(arm['separatorMs'] for arm in arms)
            / sum(arm['wallMs'] for arm in arms),
        'aborts': aborts,
        'abortCensus': {
            'count': len(aborts),
            'byKind': {kind: sum(1 for row in aborts if row['kind'] == kind)
                       for kind in sorted({row['kind'] for row in aborts})},
            'f32UlpGap': stat([row.get('f32UlpGap') for row in aborts]),
            'relativeGap': stat([row.get('relativeGap') for row in aborts]),
            'wallMsBurned': sum(row['wallMs'] for row in aborts),
        } if aborts else None,
        'componentTable': component_table,
        'phaseTable': phase_table,
        'counterTotals': arm_counters,
        'leafThreadMsTotal': leaf_total_ms,
        'armWallMsTotal': sum(arm['wallMs'] for arm in arms),
        'rungWallMsTotal': sum(rung['wallMs'] for rung in rungs),
    }
    json.dump(evidence, open(out, 'w'), indent=1)
    print(json.dumps({
        'ladders': len(ladders), 'rungs': len(rungs), 'arms': len(arms),
        'armCountByFate': evidence['armCountByFate'],
        'rungWallMs': evidence['rungWallMs'],
        'armWallMs': evidence['armWallMs'],
        'separatorShareOfArmPercent': evidence['separatorShareOfArmPercent'],
    }, indent=1))


if __name__ == '__main__':
    main()
