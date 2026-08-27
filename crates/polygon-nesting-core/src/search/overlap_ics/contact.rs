//! Φ's only primitive: the signed convex gap between two convex cells, with
//! the witnesses and the outward normal the descent needs for torque.
//!
//! ```text
//! s(A,B) = + distance(A,B)      when A and B are disjoint
//!        = - MTVdepth(A,B)      when their interiors meet
//! ```
//!
//! The converged spec (Sol R2 §2, Grok R2 §1.1) makes this the hot path and
//! keeps *two* independent oracles for it: the crate's existing
//! [`crate::validation::sat::measure_convex_sat_penetration`] for the
//! overlapping convex case, and the nine-point triangle Minkowski hull for the
//! triangle case. **Both live in `tests.rs` and neither is reachable from
//! here** — an oracle that shares code with the thing it audits is not an
//! oracle, and one compiled into the shipped module is dead weight in a hot
//! path.
//!
//! Three properties this function is written for, in order:
//!
//! 1. **allocation-free.** Not one `Vec`, not one iterator adaptor that boxes.
//!    The existing SAT allocates an axes `Vec` per call, which is why Sol R2 §2
//!    refuses it in the move loop and keeps it as a differential oracle.
//! 2. **signed and total.** The existing SAT returns `None` for both separation
//!    *and* exact contact, so it is not a field. This returns a number at every
//!    configuration, and containment is negative rather than absent.
//! 3. **deterministic.** Every extremum is taken with a strict `<`, scanning in
//!    index order, so ties resolve to the lowest index on every run.

/// One signed contact between two convex cells.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    /// `+distance` when disjoint, `-penetration` when overlapping.
    pub signed_gap_mm: f64,
    /// The unit direction in which moving cell `A` increases `signed_gap_mm`.
    pub normal: [f64; 2],
    /// The material witness on `A`.
    pub witness_a: [f64; 2],
    /// The material witness on `B`.
    pub witness_b: [f64; 2],
}

impl Contact {
    /// The contact seen from `B`: same gap, opposite normal, swapped witnesses.
    pub fn reversed(self) -> Self {
        Self {
            signed_gap_mm: self.signed_gap_mm,
            normal: [-self.normal[0], -self.normal[1]],
            witness_a: self.witness_b,
            witness_b: self.witness_a,
        }
    }
}

/// A conservative lower bound on `signed_gap` from the two cells' axis-aligned
/// boxes, used to skip a cell pair that cannot violate the clearance.
///
/// It is a **proof**, never an estimate: the box gap is at most the true
/// distance, so `box_gap >= clearance` proves the pair clears.
#[inline]
pub fn box_gap(a: [f64; 4], b: [f64; 4]) -> f64 {
    let dx = (b[0] - a[2]).max(a[0] - b[2]).max(0.0);
    let dy = (b[1] - a[3]).max(a[1] - b[3]).max(0.0);
    libm::hypot(dx, dy)
}

/// `box_gap(a, b) < threshold`, decided without the `hypot` wherever one leg
/// already settles it.
///
/// **This is the same predicate, not an approximation of it.** For
/// non-negative legs `hypot(dx, dy) >= max(dx, dy)` exactly, and `hypot` is
/// correctly rounded, so a leg at or above the threshold proves the gap is
/// too - no rounding can reverse it. `hypot(0, dy)` is exactly `dy`, so a zero
/// leg settles it as well. Only a pair whose two legs are both strictly inside
/// the threshold, the corner annulus, still pays for the square root.
///
/// The reason it is worth its own function: the pair broad phase is the single
/// most executed line in the engine - a ten-second mixed-61 request runs it
/// 1.7 billion times and rejects 93 % - and on a strip layout the rejects are
/// overwhelmingly *far in one axis*, which is the branch that now returns
/// before the second leg is even computed.
#[inline]
pub fn box_gap_below(a: [f64; 4], b: [f64; 4], threshold: f64) -> bool {
    let dx = (b[0] - a[2]).max(a[0] - b[2]).max(0.0);
    if dx >= threshold {
        return false;
    }
    let dy = (b[1] - a[3]).max(a[1] - b[3]).max(0.0);
    if dy >= threshold {
        return false;
    }
    // Both legs are below the threshold. A zero leg makes the hypotenuse the
    // other leg exactly, which the two tests above have already accepted.
    if dx == 0.0 || dy == 0.0 {
        return true;
    }
    libm::hypot(dx, dy) < threshold
}

