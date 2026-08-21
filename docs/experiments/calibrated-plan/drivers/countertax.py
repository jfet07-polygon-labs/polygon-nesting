#!/usr/bin/env python3
"""What the work counters cost, in millimetres, at a fixed wall.

    python3 countertax.py OUTDIR BINARY REQUEST TARGET_MS SEEDS ROUNDS

`search::portfolio`'s own header says a work budget "arms [the counters] and
pays the ~17% they cost". That number is a throughput claim and this round needs
a **depth** claim, because the calibrated plan is a work mode and therefore
carries the counters through the whole of the wall it was given - which is the
one cost of the mode that no constant and no ladder can remove.

The measurement is the only clean one available: the same binary, the same
`wall=<ms>` budget, the counters forced on and off by
`POLYGON_NESTING_PROFILE`, which is exactly the switch
`profiling::set_enabled` gets from a work budget. Both arms are wall-budgeted,
so both spend the same seconds; the difference is what the instrument ate.

Paired per (seed, round) and interleaved with arm order rotated, like every
battery in this campaign.
"""
import json
import os
import statistics
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402


def run_with_counters(binary, request, seed, spec, out_path, counters):
    """`runlib.run`, except that it may *set* `POLYGON_NESTING_PROFILE`.

    `runlib.run` unsets it unconditionally, which is right for every other
    driver here and wrong for this one: this battery's whole subject is that
    variable. Everything else - the argv, the allowance, the quality-trace
    scrub - is `runlib`'s, so the two arms differ in one environment entry and
    nothing else.
    """
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_QUALITY_TRACE', None)
    env.pop('POLYGON_NESTING_QUALITY_TRACE_COUNTERS', None)
    if counters:
        env['POLYGON_NESTING_PROFILE'] = '1'
    else:
        env.pop('POLYGON_NESTING_PROFILE', None)
    command = runlib.argv(binary, request, seed, spec)
    started = time.monotonic()
    with open(out_path, 'w') as handle:
        result = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                                check=False, env=env)
    wall = time.monotonic() - started
    stderr = (result.stderr or b'').decode()[-1200:]
    try:
        with open(out_path) as handle:
            return json.load(handle), wall, stderr
    except json.JSONDecodeError:
        return {}, wall, stderr


def main():
    outdir, binary, request, target_ms = sys.argv[1:5]
    seeds = [int(v) for v in sys.argv[5].split(',')]
    rounds = int(sys.argv[6])
    os.makedirs(outdir, exist_ok=True)
    arms = ['countersOff', 'countersOn']
    rows = []
    for rnd in range(rounds):
        order = arms[rnd % 2:] + arms[:rnd % 2]
        for arm in order:
            for seed in seeds:
                spec = runlib.spec_for(seed, 'wall', target_ms, True)
                tag = f'{arm}-s{seed}-r{rnd}'
                doc, wall, err = run_with_counters(
                    binary, request, seed, spec, f'{outdir}/{tag}.json',
                    arm == 'countersOn')
                portfolio = doc.get('portfolio') or {}
                if not portfolio:
                    rows.append({'tag': tag, 'error': err[-300:]})
                    continue
                rows.append({
                    'tag': tag, 'arm': arm, 'seed': seed, 'round': rnd,
                    'processWallSeconds': wall,
                    'rawDepthMm': portfolio['incumbent']['rawDepthMm'],
                    'workUnits': portfolio['workUnits'],
                })
                print(f'{tag}: wall={wall:.3f} '
                      f'depth={rows[-1]["rawDepthMm"]:.3f} '
                      f'units={rows[-1]["workUnits"]}', flush=True)

    good = [r for r in rows if 'error' not in r]
    summary = {'binary': binary, 'request': request, 'targetMs': target_ms,
               'seeds': seeds, 'rounds': rounds, 'rows': rows, 'perSeed': {}}
    deltas = []
    for seed in seeds:
        off = [r['rawDepthMm'] for r in good
               if r['seed'] == seed and r['arm'] == 'countersOff']
        on = [r['rawDepthMm'] for r in good
              if r['seed'] == seed and r['arm'] == 'countersOn']
        if not off or not on:
            continue
        delta = statistics.median(on) - statistics.median(off)
        deltas.append(delta)
        summary['perSeed'][str(seed)] = {
            'countersOffMedianMm': statistics.median(off),
            'countersOnMedianMm': statistics.median(on),
            'deltaMm': delta,
            'countersOffDistinct': sorted(set(off)),
            'countersOnDistinct': sorted(set(on)),
        }
        print(f'seed {seed}: off={statistics.median(off):.3f} '
              f'on={statistics.median(on):.3f} delta={delta:+.3f} mm',
              flush=True)
    if deltas:
        summary['medianDeltaMm'] = statistics.median(deltas)
        summary['worstDeltaMm'] = max(deltas)
    json.dump(summary, open(f'{outdir}/countertax.json', 'w'), indent=1)
    print(json.dumps({k: v for k, v in summary.items() if k != 'rows'},
                     indent=1))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
