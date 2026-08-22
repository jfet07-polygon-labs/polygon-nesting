#!/usr/bin/env python3
"""Determinism across two processes: the hard gate for anything measured.

    determinism.py OUTDIR BINARY PARENTSJSON WORKUNITS [SEEDS] [ALLOWANCE]

A diffable copy of `docs/experiments/contact-block/drivers/determinism.py`,
changed in exactly one way: the two things it runs twice are this round's two
arms - the mode-26 single rung and the mode-34 control slice at the same budget
- instead of the contact-block operator.

Two separate processes, same binary, same parent, same target, same spec. The
whole benchmark document must be identical once the wall-clock fields are
stripped: the published depth, the placement fingerprint, the work counters,
the ladder's per-rung and per-arm rows, and the moved placements themselves.

The placements are the part that matters most and the part a weaker check would
miss: a run that produced the same *depth* from a different layout would pass a
scalar comparison and still be non-deterministic. `gatelib.strip_times` removes
every wall-clock reading and every statistic derived from one; everything else
is compared.

Both arms carry `POLYGON_NESTING_PROFILE=1`, because the audition's x-axis is a
counter and the counters have to come from the same processes the digests do.
That adds a `searchProfile` block whose `phases` rows carry three fields that
are wall-clock readings under names `gatelib.strip_times` does not recognise -
`milliseconds`, `leafMilliseconds` and `leafSharePercent`, the last being a
ratio of two wall clocks. They are stripped here, by name, and nothing else is:
`calls`, `phase`, `enclosing` and the whole `counters` block stay in the digest,
which is what makes this a determinism check on the work meter and not only on
the answer.
"""
import hashlib
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import audition  # noqa: E402
import gatelib as lib  # noqa: E402
import runlib  # noqa: E402

# Wall-clock readings the shared `strip_times` name test misses, because they
# end in neither `Ms` nor `Seconds`.
PROFILE_TIME_KEYS = {'milliseconds', 'leafMilliseconds', 'leafSharePercent'}


def strip(node):
    if isinstance(node, dict):
        return {key: strip(value) for key, value in node.items()
                if key not in PROFILE_TIME_KEYS}
    if isinstance(node, list):
        return [strip(value) for value in node]
    return node


def digest(doc):
    return lib.doc_digest(strip(doc))


def main():
    outdir, binary, parents_json, work = sys.argv[1:5]
    work = int(work)
    seeds = ([int(s) for s in sys.argv[5].split(',')]
             if len(sys.argv) > 5 else [0, 1, 9])
    allowance = sys.argv[6] if len(sys.argv) > 6 else runlib.DEFAULT_ALLOWANCE
    parents = [p for p in json.load(open(parents_json))['rows']
               if p['seed'] in seeds]
    os.makedirs(outdir, exist_ok=True)
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'parents': parents_json,
        'seeds': seeds,
        'workUnits': work,
        'allowance': allowance,
        'controlSpec': audition.SPEC.format(work=work),
        'cells': [],
    }
    for parent in parents:
        seed = parent['seed']
        rung_target = audition.single_rung_target(parent['independentDepthMm'])
        drop1_target = f"{parent['rawDepthMm'] - audition.DROP_MM:.17g}"
        cell = {'seed': seed, 'arms': {}}
        plan = [
            ('m26:1rung', 26, rung_target, None),
            (f'm34:{work}', 34, drop1_target,
             audition.SPEC.format(work=work)),
        ]
        for label, mode, target, spec in plan:
            digests, depths, prints, works = [], [], [], []
            for run in (0, 1):
                tag = label.replace(':', '-')
                doc, _, err, code = audition.run_mode(
                    binary, seed, parent['fixture'], mode, target,
                    f'{outdir}/seed{seed}-{tag}-run{run}.json', allowance,
                    schedule_spec=spec)
                if doc is None:
                    digests.append(f'ERROR:{code}:{err[-200:]}')
                    depths.append(None)
                    prints.append(None)
                    works.append(None)
                    continue
                pop = audition.population(doc) or {}
                digests.append(digest(doc))
                depths.append(pop.get('rawSourceDepthMm'))
                prints.append(pop.get('finalPlacementFingerprint'))
                works.append(audition.profile_row(doc, 0.0)['processWorkUnits'])
            cell['arms'][label] = {
                'docDigests': digests,
                'rawSourceDepthMm': depths,
                'fingerprints': prints,
                'processWorkUnits': works,
                'identical': (len(set(digests)) == 1
                              and not any(isinstance(d, str)
                                          and d.startswith('ERROR')
                                          for d in digests)),
            }
            row = cell['arms'][label]
            print(f"seed{seed} {label}: identical={row['identical']} "
                  f"{digests[0][:16]} {digests[1][:16]} "
                  f"depths={depths} work={works}", flush=True)
        result['cells'].append(cell)
        json.dump(result, open(f'{outdir}/determinism.json', 'w'), indent=1)
    rows = [row for cell in result['cells'] for row in cell['arms'].values()]
    result['summary'] = {
        'cells': len(result['cells']),
        'armRuns': len(rows),
        'identical': sum(1 for r in rows if r['identical']),
        'ALL_IDENTICAL': all(r['identical'] for r in rows),
    }
    json.dump(result, open(f'{outdir}/determinism.json', 'w'), indent=1)
    print(json.dumps(result['summary'], indent=1))


if __name__ == '__main__':
    main()
