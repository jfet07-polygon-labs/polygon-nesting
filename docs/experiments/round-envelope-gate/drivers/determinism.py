#!/usr/bin/env python3
"""Two processes, whole benchmark document, wall-clock fields stripped by name.

    determinism.py OUT.json BINARY CASE [CASE ...]

A `CASE` is `label|arm|kind|payload`:

* `kind = m34`   payload is `fixture;target;work` - one mode-34 slice, the
  matched gate's own command;
* `kind = v3`    payload is `seed;spec` - one coordinator run.

`arm` is `miter`, `union` or `exclusive`, and sets - or leaves unset - the
`POLYGON_NESTING_ROUND_ENVELOPE_KERNEL` variable exactly as `matchedgate.py`
does.

# The stripped set is named, not inherited

The protocol's own note is that `gatelib.strip_times` misses `milliseconds`,
`leafMilliseconds` and `leafSharePercent`. This document has more than those:
the mode-34 schedule report carries six of its own millisecond fields, and the
coordinator carries three wall-clock stamps. Every one is listed here, and the
list is the union of `round-envelope-kernel/drivers/smoke.py`'s VOLATILE - which
was itself *measured* on this document, two runs of one binary differing in
exactly those fields and nothing else - with the schedule's own timings.

Everything else must be byte-identical: every depth, every fingerprint, every
counter, every step digest.
"""
import hashlib
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

KERNEL_ENV = 'POLYGON_NESTING_ROUND_ENVELOPE_KERNEL'
ARM_ENV = {'miter': None, 'union': '1', 'exclusive': '2'}
SPEC = 'past=1,rollback=0,work={work},lanes=1,pconfirm=0'
ROUND_ENV = ('POLYGON_NESTING_CONTACT_BLOCK', 'POLYGON_NESTING_SE2_WITNESS',
             'POLYGON_NESTING_CONTINUOUS_ROTATION',
             'POLYGON_NESTING_SPARSE_ROTATION',
             'POLYGON_NESTING_COMPRESSION_SCHEDULE', KERNEL_ENV)

# `round-envelope-kernel/drivers/smoke.py`'s VOLATILE, verbatim.
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
}
# The mode-34 schedule report's own millisecond fields. Not in `smoke.py`'s set
# because the kernel round never ran a mode-34 slice.
VOLATILE |= {
    'confirmationMs', 'repairMs', 'entryLegalizationMs',
    'currentPoseOverlaySetupMs', 'rotationSurrogateBuildMs', 'se2WitnessMs',
}


def strip(node):
    if isinstance(node, dict):
        return {k: strip(v) for k, v in sorted(node.items())
                if k not in VOLATILE}
    if isinstance(node, list):
        return [strip(v) for v in node]
    if isinstance(node, float):
        return repr(node)
    return node


def command_for(binary, kind, payload, arm):
    env = dict(os.environ)
    for name in ROUND_ENV:
        env.pop(name, None)
    if kind == 'm34':
        fixture, target, work = payload.split(';')
        # The relaxed seed is the parent's own seed, which the matched gate
        # takes from `parents12.json` and which the fixture file is named after.
        seed = fixture.rsplit('parent-seed', 1)[-1].split('.')[0]
        args = [a.format(seed=seed) for a in runlib.ARGS]
        command = ([binary, runlib.REQUESTS['mixed-61']] + args
                   + ['34', fixture, target, '', runlib.DEFAULT_ALLOWANCE])
        env['POLYGON_NESTING_PROFILE'] = '1'
        env['POLYGON_NESTING_COMPRESSION_SCHEDULE'] = SPEC.format(work=work)
    elif kind == 'v3':
        seed, spec = payload.split(';')
        args = [a.format(seed=seed) for a in runlib.ARGS]
        command = ([binary, runlib.REQUESTS['mixed-61']] + args
                   + ['0', '', '', '', runlib.DEFAULT_ALLOWANCE, spec])
    else:
        raise SystemExit(f'unknown case kind {kind}')
    if ARM_ENV[arm] is not None:
        env[KERNEL_ENV] = ARM_ENV[arm]
    return command, env


def main():
    out_path, binary = sys.argv[1], sys.argv[2]
    result = {'binary': binary,
              'binarySha256': hashlib.sha256(open(binary, 'rb').read())
              .hexdigest(),
              'volatileFieldsStripped': sorted(VOLATILE), 'cases': []}
    ok = True
    for case in sys.argv[3:]:
        label, arm, kind, payload = case.split('|')
        command, env = command_for(binary, kind, payload, arm)
        digests = []
        exits = []
        for index in range(2):
            path = f'/var/lib/t3/tmp/rekgate/det-{label}-{index}.json'
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, 'w') as handle:
                proc = subprocess.run(command, stdout=handle,
                                      stderr=subprocess.PIPE, check=False,
                                      env=env)
            exits.append(proc.returncode)
            document = json.load(open(path))
            digests.append(hashlib.sha256(
                json.dumps(strip(document), sort_keys=True).encode())
                .hexdigest())
        identical = digests[0] == digests[1] and exits[0] == exits[1] == 0
        ok = ok and identical
        result['cases'].append({'label': label, 'arm': arm, 'kind': kind,
                                'payload': payload, 'exits': exits,
                                'strippedDigests': digests,
                                'identical': identical})
        print(f'{label} [{arm}] identical={identical} '
              f'digest={digests[0][:16]}', flush=True)
    result['ALL_IDENTICAL'] = ok
    json.dump(result, open(out_path, 'w'), indent=1)
    print(json.dumps({'ALL_IDENTICAL': ok}, indent=1))
    raise SystemExit(0 if ok else 1)


if __name__ == '__main__':
    main()
