#!/usr/bin/env python3
"""§3.3: two binaries of this tree, on the **armed** arm, must agree.

`equiv.py` proves the currency is inert when it is off. This proves a
different and narrower thing: that a source change made *after* the wall-
sensitive batteries ran did not change what those batteries measured.

The campaign's rule for that situation is `docs/experiments/replan/` §8.1 -
name the delta, say why it is unreachable, and **re-run the gates that can be
affected rather than argue about them**. This is the re-run half. Both
binaries are given `cur2=1`, which is the arm the race and plan batteries were
taken on, at a pinned work budget so the comparison is a function of counters
and not of the clock.

    python3 binequiv.py OUT_JSON BINARY_A BINARY_B [SPEC_EXTRA] [work_units]
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

CELLS = [(request, seed)
         for request in ('mixed-61', 'shapes-17', 'triangle-20')
         for seed in (0, 1, 2)]


def main():
    out, left_bin, right_bin = sys.argv[1:4]
    extra = sys.argv[4] if len(sys.argv) > 4 else 'cur2=1'
    units = int(sys.argv[5]) if len(sys.argv) > 5 else runlib.WORK_10S
    outdir = os.path.dirname(out)
    rows = []
    for request, seed in CELLS:
        spec = runlib.spec_for(seed, 'work', units, True, extra)
        left, left_wall, _ = runlib.run(
            left_bin, request, seed, spec,
            f'{outdir}/binequiv-{request}-s{seed}-a.json')
        right, right_wall, _ = runlib.run(
            right_bin, request, seed, spec,
            f'{outdir}/binequiv-{request}-s{seed}-b.json')
        rows.append({
            'request': request, 'seed': seed, 'spec': spec,
            'digests': [runlib.doc_digest(left), runlib.doc_digest(right)],
            'equal': runlib.doc_digest(left) == runlib.doc_digest(right),
            'leafDiff': runlib.leaf_diff(left, right),
            'depths': [((d.get('portfolio') or {}).get('incumbent') or {}).get(
                'rawDepthMm') for d in (left, right)],
            'workUnits': [(d.get('portfolio') or {}).get('workUnits')
                          for d in (left, right)],
            'classUnits': [((d.get('portfolio') or {}).get('workCurrency')
                            or {}).get('classUnits') for d in (left, right)],
            'walls': [left_wall, right_wall],
        })
        print(f"{request} s{seed}: equal={rows[-1]['equal']} "
              f"depths={rows[-1]['depths']} "
              f"work={rows[-1]['workUnits']} "
              f"class={rows[-1]['classUnits']}", flush=True)
    document = {
        'left': left_bin, 'leftSha256': runlib.sha256_of(left_bin),
        'right': right_bin, 'rightSha256': runlib.sha256_of(right_bin),
        'extra': extra, 'workUnits': units, 'rows': rows,
        'allEqual': all(r['equal'] for r in rows),
        'equalCells': sum(r['equal'] for r in rows), 'cells': len(rows),
        'boxLoad': runlib.LOAD,
    }
    with open(out, 'w') as handle:
        json.dump(document, handle, indent=1, sort_keys=True)
    print(f"allEqual={document['allEqual']} "
          f"{document['equalCells']}/{document['cells']}")
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
