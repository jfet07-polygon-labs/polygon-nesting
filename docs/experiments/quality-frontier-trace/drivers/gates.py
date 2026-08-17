#!/usr/bin/env python3
"""The four pinned regression gates, run against one binary.

Usage: gates.py LABEL BINARY [--trace SINK_DIR]

`--trace` arms the quality-frontier sink on every gate, which is the harder
half of the gate: it proves that recording the stream does not change the
stream. Without it, a `quality-trace` build is checked with the sink closed.

Gate 3 is quoted at raw 159.07876040364795 per the finer-ladder measurement
policy: the DECLARED record is 159.07876040364792 and a replay legitimately
reports one ULP above it, so the gate compares the value this parent has
always replayed to, exactly.
"""
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

EXPECT = {
    'g1': (206.869,
           '8a7737381238fa4d4979cbd95a4f08500b6608039475243c0a24c45828f9e437'),
    'g2': (159.09233022733062,
           'fa01012af1d559ae09ce7295c146f0cdc6569cfad6f24b6154f0153c4393dbbc'),
    'g3': (159.07876040364795,
           'e28fba007f8031d4'),
    'g4': (164.0375677990678,
           '49f094d7e59a9008'),
}


def invoke(binary, tag, mode, parent, target, allowance, out_dir, sink_dir):
    argv = ([binary, lib.REQ] + [a.format(clamp='0', seed='5') for a in lib.ARGS]
            + [str(mode), parent, str(target)])
    if allowance:
        argv += ['', allowance]
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_QUALITY_TRACE', None)
    env.pop('POLYGON_NESTING_PROFILE', None)
    if sink_dir:
        os.makedirs(sink_dir, exist_ok=True)
        env['POLYGON_NESTING_QUALITY_TRACE'] = f'{sink_dir}/{tag}.jsonl'
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


def main():
    label, binary = sys.argv[1], sys.argv[2]
    sink_dir = None
    if len(sys.argv) > 4 and sys.argv[3] == '--trace':
        sink_dir = sys.argv[4]
    out_dir = f'/var/lib/t3/tmp/qft/gates/{label}'
    os.makedirs(out_dir, exist_ok=True)
    result = {'label': label, 'binary': binary, 'sink': sink_dir}

    doc, seconds = invoke(binary, f'{label}-g1', 20,
                          '/var/lib/t3/tmp/ex5-seed-native.json', '320.000',
                          None, out_dir, sink_dir)
    found = collect(doc, {'independentDepthMm', 'finalPlacementFingerprint',
                          'placementFingerprint'})
    depths = sorted({v for k, v in found.items()
                     if k.endswith('independentDepthMm') and v is not None})
    fingerprints = sorted({v for k, v in found.items()
                           if 'ingerprint' in k and v is not None})
    result['g1'] = {
        'seconds': seconds, 'depths': depths, 'fingerprints': fingerprints,
        'hit': EXPECT['g1'][0] in depths and EXPECT['g1'][1] in fingerprints,
    }

    for tag, parent, target in (
            ('g2', f'{lib.TRUE}/record-159.092/pinned-parent-159.092.json',
             '159.892624'),
            ('g3', f'{lib.LADDER}/pinned-parent-159.079.json', '159.87876'),
            ('g4', f'{lib.LADDER}/pinned-fs-parent-164.0376.json',
             '164.837568')):
        doc, seconds = invoke(binary, f'{label}-{tag}', 22, parent, target,
                              '0.0005', out_dir, sink_dir)
        pop = doc['relaxedDiagnostics']['coupledDynamicSeparator'][
            'persistentVacancyPopulation']
        raw = pop.get('rawSourceDepthMm')
        fingerprint = pop.get('finalPlacementFingerprint') or ''
        result[tag] = {
            'seconds': seconds, 'raw': raw, 'fingerprint': fingerprint,
            'exactValid': pop.get('exactValid'),
            'contractValid': pop.get('contractValid'),
            'hit': (raw == EXPECT[tag][0]
                    and fingerprint.startswith(EXPECT[tag][1])),
        }
    result['ALL_PASS'] = all(result[key]['hit']
                             for key in ('g1', 'g2', 'g3', 'g4'))
    print(json.dumps(result, indent=1))
    json.dump(result, open(f'{out_dir}/gates-{label}.json', 'w'), indent=1)


if __name__ == '__main__':
    main()
