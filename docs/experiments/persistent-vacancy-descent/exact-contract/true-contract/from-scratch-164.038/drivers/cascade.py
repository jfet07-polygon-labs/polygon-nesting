#!/usr/bin/env python3
"""Fail-closed cascade from a pinned state.

Adopts ONLY publications the engine itself validated (exactValid AND
contractValid) whose rawSourceDepthMm is strictly below the incumbent.

Round shape (protocol):
  1. mode-31 tiny-step ratchet, steps {0.006, 0.012, 0.025, 0.04}
  2. mode 22 alternation, seeds 0-3, target raw + 0.8
  3. mode 26 short ladders, drops {0.3, 0.55, 1.0}, seeds {0, 1}
  4. NEW mechanism: frontier-flatten -> modes 29/28 joint/conflict re-placement
"""
import sys, json, hashlib
sys.path.insert(0, '/var/lib/t3/tmp/combo28-fs')
import drv, lib

L = drv.OUT + '/cascade.log'
M31 = (0.006, 0.012, 0.025, 0.04)
DROPS = (0.3, 0.55, 1.0)
FLAT = (0.001, 0.002, 0.003, 0.004, 0.005, 0.008)
SLACK = (0.05, 0.5)

def adopt(out, tag, cur_raw):
    r = drv.published_raw(out)
    if r is None or r >= cur_raw - 1e-12:
        return None
    pin = f'{drv.OUT}/pins/pin-{r:.6f}.json'
    lib.pin(out, pin, f'cascade adoption {tag}')
    sha = hashlib.sha256(open(pin, 'rb').read()).hexdigest()
    drv.log(L, f'*** ADOPT {tag}: {cur_raw!r} -> {r!r} (delta {r-cur_raw:+.9f}) pin={pin} sha={sha[:16]}')
    return pin, r

def round_once(pin, raw, rnd):
    for step in M31:
        out = drv.go(f'r{rnd}-m31-e{step}', 31, pin, raw - step, 0, L)
        got = adopt(out, f'r{rnd}-m31-e{step}', raw)
        if got: return got
    for seed in range(4):
        out = drv.go(f'r{rnd}-m22-s{seed}', 22, pin, raw + 0.8, seed, L)
        got = adopt(out, f'r{rnd}-m22-s{seed}', raw)
        if got: return got
    for drop in DROPS:
        for seed in (0, 1):
            out = drv.go(f'r{rnd}-m26-d{drop}-s{seed}', 26, pin, raw - drop, seed, L)
            got = adopt(out, f'r{rnd}-m26-d{drop}-s{seed}', raw)
            if got: return got
    for delta in FLAT:
        fp, fdepth, n = drv.flatten_fixture(delta, parent=pin)
        for mode in (29, 28):
            for slack in SLACK:
                tag = f'r{rnd}-flat{delta}-m{mode}-p{slack}'
                out = drv.go(tag, mode, fp, raw + slack, 0, L)
                got = adopt(out, tag, raw)
                if got: return got
    return None

if __name__ == '__main__':
    pin, raw = sys.argv[1], float(sys.argv[2])
    max_rounds = int(sys.argv[3]) if len(sys.argv) > 3 else 40
    drv.log(L, f'=== CASCADE from {pin} raw={raw!r} ===')
    for rnd in range(max_rounds):
        got = round_once(pin, raw, rnd)
        if not got:
            drv.log(L, f'=== FIXPOINT at raw={raw!r} after {rnd} full rounds ===')
            break
        pin, raw = got
    drv.log(L, f'=== CASCADE RESULT pin={pin} raw={raw!r} ===')
