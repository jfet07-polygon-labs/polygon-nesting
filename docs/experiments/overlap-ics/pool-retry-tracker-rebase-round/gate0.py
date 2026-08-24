#!/usr/bin/env python3
"""One-shot, artifact-first Gate 0 for Pool-Retry Tracker Rebase.

Usage:
    python3 gate0.py <frozen-b1235a1-binary> <reviewed-source-commit> \
        <build-receipt.json> [new-output-dir]

The candidate defaults to target/release/examples/overlap_ics_benchmark and
may be overridden with ICS_POOL_REBASE_BIN. This script never builds, retries
a cell, overwrites an artifact, or runs Primary30. Each seed has one prefix
producer and two read-only, byte-identical checkpoint copies consumed by fresh
Saved and Rebase processes. Seed 0's same canonical artifact is reused by G0.3
and G0.4.
"""

import copy
import hashlib
import json
import os
import shutil
import statistics
import subprocess
import sys
import time


HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, '..', '..', '..', '..'))
REQUEST = os.path.join(
    ROOT, 'tests', 'fixtures', 'mixed-61',
    'mixed61-request-exact-clearance.json')
SPEC = os.path.join(ROOT, 'docs', 'pool-retry-tracker-rebase-spec.md')
SPEC_SHA256 = 'b5038979351bf2fc114a1d7a220751f0704e362e2dd62809632dceca9245a3a1'
FROZEN_COMMIT = 'b1235a11cf4a57d7437accbfc2348a05692fe0be'
SOURCE_PLAN = os.path.join(
    ROOT, 'docs', 'experiments', 'overlap-ics',
    'minimum-conflict-binary-close-round', 'evidence',
    'plan-f100-mbc.icscal.json')
DEFAULT_CANDIDATE = os.path.join(
    ROOT, 'target', 'release', 'examples', 'overlap_ics_benchmark')
FEATURES = ['overlap-ics', 'pool-retry-tracker-rebase']
PLAN_SECONDS = 27.67205079595
RETRY_CAP = 400
ALLOWED_STOPS = {'published', 'refused', 'struck', 'deadline', 'work-cap'}

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


def sha256_bytes(value):
    return hashlib.sha256(value).hexdigest()


def canonical_digest(value):
    payload = json.dumps(value, sort_keys=True, separators=(',', ':'))
    return hashlib.sha256(payload.encode()).hexdigest()


def git(*args):
    return subprocess.check_output(
        ['git', *args], cwd=ROOT, text=True).strip()


def load_snapshot():
    with open('/proc/loadavg', encoding='utf-8') as handle:
        fields = handle.read().split()
    running, total = [int(value) for value in fields[3].split('/')]
    return {
        'load1': float(fields[0]),
        'load5': float(fields[1]),
        'load15': float(fields[2]),
        'runningTasks': running,
        'totalTasks': total,
    }


def cpu_totals():
    with open('/proc/stat', encoding='utf-8') as handle:
        values = [int(value) for value in handle.readline().split()[1:]]
    idle = values[3] + values[4]
    return sum(values), idle


def quiet_snapshot():
    total_a, idle_a = cpu_totals()
    time.sleep(0.25)
    total_b, idle_b = cpu_totals()
    elapsed = total_b - total_a
    busy = 1.0 if elapsed <= 0 else 1.0 - (idle_b - idle_a) / elapsed
    load = load_snapshot()
    load['sampleSeconds'] = 0.25
    load['cpuBusyFraction'] = busy
    load['quiet'] = busy <= 0.05 and load['runningTasks'] <= 2
    return load


def run(binary, argv, path):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    if os.path.exists(path):
        raise RuntimeError(f'refusing to overwrite cell output: {path}')
    command = ([binary, f'--request={REQUEST}', '--edge=5', '--pair=5']
               + argv)
    started = time.monotonic()
    with open(path, 'x', encoding='utf-8') as stdout:
        process = subprocess.run(
            command, stdout=stdout, stderr=subprocess.PIPE, check=False)
    try:
        with open(path, encoding='utf-8') as handle:
            document = json.load(handle)
    except (OSError, json.JSONDecodeError):
        document = None
    return {
        'command': command,
        'exit': process.returncode,
        'processSeconds': time.monotonic() - started,
        'stderrTail': (process.stderr or b'').decode(errors='replace')[-2400:],
        'sourcePath': path,
        'sourceSha256': sha256(path),
        'document': document,
    }


def run_test(command, path):
    if os.path.exists(path):
        raise RuntimeError(f'refusing to overwrite test log: {path}')
    started = time.monotonic()
    process = subprocess.run(
        command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        check=False)
    with open(path, 'xb') as handle:
        handle.write(process.stdout)
    return {
        'command': command,
        'exit': process.returncode,
        'seconds': time.monotonic() - started,
        'logPath': path,
        'logSha256': sha256(path),
    }


