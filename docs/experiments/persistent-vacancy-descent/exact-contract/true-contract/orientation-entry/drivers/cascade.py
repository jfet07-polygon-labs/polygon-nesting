#!/usr/bin/env python3
"""Fail-closed cascade from a pinned state, with modes 32/33 in the round.

Adopts ONLY publications the engine itself validated (exactValid AND
contractValid) whose rawSourceDepthMm is strictly below the incumbent.

Round shape:
  1. mode-31 tiny-step ratchet, steps {0.006, 0.012, 0.025, 0.04}
  2. mode 22 alternation, seeds 0-3, target raw + 0.8
  3. mode 26 short ladders, drops {0.3, 0.55, 1.0}, seeds {0, 1}
  4. frontier flatten -> modes 33/32 (orientation) then 29/28 (control)
  5. frontier-stack single and pair nudges -> modes 33/32
"""
import sys, json, hashlib, os
sys.path.insert(0, '/var/lib/t3/tmp/orient')
import drv, lib

LOG = '/var/lib/t3/tmp/orient/cascade.log'
PINS = '/var/lib/t3/tmp/orient/pins'
os.makedirs(PINS, exist_ok=True)
M31 = (0.006, 0.012, 0.025, 0.04)
DROPS = (0.3, 0.55, 1.0)
FLAT = (0.001, 0.002, 0.003, 0.004, 0.005, 0.008, 0.012, 0.02)
NUDGE = (0.002, 0.006, 0.012, 0.02)
SLACK = (0.05, 2.0)


def adopt(out, tag, current_raw):
    raw = drv.published_raw(out)
    if raw is None or raw >= current_raw - 1e-12:
        return None
    pin = f'{PINS}/pin-{raw:.9f}.json'
    lib.pin(out, pin, f'orientation cascade adoption {tag}')
    sha = hashlib.sha256(open(pin, 'rb').read()).hexdigest()
    drv.log(LOG, f'*** ADOPT {tag}: {current_raw!r} -> {raw!r} '
                 f'(delta {raw - current_raw:+.9f}) pin={pin} sha={sha[:16]}')
    drv.log(LOG, '    attribution ' + json.dumps(drv.attribution(out)))
    return pin, raw


def round_once(pin, raw, rnd):
    for step in M31:
        out = drv.go(f'r{rnd}-m31-e{step}', 31, pin, raw - step, 0, LOG)
        got = adopt(out, f'r{rnd}-m31-e{step}', raw)
        if got:
            return got
    for seed in range(4):
        out = drv.go(f'r{rnd}-m22-s{seed}', 22, pin, raw + 0.8, seed, LOG)
        got = adopt(out, f'r{rnd}-m22-s{seed}', raw)
        if got:
            return got
    for drop in DROPS:
        for seed in (0, 1):
            out = drv.go(f'r{rnd}-m26-d{drop}-s{seed}', 26, pin, raw - drop, seed, LOG)
            got = adopt(out, f'r{rnd}-m26-d{drop}-s{seed}', raw)
            if got:
                return got
    for delta in FLAT:
        path, depth, moved = drv.flatten_fixture(delta, pin, f'c{rnd}')
        for mode in (33, 32, 29, 28):
            for slack in SLACK:
                tag = f'r{rnd}-flat{delta}-m{mode}-p{slack}'
                out = drv.go(tag, mode, path, raw + slack, 0, LOG)
                got = adopt(out, tag, raw)
                if got:
                    return got
    ranked = drv.ranked_extents(pin)
    for rank in (1, 2, 3):
        for delta in NUDGE:
            path, depth = drv.single_nudge_fixture(
                [ranked[rank - 1][1]], delta, pin, f'c{rnd}-r{rank}-d{delta}')
            for mode in (33, 32):
                tag = f'r{rnd}-nudge-r{rank}-d{delta}-m{mode}'
                out = drv.go(tag, mode, path, raw + 2.0, 0, LOG)
                got = adopt(out, tag, raw)
                if got:
                    return got
    for delta in NUDGE:
        ids = [ranked[0][1], ranked[1][1]]
        path, depth = drv.single_nudge_fixture(ids, delta, pin, f'c{rnd}-pair-d{delta}')
        for mode in (33, 32):
            tag = f'r{rnd}-nudge-pair-d{delta}-m{mode}'
            out = drv.go(tag, mode, path, raw + 2.0, 0, LOG)
            got = adopt(out, tag, raw)
            if got:
                return got
    return None


if __name__ == '__main__':
    pin, raw = sys.argv[1], float(sys.argv[2])
    max_rounds = int(sys.argv[3]) if len(sys.argv) > 3 else 40
    drv.log(LOG, f'=== CASCADE from {pin} raw={raw!r} ===')
    for rnd in range(max_rounds):
        got = round_once(pin, raw, rnd)
        if not got:
            drv.log(LOG, f'=== FIXPOINT at raw={raw!r} after {rnd} full rounds ===')
            break
        pin, raw = got
    drv.log(LOG, f'=== CASCADE RESULT pin={pin} raw={raw!r} ===')
