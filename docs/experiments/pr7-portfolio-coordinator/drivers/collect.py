#!/usr/bin/env python3
"""Copies the measurement artifacts out of the scratch tree into the repo.

The raw event streams stay in scratch: they are 2.9 MiB of JSONL per sweep and
`curve.json` already carries the incumbent series each one produced, joined to
raw depth. Everything a reader needs to check a quoted number is copied.
"""
import json
import os
import shutil

SRC = '/var/lib/t3/tmp/pr7'
DST = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                   'evidence')

COPIES = [
    ('curve/summary.json', 'summary.json'),
    ('curve/curve.json', 'curve.json'),
    ('curve-v1/curve.json', 'curve-schedule-v1-fairness-first.json'),
    ('deepen-probe/curve.json', 'curve-iterated-deepening-probe.json'),
    ('determinism/determinism-full.json', 'determinism-full-budget.json'),
    ('determinism/determinism-binding.json', 'determinism-binding-budget.json'),
    ('overhead/overhead-sample1.json', 'overhead-sample1.json'),
    ('overhead/overhead.json', 'overhead-sample2.json'),
    ('gates/pristine/gates-pristine.json', 'gates-pristine.json'),
    ('gates/final/gates-final.json', 'gates-worktree.json'),
]


def main():
    os.makedirs(DST, exist_ok=True)
    copied = []
    for source, target in COPIES:
        path = os.path.join(SRC, source)
        if not os.path.exists(path):
            print(f'missing: {path}')
            continue
        shutil.copy(path, os.path.join(DST, target))
        copied.append(target)
    print(json.dumps({'copied': copied}, indent=1))


if __name__ == '__main__':
    main()
