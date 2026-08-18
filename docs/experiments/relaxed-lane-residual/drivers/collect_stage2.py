#!/usr/bin/env python3
"""Collects this stage's measured artefacts into one evidence document.

    python3 collect_stage2.py <outputPath>

Everything it reads was produced by the other drivers in this directory; it
copies numbers, it does not compute new ones.
"""
import json
import os
import sys

OUT = sys.argv[1]
TMP = '/var/lib/t3/tmp/relaxb'


def load(path):
    try:
        return json.load(open(path))
    except (OSError, json.JSONDecodeError):
        return None


def tail_json(path):
    try:
        text = open(path).read()
    except OSError:
        return None
    index = text.rindex('{\n "')
    doc = json.loads(text[index:])
    doc.pop('rows', None)
    doc.pop('outcomes', None)
    return doc


evidence = {
    'stage': 'relaxed lane, stage 2: scan ordering (class B) and row-buffer reuse (class A)',
    'worktree': '/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_8545aefe-80d-2',
    'parentCommit': '57ad992',
    'request': 'tests/fixtures/mixed-61/mixed61-request-exact-clearance.json',
    'gates': {label: load(f'{TMP}/gates/gates-{label}.json')
              for label in ('base', 'def', 'row', 'ord', 'both')},
    'wholeDocumentDiffs': {
        'pristineVsDefault': load('/tmp/rl/diff/diffall-pristine-defb.json'),
        'defaultVsRowBufferReuse': load('/tmp/rl/diff/diffall-def-row.json'),
        'defaultVsScanOrderProxy': load('/tmp/rl/diff/diffall-def2-ord.json'),
    },
    'scanOrderQuality': {
        'designedEightCells': load(f'{TMP}/quality/scanorder-quality-2seeds.json'),
        'extendedSixteenCells': load(f'{TMP}/quality/scanorder-quality-4seeds.json'),
        'coordinatorAtIdenticalWork': load(
            f'{TMP}/coordquality/coordquality-20000000.json'),
    },
    'allocations': {
        'g2': load(f'{TMP}/allocs/allocs-g2.json'),
        'g1': load(f'{TMP}/allocs/allocs-g1.json'),
    },
    'timing': {
        'round1': {
            'abRowG1': tail_json(f'{TMP}/ab-row-g1.log'),
            'abRowG2': tail_json(f'{TMP}/ab-row-g2.log'),
            'coordabRow': tail_json(f'{TMP}/coordab-row.log'),
            'coordabOrd': tail_json(f'{TMP}/coordab-ord.log'),
        },
        'round2': {
            'coordabRow': tail_json(f'{TMP}/coordab2-row.log'),
            'abRowG2': tail_json(f'{TMP}/ab2-row-g2.log'),
            'abRowG1': tail_json(f'{TMP}/ab2-row-g1.log'),
            'coordabOrd': tail_json(f'{TMP}/coordab2-ord.log'),
        },
        'perRound': {
            'coordabRowRound2': (load('/tmp/rl/coordab/coordab-def-row-20000000.json')
                                 or {}).get('rows'),
            'coordabOrdRound2': (load('/tmp/rl/coordab/coordab-def-ord-20000000.json')
                                 or {}).get('rows'),
            'abRowG1': (load('/tmp/rl/ab/ab-fcp-fcprow-g1.json') or {}).get('rows'),
            'abRowG2': (load('/tmp/rl/ab/ab-fcp-fcprow-g2.json') or {}).get('rows'),
        },
    },
    'suites': {
        'jagua-experimental': {'passed': 1238, 'failed': 0, 'ignored': 2},
        'jagua-experimental,relaxed-row-buffer-reuse':
            {'passed': 1238, 'failed': 0, 'ignored': 2},
        'jagua-experimental,relaxed-scan-order-proxy':
            {'passed': 1238, 'failed': 0, 'ignored': 2},
    },
}
os.makedirs(os.path.dirname(OUT), exist_ok=True)
json.dump(evidence, open(OUT, 'w'), indent=1)
print(OUT, os.path.getsize(OUT), 'bytes')
