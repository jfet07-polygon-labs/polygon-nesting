#!/usr/bin/env python3
"""Aggregate tier-four statistics over every mode-26 run this session produced."""

import glob
import json
import sys


def collect(paths):
    rows = []
    for path in paths:
        try:
            with open(path) as handle:
                data = json.load(handle)
            pop = (data['relaxedDiagnostics']['coupledDynamicSeparator']
                   ['persistentVacancyPopulation'])
        except Exception:
            continue
        if pop.get('mode') != 26:
            continue
        ladder = pop.get('ladderCompression') or {}
        for step in ladder.get('steps', []):
            for arm in step.get('arms', []):
                g = arm.get('globalLegalization')
                if g is not None:
                    rows.append((path, step['boundMm'], arm, g))
    return rows


def main():
    rows = collect(sorted(set(sum([glob.glob(p) for p in sys.argv[1:]], []))))
    if not rows:
        print('no tier-four invocations found')
        return
    total = len(rows)
    attempted = sum(1 for _, _, _, g in rows if g.get('attempted'))
    resolved = sum(1 for _, _, _, g in rows if g.get('resolved'))
    valid = sum(1 for _, _, _, g in rows if g.get('exactValid'))
    capped = sum(1 for _, _, _, g in rows if g.get('displacementCapped'))
    exhausted = sum(1 for _, _, _, g in rows if g.get('capExhausted'))

    def spread(key, transform=lambda v: v):
        values = sorted(transform(g[key]) for _, _, _, g in rows
                        if g.get(key) is not None)
        if not values:
            return 'n/a'
        return (f'min={values[0]} p50={values[len(values)//2]} '
                f'p90={values[int(len(values) * 0.9)]} max={values[-1]}')

    print(f'tier-four invocations: {total}')
    print(f'  attempted (residue present): {attempted}')
    print(f'  geometry resolved:           {resolved}')
    print(f'  exact-valid publications:    {valid}')
    print(f'  trust/displacement capped:   {capped}')
    print(f'  probe budget exhausted:      {exhausted}')
    print(f'  violatingPairsBefore  {spread("violatingPairsBefore")}')
    print(f'  boundaryPiecesBefore  {spread("boundaryPiecesBefore")}')
    print(f'  violatingPairsAfter   {spread("violatingPairsAfter")}')
    print(f'  boundaryPiecesAfter   {spread("boundaryPiecesAfter")}')
    print(f'  movedPieces           {spread("movedPieces")}')
    print(f'  maxDisplacementMm     {spread("maxDisplacementMm", lambda v: round(v, 3))}')
    print(f'  meanDisplacementMm    {spread("meanDisplacementMm", lambda v: round(v, 3))}')
    print(f'  maxDualResidualMm     {spread("maxDualResidualMm", lambda v: round(v, 4))}')
    print(f'  roundsRun             {spread("roundsRun")}')
    print(f'  dualSweepsRun         {spread("dualSweepsRun")}')
    print(f'  maxRows               {spread("maxRows")}')
    print(f'  maxPairRows           {spread("maxPairRows")}')
    print(f'  pairVisits            {spread("pairVisits")}')
    reasons = {}
    for _, _, _, g in rows:
        reason = (g.get('skippedReason') or g.get('rejectionReason')
                  or 'published')
        reason = reason.split(':')[0][:64]
        reasons[reason] = reasons.get(reason, 0) + 1
    print(f'  outcomes: {reasons}')
    tiers = {}
    for _, _, arm, _ in rows:
        if arm.get('globalLegalizedDepthMm') is not None:
            tiers['tier4 produced a state'] = tiers.get('tier4 produced a state', 0) + 1
    print(f'  {tiers}')


if __name__ == '__main__':
    main()
