#!/usr/bin/env python3
"""Two processes, on both halves of this round's pipeline.

    determinism.py OUT.json MEASBINARY SCOREBINARY PLAN CELL [CELL ...]

A `CELL` is `seed@work`, run exactly as `dumpladder.py` runs it.

Three questions, because this round produces three artefacts and a determinism
check on one of them would leave the other two unmeasured:

1. **the armed run's own document** - two processes, whole benchmark document,
   wall-clock fields stripped by name. This is the protocol's check and the
   named set is `round-envelope-gate/drivers/determinism.py`'s, which was itself
   measured rather than guessed;
2. **the dump the armed run wrote** - two processes, SHA-256 of the JSONL. This
   is the artefact the scoring stage reads, and a determinism claim that skipped
   it would be a claim about the wrong file;
3. **the scored document** - two processes of the scorer on one plan. Pure
   geometry, no clock anywhere in it, so it is compared unstripped as well as
   stripped and both are reported.
"""
import hashlib
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

SPEC = 'past=1,rollback=0,work={work},lanes=1,pconfirm=0'
DROP_MM = 1.0
DUMP_ENV = 'POLYGON_NESTING_SKIP_PILE_DUMP'
CAP_ENV = 'POLYGON_NESTING_SKIP_PILE_DUMP_CAP'
ROUND_ENV = ('POLYGON_NESTING_CONTACT_BLOCK', 'POLYGON_NESTING_SE2_WITNESS',
             'POLYGON_NESTING_CONTINUOUS_ROTATION',
             'POLYGON_NESTING_SPARSE_ROTATION',
             'POLYGON_NESTING_COMPRESSION_SCHEDULE',
             'POLYGON_NESTING_ROUND_ENVELOPE_KERNEL', DUMP_ENV, CAP_ENV)

# `round-envelope-gate/drivers/determinism.py`'s VOLATILE, verbatim: that set is
# the union of the kernel round's measured set with the mode-34 schedule's own
# millisecond fields, and this round runs the same mode-34 slice.
VOLATILE = {
    'elapsedMs', 'elapsedSeconds', 'engineElapsedSeconds', 'wallMs',
    'durationMs', 'timestamp', 'totalMs', 'ms', 'processWallSeconds',
    'phaseProfile', 'phases', 'profile', 'leafSeconds', 'engineVersion',
    'buildIdentity', 'binaryPath', 'peakResidentBytes', 'allocatedBytes',
    'medianElapsedMs', 'minElapsedMs', 'maxElapsedMs',
    'firstQuartileElapsedMs', 'thirdQuartileElapsedMs', 'executableSha256',
    'relevantSourceTreeSha256', 'engineWorktreeStatus', 'engineCommit',
    'engineWorktreeDirty', 'milliseconds', 'leafMilliseconds',
    'leafSharePercent', 'birthSeconds', 'publishedSeconds',
    'occupancyOverTime',
    'confirmationMs', 'repairMs', 'entryLegalizationMs',
    'currentPoseOverlaySetupMs', 'rotationSurrogateBuildMs', 'se2WitnessMs',
    'startedSeconds', 'seconds', 'atSeconds', 'horizonSeconds',
    'remainingSeconds', 'queueSeconds', 'probeSeconds',
    'probeEffectiveSeconds', 'probeRateUnitsPerSecond',
    'queueRateUnitsPerSecond', 'rawUnits',
}

# What a stripped comparison must never be allowed to hide. Compared directly,
# by path, outside the digest - because a strip list is a licence to ignore
# fields and the way that goes wrong is that it grows to cover the disagreement
# it was supposed to expose.
RUN_VERDICT_PATHS = (
    ('placements',),
    ('usedLongAxisDepthMm',),
    ('relaxedDiagnostics', 'coupledDynamicSeparator',
     'persistentVacancyPopulation', 'rawSourceDepthMm'),
    ('relaxedDiagnostics', 'coupledDynamicSeparator',
     'persistentVacancyPopulation', 'finalPlacementFingerprint'),
    ('relaxedDiagnostics', 'coupledDynamicSeparator',
     'persistentVacancyPopulation', 'exactValid'),
    ('relaxedDiagnostics', 'coupledDynamicSeparator',
     'persistentVacancyPopulation', 'contractValid'),
    ('relaxedDiagnostics', 'coupledDynamicSeparator',
     'persistentVacancyPopulation', 'compressionSchedule', 'stepDigest'),
    ('relaxedDiagnostics', 'coupledDynamicSeparator',
     'persistentVacancyPopulation', 'compressionSchedule',
     'confirmationsSkippedInfeasible'),
)
SCORE_VERDICT_PATHS = (
    ('jointDistribution',),
    ('releasedLayoutCount',),
    ('kernelRefusesMiterAcceptsCount',),
    ('releasedPairExcursionMm',),
    ('compositeReadingsAgree',),
    ('sampledRecordsTotal',),
)


def dig(document, path):
    node = document
    for key in path:
        if not isinstance(node, dict):
            return None
        node = node.get(key)
    return node


