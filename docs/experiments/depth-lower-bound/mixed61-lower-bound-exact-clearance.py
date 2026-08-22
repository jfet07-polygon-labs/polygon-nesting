#!/usr/bin/env python3
"""The Mixed-61 depth lower bound, RE-PINNED for the exact-clearance contract.

`mixed61-lower-bound.py` next to this file computed the same construction for
the RETIRED 5.5 mm pair / 5.25 mm boundary contract and pinned
`contract_bound_strengthened_mm = 131.97838540260466`. That number is stale:
the branch recalibrated to an exact-clearance contract of **5.0 mm pair and
5.0 mm boundary** - the fixture is
`tests/fixtures/mixed-61/mixed61-request-exact-clearance.json`, whose geometry
block carries `flatteningSagToleranceMm = 0.0` and
`clearanceSafetyMarginMm = 0.0`, and every gate on this branch runs it with
positional `sheet-edge-clearance = 5` and `pair-clearance = 5`. Kimi review 1
flagged the staleness; this recomputes it.

The old file's `sparrow_bound_mm = 124.887` is NOT the replacement. That figure
is `SUM_2.5 / 2000`: the r = 2.5 inflated area over the FULL 2000 mm width,
with no boundary term at all. It was written as a calibration of an outside
packer under an assumption about what Sparrow counts, not as this engine's
bound, and using it as the engine's bound would drop both the usable-width
correction and the depth-metric term. Both are recomputed here.

Everything below is the committed script's construction with three constants
changed and one term re-derived; the geometry code - shoelace areas, the exact
Steiner formula for convex pieces, the certified 0.02 mm grid LOWER bound for
the non-convex stars - is character-for-character the same, because changing it
would make this a different bound rather than the same bound re-pinned.

CONTRACT, verified against the code at this commit:

  - pair, publication (`validation/general_polygon.rs`,
    `validate_publication_inner`):
        pair_clearance = total_padding + 2 * sag = 5.0 + 0.0 = 5.0
  - boundary, publication (`validate_sheet`):
        sheet_clearance = sheet_edge_clearance + sag = 5.0 + 0.0 = 5.0,
    on ALL FOUR sheet edges.
  - published depth (`raw_source_long_axis_depth_mm`):
        D = max over placed source vertices of y, + sheet_edge_clearance(5.0).
  - strip width 2000 mm (short axis x), depth along y.

DERIVATION (every inequality in the SAFE direction):

  Let r = 5.0 / 2 = 2.5. Inflate every placed piece by a disc of radius r.
  Pair separation >= 2r, so the inflated pieces have pairwise disjoint
  interiors. Material x lies in [5.0, 1995.0], so the inflated pieces lie in a
  strip of width 2000 - 2 * (5.0 - 2.5) = 1995.
  If the raw pieces span y-extent E, the inflated pieces lie in a band of
  height E + 2r = E + 5.0, hence

        SUM_2.5  <=  1995 * (E + 5.0)      =>   E >= SUM_2.5 / 1995 - 5.0

  and, using y_min >= 5.0 (the boundary clause) and D = y_max + 5.0,

        D = E + y_min + 5.0 >= E + 10.0    =>   D >= SUM_2.5 / 1995 + 5.0

  which is the STRENGTHENED bound. Without the depth-metric argument,

        D >= SUM_2.5 / 1995                      [PLAIN]

  A SECOND, STRICTLY STRONGER bound applies to anything the current acceptance
  authority actually publishes. `validate_and_measure_placements` additionally
  requires every pair's canonical MITER envelopes, offset by
  `expansion = total_padding/2 + clearance_safety_margin + allowance`, to be
  disjoint, and each to fit the sheet inset by
  `inset = sheet_edge_clearance - total_padding/2 = 2.5`. A miter offset
  CONTAINS the disc offset at the same radius, so miter-disjoint implies
  disc-disjoint implies material pair separation >= 2 * expansion, and
  miter-fits implies material boundary clearance >= inset + expansion. With the
  from-request allowance 0.002 that is r = 2.502 and b = 5.002; note
  b - r = 2.5 exactly, so the usable width is 1995 in this variant too:

        D >= SUM_2.502 / 1995 + (b + 5.0 - 2r) = SUM_2.502 / 1995 + 4.998

  This composite variant is the bound for the *shipping* authority. The plain
  and strengthened contract bounds above are the ones that stay true if the
  envelope is ever replaced, which is why both are pinned.

WHAT MOVED, AND WHY THE OLD NUMBER WAS NOT JUST WRONG BY THE CONTRACT DELTA:
  the retired contract inflated at r = 2.75 and added 4.75; this one inflates
  at r = 2.5 and adds 5.0. The usable width is 1995 in both, by coincidence of
  the two arithmetics (2000 - 2*(5.25 - 2.75) and 2000 - 2*(5.0 - 2.5)).
"""

import json
import math
import os
import sys

import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
FIXTURE = os.path.join(
    ROOT, "tests", "fixtures", "mixed-61", "mixed61-request-exact-clearance.json"
)
LEGACY_FIXTURE = os.path.join(ROOT, "tests", "fixtures", "mixed-61", "mixed61-request.json")

