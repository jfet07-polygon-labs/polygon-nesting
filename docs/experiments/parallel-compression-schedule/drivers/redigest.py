"""Recompute every digest-based verdict with the final repaired `doc_digest`.

The batteries stored their raw run documents, so nothing has to be re-executed:
the digests are recomputed from those files. Dropping more clock leaves can only
make two documents agree that already agreed on everything else, so this cannot
turn a failure into a pass on any non-clock difference - and the leaf diffs
alongside each verdict are what show that.
"""
import json
import os
import sys
from collections import Counter

DRIVERS = ('/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/'
           'wf_960e7225-201-2/docs/experiments/parallel-compression-schedule/'
           'drivers')
DST = ('/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/'
       'wf_960e7225-201-2/docs/experiments/parallel-compression-schedule/'
       'evidence')
sys.path.insert(0, DRIVERS)
import lib  # noqa: E402

SRC = '/var/lib/t3/tmp/pl34'


def leaves(node, path='', out=None):
    if out is None:
        out = {}
    if isinstance(node, dict):
        for k, v in node.items():
            leaves(v, f'{path}/{k}', out)
    elif isinstance(node, list):
        for i, v in enumerate(node):
            leaves(v, f'{path}/{i}', out)
    else:
        out[path] = node
    return out


def leaf_diff(a, b):
    la, lb = leaves(lib.strip_volatile(a)), leaves(lib.strip_volatile(b))
    keys = set(la) | set(lb)
    diff = sorted(k for k in keys if la.get(k) != lb.get(k))
    return diff


# ---- 1. The four pinned gates on four binaries -----------------------------
binaries = ['base-jagua', 'mine-jagua', 'mine-csched', 'mine-parallel']
tags = ['g1', 'g2', 'g3', 'g4']
gate_docs, gate_checks = {}, {}
for b in binaries:
    stored = json.load(open(f'{SRC}/gates/{b}/gates-{b}.json'))
    gate_checks[b] = {t: stored['gates'][t]['hit'] for t in tags}
    gate_docs[b] = {t: json.load(open(f'{SRC}/gates/{b}/{b}-{t}.json'))
                    for t in tags}
digests = {b: {t: lib.doc_digest(gate_docs[b][t]) for t in tags}
           for b in binaries}
ref = digests['base-jagua']
gate_diffs = {b: {t: len(leaf_diff(gate_docs['base-jagua'][t], gate_docs[b][t]))
                  for t in tags} for b in binaries}
json.dump({
    'note': 'the four pinned regression gates on every binary this round '
            'produced. `mine-parallel` is the ARMED build run with an unarmed '
            'spec: the gates are modes 20 and 22 and never reach mode 34, so an '
            'armed build must still be the shipped engine here. Digests are '
            'recomputed with this round\'s repaired `doc_digest` (see '
            'drivers/lib.py) and the leaf diff against base-jagua is reported '
            'next to each, so a match is a match on leaves and not only on a '
            'hash.',
    'pinned': {'g1': [206.869, '8a7737381238fa4d'],
               'g2': [159.09233022733062, 'fa01012af1d559ae'],
               'g3': [159.07876040364795, 'e28fba007f8031d4'],
               'g4': [164.0375677990678, '49f094d7e59a9008']},
    'allPinnedValuesHit': {b: all(gate_checks[b].values()) for b in binaries},
    'perGateHit': gate_checks,
    'docDigestsMatchBaseJagua': {b: digests[b] == ref for b in binaries},
    'differingLeavesVsBaseJagua': gate_diffs,
    'docDigests': digests,
}, open(f'{DST}/gates.json', 'w'), indent=1)
print('gates: allPass',
      {b: all(gate_checks[b].values()) for b in binaries},
      'digestsMatch', {b: digests[b] == ref for b in binaries},
      'leafDiffs', gate_diffs)

