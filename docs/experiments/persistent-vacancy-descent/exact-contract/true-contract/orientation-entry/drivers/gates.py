#!/usr/bin/env python3
"""The two regression gates, run against BENCH_BIN (defaults to lib.BIN)."""
import sys, json, subprocess, os, time
sys.path.insert(0, '/var/lib/t3/tmp/orient')
import lib

BIN = os.environ.get('BENCH_BIN', lib.BIN)
TRUE = lib.TRUE
OUT = '/var/lib/t3/tmp/orient/gates'
os.makedirs(OUT, exist_ok=True)


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


def run(tag, mode, parent, target, allowance):
    argv = ([BIN, lib.REQ] + [a.format(clamp='0', seed='5') for a in lib.ARGS]
            + [str(mode), parent, str(target)])
    if allowance:
        argv += ['', allowance]
    path = f'{OUT}/{tag}.json'
    t0 = time.time()
    with open(path, 'w') as handle:
        subprocess.run(argv, stdout=handle, stderr=subprocess.DEVNULL, check=False)
    return json.load(open(path)), time.time() - t0


def gate1(tag):
    doc, dt = run(tag + '-g1', 20, '/var/lib/t3/tmp/ex5-seed-native.json', '320.000', None)
    found = collect(doc, {'independentDepthMm', 'finalPlacementFingerprint',
                          'placementFingerprint'})
    return dt, found


def gate2(tag):
    parent = (f'{TRUE}/record-159.092/pinned-parent-159.092.json')
    doc, dt = run(tag + '-g2', 22, parent, '159.892624', '0.0005')
    pop = doc['relaxedDiagnostics']['coupledDynamicSeparator']['persistentVacancyPopulation']
    return dt, {
        'rawSourceDepthMm': pop.get('rawSourceDepthMm'),
        'finalPlacementFingerprint': pop.get('finalPlacementFingerprint'),
        'exactValid': pop.get('exactValid'),
        'contractValid': pop.get('contractValid'),
    }


if __name__ == '__main__':
    tag = sys.argv[1] if len(sys.argv) > 1 else 'x'
    which = sys.argv[2] if len(sys.argv) > 2 else 'both'
    if which in ('both', '1'):
        dt, found = gate1(tag)
        print(f'GATE1 {dt:.1f}s')
        for key, value in sorted(found.items()):
            print('   ', key, '=', value)
    if which in ('both', '2'):
        dt, found = gate2(tag)
        print(f'GATE2 {dt:.1f}s', json.dumps(found))
