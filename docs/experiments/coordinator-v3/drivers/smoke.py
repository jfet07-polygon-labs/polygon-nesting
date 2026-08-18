#!/usr/bin/env python3
"""One v2 and one v3 run at a work budget, printed as an action trace.

    smoke.py REQUEST SEED WORK
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

request = sys.argv[1]
seed = int(sys.argv[2])
work = int(sys.argv[3])

for label, v3 in (('v2', False), ('v3', True)):
    spec = runlib.spec_for(seed, 'work', work, v3)
    doc, wall, err = runlib.run(
        runlib.BIN, request, seed, spec,
        f'{runlib.OUT}/smoke/{label}-{request}-s{seed}.json')
    row = runlib.summarize(label, doc, wall)
    print(f"== {label} {request} seed {seed} work={work} spec={spec}")
    if 'loadError' in row:
        print('  ERROR', row['loadError'][-600:])
        continue
    print(f"  raw={row['rawDepthMm']} dualGate={row['dualGateValid']} "
          f"engine={row['engineDepthMm']} wall={wall:.2f}s "
          f"coordinator={row['coordinatorSeconds']:.2f}s "
          f"work={row['workUnits']}")
    for phase in row['phases']:
        print(f"    phase {phase['name']:<12} work={phase['workUnits']:>12} "
              f"s={phase['elapsedSeconds']:6.2f} calls={phase['operatorCalls']:>3} "
              f"pubs={phase['publications']} exit={phase['exitCause']}")
    schedule = row.get('schedule')
    if schedule:
        print(f"    schedule iterations={schedule['iterations']} "
              f"exit={schedule['exitCause']} "
              f"phase0Cost={schedule['phaseZeroCost']:.0f}")
        for cls in schedule['classes']:
            print(f"      class {cls['class']:<12} actions={cls['actions']:>3} "
                  f"pubs={cls['publications']:>2} "
                  f"work={cls['workUnits']:>12} "
                  f"delta={cls['deltaRawMm']:8.4f} "
                  f"est1={cls['firstEstimatedCost']:.0f} "
                  f"act1={cls['firstActualCost']:.0f}")
        for action in schedule['actions']:
            print(f"      #{action['iteration']:<3} {action['class']:<12} "
                  f"val={action['value']:7.3f} est={action['estimatedCost']:>12.0f} "
                  f"act={action['actualCost']:>12.0f} "
                  f"cand={action['candidates']:>3} pubs={action['publications']} "
                  f"{action['entryRawDepthMm']} -> {action['exitRawDepthMm']} "
                  f"| {action['label']}")
    for pub in row['publications']:
        print(f"    pub {pub['phase']:<12} {pub['source']:<10} "
              f"{pub['previousRawDepthMm']} -> {pub['rawDepthMm']} "
              f"at {pub['seconds']:.2f}s / {pub['workUnits']} units")