STRIP_WIDTH = 2000.0
BOUNDARY_CLEARANCE = 5.0        # sheet_edge_clearance + sag
PAIR_SEPARATION = 5.0           # total_padding + 2 * sag
DEPTH_EDGE_ADD = 5.0            # raw_source_long_axis_depth_mm adds edge clearance
R_CONTRACT = PAIR_SEPARATION / 2.0                                   # 2.5
USABLE_WIDTH = STRIP_WIDTH - 2.0 * (BOUNDARY_CLEARANCE - R_CONTRACT)  # 1995

# The composite variant: the acceptance authority's own envelope radius at the
# from-request allowance, and the boundary it implies.
SEARCH_OFFSET_ALLOWANCE_MM = 0.002
CLEARANCE_SAFETY_MARGIN_MM = 0.0
R_COMPOSITE = PAIR_SEPARATION / 2.0 + CLEARANCE_SAFETY_MARGIN_MM + SEARCH_OFFSET_ALLOWANCE_MM
COLLISION_INSET = BOUNDARY_CLEARANCE - PAIR_SEPARATION / 2.0          # 2.5
B_COMPOSITE = COLLISION_INSET + R_COMPOSITE
USABLE_WIDTH_COMPOSITE = STRIP_WIDTH - 2.0 * (B_COMPOSITE - R_COMPOSITE)

GRID_H = 0.02   # mm, certification cell size for non-convex offsets


def chain_polygon(segments):
    pts = []
    for seg in segments:
        if seg["kind"] != "line":
            raise ValueError("non-line segment: " + seg["kind"])
        pts.append((float(seg["x1"]), float(seg["y1"])))
    for i, seg in enumerate(segments):
        nxt = segments[(i + 1) % len(segments)]
        if abs(seg["x2"] - nxt["x1"]) > 1e-9 or abs(seg["y2"] - nxt["y1"]) > 1e-9:
            raise ValueError("segments do not chain")
    return pts


def shoelace_area(pts):
    s = 0.0
    n = len(pts)
    for i in range(n):
        x1, y1 = pts[i]
        x2, y2 = pts[(i + 1) % n]
        s += x1 * y2 - x2 * y1
    return abs(s) / 2.0


def perimeter(pts):
    n = len(pts)
    return sum(math.dist(pts[i], pts[(i + 1) % n]) for i in range(n))


def is_convex(pts):
    n = len(pts)
    sign = 0
    for i in range(n):
        ax, ay = pts[i]
        bx, by = pts[(i + 1) % n]
        cx, cy = pts[(i + 2) % n]
        cross = (bx - ax) * (cy - by) - (by - ay) * (cx - bx)
        if abs(cross) < 1e-9:
            continue
        s = 1 if cross > 0 else -1
        if sign == 0:
            sign = s
        elif s != sign:
            return False
    return True


def grid_offset_area_bounds(pts, r, h):
    """Certified (lower, upper) bounds on area(P (+) D_r) for simple polygon P."""
    xs = np.array([p[0] for p in pts])
    ys = np.array([p[1] for p in pts])
    minx, maxx = xs.min() - r - 2 * h, xs.max() + r + 2 * h
    miny, maxy = ys.min() - r - 2 * h, ys.max() + r + 2 * h
    gx = np.arange(minx + h / 2, maxx, h)
    gy = np.arange(miny + h / 2, maxy, h)
    X, Y = np.meshgrid(gx, gy)
    cx = X.ravel()
    cy = Y.ravel()

    mind = np.full(cx.shape, np.inf)
    n = len(pts)
    for i in range(n):
        x1, y1 = pts[i]
        x2, y2 = pts[(i + 1) % n]
        dx, dy = x2 - x1, y2 - y1
        L2 = dx * dx + dy * dy
        t = ((cx - x1) * dx + (cy - y1) * dy) / L2
        t = np.clip(t, 0.0, 1.0)
        px = x1 + t * dx
        py = y1 + t * dy
        d = np.hypot(cx - px, cy - py)
        np.minimum(mind, d, out=mind)

    inside = np.zeros(cx.shape, dtype=bool)
    for i in range(n):
        x1, y1 = pts[i]
        x2, y2 = pts[(i + 1) % n]
        cond = (y1 > cy) != (y2 > cy)
        with np.errstate(divide="ignore", invalid="ignore"):
            xint = x1 + (cy - y1) * (x2 - x1) / (y2 - y1)
        crosses = cond & (cx < xint)
        inside ^= crosses

    half_diag = h * math.sqrt(2.0) / 2.0
    dist_to_P = np.where(inside, 0.0, mind)
    lower_cells = dist_to_P <= (r - half_diag)
    upper_cells = dist_to_P <= (r + half_diag)
    cell = h * h
    return lower_cells.sum() * cell, upper_cells.sum() * cell


