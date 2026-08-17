#!/usr/bin/env python3
"""A/B modes 28 and 29 on perturbed fixtures: base commit binary vs new binary.

Every substantive field must match; only the run's own metadata (elapsed times,
executable hash, worktree status) may differ.
"""
import sys, json, os, subprocess
sys.path.insert(0, '/var/lib/t3/tmp/orient')
import lib
import drv

META = {'elapsedMs', 'engineWorktreeDirty', 'engineWorktreeStatus', 'executableSha256',
        'firstQuartileElapsedMs', 'maxElapsedMs', 'medianElapsedMs', 'minElapsedMs',
        'thirdQuartileElapsedMs', 'relevantSourceTreeSha256', 'engineCommit'}


def diff(x, y, path='', out=None):
    if out is None:
        out = []
    if isinstance(x, dict) and isinstance(y, dict):
        for key in sorted(set(x) | set(y)):
            if key in META:
                continue
            if key not in x:
                out.append(f'{path}/{key} ONLY-NEW {y[key]!r:.120}')
            elif key not in y:
                out.append(f'{path}/{key} ONLY-BASE {x[key]!r:.120}')
            else:
                diff(x[key], y[key], f'{path}/{key}', out)
    elif isinstance(x, list) and isinstance(y, list):
        if len(x) != len(y):
            out.append(f'{path} LEN {len(x)} {len(y)}')
        else:
            for index, (u, v) in enumerate(zip(x, y)):
                diff(u, v, f'{path}/{index}', out)
    elif x != y:
        out.append(f'{path} VAL {x!r} {y!r}')
    return out


def run(binary, tag, mode, parent, target):
    argv = ([binary, lib.REQ] + [a.format(clamp='0', seed='5') for a in lib.ARGS]
            + [str(mode), parent, str(target), '', lib.ALLOWANCE])
    path = f'/var/lib/t3/tmp/orient/ab/{tag}.json'
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, 'w') as handle:
        subprocess.run(argv, stdout=handle, stderr=subprocess.DEVNULL, check=False)
    return json.load(open(path))


ARMS = []
for line, parent, raw in (('rec', drv.RECORD, drv.RECORD_RAW),
                          ('fs', drv.SCRATCH, drv.SCRATCH_RAW)):
    for delta in (0.002, 0.004, 0.01):
        path, depth, moved = drv.flatten_fixture(delta, parent, line)
        ARMS.append((f'{line}-flat{delta}', path, raw + 2.0))
    path, depth = drv.single_nudge_fixture(
        [pid for _, pid in drv.ranked_extents(parent)[:2]], 0.05, parent, f'{line}-ab')
    ARMS.append((f'{line}-nudge2', path, raw + 2.0))

failures = 0
for name, parent, target in ARMS:
    for mode in (28, 29):
        base = run('/var/lib/t3/tmp/orient/bench-base', f'base-{name}-m{mode}', mode,
                   parent, f'{target:.6f}')
        new = run('/var/lib/t3/tmp/orient/bench-new', f'new-{name}-m{mode}', mode,
                  parent, f'{target:.6f}')
        deltas = diff(base, new)
        status = 'IDENTICAL' if not deltas else f'DIFF({len(deltas)})'
        if deltas:
            failures += 1
        print(f'{name} mode {mode}: {status}')
        for entry in deltas[:8]:
            print('   ', entry)
print('A/B arms:', len(ARMS) * 2, 'failures:', failures)
