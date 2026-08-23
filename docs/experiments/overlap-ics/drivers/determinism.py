#!/usr/bin/env python3
"""Two independently built binaries, same fixed-work trajectory, same document.

    python3 determinism.py <binary-a> <binary-b> [budget]

The two-process comparison in `smoke.py` catches ordering and allocation
nondeterminism inside one binary. This catches the other half - a build that
differs - and it is the round-boundary form of the claim in
docs/overlap-ics-converged-spec.md:

    same request, seed, binary, x86 target, Rust toolchain, libm
    implementation, feature set, worker count and fixed work quota produce
    bit-identical poses, checkpoint sequence and publications.

The two binaries here differ in their build directory and therefore in their
own SHA-256; everything the *trajectory* can see is the same, and the
comparison strips the `wall` object and the `executableSha256` field for
exactly that reason - the second is the thing being varied, so leaving it in
would make the comparison trivially false.

Cross-platform `sin`/`cos` identity is **not** a claim, here or anywhere in
this campaign.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

STRIP = lib.WALL_FIELDS + ['executableSha256']


def run_with(binary, cell_name, request, out_path, options):
    """One process from one binary. `lib.BIN` is the only global involved and it
    is put back before returning, so a caller cannot leave it pointing at the
    wrong build."""
    saved = lib.BIN
    lib.BIN = binary
    try:
        return lib.run(cell_name, request, out_path, **options)
    finally:
        lib.BIN = saved


def main():
    binary_a = sys.argv[1]
    binary_b = sys.argv[2]
    budget = int(sys.argv[3]) if len(sys.argv) > 3 else 200_000
    out = os.environ.get('ICS_OUT', lib.OUT) + '/determinism'
    cells = [
        ('s0', 'mixed-61', dict(poses=lib.SPARROW_POSES, target=150.16547,
                                budget=0, seed=0)),
        ('s1', 'mixed-61', dict(poses=lib.SPARROW_POSES, target=150.16547,
                                budget=budget, relocateevals=200_000, seed=0,
                                perturbmm=0.5, perturbdeg=2.0,
                                checkpointevery=1)),
        ('c175', 'mixed-61', dict(budget=budget, seed=0, checkpointevery=1)),
        ('triangle', 'triangle-20', dict(target=70.742, budget=budget,
                                         relocateevals=200_000, seed=0,
                                         checkpointevery=1)),
        # This round's member. Fixed work, eight workers, eight explore bites
        # and two compress bites: the same trajectory `cutclose.py`'s `bites`
        # stage compares across two processes, here compared across two
        # independently built binaries.
        ('cutclose', 'mixed-61', dict(mode='fixed', bites=8, attempts=2,
                                      iters=400, compressbites=2, workers=8,
                                      seed=0)),
    ]
    rows = []
    identical = True
    for name, request, options in cells:
        first, _, status_a, err_a = run_with(
            binary_a, name, request, f'{out}/{name}-a.json', options)
        second, _, status_b, err_b = run_with(
            binary_b, name, request, f'{out}/{name}-b.json', options)
        same = (status_a == 0 and status_b == 0
                and lib.stripped(first, STRIP) == lib.stripped(second, STRIP))
        identical = identical and same
        rows.append({
            'cell': name,
            'request': request,
            'exitA': status_a,
            'exitB': status_b,
            'digestA': lib.digest(first, STRIP),
            'digestB': lib.digest(second, STRIP),
            # RV3: the two cell documents this row reduced, by sha. A
            # two-BINARY comparison is the one place a reader most needs to be
            # able to check which bytes each side actually read.
            'sourceShaA': lib.source_sha256(f'{out}/{name}-a.json'),
            'sourceShaB': lib.source_sha256(f'{out}/{name}-b.json'),
            'bitIdentical': same,
            'stderrA': err_a[-400:] if err_a else '',
            'stderrB': err_b[-400:] if err_b else '',
        })
    # **Whether this battery compared two binaries at all.**
    #
    # The economics round ran it with `binary_b` built from the same source and
    # the same feature set into a second `CARGO_TARGET_DIR`, and got a
    # byte-identical executable: `2c5da1ac…` both sides. On a toolchain whose
    # builds are reproducible that is the normal outcome, and it makes every
    # row below **trivially** true - the comparison is a binary against itself,
    # which is the "false-green tests (literals checked, transitions not)"
    # shape the evidence audit's F9 names.
    #
    # It is not a reason to stop running this; a build that stopped being
    # reproducible is worth knowing about too. It is a reason for the document
    # to SAY which of the two things it measured, instead of letting
    # `TWO_BINARY_IDENTICAL: true` be read as a cross-build claim when the two
    # builds are one file. Additive fields only; no verdict depends on them.
    sha_a, sha_b = lib.source_sha256(binary_a), lib.source_sha256(binary_b)
    document = {
        'experiment': 'overlap-ics',
        'battery': 'two-binary-determinism',
        # RV3: every cell document this reduction spawned, with its
        # sha256, so a reader can bind any row here to the bytes it
        # came from without re-deriving the reduction.
        'cellSources': lib.MANIFEST,
        'binaryA': binary_a,
        'binaryB': binary_b,
        'binaryASha256': sha_a,
        'binaryBSha256': sha_b,
        # `false` means the two builds produced one executable, so the cells
        # below re-prove single-binary determinism and nothing more.
        'binariesDiffer': (None if sha_a is None or sha_b is None
                           else sha_a != sha_b),
        'proposalBudget': budget,
        'strippedFields': STRIP,
        'cells': rows,
        'TWO_BINARY_IDENTICAL': identical,
    }
    print(json.dumps(document, indent=1))
    os.makedirs(out, exist_ok=True)
    with open(f'{out}/determinism.json', 'w') as handle:
        json.dump(document, handle, indent=1)
    return 0 if identical else 1


if __name__ == '__main__':
    sys.exit(main())