def clone_plan(candidate_sha, path):
    with open(SOURCE_PLAN, encoding='utf-8') as handle:
        plan = json.load(handle)
    plan['key']['binaryKey'] = {
        'executableSha256': candidate_sha,
        'features': FEATURES,
    }
    plan['provenance'] = (
        'Pool-Retry Tracker Rebase Gate 0: exact deterministic-30s rates and '
        'factor cloned without recalibration; only binary key, feature '
        'provenance, and round identity changed.')
    with open(path, 'x', encoding='utf-8') as handle:
        json.dump(plan, handle, indent=1)
        handle.write('\n')
    return plan


def strip_g01(document):
    value = copy.deepcopy(document)
    for key in ('wall', 'executableSha256', 'buildFeatures'):
        value.pop(key, None)
    return value


def strip_g03(document):
    value = copy.deepcopy(document)
    for key in ('wall', 'executableSha256', 'buildFeatures'):
        value.pop(key, None)
    schedule = value.get('schedule') or {}
    schedule.pop('poolRetryArm', None)
    trace = (value.get('outcome') or {}).get('poolRetryRebase') or {}
    trace.pop('arm', None)
    for row in trace.get('decisions') or []:
        row.pop('resetWeights', None)
        row.pop('pathSeconds', None)
    return value


def strip_wall(document):
    value = copy.deepcopy(document)
    value.pop('wall', None)
    return value


def first_retry(document):
    trace = ((document or {}).get('outcome') or {}).get('poolRetryRebase') or {}
    rows = trace.get('decisions') or []
    return rows[0] if len(rows) == 1 else None


def work_reconciles(row):
    before = (row or {}).get('retryWorkBefore') or {}
    after = (row or {}).get('retryWorkAfter') or {}
    delta = (row or {}).get('pathWork') or {}
    return bool(before and before.keys() == after.keys() == delta.keys()
                and all(after[key] >= before[key]
                        and after[key] - before[key] == delta[key]
                        for key in before))


def pacer_reconciles(value):
    return bool(
        value and value.get('chargeIdentityHolds') is True
        and value.get('consumedUnitsMatchCharged') is True
        and value.get('currencyVersion') == 'U0-sample-evaluations')


def publications_revalidated(document):
    outcome = (document or {}).get('outcome') or {}
    publications = outcome.get('publications') or []
    if (outcome.get('invalidPublications') != 0
            or outcome.get('independentInvalidPublications') != 0
            or outcome.get('publicationCount') != len(publications)):
        return False
    for publication in publications:
        check = publication.get('revalidation') or {}
        kernel = check.get('exclusiveKernel') or {}
        contract = check.get('contract') or {}
        if not (
            publication.get('improvedIncumbent') is True
            and check.get('depthMatchesBitwise') is True
            and check.get('fingerprintMatches') is True
            and check.get('allAuthoritiesValid') is True
            and kernel.get('mode') == 'exclusive'
            and kernel.get('radiusMm') == 2.5
            and kernel.get('twoRMicron') == 5000
            and kernel.get('searchOffsetAllowanceMm') == 0.0
            and kernel.get('valid') is True
            and contract.get('valid') is True):
            return False
    return True


