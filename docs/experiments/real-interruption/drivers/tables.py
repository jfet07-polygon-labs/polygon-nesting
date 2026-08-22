#!/usr/bin/env python3
"""Every table in this round's README, from the committed evidence.

    python3 tables.py docs/experiments/real-interruption/evidence

One function per table, so a reader who doubts a row can run the one function
that produced it. Nothing here recomputes a measurement: every number is read
out of a JSON file a driver wrote, and the only arithmetic is medians, deltas
and counts.
"""
import json
import os
import statistics
import sys


def load(root, name):
    path = os.path.join(root, name)
    if not os.path.exists(path):
        print(f'-- {name}: MISSING')
        return None
    with open(path) as handle:
        return json.load(handle)


def rule(title):
    print()
    print('=' * 78)
    print(title)
    print('=' * 78)


def capreplay(doc):
    """The retraction: m34cap=0 against m34cap=1, at a work budget."""
    if not doc:
        return
    rule('The m34cap replay - what the cap actually changed')
    print(f'binary {doc.get("binarySha256")}')
    print('| seed | depth m34cap=0 | depth m34cap=1 | equal | work | calls | '
          'step digests | doc without slice | doc with slice |')
    print('|---:|---:|---:|:--:|---:|---:|:--:|:--:|:--:|')
    for row in doc['rows']:
        if 'error' in row:
            print(f'| {row["seed"]} | ERROR | | | | | | | |')
            continue
        print(f'| {row["seed"]} | {row["depthOff"]} | {row["depthOn"]} | '
              f'{row["depthEqual"]} | {row["workOff"]:,} | {row["callsOff"]} | '
              f'{row["stepDigestsEqual"]} | {row["digestNoSliceEqual"]} | '
              f'{row["digestWithSliceEqual"]} |')


def gates(doc):
    if not doc:
        return
    rule('The four pinned regression gates')
    for tag, gate in doc['gates'].items():
        value = gate.get('raw', gate.get('depths', [None])[0])
        fingerprint = gate.get('fp') or (gate.get('fingerprints') or [''])[-1]
        print(f'{tag}: hit={gate["hit"]} {value} {str(fingerprint)[:16]} '
              f'({gate["wallSeconds"]:.1f} s)')
    print(f'ALL_PASS {doc["ALL_PASS"]}')


def equivalence(doc, title):
    if not doc:
        return
    rule(title)
    print(f'A {doc.get("binaryAsha256", "")[:16]}  extraA={doc["extraA"]!r}')
    print(f'B {doc.get("binaryBsha256", "")[:16]}  extraB={doc["extraB"]!r}')
    print('| cell | document equal | step digests equal | depth A | depth B | '
          'batches |')
    print('|---|:--:|:--:|---:|---:|---:|')
    for row in doc['rows']:
        print(f'| {row["tag"]} | {row["documentEqual"]} | '
              f'{row["stepDigestsEqual"]}'
              f'{"" if row["stepDigestsComparable"] else " (n/a)"} | '
              f'{row["rawDepthMmA"]} | {row["rawDepthMmB"]} | '
              f'{row.get("totalBatches", 0)} |')
    print(f'allEqual={doc["allEqual"]} totalBatches={doc["totalBatches"]} '
          f'allStepDigestsEqual={doc["allStepDigestsEqual"]}')


def determinism(doc, title):
    if not doc:
        return
    rule(title)
    print(f'binary {doc.get("binarySha256", "")[:16]} extra={doc["extra"]!r} '
          f'{doc["budgetKey"]}={doc["value"]}')
    for row in doc['rows']:
        print(f'  {row["tag"]}: equal={row["equal"]} depth={row["rawDepthMmA"]}')
    print(f'allEqual={doc["allEqual"]}')


