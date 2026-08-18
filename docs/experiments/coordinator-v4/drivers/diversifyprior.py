#!/usr/bin/env python3
"""What one diversify action is *measured* to be worth, per request.

Coordinator v3 gives the diversify class a prior of `0.0` mm and then schedules
it by an eligibility rule instead of by its rank. The prior is the opportunity
ledger's mixed-61 reading - 0 descendant publications from any archived m20
basin - and it is a true statement about mixed-61 and a false one about
triangle-20, where coordinator v2's own generality measurement found the slice
publishing on 6 of 12 arms.

This driver reads the number the prior has to be: the raw-depth millimetres the
`diversify` phase's own publications moved the incumbent, divided by the
constructor arms that phase drew, on the v2 schedule that still has the phase.
It is run on every request the coordinator has been measured on, because a prior
that is quoted from one request is the defect this is fixing.

    python3 diversifyprior.py <out.json> <binary> [requests] [seeds] [work]
"""
import json
import sys

import runlib


def measure(binary, request, seed, work):
    spec = runlib.spec_for(seed, 'work', work, v3=False)
    out = f'{runlib.OUT}/divprior/{request}-s{seed}.json'
    doc, wall, err = runlib.run(binary, request, seed, spec, out)
    portfolio = doc.get('portfolio')
    if not portfolio:
        return {'request': request, 'seed': seed, 'error': err[-400:]}
    calls = portfolio['operatorCalls']
    arms = [call for call in calls
            if call['phase'] == 'diversify' and call['operator'] == 'mode20']
    published_arms = [call for call in arms if call['published']]
    gained = 0.0
    for event in portfolio['publications']:
        if event['phase'] != 'diversify':
            continue
        previous = event.get('previousRawDepthMm')
        if previous is None:
            continue
        gained += max(previous - event['rawDepthMm'], 0.0)
    return {
        'request': request,
        'seed': seed,
        'workBudget': work,
        'processSeconds': wall,
        'rawDepthMm': portfolio['incumbent']['rawDepthMm'],
        'diversifyArms': len(arms),
        'diversifyArmsPublished': len(published_arms),
        'diversifyGainedMm': gained,
        'diversifyWorkUnits': sum(call['workUnits'] for call in calls
                                  if call['phase'] == 'diversify'),
        'diversifySeconds': sum(call['elapsedSeconds'] for call in calls
                                if call['phase'] == 'diversify'),
        'phaseZeroWorkUnits': next(
            (phase['workUnits'] for phase in portfolio['phases']
             if phase['name'] == 'm0'), None),
        'phaseZeroSeconds': next(
            (phase['elapsedSeconds'] for phase in portfolio['phases']
             if phase['name'] == 'm0'), None),
    }


def main():
    out_path = sys.argv[1]
    binary = sys.argv[2]
    requests = (sys.argv[3] if len(sys.argv) > 3
                else 'mixed-61,shapes-17,triangle-20').split(',')
    seeds = [int(value) for value in
             (sys.argv[4] if len(sys.argv) > 4 else '0,1,2').split(',')]
    work = int(sys.argv[5]) if len(sys.argv) > 5 else runlib.WORK_10S
    rows = [measure(binary, request, seed, work)
            for request in requests for seed in seeds]
    summary = {}
    for request in requests:
        subset = [row for row in rows if row['request'] == request
                  and 'error' not in row]
        arms = sum(row['diversifyArms'] for row in subset)
        summary[request] = {
            'arms': arms,
            'costInPhaseZerosWork': [
                row['diversifyWorkUnits'] / row['phaseZeroWorkUnits']
                for row in subset
                if row.get('phaseZeroWorkUnits') and row['diversifyArms']],
            'costInPhaseZerosWall': [
                row['diversifySeconds'] / row['phaseZeroSeconds']
                for row in subset
                if row.get('phaseZeroSeconds') and row['diversifyArms']],
            'armsPublished': sum(row['diversifyArmsPublished']
                                 for row in subset),
            'gainedMm': sum(row['diversifyGainedMm'] for row in subset),
            'mmPerArm': (sum(row['diversifyGainedMm'] for row in subset) / arms
                         if arms else None),
        }
    arms = sum(value['arms'] for value in summary.values())
    document = {
        'workBudget': work,
        'rows': rows,
        'perRequest': summary,
        'pooled': {
            'arms': arms,
            'gainedMm': sum(value['gainedMm'] for value in summary.values()),
            'mmPerArm': (sum(value['gainedMm'] for value in summary.values())
                         / arms if arms else None),
        },
    }
    with open(out_path, 'w') as handle:
        json.dump(document, handle, indent=1)
    print(json.dumps(document['perRequest'], indent=1))
    print(json.dumps(document['pooled'], indent=1))


if __name__ == '__main__':
    main()
