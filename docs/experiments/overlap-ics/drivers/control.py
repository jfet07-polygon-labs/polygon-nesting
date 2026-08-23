#!/usr/bin/env python3
"""The interleaved AB/BA wall-arm control. **Diagnostic only, never a lane.**

    python3 control.py <old-stack-binary> [seconds]

Sol review 17 Round 2 §5 asks for "nine diagnostic wall-control cells
interleaved AB/BA with the nine new-engine 10-second cells", as separate
processes, because "a single afterward control is too weak given the measured
2-5 mm box movement". This is that: for each seed, arm **A** is this round's
`CutCloseRelocate` at the same wall and arm **B** is the campaign's published
wall arm, run back to back in one order on even seeds and the other on odd
seeds, so an ordering effect cancels across the nine rather than accumulating.

**It cannot move the bar.** §0.1: "168.484 is absolute, the control can neither
rescue nor kill." Grok review 12 Round 1 §4.2 gives the reason - that arm
reproduces 0 of 3, so a paired test is a lottery that can false-pass on a bad
day for the wall or false-fail on a good one - and adds the second one: mixing
the old stack into this battery re-opens attribution, and ICS is the only lane.
What the control is *for* is session drift: if arm B lands far from its own
published 168.484 today, the reader knows the box moved and can discount
arm A's absolute number accordingly. That is a caveat on the evidence, not an
input to the verdict.

Arm B's invocation is the campaign's pinned positional tail, byte for byte from
`docs/experiments/real-interruption/drivers/runlib.py`, with `wall=<ms>` and
`v3=1` - the spec that produced 168.484 at 10.30 s in `docs/shipped-surface.md`
§1.1. Its binary is built from the **combo** feature set and is passed in rather
than built here, so the control cannot silently be a different build from the
one whose sha256 this document records.
"""
import hashlib
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

SEEDS = list(range(9))
WORKERS = 8

# The pinned positional CLI tail. Slot 26 is the seed.
ARGS = ('1 4 0 0 0 0 1 0 0 1 1 0 16 4 8 0 0 5 5 24 8 40 10 10 5 {seed} '
        '0.005 0.001 1 6 0 0 0 structured 0 10 1 0 0 0 0').split()
# The void-grid cell divisor salt sets, unchanged from PR7 / coordinator-v2 /
# the ledger / real-interruption.
SALT_SETS = {0: '13:15:17:19', 1: '11:15:21:27', 2: '15:23:31:39'}
ALLOWANCE = '0.002'
# The campaign's published wall-arm depth on mixed-61 at 10 s
# (docs/shipped-surface.md §1.1: 168.484 at 10.30 s, reproduces 0 of 3).
PUBLISHED_WALL_ARM_MM = 168.484


def sha256_of(path):
    try:
        with open(path, 'rb') as handle:
            return hashlib.sha256(handle.read()).hexdigest()
    except OSError:
        return None


def arm_b(binary, seed, seconds, out_path):
    """One old-stack wall-arm process."""
    spec = (f'wall={int(seconds * 1000)},'
            f'cells={SALT_SETS[seed % len(SALT_SETS)]},v3=1')
    command = ([binary, lib.REQUESTS['mixed-61']]
               + [a.format(seed=seed) for a in ARGS]
               + ['0', '', '', '', ALLOWANCE, spec])
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    started = time.monotonic()
    with open(out_path, 'w') as handle:
        result = subprocess.run(command, stdout=handle,
                                stderr=subprocess.PIPE, check=False)
    wall = time.monotonic() - started
    stderr = (result.stderr or b'').decode()[-600:]
    try:
        with open(out_path) as handle:
            document = json.load(handle)
    except (OSError, json.JSONDecodeError):
        document = {}
    portfolio = document.get('portfolio') or {}
    incumbent = portfolio.get('incumbent') or {}
    return {
        'arm': 'B-old-stack-wall',
        'seed': seed,
        'spec': spec,
        'exit': result.returncode,
        'stderr': stderr,
        'rawDepthMm': incumbent.get('rawDepthMm'),
        'dualGateValid': incumbent.get('dualGateValid'),
        'independentDepthMm': document.get('independentUsedLongAxisDepthMm'),
        'coordinatorSeconds': portfolio.get('elapsedSeconds'),
        'processWallSeconds': wall,
        # RV3. Arm B's document is written by a different binary and carries no
        # per-publication clock at all, so the sha is the only binding there is.
        'sourcePath': out_path,
        'sourceSha256': sha256_of(out_path),
    }


