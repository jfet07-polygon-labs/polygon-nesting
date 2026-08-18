#!/usr/bin/env python3
"""Copy the round's evidence out of the scratch tree into the experiment dir.

    python3 collect.py

Every file named here is a driver's own emitted document, unedited. The suite
logs are truncated to their `test result` lines plus any failure block, because
the full logs are 120 kB of compiler warnings.
"""
import os
import re
import shutil

SCRATCH = '/var/lib/t3/tmp/recordline'
HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EVIDENCE = f'{HERE}/evidence'
os.makedirs(EVIDENCE, exist_ok=True)

COPY = [
    ('gates-base.json', 'gates-base.json'),
    ('gates-after.json', 'gates-after.json'),
    ('gates-sched.json', 'gates-sched.json'),
    ('gates-final.json', 'gates-final.json'),
    ('gates-final-sched.json', 'gates-final-sched.json'),
    ('docdiff-base-after.json', 'docdiff-base-after.json'),
    ('docdiff-base-sched.json', 'docdiff-base-sched.json'),
    ('fs-stepsweep/sweep.json', 'stepsweep-from-scratch-159.668.json'),
    ('rec-stepsweep/sweep.json', 'stepsweep-record-parent-159.079.json'),
    ('fs-bigbudget/sweep.json', 'stepsweep-bigbudget-156.919.json'),
    ('regrid-156.9188/regrid.json', 'regrid-156.919.json'),
    ('replay-fs159.668.json', 'replay-159.668.json'),
    ('replay-158.668.json', 'replay-158.668.json'),
    ('fsline/state.json', 'cascade-fsline-state.json'),
    ('fsline/cascade.log', 'cascade-fsline.log'),
    ('fsline2/state.json', 'cascade-fsline2-state.json'),
    ('fsline2/cascade.log', 'cascade-fsline2.log'),
    ('fsline3/state.json', 'cascade-fsline3-state.json'),
    ('fsline3/cascade.log', 'cascade-fsline3.log'),
    ('fs-seedsweep/sweep.json', 'stepsweep-seeds-156.105.json'),
    ('cert-156.0914.json', 'cert-156.0914.json'),
    ('cert-final.json', 'cert-final.json'),
    ('replay-156.0914.json', 'replay-156.0914.json'),
    ('replay-final.json', 'replay-final.json'),
]


def counts(path):
    passed = failed = ignored = 0
    for line in open(path, errors='replace'):
        match = re.search(r'(\d+) passed; (\d+) failed; (\d+) ignored', line)
        if match:
            passed += int(match.group(1))
            failed += int(match.group(2))
            ignored += int(match.group(3))
    return passed, failed, ignored


for source, target in COPY:
    path = f'{SCRATCH}/{source}'
    if os.path.exists(path):
        shutil.copyfile(path, f'{EVIDENCE}/{target}')
        print(f'copied {source} -> evidence/{target}')
    else:
        print(f'MISSING {source}')

for name in ('suite.log', 'suite-armed.log'):
    path = f'{SCRATCH}/{name}'
    if not os.path.exists(path):
        print(f'MISSING {name}')
        continue
    passed, failed, ignored = counts(path)
    with open(f'{EVIDENCE}/{name}', 'w') as handle:
        handle.write(f'{passed} passed; {failed} failed; {ignored} ignored\n\n')
        for line in open(path, errors='replace'):
            if line.startswith('test result') or 'FAILED' in line \
                    or 'panicked' in line or line.startswith('failures:'):
                handle.write(line)
    print(f'summarised {name}: {passed} passed, {failed} failed, '
          f'{ignored} ignored')
