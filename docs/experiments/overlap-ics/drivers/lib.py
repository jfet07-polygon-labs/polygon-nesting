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


# **Every cell document this process spawned, in order, with its sha256.**
#
# The evidence audit's RV3 is that a reduction never said which bytes it
# reduced, so binding a committed reduction back to its cell documents took a
# re-derivation of all 702 of its fields. Every driver appends this to its own
# document under `cellSources`, which makes the binding a field. It is
# maintained by `run` rather than by each driver, so a cell that is spawned and
# then dropped from the reduction is still listed - which is the case a
# per-row sha cannot cover and the one most worth covering.
MANIFEST = []


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
    MANIFEST.append({
        'cell': cell,
        'path': out_path,
        'sha256': source_sha256(out_path),
        'bytes': (os.path.getsize(out_path) if os.path.exists(out_path)
                  else None),
        'exit': result.returncode,
        'binary': BIN,
    })
    return document, wall, result.returncode, stderr


def source_sha256(path):
    """The sha256 of a raw cell document, for the reduction that reads it.

    The evidence audit's revalidation chapter had to re-derive all 702 fields
    of a committed reduction to bind it to the cell documents it came from,
    because no reduction said which bytes it had reduced (RV3). Every per-cell
    row this campaign emits carries this now, so the binding is a field rather
    than a reconstruction.
    """
    try:
        with open(path, 'rb') as handle:
            return hashlib.sha256(handle.read()).hexdigest()
    except OSError:
        return None


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


# --------------------------------------------------- the checkpoint frame ---
#
# **The clock the engine reports is not the clock the budget is measured in.**
#
# `PublishedBite.wallSeconds` is `Pacer::elapsed_s()`, and the `Pacer` is
# constructed inside `Engine::run_cutclose` - after the constructor has already
# spent its share of the request's budget. So a publication's `wallSeconds` is
# measured from the moment the loop entered, while `--wall` is measured from
# the decoded request, and on mixed-61 the two are about 2.3 s apart.
#
# The evidence audit's F1 is that comparing `wallSeconds <= limit` directly is
# therefore not a filter at all: its left side is bounded above by
# `limit - constructorSeconds` by construction, so it cannot exclude anything
# on any cell, whatever the loop does. `wall.py` was repaired for this in the
# audit; `control.py` had no time filter at all and took a plain minimum over
# every publication, which is the same defect one step further along - it can
# adopt a publication that landed after the arm's own budget.
#
# This is that repair, in one place, so the two drivers cannot drift apart
# again. The offset is not emitted directly, so both bounds the document does
# carry are computed:
#
#   * `requestSecondsLower = constructorSeconds + wallSeconds` - excludes the
#     engine construction between the two clock reads, so it is a LOWER bound
#     on a publication's age;
#   * `requestSecondsUpper = loopEntrySeconds + wallSeconds` - the offset
#     itself, read one statement before the `Pacer` exists, so it misses only
#     the call prologue. Documents written before the economics round do not
#     carry it and fall back to `(totalSeconds - searchSeconds) + wallSeconds`,
#     which is everything outside `run_cutclose` INCLUDING the document build
#     after it, and is therefore a much looser upper bound - it widened from
#     0.3 ms to 3.3 ms on mixed-61 the moment the driver started emitting
#     per-publication poses.
#
# A publication is excluded only when it is *certainly* late, i.e. when even
# the lower bound is past the budget. One whose two bounds straddle the budget
# is counted and reported as undecided, because the document cannot settle it
# and a driver that pretended otherwise would be inventing precision.


def checkpoint_frame(document):
    """The two offsets that convert a loop-relative clock into a request one.

    Returns `(lower, upper)`. `upper` prefers the emitted `loopEntrySeconds`
    and falls back to the old bracket for documents written before it existed,
    so every committed cell keeps reducing to exactly what it always did.
    """
    wall = document.get('wall') or {}
    constructor_s = wall.get('constructorSeconds')
    search_s = wall.get('searchSeconds')
    total_s = wall.get('totalSeconds')
    entry_s = wall.get('loopEntrySeconds')
    if entry_s is not None:
        return constructor_s, entry_s
    upper = (None if total_s is None or search_s is None
             else total_s - search_s)
    return constructor_s, upper


def request_seconds(row, offset):
    loop_s = row.get('wallSeconds')
    if loop_s is None or offset is None:
        return None
    return offset + loop_s


def within_budget(publications, document, limit):
    """`(within, late, undecided)` for one cell's publications.

    `within` is what an anytime answer may be taken from: everything except the
    publications this document can prove landed after `limit`.
    """
    lower_offset, upper_offset = checkpoint_frame(document)
    within, late, undecided = [], [], []
    for row in publications:
        low = request_seconds(row, lower_offset)
        high = request_seconds(row, upper_offset)
        if low is not None and low > limit:
            late.append(row)
            continue
        within.append(row)
        if low is not None and high is not None and high > limit:
            undecided.append(row)
    return within, late, undecided


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
