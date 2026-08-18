#!/usr/bin/env python3
"""The deliberate interleave: every instrument every round, deferred credit.

    python3 cascade2.py LABEL PIN RAW [ROUNDS] [JOBS]

The record-line round measured that an adopt-and-restart cascade ordered by arm
cost starves whichever tier is expensive: the cheap tiers published 0.001 mm and
restarted the round, so mode 26 was never reached in 555 arms, and hoisting mode
26 to the front starved the cheap tiers symmetrically. Neither pure order works,
so this cascade does not restart on the first improvement. It runs *every* tier
of a round to completion, concurrently, and then adopts the single strictly-best
publication of the whole round - deferred credit. The cost is that a round is
always a full round; the gain is that every instrument is fired against every
incumbent and the per-tier arm counts are a map rather than an artefact of the
ordering.

Adoption is strict `<` on `rawSourceDepthMm` of a publication that is
`exactValid` AND `contractValid`, with no decimal epsilon.
"""
import collections
import concurrent.futures
import hashlib
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import drv  # noqa: E402
import lib  # noqa: E402
import rotentrylib  # noqa: E402
import sched  # noqa: E402

LABEL = sys.argv[1]
OUT = f'/var/lib/t3/tmp/wf87/run/{LABEL}'
LOG = f'{OUT}/cascade.log'
PINS, RUNS, FIX = f'{OUT}/pins', f'{OUT}/runs', f'{OUT}/fix'
for directory in (OUT, PINS, RUNS, FIX):
    os.makedirs(directory, exist_ok=True)

FLAT = tuple(float(x) for x in os.environ.get(
    'C2_FLAT', '0.0005,0.001,0.0015,0.002,0.0025,0.003,0.004,0.005,0.006,'
               '0.008,0.01').split(','))
SLACK = (0.05, 2.0)
# The entry->legalization composition, tier H. Mode 33 rejects an arm in which
# any one violation component refuses to re-place, so a deep flatten throws away
# the components it *did* repair; modes 30 and 31 legalize the whole layout
# under a displacement cap instead of enumerating insertion orders, so they fail
# later on the same entry. The deltas here are an order of magnitude deeper than
# the re-insertion tier's because that is where the composition pays: on the
# 155.4087 fixpoint, flatten 0.1 -> mode 30 published below it while all 22
# re-insertion deltas did not.
LEGAL_FLAT = tuple(float(x) for x in os.environ.get(
    'C2_LEGAL_FLAT', '0.05,0.08,0.1,0.12,0.15,0.2,0.25,0.3,0.4,0.5,0.7,'
                     '1.0').split(','))
LEGAL_MODES = tuple(int(x) for x in
                    os.environ.get('C2_LEGAL_MODES', '30,31,27').split(','))
M31 = (0.006, 0.012, 0.025, 0.04)
NUDGE = (0.002, 0.006, 0.012, 0.02)
# Tier I, the rotation entry. Every other entry family on this line is a
# translation; this one perturbs the frontier pieces' poses in place, by the
# ladder's own rungs, so the orientation degree of freedom becomes reachable
# from *outside* modes 32 and 33 - which matters because their internal ladder
# can only perturb the pieces they themselves ejected, and because a rotated
# entry can then be handed to the legalization tier that §5 found to matter.
ROT_KS = tuple(int(x) for x in os.environ.get('C2_ROT_KS', '1,2,3').split(','))
ROT_DEGS = tuple(float(x) for x in os.environ.get(
    'C2_ROT_DEGS', '0.00128,-0.00128,0.0032,-0.0032,0.02,-0.02').split(','))
ROT_MODES = tuple(int(x) for x in
                  os.environ.get('C2_ROT_MODES', '30,33').split(','))
M22_SLACK = (0.8, 0.3, 2.0)
M26_DROPS = tuple(float(x) for x in
                  os.environ.get('C2_M26_DROPS', '1.0,0.55,0.3').split(','))
M26_SEEDS = tuple(int(x) for x in
                  os.environ.get('C2_M26_SEEDS', '0,1').split(','))
