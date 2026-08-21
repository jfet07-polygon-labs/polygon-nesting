#!/usr/bin/env python3
"""The §1 decomposition: where an armed slice's extra wall goes, per component.

    decompose.py OUTNAME REQUEST SEEDS BUDGETKEY BUDGETVALUE [EXTRA]

Runs both arms of `crot` on the **census** binary
(`--features ...,rotation-tax-census`) and reads the `rotationTaxCensus` line
off stderr beside the slice report the run already emits. The census build is
an instrument and never a wall claim - its own counters cost a contended
process-global atomic per pose resolution - so what this driver reports is
*call structure* plus the two regions the census times directly, and the wall
attribution of the rest is left to the ablation in §2.

The one wall number this driver does print, `perSliceSeconds`, is there to show
the census build reproduces the *shape* of the negative (an armed slice costs
about twice an unarmed one) and is deliberately not compared against the
measurement build's.
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

CENSUS = re.compile(r'rotationTaxCensus (.*)')

SLICE_KEYS = ('rotationRungsProposed', 'rotationRungsImproved',
              'mirrorTogglesProposed', 'mirrorTogglesImproved',
              'rotationSurrogateBuilds', 'rotationSurrogateHits',
              'rotationSurrogateEvictions', 'rotationSurrogateBuildMs',
              'rotationBuildsRefused', 'confirmationMs', 'repairMs')


def census_of(stderr):
    match = CENSUS.search(stderr or '')
    if not match:
        return {}
    out = {}
    for field in match.group(1).split():
        key, _, value = field.partition('=')
        out[key] = int(value)
    return out


def one(binary, request, seed, spec, out):
    doc, seconds, err = runlib.run(binary, request, seed, spec, out)
    portfolio = doc.get('portfolio') or {}
    slices = [call for call in portfolio.get('operatorCalls', [])
              if call.get('operator') == 'mode34']
    totals = {key: sum(((call.get('scheduleSlice') or {}).get(key) or 0)
                       for call in slices) for key in SLICE_KEYS}
    slice_seconds = sum((call.get('elapsedSeconds') or 0.0) for call in slices)
    return {
        'seed': seed, 'spec': spec, 'processSeconds': seconds,
        'rawDepthMm': (portfolio.get('incumbent') or {}).get('rawDepthMm'),
        'm34Slices': len(slices),
        'm34Published': sum(1 for call in slices if call.get('published')),
        'sliceSecondsTotal': slice_seconds,
        'perSliceSeconds': (slice_seconds / len(slices)) if slices else None,
        'sliceTotals': totals,
        'census': census_of(err),
        'loadError': doc.get('_loadError'),
    }


def main():
    name, request, seeds = sys.argv[1], sys.argv[2], \
        [int(s) for s in sys.argv[3].split(',')]
    budget_key, budget_value = sys.argv[4], sys.argv[5]
    extra = sys.argv[6] if len(sys.argv) > 6 else ''
    binary = os.environ.get('TAX_CENSUS_BIN', runlib.BIN)
    rows = []
    for seed in seeds:
        # Interleaved, base first, so a drifting box moves both arms together.
        for arm, crot in (('base', 0), ('crot', 1)):
            spec = runlib.spec_for(seed, budget_key, budget_value, True,
                                   (extra + ',' if extra else '')
                                   + f'crot={crot}')
            out = f'{runlib.OUT}/{name}/{arm}-s{seed}.json'
            row = one(binary, request, seed, spec, out)
            row['arm'] = arm
            rows.append(row)
            print(json.dumps(row))
            sys.stdout.flush()
    report = {'binary': binary, 'request': request, 'budget':
              f'{budget_key}={budget_value}', 'extra': extra, 'rows': rows}
    path = f'{runlib.OUT}/{name}/decompose.json'
    os.makedirs(os.path.dirname(path), exist_ok=True)
    json.dump(report, open(path, 'w'), indent=1)
    print(f'WROTE {path}')


if __name__ == '__main__':
    main()
