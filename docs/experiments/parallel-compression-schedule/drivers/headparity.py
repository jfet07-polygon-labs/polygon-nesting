#!/usr/bin/env python3
"""Is the armed build, with an unarmed spec, still HEAD on the coordinator path?

    python3 headparity.py OUTDIR HEAD_BINARY ARMED_BINARY [WORK_UNITS]

The four pinned gates prove this for modes 20 and 22, which never reach mode 34.
This proves it for the path that does: the v3 coordinator from the bare request,
where mode 34 is a scheduled class and where this round added two settings
fields and one settings construction.

The budget is **work**, not wall, for the obvious reason - a wall-budgeted
coordinator run is not reproducible across processes even on one binary, so a
wall comparison could not tell "the armed build differs" from "the box was
busy". Under a work cap the run is deterministic and load-independent, so a
difference is a difference.
"""
import hashlib
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402
import runlib  # noqa: E402


def run(binary, seed, spec, path):
    args = [a.format(seed=seed) for a in runlib.ARGS]
    tail = ['0', '', '', '', runlib.DEFAULT_ALLOWANCE, spec]
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_PROFILE', None)
    env.pop('POLYGON_NESTING_COMPRESSION_SCHEDULE', None)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, 'w') as handle:
        subprocess.run([binary, runlib.REQUESTS['mixed-61']] + args + tail,
                       stdout=handle, stderr=subprocess.DEVNULL, check=False,
                       env=env)
    return json.load(open(path))


def main():
    outdir, head, armed = sys.argv[1], sys.argv[2], sys.argv[3]
    work = int(sys.argv[4]) if len(sys.argv) > 4 else 40_000_000
    result = {
        'headBinary': head,
        'headSha256': hashlib.sha256(open(head, 'rb').read()).hexdigest(),
        'armedBinary': armed,
        'armedSha256': hashlib.sha256(open(armed, 'rb').read()).hexdigest(),
        'workUnits': work, 'cells': [],
    }
    for seed in (0, 1, 2):
        spec = f'work={work},cells={runlib.SALT_SETS[seed % 3]},v3=1'
        docs = {}
        for label, binary in (('head', head), ('armedUnarmedSpec', armed)):
            docs[label] = run(binary, seed, spec,
                              f'{outdir}/{label}-s{seed}.json')
        digests = {k: lib.doc_digest(v) for k, v in docs.items()}
        depths = {k: (v.get('portfolio') or {}).get('incumbent', {}).get(
            'rawDepthMm') for k, v in docs.items()}
        result['cells'].append({
            'seed': seed, 'spec': spec,
            'docDigests': digests,
            'docDigestsMatch': len(set(digests.values())) == 1,
            'rawDepthMm': depths,
            'depthsMatch': len(set(depths.values())) == 1,
            'workUnits': {k: (v.get('portfolio') or {}).get('workUnits')
                          for k, v in docs.items()},
        })
        json.dump(result, open(f'{outdir}/headparity.json', 'w'), indent=1)
    result['ALL_MATCH'] = all(c['docDigestsMatch'] for c in result['cells'])
    json.dump(result, open(f'{outdir}/headparity.json', 'w'), indent=1)
    print(json.dumps({'ALL_MATCH': result['ALL_MATCH'],
                      'cells': [{k: c[k] for k in
                                 ('seed', 'docDigestsMatch', 'depthsMatch',
                                  'rawDepthMm')}
                                for c in result['cells']]}, indent=1))


if __name__ == '__main__':
    main()
