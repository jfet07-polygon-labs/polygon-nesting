#!/usr/bin/env python3
"""Two processes, one work budget, whole documents.

    determinism.py NAME REQUESTS SEEDS WORKUNITS [EXTRA]

Work-budget mode is deterministic and load-independent by construction, so two
processes of the same binary on the same cell must produce the same document.
This is the check that the mechanisms this round adds - a queue filter, a
re-priced class and a step-counted probe inside the operator - did not make the
search a function of the clock. `EXTRA` defaults to the shipping arm.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import reproduce  # noqa: E402
import runlib  # noqa: E402


def main():
    name = sys.argv[1]
    requests = sys.argv[2].split(',')
    seeds = [int(v) for v in sys.argv[3].split(',')]
    units = sys.argv[4]
    extra = sys.argv[5] if len(sys.argv) > 5 else ''
    out_dir = f'{runlib.OUT}/{name}'
    result = {'name': name, 'binary': runlib.BIN, 'workUnits': units,
              'extra': extra, 'rows': []}
    ok = True
    for request in requests:
        for seed in seeds:
            spec = runlib.spec_for(seed, 'work', units, True, extra)
            tag = f'{request}-s{seed}'
            first, _, _ = runlib.run(runlib.BIN, request, seed, spec,
                                     f'{out_dir}/{tag}-a.json')
            second, _, _ = runlib.run(runlib.BIN, request, seed, spec,
                                      f'{out_dir}/{tag}-b.json')
            left, right = reproduce.digest(first), reproduce.digest(second)
            row = {'tag': tag, 'spec': spec, 'digestA': left,
                   'digestB': right, 'equal': left == right,
                   'rawDepthMmA': first.get('portfolio', {})
                   .get('incumbent', {}).get('rawDepthMm'),
                   'rawDepthMmB': second.get('portfolio', {})
                   .get('incumbent', {}).get('rawDepthMm')}
            ok = ok and row['equal']
            result['rows'].append(row)
            print(f'{tag}: equal={row["equal"]} {row["rawDepthMmA"]}',
                  flush=True)
    result['allEqual'] = ok
    os.makedirs(out_dir, exist_ok=True)
    json.dump(result, open(f'{out_dir}/determinism.json', 'w'), indent=1)
    print(f'allEqual={ok}')
    return 0 if ok else 1


if __name__ == '__main__':
    raise SystemExit(main())
