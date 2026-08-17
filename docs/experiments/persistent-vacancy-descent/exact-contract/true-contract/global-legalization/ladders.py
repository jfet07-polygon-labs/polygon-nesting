#!/usr/bin/env python3
"""Mode-26 ladders, matched baseline arm vs the arm with the global tier armed.

Every ladder is run twice - once on the pre-change binary and once on the
binary carrying the fourth repair tier - so any publication difference is
attributable to the tier and nothing else.
"""

import json
import os
import subprocess
import sys

sys.path.insert(0, '/var/lib/t3/tmp/mode31')
from run import ARGS, REQ, RECORD, SCRATCH  # noqa: E402

BASE = '/var/lib/t3/tmp/mode31-baseline'
TREAT = '/var/lib/t3/tmp/mode31-bench'
OUT = '/var/lib/t3/tmp/mode31/ladders'


def run(binary, tag, parent, bound, seed):
    os.makedirs(OUT, exist_ok=True)
    path = f'{OUT}/{tag}.json'
    if not os.path.exists(path):
        argv = [binary, REQ] + [a.format(clamp='0', seed=seed) for a in ARGS] + [
            '26', parent, str(bound), '', '0.0005']
        with open(path, 'w') as out:
            subprocess.run(argv, stdout=out, stderr=subprocess.DEVNULL, check=False)
    with open(path) as handle:
        return json.load(handle)


def summarize(tag, run_json):
    pop = (run_json['relaxedDiagnostics']['coupledDynamicSeparator']
           .get('persistentVacancyPopulation'))
    if pop is None:
        return f'{tag}: NO POPULATION'
    ladder = pop.get('ladderCompression') or {}
    lines = [f"{tag}: exactValid={pop['exactValid']} "
             f"published={pop.get('independentDepthMm')} "
             f"raw={pop.get('rawSourceDepthMm')} "
             f"parent={pop.get('parentIndependentDepthMm')} "
             f"finalBound={ladder.get('finalBoundMm')} "
             f"step={ladder.get('stepMm')} "
             f"publishedStep={ladder.get('publishedStep')} "
             f"fp={(pop.get('finalPlacementFingerprint') or '')[:16]}"]
    frontier = None
    tier4_invocations = 0
    tier4_attempted = 0
    tier4_resolved = 0
    tier4_published = 0
    tier4_pairs_before = []
    tier4_pairs_after = []
    tier4_boundary_before = []
    tier4_boundary_after = []
    tier4_moved = []
    tier4_maxdisp = []
    tier4_residual = []
    tier4_reasons = {}
    for step in ladder.get('steps', []):
        chained = step.get('chainedDepthMmAfter')
        if chained is not None and (frontier is None or chained < frontier):
            frontier = chained
        for arm in step.get('arms', []):
            g = arm.get('globalLegalization')
            if g is None:
                continue
            tier4_invocations += 1
            tier4_attempted += 1 if g.get('attempted') else 0
            tier4_resolved += 1 if g.get('resolved') else 0
            tier4_published += 1 if g.get('exactValid') else 0
            tier4_pairs_before.append(g.get('violatingPairsBefore'))
            tier4_pairs_after.append(g.get('violatingPairsAfter'))
            tier4_boundary_before.append(g.get('boundaryPiecesBefore'))
            tier4_boundary_after.append(g.get('boundaryPiecesAfter'))
            tier4_moved.append(g.get('movedPieces'))
            tier4_maxdisp.append(g.get('maxDisplacementMm') or 0.0)
            tier4_residual.append(g.get('maxDualResidualMm') or 0.0)
            reason = (g.get('skippedReason') or g.get('rejectionReason') or 'published')
            reason = reason.split(':')[0][:70]
            tier4_reasons[reason] = tier4_reasons.get(reason, 0) + 1
    lines.append(f'    frontier={frontier} steps={ladder.get("stepsRun")}')
    if tier4_invocations:
        def stat(values):
            clean = [v for v in values if v is not None]
            if not clean:
                return 'n/a'
            return f'min={min(clean)} med={sorted(clean)[len(clean)//2]} max={max(clean)}'
        lines.append(
            f'    tier4: invocations={tier4_invocations} attempted={tier4_attempted} '
            f'resolved={tier4_resolved} published={tier4_published}')
        lines.append(f'    tier4 pairsBefore {stat(tier4_pairs_before)}; '
                     f'pairsAfter {stat(tier4_pairs_after)}')
        lines.append(f'    tier4 boundaryBefore {stat(tier4_boundary_before)}; '
                     f'boundaryAfter {stat(tier4_boundary_after)}')
        lines.append(f'    tier4 movedPieces {stat(tier4_moved)}; '
                     f'maxDisp {stat([round(v, 4) for v in tier4_maxdisp])}; '
                     f'dualResidual {stat([round(v, 4) for v in tier4_residual])}')
        lines.append(f'    tier4 outcomes: {tier4_reasons}')
    return '\n'.join(lines)


LADDERS = [
    ('record-163.5-s0', RECORD, '163.5', 0),
    ('record-163.5-s1', RECORD, '163.5', 1),
    ('record-162.5-s0', RECORD, '162.5', 0),
    ('scratch-164.4-s0', SCRATCH, '164.4', 0),
]

if __name__ == '__main__':
    only = sys.argv[1] if len(sys.argv) > 1 else None
    for tag, parent, bound, seed in LADDERS:
        if only and only != tag:
            continue
        base = run(BASE, f'{tag}-base', parent, bound, seed)
        treat = run(TREAT, f'{tag}-treat', parent, bound, seed)
        print(summarize(f'{tag} BASE ', base))
        print(summarize(f'{tag} TREAT', treat))
        print()
