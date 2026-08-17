#!/usr/bin/env python3
"""Replay-validate a pinned state: modes 27, 30 and 22 seeds 0-3.

Mode 27 (micro-legalization probe) and mode 30 (global pressure-balanced
legalization) both replay the parent through the authoritative validator; mode
22 re-derives the same raw depth and fingerprint from a loose target. A pin that
is real reproduces its own raw depth and fingerprint on every arm.
"""
import sys, json, hashlib
sys.path.insert(0, '/var/lib/t3/tmp/orient')
import drv

LOG = '/var/lib/t3/tmp/orient/certify.log'
pin, raw = sys.argv[1], float(sys.argv[2])
sha = hashlib.sha256(open(pin, 'rb').read()).hexdigest()
fingerprint = json.load(open(pin))['expectedPlacementFingerprint']
drv.log(LOG, f'=== CERTIFY {pin} raw={raw!r} fp={fingerprint} sha256={sha}')

rows = []
for mode in (27, 30):
    out = drv.go(f'cert-m{mode}', mode, pin, raw, 0, LOG)
    pop = drv.lib.population(out) or {}
    rows.append((f'mode{mode}', pop.get('exactValid'), pop.get('contractValid'),
                 pop.get('rawSourceDepthMm'), (pop.get('finalPlacementFingerprint') or '')))
for seed in range(4):
    out = drv.go(f'cert-m22-s{seed}', 22, pin, raw + 0.8, seed, LOG)
    pop = drv.lib.population(out) or {}
    rows.append((f'mode22-seed{seed}', pop.get('exactValid'), pop.get('contractValid'),
                 pop.get('rawSourceDepthMm'), (pop.get('finalPlacementFingerprint') or '')))

ok = True
for name, exact, contract, replay_raw, replay_fp in rows:
    match = (replay_raw == raw and replay_fp == fingerprint)
    ok &= bool(exact) and bool(contract) and match
    drv.log(LOG, f'   {name}: exactValid={exact} contractValid={contract} '
                 f'raw={replay_raw!r} fp={replay_fp[:16]} reproduces={match}')
drv.log(LOG, f'=== CERTIFY {"PASS" if ok else "FAIL"}')
json.dump({'pin': pin, 'raw': raw, 'fingerprint': fingerprint, 'sha256': sha,
           'rows': rows, 'pass': bool(ok)},
          open('/var/lib/t3/tmp/orient/certify.json', 'w'), indent=1)
