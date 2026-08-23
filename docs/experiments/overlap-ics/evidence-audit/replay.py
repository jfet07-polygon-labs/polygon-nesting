#!/usr/bin/env python3
"""**Re-run the committed fixed-work replays and compare, on this machine.**

    python3 replay.py <root> <wall.json> <out-dir> [seeds...] [--out=doc.json]

`wall.json` records, per seed, the fixed-work ordinal of the last wall
publication and the two-process replay it drove: `replayedBites`,
`replayPublications`, `replayDepthMm`, `replayOrdinals` and two SHA-256 digests.
"Wall publications record their fixed-work ordinal" (Grok review 12 Round 2
§6.8) is only worth recording if something replays it, and round 2 is being
asked to sign the replay without anyone having re-run one.

This does two different things and keeps them apart:

  * **the machinery** - two fresh processes on THIS binary, same options,
    documents compared after stripping the one `wall` object that holds every
    clock reading. This is a claim about determinism and it must hold here.
  * **the reproduction** - the four substantive fields of the committed replay
    (`replayPublications`, `replayDepthMm`, `replayOrdinals`,
    `invalidPublications`) against what this machine produces.

The two digests in `wall.json` are **not** comparable across checkouts: the
document they cover carries the absolute request path and the executable's own
SHA-256, both of which differ between worktrees. The digest recomputed here is
therefore taken over the same document with `wall`, `binary`, `request` and
`executableSha256` removed, and both the committed-shape and the
path-independent digests are printed so a reader can see which is which.
"""
import hashlib
import json
import os
import subprocess
import sys

PATH_FIELDS = ['wall', 'binary', 'request', 'executableSha256', 'instrument']


def digest(document, fields):
    copy = {k: v for k, v in document.items() if k not in fields}
    payload = json.dumps(copy, sort_keys=True, separators=(',', ':'))
    return hashlib.sha256(payload.encode()).hexdigest()


def run(root, out_path, seed, bites):
    command = [
        f'{root}/target/release/examples/overlap_ics_benchmark',
        '--cell=cutclose',
        f'--request={root}/tests/fixtures/mixed-61/'
        'mixed61-request-exact-clearance.json',
        '--edge=5', '--pair=5', '--mode=fixed',
        f'--bites={bites}', '--attempts=1', '--iters=400',
        '--compressbites=0', '--workers=8', f'--seed={seed}',
    ]
    with open(out_path, 'w') as handle:
        result = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                                check=False)
    status = result.returncode
    try:
        with open(out_path) as handle:
            document = json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        document = {'_loadError': str(error)}
    return document, status, (result.stderr or b'').decode()[-400:]


def main():
    argv = [value for value in sys.argv[1:] if not value.startswith('--out=')]
    out_doc = next((value[6:] for value in sys.argv[1:]
                    if value.startswith('--out=')), None)
    if len(argv) < 3:
        raise SystemExit(__doc__)
    root, wall_path, out_dir = argv[0], argv[1], argv[2]
    seeds = [int(value) for value in argv[3:]] or [0, 1, 5]
    os.makedirs(out_dir, exist_ok=True)
    committed = json.load(open(wall_path))
    by_seed = {row['seed']: row for row in committed.get('fixedWorkReplay', [])
               if not row.get('skipped')}

    rows = []
    for seed in seeds:
        recorded = by_seed.get(seed)
        if recorded is None:
            rows.append({'seed': seed, 'skipped': True})
            continue
        bites = recorded['replayedBites']
        first, status_a, err_a = run(root, f'{out_dir}/replay-seed{seed}-a.json',
                                     seed, bites)
        second, status_b, err_b = run(root, f'{out_dir}/replay-seed{seed}-b.json',
                                      seed, bites)
        outcome = first.get('outcome', {})
        stripped_a = {k: v for k, v in first.items() if k != 'wall'}
        stripped_b = {k: v for k, v in second.items() if k != 'wall'}
        ordinals = [row['ordinal'] for row in outcome.get('publications', [])]
        rows.append({
            'seed': seed,
            'replayedBites': bites,
            'exitA': status_a,
            'exitB': status_b,
            'stderrA': err_a,
            'stderrB': err_b,
            # the machinery
            'twoProcessBitIdentical': (status_a == 0 and status_b == 0
                                       and stripped_a == stripped_b),
            'committedShapeDigest': digest(first, ['wall']),
            'pathIndependentDigest': digest(first, PATH_FIELDS),
            # the reproduction
            'committed': {
                'replayPublications': recorded.get('replayPublications'),
                'replayDepthMm': recorded.get('replayDepthMm'),
                'invalidPublications': recorded.get('invalidPublications'),
                'ordinals': recorded.get('replayOrdinals'),
                'digestA': recorded.get('digestA'),
            },
            'measured': {
                'replayPublications': outcome.get('publicationCount'),
                'replayDepthMm': outcome.get('depthMm'),
                'invalidPublications': outcome.get('invalidPublications'),
                'ordinals': ordinals,
            },
            'publicationsMatch':
                outcome.get('publicationCount') == recorded.get('replayPublications'),
            'depthMatchesBitForBit':
                outcome.get('depthMm') == recorded.get('replayDepthMm'),
            'ordinalsMatch': ordinals == recorded.get('replayOrdinals'),
        })

    document = {
        'experiment': 'overlap-ics',
        'battery': 'evidence-audit-fixed-work-replay',
        'root': root,
        'source': wall_path,
        'seeds': seeds,
        'rows': rows,
        'ALL_TWO_PROCESS_BIT_IDENTICAL':
            all(row.get('twoProcessBitIdentical') for row in rows
                if not row.get('skipped')),
        'ALL_REPRODUCE_COMMITTED':
            all(row.get('publicationsMatch') and row.get('depthMatchesBitForBit')
                and row.get('ordinalsMatch')
                for row in rows if not row.get('skipped')),
    }
    printable = json.loads(json.dumps(document))
    for row in printable['rows']:
        row.pop('measured', None)
        if 'committed' in row:
            row['committed'].pop('ordinals', None)
    print(json.dumps(printable, indent=1))
    if out_doc:
        os.makedirs(os.path.dirname(os.path.abspath(out_doc)), exist_ok=True)
        with open(out_doc, 'w') as handle:
            json.dump(document, handle, indent=1)
    return 0 if document['ALL_TWO_PROCESS_BIT_IDENTICAL'] else 1


if __name__ == '__main__':
    sys.exit(main())
