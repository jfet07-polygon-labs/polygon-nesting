#!/usr/bin/env python3
"""Mandatory Gate 0 for the conflict-cluster budget round.

Usage:
    python3 gate0.py <frozen-a6e5d1b-binary> [output-directory]

The frozen binary must be supplied. A runner that rebuilds its own "before"
cannot prove cross-binary identity. No quality arm is run by this script.
"""

import hashlib
import json
import os
import statistics
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, '..', '..', '..', '..'))
REQUEST = os.path.join(
    ROOT, 'tests', 'fixtures', 'mixed-61',
    'mixed61-request-exact-clearance.json')
DEFAULT_NEW = os.path.join(
    ROOT, 'target', 'release', 'examples', 'overlap_ics_benchmark')
SPEC = os.path.join(ROOT, 'docs', 'conflict-cluster-budget-spec.md')
SPEC_SHA256 = '0cfdf0e2557967e5aab3a48534e4ff6508c38b3d1054344360aedd61ce284ce9'

IDENTITY_CELLS = {
    'A': ['--cell=cutclose', '--mode=fixed', '--bites=8', '--attempts=1',
          '--iters=400', '--compressbites=0', '--workers=8', '--seed=0'],
    'B': ['--cell=cutclose', '--mode=fixed', '--bites=21', '--attempts=1',
          '--iters=400', '--compressbites=0', '--workers=8', '--seed=0'],
    'C': ['--cell=cutclose', '--mode=fixed', '--bites=21', '--attempts=1',
          '--iters=400', '--compressbites=0', '--workers=5', '--seed=5'],
    'D': ['--cell=cutclose', '--mode=fixed', '--bites=3', '--attempts=2',
          '--iters=120', '--compressbites=2', '--workers=8', '--seed=0',
          '--fingerprints=1'],
}


def sha256(path):
    with open(path, 'rb') as handle:
        return hashlib.sha256(handle.read()).hexdigest()


def loadavg():
    with open('/proc/loadavg') as handle:
        return [float(value) for value in handle.read().split()[:3]]


def canonical_digest(document):
    payload = json.dumps(document, sort_keys=True, separators=(',', ':'))
    return hashlib.sha256(payload.encode()).hexdigest()


def stripped_identity(document):
    if document is None:
        return None
    return {key: value for key, value in document.items()
            if key not in ('wall', 'executableSha256', 'buildFeatures')}


def stripped_wall(document):
    if document is None:
        return None
    return {key: value for key, value in document.items() if key != 'wall'}


def run(binary, argv, path):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    command = ([binary, f'--request={REQUEST}', '--edge=5', '--pair=5']
               + argv)
    started = time.monotonic()
    with open(path, 'w') as stdout:
        process = subprocess.run(
            command, stdout=stdout, stderr=subprocess.PIPE, check=False)
    elapsed = time.monotonic() - started
    try:
        with open(path) as handle:
            document = json.load(handle)
    except (OSError, json.JSONDecodeError):
        document = None
    return {
        'command': command,
        'exit': process.returncode,
        'processSeconds': elapsed,
        'stderrTail': (process.stderr or b'').decode(errors='replace')[-1200:],
        'sourcePath': path,
        'sourceSha256': sha256(path),
        'document': document,
    }


def run_test(command, path):
    started = time.monotonic()
    process = subprocess.run(
        command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        check=False)
    with open(path, 'wb') as handle:
        handle.write(process.stdout)
    return {
        'command': command,
        'exit': process.returncode,
        'seconds': time.monotonic() - started,
        'logPath': path,
        'logSha256': sha256(path),
    }


def partition(document):
    return ((document or {}).get('outcome') or {}).get('partition')


def publications_revalidated(document):
    outcome = (document or {}).get('outcome') or {}
    if outcome.get('invalidPublications') != 0:
        return False
    for row in outcome.get('publications') or []:
        revalidation = row.get('revalidation')
        if revalidation is None or not (
                revalidation.get('depthMatchesBitwise')
                and revalidation.get('fingerprintMatches')):
            return False
    return True


