#!/usr/bin/env python3
"""Assemble the finer-ladder experiment's summary.json from the run artefacts."""
import sys, json, os, hashlib, collections
sys.path.insert(0, '/var/lib/t3/tmp/orient-fine')
import lib

OUT = '/var/lib/t3/tmp/orient-fine'


def load(name, default=None):
    path = f'{OUT}/{name}'
    if not os.path.exists(path):
        return default
    return json.load(open(path))


RECORD_PIN = ('/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/'
              'wf_6d9d36cd-45f-1/docs/experiments/persistent-vacancy-descent/'
              'exact-contract/true-contract/finer-ladder/pinned-parent-159.079.json')
OLD_PIN = ('/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/'
           'wf_6d9d36cd-45f-1/docs/experiments/persistent-vacancy-descent/'
           'exact-contract/true-contract/orientation-entry/pinned-parent-159.083.json')


def angle_delta(a, b):
    d = (b - a) % 360.0
    return d - 360.0 if d > 180.0 else d


def geodiff(first_path, second_path):
    first = {p['pieceId']: p for p in json.load(open(first_path))['placements']}
    second = {p['pieceId']: p for p in json.load(open(second_path))['placements']}
    rot, mirror, moved = [], 0, 0
    for pid, a in first.items():
        b = second[pid]
        delta = angle_delta(a['rotationDeg'], b['rotationDeg'])
        if abs(delta) > 1e-9:
            rot.append({'pieceId': pid, 'fromDeg': a['rotationDeg'],
                        'toDeg': b['rotationDeg'], 'deltaDeg': delta})
        if a['mirrored'] != b['mirrored']:
            mirror += 1
        if (abs(a['translateShortAxis'] - b['translateShortAxis']) > 1e-9
                or abs(a['translateLongAxis'] - b['translateLongAxis']) > 1e-9):
            moved += 1
    return {'pieces': len(first), 'rotationChanges': len(rot),
            'maxAbsRotationDeltaDeg': max((abs(r['deltaDeg']) for r in rot), default=0.0),
            'mirrorFlips': mirror, 'translatedPieces': moved, 'rotated': rot}


state = load('campaign-state.json', {})
result = load('campaign-result.json', {})
cert = load('cert.json', {})
fs = load('fs-new.json', {})
gates = load('gates/gates3-new.json', {})
ab_lin = load('abarm-lin-flat0.001.json', {})
ab_fs = load('abarm-fs-flat0.001.json', {})
ab_base = load('ab33-base.json', [])
ab_new = load('ab33-new.json', [])

