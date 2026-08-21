#!/usr/bin/env python3
"""One cell, both arms, printed - the first thing to run on a new binary.

    python3 smoke.py BINARY [REQUEST] [SEED] [TARGET_MS] [RACE_SPEC]

It exists because a race that never started and a race that started and picked
the incumbent produce nearly the same headline number, and only the arm rows
tell them apart.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

binary = sys.argv[1]
request = sys.argv[2] if len(sys.argv) > 2 else 'mixed-61'
seed = int(sys.argv[3]) if len(sys.argv) > 3 else 0
target = int(sys.argv[4]) if len(sys.argv) > 4 else 10000
race_spec = sys.argv[5] if len(sys.argv) > 5 else '3:1:3'
out = os.environ.get('SMOKE_OUT', '/var/lib/t3/tmp/basinrace/out/smoke')

for extra in ['race=0', f'race={race_spec}']:
    spec = runlib.spec_for(seed, 'plan', target, True, extra)
    tag = extra.replace(':', '-').replace('=', '')
    doc, wall, err = runlib.run(binary, request, seed, spec,
                                f'{out}/{request}-s{seed}-{tag}.json')
    portfolio = doc.get('portfolio') or {}
    incumbent = portfolio.get('incumbent') or {}
    plan = portfolio.get('plan') or {}
    print(f'{extra}: wall={wall:.3f}s depth={incumbent.get("rawDepthMm")} '
          f'planUnits={plan.get("units")} '
          f'work={portfolio.get("workUnits")} '
          f'actions={(portfolio.get("schedule") or {}).get("iterations")}')
    race = portfolio.get('basinRace')
    if race:
        head = {k: v for k, v in race.items() if k != 'arms'}
        print('  race: ' + json.dumps(head))
        for arm in race.get('arms', []):
            print(f'   slot{arm["slot"]:>2} {arm["kind"]:<20} '
                  f'depth={arm["depthMm"]:.4f} yield={arm["yieldMm"]:.4f} '
                  f'stab={arm["stability"]:.3f} infeas={arm["infeasibility"]:.3f} '
                  f'steps={arm["batchSteps"]} conf={arm["batchConfirmations"]} '
                  f'rank={arm["rankSum"]} elim={arm["eliminatedRound"]} '
                  f'retired={arm["retiredFromArchive"]}')
    if err:
        print('  stderr: ' + err[-400:])