/// The axis-aligned box `[min x, min y, max x, max y]` of a point run.
#[inline]
pub fn bounds(points: &[[f64; 2]]) -> [f64; 4] {
    let mut out = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
    for point in points {
        out[0] = out[0].min(point[0]);
        out[1] = out[1].min(point[1]);
        out[2] = out[2].max(point[0]);
        out[3] = out[3].max(point[1]);
    }
    out
}

#[inline]
fn project(points: &[[f64; 2]], axis: [f64; 2]) -> (f64, f64) {
    let mut low = f64::INFINITY;
    let mut high = f64::NEG_INFINITY;
    for point in points {
        let value = point[0] * axis[0] + point[1] * axis[1];
        low = low.min(value);
        high = high.max(value);
    }
    (low, high)
}

/// The support vertex of `points` in direction `axis`, lowest index on a tie.
#[inline]
fn support(points: &[[f64; 2]], axis: [f64; 2]) -> [f64; 2] {
    let mut best = points[0];
    let mut best_value = points[0][0] * axis[0] + points[0][1] * axis[1];
    for point in &points[1..] {
        let value = point[0] * axis[0] + point[1] * axis[1];
        if value > best_value {
            best_value = value;
            best = *point;
        }
    }
    best
}

/// The signed convex gap between two counter-clockwise convex cells.
///
/// `normal` points the way `a` must move to separate further. On the
/// overlapping branch it is the minimum-translation axis; on the separated
/// branch it is the direction from the witness on `b` to the witness on `a`,
/// which is the same vector for a nondegenerate closest feature.
/// [`convex_cell_gap`] with both cells' axes and own-projections supplied.
///
/// Identical arithmetic: the caller's cached axis is the same `f64` pair this
/// function would have computed, and the cached own-interval is the same
/// `project` of the same points on it. A zero axis marks the degenerate edge
/// the uncached path skips with `continue`.
/// The absolute slack, in millimetres, that turns a rounded lower bound into a
/// **proof**.
///
/// Every bound this module prunes on - a projection interval gap, a box gap -
/// is computed in `f64` from coordinates the contract quantises to the
/// micrometre. Each carries at most a handful of roundings on values no larger
/// than the sheet, so its absolute error is under `1e-12 mm`. Requiring the
/// bound to clear the cut by `1e-9 mm` is therefore a genuine proof with three
/// orders of magnitude to spare, and it costs nothing: it only declines to
/// prune a pair whose gap sits within a nanometre of the cut, and the contract
/// cannot distinguish those from the cut itself.
pub const PRUNE_SLACK_MM: f64 = 1e-9;

