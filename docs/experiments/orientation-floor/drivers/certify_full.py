#!/usr/bin/env python3
"""The full certification battery on an incumbent.

Two jobs in one pass:

  1. REPLAY VALIDATION - modes 27, 30 and 22 seeds 0-3 must all come back
     exactValid AND contractValid and reproduce the pin's own raw depth and
     placement fingerprint. That is the pin-is-real check.
  2. FINITE NEGATIVE ON A DECLARED BATTERY - mode 26 ladders x6, mode 31 tiny
     steps x4, the frontier-flatten delta grid handed to modes 32 and 33 under
     BOTH ladder generations (the base-commit binary and the finer-ladder
     binary), the frontier-stack nudge entries, tier H's own legalization grid,
     and the mode-34 schedule specs. The claim this supports is exactly "none
     of the arms listed below found anything under the incumbent" - a finite
     negative over an enumerated set of modes, constants, seeds and budgets.

     It is NOT a fixpoint claim, and the output field is named accordingly
     (`finiteNegativeOnBattery`, not `fixpoint`). The battery says nothing
     about unenumerated angles, centres, budgets, operators or instances; Sol
     review 3 §0 and review 5 §0 both landed on the same correction, and
     review 6 §3 found the word still standing in the drivers after the READMEs
     had been fixed.

Usage: certify_full.py <pin> <raw> [label]
"""
import math
import sys, json, hashlib, os, collections, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import drv, lib, sched

PIN, RAW = sys.argv[1], float(sys.argv[2])
LABEL = sys.argv[3] if len(sys.argv) > 3 else 'cert'
OUT = '/var/lib/t3/tmp/wf87/run'
LOG = f'{OUT}/{LABEL}.log'
RUNS = f'{OUT}/{LABEL}-runs'
FIX = f'{OUT}/{LABEL}-fix'
os.makedirs(RUNS, exist_ok=True)
os.makedirs(FIX, exist_ok=True)

BASE_BIN = os.environ.get('CERT_BASE_BIN', lib.BIN)
NEW_BIN = os.environ.get('CERT_NEW_BIN', lib.BIN)
# The entry grid is a lever in its own right (the finer-ladder round's
# from-scratch adoption came from adding a single flatten delta), so the
# battery probes the grid the cascade actually runs rather than a subset of it.
FLAT = (0.0005, 0.001, 0.0015, 0.002, 0.0025, 0.003, 0.004, 0.005, 0.006,
        0.008, 0.01)
NUDGE = (0.002, 0.006, 0.012, 0.02)
SLACK = (0.05, 2.0)
M31 = (0.006, 0.012, 0.025, 0.04)
DROPS = (0.3, 0.55, 1.0)
# The mode-34 arms. The step size is the knob this round added and the one that
# decided whether the schedule moved at all, so a negative that only probed the
# 1-micron default would be the same one-step-size negative the 159.079 record
# parent's battery turned out to be.
# The step curve is non-monotone, so the fine half of it is probed explicitly:
# 0.25 is the value that published twice on this line, and 0.1875/0.125/0.0625
# are the sub-grid steps below it that no battery had covered.
#
# Eight specs, SIX distinct step sizes: 0.25 and 0.125 are each probed at two
# work budgets. Any prose derived from this tuple must count specs and step
# sizes separately.
SCHED_SPECS = ('past=1,work=20000000,step=0.25',
               'past=1,work=20000000,step=0.1875',
               'past=1,work=20000000,step=0.125',
               'past=1,work=20000000,step=0.0625',
               'past=1,work=20000000,step=1',
               'past=1,work=20000000,step=0.1',
               'past=1,work=60000000,step=0.125',
               'past=1,work=60000000,step=0.25')
SCHED_SEEDS = (5, 0)

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
    # Two ladder generations only when there really are two binaries: this
    # round's tree already carries the finer 0.0032/0.008 rungs, so a default
    # run has `CERT_BASE_BIN == CERT_NEW_BIN` and the second pass would be the
    # same arm counted twice.
    ladders = [(NEW_BIN, 'new')]
    if os.path.realpath(BASE_BIN) != os.path.realpath(NEW_BIN):
        ladders.append((BASE_BIN, 'base'))
    for binary, ladder in ladders:
        for slack in SLACK:
            probe(f'{LABEL}-{ladder}-flat{delta}-m33-p{slack}', 33, path,
                  RAW + slack, 0, binary, ladder)
        # Mode 32 is the measured-unproductive tier (it leaves the conflict's
        # partner in place), and a fixpoint claim that never fires it is a claim
        # about mode 33. One arm a delta keeps the negative current.
        probe(f'{LABEL}-{ladder}-flat{delta}-m32-p0.05', 32, path,
              RAW + 0.05, 0, binary, ladder)
# The frontier-stack nudge entries: the record line's first orientation
# adoption came from a nudged entry rather than a flattened one, so both entry
# families are in the battery.
ranked = drv.ranked_extents(PIN)
for rank in (1, 2, 3):
    for delta in NUDGE:
        path, _ = drv.single_nudge_fixture([ranked[rank - 1][1]], delta, PIN,
                                           f'{LABEL}-r{rank}-d{delta}', outdir=FIX)
        probe(f'{LABEL}-nudge-r{rank}-d{delta}-m33', 33, path, RAW + 2.0, 0,
              NEW_BIN, 'new')
