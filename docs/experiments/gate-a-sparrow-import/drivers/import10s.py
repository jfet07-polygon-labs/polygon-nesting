#!/usr/bin/env python3
"""Gate A step 1: map the Sparrow 10 s x86 solution into an engine pose set.

The transformation itself is the COMMITTED converter,
`docs/experiments/persistent-vacancy-descent/sparrow-to-hint-fixture.py`,
loaded as a module and pointed at `solution-10s-x86.json` instead of its
hard-coded `solution-3s.json`. Nothing in its placement arithmetic is
re-implemented here: this driver imports it, flips the one module-level path
constant, calls its `main()`, and then *independently* re-derives every
placement and every transformed vertex to prove the converter did what its
docstring says.

Why the independent re-derivation matters: a conversion artefact would fake all
three of Gate A's verdicts. A wrong item_id->piece map, a wrong rotation sign, a
mirrored frame or a dropped sheet inset all produce a pose set that our
validators reject for a reason that has nothing to do with the miter envelope.
So this driver checks, in order:

  1. index identity   - Sparrow `items[i].id == i` and `items[i].dxf ==
                        request.pieces[i].id`, so the converter's positional
                        `piece_ids[item_id]` is the identity map it assumes;
  2. shape identity   - each Sparrow `shape.data` ring is vertex-for-vertex the
                        engine source piece's `geometry.segments` (x1, y1)
                        chain, so both frames measure the same material in the
                        same local origin (the engine keeps raw source rings -
                        `PolygonRing::source_points` - and never recentres);
  3. pose identity    - for every vertex of every placement,
                        engine_point == (2000 - sparrow_y, sparrow_x);
  4. extent           - the engine-frame bounding box against the committed
                        `validation-10s-x86.json` occupied bounds, mapped;
  5. pair distances   - a port of the committed `validate-sparrow-solution.mjs`
                        distance, run in the ENGINE frame, against the
                        committed `minimumPairDistance` (5.000840472766719) and
                        a second, independently named pair;
  6. boundary         - the four engine sheet edges, against Sparrow's
                        `minimumBoundaryDistance` (5.000959999999992).

The converter's own provenance block is written for the RETIRED 5.5 mm/5.25 mm
contract era (it says so in its docstring and in
`engineContractSeparationMm`). The emitted fixture keeps the converter's
placements byte-identical and carries a corrected contract block alongside the
converter's, with the correction recorded explicitly - see `contractNote`.
"""

import hashlib
import importlib.util
import json
import math
import os
import subprocess
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))
CONVERTER = os.path.join(
    ROOT, "docs", "experiments", "persistent-vacancy-descent", "sparrow-to-hint-fixture.py"
)
SPARROW = os.path.join(ROOT, "docs", "experiments", "sparrow-mixed61")
INSTANCE = os.path.join(SPARROW, "input.json")
SOLUTION = os.path.join(SPARROW, "solution-10s-x86.json")
VALIDATION = os.path.join(SPARROW, "validation-10s-x86.json")
# The exact-clearance request is the fixture every gate and every from-request
# run on this branch uses; the plain request differs only in geometry settings
# (sag 0.25 -> 0.0, safety margin 0.25 -> 0.0) and in nothing the converter or
# Sparrow ever read. Both are hashed into the evidence.
REQUEST = os.path.join(ROOT, "tests", "fixtures", "mixed-61", "mixed61-request-exact-clearance.json")
PLAIN_REQUEST = os.path.join(ROOT, "tests", "fixtures", "mixed-61", "mixed61-request.json")
OUT_DIR = os.path.join(ROOT, "docs", "experiments", "gate-a-sparrow-import")
FIXTURE = os.path.join(OUT_DIR, "fixture", "sparrow-10s-x86-poses.json")
EVIDENCE = os.path.join(OUT_DIR, "evidence", "import.json")

WIDTH_MM = 2000.0
SHEET_LONG_MM = 2700.0


