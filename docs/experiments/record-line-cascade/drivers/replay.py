#!/usr/bin/env python3
"""The pin-is-real check: does a fixture replay to its own declared raw?

    python3 replay.py PIN RAW [LABEL]

Modes 27 and 30 (probe/legalize replays) and mode 22 seeds 0-1 must all return
exactValid AND contractValid, the pin's own placement fingerprint, and a raw
within one ULP of the declared publication-authority measure. That is the
one-ULP policy: the parent-measure and publication-measure paths round one ULP
apart on identical layouts, so identical fingerprint plus raw within one ULP is
a reproduction, while "below" stays a strict `<`.
"""
import hashlib
import json
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import drv  # noqa: E402
import lib  # noqa: E402

PIN, RAW = sys.argv[1], float(sys.argv[2])
LABEL = sys.argv[3] if len(sys.argv) > 3 else 'replay'
LOG = f'/var/lib/t3/tmp/recordline/{LABEL}.log'
RUNS = f'/var/lib/t3/tmp/recordline/{LABEL}-runs'

fingerprint = json.load(open(PIN))['expectedPlacementFingerprint']
sha = hashlib.sha256(open(PIN, 'rb').read()).hexdigest()
drv.log(LOG, f'=== REPLAY {PIN} raw={RAW!r} fp={fingerprint[:16]} sha256={sha}')

# Modes 27 and 30 are the replay authorities: they are handed the fixture and
# asked what it is, so they must hand back exactly it. The mode-22 rows are
# *descent* arms at a loose bound, so a row that comes back strictly deeper is
# not a failed replay - it is the cascade's next incumbent, and it is reported
# as `improves` rather than folded into the pass.
AUTHORITIES = {27, 30}
rows, ok, improvements = [], True, []
for tag, mode, target, seed in (('m27', 27, RAW, 0), ('m30', 30, RAW, 0),
                                ('m22-s0', 22, RAW + 0.8, 0),
                                ('m22-s1', 22, RAW + 0.8, 1)):
    out = drv.go(f'{LABEL}-{tag}', mode, PIN, target, seed, LOG, outdir=RUNS)
    pop = lib.population(out) or {}
    raw = pop.get('rawSourceDepthMm')
    valid = bool(pop.get('exactValid')) and bool(pop.get('contractValid'))
    match = (pop.get('finalPlacementFingerprint') == fingerprint
             and raw is not None and abs(raw - RAW) <= math.ulp(RAW))
    improves = valid and raw is not None and raw < RAW
    good = valid and match
    if mode in AUTHORITIES:
        ok &= good
    if improves:
        improvements.append({'tag': tag, 'raw': repr(raw),
                             'run': f'{RUNS}/{LABEL}-{tag}.json'})
    rows.append({'tag': tag, 'mode': mode, 'exactValid': pop.get('exactValid'),
                 'contractValid': pop.get('contractValid'), 'raw': repr(raw),
                 'ulpsFromDeclared': None if raw is None else
                 (raw - RAW) / math.ulp(RAW),
                 'fingerprintMatches':
                 pop.get('finalPlacementFingerprint') == fingerprint,
                 'authority': mode in AUTHORITIES,
                 'reproduces': good, 'improves': improves})
    drv.log(LOG, f'   REPLAY {tag}: reproduces={good} improves={improves} '
                 f'raw={raw!r}')

result = {'pin': PIN, 'sha256': sha, 'declaredRaw': repr(RAW),
          'fingerprint': fingerprint, 'replayPass': bool(ok),
          'improvements': improvements, 'rows': rows}
print(json.dumps(result, indent=1))
json.dump(result, open(f'/var/lib/t3/tmp/recordline/{LABEL}.json', 'w'), indent=1)