def retry_complete(document, row, timed):
    if not row:
        return False
    outcome = (document or {}).get('outcome') or {}
    trace = outcome.get('poolRetryRebase') or {}
    bites = outcome.get('bites') or []
    terminal_bite = bites[-1] if bites else {}
    proxy = outcome.get('proxy') or {}
    census = outcome.get('census') or {}
    key = row.get('key') or {}
    fingerprints = row.get('downstreamFingerprints') or []
    checkpoints = row.get('retryExactCheckpoints') or []
    band = row.get('retryBand') or {}
    strike = row.get('retryStrike') or {}
    disruption = row.get('disruption') or {}
    path_seconds = row.get('pathSeconds')
    publication = row.get('retryPublication')
    digests = [
        row.get('selectedPoseDigestSha256'),
        row.get('postInstallPoseDigestSha256'),
        row.get('postInstallRawRowDigestSha256'),
        disruption.get('poseTransformDigestSha256'),
        row.get('postDisruptionPoseDigestSha256'),
        row.get('postDisruptionRawRowDigestSha256'),
        row.get('coldPostDisruptionRawRowDigestSha256'),
    ]
    strike_fields = (
        'strikes', 'batches', 'chargedWorkSampleEvaluations',
        'substantial', 'marginal', 'none', 'strikeAccumulated',
        'strikeOvershoot')
    return bool(
        trace.get('invalidRetries') == 0
        and row.get('valid') is True
        and not row.get('failureReasons')
        and isinstance(key.get('requestSeed'), int)
        and isinstance(key.get('exploreBiteOrdinal'), int)
        and isinstance(key.get('attemptOrdinal'), int)
        and key['attemptOrdinal'] > 0
        and isinstance(row.get('widthBits'), int)
        and isinstance(row.get('poolLength'), int)
        and row['poolLength'] > 0
        and isinstance(row.get('selectedRank'), int)
        and 0 <= row['selectedRank'] < row['poolLength']
        and isinstance(row.get('poolEntryRawPhiBits'), int)
        and all(isinstance(value, str) and len(value) == 64
                for value in digests)
        and disruption.get('fired') is True
        and isinstance(disruption.get('swapped'), list)
        and len(disruption['swapped']) == 2
        and isinstance(disruption.get('distinct'), bool)
        and isinstance(disruption.get('followers'), list)
        and isinstance(disruption.get('followersCapped'), int)
        and isinstance(disruption.get('work'), dict)
        and row.get('incrementalColdRawRowsIdentical') is True
        and isinstance(row.get('retryIterations'), int)
        and row['retryIterations'] <= RETRY_CAP
        and row.get('retryStop') in ALLOWED_STOPS
        and len(fingerprints) == row['retryIterations']
        and row.get('fingerprintEnd', -1) - row.get('fingerprintStart', 0)
            == row['retryIterations']
        and all(item.get('committedPoseDigestSha256') for item in fingerprints)
        and all(isinstance(strike.get(field), int)
                and strike[field] >= 0 for field in strike_fields)
        and isinstance(band.get('minimumRawPhi'), (int, float))
        and isinstance(band.get('reached'), bool)
        and band.get('exactCheckpointCalls') == len(checkpoints)
        and isinstance(band.get('entries'), int)
        and band['entries'] >= band['exactCheckpointCalls']
        and all(isinstance(item.get('targetDepthBits'), int)
                and isinstance(item.get('kernelExclusiveValid'), bool)
                and isinstance(item.get('contractValid'), bool)
                for item in checkpoints)
        and work_reconciles(row)
        and pacer_reconciles(row.get('pacerBefore'))
        and pacer_reconciles(row.get('pacerAfter'))
        and isinstance(row.get('authorityParentFingerprint'), str)
        and len(row['authorityParentFingerprint']) == 64
        and row.get('retryPublished') == (publication is not None)
        and terminal_bite.get('published') == row.get('retryPublished')
        and terminal_bite.get('attempts') == key['attemptOrdinal']
            + int(not row.get('retryPublished'))
        and proxy.get('rawPhi') == 0.0
        and census.get('activePairRows') == 0
        and census.get('activeEdgeRows') == 0
        and (publication is None or (
            publication.get('phase') == 'explore'
            and publication.get('improvedIncumbent') is True
            and publication.get('parentFingerprint')
                == row['authorityParentFingerprint']))
        and (not row.get('retryPublished') or any(
            checkpoint.get('publishedRawDepthMm') is not None
            and checkpoint.get('kernelExclusiveValid') is True
            and checkpoint.get('contractValid') is True
            for checkpoint in checkpoints))
        and ((isinstance(path_seconds, (int, float)) and path_seconds > 0.0)
             if timed else path_seconds is None)
        and publications_revalidated(document))


def policy_boundary(row, arm):
    if not row:
        return False
    saved = row.get('savedWeights') or {}
    reset = row.get('resetWeights')
    post = row.get('postPolicyWeights') or {}
    disrupted = row.get('postDisruptionWeights') or {}
    common = (
        saved.get('allFinite') is True
        and saved.get('countAboveFloor', 0) > 0
        and disrupted.get('bits') == post.get('bits'))
    if arm == 'saved':
        return bool(common and reset is None and post.get('bits') == saved.get('bits'))
    if arm == 'rebase':
        return bool(
            common and reset and reset.get('allFinite') is True
            and reset.get('allExactlyOne') is True
            and post.get('allExactlyOne') is True
            and reset.get('bits') == post.get('bits'))
    if arm == 'compute-ignore':
        return bool(
            common and reset and reset.get('allExactlyOne') is True
            and post.get('bits') == saved.get('bits'))
    return False


def predecision(row):
    if not row:
        return None
    return {
        'key': row.get('key'),
        'widthBits': row.get('widthBits'),
        'poolLength': row.get('poolLength'),
        'selectedRank': row.get('selectedRank'),
        'poolEntryRawPhiBits': row.get('poolEntryRawPhiBits'),
        'selectedPoseDigestSha256': row.get('selectedPoseDigestSha256'),
        'savedWeights': row.get('savedWeights'),
        'postInstallPoseDigestSha256': row.get('postInstallPoseDigestSha256'),
        'postInstallRawRowDigestSha256': row.get('postInstallRawRowDigestSha256'),
        'disruption': row.get('disruption'),
        'postDisruptionPoseDigestSha256': row.get('postDisruptionPoseDigestSha256'),
        'postDisruptionRawRowDigestSha256': row.get('postDisruptionRawRowDigestSha256'),
        'coldPostDisruptionRawRowDigestSha256': row.get(
            'coldPostDisruptionRawRowDigestSha256'),
        'authorityParentFingerprint': row.get('authorityParentFingerprint'),
        'retryWorkBefore': row.get('retryWorkBefore'),
        'pacerBefore': row.get('pacerBefore'),
    }


