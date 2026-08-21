#!/usr/bin/env python3
"""Sol review 9's P0 replay: does `m34cap` change anything but the report?

    python3 capreplay.py OUTDIR BINARY REQUEST WORK_UNITS SEEDS

The committed `docs/experiments/replan/` §12.3 attributes a 162.846 -> 165.935
depth move on mixed-61 seed 1 to `m34cap=1`. Sol review 9 §"P0" says that is
impossible at that HEAD, because `advance()` records a checkpoint and leaves
`finished=false` while the caller loops `while !slice.finished` to the end of
the monolith: the coordinator never regains control, so the cap can change the
*checkpoint report* and nothing else.

This driver is the falsifiable form of that claim. It runs the two arms at a
**work** budget - the reproducible currency, so two arms of one binary are two
runs of one trajectory unless something really diverged - and reports three
comparisons of the pair:

  * `digestEqual`      - whole document, volatile keys stripped, `scheduleSlice`
                         **kept**. This is the strict one and it is expected to
                         *fail* if and only if the checkpoint list differs.
  * `digestNoSliceEqual` - the same document with the whole `scheduleSlice`
                         block dropped. If the cap only changes the report,
                         this is equal.
  * the trajectory columns - raw depth, work units, operator calls, and the
                         per-slice `stepDigest` list, which is the instrument
                         that can see a walk that diverged and re-converged.

A pass of the retraction is: `digestNoSliceEqual` true, depth/work/calls equal,
step digests equal, and the *only* difference inside `scheduleSlice` is
`checkpoints`.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import planbattery  # noqa: E402
import runlib  # noqa: E402

# The digest used for the strict comparison keeps `scheduleSlice`, because the
# whole question is whether the cap moved anything *outside* it.
STRICT_VOLATILE = planbattery.VOLATILE - {'scheduleSlice'}


def digest_with_slice(doc):
    import hashlib

    def volatile(key):
        return key in STRICT_VOLATILE or key.endswith(
            planbattery.VOLATILE_SUFFIXES)

    def strip(node):
        if isinstance(node, dict):
            return {k: strip(v) for k, v in node.items()
                    if not volatile(k) and v is not None}
        if isinstance(node, list):
            return [strip(v) for v in node]
        return node

    return hashlib.sha256(
        json.dumps(strip(doc), sort_keys=True).encode()).hexdigest()[:16]


def slices_of(doc):
    """Every mode-34 slice report in the document, in order.

    Keyed on `stepDigest`, which only a slice report carries. A checkpoint
    entry also has `stepsTaken` and `workUnits`, so keying on those would make
    the checkpoint list part of the trajectory comparison and the driver would
    report a divergence that is only the report.
    """
    found = []

    def walk(node):
        if isinstance(node, dict):
            if 'stepDigest' in node:
                found.append(node)
            for value in node.values():
                walk(value)
        elif isinstance(node, list):
            for value in node:
                walk(value)

    walk((doc.get('portfolio') or {}).get('operatorCalls') or [])
    if not found:
        walk(doc)
    return found


def slice_columns(doc):
    out = []
    for report in slices_of(doc):
        out.append({
            'stepDigest': report.get('stepDigest'),
            'stepsTaken': report.get('stepsTaken'),
            'workUnits': report.get('workUnits'),
            'finalDepthMm': report.get('finalDepthMm'),
            'exit': report.get('exit'),
            'checkpoints': len(report.get('checkpoints') or []),
        })
    return out


def main():
    outdir, binary, request, work = sys.argv[1:5]
    seeds = [int(v) for v in sys.argv[5].split(',')]
    os.makedirs(outdir, exist_ok=True)
    rows = []
    for seed in seeds:
        arms = {}
        for arm, cap in (('capoff', 0), ('capon', 1)):
            spec = runlib.spec_for(seed, 'work', work, True, f'm34cap={cap}')
            doc, wall, err = runlib.run(binary, request, seed, spec,
                                        f'{outdir}/{arm}-s{seed}.json')
            portfolio = doc.get('portfolio') or {}
            if not portfolio:
                print(f'{arm} s{seed}: FAILED {err[-300:]}', flush=True)
                arms[arm] = None
                continue
            arms[arm] = {
                'spec': spec,
                'wall': wall,
                'rawDepthMm': portfolio['incumbent']['rawDepthMm'],
                'fingerprint': portfolio['incumbent'].get('fingerprint'),
                'workUnits': portfolio.get('workUnits'),
                'operatorCalls': len(portfolio.get('operatorCalls') or []),
                'digestNoSlice': planbattery.digest(doc),
                'digestWithSlice': digest_with_slice(doc),
                'slices': slice_columns(doc),
            }
        off, on = arms['capoff'], arms['capon']
        if not off or not on:
            rows.append({'seed': seed, 'error': 'a run failed'})
            continue
        row = {
            'seed': seed,
            'specOff': off['spec'], 'specOn': on['spec'],
            'depthOff': off['rawDepthMm'], 'depthOn': on['rawDepthMm'],
            'depthEqual': off['rawDepthMm'] == on['rawDepthMm'],
            'fingerprintEqual': off['fingerprint'] == on['fingerprint'],
            'workOff': off['workUnits'], 'workOn': on['workUnits'],
            'workEqual': off['workUnits'] == on['workUnits'],
            'callsOff': off['operatorCalls'], 'callsOn': on['operatorCalls'],
            'callsEqual': off['operatorCalls'] == on['operatorCalls'],
            'digestNoSliceEqual': off['digestNoSlice'] == on['digestNoSlice'],
            'digestWithSliceEqual':
                off['digestWithSlice'] == on['digestWithSlice'],
            'stepDigestsEqual':
                [s['stepDigest'] for s in off['slices']]
                == [s['stepDigest'] for s in on['slices']],
            'slicesOff': off['slices'], 'slicesOn': on['slices'],
            'wallOff': off['wall'], 'wallOn': on['wall'],
        }
        rows.append(row)
        print(f"seed {seed}: depth {row['depthOff']} / {row['depthOn']} "
              f"equal={row['depthEqual']} work={row['workEqual']} "
              f"calls={row['callsEqual']} "
              f"docNoSlice={row['digestNoSliceEqual']} "
              f"docWithSlice={row['digestWithSliceEqual']}", flush=True)
    summary = {
        'binary': binary, 'binarySha256': runlib.binary_sha256(binary),
        'request': request, 'work': work, 'seeds': seeds,
        'rows': rows, 'boxLoad': runlib.LOAD,
    }
    with open(f'{outdir}/summary.json', 'w') as handle:
        json.dump(summary, handle, indent=2)
    print(json.dumps({k: v for k, v in summary.items()
                      if k not in ('rows', 'boxLoad')}, indent=2))


if __name__ == '__main__':
    main()
