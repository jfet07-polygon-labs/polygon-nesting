#!/usr/bin/env python3
"""Build the reviewed Gate-0 binary and issue its local build receipt.

Usage:
    python3 build_candidate.py <reviewed-source-commit> <receipt.json>

The output path must not exist. The script refuses a dirty tree or a HEAD
different from the reviewed commit, runs the one frozen release build, and
hashes the resulting executable after the build. Gate 0 consumes this receipt
instead of trusting a source-commit string supplied to the executable.
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
BINARY = os.path.join(
    ROOT, 'target', 'release', 'examples', 'overlap_ics_benchmark')
FEATURES = ['overlap-ics', 'pool-retry-tracker-rebase']
COMMAND = [
    'cargo', 'build', '--release', '--locked', '-p', 'polygon-nesting-core',
    '--example', 'overlap_ics_benchmark', '--features', ','.join(FEATURES),
]


def sha256(path):
    with open(path, 'rb') as handle:
        return hashlib.sha256(handle.read()).hexdigest()


def git(*args):
    return subprocess.check_output(
        ['git', *args], cwd=ROOT, text=True).strip()


def version(*command):
    return subprocess.check_output(command, cwd=ROOT, text=True).strip()


def main():
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    reviewed = sys.argv[1]
    receipt_path = os.path.abspath(sys.argv[2])
    if os.path.exists(receipt_path):
        raise SystemExit(f'receipt output already exists: {receipt_path}')
    head_before = git('rev-parse', 'HEAD')
    status_before = git('status', '--porcelain')
    if head_before != reviewed or status_before:
        raise SystemExit(
            'refusing build: HEAD must equal the reviewed commit and the '
            'worktree must be clean')

    started = datetime.now(timezone.utc).isoformat()
    process = subprocess.run(COMMAND, cwd=ROOT, check=False)
    if process.returncode != 0:
        raise SystemExit(process.returncode)
    head_after = git('rev-parse', 'HEAD')
    status_after = git('status', '--porcelain')
    if head_after != reviewed or status_after:
        raise SystemExit('source changed during the candidate build')
    if not os.path.isfile(BINARY) or not os.access(BINARY, os.X_OK):
        raise SystemExit(f'build did not produce an executable: {BINARY}')

    rustc = version('rustc', '--version', '--verbose')
    host = next(
        (line.split(':', 1)[1].strip() for line in rustc.splitlines()
         if line.startswith('host:')),
        None,
    )
    receipt = {
        'schema': 'pool-retry-tracker-rebase/build-receipt/v1',
        'reviewedSourceCommit': reviewed,
        'headBefore': head_before,
        'headAfter': head_after,
        'sourceStatusBefore': status_before,
        'sourceStatusAfter': status_after,
        'sourceTree': git('rev-parse', f'{reviewed}^{{tree}}'),
        'buildCommand': COMMAND,
        'package': 'polygon-nesting-core',
        'example': 'overlap_ics_benchmark',
        'profile': 'release',
        'features': FEATURES,
        'binaryPath': BINARY,
        'binarySha256': sha256(BINARY),
        'specSha256': sha256(SPEC),
        'requestSha256': sha256(REQUEST),
        'sourcePlanSha256': sha256(SOURCE_PLAN),
        'cargoVersion': version('cargo', '--version', '--verbose'),
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