SCHED_EVERY = int(os.environ.get('C2_SCHED_EVERY', '3'))
# Tier frequencies. Deferred credit removes the *ordering* bias but not the
# cost one: a tier that has published nothing below in N consecutive rounds is
# still charged its wall clock every round, and at this depth the entry-grid
# tiers are the ones paying. So the measured-barren tiers are not dropped -
# dropping them is how the previous round lost mode 26 for 555 arms - they are
# run every Nth round, which keeps their negative current and their arm count
# honest while letting the productive tier have the box.
A_EVERY = int(os.environ.get('C2_A_EVERY', '1'))
C_EVERY = int(os.environ.get('C2_C_EVERY', '1'))
M26_EVERY = int(os.environ.get('C2_M26_EVERY', '1'))
CROSS_EVERY = int(os.environ.get('C2_CROSS_EVERY', '1'))
SCHED_SPECS = [
    ('w20-s0.25', 'past=1,work=20000000,step=0.25'),
    ('w20-s0.125', 'past=1,work=20000000,step=0.125'),
    ('w20-s0.0625', 'past=1,work=20000000,step=0.0625'),
    ('w60-s0.125', 'past=1,work=60000000,step=0.125'),
]
SCHED_SEEDS = (5, 0)
CUTS = (0.35, 0.5, 0.65)
POOL = [p for p in os.environ.get('C2_POOL', '').split(':') if p]
DEADLINE = float(os.environ.get('C2_DEADLINE', '1e18'))
JOBS = int(sys.argv[5]) if len(sys.argv) > 5 else 4

STATS = {'arms': 0, 'byTier': collections.defaultdict(
    lambda: collections.defaultdict(int)), 'adoptions': [], 'rounds': []}


def save(pin, raw, fixpoint):
    json.dump({'label': LABEL, 'pin': pin, 'raw': repr(raw), 'rawFloat': raw,
               'arms': STATS['arms'], 'fixpoint': fixpoint,
               'byTier': {k: dict(v) for k, v in STATS['byTier'].items()},
               'adoptions': STATS['adoptions'], 'rounds': STATS['rounds']},
              open(f'{OUT}/state.json', 'w'), indent=1)


# ---- one arm of each kind, all returning the same row shape ----------------

def _row(tier, tag, out, path):
    pop = lib.population(out) or {}
    published = drv.published_raw(out)
    return {'tier': tier, 'tag': tag, 'published': published,
            'publishedRepr': repr(published),
            'rawAny': repr(pop.get('rawSourceDepthMm')),
            'exactValid': pop.get('exactValid'),
            'contractValid': pop.get('contractValid'),
            'fingerprint': pop.get('finalPlacementFingerprint'),
            'attribution': drv.attribution(out), 'run': path}


def plain(tier, tag, mode, parent, target, seed):
    out = drv.go(tag, mode, parent, target, seed, LOG, outdir=RUNS)
    return _row(tier, tag, out, f'{RUNS}/{tag}.json')


def cross(tier, tag, parent_a, parent_b, cut, seed):
    started = time.time()
    out = lib.run(tag, 23, parent_a, f'{cut:.6f}', seed, RUNS, warm=parent_b)
    drv.log(LOG, f'[{time.time() - started:7.1f}s] ' + lib.line(tag, out))
    return _row(tier, tag, out, f'{RUNS}/{tag}.json')


def schedule(tier, tag, spec, parent, target, seed):
    out, _ = sched.sched_arm(tag, parent, target, seed, spec, logfile=LOG,
                             outdir=RUNS)
    return _row(tier, tag, out, f'{RUNS}/{tag}.json')


# ---- the round -------------------------------------------------------------

