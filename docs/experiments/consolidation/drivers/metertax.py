#!/usr/bin/env python3
"""What the work counters cost, split into the counting and the timing.

    python3 metertax.py OUTDIR BINARY REQUEST TARGET_MS SEEDS ROUNDS

`docs/experiments/calibrated-plan/` §9 measured *"the work counters"* at
**+1.882 mm** on mixed-61 at a ten-second wall and called it a floor under any
work-denominated budget, *"and there is no version of this mode that avoids
it"*. That conclusion followed from the instrument as much as from the engine:
one flag - `profiling::set_enabled` - armed the counting and the timing
together, so no measurement could separate them.

`profiling::metering_enabled` separates them, and this driver is the paired
re-measurement. Three arms, one binary, one `wall=<ms>` budget, so all three
spend the same seconds and the difference is what the instrument ate:

    countersOff   `POLYGON_NESTING_PROFILE` unset. A wall budget reads no
                  counter, so this is the engine with no instrument at all.
    countersOn    `POLYGON_NESTING_PROFILE=1`. Every counter and every span -
                  which is exactly what a work or plan budget has been arming
                  since the mode was built. This is §9's `countersOn` arm.
    meterOnly     `POLYGON_NESTING_METER=1`. The two counters the work meter is
                  denominated in, and no span.

`meterOnly` needs the harness to set the flag, because no *wall* budget would
arm it on its own - `lanedebit` is a portfolio setting and the portfolio only
consults it under a work or plan budget. That is the point of running it here:
this battery is about the instrument's cost, and it has to be able to arm the
instrument without also changing the budget mode.

The decomposition the three arms give:

    countersOn - countersOff   the whole tax (§9's number, re-measured)
    meterOnly  - countersOff   the part `lanedebit` cannot remove (the counting)
    countersOn - meterOnly     the part `lanedebit` removes (the timing)

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

ARMS = ['countersOff', 'countersOn', 'meterOnly']


def run_arm(binary, request, seed, spec, out_path, arm):
    """`runlib.run`, except that it may set one of the two recording flags.

    `runlib.run` unsets `POLYGON_NESTING_PROFILE` unconditionally, which is
    right for every other driver and wrong for this one: this battery's whole
    subject is those variables. Everything else - the argv, the allowance, the
    quality-trace scrub - is `runlib`'s, so the arms differ in one environment
    entry and nothing else.
    """
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    env = dict(os.environ)
    for key in ('POLYGON_NESTING_QUALITY_TRACE',
                'POLYGON_NESTING_QUALITY_TRACE_COUNTERS',
                'POLYGON_NESTING_PROFILE', 'POLYGON_NESTING_METER'):
        env.pop(key, None)
    if arm == 'countersOn':
        env['POLYGON_NESTING_PROFILE'] = '1'
    elif arm == 'meterOnly':
        env['POLYGON_NESTING_METER'] = '1'
    command = runlib.argv(binary, request, seed, spec)
    load_before = runlib.load_now()
    started = time.monotonic()
    with open(out_path, 'w') as handle:
        result = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                                check=False, env=env)
    wall = time.monotonic() - started
    runlib.LOAD.append({'out': out_path, 'wall': wall,
                        'before': load_before[0],
                        'after': runlib.load_now()[0]})
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
    rows = []
    for rnd in range(rounds):
        order = ARMS[rnd % len(ARMS):] + ARMS[:rnd % len(ARMS)]
        for arm in order:
            for seed in seeds:
                spec = runlib.spec_for(seed, 'wall', target_ms, True)
                tag = f'{arm}-s{seed}-r{rnd}'
                doc, wall, err = run_arm(binary, request, seed, spec,
                                         f'{outdir}/{tag}.json', arm)
                portfolio = doc.get('portfolio') or {}
                if not portfolio:
                    rows.append({'tag': tag, 'arm': arm, 'seed': seed,
                                 'round': rnd, 'error': err[-300:]})
                    print(f'{tag}: FAILED {err[-200:]}', flush=True)
                    continue
                rows.append({
                    'tag': tag, 'arm': arm, 'seed': seed, 'round': rnd,
                    'spec': spec,
                    'processWallSeconds': wall,
                    'rawDepthMm': portfolio['incumbent']['rawDepthMm'],
                    # Zero on `countersOff` by construction, and the point of
                    # reading it on the other two: `meterOnly` must produce the
                    # SAME work units as `countersOn` for the same search, or
                    # the two budgets are not the same budget.
                    'workUnits': portfolio['workUnits'],
                    'operatorCalls': len(portfolio.get('operatorCalls') or []),
                })
                print(f'{tag}: wall={wall:6.3f} '
                      f'depth={rows[-1]["rawDepthMm"]:.4f} '
                      f'units={rows[-1]["workUnits"]}', flush=True)

    good = [r for r in rows if 'error' not in r]
    summary = {'binary': binary,
               'binarySha256': runlib.binary_sha256(binary),
               'request': request, 'targetMs': target_ms,
               'seeds': seeds, 'rounds': rounds, 'arms': ARMS,
               'rows': rows, 'perSeed': {}}

    def median_for(seed, arm):
        values = [r['rawDepthMm'] for r in good
                  if r['seed'] == seed and r['arm'] == arm]
        return statistics.median(values) if values else None

    whole, counting, timing = [], [], []
    for seed in seeds:
        off = median_for(seed, 'countersOff')
        on = median_for(seed, 'countersOn')
        met = median_for(seed, 'meterOnly')
        if off is None or on is None or met is None:
            continue
        block = {
            'countersOffMedianMm': off,
            'countersOnMedianMm': on,
            'meterOnlyMedianMm': met,
            'wholeTaxMm': on - off,
            'countingTaxMm': met - off,
            'timingTaxMm': on - met,
        }
        for arm in ARMS:
            block[f'{arm}Distinct'] = sorted(
                {r['rawDepthMm'] for r in good
                 if r['seed'] == seed and r['arm'] == arm})
        summary['perSeed'][str(seed)] = block
        whole.append(block['wholeTaxMm'])
        counting.append(block['countingTaxMm'])
        timing.append(block['timingTaxMm'])
        print(f'seed {seed}: off={off:.4f} meter={met:.4f} on={on:.4f} '
              f'whole={block["wholeTaxMm"]:+.3f} '
              f'counting={block["countingTaxMm"]:+.3f} '
              f'timing={block["timingTaxMm"]:+.3f} mm', flush=True)
    if whole:
        summary['medianWholeTaxMm'] = statistics.median(whole)
        summary['medianCountingTaxMm'] = statistics.median(counting)
        summary['medianTimingTaxMm'] = statistics.median(timing)
    loads = [row['before'] for row in runlib.LOAD if row['before'] is not None]
    summary['boxLoad'] = {
        'n': len(loads),
        'min': min(loads) if loads else None,
        'median': statistics.median(loads) if loads else None,
        'max': max(loads) if loads else None,
    }
    json.dump(summary, open(f'{outdir}/metertax.json', 'w'), indent=1)
    print(json.dumps({k: v for k, v in summary.items() if k != 'rows'},
                     indent=1))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