/// [`convex_cell_gap`] with both cells' axes and own-projections supplied, and
/// with the caller's **cut**: the gap at or above which the answer is thrown
/// away.
///
/// Identical arithmetic to the uncached spelling: the caller's cached axis is
/// the same `f64` pair this function would have computed, and the cached
/// own-interval is the same `project` of the same points on it. A zero axis
/// marks the degenerate edge the uncached path skips with `continue`.
///
/// **The cut is what makes it cheaper, not just tidier.** On the separated
/// branch the exact answer costs an `O(|a| * |b|)` segment scan, and on this
/// campaign's population 38 % of all queries spend it to return a gap the
/// caller then discards for being at or above the pair clearance. Every unit
/// axis carries a lower bound on the true distance - the gap between the two
/// projected intervals - so the scan of the axes that is already happening can
/// often *prove* the answer will be discarded. When it does, `None` comes back
/// and the segment scan never runs.
///
/// Two details keep the surviving answers bit-identical to the unpruned ones:
///
/// 1. the separating axis no longer ends the loop, because a later axis may
///    carry a stronger bound - but `touch_axis` still records the **first**
///    one, which is the only thing `finish_gap` reads from that loop; and
/// 2. the bound must clear the cut by [`PRUNE_SLACK_MM`], so no pair is pruned
///    whose exact answer could have been kept.
pub fn convex_cell_gap_cached(
    a: &[[f64; 2]],
    a_axes: &[[f64; 2]],
    a_own: &[[f64; 2]],
    b: &[[f64; 2]],
    b_axes: &[[f64; 2]],
    b_own: &[[f64; 2]],
    cut_mm: f64,
) -> Option<Contact> {
    debug_assert!(a.len() >= 3 && b.len() >= 3);
    let prune_at = cut_mm + PRUNE_SLACK_MM;
    let mut best_depth = f64::INFINITY;
    let mut best_axis = [0.0f64, 0.0];
    let mut separated = false;
    let mut touch_axis = [0.0f64, 0.0];
    for source in 0..2 {
        let (axes, own, count) = if source == 0 {
            (a_axes, a_own, a.len())
        } else {
            (b_axes, b_own, b.len())
        };
        for index in 0..count {
            let axis = axes[index];
            if axis == [0.0, 0.0] {
                continue;
            }
            let interval = own[index];
            let (a_low, a_high, b_low, b_high) = if source == 0 {
                let (b_low, b_high) = project(b, axis);
                (interval[0], interval[1], b_low, b_high)
            } else {
                let (a_low, a_high) = project(a, axis);
                (a_low, a_high, interval[0], interval[1])
            };
            let move_positive = b_high - a_low;
            let move_negative = a_high - b_low;
            if move_positive <= 0.0 || move_negative <= 0.0 {
                // The interval gap on a unit axis is a lower bound on the
                // distance between the cells.
                let separation = if move_positive <= 0.0 {
                    -move_positive
                } else {
                    -move_negative
                };
                if separation >= prune_at {
                    return None;
                }
                if !separated {
                    separated = true;
                    touch_axis = if move_positive <= 0.0 {
                        axis
                    } else {
                        [-axis[0], -axis[1]]
                    };
                }
                continue;
            }
            if separated {
                // Already proven disjoint; the overlap bookkeeping below is
                // dead and only a stronger separation bound is still wanted.
                continue;
            }
            let (depth, signed_axis) = if move_negative <= move_positive {
                (move_negative, [-axis[0], -axis[1]])
            } else {
                (move_positive, axis)
            };
            if depth < best_depth {
                best_depth = depth;
                best_axis = signed_axis;
            }
        }
    }
    Some(finish_gap(
        a,
        b,
        separated,
        best_depth,
        best_axis,
        touch_axis,
        cut_mm,
    ))
}

pub fn convex_cell_gap(a: &[[f64; 2]], b: &[[f64; 2]]) -> Contact {
    debug_assert!(a.len() >= 3 && b.len() >= 3);
    // Streamed SAT. No axes `Vec`: the two edge loops run in place, and the
    // first axis that separates ends the overlap question immediately.
    let mut best_depth = f64::INFINITY;
    let mut best_axis = [0.0f64, 0.0];
    let mut separated = false;
    // The axis that proved separation, oriented so that moving `a` along it
    // increases the gap. Kept for the exact-contact case below.
    let mut touch_axis = [0.0f64, 0.0];
    'axes: for source in 0..2 {
        let ring = if source == 0 { a } else { b };
        for index in 0..ring.len() {
            let first = ring[index];
            let second = ring[(index + 1) % ring.len()];
            let (dx, dy) = (second[0] - first[0], second[1] - first[1]);
            let length = libm::hypot(dx, dy);
            if !(length > 0.0) {
                continue;
            }
            // The outward normal of a counter-clockwise edge.
            let axis = [dy / length, -dx / length];
            let (a_low, a_high) = project(a, axis);
            let (b_low, b_high) = project(b, axis);
            let move_positive = b_high - a_low;
            let move_negative = a_high - b_low;
            if move_positive <= 0.0 || move_negative <= 0.0 {
                separated = true;
                // `move_positive <= 0` means `b` lies entirely on the negative
                // side of this axis, so `a` separates further along `+axis`;
                // `move_negative <= 0` is the mirror. Both can hold only when
                // both cells project to the same point, which the length guard
                // above has already excluded for this axis.
                touch_axis = if move_positive <= 0.0 {
                    axis
                } else {
                    [-axis[0], -axis[1]]
                };
                break 'axes;
            }
            let (depth, signed_axis) = if move_negative <= move_positive {
                (move_negative, [-axis[0], -axis[1]])
            } else {
                (move_positive, axis)
            };
            if depth < best_depth {
                best_depth = depth;
                best_axis = signed_axis;
            }
        }
    }
    finish_gap(
        a,
        b,
        separated,
        best_depth,
        best_axis,
        touch_axis,
        f64::INFINITY,
    )
}

