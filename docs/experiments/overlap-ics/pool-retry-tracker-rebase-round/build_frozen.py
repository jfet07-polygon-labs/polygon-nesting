#!/usr/bin/env python3
"""Build and attest the external b1235a1 Gate-0 control.

Usage:
    python3 build_frozen.py <clean-b1235a1-source-tree> <new-target-dir> \
        <new-receipt.json>

Both output paths must be new. The source must be a clean detached checkout at
the exact frozen commit. Cargo writes only to the supplied external target.
"""

import hashlib
import json
import os
import subprocess
import sys
from datetime import datetime, timezone


HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, '..', '..', '..', '..'))
SPEC = os.path.join(ROOT, 'docs', 'pool-retry-tracker-rebase-spec.md')
REQUEST = os.path.join(
    ROOT, 'tests', 'fixtures', 'mixed-61',
    'mixed61-request-exact-clearance.json')
SOURCE_PLAN = os.path.join(
    ROOT, 'docs', 'experiments', 'overlap-ics',
    'minimum-conflict-binary-close-round', 'evidence',
    'plan-f100-mbc.icscal.json')
FROZEN_COMMIT = 'b1235a11cf4a57d7437accbfc2348a05692fe0be'
FEATURES = ['overlap-ics']
COMMAND = [
    'cargo', 'build', '--release', '--locked', '-p', 'polygon-nesting-core',
    '--example', 'overlap_ics_benchmark', '--features', ','.join(FEATURES),
]


def sha256(path):
    with open(path, 'rb') as handle:
        return hashlib.sha256(handle.read()).hexdigest()


def git(source, *args):
    return subprocess.check_output(
        ['git', *args], cwd=source, text=True).strip()


def version(source, *command):
    return subprocess.check_output(command, cwd=source, text=True).strip()


def main():
    if len(sys.argv) != 4:
        raise SystemExit(__doc__)
    source = os.path.realpath(sys.argv[1])
    target = os.path.abspath(sys.argv[2])
    receipt_path = os.path.abspath(sys.argv[3])
    if not os.path.isdir(source):
        raise SystemExit(f'frozen source tree is not a directory: {source}')
    for label, path in (('target', target), ('receipt', receipt_path)):
        if os.path.exists(path):
            raise SystemExit(f'{label} output already exists: {path}')

    head_before = git(source, 'rev-parse', 'HEAD')
    status_before = git(source, 'status', '--porcelain')
    if head_before != FROZEN_COMMIT or status_before:
        raise SystemExit(
            'refusing frozen build: source must be the clean exact b1235a1 commit')
    source_tree = git(source, 'rev-parse', f'{FROZEN_COMMIT}^{{tree}}')
    started = datetime.now(timezone.utc).isoformat()
    environment = os.environ.copy()
    environment['CARGO_TARGET_DIR'] = target
    process = subprocess.run(COMMAND, cwd=source, env=environment, check=False)
    if process.returncode != 0:
        raise SystemExit(process.returncode)

    head_after = git(source, 'rev-parse', 'HEAD')
    status_after = git(source, 'status', '--porcelain')
    if head_after != FROZEN_COMMIT or status_after:
        raise SystemExit('frozen source changed during the build')
    binary = os.path.join(target, 'release', 'examples', 'overlap_ics_benchmark')
    if not os.path.isfile(binary) or not os.access(binary, os.X_OK):
        raise SystemExit(f'build did not produce an executable: {binary}')

    rustc = version(source, 'rustc', '--version', '--verbose')
    host = next(
        (line.split(':', 1)[1].strip() for line in rustc.splitlines()
         if line.startswith('host:')),
        None,
    )
    receipt = {
        'schema': 'pool-retry-tracker-rebase/frozen-build-receipt/v1',
        'sourceCommit': FROZEN_COMMIT,
        'headBefore': head_before,
        'headAfter': head_after,
        'sourceStatusBefore': status_before,
        'sourceStatusAfter': status_after,
        'sourceTree': source_tree,
        'sourcePath': source,
        'buildCommand': COMMAND,
        'cargoTargetDir': target,
        'package': 'polygon-nesting-core',
        'example': 'overlap_ics_benchmark',
        'profile': 'release',
        'features': FEATURES,
        'binaryPath': binary,
        'binarySha256': sha256(binary),
        'specSha256': sha256(SPEC),
        'requestSha256': sha256(REQUEST),
        'sourcePlanSha256': sha256(SOURCE_PLAN),
        'cargoVersion': version(source, 'cargo', '--version', '--verbose'),
        'rustcVersion': rustc,
        'targetTriple': host,
        'startedUtc': started,
        'completedUtc': datetime.now(timezone.utc).isoformat(),
    }
    os.makedirs(os.path.dirname(receipt_path), exist_ok=True)
    with open(receipt_path, 'x', encoding='utf-8') as handle:
        json.dump(receipt, handle, indent=1, sort_keys=True)
        handle.write('\n')
    print(json.dumps(receipt, indent=1, sort_keys=True))


if __name__ == '__main__':
    main()
