#!/usr/bin/env python3
"""The three regression gates from the task brief, run against BENCH_BIN."""
import sys, json, subprocess, os, time
sys.path.insert(0, '/var/lib/t3/tmp/orient-fine')
import lib

BIN = os.environ.get('BENCH_BIN', lib.BIN)
OUT = '/var/lib/t3/tmp/orient-fine/gates'
os.makedirs(OUT, exist_ok=True)
TRUE = lib.TRUE

EXPECT = {
    'g1': (206.869,
           '8a7737381238fa4d4979cbd95a4f08500b6608039475243c0a24c45828f9e437'),
    'g2': (159.09233022733062,
           'fa01012af1d559ae09ce7295c146f0cdc6569cfad6f24b6154f0153c4393dbbc'),
    'g3': (159.08263749731248,
           '145d0ed4b2f53d3fa8f524af0da6875d351e4e82abd0c6252783cd6cd6032666'),
}


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


label = sys.argv[1]
res = {'binary': BIN}
doc, dt = run(f'{label}-g1', 20, '/var/lib/t3/tmp/ex5-seed-native.json',
              '320.000', None)
found = collect(doc, {'independentDepthMm', 'finalPlacementFingerprint',
                      'placementFingerprint'})
depths = sorted({v for k, v in found.items()
                 if k.endswith('independentDepthMm') and v is not None})
fps = sorted({v for k, v in found.items() if 'ingerprint' in k and v is not None})
res['g1'] = {'dt': dt, 'depths': depths, 'fingerprints': fps,
             'hit': EXPECT['g1'][0] in depths and EXPECT['g1'][1] in fps}

for tag, parent, target in (
        ('g2', f'{TRUE}/record-159.092/pinned-parent-159.092.json', '159.892624'),
        ('g3', f'{TRUE}/orientation-entry/pinned-parent-159.083.json', '159.882637')):
    doc, dt = run(f'{label}-{tag}', 22, parent, target, '0.0005')
    pop = doc['relaxedDiagnostics']['coupledDynamicSeparator'][
        'persistentVacancyPopulation']
    raw, fp = pop.get('rawSourceDepthMm'), pop.get('finalPlacementFingerprint')
    res[tag] = {'dt': dt, 'raw': raw, 'fp': fp,
                'exactValid': pop.get('exactValid'),
                'contractValid': pop.get('contractValid'),
                'hit': raw == EXPECT[tag][0] and fp == EXPECT[tag][1]}
res['ALL_PASS'] = all(res[k]['hit'] for k in ('g1', 'g2', 'g3'))
print(json.dumps(res, indent=1))
json.dump(res, open(f'{OUT}/gates3-{label}.json', 'w'), indent=1)