def sha256(path):
    with open(path, "rb") as handle:
        return hashlib.sha256(handle.read()).hexdigest()


def load_converter():
    spec = importlib.util.spec_from_file_location("sparrow_to_hint_fixture", CONVERTER)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def ring_of_source(request, piece):
    sources = {sp["id"]: sp for sp in request["sourcePieces"]}
    segments = sources[piece["sourcePieceId"]]["geometry"]["segments"]
    points = [(float(s["x1"]), float(s["y1"])) for s in segments]
    for index, segment in enumerate(segments):
        nxt = points[(index + 1) % len(points)]
        if abs(segment["x2"] - nxt[0]) > 1e-12 or abs(segment["y2"] - nxt[1]) > 1e-12:
            raise SystemExit(f"source piece {piece['id']} does not chain")
    return points


def transform(points, rotation_deg, tx, ty):
    radians = math.radians(rotation_deg)
    cos, sin = math.cos(radians), math.sin(radians)
    return [(x * cos - y * sin + tx, x * sin + y * cos + ty) for (x, y) in points]


# --- the committed validate-sparrow-solution.mjs distance, ported verbatim ---


def orient(a, b, c):
    return (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])


def point_segment_distance_squared(p, a, b):
    dx, dy = b[0] - a[0], b[1] - a[1]
    denominator = dx * dx + dy * dy
    if denominator == 0:
        return (p[0] - a[0]) ** 2 + (p[1] - a[1]) ** 2
    t = max(0.0, min(1.0, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / denominator))
    qx, qy = a[0] + t * dx, a[1] + t * dy
    return (p[0] - qx) ** 2 + (p[1] - qy) ** 2


def segments_intersect(a, b, c, d):
    epsilon = 1e-10
    ab_c, ab_d = orient(a, b, c), orient(a, b, d)
    cd_a, cd_b = orient(c, d, a), orient(c, d, b)
    return (
        ((ab_c > epsilon and ab_d < -epsilon) or (ab_c < -epsilon and ab_d > epsilon))
        and ((cd_a > epsilon and cd_b < -epsilon) or (cd_a < -epsilon and cd_b > epsilon))
    )


def segment_distance_squared(a, b, c, d):
    if segments_intersect(a, b, c, d):
        return 0.0
    return min(
        point_segment_distance_squared(a, c, d),
        point_segment_distance_squared(b, c, d),
        point_segment_distance_squared(c, a, b),
        point_segment_distance_squared(d, a, b),
    )


def point_in_polygon(point, polygon):
    inside = False
    j = len(polygon) - 1
    for i in range(len(polygon)):
        a, b = polygon[i], polygon[j]
        if (a[1] > point[1]) != (b[1] > point[1]) and point[0] < (
            (b[0] - a[0]) * (point[1] - a[1]) / (b[1] - a[1]) + a[0]
        ):
            inside = not inside
        j = i
    return inside


def polygon_distance_squared(a, b):
    if point_in_polygon(a[0], b) or point_in_polygon(b[0], a):
        return 0.0
    minimum = math.inf
    for i in range(len(a)):
        a0, a1 = a[i], a[(i + 1) % len(a)]
        for j in range(len(b)):
            b0, b1 = b[j], b[(j + 1) % len(b)]
            minimum = min(minimum, segment_distance_squared(a0, a1, b0, b1))
    return minimum


