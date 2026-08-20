#!/usr/bin/env python3
"""One run, with the v3 action trace and every schedule action row printed.

    smoke.py REQUEST SEED BUDGETKEY BUDGETVALUE [EXTRA]

Used to read the m34 slice's wall price, its entry feasibility and its
publications out of a single process before any battery is run.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402


def main():
    request = sys.argv[1]
    seed = int(sys.argv[2])
    key = sys.argv[3]
    value = sys.argv[4]
    extra = sys.argv[5] if len(sys.argv) > 5 else ''
    spec = runlib.spec_for(seed, key, value, True, extra)
    out = f'{runlib.OUT}/smoke/{request}-s{seed}-{key}{value}.json'
    doc, wall, err = runlib.run(runlib.BIN, request, seed, spec, out)
    if '_loadError' in doc:
        print(f'LOAD ERROR: {doc["_loadError"]}')
        return 1
    portfolio = doc['portfolio']
    print(f'spec={spec}')
    print(f'process={wall:.2f}s coordinator={portfolio["elapsedSeconds"]:.2f}s '
          f'work={portfolio["workUnits"]} '
          f'raw={portfolio["incumbent"]["rawDepthMm"]} '
          f'engine={doc.get("independentUsedLongAxisDepthMm")}')
    schedule = portfolio.get('schedule')
    if not schedule:
        print('no v3 schedule in this document')
        return 0
    print(f'exit={schedule["exitCause"]} iterations={schedule["iterations"]} '
          f'phaseZeroCost={schedule["phaseZeroCost"]:.6g}')
    for action in schedule['actions']:
        print(f'#{action["iteration"]:<3} {action["class"]:<11} '
              f'val={action["value"]:.4g} '
              f'est={action["estimatedCost"]:.4g} '
              f'act={action["actualCost"]:.4g} '
              f'sec={action["seconds"]:.3f} '
              f'pub={action["publications"]} '
              f'{action["entryRawDepthMm"]} -> {action["exitRawDepthMm"]}')
    for row in schedule['classes']:
        print(f'class {row["class"]:<11} actions={row["actions"]} '
              f'pub={row["publications"]} costMax={row["costMax"]:.4g} '
              f'firstEst={row["firstEstimatedCost"]} '
              f'firstAct={row["firstActualCost"]} '
              f'dRaw={row["deltaRawMm"]:.4f}')
    for call in portfolio['operatorCalls']:
        if call.get('mode') == 34:
            print(json.dumps({k: v for k, v in call.items()
                              if k not in ('placements',)},
                             indent=1)[:2400])
    if err.strip():
        print(f'stderr tail: {err[-400:]}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