def partition_floor(document):
    part = partition(document) or {}
    request = (document or {}).get('request') or {}
    outcome = (document or {}).get('outcome') or {}
    decisions = part.get('partitionDecisions') or 0
    work = outcome.get('work') or {}
    return bool(
        decisions > 0
        and part.get('slotIdentitiesHold') is True
        and part.get('planIdentityFailureDecisions') == 0
        and part.get('executionIdentityFailureDecisions') == 0
        and part.get('invalidFallbackDecisions') == 0
        and part.get('partitionSlots') == part.get('executedSlots')
        and part.get('executedSlots')
        == (part.get('fullRelocateSlots') or 0)
        + (part.get('zeroEnergySlots') or 0)
        and work.get('pieceProposals')
        == request.get('pieceCount') * decisions
        and publications_revalidated(document))


def vector_clauses(document):
    vector = (document or {}).get('partitionVectors') or {}
    return {
        'unitSquare': vector.get('unitSquare')
        == {'center': [0.5, 0.5], 'radius': 0.5},
        'transformedDisc': (vector.get('transformedDisc') or {}).get('center')
        == [9.5, 4.5]
        and (vector.get('transformedDisc') or {}).get('radius') == 0.5,
        'purePairInversion': vector.get('pairInversion') == {
            'kind': 'pure-frozen-row-field-vector',
            'callsMeasurePair': False,
            'massTermsMm2': [1.0, 2.25],
            'massQuotas': [1, 3],
            'maxViolationWeightsMm': [2.0, 1.0],
            'maxViolationQuotas': [3, 1],
        },
        'boundaryV3': vector.get('boundaryV3TermMm2') == 9.0,
        'largestRemainder': vector.get('largestRemainder')
        == {'componentIds': [0, 3, 7], 'quotas': [3, 2, 0]},
        'mixedZeros': vector.get('mixedZeroQuotas') == [0, 5, 0],
        'zeroSignal': vector.get('zeroSignalQuotas') == [3, 1, 1]
        and vector.get('zeroSignalFallback') == 'zero-signal',
        'placebo': vector.get('placebo') == {
            'offset': 1,
            'input': [1.0, 2.0, 3.0, 4.0],
            'rotated': [2.0, 3.0, 4.0, 1.0],
            'quotas': [1, 1, 2, 0],
            'multisetPreserved': True,
            'nonIdentity': True,
        },
        'memberPermutation': vector.get('memberPermutation') == [6, 4, 3, 5],
        'roundRobin': vector.get('roundRobinSchedule') == [1, 2, 0, 1],
        'invalidFallback': vector.get('invalidQuotas') == [1, 1]
        and vector.get('invalidFallback') == 'invalid'
        and vector.get('nonfiniteSourceRejected') is True
        and vector.get('nonfinitePairRejected') is True,
        'fourAccountingIdentities': all(
            (vector.get('accountingIdentities') or {}).values())
        and len(vector.get('accountingIdentities') or {}) == 4,
    }


