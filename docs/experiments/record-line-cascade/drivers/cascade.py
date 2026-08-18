#!/usr/bin/env python3
"""The record-line cascade: every instrument on one state, to fixpoint.

    python3 cascade.py LABEL PIN RAW [ROUNDS]

One round runs the tiers below in order and restarts from the new incumbent the
moment any arm publishes a layout the engine itself validated (exactValid AND
contractValid) whose `rawSourceDepthMm` is **strictly** below the incumbent's
declared raw. Strict `<`, no decimal epsilon: the declared raw is the
publication-authority measure and ~35 ULPs of slack at this magnitude is a real
improvement hidden.

  A  mode 22 salted waves, seeds 0-7, target incumbent + 0.8
  B  frontier flatten {0.0005 .. 0.01} -> mode 33 at a loose bound
  C  mode 31 tiny-step ratchet
  D  frontier-stack single and pair nudges -> mode 33
  E  mode 34 compression-schedule slices (the port), several budgets and steps
  F  mode 26 short ladders -> the m31 rung that follows them, every third round
  G  mode 23 crossover against the co-state pool, both directions

`CASCADE_POOL` is a colon-separated list of fixtures the crossover tier may
draw parent B from. **The from-scratch line runs with a pool that contains only
from-scratch states**: its whole value is that it reached the depth without
importing record-line placements, and a crossover that pulls a record co-state
in destroys exactly that.
"""
import collections
import hashlib
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import drv  # noqa: E402
import lib  # noqa: E402
import sched  # noqa: E402

LABEL = sys.argv[1]
OUT = f'/var/lib/t3/tmp/recordline/{LABEL}'
LOG = f'{OUT}/cascade.log'
PINS = f'{OUT}/pins'
RUNS = f'{OUT}/runs'
FIX = f'{OUT}/fix'
STATE = f'{OUT}/state.json'
for directory in (OUT, PINS, RUNS, FIX):
    os.makedirs(directory, exist_ok=True)

FLAT = (0.0005, 0.001, 0.002, 0.003, 0.004, 0.01)
SLACK = (0.05, 2.0)
M31 = (0.006, 0.012, 0.025, 0.04)
NUDGE = (0.002, 0.006, 0.012, 0.02)
DROPS = tuple(float(d) for d in
              os.environ.get('CASCADE_DROPS', '1.0,0.55,0.3').split(','))
M26_SEEDS = tuple(int(s) for s in
                  os.environ.get('CASCADE_M26_SEEDS', '0,1').split(','))
M26_EVERY = int(os.environ.get('CASCADE_M26_EVERY', '3'))
CUTS = (0.35, 0.5, 0.65)
# The schedule slices. `past=1` makes it an anytime operator; `work` is in the
# schedule's own conservative currency (~18x the coordinator's meter). The
# `step` entries are grid units: 1 is the 1-micron canonical default, below 1 is
# a sub-grid frontier, which only the proxy scalar can express.
SCHED_SPECS = [
    ('w20-s0.25', 'past=1,work=20000000,step=0.25'),
    ('w20-s1', 'past=1,work=20000000,step=1'),
    ('w20-s0.5', 'past=1,work=20000000,step=0.5'),
    ('w20-s0.1', 'past=1,work=20000000,step=0.1'),
    ('w60-s0.25', 'past=1,work=60000000,step=0.25'),
    ('w20-s0.25-c1', 'past=1,work=20000000,step=0.25,confirm=1'),
]
SCHED_SEEDS = (5, 0)
SCHED_EVERY = int(os.environ.get('CASCADE_SCHED_EVERY', '4'))
# Mode 22's bound is a slack above the incumbent, not a target: the wave is an
# alternation fixpoint that may climb before it descends. 0.8 is the finer
# ladder's; the other two are this round's, and they are different waves.
M22_SLACK = (0.8, 0.3, 2.0)
POOL = [p for p in os.environ.get('CASCADE_POOL', '').split(':') if p]
DEADLINE = float(os.environ.get('CASCADE_DEADLINE', '1e18'))

STATS = {'arms': 0, 'byTier': collections.defaultdict(lambda: collections.defaultdict(int)),
         'adoptions': [], 'barren': []}


class OutOfTime(Exception):
    pass


def save():
    json.dump({'label': LABEL, 'arms': STATS['arms'],
               'byTier': {k: dict(v) for k, v in STATS['byTier'].items()},
               'adoptions': STATS['adoptions'], 'barren': STATS['barren']},
              open(STATE, 'w'), indent=1)


