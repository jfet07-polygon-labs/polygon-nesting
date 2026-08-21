#!/usr/bin/env python3
"""Where the armed arm's extra leaf time is, from the engine's own phase spans.

    phaseshare.py NAME REQUEST SEEDS BUDGETKEY BUDGETVALUE EXTRA BINARY

`POLYGON_NESTING_PROFILE=1` on both arms of `crot`, from the bare request, and
the difference of the leaf-phase tables. This is the instrument the §1
census cannot be: the census counts *calls*, and a call count only becomes a
cost once something says how long the call took. The phase spans are already
compiled into every build, they partition the search's leaf work, and they cost
the same in both arms - so their **difference** is an attribution even though
their absolute values are inflated by the profiler.

Profiled runs are not wall claims and their depths are not the battery's
depths; both arms pay the same instrument.
"""
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402


def run_profiled(binary, request, seed, spec, out_path):
    """`runlib.run`, except that it does **not** strip
    `POLYGON_NESTING_PROFILE`.

    `runlib.run` clears it on purpose - every wall driver in this repository
    must not accidentally measure an instrumented process - so a profiling
    driver has to open the door explicitly rather than by setting an
    environment variable and hoping.
    """
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    env = dict(os.environ)
    env['POLYGON_NESTING_PROFILE'] = '1'
    env.pop('POLYGON_NESTING_QUALITY_TRACE', None)
    command = runlib.argv(binary, request, seed, spec)
    started = time.monotonic()
    with open(out_path, 'w') as handle:
        result = subprocess.run(command, stdout=handle,
                                stderr=subprocess.PIPE, check=False, env=env)
    wall = time.monotonic() - started
    try:
        return json.load(open(out_path)), wall, ''
    except json.JSONDecodeError:
        return {'_loadError': (result.stderr or b'').decode()[-800:]}, wall, ''


def phases_of(doc):
    profile = doc.get('searchProfile') or {}
    out = {}
    for row in profile.get('phases') or []:
        out[row['phase']] = (row.get('milliseconds') or row.get('nanos', 0),
                             row.get('calls', 0))
    return out, (profile.get('counters') or {})


def main():
    name, request, seeds = sys.argv[1], sys.argv[2], \
        [int(s) for s in sys.argv[3].split(',')]
    budget_key, budget_value, extra = sys.argv[4], sys.argv[5], sys.argv[6]
    binary = sys.argv[7]
    out_dir = f'{runlib.OUT}/{name}'
    os.makedirs(out_dir, exist_ok=True)
    rows = []
    for seed in seeds:
        for arm, crot in (('base', 0), ('crot', 1)):
            spec = runlib.spec_for(seed, budget_key, budget_value, True,
                                   (extra + ',' if extra else '')
                                   + f'crot={crot}')
            doc, seconds, err = run_profiled(
                binary, request, seed, spec, f'{out_dir}/{arm}-s{seed}.json')
            phases, counters = phases_of(doc)
            portfolio = doc.get('portfolio') or {}
            m34 = [c for c in portfolio.get('operatorCalls', [])
                   if c.get('operator') == 'mode34']
            rows.append({'seed': seed, 'arm': arm, 'spec': spec,
                         'processSeconds': seconds,
                         'rawDepthMm': (portfolio.get('incumbent')
                                        or {}).get('rawDepthMm'),
                         'm34Slices': len(m34),
                         'phases': phases, 'counters': counters})
            print(f'{arm} s{seed}: {seconds:.2f}s slices={len(m34)}',
                  flush=True)
    report = {'binary': binary, 'request': request, 'rows': rows}
    json.dump(report, open(f'{out_dir}/phaseshare.json', 'w'), indent=1)

    names = set()
    for row in rows:
        names |= set(row['phases'])
    print(f"\n{'phase':34s} {'base ms':>12s} {'crot ms':>12s} "
          f"{'delta ms':>12s} {'base calls':>14s} {'crot calls':>14s}")
    totals = []
    for phase in sorted(names):
        base = sum(r['phases'].get(phase, (0, 0))[0] for r in rows
                   if r['arm'] == 'base')
        crot = sum(r['phases'].get(phase, (0, 0))[0] for r in rows
                   if r['arm'] == 'crot')
        base_calls = sum(r['phases'].get(phase, (0, 0))[1] for r in rows
                         if r['arm'] == 'base')
        crot_calls = sum(r['phases'].get(phase, (0, 0))[1] for r in rows
                         if r['arm'] == 'crot')
        totals.append((crot - base, phase, base, crot, base_calls, crot_calls))
    for delta, phase, base, crot, bc, cc in sorted(totals, reverse=True):
        print(f'{phase:34s} {base:12.1f} {crot:12.1f} {delta:12.1f} '
              f'{bc:14d} {cc:14d}')
    print('\ncounters')
    keys = set()
    for row in rows:
        keys |= set(row['counters'])
    for key in sorted(keys):
        base = sum(r['counters'].get(key, 0) for r in rows
                   if r['arm'] == 'base')
        crot = sum(r['counters'].get(key, 0) for r in rows
                   if r['arm'] == 'crot')
        print(f'{key:34s} {base:16d} {crot:16d} {crot - base:+16d}')


if __name__ == '__main__':
    main()
