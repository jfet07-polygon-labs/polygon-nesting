#!/usr/bin/env python3
"""Descent cascade from the orientation-entry record, finer-ladder binary.

Round shape (the brief's, plus the two tiers the stopped cascade proved cheap):

  A. frontier flatten in {0.0005, 0.001, 0.002, 0.003, 0.004, 0.01} -> mode 33
     at a LOOSE bound (incumbent + 0.05, then incumbent + 2.0)
  B. mode 22 alternation, seeds 0-3, target incumbent + 0.8
  C. mode 31 tiny-step ratchet, steps {0.006, 0.012, 0.025, 0.04}
  D. lineage basins: flatten each of the three orientation-entry lineage pins
     and hand it to mode 33 at a bound relative to the *incumbent*
  E. frontier-stack single and pair nudges -> mode 33
  F. mode 26 short ladders, drops {0.3, 0.55, 1.0}, seeds {0, 1} - EVERY THIRD
     ROUND only; it adopted nothing in four rounds of the stopped cascade and
     costs ~48 s an arm against ~3 s for every other tier.

Adopts only publications the engine itself validated (exactValid AND
contractValid) whose rawSourceDepthMm is strictly below the incumbent, and
restarts the round from the new incumbent the moment it does.
"""
import sys, json, hashlib, os, time, collections
sys.path.insert(0, '/var/lib/t3/tmp/orient-fine')
import drv, lib

LOG = '/var/lib/t3/tmp/orient-fine/campaign.log'
PINS = '/var/lib/t3/tmp/orient-fine/pins'
RUNS = '/var/lib/t3/tmp/orient-fine/campaign-runs'
FIX = '/var/lib/t3/tmp/orient-fine/campaign-fix'
STATE = '/var/lib/t3/tmp/orient-fine/campaign-state.json'
for d in (PINS, RUNS, FIX):
    os.makedirs(d, exist_ok=True)

FLAT = (0.0005, 0.001, 0.002, 0.003, 0.004, 0.01)
SLACK = (0.05, 2.0)
M31 = (0.006, 0.012, 0.025, 0.04)
NUDGE = (0.002, 0.006, 0.012, 0.02)
DROPS = (0.3, 0.55, 1.0)
LINEAGE_DIR = f'{lib.TRUE}/orientation-entry/lineage'
LINEAGE = [f'{LINEAGE_DIR}/{n}' for n in sorted(os.listdir(LINEAGE_DIR))] \
    if os.path.isdir(LINEAGE_DIR) else []
LINEAGE_FLAT = (0.001, 0.002, 0.003, 0.004)

STATS = {
    'arms': 0,
    'byTier': collections.defaultdict(lambda: collections.defaultdict(int)),
    'rungs': collections.defaultdict(int),
    'adoptRungs': collections.defaultdict(int),
    'attribution': collections.defaultdict(int),
    'adoptions': [],
}
DEADLINE = float(os.environ.get('CAMPAIGN_DEADLINE', '1e18'))


class OutOfTime(Exception):
    pass


def rung_label(delta_deg, mirrored):
    family = 'mirror' if mirrored else 'rot'
    if delta_deg is None:
        return f'{family}:?'
    return f'{family}:{abs(delta_deg):.7g}'


def observe(tier, out):
    """Fold one arm's orientation counters into the campaign-wide tallies."""
    attr = dict(drv.attribution(out))
    angles = attr.pop('acceptedAngles', [])
    STATS['arms'] += 1
    STATS['byTier'][tier]['arms'] += 1
    for key, value in attr.items():
        STATS['attribution'][key] += value
        STATS['byTier'][tier][key] += value
    local = collections.defaultdict(int)
    for (_pid, _abs_deg, delta_deg, mirrored) in angles:
        label = rung_label(delta_deg, mirrored)
        STATS['rungs'][label] += 1
        local[label] += 1
    pop = lib.population(out) or {}
    if pop.get('exactValid') and pop.get('contractValid'):
        STATS['byTier'][tier]['publications'] += 1
    return dict(local), attr, angles


def arm(tag, tier, mode, parent, target, seed, current_raw):
    if time.time() > DEADLINE:
        raise OutOfTime(tag)
    out = drv.go(tag, mode, parent, target, seed, LOG, outdir=RUNS)
    local, attr, angles = observe(tier, out)
    if local:
        drv.log(LOG, '   rungs ' + json.dumps(local, sort_keys=True))
    raw = drv.published_raw(out)
    if raw is None or raw >= current_raw - 1e-12:
        return None
    pin = f'{PINS}/pin-{raw:.9f}.json'
    lib.pin(out, pin, f'finer-ladder cascade adoption {tag}')
    sha = hashlib.sha256(open(pin, 'rb').read()).hexdigest()
    pop = lib.population(out)
    drv.log(LOG, f'*** ADOPT [{tier}] {tag}: {current_raw!r} -> {raw!r} '
                 f'(delta {raw - current_raw:+.9f})')
    drv.log(LOG, f'    pin={pin} sha256={sha} fp={pop["finalPlacementFingerprint"]}')
    drv.log(LOG, '    attribution ' + json.dumps(attr, sort_keys=True))
    drv.log(LOG, '    acceptedAngles ' + json.dumps(angles))
    for label, count in local.items():
        STATS['adoptRungs'][label] += count
    STATS['adoptions'].append({
        'tier': tier, 'tag': tag, 'from': current_raw, 'to': raw,
        'delta': raw - current_raw, 'pin': pin, 'sha256': sha,
        'fingerprint': pop['finalPlacementFingerprint'],
        'attribution': attr, 'acceptedAngles': angles, 'rungs': local,
    })
    save()
    return pin, raw


