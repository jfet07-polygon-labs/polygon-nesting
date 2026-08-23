#!/usr/bin/env python3
"""Shared invocation for the overlap-ICS cells.

One process per cell, one JSON document per process, every exit status read
directly on the line after the command and never through a pipe.

`ROOT` is the repository containing **this file**, not a hard-coded worktree.
Sol review 17 Round 2 §2 named the hard-coded default a round-validity defect
rather than a knob - "otherwise the strongest tripwires can validate the wrong
tree" - and `fast.sh` was repaired for its own path in the spec commit. This
module was not, and it is the one that resolves `BIN` and every request path, so
the corpus and smoke stages were still reading whichever worktree the constant
happened to name. That worktree still exists on this box, so the failure mode
was silent rather than loud. `ICS_ROOT` still overrides explicitly.

`BIN` is the release example built from the committed tree; override it to point
one driver at two binaries for the two-process and two-binary determinism
comparisons.
"""
import hashlib
import json
import os
import subprocess
import time

ROOT = os.environ.get(
    'ICS_ROOT',
    os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                 '..', '..', '..', '..')))
BIN = os.environ.get('ICS_BIN', f'{ROOT}/target/release/examples/overlap_ics_benchmark')
OUT = os.environ.get('ICS_OUT', '/var/lib/t3/tmp/overlapics')

REQUESTS = {
    'mixed-61': f'{ROOT}/tests/fixtures/mixed-61/'
                'mixed61-request-exact-clearance.json',
    'shapes-17': f'{ROOT}/tests/fixtures/shapes-17/2000x2700-compact/'
                 'request.json',
    'triangle-20': f'{ROOT}/tests/fixtures/triangle-20/2000x2700-compact/'
                   'request.json',
}

# The Chinese wall's one permitted reader: the S0/S1/S2 correctness pin. It is
# never a seed and never a parameter source, and no constant in
# `search::overlap_ics` was chosen by looking at it.
SPARROW_POSES = (f'{ROOT}/docs/experiments/gate-a-sparrow-import/fixture/'
                 'sparrow-10s-x86-poses.json')

# The exact-clearance contract this whole campaign is measured under.
EDGE_MM = '5'
PAIR_MM = '5'

# The wall fields the two-process bit comparison strips. Everything else in the
# document must be byte-identical, which is why the driver puts every clock
# reading inside this one object.
WALL_FIELDS = ['wall']


def argv(cell, request, **options):
    command = [BIN, f'--cell={cell}', f'--request={REQUESTS.get(request, request)}',
               f'--edge={EDGE_MM}', f'--pair={PAIR_MM}']
    for key, value in sorted(options.items()):
        if value is None:
            continue
        command.append(f'--{key}={value}')
    return command


def run(cell, request, out_path, **options):
    """One process. Returns (document, wall_seconds, exit_status, stderr)."""
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    command = argv(cell, request, **options)
    started = time.monotonic()
    with open(out_path, 'w') as handle:
        result = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                                check=False)
    wall = time.monotonic() - started
    stderr = (result.stderr or b'').decode()[-2000:]
    try:
        with open(out_path) as handle:
            document = json.load(handle)
    except (json.JSONDecodeError, OSError):
        document = {'_loadError': stderr}
    return document, wall, result.returncode, stderr


def stripped(document, fields=None):
    """The document without its wall fields, for a bit-identical comparison."""
    fields = WALL_FIELDS if fields is None else fields
    copy = json.loads(json.dumps(document))
    for field in fields:
        copy.pop(field, None)
    return copy


def digest(document, fields=None):
    payload = json.dumps(stripped(document, fields), sort_keys=True,
                         separators=(',', ':'))
    return hashlib.sha256(payload.encode()).hexdigest()


def checkpoints(document):
    return document.get('outcome', {}).get('exactCheckpoints', [])


def published(document):
    return [row for row in checkpoints(document)
            if row.get('publishedRawDepthMm') is not None]


def invalid_publications(document):
    return sum(1 for row in published(document)
               if not (row.get('kernelExclusiveValid')
                       and row.get('contractValid')))


def max_repair_um(document):
    rows = published(document)
    if not rows:
        return 0.0
    return max(row.get('repairMaxDisplacementMm', 0.0) for row in rows) * 1000.0


def max_giveback_mm(document):
    rows = published(document)
    if not rows:
        return 0.0
    return max(row.get('repairDepthGivebackMm', 0.0) for row in rows)
