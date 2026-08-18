#!/usr/bin/env python3
"""Confirms every publication an arm made, in a separate process each time.

    python3 confirmall.py GATEDIR ARM OUTDIR BINARY [OUT.json]

`0 of 12` is a result and is reported as one: a cell whose arm published
nothing gets a row saying so rather than being dropped.
"""
import json
import os
import subprocess
import sys


def main():
    gatedir, arm, outdir, binary = sys.argv[1:5]
    gate = json.load(open(f'{gatedir}/gate.json'))
    driver = os.path.join(os.path.dirname(os.path.abspath(__file__)),
                          'confirm.py')
    rows = []
    for cell in gate['cells']:
        seed = cell['seed']
        found = cell['arms'].get(arm) or {}
        parent = cell['parentRawDepthMm']
        depth = found.get('rawSourceDepthMm')
        if depth is None or depth >= parent:
            rows.append({'seed': seed, 'parentRawDepthMm': parent,
                         'published': False,
                         'rawSourceDepthMm': depth})
            continue
        source = f'{gatedir}/seed{seed}-{arm}.json'
        target = f'{outdir}/seed{seed}-{arm}'
        proc = subprocess.run(
            [sys.executable, driver, source, target, binary],
            capture_output=True, check=False)
        try:
            report = json.loads(proc.stdout.decode())
        except json.JSONDecodeError:
            rows.append({'seed': seed, 'error':
                         (proc.stderr or b'').decode()[-400:]})
            continue
        report['seed'] = seed
        report['parentRawDepthMm'] = parent
        report['published'] = True
        rows.append(report)
    ok = [r for r in rows if r.get('published')
          and (r.get('mode27') or {}).get('exactValid')
          and (r.get('mode27') or {}).get('contractValid')
          and r.get('fingerprintUnchanged') and r.get('depthAgrees')]
    out = {'gateDir': gatedir, 'arm': arm, 'binary': binary,
           'cells': len(rows),
           'published': sum(1 for r in rows if r.get('published')),
           'confirmed': len(ok),
           'rows': rows}
    print(json.dumps(out, indent=1))
    if len(sys.argv) > 5:
        json.dump(out, open(sys.argv[5], 'w'), indent=1)


if __name__ == '__main__':
    main()
