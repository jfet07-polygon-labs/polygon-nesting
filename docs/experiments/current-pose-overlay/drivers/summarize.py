#!/usr/bin/env python3
"""Reduces the campaign's per-run JSON dumps to the evidence table.

    python3 summarize.py CAMPAIGN_DIR OUT.json

Reads every `{parent}-{arm}.json` the campaign wrote, extracts exactly the
fields section 2 and 3 of the task ask for, and writes one compact JSON
document - the raw per-run dumps (searchProfile, full placements, ...) are
reproducible from `campaign.py` and are not themselves evidence.
"""
import glob
import json
import os
import statistics
import sys


def load(campaign_dir):
    rows = []
    for path in sorted(glob.glob(f'{campaign_dir}/*.json')):
        base = os.path.basename(path)
        if base == 'campaign.json':
            continue
        name, arm = base[:-5].rsplit('-', 1)
        doc = json.load(open(path))
        pop = ((doc.get('relaxedDiagnostics') or {})
               .get('coupledDynamicSeparator', {})
               .get('persistentVacancyPopulation'))
        sched = pop.get('compressionSchedule') if pop else None
        elapsed = doc.get('elapsedMs') or [None]
        wall_s = elapsed[0] / 1000.0 if elapsed[0] else None
        counters = (doc.get('searchProfile') or {}).get('counters') or {}
        row = {
            'parent': name,
            'arm': arm,
            'wallSeconds': wall_s,
            'processCandidateQueries': counters.get('candidateQueries', 0),
            'processExactPairTests': counters.get('exactPairTests', 0),
            'processQueriesPerSecond':
                (counters.get('candidateQueries', 0) / wall_s) if wall_s else None,
            'attempted': pop.get('attempted') if pop else None,
            'exactValid': pop.get('exactValid') if pop else None,
            'parentIndependentDepthMm': pop.get('parentIndependentDepthMm') if pop else None,
            'rawSourceDepthMm': pop.get('rawSourceDepthMm') if pop else None,
        }
        if sched:
            row.update({
                'entryBoundaryViolations': sched.get('parentBoundaryViolations'),
                'entryCollisionPairs': sched.get('parentCollisionPairs'),
                'entryProxyFeasible': sched.get('parentProxyFeasible'),
                'entryLoss': sched.get('parentEntryLoss'),
                'currentPoseOverlay': sched.get('currentPoseOverlay'),
                # Two counts, not one (Sol review 6 §2.4). `Entries` is a
                # catalogue size - `(geometry_class, angle, mirror)` keys, so
                # two instances of one class at one continuous pose collapse -
                # and `OffGridPieces` is the number of parent placements the
                # 2.5-degree snap would have moved. The v5 round reported the
                # first and called it the second.
                'currentPoseOverlayEntries': sched.get('currentPoseOverlayEntries'),
                'currentPoseOverlayOffGridPieces':
                    sched.get('currentPoseOverlayOffGridPieces'),
                # Sol review 6 §2.2's "measure the setup cost": the overlay's
                # own build-plus-install time, reported by the run.
                'currentPoseOverlaySetupMs':
                    sched.get('currentPoseOverlaySetupMs'),
                'confirmationsAttempted': sched.get('confirmationsAttempted'),
                'confirmationsAccepted': sched.get('confirmationsAccepted'),
                'rollbacks': sched.get('rollbacks'),
                'exitCause': sched.get('exitCause'),
                'stepsTaken': sched.get('stepsTaken'),
                'workUnits': sched.get('workUnits'),
            })
        row['published'] = bool(
            row['rawSourceDepthMm'] is not None
            and row['parentIndependentDepthMm'] is not None
            and row['rawSourceDepthMm'] < row['parentIndependentDepthMm']
        )
        rows.append(row)
    return rows


def pair(rows):
    by_parent = {}
    for r in rows:
        by_parent.setdefault(r['parent'], {})[r['arm']] = r
    return by_parent


