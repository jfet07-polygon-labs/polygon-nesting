#!/usr/bin/env python3
"""Standalone mode-30/31 probes on the two pinned parents."""

import json
import sys

sys.path.insert(0, '/var/lib/t3/tmp/mode31')
from run import RECORD, SCRATCH, run, line, population  # noqa: E402

OUT = '/var/lib/t3/tmp/mode31/probes'


def detail(tag, run_json):
    pop = population(run_json)
    if pop is None:
        return f'{tag}: NO POPULATION'
    g = pop.get('globalLegalization') or {}
    return (f"{tag}: exactValid={pop['exactValid']} "
            f"depth={pop.get('independentDepthMm')} "
            f"raw={pop.get('rawSourceDepthMm')} "
            f"parent={pop.get('parentIndependentDepthMm')}\n"
            f"    bound={g.get('boundMm')} effLong={g.get('effectiveLongAxisMm')} "
            f"pairsBefore={g.get('violatingPairsBefore')} "
            f"boundaryBefore={g.get('boundaryPiecesBefore')} "
            f"comps={g.get('componentCount')}/{g.get('largestComponentPieces')} "
            f"maxMatDef={g.get('maxMaterialDeficitMm')} "
            f"maxEnvPush={g.get('maxEnvelopePushMm')} "
            f"maxBoundDef={g.get('maxBoundaryDeficitMm')}\n"
            f"    rounds={g.get('roundsRun')} esc={g.get('escalationsRun')} "
            f"sweeps={g.get('dualSweepsRun')} residual={g.get('maxDualResidualMm')} "
            f"rows={g.get('maxRows')}({g.get('maxPairRows')}p/{g.get('maxBoundaryRows')}b) "
            f"moved={g.get('movedPieces')} maxDisp={g.get('maxDisplacementMm')} "
            f"meanDisp={g.get('meanDisplacementMm')} capped={g.get('displacementCapped')}\n"
            f"    pairsAfter={g.get('violatingPairsAfter')} "
            f"boundaryAfter={g.get('boundaryPiecesAfter')} "
            f"matDefAfter={g.get('maxMaterialDeficitAfterMm')} "
            f"envPushAfter={g.get('maxEnvelopePushAfterMm')} "
            f"boundDefAfter={g.get('maxBoundaryDeficitAfterMm')} "
            f"resolved={g.get('resolved')} "
            f"visits={g.get('pairVisits')}/{g.get('fundedPairVisits')}\n"
            f"    reject={(g.get('rejectionReason') or g.get('skippedReason') or '')[:140]}")


if __name__ == '__main__':
    print(detail('record-mode30', run('record-mode30', 30, RECORD, '0', 0, OUT)))
    for bound in ('163.8', '163.5', '163.0'):
        tag = f'record-mode31-{bound}'
        print(detail(tag, run(tag, 31, RECORD, bound, 0, OUT)))
    print(detail('scratch-mode30', run('scratch-mode30', 30, SCRATCH, '0', 0, OUT)))
    for bound in ('164.4',):
        tag = f'scratch-mode31-{bound}'
        print(detail(tag, run(tag, 31, SCRATCH, bound, 0, OUT)))