def load_shapes(path):
    with open(path) as handle:
        req = json.load(handle)
    sources = {sp["id"]: sp for sp in req["sourcePieces"]}
    pieces = req["pieces"]
    assert len(pieces) == 61, len(pieces)
    shapes = {}
    for p in pieces:
        sp = sources[p["sourcePieceId"]]
        pts = chain_polygon(sp["geometry"]["segments"])
        sig = tuple((round(x, 9), round(y, 9)) for (x, y) in pts)
        if sig not in shapes:
            shapes[sig] = {
                "label": sp["label"],
                "pts": pts,
                "count": 0,
                "area": shoelace_area(pts),
                "perimeter": perimeter(pts),
                "convex": is_convex(pts),
            }
        shapes[sig]["count"] += 1
    return shapes


def inflated_lower(shape, r):
    A, p = shape["area"], shape["perimeter"]
    steiner = A + p * r + math.pi * r * r
    if shape["convex"]:
        return steiner, steiner, steiner
    lo, hi = grid_offset_area_bounds(shape["pts"], r, GRID_H)
    return lo, min(hi, steiner), steiner


def main():
    shapes = load_shapes(FIXTURE)
    legacy = load_shapes(LEGACY_FIXTURE)
    # The two fixtures must describe the same material; only the geometry
    # settings differ, and the bound is a statement about material.
    same_material = sorted(
        (s["label"], s["count"], round(s["area"], 9)) for s in shapes.values()
    ) == sorted((s["label"], s["count"], round(s["area"], 9)) for s in legacy.values())

    print(
        f"{'shape':28s} {'n':>3s} {'convex':>6s} {'area':>10s} {'perim':>9s} "
        f"{'infl2.50 lo':>12s} {'infl2.502 lo':>13s}"
    )
    total_raw = 0.0
    total_contract = 0.0
    total_composite = 0.0
    total_contract_upper = 0.0
    for sh in shapes.values():
        lo_c, hi_c, _ = inflated_lower(sh, R_CONTRACT)
        lo_x, _, _ = inflated_lower(sh, R_COMPOSITE)
        total_raw += sh["count"] * sh["area"]
        total_contract += sh["count"] * lo_c
        total_contract_upper += sh["count"] * hi_c
        total_composite += sh["count"] * lo_x
        print(
            f"{sh['label']:28s} {sh['count']:3d} {str(sh['convex']):>6s} "
            f"{sh['area']:10.3f} {sh['perimeter']:9.3f} {lo_c:12.3f} {lo_x:13.3f}"
        )

    naive = total_raw / STRIP_WIDTH
    plain = total_contract / USABLE_WIDTH
    strengthened = plain + (BOUNDARY_CLEARANCE + DEPTH_EDGE_ADD - 2.0 * R_CONTRACT)
    composite_plain = total_composite / USABLE_WIDTH_COMPOSITE
    composite_strengthened = composite_plain + (
        B_COMPOSITE + DEPTH_EDGE_ADD - 2.0 * R_COMPOSITE
    )

    print()
    print(f"pieces: 61   distinct shapes: {len(shapes)}   grid h = {GRID_H} mm")
    print(f"same material as the legacy fixture: {same_material}")
    print(f"raw area sum                  = {total_raw:.6f} mm^2")
    print(f"inflated sum r=2.500 (LOWER)  = {total_contract:.6f} mm^2")
    print(f"inflated sum r=2.500 (upper)  = {total_contract_upper:.6f} mm^2 (sanity)")
    print(f"inflated sum r=2.502 (LOWER)  = {total_composite:.6f} mm^2")
    print(f"usable width                  = {USABLE_WIDTH:.6f} mm (contract), "
          f"{USABLE_WIDTH_COMPOSITE:.6f} mm (composite)")
    print()
    print(f"[TRUE] naive bound                 D >= {naive:.4f} mm   (raw/2000)")
    print(f"[TRUE] contract bound (plain)      D >= {plain:.4f} mm   (SUM_2.5/1995)")
    print(f"[TRUE] contract bound (+y)         D >= {strengthened:.4f} mm   "
          f"(SUM_2.5/1995 + {BOUNDARY_CLEARANCE + DEPTH_EDGE_ADD - 2.0 * R_CONTRACT:.2f})")
    print(f"[TRUE] composite bound (plain)     D >= {composite_plain:.4f} mm")
    print(f"[TRUE] composite bound (+y)        D >= {composite_strengthened:.4f} mm")
    print()
    print(f"SUPERSEDED (5.5/5.25 contract):    D >= 131.9784 mm")
    print(f"delta from the retired figure:     {strengthened - 131.97838540260466:+.4f} mm")

    return {
        "raw_area_sum_mm2": total_raw,
        "inflated_sum_r2_500_mm2": total_contract,
        "inflated_sum_r2_502_mm2": total_composite,
        "usable_width_mm": USABLE_WIDTH,
        "usable_width_composite_mm": USABLE_WIDTH_COMPOSITE,
        "naive_bound_mm": naive,
        "contract_bound_plain_mm": plain,
        "contract_bound_strengthened_mm": strengthened,
        "composite_bound_plain_mm": composite_plain,
        "composite_bound_strengthened_mm": composite_strengthened,
        "same_material_as_legacy_fixture": same_material,
        "superseded_5p5_contract_strengthened_mm": 131.97838540260466,
    }


if __name__ == "__main__":
    out = main()
    json.dump(out, sys.stdout, indent=1)
    print()
