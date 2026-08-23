#!/usr/bin/env python3
"""**The nine fixed-work replays, re-run here, on a different binary.**

`--mode=fixed` constructs no `Instant` inside the trajectory, so a fixed-work
cell is deterministic and its depth is a number a second machine can be held
to. The committed `wall.json` carries nine of them
(`fixedWorkReplay[].replayDepthMm`, `replayedBites`, `replayPublications`,
`replayOrdinals`), produced on the round's binary `b42c10af…`.

This re-runs all nine on a binary built here, twice each, and compares:

  * the two local processes to each other, with `wall` stripped - determinism;
  * the local depth to the committed `replayDepthMm`, **bit for bit**;
  * the local publication count and the whole `replayOrdinals` array to the
    committed one - the ordinals are the fixed-work coordinates the wall
    publications record, so this is the claim `wall.py:replay()` exists to make;
  * every replay publication through the dual gate, recomputed here.

Exit 0 iff all nine reproduce bit for bit.

    python3 rv_replay.py <binary> [out.json]
"""
import hashlib
import json
import os
import struct
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ICS = os.path.abspath(os.path.join(HERE, '..', '..'))
ROOT = os.path.abspath(os.path.join(ICS, '..', '..', '..'))
REQUEST = f'{ROOT}/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json'
TMP = os.environ.get('RV_TMP', '/var/lib/t3/tmp/rv-audit-out')
WORKERS = 8


def bits(x):
    return None if x is None else struct.pack('>d', x).hex()


def run(binary, seed, bites, tag):
    out = f'{TMP}/{tag}.json'
    os.makedirs(TMP, exist_ok=True)
    cmd = [binary, '--cell=cutclose', f'--request={REQUEST}', '--edge=5',
           '--pair=5', '--mode=fixed', f'--bites={bites}', '--attempts=1',
           '--iters=400', '--compressbites=0', f'--workers={WORKERS}',
           f'--seed={seed}']
    with open(out, 'w') as handle:
        result = subprocess.run(cmd, stdout=handle, stderr=subprocess.PIPE,
                                check=False)
    with open(out) as handle:
        doc = json.load(handle)
    return doc, result.returncode, result.stderr.decode()[-300:]


def digest(doc):
    copy = json.loads(json.dumps(doc))
    copy.pop('wall', None)
    return hashlib.sha256(json.dumps(copy, sort_keys=True,
                                     separators=(',', ':')).encode()).hexdigest()


def main():
    binary = sys.argv[1]
    out_path = sys.argv[2] if len(sys.argv) > 2 else None
    with open(os.path.join(ICS, 'cutclose-rerun/evidence/wall.json')) as h:
        committed = json.load(h)['fixedWorkReplay']
    rows, fails = [], []
    for row in committed:
        seed, bites = row['seed'], row['replayedBites']
        a, sa, ea = run(binary, seed, bites, f'rv-replay-seed{seed}-a')
        b, sb, eb = run(binary, seed, bites, f'rv-replay-seed{seed}-b')
        oa = a['outcome']
        cps = oa['exactCheckpoints']
        dual = all(c['kernelExclusiveValid'] and c['contractValid']
                   for c in cps if c['publishedRawDepthMm'] is not None)
        entry = {
            'seed': seed,
            'replayedBites': bites,
            'exitA': sa, 'exitB': sb, 'stderrA': ea, 'stderrB': eb,
            'localDigestA': digest(a), 'localDigestB': digest(b),
            'localTwoProcessIdentical': digest(a) == digest(b),
            'committedDepthMm': row['replayDepthMm'],
            'localDepthMm': oa['depthMm'],
            'depthBitIdentical':
                bits(oa['depthMm']) == bits(row['replayDepthMm']),
            'committedPublications': row['replayPublications'],
            'localPublications': oa['publicationCount'],
            'ordinalsIdentical':
                [p['ordinal'] for p in oa['publications']]
                == row['replayOrdinals'],
            'localInvalidPublications': oa['invalidPublications'],
            'everyLocalPublicationDualValid': dual,
            'localIncumbentMm': oa['incumbent']['rawSourceDepthMm'],
            'localCheckpoints': len(cps),
            'localPublishedCheckpoints':
                sum(1 for c in cps if c['publishedRawDepthMm'] is not None),
        }
        for clause in ('localTwoProcessIdentical', 'depthBitIdentical',
                       'ordinalsIdentical', 'everyLocalPublicationDualValid'):
            if not entry[clause]:
                fails.append({'seed': seed, 'clause': clause})
        if entry['localPublications'] != entry['committedPublications']:
            fails.append({'seed': seed, 'clause': 'publicationCount',
                          'committed': entry['committedPublications'],
                          'local': entry['localPublications']})
        if entry['localInvalidPublications'] != 0:
            fails.append({'seed': seed, 'clause': 'invalidPublications'})
        rows.append(entry)
    doc = {
        'what': 'the nine committed fixed-work replays, re-run on a binary '
                'built in this worktree',
        'binary': binary,
        'binarySha256': subprocess.run(['sha256sum', binary],
                                       capture_output=True,
                                       text=True).stdout.split()[0],
        'roundBinarySha256':
            'b42c10afca031ce24fac4cb2a85a752462c6fffb1eee42956e523ee846376f03',
        'rows': rows,
        'failures': fails,
        'ALL_NINE_REPRODUCE_BIT_FOR_BIT': not fails,
    }
    if out_path:
        with open(out_path, 'w') as handle:
            handle.write(json.dumps(doc, indent=1, sort_keys=True) + '\n')
    for r in rows:
        print(f"seed{r['seed']} bites={r['replayedBites']:>3} "
              f"committed={r['committedDepthMm']} local={r['localDepthMm']} "
              f"bitIdentical={r['depthBitIdentical']} "
              f"pubs={r['localPublications']}/{r['committedPublications']} "
              f"ordinals={r['ordinalsIdentical']} "
              f"twoProcess={r['localTwoProcessIdentical']} "
              f"dualValid={r['everyLocalPublicationDualValid']}")
    print('failures:', len(fails), fails[:10])
    return 0 if not fails else 1


if __name__ == '__main__':
    sys.exit(main())
