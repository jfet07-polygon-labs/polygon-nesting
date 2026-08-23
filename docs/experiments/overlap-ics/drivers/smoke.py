#!/usr/bin/env python3
"""The two-process fixed-work smoke: S0's pinned canary and S1's locked strip.

    python3 smoke.py [budget]

Two separate processes per cell, same seed, same work vector, compared
**bit-for-bit** after stripping one named field list (`lib.WALL_FIELDS`). That
is the whole point of a fixed-work smoke rather than a wall smoke: a wall smoke
cannot see `f64` trajectory nondeterminism, because the two runs would not have
done the same work anyway.

The comparison covers everything the spec's list names, and it covers it by
covering the entire document minus the wall object:

    every x, y and theta bit          -> `finalPoseDigest`, `perturbedPoseDigest`
    raw and guided Phi                 -> `outcome.proxy`, `entry`
    step digest                        -> `finalPoseDigest`
    work counters                      -> `outcome.work` (all ten)
    exact attempts/refusals/publications -> `outcome.exactCheckpoints`
    repair displacement and giveback   -> the same rows
    placement fingerprint and raw depth -> `outcome.incumbent`

S0's pins are the spec's, verbatim: 61 placements, `rawSourceDepthMm`
150.16451, `phi.to_bits() == 0` at `c_pair = 5.0`, Exclusive accepts at
`two_r = 5000`, the contract accepts, zero repair.

S1's are precommitted numbers rather than a digest, exactly as Sol review 14
Round 2 §3 requires until a first independently validated implementation
establishes one: locked `W = 150.16547`, dual-valid republication inside that
`W`, per-piece repair <= 0.016 mm, giveback <= 0.050 mm.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

S0_DEPTH_MM = 150.16451
S0_PLACEMENTS = 61
S1_LOCKED_W_MM = 150.16547
S1_MAX_REPAIR_MM = 0.016
S1_MAX_GIVEBACK_MM = 0.050

# **S1's quota, re-denominated.** Grok review 12 Round 1 §4.3: "Work quota for
# S1: 200,000 **relocate-evals** (not PGS proposals)". Both numbers are passed
# and the document says which one bound the run, because they are not
# interchangeable and neither alone is sufficient:
#
#   * `relocateevals` is the *work* the operator is licensed to spend, and it is
#     the unit the spec names. It is a cap S1 does not reach - the cell converges
#     at ~83.6 K - which is the honest reading of "quota", not a defect;
#   * `budget` is what makes a converged cell **stop**. Once Phi = 0 the
#     colliding set is empty, every further sweep relocates nothing and spends
#     zero relocate-evals, so a relocate-eval quota alone never terminates. With
#     the backstop removed this cell ran 10^9 empty slots in 155 seconds and
#     still finished 116 K relocate-evals short of the cap.
S1_RELOCATE_EVAL_QUOTA = 200_000



def two_process(cell, out, **options):
    first, _, status_a, err_a = lib.run(
        cell, 'mixed-61', f'{out}/{cell}-process-a.json', **options)
    second, _, status_b, err_b = lib.run(
        cell, 'mixed-61', f'{out}/{cell}-process-b.json', **options)
    return {
        'exitA': status_a,
        'exitB': status_b,
        'stderrA': err_a,
        'stderrB': err_b,
        'digestA': lib.digest(first),
        'digestB': lib.digest(second),
        'strippedFields': lib.WALL_FIELDS,
        # RV3: the two cell documents this comparison reduced, by sha.
        'sourceShaA': lib.source_sha256(f'{out}/{cell}-process-a.json'),
        'sourceShaB': lib.source_sha256(f'{out}/{cell}-process-b.json'),
        'bitIdentical': (status_a == 0 and status_b == 0
                         and lib.stripped(first) == lib.stripped(second)),
    }, first


def main():
    out = os.environ.get('ICS_OUT', lib.OUT)
    budget = int(sys.argv[1]) if len(sys.argv) > 1 else 200_000

    s0_compare, s0 = two_process(
        's0', out, poses=lib.SPARROW_POSES, target=S1_LOCKED_W_MM, budget=0,
        seed=0)
    entry = s0.get('entry', {})
    rows = lib.published(s0)
    row = rows[0] if rows else {}
    s0_pins = {
        'placementCount': s0.get('poses', {}).get('placementCount'),
        'rawSourceDepthMm': entry.get('rawSourceDepthMm'),
        'phiBits': entry.get('rawPhiBits'),
        'twoRMicron': s0.get('contract', {}).get('twoRMicron'),
        'pairClearanceMm': s0.get('contract', {}).get('pairClearanceMm'),
        'kernelExclusiveValid': row.get('kernelExclusiveValid'),
        'contractValid': row.get('contractValid'),
        'repairRows': row.get('repairRows'),
        'repairDepthGivebackMm': row.get('repairDepthGivebackMm'),
    }
    s0_pass = (s0_pins['placementCount'] == S0_PLACEMENTS
               and s0_pins['rawSourceDepthMm'] == S0_DEPTH_MM
               and s0_pins['phiBits'] == 0
               and s0_pins['pairClearanceMm'] == 5.0
               and s0_pins['twoRMicron'] == 5000
               and s0_pins['kernelExclusiveValid'] is True
               and s0_pins['contractValid'] is True
               and s0_pins['repairRows'] == 0
               and s0_pins['repairDepthGivebackMm'] == 0.0
               and s0_compare['bitIdentical'])

    s1_compare, s1 = two_process(
        's1', out, poses=lib.SPARROW_POSES, target=S1_LOCKED_W_MM,
        budget=budget, relocateevals=S1_RELOCATE_EVAL_QUOTA, seed=0,
        perturbmm=0.5, perturbdeg=2.0, checkpointevery=1)
    s1_rows = lib.published(s1)
    outcome = s1.get('outcome', {})
    s1_measured = {
        'lockedWmm': S1_LOCKED_W_MM,
        'quota': s1.get('quota'),
        'perturbationMm': 0.5,
        'perturbationDeg': 2.0,
        'perturbedPoseDigest': s1.get('poses', {}).get('perturbedPoseDigest'),
        'entryRawPhi': s1.get('entry', {}).get('rawPhi'),
        'finalRawPhi': outcome.get('proxy', {}).get('rawPhi'),
        'finalMaxViolationMm': outcome.get('proxy', {}).get('maxViolationMm'),
        'finalRawDepthMm': outcome.get('proxy', {}).get('rawSourceDepthMm'),
        'republished': bool(s1_rows),
        'publishedRawDepthMm': (outcome.get('incumbent', {})
                                .get('rawSourceDepthMm')),
        'maxRepairMm': lib.max_repair_um(s1) / 1000.0,
        'maxGivebackMm': lib.max_giveback_mm(s1),
        'invalidPublications': lib.invalid_publications(s1),
        'exactCheckpoints': len(lib.checkpoints(s1)),
    }
    # The invariant half: never violated, whatever the mechanism does.
    s1_invariants = (s1_measured['invalidPublications'] == 0
                     and s1_measured['maxRepairMm'] <= S1_MAX_REPAIR_MM
                     and s1_measured['maxGivebackMm'] <= S1_MAX_GIVEBACK_MM
                     and all(row['publishedRawDepthMm'] <= S1_LOCKED_W_MM
                             for row in s1_rows)
                     and s1_compare['bitIdentical'])
    # The mechanism half: does it come back at all.
    s1_pass = s1_invariants and s1_measured['republished']

    document = {
        'experiment': 'overlap-ics',
        'battery': 'two-process-fixed-work-smoke',
        # RV3: every cell document this reduction spawned, with its
        # sha256, so a reader can bind any row here to the bytes it
        # came from without re-deriving the reduction.
        'cellSources': lib.MANIFEST,
        'binary': lib.BIN,
        'proposalBudget': budget,
        's0': {'pins': s0_pins, 'twoProcess': s0_compare, 'pass': s0_pass},
        's1': {'measured': s1_measured, 'twoProcess': s1_compare,
               'invariantsHold': s1_invariants, 'pass': s1_pass},
        'SMOKE_PASS': bool(s0_pass and s1_pass),
        'INVARIANTS_PASS': bool(s0_pass and s1_invariants),
    }
    print(json.dumps(document, indent=1))
    with open(f'{out}/smoke.json', 'w') as handle:
        json.dump(document, handle, indent=1)
    return 0 if document['SMOKE_PASS'] else 1


if __name__ == '__main__':
    sys.exit(main())