def causal_decisions(row):
    return [
        (item.get('winner'), item.get('committedPoseDigestSha256'))
        for item in ((row or {}).get('downstreamFingerprints') or [])
    ]


def checkpoint_copy(source, destination):
    if os.path.exists(destination):
        raise RuntimeError(f'refusing to overwrite checkpoint copy: {destination}')
    shutil.copyfile(source, destination)
    os.chmod(destination, 0o444)
    return {
        'path': destination,
        'sha256': sha256(destination),
        'byteLength': os.path.getsize(destination),
        'readOnly': (os.stat(destination).st_mode & 0o222) == 0,
    }


def producer_args(seed, plan, checkpoint, reviewed):
    return [
        '--cell=cutclose', '--mode=calibrated', f'--plan={plan}',
        f'--wall={PLAN_SECONDS}', '--workers=8', '--orders=1',
        '--arm=control', f'--seed={seed}', '--fingerprints=1',
        f'--checkpointout={checkpoint}', f'--specsha={SPEC_SHA256}',
        f'--sourcecommit={reviewed}',
    ]


def resume_args(seed, arm, plan, checkpoint, reviewed, timed=False):
    values = [
        '--cell=pool-rebase-resume', f'--plan={plan}',
        f'--checkpointin={checkpoint}', '--workers=8', '--orders=1',
        '--arm=control', f'--seed={seed}', f'--poolrebase={arm}',
        '--fingerprints=1', '--revalidate=1', f'--specsha={SPEC_SHA256}',
        f'--sourcecommit={reviewed}',
    ]
    if timed:
        values.append('--poolrebasetiming=1')
    return values


def pair_row(seed, producer, artifact, saved_copy, rebase_copy,
             saved_run, rebase_run):
    saved_doc = saved_run['document']
    rebase_doc = rebase_run['document']
    saved = first_retry(saved_doc)
    rebase = first_retry(rebase_doc)
    saved_published = (saved or {}).get('retryPublished') is True
    rebase_published = (rebase or {}).get('retryPublished') is True
    saved_causal = causal_decisions(saved)
    rebase_causal = causal_decisions(rebase)
    aligned_causal_change = any(
        left != right for left, right in zip(saved_causal, rebase_causal))
    artifact_sha = sha256(artifact)
    producer_checkpoint = (producer.get('document') or {}).get('checkpoint') or {}
    saved_checkpoint = (saved_doc or {}).get('checkpoint') or {}
    rebase_checkpoint = (rebase_doc or {}).get('checkpoint') or {}
    prefix_equal = (
        producer_checkpoint.get('outputSha256') == artifact_sha
        and saved_copy['sha256'] == artifact_sha
        and rebase_copy['sha256'] == artifact_sha
        and saved_checkpoint.get('inputSha256') == artifact_sha
        and rebase_checkpoint.get('inputSha256') == artifact_sha
        and saved_checkpoint == rebase_checkpoint
        and saved_checkpoint.get('canonicalReencodeIdentical') is True)
    return {
        'seed': seed,
        'producer': producer,
        'canonicalCheckpoint': {
            'path': artifact,
            'sha256': artifact_sha,
            'byteLength': os.path.getsize(artifact),
        },
        'savedCopy': saved_copy,
        'rebaseCopy': rebase_copy,
        'savedPath': saved_run['sourcePath'],
        'rebasePath': rebase_run['sourcePath'],
        'exitsZero': producer['exit'] == saved_run['exit'] == rebase_run['exit'] == 0,
        'prefixArtifactBitIdentical': prefix_equal,
        'predecisionDigestSaved': canonical_digest(predecision(saved)),
        'predecisionDigestRebase': canonical_digest(predecision(rebase)),
        'predecisionBitIdentical': predecision(saved) == predecision(rebase),
        'savedPolicyValid': policy_boundary(saved, 'saved'),
        'rebasePolicyValid': policy_boundary(rebase, 'rebase'),
        'savedComplete': retry_complete(saved_doc, saved, timed=False),
        'rebaseComplete': retry_complete(rebase_doc, rebase, timed=False),
        'savedPublished': saved_published,
        'rebasePublished': rebase_published,
        'savedToRebasePublication': not saved_published and rebase_published,
        'reversePublication': saved_published and not rebase_published,
        'alignedCausalDecisionChanged': aligned_causal_change,
        'savedRetryIterations': (saved or {}).get('retryIterations'),
        'rebaseRetryIterations': (rebase or {}).get('retryIterations'),
        'savedStop': (saved or {}).get('retryStop'),
        'rebaseStop': (rebase or {}).get('retryStop'),
    }


