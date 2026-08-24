#!/usr/bin/env python3
"""One-shot Gate 0 for Pool-Retry Tracker Rebase.

Usage:
    python3 gate0.py <frozen-b1235a1-binary> <reviewed-source-commit> [output-dir]

The candidate defaults to target/release/examples/overlap_ics_benchmark and
may be overridden with ICS_POOL_REBASE_BIN. The script never builds, retries a
cell, or runs Primary30. It stops at the first failed Gate-0 section.
"""

import copy
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


def canonical_digest(value):
    payload = json.dumps(value, sort_keys=True, separators=(',', ':'))
    return hashlib.sha256(payload.encode()).hexdigest()


def loadavg():
    with open('/proc/loadavg', encoding='utf-8') as handle:
        return [float(value) for value in handle.read().split()[:3]]


def git(*args):
    return subprocess.check_output(
        ['git', *args], cwd=ROOT, text=True).strip()


def run(binary, argv, path):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    command = ([binary, f'--request={REQUEST}', '--edge=5', '--pair=5']
               + argv)
    started = time.monotonic()
    with open(path, 'w', encoding='utf-8') as stdout:
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
        'stderrTail': (process.stderr or b'').decode(errors='replace')[-1600:],
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
    with open(path, 'w', encoding='utf-8') as handle:
        json.dump(plan, handle, indent=1)
        handle.write('\n')
    return plan


def retry_args(seed, arm, plan):
    return [
        '--cell=cutclose', '--mode=calibrated', f'--plan={plan}',
        f'--wall={PLAN_SECONDS}', '--workers=8', '--orders=1',
        '--arm=control', f'--seed={seed}', f'--poolrebase={arm}',
        '--poolrebasetrace=1', '--firstretry=1', '--fingerprints=1',
        '--revalidate=1',
    ]


def strip_identity(document):
    value = copy.deepcopy(document)
    for key in ('wall', 'executableSha256', 'buildFeatures'):
        value.pop(key, None)
    schedule = value.get('schedule') or {}
    for key in ('poolRetryArm', 'recordPoolRetryRebase',
                'stopAfterFirstPoolRetry', 'firstPoolRetryIterationCap'):
        schedule.pop(key, None)
    outcome = value.get('outcome') or {}
    outcome.pop('poolRetryRebase', None)
    return value


def strip_retry_diagnostic(document):
    value = copy.deepcopy(document)
    value.pop('wall', None)
    schedule = value.get('schedule') or {}
    schedule.pop('poolRetryArm', None)
    trace = (value.get('outcome') or {}).get('poolRetryRebase') or {}
    trace.pop('arm', None)
    for row in trace.get('decisions') or []:
        row.pop('resetWeights', None)
        row.pop('pathSeconds', None)
    return value


def first_retry(document):
    trace = ((document or {}).get('outcome') or {}).get('poolRetryRebase') or {}
    rows = trace.get('decisions') or []
    return rows[0] if len(rows) == 1 else None


def checkpoint(row):
    if not row:
        return None
    keys = (
        'key', 'widthBits', 'poolLength', 'selectedRank',
        'poolEntryRawPhiBits', 'selectedPoseDigestSha256', 'savedWeights',
        'postInstallPoseDigestSha256', 'postInstallRawRowDigestSha256')
    return {key: row.get(key) for key in keys}


def disruption(row):
    if not row:
        return None
    keys = (
        'disruption', 'postDisruptionPoseDigestSha256',
        'postDisruptionRawRowDigestSha256',
        'coldPostDisruptionRawRowDigestSha256')
    return {key: row.get(key) for key in keys}


def retry_complete(document, row):
    outcome = (document or {}).get('outcome') or {}
    work = (row or {}).get('pathWork') or {}
    return bool(
        row and row.get('valid') is True
        and row.get('incrementalColdRawRowsIdentical') is True
        and isinstance(row.get('retryIterations'), int)
        and row.get('retryIterations') <= 400
        and row.get('retryStop') in {
            'published', 'refused', 'struck', 'deadline', 'work-cap'}
        and isinstance(row.get('downstreamFingerprints'), list)
        and len(row['downstreamFingerprints']) == row['retryIterations']
        and isinstance(row.get('pathSeconds'), (int, float))
        and row['pathSeconds'] > 0.0
        and isinstance(work.get('sampleEvaluations'), int)
        and outcome.get('invalidPublications') == 0
        and outcome.get('publicationCount')
        == len(outcome.get('publications') or []))


