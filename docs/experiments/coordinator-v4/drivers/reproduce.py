#!/usr/bin/env python3
"""Does this tree's binary, with the three v4 keys off, still run merged-HEAD v3?

The four pinned gates never enter the coordinator - they are pinned-parent
positional replays - so they cannot answer this. This can: it runs the *same*
coordinator path, on the same request, at the same work budget, on the pristine
base-commit binary and on this tree's, and compares the two documents field by
field with the wall-clock and build-identity fields removed.

A work budget is a function of the evaluation counters and nothing else, so two
binaries that agree here agree on every branch the schedule took, every
affordability decision it made and every unit it spent - not merely on the depth
it reached.

    reproduce.py OUT.json PRISTINE-BINARY AFTER-BINARY REQUEST SEEDS WORK [EXTRA]

`EXTRA` defaults to `sched=0,barren=0,divq=0`, which is the merged-HEAD v3
configuration. The pristine binary does not know those keys, so it is run
without them.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import determinism  # noqa: E402
import runlib  # noqa: E402


def main():
    out_path = sys.argv[1]
    pristine = sys.argv[2]
    after = sys.argv[3]
    request = sys.argv[4]
    seeds = [int(value) for value in sys.argv[5].split(',')]
    work = int(sys.argv[6])
    extra = sys.argv[7] if len(sys.argv) > 7 else 'sched=0,barren=0,divq=0'

    out_dir = f'{runlib.OUT}/reproduce'
    rows = []
    for seed in seeds:
        docs = {}
        for label, binary, keys in (('base', pristine, ''),
                                    ('after', after, extra)):
            spec = runlib.spec_for(seed, 'work', work, True, keys)
            doc, wall, err = runlib.run(
                binary, request, seed, spec,
                f'{out_dir}/{request}-s{seed}-{label}.json')
            docs[label] = doc
            print(f'{label} s{seed}: '
                  f"{doc.get('portfolio', {}).get('incumbent', {}).get('rawDepthMm')} "
                  f"{doc.get('portfolio', {}).get('workUnits')} {wall:.1f}s",
                  flush=True)
        left = determinism.flatten(docs['base'])
        right = determinism.flatten(docs['after'])
        keys = sorted(set(left) | set(right))
        differing = [key for key in keys if left.get(key) != right.get(key)]
        rows.append({
            'request': request,
            'seed': seed,
            'work': work,
            'extra': extra,
            'fieldsCompared': len(keys),
            'differingFields': len(differing),
            'differing': differing[:40],
            'rawDepthMm': [
                docs[label].get('portfolio', {}).get('incumbent', {})
                .get('rawDepthMm') for label in ('base', 'after')],
            'workUnits': [docs[label].get('portfolio', {}).get('workUnits')
                          for label in ('base', 'after')],
        })
    document = {
        'pristineBinary': pristine,
        'afterBinary': after,
        'rows': rows,
        'differingTotal': sum(row['differingFields'] for row in rows),
    }
    os.makedirs(os.path.dirname(os.path.abspath(out_path)), exist_ok=True)
    json.dump(document, open(out_path, 'w'), indent=1)
    print(json.dumps(
        [{key: row[key] for key in
          ('seed', 'fieldsCompared', 'differingFields', 'rawDepthMm',
           'workUnits')} for row in rows], indent=1))


if __name__ == '__main__':
    main()
