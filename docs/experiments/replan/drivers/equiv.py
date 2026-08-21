#!/usr/bin/env python3
"""Two binaries, one work budget, whole documents.

    python3 equiv.py OUTDIR BIN_A BIN_B REQUESTS SEEDS WORK [EXTRA_A] [EXTRA_B]

The refactor gate. A work budget is a function of the counters and not of the
clock, so two binaries that are semantically identical must produce the same
document to the last field. Used twice in this round:

  * the resumable-slice refactor against the base binary, `EXTRA_A == EXTRA_B`;
  * the batched slice against the monolithic one on the *same* binary, which is
    Sol review 8 §4 spend 1's concatenation gate - `EXTRA_B` carries `m34batch`.
"""
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import planbattery  # noqa: E402
import runlib  # noqa: E402


def slice_rows(doc):
    """Every mode-34 slice report in the document, in order."""
    portfolio = doc.get('portfolio') or {}
    return [call for call in (portfolio.get('operatorCalls') or [])
            if call.get('operator') == 'mode34' and call.get('scheduleSlice')]


# `planbattery.strip` drops `scheduleSlice` wholesale, which is right for a
# document digest and **wrong for this gate**: the slice report is precisely
# where a batching bug would show. So the concatenation gate uses its own
# stripper - the volatile *suffixes* only, plus the batching's own bookkeeping -
# and therefore compares every per-step row: each step's clamp, its sweeps, its
# candidate queries, its collision pairs before and after, and whether its
# confirmation was accepted.
SLICE_VOLATILE = {
    'executableSha256', 'engineWorktreeStatus', 'engineWorktreeDirty',
    'engineCommit', 'relevantSourceTreeSha256', 'rustflags', 'buildProfile',
    'cpuModel', 'actualThreads', 'requestedThreads', 'occupancyOverTime',
    'seconds', 'planCalibration',
    # the batching's own account of itself, which the monolith does not have
    'checkpoints', 'batchWorkUnits',
    # a key the base binary does not emit at all, so it cannot be compared
    # across binaries; it is compared *explicitly*, per slice, below, which is
    # the stronger of the two comparisons and the one the batching gate turns on
    'stepDigest',
}


def strip_strong(node):
    if isinstance(node, dict):
        return {k: strip_strong(v) for k, v in node.items()
                if k not in SLICE_VOLATILE
                and not k.endswith(planbattery.VOLATILE_SUFFIXES)
                and v is not None}
    if isinstance(node, list):
        return [strip_strong(v) for v in node]
    return node


def strong_digest(doc):
    return planbattery.hashlib.sha256(
        planbattery.json.dumps(strip_strong(doc),
                               sort_keys=True).encode()).hexdigest()[:16]