/// The shared tail of both spellings of the streamed SAT.
fn finish_gap(
    a: &[[f64; 2]],
    b: &[[f64; 2]],
    separated: bool,
    best_depth: f64,
    best_axis: [f64; 2],
    touch_axis: [f64; 2],
    cut_mm: f64,
) -> Contact {
    if !separated && best_depth.is_finite() {
        let witness_a = support(a, [-best_axis[0], -best_axis[1]]);
        let witness_b = support(b, best_axis);
        return Contact {
            signed_gap_mm: -best_depth,
            normal: best_axis,
            witness_a,
            witness_b,
        };
    }
    let mut contact = closest_feature(a, b, cut_mm);
    if contact.normal == [0.0, 0.0] {
        // **Exact material contact keeps the SAT axis.** At `distance == 0` the
        // witness difference is the zero vector and `closest_feature` has no
        // direction to report - but the row's violation is `c_pair - 0`, which
        // is the *full* pair clearance and very much positive. A positive
        // violation with a zero normal contributes weight to Phi and no force
        // to the gradient: the piece is charged for an overlap it is given no
        // way to leave (Sol review 15 §B.6, `contact.rs:187`).
        //
        // The separating axis the streamed SAT stopped on is that direction,
        // and it is deterministic - the first axis in edge-index order that
        // proved zero overlap, oriented so that moving `a` along it increases
        // the gap.
        contact.normal = if touch_axis != [0.0, 0.0] {
            touch_axis
        } else {
            // Two degenerate rings (no edge with positive length) meeting at a
            // point. There is no geometric axis; +x is a fixed, deterministic,
            // documented choice rather than a zero that silently disables the
            // row.
            [1.0, 0.0]
        };
    }
    contact
}