# ---- 2. Cross-process determinism, work mode ------------------------------
det = json.load(open(f'{SRC}/determinism2/determinism.json'))
arms = ['serial', 'lanes8', 'pconfirm', 'both']
cells = []
for arm in arms:
    for seed in (0, 1, 2):
        docs = [json.load(open(f'{SRC}/determinism2/{arm}-s{seed}-r{i}.json'))
                for i in range(3)]
        dg = [lib.doc_digest(d) for d in docs]
        pops = [((d.get('relaxedDiagnostics') or {})
                 .get('coupledDynamicSeparator') or {}).get(
                     'persistentVacancyPopulation') or {} for d in docs]
        cells.append({
            'arm': arm, 'seed': seed, 'processes': 3,
            'distinctDocDigests': len(set(dg)),
            'docDigest': dg[0],
            'reproducible': len(set(dg)) == 1,
            'distinctDepths': len({p.get('rawSourceDepthMm') for p in pops}),
            'rawSourceDepthMm': pops[0].get('rawSourceDepthMm'),
            'distinctFingerprints': len(
                {p.get('finalPlacementFingerprint') for p in pops}),
            'maxLeafDiffAcrossProcesses': max(
                len(leaf_diff(docs[0], docs[i])) for i in (1, 2)),
        })
cross = []
for seed in (0, 1, 2):
    base = json.load(open(f'{SRC}/determinism2/serial-s{seed}-r0.json'))
    row = {'seed': seed}
    for arm in ('pconfirm', 'lanes8', 'both'):
        other = json.load(open(f'{SRC}/determinism2/{arm}-s{seed}-r0.json'))
        diff = leaf_diff(base, other)
        row[arm] = {
            'differingLeavesVsSerial': len(diff),
            'leafNames': dict(Counter(k.rsplit('/', 1)[-1]
                                      for k in diff).most_common(4)),
        }
    cross.append(row)
json.dump({
    'note': 'the hard gate of Grok action 2: work-budget mode, bit-reproducible '
            'across two processes. Four arms x three pinned parents x three '
            'processes at work=3341379. Digests recomputed with the repaired '
            '`doc_digest`; `maxLeafDiffAcrossProcesses` is the direct check '
            'behind each verdict.',
    'firstRunFinding': 'the first pass reported 3 distinct digests for EVERY '
                       'cell including the serial shipped schedule, which is '
                       'deterministic by construction. The only leaves that '
                       'moved were repairMs and confirmationMs - the '
                       'compression-schedule round\'s own wall-clock '
                       'decomposition, never added to VOLATILE. The serial '
                       'control is what caught it.',
    'ALL_REPRODUCIBLE': all(c['reproducible'] for c in cells),
    'cells': cells,
    'semanticsPreservationVsSerial': cross,
    'arms': det['arms'],
}, open(f'{DST}/determinism.json', 'w'), indent=1)
print('determinism ALL_REPRODUCIBLE', all(c['reproducible'] for c in cells))
print('  pconfirm leaf diff vs serial:',
      [r['pconfirm']['differingLeavesVsSerial'] for r in cross])
print('  lanes8   leaf diff vs serial:',
      [r['lanes8']['differingLeavesVsSerial'] for r in cross])

# ---- 3. HEAD parity on the coordinator path -------------------------------
hp = json.load(open(f'{SRC}/headparity2/headparity.json'))
cells = []
for seed in (0, 1, 2):
    a = json.load(open(f'{SRC}/headparity2/head-s{seed}.json'))
    b = json.load(open(f'{SRC}/headparity2/armedUnarmedSpec-s{seed}.json'))
    diff = leaf_diff(a, b)
    cells.append({
        'seed': seed,
        'docDigestHead': lib.doc_digest(a),
        'docDigestArmedUnarmedSpec': lib.doc_digest(b),
        'docDigestsMatch': lib.doc_digest(a) == lib.doc_digest(b),
        'differingLeaves': len(diff),
        'rawDepthMm': {
            'head': (a.get('portfolio') or {}).get('incumbent', {}).get('rawDepthMm'),
            'armedUnarmedSpec': (b.get('portfolio') or {}).get('incumbent', {}).get('rawDepthMm'),
        },
    })
json.dump({
    'note': 'the armed build, with an unarmed spec, against HEAD 65f6fc9 on the '
            'v3 coordinator path from the bare request - the path the four '
            'pinned gates do NOT cover, because they are modes 20 and 22 and '
            'never schedule mode 34. Budget is work=40000000 rather than wall, '
            'because a wall-budgeted coordinator run is not reproducible across '
            'processes even on one binary and a wall comparison could not tell '
            '"the build differs" from "the box was busy".',
    'headBinarySha256': hp['headSha256'],
    'armedBinarySha256': hp['armedSha256'],
    'ALL_MATCH': all(c['docDigestsMatch'] for c in cells),
    'cells': cells,
}, open(f'{DST}/headparity.json', 'w'), indent=1)
print('headparity ALL_MATCH', all(c['docDigestsMatch'] for c in cells),
      'leafDiffs', [c['differingLeaves'] for c in cells])