def jobs_for(pin, raw, rnd):
    """Every arm of one round, as callables. No arm depends on another."""
    out = []
    if rnd % A_EVERY == 0:
        for slack in M22_SLACK:
            for seed in range(8):
                tag = f'r{rnd}-m22-p{slack}-s{seed}'
                out.append((lambda t=tag, p=pin, b=raw + slack, s=seed:
                            plain('A-m22', t, 22, p, b, s)))
    for delta in FLAT:
        path, depth, moved = drv.flatten_fixture(delta, pin, f'{LABEL}-r{rnd}',
                                                 outdir=FIX)
        for slack in SLACK:
            for mode in (33, 32):
                # Mode 32 was the measured-unproductive tier on the 159 basin
                # (0 of 4 sub-record publications against mode 33's 4 of 4). It
                # is not unproductive here - it took 10 of the 25 arms below the
                # incumbent in this round's first pass - so it gets the same
                # grid mode 33 does rather than a single token arm.
                tag = f'r{rnd}-flat{delta}-m{mode}-p{slack}'
                out.append((lambda t=tag, p=path, b=raw + slack, m=mode:
                            plain(f'B-flat-m{m}', t, m, p, b, 0)))
    for delta in LEGAL_FLAT:
        path, depth, moved = drv.flatten_fixture(delta, pin,
                                                 f'{LABEL}-legal-r{rnd}',
                                                 outdir=FIX)
        for mode in LEGAL_MODES:
            tag = f'r{rnd}-legalflat{delta}-m{mode}'
            out.append((lambda t=tag, p=path, b=raw + 0.05, m=mode:
                        plain(f'H-legal-m{m}', t, m, p, b, 0)))
    for k in ROT_KS:
        for degrees in ROT_DEGS:
            path, depth = rotentrylib.rotation_fixture(
                pin, k, degrees, f'{FIX}/rot-{LABEL}-r{rnd}-k{k}-d{degrees}.json')
            for mode in ROT_MODES:
                tag = f'r{rnd}-rot-k{k}-d{degrees}-m{mode}'
                out.append((lambda t=tag, p=path, b=raw + 2.0, m=mode:
                            plain(f'I-rot-m{m}', t, m, p, b, 0)))
    if rnd % C_EVERY == 0:
        for step in M31:
            tag = f'r{rnd}-m31-e{step}'
            out.append((lambda t=tag, p=pin, b=raw - step:
                        plain('C-m31', t, 31, p, b, 0)))
    ranked = drv.ranked_extents(pin)
    for rank in (1, 2, 3):
        for delta in NUDGE:
            path, _ = drv.single_nudge_fixture(
                [ranked[rank - 1][1]], delta, pin,
                f'{LABEL}-r{rnd}-r{rank}-d{delta}', outdir=FIX)
            tag = f'r{rnd}-nudge-r{rank}-d{delta}-m33'
            out.append((lambda t=tag, p=path, b=raw + 2.0:
                        plain('D-nudge', t, 33, p, b, 0)))
    for delta in NUDGE:
        path, _ = drv.single_nudge_fixture(
            [ranked[0][1], ranked[1][1]], delta, pin,
            f'{LABEL}-r{rnd}-pair-d{delta}', outdir=FIX)
        tag = f'r{rnd}-nudge-pair-d{delta}-m33'
        out.append((lambda t=tag, p=path, b=raw + 2.0:
                    plain('D-nudge', t, 33, p, b, 0)))
    if rnd % M26_EVERY == 0:
        for drop in M26_DROPS:
            for seed in M26_SEEDS:
                tag = f'r{rnd}-m26-d{drop}-s{seed}'
                out.append((lambda t=tag, p=pin, b=raw - drop, s=seed:
                            plain('F-m26', t, 26, p, b, s)))
    if rnd % SCHED_EVERY == 0:
        for seed in SCHED_SEEDS:
            for tag, spec in SCHED_SPECS:
                full = f'r{rnd}-m34-{tag}-s{seed}'
                out.append((lambda t=full, sp=spec, p=pin, b=raw - 0.3, s=seed:
                            schedule('E-m34', t, sp, p, b, s)))
    for other in (POOL if rnd % CROSS_EVERY == 0 else []):
        if os.path.abspath(other) == os.path.abspath(pin):
            continue
        name = os.path.basename(other).replace('.json', '')[:28]
        for cut in CUTS:
            for a, b, side in ((pin, other, 'ab'), (other, pin, 'ba')):
                tag = f'r{rnd}-x{side}-{name}-c{cut}'
                out.append((lambda t=tag, x=a, y=b, c=cut: cross('G-cross', t,
                                                                 x, y, c, 0)))
    return out