def arm_a(seed, seconds, out_path):
    """One `CutCloseRelocate` process at the same wall.

    **The publication set is filtered by the budget, in the budget's own
    frame.** This driver used to take a plain minimum over every publication
    the cell emitted, with no time filter of any kind - which meant arm A's
    reported depth could be a publication that landed after the arm's own
    `seconds`, while arm B's is what the old stack had at its deadline. The
    evidence audit records that as the caveat on this file ("min over all
    publications, no frame"); `wall.py` was repaired for the same defect one
    step milder (F1: a filter in the wrong clock frame, which could never
    fire). Both now go through `lib.within_budget`, so a future repair to one
    is a repair to both.

    The filter uses the LOWER bound: a publication is dropped only when it is
    *certainly* late. `undecidedByFrame` is the band the document cannot
    settle, and it is reported rather than resolved.
    """
    doc, wall, status, err = lib.run(
        'cutclose', 'mixed-61', out_path, mode='wall', wall=seconds,
        workers=WORKERS, seed=seed)
    outcome = doc.get('outcome', {})
    constructor = doc.get('constructor', {})
    within, late, undecided = lib.within_budget(
        outcome.get('publications', []), doc, seconds)
    strict = [row for row in within
              if row['placementFingerprint']
              != constructor.get('placementFingerprint')]
    return {
        'arm': 'A-cutclose',
        'seed': seed,
        'exit': status,
        'stderr': err[-600:] if err else '',
        'rawDepthMm': min((row['publishedRawDepthMm'] for row in strict),
                          default=None),
        'incumbentMm': outcome.get('incumbent', {}).get('rawSourceDepthMm'),
        'incumbentIsConstructor':
            outcome.get('incumbent', {}).get('fromConstructor'),
        'invalidPublications': outcome.get('invalidPublications'),
        'processWallSeconds': wall,
        'totalSeconds': doc.get('wall', {}).get('totalSeconds'),
        # The audit's caveat on this file, closed and reported.
        'publicationsTotal': len(outcome.get('publications', [])),
        'publicationsWithinBudget': len(within),
        'publicationsExcludedAsLate': len(late),
        'publicationsUndecidedByFrame': len(undecided),
        'strictChildrenWithinBudget': len(strict),
        # RV3.
        'sourcePath': out_path,
        'sourceSha256': lib.source_sha256(out_path),
    }


def main():
    binary = sys.argv[1]
    seconds = float(sys.argv[2]) if len(sys.argv) > 2 else 10.0
    out = os.environ.get('ICS_OUT', lib.OUT) + '/control'
    os.makedirs(out, exist_ok=True)
    pairs = []
    for seed in SEEDS:
        # AB on even seeds, BA on odd ones. The order is a function of the seed
        # and of nothing else, so it is reproducible and it is not chosen after
        # seeing a number.
        order = 'AB' if seed % 2 == 0 else 'BA'
        runs = {}
        for letter in order:
            if letter == 'A':
                runs['A'] = arm_a(seed, seconds,
                                  f'{out}/ctl-A-seed{seed}.json')
            else:
                runs['B'] = arm_b(binary, seed, seconds,
                                  f'{out}/ctl-B-seed{seed}.json')
        pairs.append({'seed': seed, 'order': order,
                      'A': runs['A'], 'B': runs['B'],
                      'deltaMm': (
                          None if runs['A']['rawDepthMm'] is None
                          or runs['B']['rawDepthMm'] is None
                          else runs['A']['rawDepthMm'] - runs['B']['rawDepthMm'])})

    b_depths = sorted(row['B']['rawDepthMm'] for row in pairs
                      if row['B']['rawDepthMm'] is not None)
    a_depths = sorted(row['A']['rawDepthMm'] for row in pairs
                      if row['A']['rawDepthMm'] is not None)

    def median(values):
        if not values:
            return None
        middle = len(values) // 2
        if len(values) % 2:
            return values[middle]
        return (values[middle - 1] + values[middle]) / 2.0

    document = {
        'experiment': 'overlap-ics',
        'battery': 'cutclose-round1-ab-ba-wall-control',
        # RV3: every cell document this reduction spawned, with its
        # sha256, so a reader can bind any row here to the bytes it
        # came from without re-deriving the reduction.
        'cellSources': lib.MANIFEST,
        'diagnosticOnly': True,
        'note': ('The bar is the pinned 168.484 and this control can neither '
                 'raise nor lower it (docs/cutclose-relocate-spec.md, "The '
                 'gate"). It is reported to expose session drift: arm B is the '
                 'campaign\'s published wall arm, which reproduces 0 of 3, so '
                 'its spread today is a statement about the box.'),
        'seconds': seconds,
        'armA': {'binary': lib.BIN, 'sha256': sha256_of(lib.BIN),
                 'what': 'CutCloseRelocate, feature overlap-ics, 8 workers'},
        'armB': {'binary': binary, 'sha256': sha256_of(binary),
                 'what': 'the campaign wall arm, combo features, wall=<ms>,v3=1'},
        'publishedWallArmMm': PUBLISHED_WALL_ARM_MM,
        'pairs': pairs,
        'armAMedianMm': median(a_depths),
        'armBMedianMm': median(b_depths),
        'armBSpreadMm': (None if len(b_depths) < 2
                         else b_depths[-1] - b_depths[0]),
        'armBMedianDriftFromPublishedMm': (
            None if median(b_depths) is None
            else median(b_depths) - PUBLISHED_WALL_ARM_MM),
        'armBDualValidEverywhere': all(
            row['B']['dualGateValid'] is True for row in pairs),
        'armAInvalidPublications': sum(
            row['A'].get('invalidPublications') or 0 for row in pairs),
        # The audit's caveat on this file, in the aggregate. A nonzero
        # `armAPublicationsExcludedAsLate` means arm A really did publish after
        # its own budget on some seed and the old reduction would have adopted
        # it; a nonzero undecided count means the document could not settle it
        # either way and the reader should say so rather than round.
        'armAPublicationsExcludedAsLate': sum(
            row['A'].get('publicationsExcludedAsLate') or 0 for row in pairs),
        'armAPublicationsUndecidedByFrame': sum(
            row['A'].get('publicationsUndecidedByFrame') or 0 for row in pairs),
    }
    print(json.dumps(document, indent=1))
    with open(f'{out}/control.json', 'w') as handle:
        json.dump(document, handle, indent=1)
    return 0


if __name__ == '__main__':
    sys.exit(main())
