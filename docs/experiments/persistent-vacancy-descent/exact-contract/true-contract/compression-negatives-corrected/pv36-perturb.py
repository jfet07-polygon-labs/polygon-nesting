"""Corrected overlap-then-legalize probes on a deep exact-valid layout.

Replaces pv34-squeeze.py / pv35-nudge.py, both of which were instrument
failures:

  * they wrote `reportedDepthMm` / `independentDepthMm` = 200.0 into the
    perturbed warm-start fixture. The harness installs that field as the
    incumbent depth (general_request_benchmark.rs ~316/378) and every
    separator contraction target derives from it (general_relaxed.rs ~2628),
    so the separator was handed ~30 mm of headroom that did not exist and
    "relaxed to ~179" is a measurement of that headroom, not of the layout;
  * they also passed 200.0 as the CLI target depth, so nothing anywhere in
    the run was bounded by the parent's own depth;
  * pv35 picked "the k deepest pieces" by the `translateLongAxis` anchor.
    Anchors are post-rotation offsets: on the 164.058 record they range from
    -25.2 to +175.7, so the ranking is unrelated to which pieces are actually
    on the depth frontier. The engine's own ordering is
    `high_frontier_blockers` (general_relaxed.rs ~13165), by transformed
    source max-Y.

This driver fixes all three:

  * the squeeze is anchored on the layout's true transformed depth floor and
    the nudge selects by true transformed max-Y;
  * every emitted fixture declares the parent's own depth carried at the
    mode-26 rung seed convention, parent_depth / (1 - contraction ratio), so
    the separator's first contraction target lands exactly on the parent
    depth and no state above it can be accepted;
  * the runs clamp the sheet long axis at the parent depth (CLI argument 17),
    which is exactly the mode-26 rung clamp - `collision_fits_sheet` is the
    only place the long axis bounds anything and it gates every acceptance -
    so the separator provably cannot relax depth-ward.

The perturbed states are then handed to each repair tier in turn: mode 0
(separator + boundary-projection terminal, under the clamp), mode 27
(micro-legalization), mode 28 (conflict-targeted re-placement under the
clamp) and mode 29 (joint multi-piece re-placement under the clamp).

usage: pv36-perturb.py <worktree-root> <outdir>
"""

import json
import math
import os
import subprocess
import sys

ROOT = sys.argv[1] if len(sys.argv) > 1 else os.getcwd()
OUT = sys.argv[2] if len(sys.argv) > 2 else '/var/lib/t3/tmp/pv36'
BIN = f'{ROOT}/target-iso/release/examples/general_request_benchmark'
REQ = f'{ROOT}/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json'
PARENT = (f'{ROOT}/docs/experiments/persistent-vacancy-descent/exact-contract/true-contract/'
          'record-164.058/pinned-parent-164.058.json')
REQ_SHA = 'ecfe126f431f08b817813d4af1ad438399585c6cc1c4f16b835e5b6874878bb3'
ARGS = ('1 4 0 0 0 0 1 0 0 1 1 0 16 4 8 {clamp} 0 5 5 24 8 40 10 10 5 {seed} '
        '0.005 0.001 1 6 0 0 0 structured 0 10 1 0 0 0 0').split()
EDGE_CLEARANCE_MM = 5.0
# COUPLED_SEPARATOR_CONTRACTION_RATIO in general_relaxed.rs.
CONTRACTION_RATIO = 0.001

_request = json.load(open(REQ))
# Every mixed-61 source segment is a straight line and the sheet is 2000x2700,
# so `normalize_polygon_axes` is a no-op (width >= height is false) and the
# transformed extent is the raw segment chain under the placement transform.
SRC = {}
for source in _request['sourcePieces']:
    points = []
    for segment in source['geometry']['segments']:
        assert segment['kind'] == 'line' and not segment.get('bulge')
        assert not segment.get('sourceCurve')
        points.append((segment['x1'], segment['y1']))
    SRC[source['id']] = points
PIECE_SRC = {piece['id']: piece['sourcePieceId'] for piece in _request['pieces']}


