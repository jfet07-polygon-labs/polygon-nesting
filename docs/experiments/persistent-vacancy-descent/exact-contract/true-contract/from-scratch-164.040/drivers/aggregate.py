#!/usr/bin/env python3
"""Session-wide k x d table: every mode-31 run this session produced."""

import glob
import json
import re
import sys

sys.path.insert(0, '/var/lib/t3/tmp/combo')
import base  # noqa: E402

PATTERN = re.compile(r'^(ctl|k(\d+)-d([\d.]+))-b([\d.]+)$')


def main():
    agg = {}
    for path in sorted(glob.glob('/var/lib/t3/tmp/combo/**/*.json', recursive=True)):
        name = path.split('/')[-1][:-5]
        match = PATTERN.match(name)
        if not match:
            continue
        key = 'control' if match.group(1) == 'ctl' else (int(match.group(2)),
                                                         float(match.group(3)))
        try:
            run_json = json.load(open(path))
        except Exception:
            continue
        pop = base.population(run_json)
        if pop is None or pop.get('mode') != 31:
            continue
        entry = agg.setdefault(key, {'runs': 0, 'valid': 0, 'best_gain': None,
                                     'best_tag': None, 'eps': {}})
        entry['runs'] += 1
        eps = float(match.group(4))
        slot = entry['eps'].setdefault(eps, [0, 0])
        slot[0] += 1
        if base.published(run_json) is None:
            continue
        entry['valid'] += 1
        slot[1] += 1
        # The round's incumbent is recoverable from the bound: every job was
        # dispatched at bound = incumbent_raw - eps, so gain against the
        # INCUMBENT (not against the perturbed state's own shallower depth,
        # which is what parentIndependentDepthMm reports) is bound + eps - published.
        bound = (pop.get('globalLegalization') or {}).get('boundMm')
        gain = None if bound is None else (bound + eps) - pop['rawSourceDepthMm']
        if gain is not None and (entry['best_gain'] is None or gain > entry['best_gain']):
            entry['best_gain'] = gain
            entry['best_tag'] = path

    print(f'{"case":>10} {"valid/runs":>12} {"rate":>7}   best single-round drop')
    for key in sorted(agg, key=lambda k: (k == 'control', k)):
        e = agg[key]
        rate = e['valid'] / e['runs'] if e['runs'] else 0.0
        gain = f"{e['best_gain']:.4f} mm" if e['best_gain'] is not None else 'none'
        print(f'{str(key):>10} {e["valid"]:>5}/{e["runs"]:<6} {rate:>6.1%}   {gain}')
    print()
    print('publication rate by bound offset eps (perturbed cases only):')
    by_eps = {}
    for key, e in agg.items():
        if key == 'control':
            continue
        for eps, (runs, valid) in e['eps'].items():
            slot = by_eps.setdefault(eps, [0, 0])
            slot[0] += runs
            slot[1] += valid
    for eps in sorted(by_eps):
        runs, valid = by_eps[eps]
        print(f'  eps {eps:<6} {valid:>4}/{runs:<4} {valid / runs:>6.1%}')


if __name__ == '__main__':
    main()