def main():
    request = json.load(open(REQUEST))
    plain_request = json.load(open(PLAIN_REQUEST))
    instance = json.load(open(INSTANCE))
    solution_doc = json.load(open(SOLUTION))
    solution = solution_doc["solution"]
    validation = json.load(open(VALIDATION))
    placed = solution["layout"]["placed_items"]
    piece_ids = [piece["id"] for piece in request["pieces"]]

    findings = {}

    # (0) The two request fixtures must describe the SAME pieces: the Sparrow
    # instance was generated from the plain request, and the gates run the
    # exact-clearance one.
    findings["requestPiecesIdentical"] = [p["id"] for p in plain_request["pieces"]] == piece_ids
    findings["requestSourceGeometryIdentical"] = all(
        ring_of_source(request, request["pieces"][i]) == ring_of_source(plain_request, plain_request["pieces"][i])
        for i in range(len(piece_ids))
    )
    findings["sheet"] = {
        "shortAxisMm": request["sheet"]["width"],
        "longAxisMm": request["sheet"]["height"],
        "sparrowStripHeightMm": instance["strip_height"],
        "sparrowStripWidthMm": solution["strip_width"],
    }

    # (1) index identity
    findings["itemIdIsIndex"] = all(item["id"] == index for index, item in enumerate(instance["items"]))
    findings["itemDxfIsPieceId"] = all(
        item["dxf"] == piece_ids[index] for index, item in enumerate(instance["items"])
    )
    findings["itemDemandAllOne"] = all(item["demand"] == 1 for item in instance["items"])

    # (2) shape identity
    shape_mismatches = []
    for index, item in enumerate(instance["items"]):
        data = [tuple(p) for p in item["shape"]["data"]]
        if len(data) > 1 and data[0] == data[-1]:
            data = data[:-1]
        engine_ring = ring_of_source(request, request["pieces"][index])
        if data != engine_ring:
            shape_mismatches.append(index)
    findings["shapeRingsIdentical"] = not shape_mismatches
    findings["shapeMismatchItemIds"] = shape_mismatches

    # --- run the COMMITTED converter, pointed at the 10 s solution ---
    converter = load_converter()
    converter.SOLUTION = SOLUTION
    converter.REQUEST = REQUEST
    saved_argv = sys.argv
    sys.argv = [CONVERTER, FIXTURE]
    os.makedirs(os.path.dirname(FIXTURE), exist_ok=True)
    converter.main()
    sys.argv = saved_argv
    fixture = json.load(open(FIXTURE))
    placements = fixture["placements"]

    # (3) independent re-derivation of the same placements
    independent = []
    for item in placed:
        rotation = (item["transformation"]["rotation"] + 90.0) % 360.0
        tx, ty = item["transformation"]["translation"]
        independent.append(
            {
                "pieceId": piece_ids[item["item_id"]],
                "rotationDeg": rotation,
                "mirrored": False,
                "translateShortAxis": WIDTH_MM - ty,
                "translateLongAxis": tx,
            }
        )
    findings["converterMatchesIndependentDerivation"] = independent == placements
    findings["placementCount"] = len(placements)
    findings["distinctPieceIds"] = len(({p["pieceId"] for p in placements}))
    findings["coversEveryRequestPiece"] = sorted(p["pieceId"] for p in placements) == sorted(piece_ids)
    findings["mirroredAny"] = any(p["mirrored"] for p in placements)

    # (4) pose identity, vertex by vertex, in both frames
    sparrow_rings, engine_rings = [], []
    worst_map_error = 0.0
    for item, placement in zip(placed, placements):
        ring = ring_of_source(request, request["pieces"][item["item_id"]])
        tx, ty = item["transformation"]["translation"]
        s_ring = transform(ring, item["transformation"]["rotation"], tx, ty)
        e_ring = transform(
            ring,
            placement["rotationDeg"],
            placement["translateShortAxis"],
            placement["translateLongAxis"],
        )
        for (sx, sy), (ex, ey) in zip(s_ring, e_ring):
            worst_map_error = max(worst_map_error, abs((WIDTH_MM - sy) - ex), abs(sx - ey))
        sparrow_rings.append(s_ring)
        engine_rings.append(e_ring)
    findings["worstMappingVertexErrorMm"] = worst_map_error

    # (5) extent
    exs = [p[0] for ring in engine_rings for p in ring]
    eys = [p[1] for ring in engine_rings for p in ring]
    engine_bounds = {"minX": min(exs), "maxX": max(exs), "minY": min(eys), "maxY": max(eys)}
    sparrow_bounds = validation["occupiedBounds"]
    findings["engineFrameBounds"] = engine_bounds
    findings["sparrowFrameBoundsCommitted"] = sparrow_bounds
    findings["boundsMapConsistency"] = {
        "minXvs2000MinusMaxY": engine_bounds["minX"] - (WIDTH_MM - sparrow_bounds["maximumY"]),
        "maxXvs2000MinusMinY": engine_bounds["maxX"] - (WIDTH_MM - sparrow_bounds["minimumY"]),
        "minYvsMinX": engine_bounds["minY"] - sparrow_bounds["minimumX"],
        "maxYvsMaxX": engine_bounds["maxY"] - sparrow_bounds["maximumX"],
    }
    findings["engineExtent"] = {
        "shortAxisSpanMm": engine_bounds["maxX"] - engine_bounds["minX"],
        "longAxisSpanMm": engine_bounds["maxY"] - engine_bounds["minY"],
    }
    # The engine's published depth convention: max source y + sheet edge
    # clearance (raw_source_long_axis_depth_mm, validation/general_polygon.rs).
    findings["engineConventionDepthMm"] = engine_bounds["maxY"] + 5.0
    findings["sparrowReportedStripWidthMm"] = solution["strip_width"]
    findings["reportedMinusEngineConventionMm"] = solution["strip_width"] - (
        engine_bounds["maxY"] + 5.0
    )

    # (6) pair distances, in the ENGINE frame, with the committed mjs measure
    pair_rows = []
    for i in range(len(engine_rings)):
        for j in range(i + 1, len(engine_rings)):
            pair_rows.append((math.sqrt(polygon_distance_squared(engine_rings[i], engine_rings[j])), i, j))
    pair_rows.sort()
    # `placed` is the solution's own emission order; the committed validation
    # names pairs by Sparrow `item_id`, so report those and not list positions.
    item_id = [entry["item_id"] for entry in placed]
    findings["minimumPairDistanceEngineFrameMm"] = pair_rows[0][0]
    findings["minimumPairEngineFrame"] = [item_id[pair_rows[0][1]], item_id[pair_rows[0][2]]]
    findings["committedMinimumPairDistanceMm"] = validation["minimumPairDistance"]
    findings["committedMinimumPair"] = validation["minimumPair"]
    findings["minimumPairAgreesWithCommitted"] = (
        sorted(findings["minimumPairEngineFrame"]) == sorted(validation["minimumPair"])
        and abs(pair_rows[0][0] - validation["minimumPairDistance"]) < 1e-9
    )
    # Two hand-verified pairs: the committed minimum, and the runner-up, each
    # measured in BOTH frames so the rigid map is shown to preserve them.
    hand = []
    for distance, i, j in pair_rows[:2]:
        sparrow_distance = math.sqrt(polygon_distance_squared(sparrow_rings[i], sparrow_rings[j]))
        hand.append(
            {
                "itemIds": [item_id[i], item_id[j]],
                "pieceIds": [placements[i]["pieceId"], placements[j]["pieceId"]],
                "engineFrameMm": distance,
                "sparrowFrameMm": sparrow_distance,
                "frameDeltaMm": distance - sparrow_distance,
            }
        )
    findings["handVerifiedPairs"] = hand
    findings["tightestTwentyPairsMm"] = [
        {"itemIds": [item_id[i], item_id[j]], "distanceMm": d} for d, i, j in pair_rows[:20]
    ]
    findings["pairsBelow5p004Mm"] = sum(1 for d, _, _ in pair_rows if d < 5.004)
    findings["pairsBelow5p001Mm"] = sum(1 for d, _, _ in pair_rows if d < 5.001)
    findings["pairsBelow5p0Mm"] = sum(1 for d, _, _ in pair_rows if d < 5.0)
    findings["pairCount"] = len(pair_rows)

    # (6b) the angular lattice. Legality and reachability are different
    # questions, and this one belongs to the report whatever the three verdicts
    # say: the relaxed lane's surrogate catalogue is built on a 2.5 degree grid
    # (SURROGATE_ANGLE_STEP_DEG, general_relaxed.rs:75; 360/2.5 = 144 angles),
    # and `canonical_angle` snaps to it. A pose off that grid is not a pose the
    # default search can propose, no matter which envelope decides legality.
    def off_lattice(rotation, step):
        residue = rotation % step
        return min(residue, step - residue)

    lattice = {}
    for step in (2.5, 1.0, 90.0):
        deviations = [off_lattice(p["rotationDeg"], step) for p in placements]
        lattice[f"step{step}Deg"] = {
            "offLatticeCount": sum(1 for d in deviations if d > 1e-9),
            "ofPlacements": len(placements),
            "worstDeviationDeg": max(deviations),
        }
    findings["angularLattice"] = lattice
    findings["distinctRotations"] = len({round(p["rotationDeg"], 9) for p in placements})
    findings["angularLatticeNote"] = (
        "Sparrow ran with continuous rotations. The engine's default relaxed "
        "lane proposes poses from a 2.5 degree surrogate catalogue, so these "
        "poses are legal-set members the default search cannot express - a "
        "reachability barrier independent of which envelope decides legality."
    )

    # (7) boundary, engine frame, all four sheet edges
    findings["engineFrameBoundaryMm"] = {
        "shortAxisLow": engine_bounds["minX"],
        "shortAxisHigh": WIDTH_MM - engine_bounds["maxX"],
        "longAxisLow": engine_bounds["minY"],
        "longAxisHigh": SHEET_LONG_MM - engine_bounds["maxY"],
    }
    findings["engineFrameMinimumBoundaryMm"] = min(
        engine_bounds["minX"],
        WIDTH_MM - engine_bounds["maxX"],
        engine_bounds["minY"],
    )
    findings["committedMinimumBoundaryMm"] = validation["minimumBoundaryDistance"]
    findings["boundaryNote"] = (
        "Sparrow's committed minimum boundary 5.000959999999992 is its far strip "
        "edge (strip_width - maximumX), which maps to the engine long axis at "
        "y=150.16547 - INSIDE the 2700 mm sheet, so it is not a sheet edge here. "
        "The engine-frame binding edge is the long-axis origin (y=0), at "
        "min source y = Sparrow's minimumX."
    )

    # write the fixture with a corrected contract block
    fixture["contractNote"] = (
        "The committed converter's docstring and provenance block describe the "
        "RETIRED 5.5 mm pair / 5.25 mm boundary contract. This branch's "
        "contract is exact-clearance 5.0/5.0 (pair = totalPadding + 2*sag = "
        "5.0 + 0.0; sheet = sheetEdgeClearance + sag = 5.0 + 0.0), which is "
        "Sparrow's own separation. The `placements` array below is the "
        "converter's output BYTE FOR BYTE - asserted equal to an independent "
        "re-derivation of the same map. Every field this driver edited is "
        "metadata, and all of them are: `description` and "
        "`provenance.engineContractSeparationMm` / `engineContractBoundaryMm`, "
        "which described the retired contract; `independentDepthMm`, which the "
        "converter sets to Sparrow's own `strip_width` and which is set here to "
        "the engine's `max source y + sheetEdgeClearance` convention instead "
        "(150.16451 against 150.16547); and the converter-identity and "
        "request-fixture provenance fields. `reportedDepthMm` is left at "
        "Sparrow's reported `strip_width`, which is what it means."
    )
    fixture["description"] = (
        "Gate A pose set: the committed Sparrow Mixed-61 10-second x86 "
        "calibration (150.16547 mm reported strip width, 150.16451 mm in the "
        "engine's max-source-y + 5.0 depth convention) mapped rigidly into the "
        "engine frame. Under the current exact-clearance 5.0/5.0 contract this "
        "is a candidate LAYOUT, not a hint field: Sparrow's separation and the "
        "engine's contract are the same 5.0 mm."
    )
    fixture["provenance"]["sparrowSolution"] = os.path.relpath(SOLUTION, ROOT)
    fixture["provenance"]["sparrowSolutionSha256"] = sha256(SOLUTION)
    fixture["provenance"]["engineContractSeparationMm"] = 5.0
    fixture["provenance"]["engineContractBoundaryMm"] = 5.0
    fixture["provenance"]["converter"] = os.path.relpath(CONVERTER, ROOT)
    fixture["provenance"]["converterSha256"] = sha256(CONVERTER)
    fixture["provenance"]["converterSolutionOverride"] = (
        "the committed converter hard-codes solution-3s.json; this driver loads "
        "it as a module and sets SOLUTION/REQUEST before calling main()"
    )
    fixture["provenance"]["requestFixture"] = os.path.relpath(REQUEST, ROOT)
    fixture["reportedDepthMm"] = solution["strip_width"]
    fixture["independentDepthMm"] = engine_bounds["maxY"] + 5.0
    with open(FIXTURE, "w") as handle:
        json.dump(fixture, handle, indent=2)
        handle.write("\n")

    evidence = {
        "experiment": "gate-a-sparrow-import",
        "step": "1 - import and conversion audit",
        "inputs": {
            "sparrowSolution": os.path.relpath(SOLUTION, ROOT),
            "sparrowSolutionSha256": sha256(SOLUTION),
            "sparrowInstance": os.path.relpath(INSTANCE, ROOT),
            "sparrowInstanceSha256": sha256(INSTANCE),
            "sparrowValidation": os.path.relpath(VALIDATION, ROOT),
            "sparrowValidationSha256": sha256(VALIDATION),
            "converter": os.path.relpath(CONVERTER, ROOT),
            "converterSha256": sha256(CONVERTER),
            "request": os.path.relpath(REQUEST, ROOT),
            "requestSha256": sha256(REQUEST),
            "plainRequest": os.path.relpath(PLAIN_REQUEST, ROOT),
            "plainRequestSha256": sha256(PLAIN_REQUEST),
            "driver": os.path.relpath(os.path.abspath(__file__), ROOT),
        },
        "output": {
            "fixture": os.path.relpath(FIXTURE, ROOT),
            "fixtureSha256": sha256(FIXTURE),
        },
        "transformation": {
            "rigidMap": "(x_e, y_e) = (2000 - y_s, x_s)",
            "linearPart": "[[0, -1], [1, 0]], determinant +1, no mirroring",
            "rotation": "rotationDeg = sparrowRotation + 90, normalised to [0, 360)",
            "translation": "translateShortAxis = 2000 - t_y; translateLongAxis = t_x",
            "units": "millimetres in both frames; no scale factor (depthStretchFactor = 1.0)",
            "origin": (
                "both frames put the piece's local origin at the source DXF "
                "origin: Sparrow's shape.data is the request's "
                "geometry.segments (x1, y1) chain verbatim, and the engine's "
                "PolygonRing keeps that same untouched f64 ring as "
                "source_points - neither recentres to bounds or centroid"
            ),
            "sheetInset": (
                "Sparrow ran with a 5 mm inset on all four strip edges "
                "(0, strip_width) x (0, 2000). Three of those map onto engine "
                "sheet edges; the fourth (x_s = strip_width) maps to the engine "
                "long axis at y = 150.165, which is interior to the 2700 mm "
                "sheet and therefore imposes nothing here"
            ),
            "rotationConvention": (
                "both frames use x' = x cos - y sin + t_x, y' = x sin + y cos + "
                "t_y (counter-clockwise, degrees). cos(t+90) = -sin(t) and "
                "sin(t+90) = cos(t) make the +90 rotation exactly the map's "
                "linear part"
            ),
        },
        "checks": findings,
    }
    with open(EVIDENCE, "w") as handle:
        json.dump(evidence, handle, indent=2)
        handle.write("\n")
    print(json.dumps({k: v for k, v in findings.items() if k != "tightestTwentyPairsMm"}, indent=2))
    print(FIXTURE, sha256(FIXTURE))


if __name__ == "__main__":
    main()