/// The closest material feature between two disjoint (or touching) convex
/// cells, scanned in edge-index order.
///
/// `O(|a| * |b|)` segment pairs, which on this campaign's population is at most
/// `10 * 10`: mixed-61's rings are 3, 4, 6 and 10 vertices and every nonconvex
/// one is decomposed into triangles.
fn closest_feature(a: &[[f64; 2]], b: &[[f64; 2]], cut_mm: f64) -> Contact {
    let mut best = f64::INFINITY;
    let mut witness_a = a[0];
    let mut witness_b = b[0];
    // **The scan keeps its order and its strict `<`, and only skips pairs it
    // has proven cannot beat the incumbent.** `hypot(dx, dy) >= dx.max(dy)`
    // for non-negative legs, so the larger box-gap leg of a segment pair is a
    // lower bound on that pair's distance - two subtractions and two maxima,
    // against a `segment_distance` that costs divisions and a `hypot`.
    //
    // `bar` starts at the caller's cut because a distance at or above it is
    // discarded anyway, so the very first pairs are already pruned against a
    // real threshold rather than against infinity.
    let mut bar = cut_mm;
    for first in 0..a.len() {
        let a0 = a[first];
        let a1 = a[(first + 1) % a.len()];
        let (ax_low, ax_high) = if a0[0] < a1[0] { (a0[0], a1[0]) } else { (a1[0], a0[0]) };
        let (ay_low, ay_high) = if a0[1] < a1[1] { (a0[1], a1[1]) } else { (a1[1], a0[1]) };
        for second in 0..b.len() {
            let b0 = b[second];
            let b1 = b[(second + 1) % b.len()];
            let (bx_low, bx_high) = if b0[0] < b1[0] { (b0[0], b1[0]) } else { (b1[0], b0[0]) };
            let (by_low, by_high) = if b0[1] < b1[1] { (b0[1], b1[1]) } else { (b1[1], b0[1]) };
            let dx = (bx_low - ax_high).max(ax_low - bx_high);
            let dy = (by_low - ay_high).max(ay_low - by_high);
            if dx.max(dy) >= bar + PRUNE_SLACK_MM {
                continue;
            }
            let (distance, pa, pb) = segment_distance(a0, a1, b0, b1);
            if distance < best {
                best = distance;
                bar = best;
                witness_a = pa;
                witness_b = pb;
            }
        }
    }
    if !best.is_finite() {
        // Every pair was pruned against the caller's cut, which proves the
        // exact distance is above it and the caller discards the contact
        // whatever it holds. An infinite gap is the value that cannot be
        // mistaken for a real one by any caller's threshold, and it is exactly
        // what the empty pair row already carries.
        return Contact {
            signed_gap_mm: f64::INFINITY,
            normal: [0.0, 0.0],
            witness_a,
            witness_b,
        };
    }
    let (dx, dy) = (witness_a[0] - witness_b[0], witness_a[1] - witness_b[1]);
    let length = libm::hypot(dx, dy);
    let normal = if length > 0.0 {
        [dx / length, dy / length]
    } else {
        // Exact contact. The gap is zero and the direction is degenerate; the
        // spec's guard is that a zero-length normal contributes no force rather
        // than an arbitrary one.
        [0.0, 0.0]
    };
    Contact {
        signed_gap_mm: best,
        normal,
        witness_a,
        witness_b,
    }
}

/// Distance between two segments, with the realizing point on each.
#[inline]
fn segment_distance(
    a0: [f64; 2],
    a1: [f64; 2],
    b0: [f64; 2],
    b1: [f64; 2],
) -> (f64, [f64; 2], [f64; 2]) {
    let d1 = [a1[0] - a0[0], a1[1] - a0[1]];
    let d2 = [b1[0] - b0[0], b1[1] - b0[1]];
    let r = [a0[0] - b0[0], a0[1] - b0[1]];
    let aa = d1[0] * d1[0] + d1[1] * d1[1];
    let e = d2[0] * d2[0] + d2[1] * d2[1];
    let f = d2[0] * r[0] + d2[1] * r[1];
    let (mut s, mut t);
    if aa <= 0.0 && e <= 0.0 {
        s = 0.0;
        t = 0.0;
    } else if aa <= 0.0 {
        s = 0.0;
        t = (f / e).clamp(0.0, 1.0);
    } else {
        let c = d1[0] * r[0] + d1[1] * r[1];
        if e <= 0.0 {
            t = 0.0;
            s = (-c / aa).clamp(0.0, 1.0);
        } else {
            let b = d1[0] * d2[0] + d1[1] * d2[1];
            let denominator = aa * e - b * b;
            s = if denominator != 0.0 {
                ((b * f - c * e) / denominator).clamp(0.0, 1.0)
            } else {
                0.0
            };
            t = (b * s + f) / e;
            if t < 0.0 {
                t = 0.0;
                s = (-c / aa).clamp(0.0, 1.0);
            } else if t > 1.0 {
                t = 1.0;
                s = ((b - c) / aa).clamp(0.0, 1.0);
            }
        }
    }
    let pa = [a0[0] + d1[0] * s, a0[1] + d1[1] * s];
    let pb = [b0[0] + d2[0] * t, b0[1] + d2[1] * t];
    (libm::hypot(pa[0] - pb[0], pa[1] - pb[1]), pa, pb)
}
