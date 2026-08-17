#!/usr/bin/env python3
"""Fixpoint probe on the certified record: everything the cascade round runs
except the mode-26 ladder tier, which costs most of the wall clock and adopted
nothing in four rounds."""
import sys, json
sys.path.insert(0, '/var/lib/t3/tmp/orient')
import drv

LOG = '/var/lib/t3/tmp/orient/fixpoint.log'
PIN = sys.argv[1]
RAW = float(sys.argv[2])
arms = 0
wins = []


def probe(tag, mode, fixture, target, seed=0):
    global arms
    arms += 1
    out = drv.go(tag, mode, fixture, target, seed, LOG)
    published = drv.published_raw(out)
    if published is not None and published < RAW - 1e-12:
        wins.append({'tag': tag, 'mode': mode, 'published': published,
                     'attribution': drv.attribution(out)})
        drv.log(LOG, f'*** BELOW INCUMBENT {tag}: {published!r}')


drv.log(LOG, f'=== FIXPOINT PROBE {PIN} raw={RAW!r}')
for step in (0.006, 0.012, 0.025, 0.04):
    probe(f'fp-m31-e{step}', 31, PIN, RAW - step)
for seed in range(4):
    probe(f'fp-m22-s{seed}', 22, PIN, RAW + 0.8, seed)
for delta in (0.001, 0.002, 0.003, 0.004, 0.005, 0.008, 0.012, 0.02, 0.03):
    path, depth, moved = drv.flatten_fixture(delta, PIN, 'fp')
    for mode in (33, 32, 29, 28):
        for slack in (0.05, 2.0):
            probe(f'fp-flat{delta}-m{mode}-p{slack}', mode, path, RAW + slack)
ranked = drv.ranked_extents(PIN)
for rank in (1, 2, 3, 4):
    for delta in (0.002, 0.006, 0.012, 0.02):
        path, depth = drv.single_nudge_fixture(
            [ranked[rank - 1][1]], delta, PIN, f'fp-r{rank}-d{delta}')
        for mode in (33, 32):
            probe(f'fp-nudge-r{rank}-d{delta}-m{mode}', mode, path, RAW + 2.0)
for delta in (0.002, 0.006, 0.012, 0.02):
    ids = [ranked[0][1], ranked[1][1]]
    path, depth = drv.single_nudge_fixture(ids, delta, PIN, f'fp-pair-d{delta}')
    for mode in (33, 32):
        probe(f'fp-nudge-pair-d{delta}-m{mode}', mode, path, RAW + 2.0)

drv.log(LOG, f'=== FIXPOINT PROBE: {arms} arms, {len(wins)} below the incumbent')
json.dump({'pin': PIN, 'raw': RAW, 'arms': arms, 'wins': wins},
          open('/var/lib/t3/tmp/orient/fixpoint.json', 'w'), indent=1)
