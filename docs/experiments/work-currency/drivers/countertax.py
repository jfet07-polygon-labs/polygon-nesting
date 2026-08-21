#!/usr/bin/env python3
"""§6: `calibrated-plan` §9's 1.882 mm, re-measured, and what `cur2` does to it.

    python3 countertax.py OUT_JSON BINARY REQUEST TARGET_MS SEEDS ROUNDS

`calibrated-plan` §9 measured the work counters at **+1.882 mm** of depth on
mixed-61 at a ten-second wall - a floor under any work-denominated budget,
because a work budget is a reading of the counters - and Sol review 8 §3
condition 4 asks for a "debit lane-local economico" that would remove it. The
brief for this round asks whether comparable pricing recovers part of it.

Three arms, not two, and the third is the new question:

    countersOff   `POLYGON_NESTING_PROFILE` unset          the untaxed run
    countersOn    `POLYGON_NESTING_PROFILE=1`, `cur2=0`    `calibrated-plan`'s
                                                           own measurement
    countersOnCur2 `POLYGON_NESTING_PROFILE=1`, `cur2=1`   the counters plus
                                                           the currency's two
                                                           extra reads per call

Every arm is a **wall** budget, so all three spend the same seconds and the
difference is what the instrument ate. Note that `cur2` is inert as a *budget*
under a wall budget by construction (`debit_self_metered` returns zero), so the
third arm isolates the currency's own overhead - the `counter_totals()` calls
and the price - with none of its repricing.

Paired per (seed, round), arm order rotated.
"""
import json
import os
import statistics
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

ARMS = ['countersOff', 'countersOn', 'countersOnCur2']


def run_with_counters(binary, request, seed, spec, out_path, counters):
    """`runlib.run`, except that it may *set* `POLYGON_NESTING_PROFILE`.

    `runlib.run` unsets it unconditionally, which is right for every other
    driver here and wrong for this one: this battery's whole subject is that
    variable.
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
    before = runlib.load_now()
    started = time.monotonic()
    with open(out_path, 'w') as handle:
        result = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                                check=False, env=env)
    wall = time.monotonic() - started
    runlib.LOAD.append({'out': out_path, 'wall': wall, 'before': before[0],
                        'after': runlib.load_now()[0]})
    stderr = (result.stderr or b'').decode()[-1200:]
    try:
        with open(out_path) as handle:
            return json.load(handle), wall, stderr
    except json.JSONDecodeError:
        return {}, wall, stderr


def spec_for_arm(seed, target_ms, arm):
    extra = '' if arm == 'countersOff' else (
        'cur2=1' if arm == 'countersOnCur2' else 'cur2=0')
    return runlib.spec_for(seed, 'wall', target_ms, True, extra)


def main():
    out, binary, request, target_ms = sys.argv[1:5]
    seeds = [int(v) for v in sys.argv[5].split(',')]
    rounds = int(sys.argv[6])
    outdir = os.path.dirname(out)
    rows = []
    for rnd in range(rounds):
        order = ARMS[rnd % 3:] + ARMS[:rnd % 3]
        for arm in order:
            for seed in seeds:
                spec = spec_for_arm(seed, target_ms, arm)
                tag = f'ct-{arm}-s{seed}-r{rnd}'
                doc, wall, err = run_with_counters(
                    binary, request, seed, spec, f'{outdir}/{tag}.json',
                    arm != 'countersOff')
                portfolio = doc.get('portfolio') or {}
                incumbent = portfolio.get('incumbent') or {}
                rows.append({
                    'arm': arm, 'seed': seed, 'round': rnd, 'spec': spec,
                    'depthMm': incumbent.get('rawDepthMm'),
                    'processSeconds': wall,
                    'coordinatorSeconds': portfolio.get('elapsedSeconds'),
                    'operatorCalls': len(portfolio.get('operatorCalls') or []),
                    'publications': len(portfolio.get('publications') or []),
                    'workCurrency': portfolio.get('workCurrency'),
                    'stderrTail': err[-200:] if err else None,
                })
                print(f"r{rnd} {arm} s{seed}: {rows[-1]['depthMm']} "
                      f"@ {wall:.2f}s", flush=True)

    def medians(arm):
        by_seed = {}
        for row in rows:
            if row['arm'] == arm and row['depthMm'] is not None:
                by_seed.setdefault(row['seed'], []).append(row['depthMm'])
        return {seed: statistics.median(values)
                for seed, values in sorted(by_seed.items())}

    off, on, cur2 = medians('countersOff'), medians('countersOn'), \
        medians('countersOnCur2')
    per_seed = []
    for seed in sorted(off):
        per_seed.append({
            'seed': seed,
            'countersOff': off.get(seed),
            'countersOn': on.get(seed),
            'countersOnCur2': cur2.get(seed),
            # `calibrated-plan` §9's number, reproduced.
            'counterTaxMm': (on[seed] - off[seed]
                             if seed in on and seed in off else None),
            # What the currency's own instrumentation adds on top of it.
            'currencyTaxMm': (cur2[seed] - on[seed]
                              if seed in cur2 and seed in on else None),
            'totalMm': (cur2[seed] - off[seed]
                        if seed in cur2 and seed in off else None),
        })
    taxes = [r['counterTaxMm'] for r in per_seed if r['counterTaxMm'] is not None]
    currency = [r['currencyTaxMm'] for r in per_seed
                if r['currencyTaxMm'] is not None]
    document = {
        'binary': binary, 'binarySha256': runlib.sha256_of(binary),
        'request': request, 'targetMillis': int(target_ms), 'seeds': seeds,
        'rounds': rounds, 'rows': rows, 'perSeed': per_seed,
        'summary': {
            'medianCounterTaxMm': statistics.median(taxes) if taxes else None,
            'medianCurrencyTaxMm': (statistics.median(currency)
                                    if currency else None),
            'calibratedPlanReference': 1.882,
        },
        'boxLoad': runlib.LOAD,
    }
    with open(out, 'w') as handle:
        json.dump(document, handle, indent=1, sort_keys=True)
    print(json.dumps(document['summary'], indent=1))
    for row in per_seed:
        print(f"  seed {row['seed']}: off {row['countersOff']:.4f}  "
              f"on {row['countersOn']:.4f}  onCur2 {row['countersOnCur2']:.4f}"
              f"   tax {row['counterTaxMm']:+.4f}  currency "
              f"{row['currencyTaxMm']:+.4f}")
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
