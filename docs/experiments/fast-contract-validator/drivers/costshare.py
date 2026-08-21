#!/usr/bin/env python3
"""The validator's share of a full from-request run, before and after.

    python3 costshare.py OUTDIR OFF_BINARY ON_BINARY SECONDS SEEDS

docs/experiments/parallel-compression-schedule/ §3 is the reason this feature
exists, and it measured the validator's cost inside a mode-34 *slice*. This
driver asks the wider question the owner's priority is denominated in: of a
whole ten-second run from a bare request, how much is `publicationValidate`, and
what is left of it afterwards.

`POLYGON_NESTING_PROFILE=1` compiles-in nothing - the spans are always there -
but it does cost something to record them, so these runs are NOT the wall runs.
The share is the honest reading; the milliseconds are a decomposition and are
quoted as one. Shares are taken against `leafMilliseconds`, the leaf-phase
total, because enclosing phases double-count the spans inside them - the same
convention `search_profile_json` documents.
"""
import hashlib
import json
import os
import statistics
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

BASE = 'wall={ms},cells={cells},v3=1,m34lanes=1,m34pconfirm=1'
WATCH = ('publicationValidate', 'exactOverlapTest', 'collisionPolygonBuild')


def run(binary, seed, spec, path):
    args = [a.format(seed=seed) for a in runlib.ARGS]
    tail = ['0', '', '', '', runlib.DEFAULT_ALLOWANCE, spec]
    env = dict(os.environ)
    env['POLYGON_NESTING_PROFILE'] = '1'
    env.pop('POLYGON_NESTING_COMPRESSION_SCHEDULE', None)
    env.pop('POLYGON_NESTING_CONTRACT_VALIDATOR_CENSUS', None)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    started = time.monotonic()
    with open(path, 'w') as handle:
        proc = subprocess.run([binary, runlib.REQUESTS['mixed-61']] + args + tail,
                              stdout=handle, stderr=subprocess.PIPE,
                              check=False, env=env)
    wall = time.monotonic() - started
    try:
        doc = json.load(open(path))
    except json.JSONDecodeError:
        return {'error': (proc.stderr or b'').decode()[-400:],
                'processWallSeconds': wall}
    profile = doc.get('searchProfile') or {}
    phases = {p['phase']: p for p in profile.get('phases') or []}
    portfolio = doc.get('portfolio') or {}
    row = {
        'processWallSeconds': wall,
        'leafMilliseconds': profile.get('leafMilliseconds'),
        'rawDepthMm': (portfolio.get('incumbent') or {}).get('rawDepthMm'),
        'threads': profile.get('threads'),
    }
    for name in WATCH:
        phase = phases.get(name) or {}
        row[name] = {
            'milliseconds': phase.get('milliseconds'),
            'calls': phase.get('calls'),
            'leafSharePercent': phase.get('leafSharePercent'),
        }
    return row


def main():
    outdir, off_binary, on_binary = sys.argv[1], sys.argv[2], sys.argv[3]
    seconds = float(sys.argv[4])
    seeds = [int(s) for s in sys.argv[5].split(',')]
    binaries = {'off': off_binary, 'on': on_binary}
    result = {'binaries': binaries,
              'binarySha256': {k: hashlib.sha256(open(v, 'rb').read()).hexdigest()
                               for k, v in binaries.items()},
              'seconds': seconds, 'seeds': seeds,
              'note': 'profiled runs; the share is the reading, the '
                      'milliseconds are a decomposition and not a wall claim',
              'observations': []}
    os.makedirs(outdir, exist_ok=True)
    for seed in seeds:
        for flag in ('off', 'on'):
            spec = BASE.format(
                ms=int(seconds * 1000),
                cells=runlib.SALT_SETS[seed % len(runlib.SALT_SETS)])
            row = run(binaries[flag], seed, spec,
                      f'{outdir}/s{seed}-{flag}.json')
            row.update({'seed': seed, 'flag': flag})
            result['observations'].append(row)
            print(json.dumps({k: row.get(k) for k in
                              ('seed', 'flag', 'publicationValidate',
                               'leafMilliseconds', 'rawDepthMm')}))
    summary = {}
    for flag in ('off', 'on'):
        rows = [r for r in result['observations'] if r['flag'] == flag
                and 'publicationValidate' in r]
        shares = [r['publicationValidate']['leafSharePercent'] for r in rows
                  if r['publicationValidate']['leafSharePercent'] is not None]
        ms = [r['publicationValidate']['milliseconds'] for r in rows
              if r['publicationValidate']['milliseconds'] is not None]
        calls = [r['publicationValidate']['calls'] for r in rows
                 if r['publicationValidate']['calls'] is not None]
        summary[flag] = {
            'n': len(rows),
            'medianLeafSharePercent': statistics.median(shares) if shares else None,
            'medianMilliseconds': statistics.median(ms) if ms else None,
            'medianCalls': statistics.median(calls) if calls else None,
            'medianLeafMilliseconds': statistics.median(
                [r['leafMilliseconds'] for r in rows
                 if r.get('leafMilliseconds') is not None]) if rows else None,
        }
    result['summary'] = summary
    json.dump(result, open(f'{outdir}/costshare.json', 'w'), indent=1)
    print(json.dumps(summary, indent=1))


if __name__ == '__main__':
    main()