def extents(placements):
    """pieceId -> (min_y, max_y) along the depth axis, in the transformed frame."""
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
    """The independent source depth the engine measures for this layout."""
    return max(high for _, high in extents(placements).values()) + EDGE_CLEARANCE_MM


def write_fixture(path, description, placements, declared_depth_mm):
    json.dump({
        'schemaVersion': 1,
        'description': description,
        'requestSha256': REQ_SHA,
        'expectedPlacementFingerprint': 'perturbation',
        'reportedDepthMm': declared_depth_mm,
        'independentDepthMm': declared_depth_mm,
        'provenance': {'producedBy': description},
        'placements': placements,
    }, open(path, 'w'), indent=1)


def run(tag, mode, parent, target, seed, outdir, clamp='0', warm=''):
    os.makedirs(outdir, exist_ok=True)
    argv = [BIN, REQ] + [a.format(clamp=clamp, seed=seed) for a in ARGS] + [
        str(mode), parent, str(target), warm, '0.0005']
    with open(f'{outdir}/{tag}.json', 'w') as out:
        subprocess.run(argv, stdout=out, stderr=subprocess.DEVNULL, check=False)
    return json.load(open(f'{outdir}/{tag}.json'))


def verdict(run_json):
    coupled = run_json['relaxedDiagnostics']['coupledDynamicSeparator']
    population = coupled.get('persistentVacancyPopulation')
    if population is None:
        arm = coupled['boundaryProjectionTreatment']
        return (f"separator targetsAccepted={arm['targetsAccepted']} "
                f"stateDepth={arm['independentlyMeasuredFinalDepthMm']}")
    return (f"mode {population['mode']} exactValid={population['exactValid']} "
            f"depth={population.get('independentDepthMm')} "
            f"parentDepth={population.get('parentIndependentDepthMm')} "
            f"{(population.get('failureReason') or '')[:70]}")


def main():
    os.makedirs(OUT, exist_ok=True)
    placements = json.load(open(PARENT))['placements']
    parent_depth = depth_mm(placements)
    seed_depth = parent_depth / (1.0 - CONTRACTION_RATIO)
    clamp = f'{parent_depth:.11f}'
    extent = extents(placements)
    floor = min(low for low, _ in extent.values())
    ranked = sorted(placements, key=lambda p: (-extent[p['pieceId']][1], p['pieceId']))

    cases = [('control-parent', [dict(p) for p in placements])]
    for factor in (0.995, 0.99, 0.98):
        cases.append((f'squeeze-{factor}', [
            dict(p, translateLongAxis=p['translateLongAxis']
                 + (factor - 1.0) * (extent[p['pieceId']][0] - floor))
            for p in placements]))
    for pieces, delta in ((3, 2.0), (6, 2.0), (10, 1.0)):
        ids = {p['pieceId'] for p in ranked[:pieces]}
        cases.append((f'nudge-k{pieces}-d{delta}', [
            dict(p, translateLongAxis=p['translateLongAxis'] - delta)
            if p['pieceId'] in ids else dict(p) for p in placements]))

    print(f'parent depth {parent_depth:.6f}, warm-start depth {seed_depth:.6f}, clamp {clamp}')
    for tag, perturbed in cases:
        path = f'{OUT}/{tag}.json'
        write_fixture(path, f'corrected perturbation {tag}', perturbed, seed_depth)
        print(f'{tag}: raw perturbed depth {depth_mm(perturbed):.4f}')
        for seed in (0, 1):
            # Tier zero: the ordinary mode-0 pipeline under the clamp, with the
            # perturbed state as warm start at a truthful incumbent depth.
            out = run(f'{tag}-s{seed}', 0, PARENT, f'{parent_depth:.3f}', seed,
                      f'{OUT}/mode0', clamp=clamp, warm=path)
            print(f'  mode0  seed {seed}: {verdict(out)}')
        for mode in (27, 28, 29):
            out = run(tag, mode, path, f'{parent_depth:.3f}', 0, f'{OUT}/mode{mode}')
            print(f'  mode{mode} seed 0: {verdict(out)}')


if __name__ == '__main__':
    main()