for delta in NUDGE:
    path, _ = drv.single_nudge_fixture([ranked[0][1], ranked[1][1]], delta, PIN,
                                       f'{LABEL}-pair-d{delta}', outdir=FIX)
    probe(f'{LABEL}-nudge-pair-d{delta}-m33', 33, path, RAW + 2.0, 0, NEW_BIN,
          'new')
# Tier H, the entry -> global legalization composition. This is the tier that
# produced most of this round's descent, so a fixpoint claim that does not probe
# it is a claim about the tiers that were already exhausted.
LEGAL_FLAT = (0.01, 0.02, 0.03, 0.05, 0.08, 0.1, 0.15, 0.2, 0.25, 0.3, 0.5,
              1.0)
for delta in LEGAL_FLAT:
    path, depth, moved = drv.flatten_fixture(delta, PIN, f'{LABEL}-legal',
                                             outdir=FIX)
    for mode in (30, 31):
        probe(f'{LABEL}-legalflat{delta}-m{mode}', mode, path, RAW + 0.05, 0,
              NEW_BIN, 'new')
for drop in DROPS:
    for seed in (0, 1):
        probe(f'{LABEL}-m26-d{drop}-s{seed}', 26, PIN, RAW - drop, seed, NEW_BIN, 'new')

# The mode-34 arm. It runs on the schedule binary rather than the replay binary
# and through `sched.sched_arm`, because its knobs live in the environment.
#
# The tag must be unique per spec: SCHED_SPECS above probes both `step=0.25`
# and `step=0.125` at two work budgets each, and `sched.sched_arm` writes its
# raw artifact to `{outdir}/{tag}.json`, so a tag built from `step=` alone
# collides and the 60M run silently overwrites the 20M run's raw file on disk
# (the in-memory summary row is unaffected, since it comes from the run's own
# return value rather than a re-read of that file - four raw artifacts of this
# round's battery were lost this way, two per seed). Encode both `work=` and
# `step=` in the tag so every spec gets its own artifact. Same fix as
# record-line-cascade's copy of this driver; the existing evidence is left as
# it was produced, not re-run.
for seed in SCHED_SEEDS:
    for spec in SCHED_SPECS:
        fields = dict(kv.split('=') for kv in spec.split(','))
        tag = f'{LABEL}-m34-w{fields["work"]}-{fields["step"]}-s{seed}'
        out, _ = sched.sched_arm(tag, PIN, RAW - 0.3, seed, spec, logfile=LOG,
                                 outdir=RUNS)
        pop = lib.population(out) or {}
        published = drv.published_raw(out)
        row = {'tag': tag, 'mode': 34, 'ladder': 'sched', 'spec': spec,
               'published': published,
               'below': published is not None and published < RAW,
               'exactValid': pop.get('exactValid'),
               'contractValid': pop.get('contractValid'),
               'raw': pop.get('rawSourceDepthMm'),
               'fp': pop.get('finalPlacementFingerprint'),
               'attribution': {}}
        probes.append(row)
        if row['below']:
            drv.log(LOG, f'!!! BELOW INCUMBENT {tag}: {published!r}')

below = [row for row in probes if row['below']]
replay_tags = {row['tag'] for _name, row in replay}
search_arms = [row for row in probes if row['tag'] not in replay_tags]
result = {
    'pin': PIN, 'raw': RAW, 'fingerprint': fingerprint, 'sha256': sha,
    'replayPass': bool(replay_ok),
    'replay': [(name, row) for name, row in replay],
    # `probeArms` is EVERY arm this driver ran, replays included, and is kept
    # under its original name because the round's arm totals are quoted from
    # it. `searchArms` is the number that actually probed for a better
    # neighbour, which is the number a coverage claim may cite.
    'probeArms': len(probes),
    'searchArms': len(search_arms), 'replayArms': len(replay),
    'belowIncumbent': len(below), 'below': below,
    'rungs': dict(rungs), 'elapsedS': time.time() - t0,
    # Renamed from `fixpoint`, whose semantics this predicate never had: it is
    # "the replays reproduced the pin AND no arm in the declared battery came
    # back under the incumbent". That is a finite negative over an enumerated
    # set of arms, not a statement that no better neighbour exists.
    'finiteNegativeOnBattery': bool(replay_ok) and not below,
    'probes': probes,
}
json.dump(result, open(f'{OUT}/{LABEL}.json', 'w'), indent=1)
drv.log(LOG, f'=== CERTIFY replayPass={replay_ok} arms={len(probes)} '
             f'(search={len(search_arms)} replay={len(replay)}) '
             f'below={len(below)} '
             f'finiteNegativeOnBattery={result["finiteNegativeOnBattery"]} '
             f'({result["elapsedS"]:.0f}s)')
drv.log(LOG, '    rungs ' + json.dumps(dict(rungs), sort_keys=True))
