#!/usr/bin/env python3
"""Copy this round's emitted documents out of the scratch tree, unedited.

    python3 collect2.py DEST

Every driver writes one JSON document of its own; nothing here is rewritten,
summarised or filtered, so the evidence directory is what the drivers produced.
"""
import json
import os
import shutil
import sys

DEST = sys.argv[1]
SRC = '/var/lib/t3/tmp/wf87'
os.makedirs(DEST, exist_ok=True)

# (source path, destination name). Sweep documents live under run/<label>/.
ITEMS = [
    (f'{SRC}/gates/gates-base.json', 'gates-base.json'),
    (f'{SRC}/gates/gates-l10.json', 'gates-l10.json'),
    (f'{SRC}/run/replay-incumbent.json', 'replay-incumbent-155.4223.json'),
    (f'{SRC}/run/l10-flat-inc/sweep.json', 'ladder-ab-new-155.4223.json'),
    (f'{SRC}/run/l9-flat-inc-ab/sweep.json', 'ladder-ab-base-155.4223.json'),
    (f'{SRC}/run/l10-flat-4563/sweep.json', 'basin-155.4563.json'),
    (f'{SRC}/run/l10-flat-4633/sweep.json', 'basin-155.4633.json'),
    (f'{SRC}/run/l10-flat-60914/sweep.json', 'basin-156.0914.json'),
    (f'{SRC}/run/m34fine-inc/sweep.json', 'm34fine-155.42197.json'),
    (f'{SRC}/run/m34fine-4563/sweep.json', 'm34fine-155.4563.json'),
    (f'{SRC}/run/m34fine-4633/sweep.json', 'm34fine-155.4633.json'),
    (f'{SRC}/run/deepflat/sweep.json', 'deepflat-155.41964.json'),
    (f'{SRC}/run/knudge/sweep.json', 'knudge-155.41964.json'),
    (f'{SRC}/run/seedtest/sweep.json', 'seedtest-m33-155.40873.json'),
    (f'{SRC}/run/flat-m30/sweep.json', 'legalentry-first-155.40873.json'),
    (f'{SRC}/run/legal-m30/sweep.json', 'legalentry-m30-155.40873.json'),
    (f'{SRC}/run/legal-m31/sweep.json', 'legalentry-m31-155.40873.json'),
    (f'{SRC}/run/legal-m27/sweep.json', 'legalentry-m27-155.40873.json'),
    (f'{SRC}/run/regrid-1554/regrid.json', 'regrid-155.40873.json'),
    (f'{SRC}/c2a-state.json', 'cascade-c2a-state.json'),
    (f'{SRC}/c2a-cascade.log', 'cascade-c2a.log'),
    (f'{SRC}/c2b-state.json', 'cascade-c2b-state.json'),
    (f'{SRC}/c2b-cascade.log', 'cascade-c2b.log'),
    (f'{SRC}/run/c2c/state.json', 'cascade-c2c-state.json'),
    (f'{SRC}/run/c2c/cascade.log', 'cascade-c2c.log'),
    (f'{SRC}/run/c2d/state.json', 'cascade-c2d-state.json'),
    (f'{SRC}/run/c2d/cascade.log', 'cascade-c2d.log'),
    (f'{SRC}/run/c2e/state.json', 'cascade-c2e-state.json'),
    (f'{SRC}/run/c2e/cascade.log', 'cascade-c2e.log'),
    (f'{SRC}/run/docdiff-base-l10.json', 'docdiff-base-l10.json'),
    (f'{SRC}/run/cert-final.json', 'cert-final.json'),
    (f'{SRC}/run/replay-final.json', 'replay-final.json'),
    (f'{SRC}/run/replay-final-base.json', 'replay-final-basebinary.json'),
    (f'{SRC}/run/geodiff-record.json', 'geodiff-155.4223-to-first.json'),
    (f'{SRC}/run/crossover-final.json', 'crossover-final.json'),
    (f'{SRC}/run/gates-final.json', 'gates-final.json'),
]

copied, missing = [], []
for source, name in ITEMS:
    if os.path.exists(source):
        shutil.copy(source, f'{DEST}/{name}')
        copied.append(name)
    else:
        missing.append(name)
print(json.dumps({'dest': DEST, 'copied': copied, 'missing': missing}, indent=1))
