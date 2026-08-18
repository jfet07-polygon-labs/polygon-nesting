#!/usr/bin/env python3
"""The full certification battery on an incumbent.

Two jobs in one pass:

  1. REPLAY VALIDATION - modes 27, 30 and 22 seeds 0-3 must all come back
     exactValid AND contractValid and reproduce the pin's own raw depth and
     placement fingerprint. That is the pin-is-real check.
  2. FIXPOINT CLAIM - mode 22 seeds 0-3, mode 26 ladders x6, mode 31 tiny steps
     x4, mode 27, mode 30, and the whole frontier-flatten delta grid handed to
     mode 33 under BOTH ladder generations (the base-commit binary and the
     finer-ladder binary). Nothing below the incumbent anywhere is the claim.

Usage: certify_full.py <pin> <raw> [label]
"""
import math
import sys, json, hashlib, os, collections, time
sys.path.insert(0, '/var/lib/t3/tmp/orient-fine')
import drv, lib

PIN, RAW = sys.argv[1], float(sys.argv[2])
LABEL = sys.argv[3] if len(sys.argv) > 3 else 'cert'
LOG = f'/var/lib/t3/tmp/orient-fine/{LABEL}.log'
RUNS = f'/var/lib/t3/tmp/orient-fine/{LABEL}-runs'
FIX = f'/var/lib/t3/tmp/orient-fine/{LABEL}-fix'
os.makedirs(RUNS, exist_ok=True)
os.makedirs(FIX, exist_ok=True)

BASE_BIN = os.environ.get('CERT_BASE_BIN', '/var/lib/t3/tmp/orient-fine/bench-base')
NEW_BIN = os.environ.get('CERT_NEW_BIN', '/var/lib/t3/tmp/orient-fine/bench-new')
FLAT = (0.0005, 0.001, 0.002, 0.003, 0.004, 0.01)
SLACK = (0.05, 2.0)
M31 = (0.006, 0.012, 0.025, 0.04)
DROPS = (0.3, 0.55, 1.0)

sha = hashlib.sha256(open(PIN, 'rb').read()).hexdigest()
fingerprint = json.load(open(PIN))['expectedPlacementFingerprint']
drv.log(LOG, f'=== CERTIFY {PIN} raw={RAW!r} fp={fingerprint} sha256={sha}')

replay, probes = [], []
rungs = collections.Counter()


def probe(tag, mode, parent, target, seed, binary, ladder):
    lib.BIN = binary
    out = drv.go(tag, mode, parent, target, seed, LOG, outdir=RUNS)
    pop = lib.population(out) or {}
    attr = dict(drv.attribution(out))
    angles = attr.pop('acceptedAngles', [])
    for (_pid, _abs, dd, mirror) in angles:
        key = ('mirror' if mirror else 'rot') + (f':{abs(dd):.7g}' if dd is not None else ':?')
        rungs[key] += 1
    published = drv.published_raw(out)
    # Strict raw comparison, no decimal epsilon: RAW is the publication-authority
    # measure, so anything the authority measures strictly below it is a real
    # improvement. (The old `RAW - 1e-12` hid ~35 f64 ULPs at this magnitude.)
    row = {'tag': tag, 'mode': mode, 'ladder': ladder, 'published': published,
           'below': published is not None and published < RAW,
           'exactValid': pop.get('exactValid'), 'contractValid': pop.get('contractValid'),
           'raw': pop.get('rawSourceDepthMm'),
           'fp': pop.get('finalPlacementFingerprint'), 'attribution': attr}
    probes.append(row)
    if row['below']:
        drv.log(LOG, f'!!! BELOW INCUMBENT {tag}: {published!r}')
    return row


t0 = time.time()
# 1. Replay validation.
for mode in (27, 30):
    row = probe(f'{LABEL}-replay-m{mode}', mode, PIN, RAW, 0, NEW_BIN, 'new')
    replay.append((f'mode{mode}', row))
for seed in range(4):
    row = probe(f'{LABEL}-replay-m22-s{seed}', 22, PIN, RAW + 0.8, seed, NEW_BIN, 'new')
    replay.append((f'mode22-seed{seed}', row))

replay_ok = True
for name, row in replay:
    # The parent-measure path (modes 22/27 returning the incumbent) and the
    # publication-measure path (a mode re-publishing the same placements) round
    # one ULP apart on identical layouts; identical fingerprint plus raw within
    # one ULP of the declared (publication-authority) measure is a reproduction.
    match = (row['fp'] == fingerprint
             and row['raw'] is not None
             and abs(row['raw'] - RAW) <= math.ulp(RAW))
    replay_ok &= bool(row['exactValid']) and bool(row['contractValid']) and match
    drv.log(LOG, f'   REPLAY {name}: exactValid={row["exactValid"]} '
                 f'contractValid={row["contractValid"]} raw={row["raw"]!r} '
                 f'fp={(row["fp"] or "")[:16]} reproduces={match}')

# 2. Fixpoint battery.
for step in M31:
    probe(f'{LABEL}-m31-e{step}', 31, PIN, RAW - step, 0, NEW_BIN, 'new')
for delta in FLAT:
    path, depth, moved = drv.flatten_fixture(delta, PIN, LABEL, outdir=FIX)
    drv.log(LOG, f'-- flatten {delta}: depth {depth:.9f}, {len(moved)} moved')
    for binary, ladder in ((NEW_BIN, 'new'), (BASE_BIN, 'base')):
        for slack in SLACK:
            probe(f'{LABEL}-{ladder}-flat{delta}-m33-p{slack}', 33, path,
                  RAW + slack, 0, binary, ladder)
for drop in DROPS:
    for seed in (0, 1):
        probe(f'{LABEL}-m26-d{drop}-s{seed}', 26, PIN, RAW - drop, seed, NEW_BIN, 'new')

below = [row for row in probes if row['below']]
result = {
    'pin': PIN, 'raw': RAW, 'fingerprint': fingerprint, 'sha256': sha,
    'replayPass': bool(replay_ok),
    'replay': [(name, row) for name, row in replay],
    'probeArms': len(probes), 'belowIncumbent': len(below), 'below': below,
    'rungs': dict(rungs), 'elapsedS': time.time() - t0,
    'fixpoint': bool(replay_ok) and not below,
    'probes': probes,
}
json.dump(result, open(f'/var/lib/t3/tmp/orient-fine/{LABEL}.json', 'w'), indent=1)
drv.log(LOG, f'=== CERTIFY replayPass={replay_ok} arms={len(probes)} '
             f'below={len(below)} fixpoint={result["fixpoint"]} '
             f'({result["elapsedS"]:.0f}s)')
drv.log(LOG, '    rungs ' + json.dumps(dict(rungs), sort_keys=True))