def strip(node):
    if isinstance(node, dict):
        return {k: strip(v) for k, v in sorted(node.items())
                if k not in VOLATILE}
    if isinstance(node, list):
        return [strip(v) for v in node]
    if isinstance(node, float):
        return repr(node)
    return node


def digest_of(document):
    return hashlib.sha256(
        json.dumps(strip(document), sort_keys=True).encode()).hexdigest()


def file_sha256(path):
    return hashlib.sha256(open(path, 'rb').read()).hexdigest()


def run_cell(binary, seed, fixture, target, work, out_path, dump_path):
    args = [a.format(seed=seed) for a in runlib.ARGS]
    command = ([binary, runlib.REQUESTS['mixed-61']] + args
               + ['34', fixture, f'{target:.17g}', '',
                  runlib.DEFAULT_ALLOWANCE])
    env = dict(os.environ)
    for name in ROUND_ENV:
        env.pop(name, None)
    env['POLYGON_NESTING_PROFILE'] = '1'
    env['POLYGON_NESTING_COMPRESSION_SCHEDULE'] = SPEC.format(work=int(work))
    env[DUMP_ENV] = dump_path
    env[CAP_ENV] = '20000'
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    with open(out_path, 'w') as handle:
        proc = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                              check=False, env=env)
    return json.load(open(out_path)), proc.returncode


def main():
    out_path, meas, scorer, plan = sys.argv[1:5]
    parents = {row['seed']: row for row in json.load(open(
        f'{runlib.ROOT}/docs/experiments/contact-block/drivers/'
        'parents12.json'))['rows']}
    matched = json.load(open(
        f'{runlib.ROOT}/docs/experiments/round-envelope-gate/evidence/'
        'matched.json'))
    parent_depth = {cell['seed']: cell['parentRawDepthMm']
                    for cell in matched['cells']}
    result = {'measBinary': meas, 'measBinarySha256': file_sha256(meas),
              'scoreBinary': scorer,
              'scoreBinarySha256': file_sha256(scorer),
              'plan': plan,
              'volatileFieldsStripped': sorted(VOLATILE), 'cases': []}
    ok = True

    for item in sys.argv[5:]:
        seed_text, _, work_text = item.partition('@')
        seed, work = int(seed_text), int(work_text)
        target = parent_depth[seed] - DROP_MM
        digests, exits, dumps, documents = [], [], [], []
        for index in range(2):
            base = f'/var/lib/t3/tmp/skippile/det/seed{seed}-{work}-{index}'
            os.makedirs(os.path.dirname(base), exist_ok=True)
            document, code = run_cell(meas, seed, parents[seed]['fixture'],
                                      target, work, f'{base}.json',
                                      f'{base}.jsonl')
            documents.append(document)
            exits.append(code)
            digests.append(digest_of(document))
            dumps.append(file_sha256(f'{base}.jsonl'))
        verdict_diffs = ['.'.join(path) for path in RUN_VERDICT_PATHS
                         if dig(documents[0], path) != dig(documents[1], path)]
        identical = (digests[0] == digests[1] and dumps[0] == dumps[1]
                     and exits == [0, 0] and not verdict_diffs)
        ok = ok and identical
        result['cases'].append({
            'label': f'run:seed{seed}@{work}', 'kind': 'armed-run',
            'exits': exits, 'strippedDigests': digests,
            'dumpSha256': dumps, 'verdictFieldsThatDiffer': verdict_diffs,
            'identical': identical})
        print(f'run:seed{seed}@{work} identical={identical} '
              f'doc={digests[0][:16]} dump={dumps[0][:16]}', flush=True)

    raw, stripped, exits = [], [], []
    for index in range(2):
        path = f'/var/lib/t3/tmp/skippile/det/score-{index}.json'
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, 'w') as handle:
            proc = subprocess.run([scorer, plan], stdout=handle,
                                  stderr=subprocess.PIPE, check=False)
        exits.append(proc.returncode)
        raw.append(file_sha256(path))
        stripped.append(digest_of(json.load(open(path))))
    documents = [json.load(open(f'/var/lib/t3/tmp/skippile/det/score-{i}.json'))
                 for i in range(2)]
    verdict_diffs = ['.'.join(path) for path in SCORE_VERDICT_PATHS
                     if dig(documents[0], path) != dig(documents[1], path)]
    identical = (raw[0] == raw[1] and stripped[0] == stripped[1]
                 and exits == [0, 0] and not verdict_diffs)
    ok = ok and identical
    result['cases'].append({'label': 'score', 'kind': 'scored-document',
                            'exits': exits, 'rawSha256': raw,
                            'strippedDigests': stripped,
                            'verdictFieldsThatDiffer': verdict_diffs,
                            'identical': identical})
    print(f'score identical={identical} raw={raw[0][:16]}', flush=True)

    result['ALL_IDENTICAL'] = ok
    json.dump(result, open(out_path, 'w'), indent=1)
    print(json.dumps({'ALL_IDENTICAL': ok}, indent=1))
    raise SystemExit(0 if ok else 1)


if __name__ == '__main__':
    main()
