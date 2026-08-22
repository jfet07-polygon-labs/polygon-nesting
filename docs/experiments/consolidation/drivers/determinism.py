#!/usr/bin/env python3
"""Two processes, one budget, whole documents.

    python3 determinism.py OUTDIR BINARY REQUESTS SEEDS BUDGET_KEY VALUE [EXTRA]

The hard gate. Run twice with `work=<units>` and the two documents must be
identical, field for field, because a work budget is a function of counters and
not of the clock.

Run twice with `plan=<ms>` and the claim is *narrower and has to be stated as
two claims*, because the plan reads the clock exactly once:

  * `portfolio.plan.units` must agree - the two processes chose the same plan.
    This is the quantisation ladder's job and it can fail; when it does, that
    is a finding and not an error, and `planbattery.py` measures how often.
  * given that they did, the documents must be identical with
    `planCalibration` stripped - the clock reading itself differs by
    construction and is reported separately for exactly this reason.

So this driver reports both, and only calls a cell equal when both hold.
"""
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import planbattery  # noqa: E402
import runlib  # noqa: E402


def main():
    outdir, binary = sys.argv[1:3]
    requests = sys.argv[3].split(',')
    seeds = [int(v) for v in sys.argv[4].split(',')]
    budget_key, value = sys.argv[5:7]
    extra = sys.argv[7] if len(sys.argv) > 7 else ''
    os.makedirs(outdir, exist_ok=True)
    result = {'binary': binary, 'binarySha256': runlib.binary_sha256(binary),
              'budgetKey': budget_key, 'value': value,
              'extra': extra, 'requests': requests, 'seeds': seeds, 'rows': []}
    ok = True
    for request in requests:
        for seed in seeds:
            spec = runlib.spec_for(seed, budget_key, value, True, extra)
            tag = f'{request}-s{seed}'
            first, _, err_a = runlib.run(binary, request, seed, spec,
                                         f'{outdir}/{tag}-a.json')
            second, _, err_b = runlib.run(binary, request, seed, spec,
                                          f'{outdir}/{tag}-b.json')
            pa = first.get('portfolio') or {}
            pb = second.get('portfolio') or {}
            plan_a = (pa.get('plan') or {}).get('units')
            plan_b = (pb.get('plan') or {}).get('units')
            # The re-plan's half of the same claim, and it is a *sequence*: two
            # processes must agree on how many tranches they took and on what
            # each installed, not merely on the total. A run that took one
            # tranche of 24 M and one that took two summing to 24 M ran
            # different searches.
            tr_a = [t['units'] for t in (pa.get('tranches') or [])]
            tr_b = [t['units'] for t in (pb.get('tranches') or [])]
            left, right = planbattery.digest(first), planbattery.digest(second)
            row = {
                'tag': tag, 'spec': spec,
                'digestA': left, 'digestB': right,
                'planUnitsA': plan_a, 'planUnitsB': plan_b,
                'planAgrees': plan_a == plan_b,
                'tranchesA': tr_a, 'tranchesB': tr_b,
                'tranchesAgree': tr_a == tr_b,
                'documentEqual': left == right,
                'rawDepthMmA': (pa.get('incumbent') or {}).get('rawDepthMm'),
                'rawDepthMmB': (pb.get('incumbent') or {}).get('rawDepthMm'),
            }
            if not pa or not pb:
                row['error'] = (err_a or err_b)[-300:]
            row['equal'] = (row['documentEqual'] and row['planAgrees']
                            and row['tranchesAgree'])
            ok = ok and row['equal']
            result['rows'].append(row)
            print(f'{tag}: equal={row["equal"]} planAgrees={row["planAgrees"]} '
                  f'tranches={tr_a}/{tr_b} '
                  f'depth={row["rawDepthMmA"]}', flush=True)
    result['allEqual'] = ok
    result['allPlansAgree'] = all(r['planAgrees'] for r in result['rows'])
    result['allTranchesAgree'] = all(
        r['tranchesAgree'] for r in result['rows'])
    loads = [row['before'] for row in runlib.LOAD
             if row['before'] is not None]
    result['boxLoad'] = {
        'n': len(loads),
        'min': min(loads) if loads else None,
        'median': statistics.median(loads) if loads else None,
        'max': max(loads) if loads else None,
    }
    json.dump(result, open(f'{outdir}/determinism.json', 'w'), indent=1)
    print(f'allEqual={ok} allPlansAgree={result["allPlansAgree"]} '
          f'allTranchesAgree={result["allTranchesAgree"]}')
    return 0 if ok else 1


if __name__ == '__main__':
    raise SystemExit(main())
