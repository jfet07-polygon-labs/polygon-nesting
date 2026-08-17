"""Shared driver for the PERTURB -> {mode 31, mode 26, mode 22} combination.

Reuses pv36-perturb.py's corrected conventions verbatim:
  * depth is measured on the true transformed frame (raw source depth + edge
    clearance), not on the post-rotation translateLongAxis anchor;
  * the nudge selects the k deepest pieces by true transformed max-Y, which is
    the engine's own `high_frontier_blockers` ordering;
  * emitted fixtures declare a depth the harness will accept.
"""

import json
import math
import os
import subprocess

ROOT = '/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a'
BIN = f'{ROOT}/target/release/examples/general_request_benchmark'
REQ = f'{ROOT}/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json'
PARENT = (f'{ROOT}/docs/experiments/persistent-vacancy-descent/exact-contract/'
          'true-contract/from-scratch-164.096/pinned-parent-164.096.json')
REQ_SHA = 'ecfe126f431f08b817813d4af1ad438399585c6cc1c4f16b835e5b6874878bb3'
ARGS = ('1 4 0 0 0 0 1 0 0 1 1 0 16 4 8 {clamp} 0 5 5 24 8 40 10 10 5 {seed} '
        '0.005 0.001 1 6 0 0 0 structured 0 10 1 0 0 0 0').split()
ALLOWANCE = '0.0005'
EDGE_CLEARANCE_MM = 5.0
CONTRACTION_RATIO = 0.001

_request = json.load(open(REQ))
SRC = {}
for _source in _request['sourcePieces']:
    _points = []
    for _segment in _source['geometry']['segments']:
        assert _segment['kind'] == 'line' and not _segment.get('bulge')
        assert not _segment.get('sourceCurve')
        _points.append((_segment['x1'], _segment['y1']))
    SRC[_source['id']] = _points
PIECE_SRC = {p['id']: p['sourcePieceId'] for p in _request['pieces']}


def extents(placements):
    out = {}
    for placement in placements:
        radians = math.radians(placement['rotationDeg'])
        sin, cos = math.sin(radians), math.cos(radians)
        ys = []
        for (x, y) in SRC[PIECE_SRC[placement['pieceId']]]:
            mirrored_x = -x if placement['mirrored'] else x
            ys.append(mirrored_x * sin + y * cos + placement['translateLongAxis'])
        out[placement['pieceId']] = (min(ys), max(ys))
    return out


def depth_mm(placements):
    return max(high for _, high in extents(placements).values()) + EDGE_CLEARANCE_MM


def write_fixture(path, description, placements, declared_depth_mm,
                  fingerprint='perturbation'):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    json.dump({
        'schemaVersion': 1,
        'description': description,
        'requestSha256': REQ_SHA,
        'expectedPlacementFingerprint': fingerprint,
        'reportedDepthMm': declared_depth_mm,
        'independentDepthMm': declared_depth_mm,
        'provenance': {'producedBy': description},
        'placements': placements,
    }, open(path, 'w'), indent=1)


def run(tag, mode, parent, target, seed, outdir, clamp='0', warm='', reuse=True):
    os.makedirs(outdir, exist_ok=True)
    path = f'{outdir}/{tag}.json'
    if reuse and os.path.exists(path) and os.path.getsize(path) > 0:
        try:
            return json.load(open(path))
        except Exception:
            pass
    argv = [BIN, REQ] + [a.format(clamp=clamp, seed=seed) for a in ARGS] + [
        str(mode), parent, str(target), warm, ALLOWANCE]
    with open(path, 'w') as out:
        err = subprocess.run(argv, stdout=out, stderr=subprocess.PIPE, check=False)
    try:
        return json.load(open(path))
    except Exception:
        return {'__error__': (err.stderr or b'').decode()[-400:] or 'unparseable stdout'}


def population(run_json):
    if '__error__' in run_json:
        return None
    coupled = run_json['relaxedDiagnostics']['coupledDynamicSeparator']
    return coupled.get('persistentVacancyPopulation')


def published(run_json):
    """Depth of an exact-valid AND contract-valid publication, else None."""
    pop = population(run_json)
    if pop is None or not pop.get('exactValid') or not pop.get('contractValid'):
        return None
    return pop.get('independentDepthMm')


def line(tag, run_json):
    if '__error__' in run_json:
        return f'{tag}: HARNESS ERROR {run_json["__error__"].strip()[:200]}'
    pop = population(run_json)
    if pop is None:
        arm = run_json['relaxedDiagnostics']['coupledDynamicSeparator'].get(
            'boundaryProjectionTreatment') or {}
        return (f"{tag}: no population; accepted={arm.get('targetsAccepted')} "
                f"depth={arm.get('independentlyMeasuredFinalDepthMm')}")
    return (f"{tag}: mode={pop['mode']} exactValid={pop['exactValid']} "
            f"contractValid={pop.get('contractValid')} "
            f"depth={pop.get('independentDepthMm')} "
            f"raw={pop.get('rawSourceDepthMm')} "
            f"parentDepth={pop.get('parentIndependentDepthMm')} "
            f"fp={(pop.get('finalPlacementFingerprint') or '')[:16]} "
            f"{(pop.get('failureReason') or '')[:110]}")


def pin(run_json, out_path, description):
    pop = population(run_json)
    assert pop and pop.get('exactValid') and pop.get('contractValid'), 'refusing to pin'
    placements = [{
        'pieceId': p['pieceId'],
        'rotationDeg': p['rotationDeg'],
        'mirrored': p['mirrored'],
        'translateShortAxis': p['translateShortAxis'],
        'translateLongAxis': p['translateLongAxis'],
    } for p in pop['finalPlacements']]
    write_fixture(out_path, description, placements, pop['independentDepthMm'],
                  fingerprint=pop['finalPlacementFingerprint'])
    return out_path, pop['independentDepthMm'], pop['rawSourceDepthMm']
