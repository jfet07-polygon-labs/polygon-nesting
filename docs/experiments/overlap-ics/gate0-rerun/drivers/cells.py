#!/usr/bin/env python3
"""Gate 0's battery, re-run under the frozen fix list.

    python3 cells.py [cell ...]        # default: all of them

This is the previous round's `drivers/cells.py` with exactly three differences,
all of them named in the re-run's README §0 before the battery ran:

  1. C175's quota is 240,000, derived in README §0.6 from the *original*
     round's slowest measured throughput and the spec's two-solver-second
     clause. Every other cell keeps 200,000.
  2. Every cell's verdict row now carries the jump accounting - `jumpAttempted`,
     `jumpCommitted`, and the recorded scale of every attempt - because the
     pre-committed reading of S1 is stated in those terms.
  3. C175 additionally requires `entryDepthWithinTarget` on every seed. The
     binary asserts it as a hard failure; this is the belt to that's braces.

The fatal/diagnostic split is unchanged, and it is the converged spec's
arbitration rather than this driver's opinion:

  FATAL       S0, S1, C175, triangle-20, numeric soundness, throughput
  DIAGNOSTIC  S2, C168, random-T, the 10,000-state corpus
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

# The pins. Every one of these is a committed number from the spec of record,
# never a value read back off a run.
S0_DEPTH_MM = 150.16451
S0_PLACEMENTS = 61
S1_LOCKED_W_MM = 150.16547
S1_MAX_REPAIR_UM = 16.0
S1_MAX_GIVEBACK_MM = 0.050
TRIANGLE_W_MM = 70.742
C168_W_MM = 168.484
CORPUS_STATES = 1_000
HEAVY_CORPUS_STATES = 10_000
# README §0.6: floor(987,861 / 8 s x 2 s), from the original round's slowest
# measured throughput. Derived before the code, written down before the run.
C175_BUDGET = 240_000
CELL_BUDGET = 200_000
# The two-scale gate, `25 * EPSILON_GRID_MM`. Read here only to *report* which
# side of it a cell stalled on; the binary owns the number.
JUMP_STRIP_THRESHOLD_MM = 0.100


def verdict(name, fatal, passed, detail):
    return {'cell': name, 'fatal': fatal, 'pass': bool(passed), **detail}


def jump_detail(doc):
    outcome = doc.get('outcome', {})
    events = lib.jump_events(doc)
    return {
        'jumps': outcome.get('jumps'),
        'jumpAttempted': outcome.get('jumpAttempted'),
        'jumpCommitted': outcome.get('jumpCommitted'),
        'jumpsImprovingGuided': outcome.get('jumpsImprovingGuided'),
        'jumpEvents': events,
        'stripJumpFired': lib.strip_jump_fired(doc),
        'jumpStripThresholdMm': JUMP_STRIP_THRESHOLD_MM,
    }


def s0(out):
    doc, wall, status, err = lib.run(
        's0', 'mixed-61', f'{out}/s0.json', poses=lib.SPARROW_POSES,
        target=S1_LOCKED_W_MM, budget=0)
    if status != 0:
        return verdict('S0', True, False, {'exit': status, 'stderr': err})
    entry = doc.get('entry', {})
    rows = lib.published(doc)
    row = rows[0] if rows else {}
    detail = {
        'exit': status,
        'placementCount': doc.get('poses', {}).get('placementCount'),
        'rawSourceDepthMm': entry.get('rawSourceDepthMm'),
        'phiBits': entry.get('rawPhiBits'),
        'kernelExclusiveValid': row.get('kernelExclusiveValid'),
        'contractValid': row.get('contractValid'),
        'repairRows': row.get('repairRows'),
        'repairDepthGivebackMm': row.get('repairDepthGivebackMm'),
        'twoRMicron': doc.get('contract', {}).get('twoRMicron'),
        'physicalEdgeClearanceMm': doc.get('contract', {}).get('physicalEdgeClearanceMm'),
        'depthTopInsetMm': doc.get('contract', {}).get('depthTopInsetMm'),
        'wallSeconds': wall,
    }
    passed = (detail['placementCount'] == S0_PLACEMENTS
              and detail['rawSourceDepthMm'] == S0_DEPTH_MM
              and detail['phiBits'] == 0
              and detail['kernelExclusiveValid'] is True
              and detail['contractValid'] is True
              and detail['repairRows'] == 0
              and detail['repairDepthGivebackMm'] == 0.0
              and detail['twoRMicron'] == 5000)
    return verdict('S0', True, passed, detail)


def perturbed(name, cell, out, fatal, perturb_mm, perturb_deg, budget):
    doc, wall, status, err = lib.run(
        cell, 'mixed-61', f'{out}/{cell}.json', poses=lib.SPARROW_POSES,
        target=S1_LOCKED_W_MM, budget=budget, seed=0,
        perturbmm=perturb_mm, perturbdeg=perturb_deg, checkpointevery=1)
    if status != 0:
        return verdict(name, fatal, False, {'exit': status, 'stderr': err})
    outcome = doc.get('outcome', {})
    rows = lib.published(doc)
    detail = {
        'exit': status,
        'perturbationMm': perturb_mm,
        'perturbationDeg': perturb_deg,
        'perturbedPoseDigest': doc.get('poses', {}).get('perturbedPoseDigest'),
        'entryRawPhi': doc.get('entry', {}).get('rawPhi'),
        'entryMaxViolationMm': doc.get('entry', {}).get('maxViolationMm'),
        'finalRawPhi': outcome.get('proxy', {}).get('rawPhi'),
        'finalMaxViolationMm': outcome.get('proxy', {}).get('maxViolationMm'),
        'finalRawDepthMm': outcome.get('proxy', {}).get('rawSourceDepthMm'),
        'census': outcome.get('census'),
        'lockedWmm': S1_LOCKED_W_MM,
        'publications': len(rows),
        'publishedRawDepthMm': outcome.get('incumbent', {}).get('rawSourceDepthMm'),
        'invalidPublications': lib.invalid_publications(doc),
        'maxRepairUm': lib.max_repair_um(doc),
        'maxGivebackMm': lib.max_giveback_mm(doc),
        'exactCheckpoints': len(lib.checkpoints(doc)),
        'firstStrictChildProposal': outcome.get('firstStrictChildProposal'),
        'work': outcome.get('work'),
        'rejectionCensus': lib.rejection_census(doc),
        'wallSeconds': wall,
        'solverSeconds': doc.get('wall', {}).get('solverSeconds'),
    }
    detail.update(jump_detail(doc))
    dual = bool(rows) and all(row['kernelExclusiveValid'] and row['contractValid']
                              for row in rows)
    within = all(row['publishedRawDepthMm'] <= S1_LOCKED_W_MM for row in rows)
    passed = (dual and within
              and detail['invalidPublications'] == 0
              and detail['maxRepairUm'] <= S1_MAX_REPAIR_UM
              and detail['maxGivebackMm'] <= S1_MAX_GIVEBACK_MM)
    return verdict(name, fatal, passed, detail)


def c175(out):
    """The fatal inflation cell: three fixed seeds, a 0.10 (D0 - L) shock, a
    strict dual-valid non-constructor child inside two solver seconds."""
    seeds = []
    for seed in (0, 1, 2):
        doc, wall, status, err = lib.run(
            'c175', 'mixed-61', f'{out}/c175-seed{seed}.json', seed=seed,
            budget=C175_BUDGET, checkpointevery=1)
        if status != 0:
            seeds.append({'seed': seed, 'exit': status, 'stderr': err,
                          'strictChild': False, 'entryWithinTarget': False})
            continue
        constructor = doc.get('constructor', {})
        outcome = doc.get('outcome', {})
        incumbent = outcome.get('incumbent', {})
        rows = lib.published(doc)
        d0 = constructor.get('rawSourceDepthMm')
        depth = incumbent.get('rawSourceDepthMm')
        strict = (not incumbent.get('fromConstructor')
                  and incumbent.get('fingerprintDiffersFromConstructor')
                  and depth is not None and d0 is not None and depth < d0)
        dual = all(row['kernelExclusiveValid'] and row['contractValid']
                   for row in rows)
        row = {
            'seed': seed,
            'exit': status,
            'constructorDepthMm': d0,
            'lowerScaleMm': doc.get('lowerScaleMm'),
            'lockedTargetMm': doc.get('entry', {}).get('lockedTargetMm'),
            'shockMm': constructor.get('shockMm'),
            'halfShockMm': constructor.get('halfShockMm'),
            'entryRawPhi': doc.get('entry', {}).get('rawPhi'),
            'entryMaxViolationMm': doc.get('entry', {}).get('maxViolationMm'),
            'entryRawDepthMm': doc.get('entry', {}).get('rawSourceDepthMm'),
            'entryWithinTarget': doc.get('shock', {}).get('entryDepthWithinTarget'),
            'entryDepthSlackMm': doc.get('shock', {}).get('entryDepthSlackMm'),
            'finalRawPhi': outcome.get('proxy', {}).get('rawPhi'),
            'finalGuidedPhi': outcome.get('proxy', {}).get('guidedPhi'),
            'finalMaxViolationMm': outcome.get('proxy', {}).get('maxViolationMm'),
            'finalRawDepthMm': outcome.get('proxy', {}).get('rawSourceDepthMm'),
            'census': outcome.get('census'),
            'shock': doc.get('shock'),
            'finalPoseDigest': doc.get('finalPoseDigest'),
            'publishedDepthMm': depth,
            'recoveredMm': None if depth is None or d0 is None else d0 - depth,
            'strictChild': bool(strict),
            'dualValid': dual,
            'publications': len(rows),
            'exactCheckpoints': len(lib.checkpoints(doc)),
            'invalidPublications': lib.invalid_publications(doc),
            'maxRepairUm': lib.max_repair_um(doc),
            'maxGivebackMm': lib.max_giveback_mm(doc),
            'solverSeconds': doc.get('wall', {}).get('solverSeconds'),
            'wallSeconds': wall,
            'work': outcome.get('work'),
            'rejectionCensus': lib.rejection_census(doc),
        }
        row.update(jump_detail(doc))
        seeds.append(row)
    children = sum(1 for row in seeds if row.get('strictChild'))
    within_two_seconds = all((row.get('solverSeconds') or 1e9) <= 2.0
                             for row in seeds)
    entries_within = all(row.get('entryWithinTarget') is True for row in seeds)
    invalid = sum(row.get('invalidPublications', 0) for row in seeds)
    passed = (children >= 1 and invalid == 0 and within_two_seconds
              and entries_within)
    return verdict('C175', True, passed, {
        'proposalBudget': C175_BUDGET,
        'seeds': seeds,
        'strictChildren': children,
        'invalidPublications': invalid,
        'allWithinTwoSolverSeconds': within_two_seconds,
        'allEntriesWithinTarget': entries_within,
    })


def triangle(out):
    doc, wall, status, err = lib.run(
        'triangle', 'triangle-20', f'{out}/triangle20.json',
        target=TRIANGLE_W_MM, budget=CELL_BUDGET, seed=0, checkpointevery=1)
    if status != 0:
        return verdict('triangle-20', True, False, {'exit': status, 'stderr': err})
    outcome = doc.get('outcome', {})
    incumbent = outcome.get('incumbent', {})
    rows = lib.published(doc)
    detail = {
        'exit': status,
        'lockedWmm': TRIANGLE_W_MM,
        'lowerScaleMm': doc.get('lowerScaleMm'),
        'constructorDepthMm': doc.get('constructor', {}).get('rawSourceDepthMm'),
        'entryRawPhi': doc.get('entry', {}).get('rawPhi'),
        'entryRawDepthMm': doc.get('entry', {}).get('rawSourceDepthMm'),
        'entryWithinTarget': doc.get('shock', {}).get('entryDepthWithinTarget'),
        'finalRawPhi': outcome.get('proxy', {}).get('rawPhi'),
        'finalMaxViolationMm': outcome.get('proxy', {}).get('maxViolationMm'),
        'finalRawDepthMm': outcome.get('proxy', {}).get('rawSourceDepthMm'),
        'census': outcome.get('census'),
        'publishedDepthMm': incumbent.get('rawSourceDepthMm'),
        'strictChild': not incumbent.get('fromConstructor'),
        'publications': len(rows),
        'exactCheckpoints': len(lib.checkpoints(doc)),
        'invalidPublications': lib.invalid_publications(doc),
        'maxRepairUm': lib.max_repair_um(doc),
        'maxGivebackMm': lib.max_giveback_mm(doc),
        'firstStrictChildProposal': outcome.get('firstStrictChildProposal'),
        'work': outcome.get('work'),
        'rejectionCensus': lib.rejection_census(doc),
        'solverSeconds': doc.get('wall', {}).get('solverSeconds'),
        'wallSeconds': wall,
    }
    detail.update(jump_detail(doc))
    legalized = bool(rows) and all(
        row['kernelExclusiveValid'] and row['contractValid']
        and row['publishedRawDepthMm'] <= TRIANGLE_W_MM for row in rows)
    passed = (legalized and detail['invalidPublications'] == 0
              and detail['maxRepairUm'] <= S1_MAX_REPAIR_UM
              and detail['maxGivebackMm'] <= S1_MAX_GIVEBACK_MM)
    return verdict('triangle-20', True, passed, detail)


def corpus(out, states, fatal, name):
    doc, wall, status, err = lib.run(
        'corpus', 'mixed-61', f'{out}/{name}.json', states=states, seed=0)
    if status != 0:
        return verdict(name, fatal, False, {'exit': status, 'stderr': err})
    detail = dict(doc.get('corpus', {}))
    detail.update(doc.get('verdict', {}))
    detail['wallSeconds'] = wall
    detail['corpusSeconds'] = doc.get('wall', {}).get('corpusSeconds')
    return verdict(name, fatal, doc.get('verdict', {}).get('pass'), detail)


def throughput(out):
    doc, wall, status, err = lib.run(
        'throughput', 'mixed-61', f'{out}/throughput.json',
        repeats=300, proposals=20_000, seed=0)
    if status != 0:
        return verdict('throughput', True, False, {'exit': status, 'stderr': err})
    detail = dict(doc.get('throughput', {}))
    detail['wallSeconds'] = wall
    return verdict('throughput', True, detail.get('pass'), detail)


def c168(out):
    doc, wall, status, err = lib.run(
        'c168', 'mixed-61', f'{out}/c168.json', target=C168_W_MM,
        budget=CELL_BUDGET, seed=0, checkpointevery=1)
    if status != 0:
        return verdict('C168', False, False, {'exit': status, 'stderr': err})
    outcome = doc.get('outcome', {})
    incumbent = outcome.get('incumbent', {})
    rows = lib.published(doc)
    detail = {
        'exit': status,
        'lockedWmm': C168_W_MM,
        'constructorDepthMm': doc.get('constructor', {}).get('rawSourceDepthMm'),
        'entryRawPhi': doc.get('entry', {}).get('rawPhi'),
        'entryWithinTarget': doc.get('shock', {}).get('entryDepthWithinTarget'),
        'finalRawPhi': outcome.get('proxy', {}).get('rawPhi'),
        'finalMaxViolationMm': outcome.get('proxy', {}).get('maxViolationMm'),
        'census': outcome.get('census'),
        'publishedDepthMm': incumbent.get('rawSourceDepthMm'),
        'strictChild': not incumbent.get('fromConstructor'),
        'publications': len(rows),
        'exactCheckpoints': len(lib.checkpoints(doc)),
        'invalidPublications': lib.invalid_publications(doc),
        'maxRepairUm': lib.max_repair_um(doc),
        'work': outcome.get('work'),
        'solverSeconds': doc.get('wall', {}).get('solverSeconds'),
        'wallSeconds': wall,
    }
    detail.update(jump_detail(doc))
    passed = (bool(rows)
              and detail['invalidPublications'] == 0
              and (detail['publishedDepthMm'] or 1e9) <= C168_W_MM)
    return verdict('C168', False, passed, detail)


def random_throw(out):
    doc, wall, status, err = lib.run(
        'randomt', 'mixed-61', f'{out}/randomt.json', target=C168_W_MM,
        budget=CELL_BUDGET, seed=0, jumps=8, checkpointevery=1)
    if status != 0:
        return verdict('random-T', False, False, {'exit': status, 'stderr': err})
    outcome = doc.get('outcome', {})
    incumbent = outcome.get('incumbent', {})
    rows = lib.published(doc)
    detail = {
        'exit': status,
        'lockedWmm': C168_W_MM,
        'jumpAllowance': 8,
        'entryRawPhi': doc.get('entry', {}).get('rawPhi'),
        'finalRawPhi': outcome.get('proxy', {}).get('rawPhi'),
        'finalMaxViolationMm': outcome.get('proxy', {}).get('maxViolationMm'),
        'census': outcome.get('census'),
        'publishedDepthMm': incumbent.get('rawSourceDepthMm'),
        'strictChild': not incumbent.get('fromConstructor'),
        'publications': len(rows),
        'exactCheckpoints': len(lib.checkpoints(doc)),
        'invalidPublications': lib.invalid_publications(doc),
        'work': outcome.get('work'),
        'solverSeconds': doc.get('wall', {}).get('solverSeconds'),
        'wallSeconds': wall,
    }
    detail.update(jump_detail(doc))
    passed = bool(rows) and detail['invalidPublications'] == 0
    return verdict('random-T', False, passed, detail)


CELLS = {
    's0': s0,
    's1': lambda out: perturbed('S1', 's1', out, True, 0.5, 2.0, CELL_BUDGET),
    's2': lambda out: perturbed('S2', 's2', out, False, 2.0, 10.0, CELL_BUDGET),
    'c175': c175,
    'triangle': triangle,
    'corpus': lambda out: corpus(out, CORPUS_STATES, True, 'corpus-1000'),
    'throughput': throughput,
    'c168': c168,
    'randomt': random_throw,
    'corpus10k': lambda out: corpus(out, HEAVY_CORPUS_STATES, False,
                                    'corpus-10000'),
}

FATAL_ORDER = ['s0', 's1', 'c175', 'triangle', 'corpus', 'throughput']
DIAGNOSTIC_ORDER = ['s2', 'c168', 'randomt', 'corpus10k']


def main():
    out = os.environ.get('ICS_OUT', lib.OUT)
    os.makedirs(out, exist_ok=True)
    wanted = sys.argv[1:] or (FATAL_ORDER + DIAGNOSTIC_ORDER)
    results = []
    for name in wanted:
        if name not in CELLS:
            raise SystemExit(f'unknown cell {name}')
        results.append(CELLS[name](out))
    fatal_failures = [row['cell'] for row in results
                      if row['fatal'] and not row['pass']]
    document = {
        'experiment': 'overlap-ics',
        'battery': 'gate-0-rerun',
        'binary': lib.BIN,
        'c175ProposalBudget': C175_BUDGET,
        'cellProposalBudget': CELL_BUDGET,
        'cells': results,
        'fatalFailures': fatal_failures,
        'GATE0_PASS': not fatal_failures,
    }
    print(json.dumps(document, indent=1))
    with open(f'{out}/gate0-rerun.json', 'w') as handle:
        json.dump(document, handle, indent=1)


if __name__ == '__main__':
    main()