def main():
    campaign_dir = sys.argv[1]
    out_path = sys.argv[2]
    rows = load(campaign_dir)
    by_parent = pair(rows)

    per_arm = {}
    for arm in ('grid', 'overlay'):
        arm_rows = [r for r in rows if r['arm'] == arm]
        qps = [r['processQueriesPerSecond'] for r in arm_rows if r['processQueriesPerSecond']]
        published_drop = sum(
            (r['parentIndependentDepthMm'] - r['rawSourceDepthMm'])
            for r in arm_rows if r['published']
        )
        setup = [r.get('currentPoseOverlaySetupMs') for r in arm_rows
                 if r.get('currentPoseOverlaySetupMs') is not None]
        per_arm[arm] = {
            'parents': len(arm_rows),
            'medianQueriesPerSecond': statistics.median(qps) if qps else None,
            'confirmationsAttemptedTotal':
                sum(r.get('confirmationsAttempted') or 0 for r in arm_rows),
            'confirmationsAcceptedTotal':
                sum(r.get('confirmationsAccepted') or 0 for r in arm_rows),
            'publications': sum(1 for r in arm_rows if r['published']),
            'sumDropOverPublishedMm': round(published_drop, 6),
            'rollbacksTotal': sum(r.get('rollbacks') or 0 for r in arm_rows),
            'exitCauses': sorted({r.get('exitCause') for r in arm_rows}),
            'overlaySetupMsMedian':
                statistics.median(setup) if setup else None,
            'overlaySetupMsMax': max(setup) if setup else None,
        }

    loss_deltas, viol_deltas, pair_deltas = [], [], []
    # The downstream number, paired. `rawSourceDepthMm` is what the arm
    # actually published (or the parent, when it published nothing), so a
    # negative delta is the overlay ending deeper than the grid on the same
    # parent at the same budget. Counting publications alone hides this: an arm
    # can publish on more parents and still end shallower on most of them.
    depth_deltas = []
    composability_flips = 0
    per_parent = []
    for name in sorted(by_parent):
        arms = by_parent[name]
        g, o = arms.get('grid'), arms.get('overlay')
        if not g or not o:
            continue
        loss_delta = o['entryLoss'] - g['entryLoss']
        viol_delta = o['entryBoundaryViolations'] - g['entryBoundaryViolations']
        pair_delta = o['entryCollisionPairs'] - g['entryCollisionPairs']
        loss_deltas.append(loss_delta)
        viol_deltas.append(viol_delta)
        pair_deltas.append(pair_delta)
        depth_delta = None
        if g['rawSourceDepthMm'] is not None and o['rawSourceDepthMm'] is not None:
            depth_delta = o['rawSourceDepthMm'] - g['rawSourceDepthMm']
            depth_deltas.append(depth_delta)
        flipped = (not g['entryProxyFeasible']) and o['entryProxyFeasible']
        composability_flips += int(flipped)
        per_parent.append({
            'parent': name,
            'currentPoseOverlayEntries': o['currentPoseOverlayEntries'],
            'currentPoseOverlayOffGridPieces':
                o.get('currentPoseOverlayOffGridPieces'),
            'currentPoseOverlaySetupMs': o.get('currentPoseOverlaySetupMs'),
            'grid': {k: g[k] for k in (
                'entryBoundaryViolations', 'entryCollisionPairs', 'entryProxyFeasible',
                'entryLoss', 'confirmationsAccepted', 'published', 'rawSourceDepthMm',
                'rollbacks', 'exitCause', 'stepsTaken', 'workUnits')},
            'overlay': {k: o[k] for k in (
                'entryBoundaryViolations', 'entryCollisionPairs', 'entryProxyFeasible',
                'entryLoss', 'confirmationsAccepted', 'published', 'rawSourceDepthMm',
                'rollbacks', 'exitCause', 'stepsTaken', 'workUnits')},
            'entryLossDeltaMm': loss_delta,
            'entryBoundaryViolationsDelta': viol_delta,
            'entryCollisionPairsDelta': pair_delta,
            'publishedDepthDeltaMm': depth_delta,
            'composabilityFlip': flipped,
        })

    entries = [p['currentPoseOverlayEntries'] for p in per_parent]
    off_grid = [p['currentPoseOverlayOffGridPieces'] for p in per_parent
                if p['currentPoseOverlayOffGridPieces'] is not None]
    try:
        campaign_doc = json.load(open(f'{campaign_dir}/campaign.json'))
        schedule_spec = campaign_doc.get('schedule')
    except (OSError, json.JSONDecodeError):
        schedule_spec = None

    summary = {
        'schedule': schedule_spec,
        'perArm': per_arm,
        # Sol review 6 §2.4: these are two different numbers and the v5 round
        # reported one of them under the other's name.
        'overlayCounts': {
            'entriesMedian': statistics.median(entries) if entries else None,
            'entriesMin': min(entries) if entries else None,
            'entriesMax': max(entries) if entries else None,
            'offGridPiecesMedian':
                statistics.median(off_grid) if off_grid else None,
            'offGridPiecesMin': min(off_grid) if off_grid else None,
            'offGridPiecesMax': max(off_grid) if off_grid else None,
            'parentsWhereTheyDiffer': sum(
                1 for p in per_parent
                if p['currentPoseOverlayOffGridPieces'] is not None
                and p['currentPoseOverlayOffGridPieces']
                != p['currentPoseOverlayEntries']),
        },
        'entryLoss': {
            'medianDelta': statistics.median(loss_deltas),
            'meanDelta': statistics.mean(loss_deltas),
            'nReduced': sum(1 for d in loss_deltas if d < 0),
            'nIncreased': sum(1 for d in loss_deltas if d > 0),
            'nUnchanged': sum(1 for d in loss_deltas if d == 0),
        },
        'entryBoundaryViolations': {
            'medianDelta': statistics.median(viol_deltas),
            'nReduced': sum(1 for d in viol_deltas if d < 0),
            'nIncreased': sum(1 for d in viol_deltas if d > 0),
            'nUnchanged': sum(1 for d in viol_deltas if d == 0),
        },
        'entryCollisionPairs': {
            'medianDelta': statistics.median(pair_deltas),
            'nReduced': sum(1 for d in pair_deltas if d < 0),
            'nIncreased': sum(1 for d in pair_deltas if d > 0),
            'nUnchanged': sum(1 for d in pair_deltas if d == 0),
        },
        'publishedDepth': {
            'medianDeltaMm':
                statistics.median(depth_deltas) if depth_deltas else None,
            'sumDeltaMm': sum(depth_deltas) if depth_deltas else None,
            'nOverlayDeeper': sum(1 for d in depth_deltas if d < 0),
            'nGridDeeper': sum(1 for d in depth_deltas if d > 0),
            'nTied': sum(1 for d in depth_deltas if d == 0),
        },
        'composabilityPrize': {
            'parentsTested': len(loss_deltas),
            'flips': composability_flips,
        },
        'perParent': per_parent,
    }
    json.dump(summary, open(out_path, 'w'), indent=1)
    print(json.dumps(summary['perArm'], indent=1))
    print(json.dumps({k: v for k, v in summary.items() if k != 'perParent'}, indent=1))


if __name__ == '__main__':
    main()
