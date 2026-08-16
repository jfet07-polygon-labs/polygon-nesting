#!/usr/bin/env python3
"""Defensible LOWER BOUNDS on Mixed-61 strip depth.

Engine contract (verified against code on branch engine/topology-archive-search):
  - pair separation: both real polygons are offset by
      expansion = total_padding/2 + clearance_safety_margin + CONSERVATIVE_OFFSET_ALLOWANCE
                = 2.5 + 0.25 + 0.002 = 2.752  (effective totalPadding 5)
    and the two offsets must not overlap  =>  real-boundary pair separation >= 5.504 >= 5.5.
    (general_fast.rs: collision_expansion_mm, validate_and_measure_placements)
  - boundary: the offset polygon must fit the sheet inset by
      collision_sheet_inset = edge_clearance - total_padding/2 = 5.0 - 2.5 = 2.5
    =>  real-boundary clearance to every sheet edge >= 2.5 + 2.752 = 5.252 >= 5.25.
  - published depth (general_relaxed.rs: coupled_independent_source_depth):
      D = max over pieces of (real polygon max_y) + edge_clearance(5.0).
  - strip width 2000 (short axis x), depth along y.

Lower-bound argument (all inequalities in the SAFE direction):
  Let r = 5.5/2 = 2.75.  Inflate every placed real piece by a disc of radius r
  (Minkowski sum).  Because real pair separation >= 2r, the inflated pieces have
  pairwise disjoint interiors.  Because real x in [5.25, 2000-5.25], the inflated
  pieces lie in a vertical strip of width  2000 - 2*(5.25 - 2.75) = 1995
  (identical to the variant 2000 - 2*5.25 + 2*2.75 = 1995).
  If the raw pieces span y-extent E, the inflated pieces lie in a band of height
  E + 2r, hence
        sum_i area(P_i (+) D_r)  <=  1995 * (E + 2r)
        =>  E >= SUM / 1995 - 5.5.
  Published depth: D = y_max + 5.0 = E + y_min + 5.0 >= E + 5.25 + 5.0
        =>  D >= SUM / 1995 + 4.75          [strengthened bound]
  and a fortiori D >= SUM / 1995            [plain bound, independent of any
                                             y-clearance / depth-metric argument].

  SUM must be a LOWER bound on the true inflated areas:
   - convex piece:  area(P (+) D_r) = A + p*r + pi*r^2  EXACTLY (Steiner formula).
   - non-convex piece: A + p*r + pi*r^2 is only an UPPER bound (offset bands
     overlap at reflex features), so using it would break the bound direction.
     Instead we certify a lower bound by grid counting: a cell of side h whose
     center c satisfies dist(c, P) <= r - h*sqrt(2)/2 is entirely inside
     P (+) D_r (dist(., P) is 1-Lipschitz and dist = 0 inside P).

Naive bound: raw pieces are disjoint and lie in [0,2000] x [0, y_max], and
  D >= y_max >= sum(raw areas)/2000.  (TRUE, very loose.)

Sparrow calibration (5.0 mm pair separation): r = 2.5.
  Assuming Sparrow also keeps items >= 2.5 from the strip sides and counts the
  2.5 margins in its reported depth, D_sparrow >= SUM_2.5 / 2000.
  Fully-assumption-free variant (no boundary clearance at all, depth = raw
  piece extent): D >= SUM_2.5 / 2005 - 5.0.
"""

import json
import math
import sys

import numpy as np

FIXTURE = "/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/tests/fixtures/mixed-61/mixed61-request.json"

STRIP_WIDTH = 2000.0
BOUNDARY_CLEARANCE = 5.25       # certified >= (actual 5.252)
PAIR_SEPARATION = 5.5           # certified >= (actual 5.504)
DEPTH_EDGE_ADD = 5.0            # coupled_independent_source_depth adds edge_clearance
R_ENGINE = PAIR_SEPARATION / 2.0            # 2.75
USABLE_WIDTH = STRIP_WIDTH - 2.0 * (BOUNDARY_CLEARANCE - R_ENGINE)   # 1995
R_SPARROW = 2.5

GRID_H = 0.02   # mm, certification cell size for non-convex offsets


def chain_polygon(segments):
    pts = []
    for seg in segments:
        if seg["kind"] != "line":
            raise ValueError("non-line segment: " + seg["kind"])
        pts.append((float(seg["x1"]), float(seg["y1"])))
    # verify chaining and closure
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
    X, Y = np.meshgrid(gx, gy)            # cell centers
    cx = X.ravel()
    cy = Y.ravel()

    # min distance from centers to polygon boundary (vectorized per edge)
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

    # point-in-polygon via crossing number (vectorized per edge)
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
    lower_cells = dist_to_P <= (r - half_diag)          # cell certified inside
    upper_cells = dist_to_P <= (r + half_diag)          # cell possibly touching
    cell = h * h
    return lower_cells.sum() * cell, upper_cells.sum() * cell