def main():
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    frozen = os.path.abspath(sys.argv[1])
    out = (os.path.abspath(sys.argv[2]) if len(sys.argv) > 2 else
           '/var/lib/t3/tmp/overlapics/conflict-cluster-budget-gate0')
    candidate = os.path.abspath(os.environ.get('ICS_CCB_BIN', DEFAULT_NEW))
    os.makedirs(os.path.join(out, 'cells'), exist_ok=True)
    if not os.path.isfile(frozen) or not os.access(frozen, os.X_OK):
        raise SystemExit(f'frozen binary is not executable: {frozen}')
    if not os.path.isfile(candidate) or not os.access(candidate, os.X_OK):
        raise SystemExit(f'candidate binary is not executable: {candidate}')

    document = {
        'experiment': 'overlap-ics',
        'battery': 'conflict-cluster-budget-gate0',
        'specPath': SPEC,
        'specSha256': sha256(SPEC),
        'expectedSpecSha256': SPEC_SHA256,
        'frozenCommit': 'a6e5d1b13b14b3b776d48d7f3298af5980fb762d',
        'frozenBinary': frozen,
        'frozenBinarySha256': sha256(frozen),
        'candidateBinary': candidate,
        'candidateBinarySha256': sha256(candidate),
        'request': REQUEST,
        'requestSha256': sha256(REQUEST),
        'machine': {'cpus': os.cpu_count(), 'loadBefore': loadavg()},
    }
    clauses = {
        'specDigest': document['specSha256'] == SPEC_SHA256,
        'quietBoxAtStart': document['machine']['loadBefore'][0] < 1.0,
    }

    # G0.1: exact recursive document identity after the only three removals.
    identity = []
    for name, argv in IDENTITY_CELLS.items():
        print(f'G0.1 identity cell {name}', file=sys.stderr, flush=True)
        base = run(frozen, argv, os.path.join(out, 'cells', f'identity-{name}-base.json'))
        new = run(candidate, argv + ['--partition=off'],
                  os.path.join(out, 'cells', f'identity-{name}-new-off.json'))
        base_doc = stripped_identity(base['document'])
        new_doc = stripped_identity(new['document'])
        row = {
            'cell': name,
            'argv': argv,
            'baseExit': base['exit'],
            'candidateExit': new['exit'],
            'baseSourcePath': base['sourcePath'],
            'baseSourceSha256': base['sourceSha256'],
            'candidateSourcePath': new['sourcePath'],
            'candidateSourceSha256': new['sourceSha256'],
            'baseDigestStripped': (canonical_digest(base_doc)
                                   if base_doc is not None else None),
            'candidateDigestStripped': (canonical_digest(new_doc)
                                        if new_doc is not None else None),
            'bitIdentical': base['exit'] == 0 and new['exit'] == 0
            and base_doc == new_doc,
            'stderrTail': base['stderrTail'] or new['stderrTail'],
        }
        identity.append(row)
    document['g01OffIdentity'] = identity
    clauses['g01OffIdentity'] = all(row['bitIdentical'] for row in identity)

    # G0.2: printed vectors plus the complete default/feature test corpora.
    print('G0.2 exact vectors and test corpora', file=sys.stderr, flush=True)
    vector_run = run(
        candidate, ['--cell=partition-vectors', '--partition=off'],
        os.path.join(out, 'cells', 'exact-vectors.json'))
    vector_checks = vector_clauses(vector_run['document'])
    tests = {
        'feature': run_test([
            'cargo', 'test', '-q', '-p', 'polygon-nesting-core',
            '--features', 'overlap-ics,conflict-cluster-budget', '--lib'],
            os.path.join(out, 'feature-tests.log')),
        'default': run_test([
            'cargo', 'test', '-q', '-p', 'polygon-nesting-core',
            '--features', 'overlap-ics', '--lib'],
            os.path.join(out, 'default-tests.log')),
    }
    document['g02ExactVectors'] = {
        'exit': vector_run['exit'],
        'sourcePath': vector_run['sourcePath'],
        'sourceSha256': vector_run['sourceSha256'],
        'clauses': vector_checks,
        'tests': tests,
    }
    clauses['g02ExactVectors'] = vector_run['exit'] == 0 \
        and all(vector_checks.values()) \
        and all(row['exit'] == 0 for row in tests.values())

    # G0.3: every reached Shadow decision, no sampling.
    shadow = []
    spearman_series = []
    total_eligible = 0
    total_disagreements = 0
    for seed in range(9):
        print(f'G0.3 shadow seed {seed}', file=sys.stderr, flush=True)
        argv = [
            '--cell=cutclose', '--mode=fixed', '--bites=1', '--attempts=1',
            '--iters=400', '--compressbites=0', '--workers=8',
            f'--seed={seed}', '--orders=1', '--arm=control',
            '--partition=shadow', '--revalidate=1']
        result = run(candidate, argv,
                     os.path.join(out, 'cells', f'shadow-seed{seed}.json'))
        part = partition(result['document']) or {}
        decisions = part.get('decisions') or []
        eligible = [row for row in decisions
                    if len(row.get('components') or []) >= 2]
        disagreements = [row for row in eligible
                         if row.get('massDiffersFromMaxViolation')]
        for row in eligible:
            spearman_series.append({
                'seed': row.get('seed'),
                'bite': row.get('bite'),
                'iteration': row.get('iteration'),
                'worker': row.get('worker'),
                'field': row.get('spearmanFieldMassMaxViolation'),
                'quota': row.get('spearmanQuotaMassMaxViolation'),
            })
        total_eligible += len(eligible)
        total_disagreements += len(disagreements)
        complete = len(decisions) == (part.get('partitionDecisions') or 0)
        detailed = all(
            len(row.get('massBits') or []) == len(row.get('components') or [])
            and len(row.get('maxViolationBits') or [])
            == len(row.get('components') or [])
            and row.get('planIdentitiesHold') is True
            for row in decisions)
        row = {
            'seed': seed,
            'exit': result['exit'],
            'sourcePath': result['sourcePath'],
            'sourceSha256': result['sourceSha256'],
            'partitionDecisions': part.get('partitionDecisions'),
            'eligibleDecisions': len(eligible),
            'eligibleDisagreements': len(disagreements),
            'reportedEligibleDecisions': part.get('eligibleDecisions'),
            'reportedEligibleDisagreements':
                part.get('eligibleDisagreementDecisions'),
            'reportedDisagreementRate': part.get('eligibleDisagreementRate'),
            'completeDecisionRecord': complete,
            'completeMassAllocationBits': detailed,
            'partitionFloor': partition_floor(result['document']),
        }
        row['pass'] = bool(
            row['exit'] == 0 and complete and detailed and row['partitionFloor']
            and row['eligibleDecisions'] == row['reportedEligibleDecisions']
            and row['eligibleDisagreements']
            == row['reportedEligibleDisagreements'])
        shadow.append(row)
    disagreement_rate = (total_disagreements / total_eligible
                          if total_eligible else None)
    document['g03ShadowEngagement'] = {
        'seeds': shadow,
        'eligibleDecisions': total_eligible,
        'eligibleDisagreementDecisions': total_disagreements,
        'completeDisagreementRate': disagreement_rate,
        'spearmanSeries': spearman_series,
        'selectedSeedGate': False,
        'correlationGate': False,
    }
    clauses['g03ShadowEngagement'] = bool(
        all(row['pass'] for row in shadow)
        and total_eligible > 0 and total_disagreements > 0)

    # G0.4: five fresh alternating AB/BA pairs, exact 32/256 schedule.
    costs = []
    for pair in range(5):
        sequence = 'AB' if pair % 2 == 0 else 'BA'
        print(f'G0.4 cost pair {pair} ({sequence})', file=sys.stderr, flush=True)
        result = run(candidate, [
            '--cell=partition-cost', '--partition=off', '--seed=0',
            '--warmups=32', '--measured=256',
            f'--sequence={sequence}'],
            os.path.join(out, 'cells', f'cost-p{pair}-{sequence}.json'))
        cost = ((result['document'] or {}).get('partitionCost') or {})
        identities = [
            'poseIdentity', 'orderIdentity', 'workIdentity',
            'actualSlotsIdentity', 'offActualMatchesExpected',
            'computeActualMatchesExpected',
            'computePartitionSlotsMatchActual', 'legacyProposalIdentity',
            'computeSlotIdentitiesHold']
        row = {
            'pair': pair,
            'sequence': sequence,
            'exit': result['exit'],
            'sourcePath': result['sourcePath'],
            'sourceSha256': result['sourceSha256'],
            'ratioComputeIgnoreOverOff':
                cost.get('ratioComputeIgnoreOverOff'),
            'identities': {key: cost.get(key) for key in identities},
            'computeInvalidFallbacks': cost.get('computeInvalidFallbacks'),
        }
        row['pass'] = bool(
            row['exit'] == 0 and all(row['identities'].values())
            and row['computeInvalidFallbacks'] == 0
            and row['ratioComputeIgnoreOverOff'] is not None)
        costs.append(row)
    ratios = [row['ratioComputeIgnoreOverOff'] for row in costs
              if row['ratioComputeIgnoreOverOff'] is not None]
    median_ratio = statistics.median(ratios) if len(ratios) == 5 else None
    document['g04ComputeIgnoreCost'] = {
        'pairs': costs,
        'ratios': ratios,
        'medianRatioComputeIgnoreOverOff': median_ratio,
        'threshold': 0.95,
    }
    clauses['g04ComputeIgnoreCost'] = bool(
        all(row['pass'] for row in costs)
        and median_ratio is not None and median_ratio >= 0.95)

    # G0.5: consume B/C/D directly and replay B in fresh processes.
    direct = []
    for arm in ('mass', 'shuffled-mass', 'max-violation'):
        print(f'G0.5 direct arm {arm}', file=sys.stderr, flush=True)
        result = run(candidate, [
            '--cell=cutclose', '--mode=fixed', '--bites=1', '--attempts=1',
            '--iters=1', '--compressbites=0', '--workers=8', '--seed=0',
            '--orders=1', '--arm=control', f'--partition={arm}',
            '--revalidate=1'],
            os.path.join(out, 'cells', f'direct-{arm}.json'))
        part = partition(result['document']) or {}
        direct.append({
            'arm': arm,
            'exit': result['exit'],
            'sourcePath': result['sourcePath'],
            'sourceSha256': result['sourceSha256'],
            'graphDigestSha256': part.get('graphDigestSha256'),
            'allocationDigestSha256': part.get('allocationDigestSha256'),
            'scheduleDigestSha256': part.get('scheduleDigestSha256'),
            'partitionFloor': partition_floor(result['document']),
        })
    replay_argv = [
        '--cell=cutclose', '--mode=fixed', '--bites=1', '--attempts=1',
        '--iters=8', '--compressbites=0', '--workers=8', '--seed=0',
        '--orders=1', '--arm=control', '--partition=mass',
        '--fingerprints=1', '--revalidate=1']
    print('G0.5 deterministic replay process 1', file=sys.stderr, flush=True)
    replay_a = run(candidate, replay_argv,
                   os.path.join(out, 'cells', 'determinism-mass-p1.json'))
    print('G0.5 deterministic replay process 2', file=sys.stderr, flush=True)
    replay_b = run(candidate, replay_argv,
                   os.path.join(out, 'cells', 'determinism-mass-p2.json'))
    deterministic = bool(
        replay_a['exit'] == 0 and replay_b['exit'] == 0
        and stripped_wall(replay_a['document'])
        == stripped_wall(replay_b['document'])
        and partition_floor(replay_a['document'])
        and partition_floor(replay_b['document']))
    document['g05AuthorityAccountingDeterminism'] = {
        'directArms': direct,
        'directAllPass': all(row['exit'] == 0 and row['partitionFloor']
                             for row in direct),
        'replay': {
            'process1Path': replay_a['sourcePath'],
            'process1Sha256': replay_a['sourceSha256'],
            'process2Path': replay_b['sourcePath'],
            'process2Sha256': replay_b['sourceSha256'],
            'bitIdenticalAfterRemovingOnlyWall': deterministic,
        },
        'implementationReview':
            'docs/conflict-cluster-budget-implementation-review.md',
    }
    clauses['g05AuthorityAccountingDeterminism'] = bool(
        document['g05AuthorityAccountingDeterminism']['directAllPass']
        and deterministic)

    document['machine']['loadAfter'] = loadavg()
    document['candidateBinarySha256After'] = sha256(candidate)
    document['binaryUnchangedDuringGate0'] = (
        document['candidateBinarySha256']
        == document['candidateBinarySha256After'])
    clauses['binaryUnchangedDuringGate0'] = document['binaryUnchangedDuringGate0']
    document['clauses'] = clauses
    document['GATE0_PASS'] = all(clauses.values())
    aggregate = os.path.join(out, 'gate0.json')
    with open(aggregate, 'w') as handle:
        json.dump(document, handle, indent=1)
    print(json.dumps({
        'GATE0_PASS': document['GATE0_PASS'],
        'clauses': clauses,
        'eligibleDecisions': total_eligible,
        'eligibleDisagreements': total_disagreements,
        'completeDisagreementRate': disagreement_rate,
        'medianComputeIgnoreOverOff': median_ratio,
        'aggregate': aggregate,
    }, indent=1))
    return 0 if document['GATE0_PASS'] else 1


if __name__ == '__main__':
    sys.exit(main())
