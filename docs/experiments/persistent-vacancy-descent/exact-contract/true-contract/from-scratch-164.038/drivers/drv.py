#!/usr/bin/env python3
"""Driver: entry-mode (27/28/29) grid on the FROM-SCRATCH line 164.03956779906775."""
import sys, json, time, os
sys.path.insert(0, '/var/lib/t3/tmp/combo28-fs')
import lib

PARENT = ('/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/'
          'persistent-vacancy-descent/exact-contract/true-contract/from-scratch-164.040/'
          'pinned-parent-164.040.json')
RAW = 164.03956779906775
OUT = '/var/lib/t3/tmp/combo28-fs'
RUNS = OUT + '/runs'

def log(logfile, msg):
    print(msg, flush=True)
    with open(logfile, 'a') as h:
        h.write(msg + '\n')

def extra(run_json):
    """Mode-28/29 specific diagnostics that lib.line() does not carry."""
    pop = lib.population(run_json)
    if not pop:
        return ''
    blk = pop.get('replacementRepair') or pop.get('jointReplacement')
    if not blk:
        return ''
    bits = [f"pairs={blk['violatingPairs']}", f"bnd={blk['boundaryPieces']}",
            f"comp={blk['componentCount']}", f"maxComp={blk['largestComponentPieces']}",
            f"/{blk['componentLimit']}", f"ejected={blk['ejectedCount']}",
            f"/{blk['ejectionLimit']}", f"att={blk['attempted']}",
            f"maxMatDef={blk.get('maxMaterialDeficitMm')}",
            f"maxEnvPush={blk.get('maxEnvelopePushMm')}"]
    if 'ordersTried' in blk:
        bits += [f"orders={blk['ordersTried']}/{blk['ordersPlanned']}",
                 f"swaps={blk['swapAttemptsTried']}",
                 f"compRep={blk['componentsRepaired']}",
                 f"compRef={blk['componentsRefused']}"]
    else:
        bits += [f"replaced={blk['replacedCount']}",
                 f"projConv={blk['projectionsConverged']}",
                 f"projFail={blk['projectionFailures']}"]
    return '   ~ ' + ' '.join(str(b) for b in bits)

def go(tag, mode, parent, target, seed, logfile, outdir=RUNS):
    t0 = time.time()
    tgt = f'{target:.6f}' if isinstance(target, float) else str(target)
    out = lib.run(tag, mode, parent, tgt, seed, outdir)
    dt = time.time() - t0
    log(logfile, f'[{dt:6.1f}s] ' + lib.line(tag, out))
    ex = extra(out)
    if ex:
        log(logfile, ex)
    return out

def published_raw(run_json):
    """Raw source depth of a legal publication, or None."""
    pop = lib.population(run_json)
    if not pop or not pop.get('exactValid') or not pop.get('contractValid'):
        return None
    return pop.get('rawSourceDepthMm')

def perturb_fixture(k, d, parent=PARENT, outdir=OUT + '/fix'):
    os.makedirs(outdir, exist_ok=True)
    pl = json.load(open(parent))['placements']
    if k == 0:
        return parent, lib.depth_mm(pl)
    nudged = lib.nudge(pl, k, d)
    path = f'{outdir}/k{k}-d{d}.json'
    depth = lib.write_fixture(path, f'binding-stack nudge k{k}-d{d} from {parent}', nudged)
    return path, depth

def flatten_fixture(delta, parent=PARENT, outdir=OUT + '/fix'):
    """Frontier-flatten: every piece deeper than (frontier - delta) is moved in
    by exactly its own excess, so the measured depth becomes RAW - delta with
    the smallest possible perturbation. Designed for the re-placement modes
    (small deficits, bounded component size) rather than for mode 31's
    displacement-cap law."""
    os.makedirs(outdir, exist_ok=True)
    pl = json.load(open(parent))['placements']
    ext = lib.extents(pl)
    frontier = max(hi for _, hi in ext.values())
    cut = frontier - delta
    moved = {p['pieceId']: ext[p['pieceId']][1] - cut
             for p in pl if ext[p['pieceId']][1] > cut}
    out = [dict(p, translateLongAxis=p['translateLongAxis'] - moved[p['pieceId']])
           if p['pieceId'] in moved else dict(p) for p in pl]
    path = f'{outdir}/flat-{delta}.json'
    depth = lib.write_fixture(path, f'frontier flatten delta={delta} from {parent}', out)
    return path, depth, len(moved)