def round_once(pin, raw, rnd):
    started = time.time()
    calls = jobs_for(pin, raw, rnd)
    rows = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=JOBS) as pool:
        for row in pool.map(lambda call: call(), calls):
            rows.append(row)
            STATS['arms'] += 1
            STATS['byTier'][row['tier']]['arms'] += 1
            if row['exactValid'] and row['contractValid']:
                STATS['byTier'][row['tier']]['publications'] += 1
            if row['published'] is not None and row['published'] < raw:
                STATS['byTier'][row['tier']]['below'] += 1
    below = sorted((r for r in rows if r['published'] is not None
                    and r['published'] < raw), key=lambda r: r['published'])
    seconds = time.time() - started
    STATS['rounds'].append({
        'round': rnd, 'from': repr(raw), 'arms': len(rows),
        'below': len(below), 'seconds': seconds,
        'belowTags': [{'tier': r['tier'], 'tag': r['tag'],
                       'raw': r['publishedRepr']} for r in below[:12]]})
    drv.log(LOG, f'=== r{rnd}: {len(rows)} arms, {len(below)} below, '
                 f'{seconds:.0f}s ===')
    return below


if __name__ == '__main__':
    pin, raw = sys.argv[2], float(sys.argv[3])
    max_rounds = int(sys.argv[4]) if len(sys.argv) > 4 else 20
    drv.log(LOG, f'=== CASCADE2 {LABEL} from {pin} raw={raw!r} '
                 f'bin={lib.BIN} pool={POOL} ===')
    fixpoint = False
    for rnd in range(max_rounds):
        if time.time() > DEADLINE:
            drv.log(LOG, '=== OUT OF TIME ===')
            break
        below = round_once(pin, raw, rnd)
        if not below:
            drv.log(LOG, f'=== FIXPOINT at raw={raw!r} after round {rnd} ===')
            fixpoint = True
            save(pin, raw, fixpoint)
            break
        best = below[0]
        new_pin = f'{PINS}/pin-{best["published"]:.11f}.json'
        lib.pin(json.load(open(best['run'])), new_pin,
                f'{LABEL} round {rnd} adoption {best["tag"]}')
        sha = hashlib.sha256(open(new_pin, 'rb').read()).hexdigest()
        drv.log(LOG, f'*** ADOPT [{best["tier"]}] {best["tag"]}: {raw!r} -> '
                     f'{best["published"]!r} '
                     f'(delta {best["published"] - raw:+.11f}) of '
                     f'{len(below)} below')
        drv.log(LOG, f'    pin={new_pin} sha256={sha} '
                     f'fp={best["fingerprint"]} '
                     f'attr={json.dumps(best["attribution"])}')
        STATS['adoptions'].append({
            'round': rnd, 'tier': best['tier'], 'tag': best['tag'],
            'from': repr(raw), 'to': best['publishedRepr'],
            'delta': best['published'] - raw, 'pin': new_pin, 'sha256': sha,
            'fingerprint': best['fingerprint'],
            'attribution': best['attribution'],
            'alsoBelow': len(below) - 1})
        pin, raw = new_pin, best['published']
        save(pin, raw, fixpoint)
    save(pin, raw, fixpoint)
    drv.log(LOG, f'=== RESULT {LABEL} pin={pin} raw={raw!r} '
                 f'fixpoint={fixpoint} arms={STATS["arms"]} ===')
    print(json.dumps({'label': LABEL, 'pin': pin, 'raw': repr(raw),
                      'fixpoint': fixpoint, 'arms': STATS['arms']}, indent=1))
