#!/usr/bin/env python3
"""Driver for the orientation-perturbed re-insertion experiment (modes 32/33)."""
import sys, json, time, os, hashlib, subprocess
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib

OUT = '/var/lib/t3/tmp/wf87/run'
RUNS = OUT + '/runs'
FIX = OUT + '/fix'
os.makedirs(RUNS, exist_ok=True)
os.makedirs(FIX, exist_ok=True)

RECORD = f'{lib.TRUE}/finer-ladder/pinned-parent-159.079.json'
RECORD_RAW = 159.07876040364792
SCRATCH = f'{lib.TRUE}/finer-ladder/pinned-fs-parent-164.0376.json'
SCRATCH_RAW = 164.0375677990678


def log(logfile, msg):
    print(msg, flush=True)
    with open(logfile, 'a') as handle:
        handle.write(msg + '\n')


def blk(run_json):
    pop = lib.population(run_json)
    if not pop:
        return None
    return pop.get('replacementRepair') or pop.get('jointReplacement')


def attribution(run_json):
    """The mandatory accepted-pose attribution counters."""
    pop = lib.population(run_json)
    if not pop:
        return {}
    block = blk(run_json) or {}
    keys = ('variants', 'candidates', 'rows', 'finalists', 'acceptedVacated',
            'acceptedAnchorLocal', 'acceptedOrientation', 'acceptedStation')
    found = {}
    angles = []

    def walk(node):
        if isinstance(node, dict):
            if 'orientation' in node and isinstance(node['orientation'], dict):
                block = node['orientation']
                for key in keys:
                    if isinstance(block.get(key), int):
                        found[key] = found.get(key, 0) + block[key]
                if block.get('acceptedOrientation'):
                    angles.append((node.get('pieceId'),
                                   block.get('acceptedRotationDeg'),
                                   block.get('acceptedRotationDeltaDeg'),
                                   block.get('acceptedMirrorFlipped')))
            for value in node.values():
                walk(value)
        elif isinstance(node, list):
            for value in node:
                walk(value)
    walk(block)
    if angles:
        found['acceptedAngles'] = angles
    return found


def extra(run_json):
    block = blk(run_json)
    if not block:
        return ''
    bits = [f"pairs={block['violatingPairs']}", f"bnd={block['boundaryPieces']}",
            f"comp={block['componentCount']}", f"maxComp={block['largestComponentPieces']}",
            f"/{block['componentLimit']}", f"ej={block['ejectedCount']}",
            f"/{block['ejectionLimit']}", f"att={block['attempted']}",
            f"matDef={block.get('maxMaterialDeficitMm')}"]
    if 'ordersTried' in block:
        bits += [f"orders={block['ordersTried']}/{block['ordersPlanned']}",
                 f"swaps={block['swapAttemptsTried']}",
                 f"cRep={block['componentsRepaired']}",
                 f"cRef={block['componentsRefused']}"]
    else:
        bits += [f"replaced={block['replacedCount']}"]
    attr = attribution(run_json)
    if attr:
        bits.append('| ' + ' '.join(f'{k}={v}' for k, v in sorted(attr.items())))
    if block.get('skippedReason'):
        bits.append('| skip=' + block['skippedReason'][:80])
    return '   ~ ' + ' '.join(str(b) for b in bits)


def go(tag, mode, parent, target, seed, logfile, outdir=RUNS, binary=None,
       env=None):
    t0 = time.time()
    target = f'{target:.6f}' if isinstance(target, float) else str(target)
    out = lib.run(tag, mode, parent, target, seed, outdir, binary=binary,
                  env=env)
    dt = time.time() - t0
    log(logfile, f'[{dt:7.1f}s] ' + lib.line(tag, out))
    line = extra(out)
    if line:
        log(logfile, line)
    return out


def published_raw(run_json):
    pop = lib.population(run_json)
    if not pop or not pop.get('exactValid') or not pop.get('contractValid'):
        return None
    return pop.get('rawSourceDepthMm')


def flatten_fixture(delta, parent, tag=None, outdir=FIX):
    """Frontier flatten: every piece deeper than frontier - delta moved in by
    exactly its own excess."""
    placements = json.load(open(parent))['placements']
    extent = lib.extents(placements)
    frontier = max(high for _, high in extent.values())
    cut = frontier - delta
    moved = {p['pieceId']: extent[p['pieceId']][1] - cut
             for p in placements if extent[p['pieceId']][1] > cut}
    out = [dict(p, translateLongAxis=p['translateLongAxis'] - moved[p['pieceId']])
           if p['pieceId'] in moved else dict(p) for p in placements]
    name = tag or os.path.basename(parent).replace('.json', '')
    path = f'{outdir}/flat-{name}-{delta}.json'
    depth = lib.write_fixture(path, f'frontier flatten delta={delta} from {parent}', out)
    return path, depth, sorted(moved)


def single_nudge_fixture(piece_ids, delta, parent, tag, outdir=FIX):
    """Move the named pieces in by delta along the depth axis."""
    placements = json.load(open(parent))['placements']
    ids = set(piece_ids)
    out = [dict(p, translateLongAxis=p['translateLongAxis'] - delta)
           if p['pieceId'] in ids else dict(p) for p in placements]
    path = f'{outdir}/nudge-{tag}.json'
    depth = lib.write_fixture(path, f'nudge {sorted(ids)} by {delta} from {parent}', out)
    return path, depth


def ranked_extents(parent):
    placements = json.load(open(parent))['placements']
    extent = lib.extents(placements)
    return sorted(((high, pid) for pid, (_, high) in extent.items()), reverse=True)


def sha256(path):
    return hashlib.sha256(open(path, 'rb').read()).hexdigest()
