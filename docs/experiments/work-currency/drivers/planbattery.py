#!/usr/bin/env python3
"""§5: the canonical instrument - plan mode, `cur2` on against off, at equal
plan.

`plan=<ms>` is the campaign's canonical instrument and the binding user
priority is quality at ten seconds from a bare request, so this is the arm the
currency has to survive: the race is off, the coordinator is the shipped v3
queue, and the only difference between the two arms is whether the queue's
affordability rule, its class ranking and its phase deadlines are reading a
currency that can see a constructor draw.

**Equal plan is a strong statement here and the driver checks it.**
`install_plan` runs at the end of phase 0, and phase 0 is not an operator call
- it does not go through `run_operator` - so the currency cannot touch the
probe. Both arms therefore read a bit-identical `probe_work_units` and should
land on the same rung. Should, not must: the ladder straddles under load, so
`portfolio.plan.units` is compared per cell and a row where the two disagree is
excluded from every aggregate rather than averaged in.

Arms are interleaved by round so neither is always first into a warm box.

    python3 planbattery.py OUT_JSON BINARY REQUESTS SEEDS TARGET_MS ROUNDS
"""
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

ARMS = {'off': 'cur2=0', 'on': 'cur2=1'}


def cell(doc, wall):
    portfolio = doc.get('portfolio') or {}
    incumbent = portfolio.get('incumbent') or {}
    plan = portfolio.get('plan') or {}
    schedule = portfolio.get('schedule') or {}
    calls = portfolio.get('operatorCalls') or []
    draws = [c for c in calls if c['operator'] == 'mode20']
    return {
        'processSeconds': wall,
        'depthMm': incumbent.get('rawDepthMm'),
        'dualGateValid': incumbent.get('dualGateValid'),
        'planUnits': plan.get('units'),
        'probeWorkUnits': plan.get('probeWorkUnits'),
        'workUnits': portfolio.get('workUnits'),
        'coordinatorSeconds': portfolio.get('elapsedSeconds'),
        'operatorCalls': len(calls),
        'publications': len(portfolio.get('publications') or []),
        'scheduleIterations': schedule.get('iterations'),
        'scheduleExitCause': schedule.get('exitCause'),
        'workCurrency': portfolio.get('workCurrency'),
        'drawCalls': len(draws),
        'drawSeconds': sum(c['elapsedSeconds'] for c in draws),
        'drawChargedUnits': sum(c['workUnits'] for c in draws),
        'digest': runlib.doc_digest(doc),
    }


def main():
    out, binary = sys.argv[1:3]
    requests = sys.argv[3].split(',')
    seeds = [int(v) for v in sys.argv[4].split(',')]
    target = int(sys.argv[5])
    rounds = int(sys.argv[6]) if len(sys.argv) > 6 else 2
    outdir = os.path.dirname(out)

    rows = []
    for round_index in range(rounds):
        for request in requests:
            for seed in seeds:
                order = (['off', 'on'] if (round_index + seed) % 2 == 0
                         else ['on', 'off'])
                row = {'request': request, 'seed': seed,
                       'round': round_index, 'armOrder': order}
                for arm in order:
                    spec = runlib.spec_for(seed, 'plan', target, True,
                                           ARMS[arm])
                    tag = f'plan-{request}-s{seed}-r{round_index}-{arm}'
                    doc, wall, err = runlib.run(
                        binary, request, seed, spec, f'{outdir}/{tag}.json')
                    row[arm] = cell(doc, wall)
                    row[arm]['spec'] = spec
                    if err:
                        row[arm]['stderrTail'] = err[-300:]
                off, on = row['off'], row['on']
                row['equalWork'] = (off['planUnits'] is not None
                                    and off['planUnits'] == on['planUnits'])
                if off['depthMm'] is not None and on['depthMm'] is not None:
                    row['deltaMm'] = on['depthMm'] - off['depthMm']
                rows.append(row)
                print(f"r{round_index} {request} s{seed}: "
                      f"off={off['depthMm']} on={on['depthMm']} "
                      f"delta={row.get('deltaMm')} "
                      f"equal={row['equalWork']} "
                      f"draws={off['drawCalls']}/{on['drawCalls']} "
                      f"extra={(on.get('workCurrency') or {}).get('chargedExtraUnits')}",
                      flush=True)

    paired = [r for r in rows
              if r.get('equalWork') and r.get('deltaMm') is not None]
    deltas = [r['deltaMm'] for r in paired]
    # Per-seed medians, because a pooled median over rounds of one seed is a
    # measurement of that seed. `calibrated-plan` §10 quotes the same shape.
    per_seed = {}
    for row in paired:
        per_seed.setdefault((row['request'], row['seed']), []).append(
            row['deltaMm'])
    document = {
        'binary': binary, 'binarySha256': runlib.sha256_of(binary),
        'targetMillis': target, 'requests': requests, 'seeds': seeds,
        'rounds': rounds, 'arms': ARMS, 'rows': rows,
        'summary': {
            'cells': len(rows),
            'equalWorkCells': len(paired),
            'medianDeltaMm': statistics.median(deltas) if deltas else None,
            'meanDeltaMm': statistics.fmean(deltas) if deltas else None,
            'better': sum(1 for d in deltas if d < 0),
            'worse': sum(1 for d in deltas if d > 0),
            'tied': sum(1 for d in deltas if d == 0),
            'perSeedMedians': {f'{k[0]}-s{k[1]}': statistics.median(v)
                               for k, v in sorted(per_seed.items())},
        },
        'boxLoad': runlib.LOAD,
    }
    with open(out, 'w') as handle:
        json.dump(document, handle, indent=1, sort_keys=True)
    print(json.dumps(document['summary'], indent=1))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
