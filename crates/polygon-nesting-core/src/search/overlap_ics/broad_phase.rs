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
use super::state::Contract;

/// Whether a pair of boxes can possibly hold material closer than `clearance`.
#[inline]
pub fn pair_is_near(first: [f64; 4], second: [f64; 4], clearance: f64) -> bool {
    box_gap(first, second) < clearance
}

/// The four boundary residuals of one box, `[left, right, bottom, top]`, each
/// `>= 0`.
///
/// **Two different clearances meet here and they are not the same number.**
///
/// * left, right and bottom are true edges of the physical sheet, so they are
///   charged the material contract's own `edge + sag`
///   ([`Contract::physical_edge_clearance_mm`]).
/// * the top is the tighter of two constraints: the **locked strip** `T`,
///   charged in the sag-less depth convention
///   ([`Contract::depth_top_inset_mm`]) because that is the convention
///   `raw_source_depth_mm` and the `proxy_depth <= T` publication gate are
///   written in - and the **physical sheet top** at `sheet_long_axis_mm`,
///   charged `edge + sag` like the other three. On every Gate-0 cell
///   `T << sheet_long_axis_mm` so the strip binds, but the sheet top is a real
///   boundary and is not omitted for that reason.
///
/// The contract is taken whole rather than as a clearance scalar on purpose:
/// the previous signature let a caller hand one clearance to all four sides,
/// and five callers did (Sol review 15 §A.1, Grok review 10 Finding 1). There
/// is now no argument to get wrong.
///
/// `depth_target_mm` is the locked strip `T`, not the sheet: the strip is a
/// hard boundary of the objective, and the spec refuses `E + lambda * D`
/// precisely so that the optimizer can never trade illegality against depth.
#[inline]
pub fn boundary_residuals(
    box_mm: [f64; 4],
    contract: &Contract,
    depth_target_mm: f64,
) -> [f64; 4] {
    let physical = contract.physical_edge_clearance_mm();
    let strip_top = depth_target_mm - contract.depth_top_inset_mm();
    let sheet_top = contract.sheet_long_axis_mm - physical;
    [
        (physical - box_mm[0]).max(0.0),
        (box_mm[2] - (contract.sheet_short_axis_mm - physical)).max(0.0),
        (physical - box_mm[1]).max(0.0),
        (box_mm[3] - strip_top.min(sheet_top)).max(0.0),
    ]
}
