#!/usr/bin/env python3
"""Copy the round's whole lineage of pins into the true-contract tree.

    python3 collectpins.py DEST

Named by declared raw so a reader can order the line by filename, and emitted
with an index that carries each pin's sha256 and placement fingerprint - the two
identities every table in the README quotes.
"""
import hashlib
import json
import os
import shutil
import sys

DEST = sys.argv[1]
os.makedirs(DEST, exist_ok=True)
SOURCES = ['/var/lib/t3/tmp/wf87/pins']
SOURCES += [f'/var/lib/t3/tmp/wf87/run/{c}/pins' for c in
            ('c2a', 'c2b', 'c2c', 'c2d', 'c2e', 'c2f')]

candidates = []
for directory in SOURCES:
    if os.path.isdir(directory):
        candidates += [f'{directory}/{n}' for n in sorted(os.listdir(directory))]

by_fingerprint = {}
for path in candidates:
    doc = json.load(open(path))
    by_fingerprint.setdefault(doc['expectedPlacementFingerprint'], path)

index = []
for fingerprint, path in by_fingerprint.items():
    # The filename's number is the declared raw, taken from the run that
    # produced it; the fixture itself carries the rounded independentDepthMm,
    # so the raw is recovered from the source filename where the driver put it.
    stem = os.path.basename(path).replace('.json', '')
    raw = stem.split('-')[-1]
    name = f'pinned-fs-{raw}.json'
    shutil.copy(path, f'{DEST}/{name}')
    index.append({'pin': name,
                  'sha256': hashlib.sha256(open(path, 'rb').read()).hexdigest(),
                  'fingerprint': fingerprint,
                  'independentDepthMm': json.load(open(path))['independentDepthMm'],
                  'source': path})
index.sort(key=lambda row: row['pin'], reverse=True)
json.dump(index, open(f'{DEST}/index.json', 'w'), indent=1)
print(json.dumps(index, indent=1))
