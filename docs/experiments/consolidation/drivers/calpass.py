#!/usr/bin/env python3
"""The offline calibration pass: fill the per-box files the plan reads.

    python3 calpass.py OUTDIR BINARY ROUNDS [FIXTURES] [SEEDS] [TARGET_MS]

Sol review 8 §3 condition 1 is one sentence - *"il probe hardware dev'essere
offline/persistito e il cap parte della spec"* - and this is the offline half of
it. It runs the request set with `plancalwrite=1` and no measurement claim
attached, so the file is a product of a declared pass rather than a side effect
of whichever battery happened to run first.

Two files, because there are two estimators and the round has to be able to
separate them:

* `live.json` - written by a pass with no `planprobe`, so each entry is the
  **least-loaded whole-phase reading** for that cell. This is the same quantity
  the shipping `plan=<ms>` arm divides by, so a run that reads this file plans
  the incumbent's own budget and differs from it only in being a function of
  counters instead of a clock.
* `probe.json` - written by a pass with `planprobe=8`, so each entry is the
  least-loaded **max-of-k bucket** estimate, which is systematically shorter and
  buys a rung more.

The min rule does the converging: a round that hits a load spike writes nothing,
a round that catches the box quiet lowers the entry. So the pass is run more
than once on purpose and the file is reported after each round, which is how a
reader can see whether it had converged before the batteries used it.

The key is `probe_work_units`, a counter, so one file serves every target: phase
0 does not know what the target is.
"""
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

LIVE = os.environ.get('PLAN_CAL_LIVE', '/var/lib/t3/tmp/consol/cal/live.json')
PROBE = os.environ.get('PLAN_CAL_PROBE', '/var/lib/t3/tmp/consol/cal/probe.json')


def entries(path):
    try:
        with open(path) as handle:
            return {int(k): v['probeSeconds']
                    for k, v in json.load(handle)['entries'].items()}
    except (OSError, ValueError, KeyError):
        return {}


def main():
    outdir, binary, rounds = sys.argv[1], sys.argv[2], int(sys.argv[3])
    fixtures = (sys.argv[4].split(',') if len(sys.argv) > 4
                else ['mixed-61', 'shapes-17', 'triangle-20'])
    seeds = ([int(v) for v in sys.argv[5].split(',')] if len(sys.argv) > 5
             else [0, 1, 2])
    target = sys.argv[6] if len(sys.argv) > 6 else '10000'
    os.makedirs(outdir, exist_ok=True)
    os.makedirs(os.path.dirname(LIVE), exist_ok=True)
    rows = []
    history = []
    for rnd in range(rounds):
        for fixture in fixtures:
            for seed in seeds:
                for pass_name, extra in (
                        ('live', f'plancal={LIVE},plancalwrite=1'),
                        ('probe',
                         f'planprobe=8,plancal={PROBE},plancalwrite=1')):
                    spec = runlib.spec_for(seed, 'plan', target, True, extra)
                    tag = f'{pass_name}-{fixture}-s{seed}-r{rnd}'
                    doc, wall, err = runlib.run(binary, fixture, seed, spec,
                                                f'{outdir}/{tag}.json')
                    portfolio = doc.get('portfolio') or {}
                    if not portfolio:
                        print(f'{tag}: FAILED {err[-200:]}', flush=True)
                        continue
                    plan = portfolio.get('plan') or {}
                    cal = portfolio.get('planCalibration') or {}
                    row = {
                        'tag': tag, 'pass': pass_name, 'fixture': fixture,
                        'seed': seed, 'round': rnd,
                        'processWallSeconds': wall,
                        'probeWorkUnits': plan.get('probeWorkUnits'),
                        'probeSeconds': cal.get('probeSeconds'),
                        'probeEffectiveSeconds':
                            cal.get('probeEffectiveSeconds'),
                        'probeSamples': cal.get('probeSamples'),
                        'calibrationSource': plan.get('calibrationSource'),
                        'planUnits': plan.get('units'),
                        'rawDepthMm': portfolio['incumbent']['rawDepthMm'],
                        'load1': runlib.LOAD[-1]['before'],
                    }
                    rows.append(row)
                    print(f'{tag}: t0={row["probeSeconds"]:.4f} '
                          f'teff={row["probeEffectiveSeconds"]:.4f} '
                          f'W0={row["probeWorkUnits"]} '
                          f'units={row["planUnits"]} '
                          f'load={row["load1"]:.2f}', flush=True)
        history.append({'round': rnd, 'live': entries(LIVE),
                        'probe': entries(PROBE)})

    live, probe = entries(LIVE), entries(PROBE)
    summary = {
        'binary': binary, 'rounds': rounds, 'fixtures': fixtures,
        'seeds': seeds, 'targetMs': target,
        'livePath': LIVE, 'probePath': PROBE,
        'live': {str(k): v for k, v in live.items()},
        'probe': {str(k): v for k, v in probe.items()},
        # Did the last round change anything? An entry that moved on the final
        # round is an entry the pass had not finished converging.
        'convergedOnLastRound': (
            len(history) > 1
            and history[-1]['live'] == history[-2]['live']
            and history[-1]['probe'] == history[-2]['probe']),
        'history': [{'round': h['round'],
                     'live': {str(k): v for k, v in h['live'].items()},
                     'probe': {str(k): v for k, v in h['probe'].items()}}
                    for h in history],
        # What the bucket estimator is worth on this box, as a ratio: the file's
        # probe entry against its live entry for the same cell.
        'probeOverLive': {
            str(key): probe[key] / live[key]
            for key in sorted(set(live) & set(probe)) if live[key] > 0},
        'rows': rows,
    }
    loads = [row['before'] for row in runlib.LOAD if row['before'] is not None]
    summary['boxLoad'] = {
        'n': len(loads), 'min': min(loads) if loads else None,
        'median': statistics.median(loads) if loads else None,
        'max': max(loads) if loads else None,
    }
    json.dump(summary, open(f'{outdir}/calpass.json', 'w'), indent=1)
    print(json.dumps({k: summary[k] for k in
                      ('live', 'probe', 'probeOverLive',
                       'convergedOnLastRound', 'boxLoad')}, indent=1))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
