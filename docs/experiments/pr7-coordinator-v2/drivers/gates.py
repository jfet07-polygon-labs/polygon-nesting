#!/usr/bin/env python3
"""The four pinned regression gates, run against one binary.

Identical in every argument to the PR7 stage's gate driver; only `ROOT` moves,
because the gates are the contract and a stage that restates them differently
is not running the same gate. The comparison is the pinned value *and* the
pinned fingerprint, and `--twice` additionally re-runs every gate and compares
the two runs as whole documents, which catches a change that is deterministic
per process but not across them.

Usage: gates.py LABEL BINARY [--twice]
"""
import json
import os
import sys
import time
import subprocess

sys.path.insert(0, os.path.join(
    os.path.dirname(os.path.dirname(os.path.dirname(
        os.path.abspath(__file__)))),
    'pr7-portfolio-coordinator', 'drivers'))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import docdiff  # noqa: E402
import lib  # noqa: E402

TRUE = (f'{lib.ROOT}/docs/experiments/persistent-vacancy-descent/'
        'exact-contract/true-contract')
LADDER = f'{TRUE}/finer-ladder'

EXPECT = {
    'g1': (206.869,
           '8a7737381238fa4d4979cbd95a4f08500b6608039475243c0a24c45828f9e437'),
    'g2': (159.09233022733062,
           'fa01012af1d559ae09ce7295c146f0cdc6569cfad6f24b6154f0153c4393dbbc'),
    'g3': (159.07876040364795, 'e28fba007f8031d4'),
    'g4': (164.0375677990678, '49f094d7e59a9008'),
}
CASES = {
    'g1': (20, '/var/lib/t3/tmp/ex5-seed-native.json', '320.000', None),
    'g2': (22, f'{TRUE}/record-159.092/pinned-parent-159.092.json',
           '159.892624', '0.0005'),
    'g3': (22, f'{LADDER}/pinned-parent-159.079.json', '159.87876', '0.0005'),
    'g4': (22, f'{LADDER}/pinned-fs-parent-164.0376.json', '164.837568',
           '0.0005'),
}


def invoke(binary, tag, mode, parent, target, allowance, out_dir):
    argv = ([binary, lib.REQUESTS['mixed-61']]
            + [a.format(seed='5') for a in lib.ARGS]
            + [str(mode), parent, str(target)])
    if allowance:
        argv += ['', allowance]
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_QUALITY_TRACE', None)
    env.pop('POLYGON_NESTING_PROFILE', None)
    os.makedirs(out_dir, exist_ok=True)
    path = f'{out_dir}/{tag}.json'
    started = time.monotonic()
    with open(path, 'w') as handle:
        subprocess.run(argv, stdout=handle, stderr=subprocess.DEVNULL,
                       check=False, env=env)
    return json.load(open(path)), time.monotonic() - started


def collect(node, keys, path='', out=None):
    if out is None:
        out = {}
    if isinstance(node, dict):
        for key, value in node.items():
            if key in keys and not isinstance(value, (dict, list)):
                out.setdefault(path + '/' + key, value)
            collect(value, keys, path + '/' + key, out)
    elif isinstance(node, list):
        for index, value in enumerate(node):
            collect(value, keys, path + f'/{index}', out)
    return out


def verdict(tag, doc):
    if tag == 'g1':
        found = collect(doc, {'independentDepthMm', 'finalPlacementFingerprint',
                              'placementFingerprint'})
        depths = sorted({v for k, v in found.items()
                         if k.endswith('independentDepthMm') and v is not None})
        fingerprints = sorted({v for k, v in found.items()
                               if 'ingerprint' in k and v is not None})
        return {'depths': depths, 'fingerprints': fingerprints,
                'hit': EXPECT['g1'][0] in depths
                and EXPECT['g1'][1] in fingerprints}
    pop = doc['relaxedDiagnostics']['coupledDynamicSeparator'][
        'persistentVacancyPopulation']
    raw = pop.get('rawSourceDepthMm')
    fingerprint = pop.get('finalPlacementFingerprint') or ''
    return {'raw': raw, 'fingerprint': fingerprint,
            'exactValid': pop.get('exactValid'),
            'contractValid': pop.get('contractValid'),
            'hit': raw == EXPECT[tag][0]
            and fingerprint.startswith(EXPECT[tag][1])}


def main():
    label, binary = sys.argv[1], sys.argv[2]
    twice = '--twice' in sys.argv
    out_dir = f'{lib.OUT}/gates/{label}'
    result = {'label': label, 'binary': binary, 'twice': twice}
    for tag, (mode, parent, target, allowance) in CASES.items():
        doc, seconds = invoke(binary, tag, mode, parent, target, allowance,
                              out_dir)
        row = verdict(tag, doc)
        row['seconds'] = seconds
        if twice:
            again, _ = invoke(binary, f'{tag}-b', mode, parent, target,
                              allowance, out_dir)
            left = dict(docdiff.paths(docdiff.strip(doc)))
            right = dict(docdiff.paths(docdiff.strip(again)))
            differing = sorted(
                key for key in set(left) | set(right)
                if left.get(key, '<absent>') != right.get(key, '<absent>'))
            row['wholeDocumentFields'] = len(left)
            row['crossRunDifferences'] = len(differing)
            row['crossRunFirst'] = differing[:8]
        result[tag] = row
    result['ALL_PASS'] = all(result[tag]['hit'] for tag in CASES)
    if twice:
        result['ALL_STABLE'] = all(result[tag]['crossRunDifferences'] == 0
                                   for tag in CASES)
    print(json.dumps(result, indent=1))
    os.makedirs(out_dir, exist_ok=True)
    json.dump(result, open(f'{out_dir}/gates-{label}.json', 'w'), indent=1)


if __name__ == '__main__':
    main()
