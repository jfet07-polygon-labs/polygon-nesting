#!/usr/bin/env python3
"""Total the round's arms from the drivers' own documents."""
import json
import os

RUN = '/var/lib/t3/tmp/wf87/run'
sweeps = ['l10-flat-inc', 'l9-flat-inc-ab', 'l10-flat-4563', 'l10-flat-4633',
          'l10-flat-60914', 'm34fine-inc', 'm34fine-4563', 'm34fine-4633',
          'deepflat', 'knudge', 'seedtest', 'flat-m30', 'legal-m30',
          'legal-m31', 'legal-m27', 'knudge-legal', 'm30seed', 'legal-deeper',
          'legal-deeper31']
total = 0
for label in sweeps:
    path = f'{RUN}/{label}/sweep.json'
    if not os.path.exists(path):
        continue
    doc = json.load(open(path))
    n = doc.get('arms') or len(doc['rows'])
    total += n
    print(f'{label:18s} {n:4d} arms, {doc.get("armsBelow", sum(1 for r in doc["rows"] if r.get("below"))):3d} below')
cross = json.load(open(f'{RUN}/cross-final/cross.json'))
print(f'{"cross-final":18s} {cross["arms"]:4d} arms, {cross["armsBelow"]:3d} below')
total += cross['arms']
regrid = json.load(open(f'{RUN}/regrid-1554/regrid.json'))
print(f'{"regrid":18s} {len(regrid["rows"]):4d} arms, {sum(1 for r in regrid["rows"] if r["belowIncumbent"]):3d} below')
total += len(regrid['rows'])
cert = json.load(open(f'{RUN}/cert-final.json'))
print(f'{"cert-final":18s} {cert["probeArms"]:4d} arms, {cert["belowIncumbent"]:3d} below')
replays = 4 * 3          # incumbent, final on both binaries
gates = 4 * 2
print(f'{"replays+gates":18s} {replays + gates:4d} arms')
standalone = total + replays + gates
print(f'\nstandalone (excl. cert) {standalone}')
print(f'standalone + cert       {standalone + cert["probeArms"]}')
print(f'cascade                 2413')
print(f'ROUND TOTAL             {standalone + cert["probeArms"] + 2413}')