def publications_revalidated(document):
    outcome = (document or {}).get('outcome') or {}
    if outcome.get('invalidPublications') != 0:
        return False
    for row in outcome.get('publications') or []:
        check = row.get('revalidation') or {}
        if not (row.get('improvedIncumbent') is True
                and check.get('depthMatchesBitwise') is True
                and check.get('fingerprintMatches') is True):
            return False
    return True


def pair_row(seed, saved_run, rebase_run):
    saved_doc = saved_run['document']
    rebase_doc = rebase_run['document']
    saved = first_retry(saved_doc)
    rebase = first_retry(rebase_doc)
    saved_weights = (saved or {}).get('savedWeights') or {}
    reset = (rebase or {}).get('resetWeights') or {}
    post = (rebase or {}).get('postPolicyWeights') or {}
    prefix_equal = checkpoint(saved) == checkpoint(rebase)
    disruption_equal = disruption(saved) == disruption(rebase)
    saved_published = (saved or {}).get('retryPublished') is True
    rebase_published = (rebase or {}).get('retryPublished') is True
    downstream_changed = (
        (saved or {}).get('downstreamFingerprints')
        != (rebase or {}).get('downstreamFingerprints'))
    return {
        'seed': seed,
        'savedPath': saved_run['sourcePath'],
        'savedSha256': saved_run['sourceSha256'],
        'rebasePath': rebase_run['sourcePath'],
        'rebaseSha256': rebase_run['sourceSha256'],
        'exitsZero': saved_run['exit'] == 0 and rebase_run['exit'] == 0,
        'checkpointDigestSaved': canonical_digest(checkpoint(saved)),
        'checkpointDigestRebase': canonical_digest(checkpoint(rebase)),
        'prefixBitIdentical': prefix_equal,
        'savedWeightsFiniteNontrivial':
            saved_weights.get('allFinite') is True
            and saved_weights.get('countAboveFloor', 0) > 0,
        'rebaseExactlyOne':
            reset.get('allFinite') is True
            and reset.get('allExactlyOne') is True
            and post.get('allExactlyOne') is True,
        'disruptionBitIdentical': disruption_equal,
        'savedPublished': saved_published,
        'rebasePublished': rebase_published,
        'savedToRebasePublication': not saved_published and rebase_published,
        'reversePublication': saved_published and not rebase_published,
        'downstreamChanged': downstream_changed,
        'complete':
            retry_complete(saved_doc, saved)
            and retry_complete(rebase_doc, rebase)
            and ((saved_doc or {}).get('schedule') or {}).get(
                'firstPoolRetryIterationCap') == 400
            and ((rebase_doc or {}).get('schedule') or {}).get(
                'firstPoolRetryIterationCap') == 400
            and publications_revalidated(saved_doc)
            and publications_revalidated(rebase_doc),
        'savedRetryIterations': (saved or {}).get('retryIterations'),
        'rebaseRetryIterations': (rebase or {}).get('retryIterations'),
        'savedStop': (saved or {}).get('retryStop'),
        'rebaseStop': (rebase or {}).get('retryStop'),
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
    document['machine']['loadAfter'] = loadavg()
    document['clauses'] = clauses
    document['GATE0_PASS'] = all(clauses.values())
    aggregate = os.path.join(out, 'gate0.json')
    with open(aggregate, 'w', encoding='utf-8') as handle:
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
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    frozen = os.path.abspath(sys.argv[1])
    reviewed = sys.argv[2]
    out = (os.path.abspath(sys.argv[3]) if len(sys.argv) > 3 else
           '/var/lib/t3/tmp/overlapics/pool-retry-tracker-rebase-gate0')
    candidate = os.path.abspath(
        os.environ.get('ICS_POOL_REBASE_BIN', DEFAULT_CANDIDATE))
    cells = os.path.join(out, 'cells')
    os.makedirs(cells, exist_ok=True)
    for label, binary in (('frozen', frozen), ('candidate', candidate)):
        if not os.path.isfile(binary) or not os.access(binary, os.X_OK):
            raise SystemExit(f'{label} binary is not executable: {binary}')

    head = git('rev-parse', 'HEAD')
    status = git('status', '--porcelain')
    initial_load = loadavg()
    document = {
        'experiment': 'pool-retry-tracker-rebase-gate0',
        'specPath': SPEC,
        'specSha256': sha256(SPEC),
        'expectedSpecSha256': SPEC_SHA256,
        'frozenCommit': FROZEN_COMMIT,
        'frozenBinary': frozen,
        'frozenBinarySha256': sha256(frozen),
        'candidateBinary': candidate,
        'candidateBinarySha256': sha256(candidate),
        'reviewedSourceCommit': reviewed,
        'headSourceCommit': head,
        'sourceStatusAtStart': status,
        'reviewQuorum': ['Sol', 'Grok', 'ox-alpha'],
        'request': REQUEST,
        'requestSha256': sha256(REQUEST),
        'sourcePlan': SOURCE_PLAN,
        'sourcePlanSha256': sha256(SOURCE_PLAN),
        'machine': {'cpus': os.cpu_count(), 'loadBefore': initial_load},
    }
    clauses = {
        'specDigest': document['specSha256'] == SPEC_SHA256,
        'reviewedFrozenSource': reviewed == head and status == '',
        'quietBoxAtStart': initial_load[0] < 1.0,
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
            'bitIdentical':
                base['exit'] == 0 and saved['exit'] == 0
                and (base['document'] or {}).get('buildFeatures')
                == ['overlap-ics']
                and (saved['document'] or {}).get('buildFeatures') == FEATURES
                and strip_identity(base['document'])
                == strip_identity(saved['document']),
        })

    vector = run(candidate, ['--cell=pool-rebase-vectors'],
                 os.path.join(cells, 'g01-vectors.json'))
    vector_doc = (vector['document'] or {}).get('poolRetryRebaseVectors') or {}
    vector_pass = bool(
        vector['exit'] == 0
        and vector_doc.get('savedRestoredExactly') is True
        and vector_doc.get('rebaseAllExactlyOne') is True
        and vector_doc.get('computeIgnoreRestoredExactly') is True
        and vector_doc.get('rawRowsUnchanged') is True
        and vector_doc.get('nonfiniteSavedVisible') is True)
    default_tests = run_test([
        'cargo', 'test', '-p', 'polygon-nesting-core', '--lib',
        '--features', 'overlap-ics', 'search::overlap_ics', '--',
        '--nocapture'], os.path.join(out, 'g01-default-tests.log'))
    feature_tests = run_test([
        'cargo', 'test', '-p', 'polygon-nesting-core', '--lib',
        '--features', 'overlap-ics,pool-retry-tracker-rebase',
        'search::overlap_ics', '--', '--nocapture'],
        os.path.join(out, 'g01-feature-tests.log'))
    document['g01'] = {
        'identity': identity,
        'vector': vector,
        'vectorPass': vector_pass,
        'defaultTests': default_tests,
        'featureTests': feature_tests,
    }
    clauses['g01FeatureRuntimeVectorsTests'] = bool(
        all(row['bitIdentical'] for row in identity)
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
    for seed in range(9):
        saved = run(candidate, retry_args(seed, 'saved', plan_path),
                    os.path.join(cells, f'g02-seed{seed}-saved.json'))
        rebase = run(candidate, retry_args(seed, 'rebase', plan_path),
                     os.path.join(cells, f'g02-seed{seed}-rebase.json'))
        pairs.append(pair_row(seed, saved, rebase))
    treatment_wins = [row for row in pairs if row['savedToRebasePublication']]
    reverses = [row for row in pairs if row['reversePublication']]
    causal_wins = [row for row in treatment_wins if row['downstreamChanged']]
    document['g02'] = {
        'pairs': pairs,
        'treatmentWinSeeds': [row['seed'] for row in treatment_wins],
        'reverseSeeds': [row['seed'] for row in reverses],
        'causalTreatmentWinSeeds': [row['seed'] for row in causal_wins],
    }
    clauses['g02NinePairedFirstRetries'] = bool(
        all(row['exitsZero'] and row['prefixBitIdentical']
            and row['savedWeightsFiniteNontrivial']
            and row['rebaseExactlyOne']
            and row['disruptionBitIdentical']
            and row['complete'] for row in pairs)
        and len(treatment_wins) >= 2
        and not reverses
        and len(causal_wins) >= 2)
    stopped = stop_after(
        document, clauses, candidate, frozen, out,
        'g02NinePairedFirstRetries')
    if stopped is not None:
        return stopped

    cost_pairs = []
    for index, order in enumerate(('AB', 'BA', 'AB', 'BA', 'AB')):
        arms = ('saved', 'compute-ignore') if order == 'AB' else (
            'compute-ignore', 'saved')
        runs = {}
        for arm in arms:
            runs[arm] = run(
                candidate, retry_args(0, arm, plan_path),
                os.path.join(cells, f'g03-p{index}-{order}-{arm}.json'))
        saved_row = first_retry(runs['saved']['document'])
        compute_row = first_retry(runs['compute-ignore']['document'])
        saved_rate = ((saved_row['pathWork']['sampleEvaluations']
                       / saved_row['pathSeconds']) if saved_row else 0.0)
        compute_rate = ((compute_row['pathWork']['sampleEvaluations']
                         / compute_row['pathSeconds']) if compute_row else 0.0)
        cost_pairs.append({
            'pair': index,
            'order': order,
            'savedPath': runs['saved']['sourcePath'],
            'computeIgnorePath': runs['compute-ignore']['sourcePath'],
            'savedRate': saved_rate,
            'computeIgnoreRate': compute_rate,
            'ratio': compute_rate / saved_rate if saved_rate > 0.0 else 0.0,
            'identity':
                runs['saved']['exit'] == 0
                and runs['compute-ignore']['exit'] == 0
                and strip_retry_diagnostic(runs['saved']['document'])
                == strip_retry_diagnostic(runs['compute-ignore']['document']),
        })
    ratios = [row['ratio'] for row in cost_pairs]
    document['g03'] = {
        'pairs': cost_pairs,
        'medianComputeIgnoreOverSaved': statistics.median(ratios),
    }
    clauses['g03ComputeIgnoreCostIdentity'] = bool(
        all(row['identity'] for row in cost_pairs)
        and statistics.median(ratios) >= 0.95)
    stopped = stop_after(
        document, clauses, candidate, frozen, out,
        'g03ComputeIgnoreCostIdentity')
    if stopped is not None:
        return stopped

    replay_a = run(candidate, retry_args(0, 'rebase', plan_path),
                   os.path.join(cells, 'g04-seed0-rebase-a.json'))
    replay_b = run(candidate, retry_args(0, 'rebase', plan_path),
                   os.path.join(cells, 'g04-seed0-rebase-b.json'))
    document['g04'] = {
        'firstPath': replay_a['sourcePath'],
        'secondPath': replay_b['sourcePath'],
        'firstSha256': replay_a['sourceSha256'],
        'secondSha256': replay_b['sourceSha256'],
        'deterministicAfterTimingStrip':
            strip_retry_diagnostic(replay_a['document'])
            == strip_retry_diagnostic(replay_b['document']),
        'invalidRetriesFirst':
            (((replay_a['document'] or {}).get('outcome') or {})
             .get('poolRetryRebase') or {}).get('invalidRetries'),
        'invalidRetriesSecond':
            (((replay_b['document'] or {}).get('outcome') or {})
             .get('poolRetryRebase') or {}).get('invalidRetries'),
    }
    clauses['g04DeterminismAuthorityProvenance'] = bool(
        replay_a['exit'] == 0 and replay_b['exit'] == 0
        and document['g04']['deterministicAfterTimingStrip']
        and document['g04']['invalidRetriesFirst'] == 0
        and document['g04']['invalidRetriesSecond'] == 0
        and publications_revalidated(replay_a['document'])
        and publications_revalidated(replay_b['document']))
    return write_result(document, clauses, candidate, frozen, out)


if __name__ == '__main__':
    raise SystemExit(main())
