#!/usr/bin/env python3
"""Phase D: launch-pad descent. Every distinct legal state the pose-changing
modes published near the frontier is used as a start point for a short descent
probe. Adoption is fail-closed and measured against the GLOBAL best."""
import sys, json, hashlib
sys.path.insert(0, '/var/lib/t3/tmp/combo28-fs')
import drv, lib

L = drv.OUT + '/phase-d.log'
BEST = 164.03856779906783

def probe(pad, praw, tagbase):
    hits = []
    for step in (0.006, 0.012, 0.025, 0.04):
        out = drv.go(f'{tagbase}-m31-e{step}', 31, pad, praw - step, 0, L)
        r = drv.published_raw(out)
        if r is not None and r < BEST - 1e-12:
            hits.append((r, f'{tagbase}-m31-e{step}', out))
    out = drv.go(f'{tagbase}-m22', 22, pad, praw + 0.8, 0, L)
    r = drv.published_raw(out)
    if r is not None and r < BEST - 1e-12:
        hits.append((r, f'{tagbase}-m22', out))
    for delta in (0.002, 0.003, 0.004):
        fp, _, _ = drv.flatten_fixture(delta, parent=pad, outdir=drv.OUT + '/fix')
        for mode in (29, 28):
            tag = f'{tagbase}-flat{delta}-m{mode}'
            out = drv.go(tag, mode, fp, praw + 0.5, 0, L)
            r = drv.published_raw(out)
            if r is not None and r < BEST - 1e-12:
                hits.append((r, tag, out))
    return hits

if __name__ == '__main__':
    pads = json.load(open(drv.OUT + '/pads/index.json'))
    lo, hi = float(sys.argv[1]), float(sys.argv[2])
    drv.log(L, f'=== PHASE D probes on pads with raw in [{lo}, {hi}] (global best {BEST!r}) ===')
    allhits = []
    for pad, praw in pads:
        if not (lo <= praw <= hi):
            continue
        drv.log(L, f'-- pad {pad} raw={praw!r}')
        hits = probe(pad, praw, 'd-' + pad.split('pad-')[1][:-5])
        for r, tag, out in hits:
            p = f'{drv.OUT}/pins/pin-{r:.6f}.json'
            lib.pin(out, p, f'phase-D adoption {tag}')
            drv.log(L, f'*** BELOW GLOBAL BEST: raw={r!r} from {tag} pinned {p} '
                       f'sha={hashlib.sha256(open(p,"rb").read()).hexdigest()[:16]}')
            allhits.append((r, tag))
    drv.log(L, f'PHASE D hits in [{lo},{hi}]: {sorted(allhits)}')
