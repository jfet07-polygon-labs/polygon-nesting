#!/usr/bin/env python3
"""Shared helpers for the PERTURB -> mode-31 / mode-26 / mode-22 cascade.

Reuses the committed pv36-perturb.py conventions verbatim where they matter:

  * the nudge selects the k deepest pieces by TRUE transformed max-Y
    (`extents`), not by the `translateLongAxis` anchor;
  * depth is the raw transformed source depth (`depth_mm`), which reproduces
    the record fixture's `rawSourceDepthMm` to the last bit;
  * the emitted fixture's depth fields are honest about the layout it carries,
    which is what `check_parent_fixture_depths` re-derives on load.

Unlike pv36 the perturbed state is handed to the repair mode as the PARENT
fixture (CLI argument 43), not as a warm start: modes 26/27/28/29/30/31 read
`effective_parent.final_placements` and never look at the warm-start slot.
"""

import json
import math
import os
import subprocess

ROOT = '/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a'
BIN = f'{ROOT}/target/release/examples/general_request_benchmark'
REQ = f'{ROOT}/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json'
TRUE = (f'{ROOT}/docs/experiments/persistent-vacancy-descent/exact-contract/'
        'true-contract')
PARENT = f'{TRUE}/record-159.150/pinned-parent-159.150.json'
REQ_SHA = 'ecfe126f431f08b817813d4af1ad438399585c6cc1c4f16b835e5b6874878bb3'
OUT = '/var/lib/t3/tmp/combo'

# The pinned CLI tail. Argument 16 (sheet-long-axis-override) stays 0: every
# mode used here carries its own bound (mode 31 argument 44, mode 26 per rung).
ARGS = ('1 4 0 0 0 0 1 0 0 1 1 0 16 4 8 {clamp} 0 5 5 24 8 40 10 10 5 {seed} '
        '0.005 0.001 1 6 0 0 0 structured 0 10 1 0 0 0 0').split()
ALLOWANCE = '0.0005'
EDGE_CLEARANCE_MM = 5.0
# Measured on the mode-31 control probe: bound 159.15 against a 159.14999
# layout reports maxBoundaryDeficitMm = 0.003, so a mode-31 bound B admits a
# source depth of B - BOUND_OFFSET_MM.
BOUND_OFFSET_MM = 0.003

_request = json.load(open(REQ))
SRC = {}
for _source in _request['sourcePieces']:
    _points = []
    for _segment in _source['geometry']['segments']:
        assert _segment['kind'] == 'line' and not _segment.get('bulge')
        assert not _segment.get('sourceCurve')
        _points.append((_segment['x1'], _segment['y1']))
    SRC[_source['id']] = _points
PIECE_SRC = {piece['id']: piece['sourcePieceId'] for piece in _request['pieces']}


def extents(placements):
    """pieceId -> (min_y, max_y) along the depth axis, transformed frame."""
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


def write_fixture(path, description, placements, reported_depth_mm=None):
    """Emits a loadable parent fixture whose depth fields describe its own
    geometry: independentDepthMm is the raw source depth of these placements
    (an exact `MeasuredDepths` convention) and reportedDepthMm is the strip the
    perturbation was taken in, which may legitimately be deeper."""
    measured = depth_mm(placements)
    json.dump({
        'schemaVersion': 1,
        'description': description,
        'requestSha256': REQ_SHA,
        'expectedPlacementFingerprint': 'perturbation',
        'reportedDepthMm': max(measured, reported_depth_mm or measured),
        'independentDepthMm': measured,
        'provenance': {'producedBy': description},
        'placements': placements,
    }, open(path, 'w'), indent=1)
    return measured


def run(tag, mode, parent, target, seed, outdir, clamp='0', warm=''):
    os.makedirs(outdir, exist_ok=True)
    argv = [BIN, REQ] + [a.format(clamp=clamp, seed=seed) for a in ARGS] + [
        str(mode), parent, str(target), warm, ALLOWANCE]
    path = f'{outdir}/{tag}.json'
    with open(path, 'w') as out:
        result = subprocess.run(argv, stdout=out, stderr=subprocess.PIPE,
                                check=False)
    try:
        with open(path) as handle:
            return json.load(handle)
    except json.JSONDecodeError:
        return {'_loadError': (result.stderr or b'').decode()[-400:]}


def population(run_json):
    if '_loadError' in run_json:
        return None
    coupled = run_json['relaxedDiagnostics']['coupledDynamicSeparator']
    return coupled.get('persistentVacancyPopulation')


def line(tag, run_json):
    pop = population(run_json)
    if pop is None:
        return f'{tag}: NO POPULATION {run_json.get("_loadError", "")[:200]}'
    global_diag = pop.get('globalLegalization') or {}
    return (f"{tag}: mode={pop['mode']} exactValid={pop['exactValid']} "
            f"contractValid={pop.get('contractValid')} "
            f"depth={pop.get('independentDepthMm')} "
            f"raw={pop.get('rawSourceDepthMm')} "
            f"parent={pop.get('parentIndependentDepthMm')} "
            f"fp={(pop.get('finalPlacementFingerprint') or '')[:16]} "
            f"| before {global_diag.get('violatingPairsBefore')}p/"
            f"{global_diag.get('boundaryPiecesBefore')}b "
            f"after {global_diag.get('violatingPairsAfter')}p/"
            f"{global_diag.get('boundaryPiecesAfter')}b "
            f"cap={global_diag.get('displacementCapMm')} "
            f"maxDisp={global_diag.get('maxDisplacementMm')} "
            f"moved={global_diag.get('movedPieces')} "
            f"resid={global_diag.get('maxDualResidualMm')} "
            f"| {(pop.get('failureReason') or '')[:90]}")


def pin(run_json, out_path, description):
    pop = population(run_json)
    assert pop and pop['exactValid'], 'refusing to pin a state that did not validate'
    placements = [{
        'pieceId': p['pieceId'],
        'rotationDeg': p['rotationDeg'],
        'mirrored': p['mirrored'],
        'translateShortAxis': p['translateShortAxis'],
        'translateLongAxis': p['translateLongAxis'],
    } for p in pop['finalPlacements']]
    depth = pop['independentDepthMm']
    json.dump({
        'schemaVersion': 1,
        'description': description,
        'requestSha256': REQ_SHA,
        'expectedPlacementFingerprint': pop['finalPlacementFingerprint'],
        'reportedDepthMm': depth,
        'independentDepthMm': depth,
        'provenance': {'producedBy': description},
        'placements': placements,
    }, open(out_path, 'w'), indent=1)
    return out_path


def nudge(placements, k, delta):
    """pv36's nudge: the k deepest pieces by true transformed max-Y, moved
    `delta` mm toward the packed body along the depth axis."""
    extent = extents(placements)
    ranked = sorted(placements,
                    key=lambda p: (-extent[p['pieceId']][1], p['pieceId']))
    ids = {p['pieceId'] for p in ranked[:k]}
    return [dict(p, translateLongAxis=p['translateLongAxis'] - delta)
            if p['pieceId'] in ids else dict(p) for p in placements]