def judge(tier, tag, out, current_raw):
    """Fold one arm into the tallies; adopt it if it is strictly deeper."""
    STATS['arms'] += 1
    STATS['byTier'][tier]['arms'] += 1
    pop = lib.population(out) or {}
    if pop.get('exactValid') and pop.get('contractValid'):
        STATS['byTier'][tier]['publications'] += 1
    raw = drv.published_raw(out)
    if raw is None or not raw < current_raw:
        return None
    pin = f'{PINS}/pin-{raw:.9f}.json'
    lib.pin(out, pin, f'{LABEL} cascade adoption {tag}')
    sha = hashlib.sha256(open(pin, 'rb').read()).hexdigest()
    drv.log(LOG, f'*** ADOPT [{tier}] {tag}: {current_raw!r} -> {raw!r} '
                 f'(delta {raw - current_raw:+.9f})')
    drv.log(LOG, f'    pin={pin} sha256={sha} '
                 f'fp={pop["finalPlacementFingerprint"]}')
    STATS['byTier'][tier]['adoptions'] += 1
    STATS['adoptions'].append({
        'tier': tier, 'tag': tag, 'from': repr(current_raw), 'to': repr(raw),
        'delta': raw - current_raw, 'pin': pin, 'sha256': sha,
        'fingerprint': pop['finalPlacementFingerprint']})
    save()
    return pin, raw


def arm(tag, tier, mode, parent, target, seed, current_raw, warm=''):
    if time.time() > DEADLINE:
        raise OutOfTime(tag)
    out = drv.go(tag, mode, parent, target, seed, LOG, outdir=RUNS)
    if warm:
        # drv.go cannot pass a warm start; mode 23 needs one, so it is run here.
        raise AssertionError('use cross_arm for mode 23')
    return judge(tier, tag, out, current_raw)


def cross_arm(tag, parent_a, parent_b, cut, seed, current_raw):
    if time.time() > DEADLINE:
        raise OutOfTime(tag)
    t0 = time.time()
    out = lib.run(tag, 23, parent_a, f'{cut:.6f}', seed, RUNS, warm=parent_b)
    drv.log(LOG, f'[{time.time() - t0:7.1f}s] ' + lib.line(tag, out))
    return judge('G-cross', tag, out, current_raw)


def sched_arm(tag, spec, parent, target, seed, current_raw):
    if time.time() > DEADLINE:
        raise OutOfTime(tag)
    out, _ = sched.sched_arm(tag, parent, target, seed, spec, logfile=LOG,
                             outdir=RUNS)
    return judge('E-m34', tag, out, current_raw)