def validate_receipt(receipt, receipt_path, candidate, reviewed):
    expected_command = [
        'cargo', 'build', '--release', '--locked', '-p', 'polygon-nesting-core',
        '--example', 'overlap_ics_benchmark', '--features', ','.join(FEATURES),
    ]
    return {
        'schema': receipt.get('schema')
            == 'pool-retry-tracker-rebase/build-receipt/v1',
        'reviewedCommit': receipt.get('reviewedSourceCommit') == reviewed,
        'headFrozen': receipt.get('headBefore') == reviewed
            and receipt.get('headAfter') == reviewed,
        'statusClean': receipt.get('sourceStatusBefore') == ''
            and receipt.get('sourceStatusAfter') == '',
        'sourceTree': receipt.get('sourceTree')
            == git('rev-parse', f'{reviewed}^{{tree}}'),
        'buildCommand': receipt.get('buildCommand') == expected_command,
        'features': receipt.get('features') == FEATURES,
        'binaryPath': os.path.realpath(receipt.get('binaryPath', ''))
            == os.path.realpath(candidate),
        'binarySha256': receipt.get('binarySha256') == sha256(candidate),
        'specSha256': receipt.get('specSha256') == sha256(SPEC),
        'requestSha256': receipt.get('requestSha256') == sha256(REQUEST),
        'sourcePlanSha256': receipt.get('sourcePlanSha256') == sha256(SOURCE_PLAN),
        'receiptSha256': len(sha256(receipt_path)) == 64,
    }


def write_result(document, clauses, candidate, frozen, out):
    document['candidateBinarySha256After'] = sha256(candidate)
    document['frozenBinarySha256After'] = sha256(frozen)
    clauses['candidateBinaryUnchanged'] = (
        document['candidateBinarySha256']
        == document['candidateBinarySha256After'])
    clauses['frozenBinaryUnchanged'] = (
        document['frozenBinarySha256']
        == document['frozenBinarySha256After'])
    document['sourceCommitAfter'] = git('rev-parse', 'HEAD')
    document['sourceStatusAfter'] = git('status', '--porcelain')
    clauses['reviewedSourceStillFrozen'] = (
        document['sourceCommitAfter'] == document['reviewedSourceCommit']
        and document['sourceStatusAfter'] == '')
    document['machine']['loadAfter'] = load_snapshot()
    document['clauses'] = clauses
    document['GATE0_PASS'] = all(clauses.values())
    aggregate = os.path.join(out, 'gate0.json')
    with open(aggregate, 'x', encoding='utf-8') as handle:
        json.dump(document, handle, indent=1)
        handle.write('\n')
    print(json.dumps({
        'GATE0_PASS': document['GATE0_PASS'],
        'clauses': clauses,
        'aggregate': aggregate,
    }, indent=1))
    return 0 if document['GATE0_PASS'] else 1


def stop_after(document, clauses, candidate, frozen, out, clause):
    if clauses.get(clause) is True:
        return None
    document['stoppedAfter'] = clause
    return write_result(document, clauses, candidate, frozen, out)