def boundsweep(doc, title):
    if not doc:
        return
    rule(title)
    print(f'binary {doc.get("binarySha256", "")[:16]}  '
          f'{doc["request"]} target={doc["target"]}ms '
          f'seeds={doc["seeds"]} rounds={doc["rounds"]}')
    load = doc.get('boxLoad') or {}
    print(f'load1 min {load.get("min")} / median {load.get("median")} / '
          f'max {load.get("max")} over {load.get("n")} runs')
    base = doc['cells'].get('base', {}).get('medianOfSeedMedians')
    print()
    print('| arm | depth (median of seed medians) | vs base | wall p50 | '
          'wall max | over target | first-slice drop | first-slice exit | '
          'batches | resumes | interrupted |')
    print('|---|---:|---:|---:|---:|---:|---:|---|---:|---:|---:|')
    for arm in doc['arms']:
        cell = doc['cells'].get(arm)
        if not cell:
            continue
        rows = [r for r in doc['rows'] if r.get('arm') == arm
                and 'rawDepthMm' in r]
        drops = [r['firstSliceDropMm'] for r in rows
                 if r.get('firstSliceDropMm') is not None]
        exits = sorted({r['firstSliceExit'] for r in rows
                        if r.get('firstSliceExit')})
        depth = cell['medianOfSeedMedians']
        delta = '' if base is None else f'{depth - base:+.3f}'
        print(f'| {arm} | {depth:.3f} | {delta} | {cell["wallP50"]:.2f} s | '
              f'{cell["wallMax"]:.2f} s | {cell["overran"]}/{cell["runs"]} | '
              f'{statistics.median(drops):.4f} | {"/".join(exits)} | '
              f'{statistics.median(r["batchesTotal"] for r in rows):.0f} | '
              f'{sum(r["resumptionsTotal"] for r in rows)} | '
              f'{sum(r["interruptedSlices"] for r in rows)} |')
    print()
    print('Per seed, so a median of three is not read as three agreeing runs:')
    print('| arm | ' + ' | '.join(f'seed {s}' for s in doc['seeds'])
          + ' | distinct depths | distinct documents |')
    print('|---|' + '---:|' * (len(doc['seeds']) + 2))
    for arm in doc['arms']:
        cell = doc['cells'].get(arm)
        if not cell:
            continue
        per = cell['perSeed']
        depths = ' | '.join(
            f'{per[str(s)]["depth"]:.3f}' if str(s) in per
            else (f'{per[s]["depth"]:.3f}' if s in per else '-')
            for s in doc['seeds'])
        distinct = sum((per.get(str(s)) or per.get(s) or {}).get(
            'distinctDepths', 0) for s in doc['seeds'])
        docs = sum((per.get(str(s)) or per.get(s) or {}).get(
            'distinctDigests', 0) for s in doc['seeds'])
        print(f'| {arm} | {depths} | {distinct} | {docs} |')


SPARROW = {
    ('mixed-61', '3000'): 157.971,
    ('mixed-61', '10000'): 150.165,
}


def anytime(doc, title):
    if not doc:
        return
    rule(title)
    print(f'binary {doc.get("binarySha256", "")[:16]} lever={doc.get("lever")!r}')
    print('| fixture | target | arm | median depth | reproduced | wall max | '
          'Sparrow | gap |')
    print('|---|---:|---|---:|---:|---:|---:|---:|')
    for key, cell in doc['table'].items():
        request, target, arm = key.split('|')
        sparrow = SPARROW.get((request, target))
        gap = ('' if sparrow is None
               else f'{cell["medianDepthMm"] - sparrow:+.3f}')
        print(f'| {request} | {target} | {arm} | {cell["medianDepthMm"]:.3f} | '
              f'{cell["reproducedCells"]}/{cell["n"]} | '
              f'{cell["wallMaxSeconds"]:.2f} s | '
              f'{sparrow if sparrow else "-"} | {gap} |')


def main():
    root = sys.argv[1] if len(sys.argv) > 1 else 'evidence'
    capreplay(load(root, 'capreplay-30M.json'))
    gates(load(root, 'gates-ship.json'))
    equivalence(load(root, 'equiv-base.json'),
                'The refactor equivalence: base binary vs this one, unarmed')
    for batch in ('25000', '400000', '2000000'):
        equivalence(load(root, f'concat-{batch}.json'),
                    f'The concatenation gate at m34batch={batch}')
    for tag, title in (
            ('base', 'Determinism, two processes, work mode, nothing armed'),
            ('m34past1', 'Determinism, two processes, work mode, m34past=1'),
            ('m34past1m34yield2',
             'Determinism, two processes, work mode, m34past=1,m34yield=2'),
            ('m34batch400000',
             'Determinism, two processes, work mode, m34batch=400000')):
        determinism(load(root, f'determinism-{tag}.json'), title)
    boundsweep(load(root, 'bound-10s.json'),
               'The bound lever at ten seconds, mixed-61')
    boundsweep(load(root, 'bound-30s.json'),
               'The bound lever at thirty seconds, mixed-61')
    boundsweep(load(root, 'density-past.json'),
               'The density point, with the bound unlocked')
    anytime(load(root, 'anytime.json'),
            'The anytime table at three and ten seconds')
    anytime(load(root, 'anytime30.json'), 'The thirty-second cell')


if __name__ == '__main__':
    main()
