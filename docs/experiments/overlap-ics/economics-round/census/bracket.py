#!/usr/bin/env python3
"""**How wide the checkpoint bracket is now, on a cell written today.**

    python3 bracket.py [work-dir] [seconds]

`frame_vector.py` proves the shared helper reduces every committed cell exactly
as `wall.py` always did. This measures the other half - the part that is
deliberately *not* the same - on a fresh wall cell.

Round 2 added a per-publication layout to every cell document (RV2), and the
document build happens between the loop's last clock read and `totalSeconds`.
The audit's upper bound for a publication's request-relative age was
`(totalSeconds - searchSeconds) + wallSeconds`, which contains that build, so
emitting more evidence made the undecided band wider: it went from the audit's
0.3 ms to 3.8 ms the moment the poses landed.

So the driver now emits `loopEntrySeconds` - the offset itself, read one
statement before the `Pacer` exists. This runs one wall cell and prints both
brackets side by side. The new one has to be **narrower** than the audit's
0.3 ms, not merely narrower than the regression it repairs, or the poses cost
the round something after all.
"""
import json
import os
import subprocess
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                    '..', '..', '..', '..', '..'))
REQUEST = (f'{ROOT}/tests/fixtures/mixed-61/'
           'mixed61-request-exact-clearance.json')
BIN = f'{ROOT}/target/release/examples/overlap_ics_benchmark'
# The audit's own measurement of the bracket it left behind, in milliseconds
# (evidence-audit README, F2: "the two bounds differ by 0.3 ms, so the bracket
# is tight enough to decide the clause"). The bar this script has to clear.
AUDIT_BRACKET_MS = 0.3

sys.path.insert(0, os.path.abspath(os.path.join(
    os.path.dirname(os.path.abspath(__file__)), '..', '..', 'drivers')))
import lib  # noqa: E402


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else '/var/lib/t3/tmp/census-wave1'
    seconds = float(sys.argv[2]) if len(sys.argv) > 2 else 3.0
    os.makedirs(out, exist_ok=True)
    path = f'{out}/bracket-cell.json'
    command = [BIN, '--cell=cutclose', f'--request={REQUEST}', '--edge=5',
               '--pair=5', '--mode=wall', f'--wall={seconds}', '--workers=8',
               '--seed=0']
    with open(path, 'w') as handle:
        result = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                                check=False)
    status = result.returncode
    if status != 0:
        print(json.dumps({'exit': status,
                          'stderr': (result.stderr or b'').decode()[-600:],
                          'BRACKET_PASS': False}, indent=1))
        return 1
    with open(path) as handle:
        document = json.load(handle)
    wall = document['wall']
    lower = wall['constructorSeconds']
    emitted = wall.get('loopEntrySeconds')
    old_upper = wall['totalSeconds'] - wall['searchSeconds']
    publications = document['outcome']['publications']
    within, late, undecided = lib.within_budget(publications, document, seconds)
    new_ms = None if emitted is None else (emitted - lower) * 1000.0
    reduced = {
        'experiment': 'overlap-ics',
        'battery': 'economics-round-census-checkpoint-bracket',
        'exit': status,
        'binary': BIN,
        'sourcePath': path,
        'sourceSha256': lib.source_sha256(path),
        'budgetSeconds': seconds,
        'publications': len(publications),
        'constructorSeconds': lower,
        'loopEntrySeconds': emitted,
        'totalMinusSearchSeconds': old_upper,
        'oldBracketMs': (old_upper - lower) * 1000.0,
        'newBracketMs': new_ms,
        'auditBracketMs': AUDIT_BRACKET_MS,
        'narrowerThanAudit': bool(new_ms is not None
                                  and new_ms < AUDIT_BRACKET_MS),
        'publicationsWithinBudget': len(within),
        'publicationsExcludedAsLate': len(late),
        'publicationsUndecidedByFrame': len(undecided),
        'maxRequestSecondsLower': max(
            (lib.request_seconds(row, lower) for row in publications),
            default=None),
        'maxRequestSecondsUpper': max(
            (lib.request_seconds(row, emitted) for row in publications),
            default=None),
    }
    reduced['BRACKET_PASS'] = bool(reduced['narrowerThanAudit'])
    print(json.dumps(reduced, indent=1))
    with open(f'{out}/bracket.json', 'w') as handle:
        json.dump(reduced, handle, indent=1)
    return 0 if reduced['BRACKET_PASS'] else 1


if __name__ == '__main__':
    sys.exit(main())
