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
    let mut contact = closest_feature(a, b);
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
fn closest_feature(a: &[[f64; 2]], b: &[[f64; 2]]) -> Contact {
    let mut best = f64::INFINITY;
    let mut witness_a = a[0];
    let mut witness_b = b[0];
    for first in 0..a.len() {
        let a0 = a[first];
        let a1 = a[(first + 1) % a.len()];
        for second in 0..b.len() {
            let b0 = b[second];
            let b1 = b[(second + 1) % b.len()];
            let (distance, pa, pb) = segment_distance(a0, a1, b0, b1);
            if distance < best {
                best = distance;
                witness_a = pa;
                witness_b = pb;
            }
        }
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
