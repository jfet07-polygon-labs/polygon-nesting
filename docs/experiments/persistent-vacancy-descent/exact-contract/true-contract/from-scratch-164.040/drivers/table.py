#!/usr/bin/env python3
"""k x d results table for one mode-31 sweep directory."""

import glob
import json
import re
import sys

sys.path.insert(0, '/var/lib/t3/tmp/combo')
import base  # noqa: E402


def rows(outdir, incumbent_raw):
    table = {}
    for path in sorted(glob.glob(f'{outdir}/*.json')):
        name = path.split('/')[-1][:-5]
        match = re.match(r'^(ctl|k(\d+)-d([\d.]+))-b([\d.]+)$', name)
        if not match:
            continue
        key = 'control' if match.group(1) == 'ctl' else (int(match.group(2)),
                                                        float(match.group(3)))
        run_json = json.load(open(path))
        depth = base.published(run_json)
        pop = base.population(run_json) or {}
        entry = table.setdefault(key, {'best': None, 'bound': None, 'valid': 0,
                                       'runs': 0, 'reasons': {}})
        entry['runs'] += 1
        if depth is not None:
            entry['valid'] += 1
            raw = pop['rawSourceDepthMm']
            if entry['best'] is None or raw < entry['best']:
                entry['best'] = raw
                entry['bound'] = float(match.group(4))
        else:
            reason = re.sub(r'\d+', 'N', (pop.get('failureReason') or 'no population'))[:60]
            entry['reasons'][reason] = entry['reasons'].get(reason, 0) + 1
    return table


if __name__ == '__main__':
    outdir, incumbent = sys.argv[1], float(sys.argv[2])
    table = rows(outdir, incumbent)
    print(f'incumbent raw {incumbent:.6f}')
    for key in sorted(table, key=lambda k: (k == 'control', k)):
        e = table[key]
        best = f'{e["best"]:.6f}' if e['best'] is not None else 'none'
        gain = f'{incumbent - e["best"]:+.6f}' if e['best'] is not None else '     -'
        print(f'  {str(key):>12}: best {best} (gain {gain}) at eps {e["bound"]} '
              f'[{e["valid"]}/{e["runs"]} valid]')
        for reason, count in sorted(e['reasons'].items(), key=lambda r: -r[1]):
            print(f'                 x{count} {reason}')