def main():
    outdir, bin_a, bin_b = sys.argv[1:4]
    requests = sys.argv[4].split(',')
    seeds = [int(v) for v in sys.argv[5].split(',')]
    work = sys.argv[6]
    extra_a = sys.argv[7] if len(sys.argv) > 7 else ''
    extra_b = sys.argv[8] if len(sys.argv) > 8 else ''
    os.makedirs(outdir, exist_ok=True)
    result = {'binaryA': bin_a, 'binaryB': bin_b, 'work': work,
              'extraA': extra_a, 'extraB': extra_b,
              'requests': requests, 'seeds': seeds, 'rows': []}
    ok = True
    for request in requests:
        for seed in seeds:
            tag = f'{request}-s{seed}'
            spec_a = runlib.spec_for(seed, 'work', work, True, extra_a)
            spec_b = runlib.spec_for(seed, 'work', work, True, extra_b)
            a, wa, ea = runlib.run(bin_a, request, seed, spec_a,
                                   f'{outdir}/{tag}-a.json')
            b, wb, eb = runlib.run(bin_b, request, seed, spec_b,
                                   f'{outdir}/{tag}-b.json')
            pa, pb = a.get('portfolio') or {}, b.get('portfolio') or {}
            # `checkpoints` is the batched arm's own account of where it
            # stopped; it does not exist on the monolith, so it is stripped
            # from both sides of the comparison and reported separately. It is
            # a *record* of the batching, not a product of the search.
            dl, dr = strong_digest(a), strong_digest(b)
            weak_l, weak_r = planbattery.digest(a), planbattery.digest(b)
            slices_a, slices_b = slice_rows(a), slice_rows(b)
            digests_a = [s['scheduleSlice'].get('stepDigest')
                         for s in slices_a]
            digests_b = [s['scheduleSlice'].get('stepDigest')
                         for s in slices_b]
            row = {
                'tag': tag, 'specA': spec_a, 'specB': spec_b,
                'sliceDigestA': dl, 'sliceDigestB': dr,
                'documentEqual': dl == dr,
                'weakDigestA': weak_l, 'weakDigestB': weak_r,
                'weakDigestEqual': weak_l == weak_r,
                'rawDepthMmA': (pa.get('incumbent') or {}).get('rawDepthMm'),
                'rawDepthMmB': (pb.get('incumbent') or {}).get('rawDepthMm'),
                'workUnitsA': pa.get('workUnits'),
                'workUnitsB': pb.get('workUnits'),
                'm34CallsA': len(slices_a), 'm34CallsB': len(slices_b),
                'stepDigestsA': digests_a, 'stepDigestsB': digests_b,
                # A binary that predates this round emits no `stepDigest` at
                # all, so the per-step claim is only *available* when both
                # sides carry one. Reported rather than silently downgraded:
                # the refactor gate below is a whole-document claim and the
                # batching gate is a whole-document claim *plus* this one, and
                # a reader has to be able to see which of the two a row is.
                'stepDigestsComparable': (
                    bool(digests_a) and all(d is not None for d in digests_a)
                    and all(d is not None for d in digests_b)),
                'stepDigestsEqual': digests_a == digests_b,
                'processSecondsA': wa, 'processSecondsB': wb,
            }
            # The batching's own account: how many batches each slice ran, and
            # what the deepest-confirmed slot held at each checkpoint.
            checks = []
            for call in slices_b:
                report = call.get('scheduleSlice') or {}
                cps = report.get('checkpoints') or []
                if cps:
                    checks.append({
                        'batches': len(cps),
                        'stepsTaken': report.get('stepsTaken'),
                        'workUnits': report.get('workUnits'),
                        'perBatchSteps': [c['stepsTaken'] for c in cps],
                        'floorMm': [c['floorMm'] for c in cps],
                        'publishedDepthMm': [c['publishedDepthMm']
                                             for c in cps],
                        'confirmationsAccepted': [c['confirmationsAccepted']
                                                  for c in cps],
                    })
            row['batchedSlices'] = checks
            row['totalBatches'] = sum(c['batches'] for c in checks)
            if not pa or not pb:
                row['error'] = (ea or eb)[-300:]
            # A cell passes only if both hold. `documentEqual` alone would pass
            # two slices that took different walks to the same aggregate; the
            # step digests alone would pass a run whose *coordinator* diverged.
            row['equal'] = row['documentEqual'] and (
                row['stepDigestsEqual']
                or not row['stepDigestsComparable'])
            ok = ok and row['equal']
            result['rows'].append(row)
            print(f'{tag}: equal={row["equal"]} '
                  f'stepDigests={row["stepDigestsEqual"]}'
                  f'{"" if row["stepDigestsComparable"] else "(n/a)"} '
                  f'depthA={row["rawDepthMmA"]} depthB={row["rawDepthMmB"]} '
                  f'm34={row["m34CallsA"]}/{row["m34CallsB"]} '
                  f'batches={row["totalBatches"]}', flush=True)
    result['allEqual'] = ok
    result['allStepDigestsEqual'] = all(
        r.get('stepDigestsEqual') for r in result['rows']
        if r.get('stepDigestsComparable'))
    result['stepDigestCells'] = sum(
        1 for r in result['rows'] if r.get('stepDigestsComparable'))
    result['totalBatches'] = sum(r.get('totalBatches', 0)
                                 for r in result['rows'])
    loads = [row['before'] for row in runlib.LOAD
             if row['before'] is not None]
    result['boxLoad'] = {
        'n': len(loads),
        'min': min(loads) if loads else None,
        'median': statistics.median(loads) if loads else None,
        'max': max(loads) if loads else None,
    }
    json.dump(result, open(f'{outdir}/equiv.json', 'w'), indent=1)
    print(f'allEqual={ok} totalBatches={result["totalBatches"]} '
          f'stepDigestCells={result["stepDigestCells"]} '
          f'allStepDigestsEqual={result["allStepDigestsEqual"]}')
    return 0 if ok else 1


def drop_checkpoints(node):
    if isinstance(node, dict):
        node.pop('checkpoints', None)
        node.pop('batchWorkUnits', None)
        for value in node.values():
            drop_checkpoints(value)
    elif isinstance(node, list):
        for value in node:
            drop_checkpoints(value)


if __name__ == '__main__':
    raise SystemExit(main())