def round_once(pin, raw, rnd):
    tried = []
    # The tier order is measured, not designed. Cheap-and-sometimes (m22, the
    # flatten grid, m31: 2-4 s an arm) run first so a barren pass costs about
    # two minutes; mode 26 (44-88 s an arm) runs next, because the 156.091
    # certification found six of six mode-26 arms strictly below the incumbent,
    # the best by 0.628 mm, while the cheap tiers had been grinding 0.001 mm a
    # round - the first ordering starved the most productive instrument for 555
    # arms; mode 34 (23-70 s) runs last of the descent tiers because it is inert
    # on any state it did not itself produce (see the README's §4).
    # A. mode 22 salted waves, over the seed salt and the bound slack.
    for slack in M22_SLACK:
        for seed in range(8):
            tried.append(('A-m22', f'r{rnd}-m22-p{slack}-s{seed}'))
            got = arm(f'r{rnd}-m22-p{slack}-s{seed}', 'A-m22', 22, pin,
                      raw + slack, seed, raw)
            if got:
                return got, tried
    # B'. flatten grid and m31, hoisted above mode 26 for the same reason.
    for delta in FLAT:
        path, depth, moved = drv.flatten_fixture(delta, pin, f'{LABEL}-r{rnd}',
                                                 outdir=FIX)
        for slack in SLACK:
            tried.append(('B-flat', f'r{rnd}-flat{delta}-m33-p{slack}'))
            got = arm(f'r{rnd}-flat{delta}-m33-p{slack}', 'B-flat', 33, path,
                      raw + slack, 0, raw)
            if got:
                return got, tried
    for step in M31:
        tried.append(('C-m31', f'r{rnd}-m31-e{step}'))
        got = arm(f'r{rnd}-m31-e{step}', 'C-m31', 31, pin, raw - step, 0, raw)
        if got:
            return got, tried
    # F. mode 26 ladders.
    if rnd % M26_EVERY == 0:
        for drop in DROPS:
            for seed in M26_SEEDS:
                tried.append(('F-m26', f'r{rnd}-m26-d{drop}-s{seed}'))
                got = arm(f'r{rnd}-m26-d{drop}-s{seed}', 'F-m26', 26, pin,
                          raw - drop, seed, raw)
                if got:
                    return got, tried
    # E. mode 34 compression-schedule slices. It is the instrument that moved
    # this line 164.038 -> 159.668 -> 158.668 -> 157.484, so it runs before the
    # repair tiers - but only every `SCHED_EVERY` rounds. Measured reason: a
    # state the schedule itself produced arrives proxy-*feasible* and the
    # schedule ratchets on it, while a state modes 22/33 produced arrives with
    # 28-38 colliding pairs after the 2.5-degree entry snap and the schedule
    # confirms nothing at all. Twelve 25-70 s arms per round against a tier that
    # publishes 0.005 mm in 40 s is the wrong trade every round but the ones
    # right after a schedule adoption.
    if rnd % SCHED_EVERY == 0:
        for seed in SCHED_SEEDS:
            for tag, spec in SCHED_SPECS:
                tried.append(('E-m34', f'r{rnd}-m34-{tag}-s{seed}'))
                got = sched_arm(f'r{rnd}-m34-{tag}-s{seed}', spec, pin,
                                raw - 0.3, seed, raw)
                if got:
                    return got, tried
    # B. flatten grid -> mode 33.
    for delta in FLAT:
        path, depth, moved = drv.flatten_fixture(delta, pin, f'{LABEL}-r{rnd}',
                                                 outdir=FIX)
        drv.log(LOG, f'-- r{rnd} flatten {delta}: depth {depth:.9f}, '
                     f'{len(moved)} moved')
        for slack in SLACK:
            tried.append(('B-flat', f'r{rnd}-flat{delta}-m33-p{slack}'))
            got = arm(f'r{rnd}-flat{delta}-m33-p{slack}', 'B-flat', 33, path,
                      raw + slack, 0, raw)
            if got:
                return got, tried
    # C. mode 31 tiny steps.
    for step in M31:
        tried.append(('C-m31', f'r{rnd}-m31-e{step}'))
        got = arm(f'r{rnd}-m31-e{step}', 'C-m31', 31, pin, raw - step, 0, raw)
        if got:
            return got, tried
    # D. frontier-stack nudges -> mode 33.
    ranked = drv.ranked_extents(pin)
    for rank in (1, 2, 3):
        for delta in NUDGE:
            path, _ = drv.single_nudge_fixture(
                [ranked[rank - 1][1]], delta, pin,
                f'{LABEL}-r{rnd}-r{rank}-d{delta}', outdir=FIX)
            tried.append(('D-nudge', f'r{rnd}-nudge-r{rank}-d{delta}-m33'))
            got = arm(f'r{rnd}-nudge-r{rank}-d{delta}-m33', 'D-nudge', 33, path,
                      raw + 2.0, 0, raw)
            if got:
                return got, tried
    for delta in NUDGE:
        path, _ = drv.single_nudge_fixture(
            [ranked[0][1], ranked[1][1]], delta, pin,
            f'{LABEL}-r{rnd}-pair-d{delta}', outdir=FIX)
        tried.append(('D-nudge', f'r{rnd}-nudge-pair-d{delta}-m33'))
        got = arm(f'r{rnd}-nudge-pair-d{delta}-m33', 'D-nudge', 33, path,
                  raw + 2.0, 0, raw)
        if got:
            return got, tried
    # G. mode 23 crossover, both directions.
    for other in POOL:
        if os.path.abspath(other) == os.path.abspath(pin):
            continue
        name = os.path.basename(other).replace('.json', '')[:28]
        for cut in CUTS:
            for a, b, side in ((pin, other, 'ab'), (other, pin, 'ba')):
                tried.append(('G-cross', f'r{rnd}-x{side}-{name}-c{cut}'))
                got = cross_arm(f'r{rnd}-x{side}-{name}-c{cut}', a, b, cut, 0,
                                raw)
                if got:
                    return got, tried
    return None, tried


if __name__ == '__main__':
    pin, raw = sys.argv[2], float(sys.argv[3])
    max_rounds = int(sys.argv[4]) if len(sys.argv) > 4 else 30
    drv.log(LOG, f'=== CASCADE {LABEL} from {pin} raw={raw!r} ===')
    drv.log(LOG, f'    pool: {POOL}')
    fixpoint = False
    try:
        for rnd in range(max_rounds):
            t0 = time.time()
            got, tried = round_once(pin, raw, rnd)
            if not got:
                drv.log(LOG, f'=== FIXPOINT at raw={raw!r} after round {rnd} '
                             f'({time.time() - t0:.0f}s, {len(tried)} arms) ===')
                STATS['barren'].append({'round': rnd, 'raw': repr(raw),
                                        'pin': pin, 'arms': len(tried),
                                        'seconds': time.time() - t0})
                fixpoint = True
                break
            pin, raw = got
    except OutOfTime as stop:
        drv.log(LOG, f'=== OUT OF TIME at {stop} ===')
    drv.log(LOG, f'=== RESULT {LABEL} pin={pin} raw={raw!r} '
                 f'fixpoint={fixpoint} arms={STATS["arms"]} ===')
    save()
    json.dump({'label': LABEL, 'pin': pin, 'raw': repr(raw),
               'rawFloat': raw, 'fixpoint': fixpoint, 'arms': STATS['arms']},
              open(f'{OUT}/result.json', 'w'), indent=1)