def main():
    if len(sys.argv) not in (4, 5):
        raise SystemExit(__doc__)
    frozen = os.path.abspath(sys.argv[1])
    reviewed = sys.argv[2]
    receipt_path = os.path.abspath(sys.argv[3])
    out = (os.path.abspath(sys.argv[4]) if len(sys.argv) == 5 else
           '/var/lib/t3/tmp/overlapics/pool-retry-tracker-rebase-gate0')
    candidate = os.path.abspath(
        os.environ.get('ICS_POOL_REBASE_BIN', DEFAULT_CANDIDATE))
    if os.path.exists(out):
        raise SystemExit(f'one-shot output directory already exists: {out}')
    os.makedirs(out)
    cells = os.path.join(out, 'cells')
    checkpoints = os.path.join(out, 'checkpoints')
    os.makedirs(cells)
    os.makedirs(checkpoints)
    for label, binary in (('frozen', frozen), ('candidate', candidate)):
        if not os.path.isfile(binary) or not os.access(binary, os.X_OK):
            raise SystemExit(f'{label} binary is not executable: {binary}')
    with open(receipt_path, encoding='utf-8') as handle:
        receipt = json.load(handle)

    head = git('rev-parse', 'HEAD')
    status = git('status', '--porcelain')
    document = {
        'experiment': 'pool-retry-tracker-rebase-gate0-artifact-first',
        'specPath': SPEC,
        'specSha256': sha256(SPEC),
        'expectedSpecSha256': SPEC_SHA256,
        'frozenCommit': FROZEN_COMMIT,
        'frozenBinary': frozen,
        'frozenBinarySha256': sha256(frozen),
        'candidateBinary': candidate,
        'candidateBinarySha256': sha256(candidate),
        'buildReceiptPath': receipt_path,
        'buildReceiptSha256': sha256(receipt_path),
        'buildReceipt': receipt,
        'reviewedSourceCommit': reviewed,
        'headSourceCommit': head,
        'sourceStatusAtStart': status,
        'reviewQuorum': ['Sol', 'Grok', 'ox-alpha'],
        'request': REQUEST,
        'requestSha256': sha256(REQUEST),
        'sourcePlan': SOURCE_PLAN,
        'sourcePlanSha256': sha256(SOURCE_PLAN),
        'machine': {'cpus': os.cpu_count(), 'loadBefore': load_snapshot()},
    }
    receipt_checks = validate_receipt(
        receipt, receipt_path, candidate, reviewed)
    document['buildReceiptChecks'] = receipt_checks
    clauses = {
        'specDigest': document['specSha256'] == SPEC_SHA256,
        'reviewedFrozenSource': reviewed == head and status == '',
        'buildReceiptBound': all(receipt_checks.values()),
    }
    if not all(clauses.values()):
        return write_result(document, clauses, candidate, frozen, out)

    identity = []
    for name, argv in IDENTITY_CELLS.items():
        base = run(frozen, argv, os.path.join(cells, f'g01-{name}-frozen.json'))
        saved = run(candidate, argv + ['--poolrebase=saved'],
                    os.path.join(cells, f'g01-{name}-saved.json'))
        identity.append({
            'cell': name,
            'baseExit': base['exit'],
            'savedExit': saved['exit'],
            'basePath': base['sourcePath'],
            'savedPath': saved['sourcePath'],
            'baseSha256': base['sourceSha256'],
            'savedSha256': saved['sourceSha256'],
            'documentIdenticalAfterExactStrip': bool(
                base['exit'] == 0 and saved['exit'] == 0
                and (base['document'] or {}).get('buildFeatures')
                    == ['overlap-ics']
                and (saved['document'] or {}).get('buildFeatures') == FEATURES
                and strip_g01(base['document']) == strip_g01(saved['document'])),
        })

    vector = run(candidate, ['--cell=pool-rebase-vectors', '--orders=1'],
                 os.path.join(cells, 'g01-vectors.json'))
    vector_doc = (vector['document'] or {}).get('poolRetryRebaseVectors') or {}
    lifecycle = vector_doc.get('lifecycle') or {}
    new_width = vector_doc.get('newWidthReset') or {}
    vector_pass = bool(
        vector['exit'] == 0
        and vector_doc.get('savedRestoredExactly') is True
        and vector_doc.get('rebaseAllExactlyOne') is True
        and vector_doc.get('computeIgnoreRestoredExactly') is True
        and vector_doc.get('rawRowsUnchanged') is True
        and vector_doc.get('nonfiniteSavedVisible') is True
        and vector_doc.get('nonfiniteLifecycleInvalid') is True
        and vector_doc.get('lifecycleDisruptionIdentical') is True
        and lifecycle.get('savedValid') is True
        and lifecycle.get('rebaseValid') is True
        and lifecycle.get('computeIgnoreValid') is True
        and new_width.get('valid') is True
        and (new_width.get('weights') or {}).get('allExactlyOne') is True)
    default_tests = run_test([
        'cargo', 'test', '--locked', '-p', 'polygon-nesting-core', '--lib', '--tests',
        '--features', 'overlap-ics'],
        os.path.join(out, 'g01-default-tests.log'))
    feature_tests = run_test([
        'cargo', 'test', '--locked', '-p', 'polygon-nesting-core', '--lib', '--tests',
        '--features', 'overlap-ics,pool-retry-tracker-rebase'],
        os.path.join(out, 'g01-feature-tests.log'))
    document['g01'] = {
        'identity': identity,
        'vector': vector,
        'vectorPass': vector_pass,
        'defaultTests': default_tests,
        'featureTests': feature_tests,
    }
    clauses['g01FeatureRuntimeVectorsTests'] = bool(
        all(row['documentIdenticalAfterExactStrip'] for row in identity)
        and vector_pass
        and default_tests['exit'] == 0
        and feature_tests['exit'] == 0)
    stopped = stop_after(
        document, clauses, candidate, frozen, out,
        'g01FeatureRuntimeVectorsTests')
    if stopped is not None:
        return stopped

    plan_path = os.path.join(out, 'plan-f100-pool-rebase.icscal.json')
    plan = clone_plan(document['candidateBinarySha256'], plan_path)
    document['gatePlan'] = {
        'path': plan_path,
        'sha256': sha256(plan_path),
        'document': plan,
    }

    pairs = []
    canonical_artifacts = {}
    for seed in range(9):
        artifact = os.path.join(checkpoints, f'seed{seed}-canonical.chk')
        producer = run(
            candidate, producer_args(seed, plan_path, artifact, reviewed),
            os.path.join(cells, f'g02-seed{seed}-producer.json'))
        if not os.path.isfile(artifact):
            pairs.append({
                'seed': seed, 'producer': producer, 'exitsZero': False,
                'prefixArtifactBitIdentical': False,
                'predecisionBitIdentical': False, 'savedPolicyValid': False,
                'rebasePolicyValid': False, 'savedComplete': False,
                'rebaseComplete': False, 'savedToRebasePublication': False,
                'reversePublication': False,
                'alignedCausalDecisionChanged': False,
            })
            break
        os.chmod(artifact, 0o444)
        canonical_artifacts[seed] = artifact
        saved_path = os.path.join(checkpoints, f'seed{seed}-saved.chk')
        rebase_path = os.path.join(checkpoints, f'seed{seed}-rebase.chk')
        saved_copy = checkpoint_copy(artifact, saved_path)
        rebase_copy = checkpoint_copy(artifact, rebase_path)
        saved = run(
            candidate,
            resume_args(seed, 'saved', plan_path, saved_path, reviewed),
            os.path.join(cells, f'g02-seed{seed}-saved.json'))
        rebase = run(
            candidate,
            resume_args(seed, 'rebase', plan_path, rebase_path, reviewed),
            os.path.join(cells, f'g02-seed{seed}-rebase.json'))
        pairs.append(pair_row(
            seed, producer, artifact, saved_copy, rebase_copy, saved, rebase))

    treatment_wins = [row for row in pairs if row['savedToRebasePublication']]
    reverses = [row for row in pairs if row['reversePublication']]
    causal_wins = [
        row for row in treatment_wins if row['alignedCausalDecisionChanged']]
    document['g02'] = {
        'pairs': pairs,
        'treatmentWinSeeds': [row['seed'] for row in treatment_wins],
        'reverseSeeds': [row['seed'] for row in reverses],
        'causalTreatmentWinSeeds': [row['seed'] for row in causal_wins],
    }
    clauses['g02NinePairedFirstRetries'] = bool(
        len(pairs) == 9
        and all(
            row['exitsZero'] and row['prefixArtifactBitIdentical']
            and row['predecisionBitIdentical'] and row['savedPolicyValid']
            and row['rebasePolicyValid'] and row['savedComplete']
            and row['rebaseComplete'] for row in pairs)
        and len(treatment_wins) >= 2 and not reverses and len(causal_wins) >= 2)
    stopped = stop_after(
        document, clauses, candidate, frozen, out,
        'g02NinePairedFirstRetries')
    if stopped is not None:
        return stopped

    g03_quiet = quiet_snapshot()
    document['g03QuietImmediatelyBefore'] = g03_quiet
    clauses['g03QuietBox'] = g03_quiet['quiet']
    stopped = stop_after(document, clauses, candidate, frozen, out, 'g03QuietBox')
    if stopped is not None:
        return stopped

    seed0_artifact = canonical_artifacts[0]
    cost_pairs = []
    for index, order in enumerate(('AB', 'BA', 'AB', 'BA', 'AB')):
        arms = ('saved', 'compute-ignore') if order == 'AB' else (
            'compute-ignore', 'saved')
        runs = {}
        copies = {}
        for arm in arms:
            checkpoint = os.path.join(
                checkpoints, f'g03-p{index}-{order}-{arm}.chk')
            copies[arm] = checkpoint_copy(seed0_artifact, checkpoint)
            runs[arm] = run(
                candidate,
                resume_args(0, arm, plan_path, checkpoint, reviewed, timed=True),
                os.path.join(cells, f'g03-p{index}-{order}-{arm}.json'))
        saved_row = first_retry(runs['saved']['document'])
        compute_row = first_retry(runs['compute-ignore']['document'])
        saved_seconds = (saved_row or {}).get('pathSeconds') or 0.0
        compute_seconds = (compute_row or {}).get('pathSeconds') or 0.0
        saved_samples = ((saved_row or {}).get('pathWork') or {}).get(
            'sampleEvaluations', 0)
        compute_samples = ((compute_row or {}).get('pathWork') or {}).get(
            'sampleEvaluations', 0)
        saved_rate = saved_samples / saved_seconds if saved_seconds > 0.0 else 0.0
        compute_rate = (
            compute_samples / compute_seconds if compute_seconds > 0.0 else 0.0)
        cost_pairs.append({
            'pair': index,
            'order': order,
            'copies': copies,
            'savedPath': runs['saved']['sourcePath'],
            'computeIgnorePath': runs['compute-ignore']['sourcePath'],
            'savedRate': saved_rate,
            'computeIgnoreRate': compute_rate,
            'ratio': compute_rate / saved_rate if saved_rate > 0.0 else 0.0,
            'savedComplete': retry_complete(
                runs['saved']['document'], saved_row, timed=True),
            'computeIgnoreComplete': retry_complete(
                runs['compute-ignore']['document'], compute_row, timed=True),
            'computeIgnorePolicyValid': policy_boundary(
                compute_row, 'compute-ignore'),
            'identityAfterExactStrip': bool(
                runs['saved']['exit'] == runs['compute-ignore']['exit'] == 0
                and copies['saved']['sha256'] == copies['compute-ignore']['sha256']
                    == sha256(seed0_artifact)
                and strip_g03(runs['saved']['document'])
                    == strip_g03(runs['compute-ignore']['document'])),
        })
    ratios = [row['ratio'] for row in cost_pairs]
    document['g03'] = {
        'pairs': cost_pairs,
        'medianComputeIgnoreOverSaved': statistics.median(ratios),
    }
    clauses['g03ComputeIgnoreCostIdentity'] = bool(
        all(
            row['identityAfterExactStrip'] and row['savedComplete']
            and row['computeIgnoreComplete'] and row['computeIgnorePolicyValid']
            for row in cost_pairs)
        and statistics.median(ratios) >= 0.95)
    stopped = stop_after(
        document, clauses, candidate, frozen, out,
        'g03ComputeIgnoreCostIdentity')
    if stopped is not None:
        return stopped

    replay_runs = []
    for label in ('a', 'b'):
        checkpoint = os.path.join(checkpoints, f'g04-seed0-rebase-{label}.chk')
        checkpoint_meta = checkpoint_copy(seed0_artifact, checkpoint)
        result = run(
            candidate,
            resume_args(0, 'rebase', plan_path, checkpoint, reviewed),
            os.path.join(cells, f'g04-seed0-rebase-{label}.json'))
        replay_runs.append((checkpoint_meta, result))
    replay_a = replay_runs[0][1]
    replay_b = replay_runs[1][1]
    replay_a_row = first_retry(replay_a['document'])
    replay_b_row = first_retry(replay_b['document'])

    diff_names = git('diff', '--name-only', FROZEN_COMMIT, reviewed).splitlines()
    dependency_diff = git(
        'diff', '--unified=0', FROZEN_COMMIT, reviewed, '--',
        'Cargo.lock', 'Cargo.toml', 'crates/polygon-nesting-core/Cargo.toml')
    added_dependency_lines = [
        line for line in dependency_diff.splitlines()
        if line.startswith('+') and not line.startswith('+++')
        and 'pool-retry-tracker-rebase = []' not in line]
    source_diff = subprocess.check_output(
        ['git', 'diff', '--binary', FROZEN_COMMIT, reviewed], cwd=ROOT)
    provenance = {
        'diffNames': diff_names,
        'diffSha256': sha256_bytes(source_diff),
        'cargoLockUnchanged': 'Cargo.lock' not in diff_names,
        'onlyFeatureAddedToDependencyManifests': not added_dependency_lines,
        'unexpectedAddedManifestLines': added_dependency_lines,
        'sparrowOrJaguaSourceFilesAdded': any(
            'sparrow' in name.lower() or 'jagua' in name.lower()
            for name in diff_names),
    }
    document['g04'] = {
        'firstCopy': replay_runs[0][0],
        'secondCopy': replay_runs[1][0],
        'firstPath': replay_a['sourcePath'],
        'secondPath': replay_b['sourcePath'],
        'firstSha256': replay_a['sourceSha256'],
        'secondSha256': replay_b['sourceSha256'],
        'deterministicAfterWallStrip':
            strip_wall(replay_a['document']) == strip_wall(replay_b['document']),
        'firstComplete': retry_complete(
            replay_a['document'], replay_a_row, timed=False),
        'secondComplete': retry_complete(
            replay_b['document'], replay_b_row, timed=False),
        'provenance': provenance,
    }
    clauses['g04DeterminismAuthorityProvenance'] = bool(
        replay_a['exit'] == replay_b['exit'] == 0
        and replay_runs[0][0]['sha256'] == replay_runs[1][0]['sha256']
            == sha256(seed0_artifact)
        and document['g04']['deterministicAfterWallStrip']
        and document['g04']['firstComplete'] and document['g04']['secondComplete']
        and (((replay_a['document'] or {}).get('outcome') or {})
             .get('poolRetryRebase') or {}).get('invalidRetries') == 0
        and (((replay_b['document'] or {}).get('outcome') or {})
             .get('poolRetryRebase') or {}).get('invalidRetries') == 0
        and provenance['cargoLockUnchanged']
        and provenance['onlyFeatureAddedToDependencyManifests']
        and not provenance['sparrowOrJaguaSourceFilesAdded'])
    return write_result(document, clauses, candidate, frozen, out)


if __name__ == '__main__':
    main()