pin = result.get('pin')
summary = {
    'milestone': (
        'NEW ABSOLUTE RECORD on the true 5.0/5.0 exact-clearance contract: '
        '159.078760 mm raw, from 159.082637. Two rungs added to the bottom of '
        'the orientation ladder (0.008 and 0.0032 degrees, same 5/2 ratio) and '
        'every one of the three adoptions is an orientation acceptance at one '
        'of the two NEW rungs - the accepted-rung distribution moved off the '
        'old floor entirely (27 acceptances at 0.0032, 13 at 0.008, none at '
        '0.02 or coarser across 110 campaign arms). The incumbent is a '
        'certified fixpoint of 40 further probe arms including mode 26.'),
    'request': lib.REQ.split('/tests/')[-1],
    'requestSha256': lib.REQ_SHA,
    'searchOffsetAllowanceMm': float(lib.ALLOWANCE),
    'machine': 'x86_64, 16 cores, runs pinned at 8 threads',
    'binarySha256': hashlib.sha256(open(f'{OUT}/bench-new', 'rb').read()).hexdigest(),
    'baseBinarySha256': hashlib.sha256(open(f'{OUT}/bench-base', 'rb').read()).hexdigest(),
    'ladder': {
        'base': [0.02, 0.05, 0.125, 0.3125, 0.78125, 1.953125, 4.8828125],
        'new': [0.0032, 0.008, 0.02, 0.05, 0.125, 0.3125, 0.78125, 1.953125, 4.8828125],
        'variants': {'base': 29, 'new': 37},
    },
    'gates': gates,
    'campaign': {
        'startRaw': 159.08263749731248,
        'startPin': ('docs/experiments/persistent-vacancy-descent/exact-contract/'
                     'true-contract/orientation-entry/pinned-parent-159.083.json'),
        'finalRaw': result.get('raw'),
        'finalPin': pin,
        'fixpoint': result.get('fixpoint'),
        'arms': state.get('arms'),
        'byTier': state.get('byTier'),
        'rungsSeenAcrossAllArms': state.get('rungs'),
        'rungsOnAdoptions': state.get('adoptRungs'),
        'attribution': state.get('attribution'),
        'trajectory': [{'via': a['tag'], 'tier': a['tier'],
                        'rawSourceDepthMm': a['to'], 'deltaMm': a['delta'],
                        'rungs': a['rungs'],
                        'acceptedAngles': a['acceptedAngles'],
                        'attribution': a['attribution']}
                       for a in state.get('adoptions', [])],
    },
    'pinnedParent': {
        'file': 'pinned-parent-159.079.json',
        'rawSourceDepthMm': result.get('raw'),
        'independentDepthMm': json.load(open(RECORD_PIN))['independentDepthMm'],
        'placementFingerprint': json.load(open(RECORD_PIN))['expectedPlacementFingerprint'],
        'fixtureSha256': hashlib.sha256(open(RECORD_PIN, 'rb').read()).hexdigest(),
    },
    'geometricDiffAgainstOldRecord': geodiff(OLD_PIN, RECORD_PIN),
    'certification': {k: cert.get(k) for k in
                      ('replayPass', 'probeArms', 'belowIncumbent', 'fixpoint',
                       'rungs', 'elapsedS')} if cert else None,
    'abLadderGenerations': {
        'decisiveArm': {
            'fixture': 'frontier flatten 0.001 of the 159.083624 lineage pin',
            'mode': 33,
            'outcomeIdentical': ab_lin.get('outcomeIdentical'),
            'base': {k: ab_lin.get('base', {}).get(k) for k in
                     ('published', 'exactValid', 'attribution', 'rungs',
                      'failureReason')},
            'new': {k: ab_lin.get('new', {}).get(k) for k in
                    ('published', 'exactValid', 'attribution', 'rungs')},
        },
        'recordLineFlattenArms': [
            {'delta': b['delta'], 'baseRaw': b['raw'], 'newRaw': n['raw'],
             'baseFp': b['fp'], 'newFp': n['fp'],
             'identical': b['raw'] == n['raw'] and b['fp'] == n['fp'],
             'baseCandidates': b['attribution'].get('candidates'),
             'newCandidates': n['attribution'].get('candidates'),
             'baseRungs': b['rungs'], 'newRungs': n['rungs']}
            for b, n in zip(ab_base, ab_new)],
    },
    'fromScratchLine': {
        'parentRaw': fs.get('incumbent'),
        'arms': fs.get('arms'),
        'publications': fs.get('publications'),
        'belowIncumbent': fs.get('belowIncumbent'),
        'bestPublished': fs.get('bestPublished'),
        'rungs': fs.get('rungs'),
        'finerRungsUnlockedIt': False,
        'attribution': (
            'The one sub-incumbent arm (frontier flatten 0.001 -> mode 33) is '
            'reproduced bit-identically by the base-commit binary - same raw '
            f'{ab_fs.get("base", {}).get("published")!r}, same fingerprint '
            f'{(ab_fs.get("base", {}).get("fp") or "")[:16]} - and its accepted '
            'orientation pose is a pure mirror flip, a variant the old ladder '
            'already carried. What unlocked the basin is the finer FLATTEN '
            'delta grid (0.001 was never tried on this line; the old grid ran '
            '0.002/0.004/0.01/0.02), not the finer rungs.'),
        'abOutcomeIdentical': ab_fs.get('outcomeIdentical'),
    },
}
json.dump(summary, open(f'{OUT}/summary.json', 'w'), indent=1)
print(json.dumps({k: v for k, v in summary.items()
                  if k not in ('campaign', 'abLadderGenerations')}, indent=1)[:3000])