def main():
    with open(FIXTURE) as f:
        req = json.load(f)

    sources = {sp["id"]: sp for sp in req["sourcePieces"]}
    pieces = req["pieces"]
    assert len(pieces) == 61, len(pieces)

    # group identical geometries so grid work is done once per distinct shape
    shape_of_piece = []
    shapes = {}          # signature -> dict
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
        shape_of_piece.append(sig)

    print(f"{'shape':28s} {'n':>3s} {'convex':>6s} {'area':>10s} {'perim':>9s} "
          f"{'infl2.75 lo':>12s} {'infl2.75 hi/steiner':>19s}")

    def inflated_lower(shape, r):
        A, p = shape["area"], shape["perimeter"]
        steiner = A + p * r + math.pi * r * r
        if shape["convex"]:
            return steiner, steiner, steiner   # exact
        lo, hi = grid_offset_area_bounds(shape["pts"], r, GRID_H)
        # steiner is a valid upper bound for any simple polygon
        return lo, min(hi, steiner), steiner

    total_raw = 0.0
    total_infl_engine_lo = 0.0
    total_infl_engine_hi = 0.0
    total_infl_sparrow_lo = 0.0
    for sig, sh in shapes.items():
        lo_e, hi_e, steiner_e = inflated_lower(sh, R_ENGINE)
        lo_s, _, _ = inflated_lower(sh, R_SPARROW)
        total_raw += sh["count"] * sh["area"]
        total_infl_engine_lo += sh["count"] * lo_e
        total_infl_engine_hi += sh["count"] * hi_e
        total_infl_sparrow_lo += sh["count"] * lo_s
        print(f"{sh['label']:28s} {sh['count']:3d} {str(sh['convex']):>6s} "
              f"{sh['area']:10.3f} {sh['perimeter']:9.3f} {lo_e:12.3f} "
              f"{hi_e:9.3f}/{steiner_e:9.3f}")

    naive = total_raw / STRIP_WIDTH
    plain = total_infl_engine_lo / USABLE_WIDTH
    strengthened = plain + 2.0 * (BOUNDARY_CLEARANCE - R_ENGINE) \
        + (DEPTH_EDGE_ADD - (BOUNDARY_CLEARANCE - 2.0 * R_ENGINE))
    # strengthened = plain + (b - r) + (b_bottom... ) ; algebra below, printed explicitly
    # D = y_max + 5.0 >= E + y_min + 5.0 >= E + 5.25 + 5.0
    # E >= SUM/1995 - 5.5  =>  D >= SUM/1995 - 5.5 + 10.25 = SUM/1995 + 4.75
    strengthened = plain + 4.75

    sparrow_main = total_infl_sparrow_lo / STRIP_WIDTH
    sparrow_safe = total_infl_sparrow_lo / (STRIP_WIDTH + 2 * R_SPARROW) - 2 * R_SPARROW

    print()
    print(f"pieces: 61   distinct shapes: {len(shapes)}   grid h = {GRID_H} mm")
    print(f"raw area sum                 = {total_raw:.6f} mm^2")
    print(f"inflated sum r=2.75 (LOWER)  = {total_infl_engine_lo:.6f} mm^2")
    print(f"inflated sum r=2.75 (upper)  = {total_infl_engine_hi:.6f} mm^2 (sanity)")
    print(f"inflated sum r=2.50 (LOWER)  = {total_infl_sparrow_lo:.6f} mm^2")
    print(f"usable width both variants   = {USABLE_WIDTH:.6f} mm "
          f"(2000-2*(5.25-2.75) == 2000-2*5.25+2*2.75)")
    print()
    print(f"[TRUE] naive bound            D >= {naive:.4f} mm   (raw/2000)")
    print(f"[TRUE] contract bound (plain) D >= {plain:.4f} mm   (SUM_2.75/1995)")
    print(f"[TRUE] contract bound (+y)    D >= {strengthened:.4f} mm   "
          f"(SUM_2.75/1995 + 4.75; uses depth = max_y + 5.0 and bottom clearance 5.25)")
    print(f"[calibration, assumes Sparrow side clearance 2.5 counted in depth]")
    print(f"       sparrow 5.0mm bound    D >= {sparrow_main:.4f} mm   (SUM_2.5/2000)")
    print(f"[TRUE under zero-clearance]   D >= {sparrow_safe:.4f} mm   (SUM_2.5/2005 - 5)")
    print()
    print(f"contract-vs-sparrow bound gap: {strengthened - sparrow_main:.4f} mm")
    print(f"reference: sparrow 3s calibration 154.44858, engine best 168.277, "
          f"native floor 177.081, session target 155")

    return {
        "raw_area_sum_mm2": total_raw,
        "usable_width_mm": USABLE_WIDTH,
        "naive_bound_mm": naive,
        "contract_bound_plain_mm": plain,
        "contract_bound_strengthened_mm": strengthened,
        "sparrow_bound_mm": sparrow_main,
        "sparrow_bound_zero_clearance_mm": sparrow_safe,
    }


if __name__ == "__main__":
    out = main()
    json.dump(out, sys.stdout, indent=1)
    print()
