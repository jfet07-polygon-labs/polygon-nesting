//! `f64` axis-aligned boxes, and nothing else.
//!
//! The reject rule is a **proof**, not an estimate: the gap between two
//! axis-aligned boxes is a lower bound on the distance between the material
//! inside them, so `box_gap >= c_pair` proves `v_ij = 0` and the row can be
//! zeroed without one segment query. Sol review 14 §5 forbids `GridSet` here
//! for the opposite reason - it is a canonical Boolean endpoint, not a
//! directional field, and rounding a continuous broad phase to 1 µm would put
//! quantization inside the move loop.

use super::contact::box_gap;

/// Whether a pair of boxes can possibly hold material closer than `clearance`.
#[inline]
pub fn pair_is_near(first: [f64; 4], second: [f64; 4], clearance: f64) -> bool {
    box_gap(first, second) < clearance
}

/// The four boundary residuals of one box against the strip.
///
/// `[left, right, bottom, top]`, each `>= 0`. `depth_target_mm` is the locked
/// strip `T`, not the sheet: the strip is a hard boundary of the objective, and
/// the spec refuses `E + lambda * D` precisely so that the optimizer can never
/// trade illegality against depth.
#[inline]
pub fn boundary_residuals(
    box_mm: [f64; 4],
    short_axis_mm: f64,
    depth_target_mm: f64,
    edge_clearance_mm: f64,
) -> [f64; 4] {
    [
        (edge_clearance_mm - box_mm[0]).max(0.0),
        (box_mm[2] - (short_axis_mm - edge_clearance_mm)).max(0.0),
        (edge_clearance_mm - box_mm[1]).max(0.0),
        (box_mm[3] - (depth_target_mm - edge_clearance_mm)).max(0.0),
    ]
}
