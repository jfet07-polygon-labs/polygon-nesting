#!/usr/bin/env python3
"""One-shot Gate 0 for Minimum-Conflict Binary Close.

Usage:
    python3 gate0.py <frozen-918d6ff-binary> <reviewed-source-commit> [output-dir]

The caller supplies the externally built frozen control. The candidate defaults
to target/release/examples/overlap_ics_benchmark and may be overridden with
ICS_MBC_BIN. This script never runs a quality cell and never retries a cell.
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
DEFAULT_CANDIDATE = os.path.join(
    ROOT, 'target', 'release', 'examples', 'overlap_ics_benchmark')
SPEC = os.path.join(ROOT, 'docs', 'minimum-conflict-binary-close-spec.md')
SPEC_SHA256 = '7ac45b62247bbae8e0390a3e5ade1f7d60f24c4d418f37ca918c0fb67706b3d4'
FROZEN_COMMIT = '918d6ff2041a652fbebbd91b9a8fba4d0cb1ad81'

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
    with open('/proc/loadavg', encoding='utf-8') as handle:
        return [float(value) for value in handle.read().split()[:3]]


def git(*args):
    return subprocess.check_output(
        ['git', *args], cwd=ROOT, text=True).strip()


def canonical_digest(document):
    payload = json.dumps(document, sort_keys=True, separators=(',', ':'))
    return hashlib.sha256(payload.encode()).hexdigest()


def stripped_identity(document):
    if document is None:
        return None
    value = copy.deepcopy(document)
    for key in ('wall', 'executableSha256', 'buildFeatures'):
        value.pop(key, None)
    return value


def stripped_wall(document):
    if document is None:
        return None
    value = copy.deepcopy(document)
    value.pop('wall', None)
    return value


def stripped_compute_diagnostic(document):
    value = stripped_identity(document)
    if value is None:
        return None
    value.pop('binaryClosePrefix', None)
    for key in ('outcome', 'prefixOutcome'):
        if isinstance(value.get(key), dict):
            value[key].pop('binaryClose', None)
    return value


def run(binary, argv, path):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    command = ([binary, f'--request={REQUEST}', '--edge=5', '--pair=5']
               + argv)
    started = time.monotonic()
    with open(path, 'w', encoding='utf-8') as stdout:
        process = subprocess.run(
            command, stdout=stdout, stderr=subprocess.PIPE, check=False)
    elapsed = time.monotonic() - started
    try:
        with open(path, encoding='utf-8') as handle:
            document = json.load(handle)
    except (OSError, json.JSONDecodeError):
        document = None
    return {
        'command': command,
        'exit': process.returncode,
        'processSeconds': elapsed,
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


def publications_strict_and_revalidated(outcome):
    if not isinstance(outcome, dict) or outcome.get('invalidPublications') != 0:
        return False
    parent_depth = outcome.get('startDepthMm')
    if not isinstance(parent_depth, (int, float)):
        return False
    for row in outcome.get('publications') or []:
        target = row.get('targetDepthMm')
        depth = row.get('publishedRawDepthMm')
        revalidation = row.get('revalidation') or {}
        if not (isinstance(target, (int, float))
                and isinstance(depth, (int, float))
                and target < parent_depth and depth < parent_depth
                and depth <= target and row.get('improvedIncumbent') is True
                and revalidation.get('depthMatchesBitwise') is True
                and revalidation.get('fingerprintMatches') is True):
            return False
        parent_depth = depth
    return outcome.get('publicationCount') == len(outcome.get('publications') or [])


def complete_schedule_records(outcome):
    if not isinstance(outcome, dict):
        return False
    required = {
        'bites', 'publications', 'work', 'fingerprints', 'funnel',
        'invalidPublications', 'calibrated', 'strikeArm', 'exactCheckpoints',
        'consumedWorkerOrders',
    }
    if not required.issubset(outcome):
        return False
    orders = outcome.get('consumedWorkerOrders') or {}
    work = outcome.get('work') or {}
    return bool(
        orders.get('digestSha256')
        and orders.get('completeStateDigestSha256')
        and isinstance(orders.get('sweeps'), int)
        and isinstance(orders.get('slots'), int)
        and work
        and all(all(key in bite for key in (
            'ordinal', 'masterIterations', 'strikes', 'attempts',
            'exactBandEntries', 'exactCheckpointCalls', 'published'))
                for bite in outcome.get('bites') or []))


def decision_complete(decision):
    labels = decision.get('labels') or []
    count = len(labels)
    pairs = decision.get('pairs') or []
    unaries = decision.get('unaries') or []
    digests = decision.get('digests') or {}
    key = decision.get('key') or {}
    return bool(
        count > 0
        and len(decision.get('centreLabels') or []) == count
        and len(decision.get('residualSourceReachable') or []) == count
        and len(unaries) == count
        and len(pairs) == count * (count - 1) // 2
        and len(decision.get('graphEdges') or []) == 2 * count + 2 * len(pairs)
        and isinstance(key.get('requestSeed'), int)
        and isinstance(key.get('exploreBiteOrdinal'), int)
        and decision.get('poseStateBitsValid') is True
        and decision.get('parentProxyPairLegal') is True
        and decision.get('allFiniteNonnegative') is True
        and decision.get('allZeroDiagonal') is True
        and decision.get('allSubmodular') is True
        and decision.get('selectedTotalsFiniteNonnegative') is True
        and decision.get('cutTableBitsEqual') is True
        and decision.get('tableColdBitsEqual') is True
        and decision.get('installedRowsMatchTable') is True
        and decision.get('valid') is True
        and all(digests.get(name) for name in (
            'termTableSha256', 'graphSha256', 'residualSha256',
            'labelsSha256', 'installedPosesSha256', 'installedRowsSha256'))
        and all(len(pair.get('violationBits') or []) == 2
                and len(pair.get('costBits') or []) == 2
                and len(pair.get('rowBits') or []) == 2 for pair in pairs)
        and all(len(unary.get('violationBits') or []) == 2
                and len(unary.get('rowBits') or []) == 2 for unary in unaries))


def binary_trace(document):
    return ((document or {}).get('outcome') or {}).get('binaryClose')


def vector_clauses(document):
    root = (document or {}).get('binaryCloseVectors') or {}
    arithmetic = root.get('synthetic') or {}
    geometry = root.get('syntheticGeometry') or {}
    geometry_trace = ((geometry.get('decision') or {}).get('decisions') or [])
    real_trace = ((root.get('realGeometry') or {}).get('decisions') or [])
    decision = geometry_trace[0] if len(geometry_trace) == 1 else {}
    labels = decision.get('labels') or []
    active_selected_pair = any(
        (pair.get('violationBits') or [[0, 0], [0, 0]])[int(labels[pair['first']])]
        [int(labels[pair['second']])] != 0
        for pair in decision.get('pairs') or []) if labels else False
    active_selected_boundary = any(
        any(value != 0 for value in
            (unary.get('violationBits') or [[0] * 4, [0] * 4])
            [int(labels[unary['piece']])])
        for unary in decision.get('unaries') or []) if labels else False
    pose_bits = geometry.get('poseStates') or []
    return {
        'asymmetricUniqueNontrivial':
            arithmetic.get('uniqueNontrivialMinimum') is True
            and arithmetic.get('expectedLabels') == arithmetic.get('solverLabels')
            and any(arithmetic.get('solverLabels') or [])
            and not all(arithmetic.get('solverLabels') or []),
        'everyLabelCutEnergyIdentity':
            arithmetic.get('everyLabelCutEnergyIdentity') is True,
        'domainAcceptanceAndFailures': all(
            arithmetic.get(key) is True for key in (
                'acceptsZeroDiagonalSubmodular', 'rejectsNonfinite',
                'rejectsNegative', 'rejectsNonzeroDiagonal',
                'rejectsNonsubmodular', 'rejectsAggregateOverflow',
                'rejectsNonnegativeDelta')),
        'trivialAndTieStable':
            arithmetic.get('allZeroLabels') == [False, False]
            and arithmetic.get('allOneLabels') == [True, True]
            and arithmetic.get('tieStable') is True,
        'poseStateBits': bool(pose_bits) and all(
            row.get('zeroBits', [None] * 3)[0]
            == row.get('oneBits', [None] * 3)[0]
            and row.get('zeroBits', [None] * 3)[2]
            == row.get('oneBits', [None] * 3)[2]
            and row.get('zeroBits', [None] * 3)[1]
            != row.get('oneBits', [None] * 3)[1]
            for row in pose_bits),
        'mixedGeometryAndCompleteRows':
            decision_complete(decision) and any(labels) and not all(labels)
            and active_selected_pair and active_selected_boundary,
        'incrementalColdIdentity':
            geometry.get('incrementalMatchesCold') is True
            and geometry.get('incrementalPoseDigestSha256')
            == (decision.get('digests') or {}).get('installedPosesSha256')
            and geometry.get('incrementalRowDigestSha256')
            == (decision.get('digests') or {}).get('installedRowsSha256')
            and geometry.get('incrementalRawPhiBits')
            == decision.get('coldRawPhiBits'),
        'realGeometryDecision':
            len(real_trace) == 1 and decision_complete(real_trace[0]),
    }


def prefix_probe_args(seed, prefix_arm, probe_arm):
    return [
        '--cell=spawntax', '--mode=fixed', '--workers=8',
        '--prefixworkers=8', '--shelfbites=21', '--prefixiters=400',
        '--probeiters=400', '--fingerprints=1', '--consumedorders=1',
        '--revalidate=1', '--orders=1', '--arm=control', f'--seed={seed}',
        f'--prefixbinaryclose={prefix_arm}', f'--binaryclose={probe_arm}',
    ]


def write_result(document, clauses, candidate, out):
    document['frozenBinarySha256After'] = sha256(document['frozenBinary'])
    document['candidateBinarySha256After'] = sha256(candidate)
    document['frozenBinaryUnchangedDuringGate0'] = (
        document['frozenBinarySha256']
        == document['frozenBinarySha256After'])
    document['binaryUnchangedDuringGate0'] = (
        document['candidateBinarySha256']
        == document['candidateBinarySha256After'])
    clauses['frozenBinaryUnchangedDuringGate0'] = (
        document['frozenBinaryUnchangedDuringGate0'])
    clauses['binaryUnchangedDuringGate0'] = document['binaryUnchangedDuringGate0']
    document['machine']['loadAfter'] = loadavg()
    document['clauses'] = clauses
    document['GATE0_PASS'] = all(clauses.values())
    aggregate = os.path.join(out, 'gate0.json')
    with open(aggregate, 'w', encoding='utf-8') as handle:
        json.dump(document, handle, indent=1)
    print(json.dumps({
        'GATE0_PASS': document['GATE0_PASS'],
        'clauses': clauses,
        'aggregate': aggregate,
    }, indent=1))
    return 0 if document['GATE0_PASS'] else 1


def stop_on_failure(document, clauses, candidate, out, clause):
    if clauses.get(clause) is True:
        return None
    document['stoppedAfter'] = clause
    return write_result(document, clauses, candidate, out)


def wait_for_quiet(limit_seconds=600):
    deadline = time.monotonic() + limit_seconds
    while loadavg()[0] >= 1.0 and time.monotonic() < deadline:
        time.sleep(5)
    return loadavg()


def main():
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    frozen = os.path.abspath(sys.argv[1])
    reviewed_commit = sys.argv[2]
    out = (os.path.abspath(sys.argv[3]) if len(sys.argv) > 3 else
           '/var/lib/t3/tmp/overlapics/minimum-conflict-binary-close-gate0')
    candidate = os.path.abspath(os.environ.get('ICS_MBC_BIN', DEFAULT_CANDIDATE))
    os.makedirs(os.path.join(out, 'cells'), exist_ok=True)
    for label, binary in (('frozen', frozen), ('candidate', candidate)):
        if not os.path.isfile(binary) or not os.access(binary, os.X_OK):
            raise SystemExit(f'{label} binary is not executable: {binary}')

    head = git('rev-parse', 'HEAD')
    clean = git('status', '--porcelain') == ''
    initial_load = loadavg()
    document = {
        'experiment': 'overlap-ics',
        'battery': 'minimum-conflict-binary-close-gate0',
        'specPath': SPEC,
        'specSha256': sha256(SPEC),
        'expectedSpecSha256': SPEC_SHA256,
        'frozenCommit': FROZEN_COMMIT,
        'frozenBinary': frozen,
        'frozenBinarySha256': sha256(frozen),
        'candidateBinary': candidate,
        'candidateBinarySha256': sha256(candidate),
        'reviewedSourceCommit': reviewed_commit,
        'headSourceCommit': head,
        'cleanSourceTreeAtStart': clean,
        'reviewQuorum': ['Sol', 'Grok', 'ox-alpha'],
        'request': REQUEST,
        'requestSha256': sha256(REQUEST),
        'machine': {'cpus': os.cpu_count(), 'loadBefore': initial_load},
    }
    clauses = {
        'specDigest': document['specSha256'] == SPEC_SHA256,
        'reviewedFrozenSource': reviewed_commit == head and clean,
        'quietBoxAtStart': initial_load[0] < 1.0,
    }
    if not all(clauses.values()):
        return write_result(document, clauses, candidate, out)

    # G0.1: frozen external control vs feature candidate runtime Centre.
    identity = []
    for name, argv in IDENTITY_CELLS.items():
        print(f'G0.1 identity cell {name}', file=sys.stderr, flush=True)
        base = run(frozen, argv,
                   os.path.join(out, 'cells', f'g01-{name}-frozen.json'))
        new = run(candidate, argv + ['--binaryclose=centre'],
                  os.path.join(out, 'cells', f'g01-{name}-centre.json'))
        base_doc = stripped_identity(base['document'])
        new_doc = stripped_identity(new['document'])
        base_provenance = bool(
            (base['document'] or {}).get('buildFeatures') == ['overlap-ics']
            and (base['document'] or {}).get('executableSha256')
            == document['frozenBinarySha256'])
        candidate_provenance = bool(
            (new['document'] or {}).get('buildFeatures')
            == ['overlap-ics', 'minimum-conflict-binary-close']
            and (new['document'] or {}).get('executableSha256')
            == document['candidateBinarySha256'])
        identity.append({
            'cell': name,
            'baseExit': base['exit'],
            'candidateExit': new['exit'],
            'basePath': base['sourcePath'],
            'candidatePath': new['sourcePath'],
            'baseSourceSha256': base['sourceSha256'],
            'candidateSourceSha256': new['sourceSha256'],
            'baseDigestStripped': canonical_digest(base_doc) if base_doc else None,
            'candidateDigestStripped': canonical_digest(new_doc) if new_doc else None,
            'frozenBuildAndExecutableProvenance': base_provenance,
            'candidateBuildAndExecutableProvenance': candidate_provenance,
            'bitIdentical': base['exit'] == 0 and new['exit'] == 0
            and base_provenance and candidate_provenance
            and base_doc == new_doc,
            'stderrTail': base['stderrTail'] or new['stderrTail'],
        })
    document['g01FeatureRuntimeIsolation'] = identity
    clauses['g01FeatureRuntimeIsolation'] = all(
        row['bitIdentical'] for row in identity)
    stopped = stop_on_failure(
        document, clauses, candidate, out, 'g01FeatureRuntimeIsolation')
    if stopped is not None:
        return stopped

    # G0.2: exact printed vectors and both complete overlap-ICS test corpora.
    print('G0.2 vectors and test corpora', file=sys.stderr, flush=True)
    vector = run(candidate, [
        '--cell=binary-close-vectors', '--binaryclose=centre'],
        os.path.join(out, 'cells', 'g02-vectors.json'))
    checks = vector_clauses(vector['document'])
    tests = {
        'feature': run_test([
            'cargo', 'test', '-q', '-p', 'polygon-nesting-core',
            '--features', 'overlap-ics,minimum-conflict-binary-close', '--lib'],
            os.path.join(out, 'g02-feature-tests.log')),
        'default': run_test([
            'cargo', 'test', '-q', '-p', 'polygon-nesting-core',
            '--features', 'overlap-ics', '--lib'],
            os.path.join(out, 'g02-default-tests.log')),
    }
    document['g02VectorsAndTests'] = {
        'vectorExit': vector['exit'],
        'vectorPath': vector['sourcePath'],
        'vectorSha256': vector['sourceSha256'],
        'clauses': checks,
        'tests': tests,
    }
    clauses['g02VectorsAndTests'] = bool(
        vector['exit'] == 0 and all(checks.values())
        and all(row['exit'] == 0 for row in tests.values()))
    stopped = stop_on_failure(document, clauses, candidate, out, 'g02VectorsAndTests')
    if stopped is not None:
        return stopped

    # G0.3: all nine seeds; prefixAllPublished scopes only the existence test.
    seed_rows = []
    existence = []
    for seed in range(9):
        print(f'G0.3 seed {seed}: Centre then MinCut', file=sys.stderr, flush=True)
        centre = run(candidate, prefix_probe_args(seed, 'centre', 'centre'),
                     os.path.join(out, 'cells', f'g03-seed{seed}-centre.json'))
        mincut = run(candidate, prefix_probe_args(seed, 'centre', 'mincut'),
                     os.path.join(out, 'cells', f'g03-seed{seed}-mincut.json'))
        centre_doc = centre['document'] or {}
        mincut_doc = mincut['document'] or {}
        centre_spawn = centre_doc.get('spawnTax') or {}
        mincut_spawn = mincut_doc.get('spawnTax') or {}
        centre_out = centre_doc.get('outcome') or {}
        mincut_out = mincut_doc.get('outcome') or {}
        trace = mincut_out.get('binaryClose') or {}
        decisions = trace.get('decisions') or []
        prefix_identity = bool(
            centre_doc.get('constructor') == mincut_doc.get('constructor')
            and centre_doc.get('prefixOutcome') == mincut_doc.get('prefixOutcome')
            and centre_spawn.get('prefixDepthMm') == mincut_spawn.get('prefixDepthMm')
            and centre_spawn.get('prefixFingerprint')
            == mincut_spawn.get('prefixFingerprint')
            and centre_spawn.get('prefixPoseDigestSha256')
            == mincut_spawn.get('prefixPoseDigestSha256')
            and centre_spawn.get('prefixPoses') == mincut_spawn.get('prefixPoses')
            and centre_spawn.get('prefixWork') == mincut_spawn.get('prefixWork')
            and centre_spawn.get('prefixCompleteStateDigestSha256')
            == mincut_spawn.get('prefixCompleteStateDigestSha256')
            and centre_spawn.get('prefixConsumedOrderDigestSha256')
            == mincut_spawn.get('prefixConsumedOrderDigestSha256')
            and centre_spawn.get('shelfEntryWidthMm')
            == mincut_spawn.get('shelfEntryWidthMm'))
        regression = not bool(centre_spawn.get('shelfPublished')) \
            or bool(mincut_spawn.get('shelfPublished'))
        valid_treatment = bool(
            trace.get('arm') == 'mincut'
            and trace.get('invalidDecisions') == 0
            and decisions and all(decision_complete(row) for row in decisions))
        all_records = bool(
            complete_schedule_records(centre_doc.get('prefixOutcome'))
            and complete_schedule_records(mincut_doc.get('prefixOutcome'))
            and complete_schedule_records(centre_out)
            and complete_schedule_records(mincut_out))
        authority = bool(
            publications_strict_and_revalidated(centre_doc.get('prefixOutcome'))
            and publications_strict_and_revalidated(mincut_doc.get('prefixOutcome'))
            and publications_strict_and_revalidated(centre_out)
            and publications_strict_and_revalidated(mincut_out))
        centre_has_no_diagnostic = 'binaryClose' not in centre_out
        true_22 = bool(
            centre_spawn.get('prefixAllPublished') is True
            and mincut_spawn.get('prefixAllPublished') is True)
        inversion = bool(
            true_22 and centre_spawn.get('shelfPublished') is False
            and mincut_spawn.get('shelfPublished') is True
            and any((row.get('key') or {}).get('exploreBiteOrdinal') == 22
                    and row.get('hammingDisagreement', 0) > 0
                    for row in decisions))
        if inversion:
            existence.append(seed)
        row = {
            'seed': seed,
            'centreExit': centre['exit'],
            'mincutExit': mincut['exit'],
            'centrePath': centre['sourcePath'],
            'mincutPath': mincut['sourcePath'],
            'prefixIdentity': prefix_identity,
            'regressionImplication': regression,
            'validTreatment': valid_treatment,
            'completeRecords': all_records,
            'authorityAndRevalidation': authority,
            'centreHasNoBinaryDiagnostic': centre_has_no_diagnostic,
            'true22ndBiteCell': true_22,
            'causalPublicationInversion': inversion,
        }
        row['pass'] = bool(
            centre['exit'] == 0 and mincut['exit'] == 0
            and prefix_identity and regression and valid_treatment
            and all_records and authority and centre_has_no_diagnostic)
        seed_rows.append(row)
    document['g03CausalInversion'] = {
        'seeds': seed_rows,
        'existenceSeeds': existence,
        'allNineSeedsAreRegressionCells': len(seed_rows) == 9,
        'prefixAllPublishedScopesOnlyExistence': True,
    }
    clauses['g03CausalInversion'] = bool(
        len(seed_rows) == 9 and all(row['pass'] for row in seed_rows)
        and existence)
    stopped = stop_on_failure(document, clauses, candidate, out, 'g03CausalInversion')
    if stopped is not None:
        return stopped

    # G0.4: five fresh AB/BA pairs; prefix and probe are both ComputeIgnore.
    load_before_cost = wait_for_quiet()
    cost_rows = []
    for pair in range(5):
        sequence = 'AB' if pair % 2 == 0 else 'BA'
        print(f'G0.4 pair {pair} ({sequence})', file=sys.stderr, flush=True)
        arms = ['centre', 'compute-ignore'] if sequence == 'AB' else [
            'compute-ignore', 'centre']
        results = {}
        for arm in arms:
            results[arm] = run(
                candidate, prefix_probe_args(0, arm, arm),
                os.path.join(out, 'cells', f'g04-p{pair}-{sequence}-{arm}.json'))
        centre = results['centre']
        compute = results['compute-ignore']
        centre_doc = centre['document'] or {}
        compute_doc = compute['document'] or {}
        centre_wall = centre_doc.get('wall') or {}
        compute_wall = compute_doc.get('wall') or {}
        centre_proposals = (centre_doc.get('spawnTax') or {}).get(
            'totalLegacyProposals')
        compute_proposals = (compute_doc.get('spawnTax') or {}).get(
            'totalLegacyProposals')
        centre_seconds = centre_wall.get('totalSearchSeconds')
        compute_seconds = compute_wall.get('totalSearchSeconds')
        centre_rate = (centre_proposals / centre_seconds
                       if centre_proposals is not None and centre_seconds else None)
        compute_rate = (compute_proposals / compute_seconds
                        if compute_proposals is not None and compute_seconds else None)
        ratio = (compute_rate / centre_rate
                 if compute_rate is not None and centre_rate else None)
        identity = stripped_compute_diagnostic(centre_doc) \
            == stripped_compute_diagnostic(compute_doc)
        centre_orders = (centre_doc.get('outcome') or {}).get(
            'consumedWorkerOrders') or {}
        compute_orders = (compute_doc.get('outcome') or {}).get(
            'consumedWorkerOrders') or {}
        row = {
            'pair': pair,
            'sequence': sequence,
            'centreExit': centre['exit'],
            'computeExit': compute['exit'],
            'centrePath': centre['sourcePath'],
            'computePath': compute['sourcePath'],
            'centreLegacyProposals': centre_proposals,
            'computeLegacyProposals': compute_proposals,
            'centreSearchSeconds': centre_seconds,
            'computeSearchSeconds': compute_seconds,
            'centreRate': centre_rate,
            'computeIgnoreRate': compute_rate,
            'ratioComputeIgnoreOverCentre': ratio,
            'documentIdentityAfterAllowedRemovals': identity,
            'consumedOrderDigestIdentity':
                centre_orders.get('digestSha256')
                == compute_orders.get('digestSha256'),
            'completeStateDigestIdentity':
                centre_orders.get('completeStateDigestSha256')
                == compute_orders.get('completeStateDigestSha256'),
            'legacyProposalIdentity': centre_proposals == compute_proposals,
        }
        row['pass'] = bool(
            centre['exit'] == 0 and compute['exit'] == 0 and identity
            and row['consumedOrderDigestIdentity']
            and row['completeStateDigestIdentity']
            and row['legacyProposalIdentity'] and ratio is not None)
        cost_rows.append(row)
    ratios = [row['ratioComputeIgnoreOverCentre'] for row in cost_rows
              if row['ratioComputeIgnoreOverCentre'] is not None]
    median_ratio = statistics.median(ratios) if len(ratios) == 5 else None
    document['g04ComputeIgnoreCost'] = {
        'loadBeforeBattery': load_before_cost,
        'pairs': cost_rows,
        'ratiosComputeIgnoreOverCentre': ratios,
        'medianRatioComputeIgnoreOverCentre': median_ratio,
        'threshold': 0.95,
        'reciprocalMedianUsed': False,
    }
    clauses['g04ComputeIgnoreCost'] = bool(
        load_before_cost[0] < 1.0 and all(row['pass'] for row in cost_rows)
        and median_ratio is not None and median_ratio >= 0.95)
    stopped = stop_on_failure(
        document, clauses, candidate, out, 'g04ComputeIgnoreCost')
    if stopped is not None:
        return stopped

    # G0.5: two fresh seed-0 MinCut prefix/probe documents, remove only wall.
    replay_args = prefix_probe_args(0, 'centre', 'mincut')
    print('G0.5 deterministic MinCut replay 1', file=sys.stderr, flush=True)
    replay_a = run(candidate, replay_args,
                   os.path.join(out, 'cells', 'g05-mincut-p1.json'))
    print('G0.5 deterministic MinCut replay 2', file=sys.stderr, flush=True)
    replay_b = run(candidate, replay_args,
                   os.path.join(out, 'cells', 'g05-mincut-p2.json'))
    a_doc = replay_a['document'] or {}
    b_doc = replay_b['document'] or {}
    a_trace = binary_trace(a_doc) or {}
    b_trace = binary_trace(b_doc) or {}
    deterministic = bool(
        replay_a['exit'] == 0 and replay_b['exit'] == 0
        and stripped_wall(a_doc) == stripped_wall(b_doc))
    valid = bool(
        a_trace.get('invalidDecisions') == 0
        and b_trace.get('invalidDecisions') == 0
        and ((a_doc.get('outcome') or {}).get('invalidPublications') == 0)
        and ((b_doc.get('outcome') or {}).get('invalidPublications') == 0)
        and publications_strict_and_revalidated(a_doc.get('prefixOutcome'))
        and publications_strict_and_revalidated(b_doc.get('prefixOutcome'))
        and publications_strict_and_revalidated(a_doc.get('outcome'))
        and publications_strict_and_revalidated(b_doc.get('outcome')))
    document['g05DeterminismAuthorityProvenance'] = {
        'process1Path': replay_a['sourcePath'],
        'process1Sha256': replay_a['sourceSha256'],
        'process2Path': replay_b['sourcePath'],
        'process2Sha256': replay_b['sourceSha256'],
        'bitIdenticalAfterRemovingOnlyWall': deterministic,
        'zeroInvalidDecisionsAndPublications': valid,
        'sourceProvenanceAuditedByReviewQuorum': document['reviewQuorum'],
        'reviewedSourceCommit': reviewed_commit,
    }
    clauses['g05DeterminismAuthorityProvenance'] = deterministic and valid
    return write_result(document, clauses, candidate, out)


if __name__ == '__main__':
    sys.exit(main())