def save():
    json.dump({
        'arms': STATS['arms'],
        'byTier': {k: dict(v) for k, v in STATS['byTier'].items()},
        'rungs': dict(STATS['rungs']),
        'adoptRungs': dict(STATS['adoptRungs']),
        'attribution': dict(STATS['attribution']),
        'adoptions': STATS['adoptions'],
    }, open(STATE, 'w'), indent=1)


def round_once(pin, raw, rnd):
    # A. flatten grid -> mode 33, loose bound.
    for delta in FLAT:
        path, depth, moved = drv.flatten_fixture(delta, pin, f'r{rnd}', outdir=FIX)
        drv.log(LOG, f'-- r{rnd} flatten {delta}: depth {depth:.9f}, {len(moved)} moved')
        for slack in SLACK:
            got = arm(f'r{rnd}-flat{delta}-m33-p{slack}', 'A-flat', 33, path,
                      raw + slack, 0, raw)
            if got:
                return got
    # B. mode 22 alternation.
    for seed in range(4):
        got = arm(f'r{rnd}-m22-s{seed}', 'B-m22', 22, pin, raw + 0.8, seed, raw)
        if got:
            return got
    # C. mode 31 tiny steps.
    for step in M31:
        got = arm(f'r{rnd}-m31-e{step}', 'C-m31', 31, pin, raw - step, 0, raw)
        if got:
            return got
    # D. lineage basins, flattened, judged against the incumbent.
    for source in LINEAGE:
        name = os.path.basename(source).replace('.json', '')
        for delta in LINEAGE_FLAT:
            path, depth, moved = drv.flatten_fixture(
                delta, source, f'r{rnd}-{name}', outdir=FIX)
            got = arm(f'r{rnd}-lin-{name}-flat{delta}-m33', 'D-lineage', 33,
                      path, raw + 0.05, 0, raw)
            if got:
                return got
    # E. frontier-stack nudges.
    ranked = drv.ranked_extents(pin)
    for rank in (1, 2, 3):
        for delta in NUDGE:
            path, depth = drv.single_nudge_fixture(
                [ranked[rank - 1][1]], delta, pin, f'r{rnd}-r{rank}-d{delta}',
                outdir=FIX)
            got = arm(f'r{rnd}-nudge-r{rank}-d{delta}-m33', 'E-nudge', 33, path,
                      raw + 2.0, 0, raw)
            if got:
                return got
    for delta in NUDGE:
        ids = [ranked[0][1], ranked[1][1]]
        path, depth = drv.single_nudge_fixture(ids, delta, pin,
                                               f'r{rnd}-pair-d{delta}', outdir=FIX)
        got = arm(f'r{rnd}-nudge-pair-d{delta}-m33', 'E-nudge', 33, path,
                  raw + 2.0, 0, raw)
        if got:
            return got
    # F. mode 26, every third round only.
    if rnd % 3 == 2:
        for drop in DROPS:
            for seed in (0, 1):
                got = arm(f'r{rnd}-m26-d{drop}-s{seed}', 'F-m26', 26, pin,
                          raw - drop, seed, raw)
                if got:
                    return got
    return None


if __name__ == '__main__':
    pin, raw = sys.argv[1], float(sys.argv[2])
    max_rounds = int(sys.argv[3]) if len(sys.argv) > 3 else 40
    drv.log(LOG, f'=== CAMPAIGN from {pin} raw={raw!r} bin={lib.BIN} ===')
    drv.log(LOG, f'    lineage pins: {LINEAGE}')
    fixpoint = False
    try:
        for rnd in range(max_rounds):
            t0 = time.time()
            got = round_once(pin, raw, rnd)
            if not got:
                drv.log(LOG, f'=== FIXPOINT at raw={raw!r} after round {rnd} '
                             f'({time.time() - t0:.0f}s, mode26={"yes" if rnd % 3 == 2 else "no"}) ===')
                fixpoint = True
                break
            pin, raw = got
    except OutOfTime as stop:
        drv.log(LOG, f'=== OUT OF TIME at {stop} ===')
    drv.log(LOG, f'=== CAMPAIGN RESULT pin={pin} raw={raw!r} fixpoint={fixpoint} '
                 f'arms={STATS["arms"]} ===')
    drv.log(LOG, '    rungs seen      ' + json.dumps(dict(STATS['rungs']), sort_keys=True))
    drv.log(LOG, '    rungs on adopts ' + json.dumps(dict(STATS['adoptRungs']), sort_keys=True))
    save()
    json.dump({'pin': pin, 'raw': raw, 'fixpoint': fixpoint},
              open('/var/lib/t3/tmp/orient-fine/campaign-result.json', 'w'), indent=1)
