//! The strip homotopy schedule — **a stub in this round, deliberately.**
//!
//! This round's task is the vertical slice and Gate 0: the falsifier cells all
//! run at a *locked* strip, because a schedule is exactly the thing that can
//! hide a field that cannot legalize. Grok review 9 Round 2 §1.4 puts it
//! plainly - "if T fails, jumps are not licensed" - and the same holds for
//! epochs: a homotopy that keeps bisecting toward a reachable target would turn
//! a Gate-0 failure into a slow success and make the round uninterpretable.
//!
//! So [`StripSchedule::target_mm`] returns the target it was handed, and
//! [`StripSchedule::on_epoch`] returns it unchanged. The next agent owns this
//! file. What it has to implement is already specified and is written down here
//! rather than in a ticket, so the seam is visible from the code:
//!
//! * `L`, the safe request-level lower scale, from raw material area, usable
//!   sheet width, edge clearance and the tallest piece — **not**
//!   `portfolio::area_lower_bound_depth_mm`, which offsets with the
//!   miter/search allowance. [`lower_scale_mm`] below is that derivation and is
//!   already used by the C175 cell, so it is measured, not deferred.
//! * `T_0 = D* - 0.10 * (D* - L)`, with the affine compression bisected onto
//!   `T_0` ([`affine_compression_factor`] is here for the same reason).
//! * eight equal-work epochs at ten seconds; on a successful publication set
//!   `D*` to the raw exact depth and take the next 10 %-residual target; on an
//!   epoch-limit failure set `T <- (T + D*) / 2` **retaining the infeasible
//!   state**, never restarting.
//! * publication repair stays inside the same `T`; the strip is never enlarged
//!   to rescue a failed target.

use super::state::{Contract, PieceSource, Pose};

/// The strip target the descent is currently locked into.
///
/// The stub holds one immutable number. The type exists now so that the
/// schedule can be added without touching a single call site in `mod.rs`.
#[derive(Clone, Copy, Debug)]
pub struct StripSchedule {
    target_mm: f64,
}

impl StripSchedule {
    /// A locked strip. Every Gate-0 cell uses this constructor.
    pub fn locked(target_mm: f64) -> Self {
        Self { target_mm }
    }

    pub fn target_mm(&self) -> f64 {
        self.target_mm
    }

    /// The stub: the target after an epoch is the target before it, whatever
    /// happened. A real schedule contracts on success and bisects on failure.
    pub fn on_epoch(&mut self, _published_depth_mm: Option<f64>) -> f64 {
        self.target_mm
    }
}

/// `L`: a safe request-level lower scale for the strip depth.
///
/// Two independent bounds, whichever is larger, plus the two edge clearances
/// the depth convention always contains:
///
/// * **area.** The material cannot be thinner than `total area / usable width`
///   however it is arranged.
/// * **the tallest piece.** A piece occupies at least its own minimum width
///   over all rotations, whatever angle it is placed at. `min(bbox width,
///   bbox height)` would over-state that and make `L` unsafe, which is why
///   `decomposition::minimum_width` computes the real supporting-line width.
///
/// This deliberately does not call `portfolio::area_lower_bound_depth_mm`: that
/// bound is offset with the miter/search allowance and so is not a statement
/// about raw material.
///
/// **Sag-aware, and asymmetrically so.** The usable width and the floor are
/// bounded by two *physical* sheet edges, which cost `edge + sag` each; the
/// depth this returns is in the sag-less publication convention, whose top term
/// is `depth_top_inset_mm`. On triangle-20 (`sag = 0.25`) that is
/// `60.0 + 5.25 + 5.0 = 70.25`, not the 70.0 the symmetric `2 * edge` produced
/// (Sol review 15 §A.1). On mixed-61 `sag = 0` and `L` is unchanged to the last
/// bit, which is why C175's `T = D - 0.10 (D - L)` does not move.
pub fn lower_scale_mm(sources: &[PieceSource], contract: &Contract) -> f64 {
    let edge = contract.physical_edge_clearance_mm();
    let usable_width = (contract.sheet_short_axis_mm - 2.0 * edge).max(f64::MIN_POSITIVE);
    let mut area = 0.0f64;
    let mut tallest = 0.0f64;
    for source in sources {
        area += source.area_mm2;
        tallest = tallest.max(source.min_width_mm);
    }
    (area / usable_width).max(tallest) + edge + contract.depth_top_inset_mm()
}

/// The affine factor that compresses a layout's centroids along the long axis
/// onto `target_mm`, found by bisection on the resulting depth.
///
/// Shapes stay rigid: only the centroid offsets scale, so the overlap the shock
/// creates is distributed through the layout instead of piling up against the
/// top boundary. That is the whole reason the spec prefers this to a uniform
/// SE(2) throw.
pub fn affine_compression_factor(
    sources: &[PieceSource],
    poses: &[Pose],
    contract: &Contract,
    target_mm: f64,
) -> f64 {
    let mut low = 0.0f64;
    let mut high = 1.0f64;
    if depth_after(sources, poses, contract, high) <= target_mm {
        return high;
    }
    for _ in 0..64 {
        let middle = (low + high) / 2.0;
        if depth_after(sources, poses, contract, middle) <= target_mm {
            low = middle;
        } else {
            high = middle;
        }
    }
    low
}

/// The poses a compression factor produces. Rigid: `theta` and the mirror are
/// untouched and only the long-axis offset from the strip's floor scales.
///
/// The floor is the **physical** bottom edge, `edge + sag`, because that is
/// what Phi's bottom row charges. Anchoring the compression at the sag-less
/// inset instead pushed every centroid one sag tolerance below the row that
/// judges it and *manufactured* bottom residuals of up to `sag` on any request
/// with `sag > 0` (Grok review 10 §B.2). On mixed-61 `sag = 0` and this is a
/// no-op to the last bit.
pub fn compressed(
    sources: &[PieceSource],
    poses: &[Pose],
    contract: &Contract,
    factor: f64,
) -> Vec<Pose> {
    let floor = contract.physical_edge_clearance_mm();
    sources
        .iter()
        .zip(poses)
        .map(|(source, pose)| {
            let (sin, cos) = super::state::pose_sin_cos(pose.theta_deg);
            let centre = super::state::apply_pose(
                source.centroid,
                pose.mirrored,
                sin,
                cos,
                pose.tx_mm,
                pose.ty_mm,
            );
            let shifted = floor + (centre[1] - floor) * factor;
            Pose {
                tx_mm: pose.tx_mm,
                ty_mm: pose.ty_mm + (shifted - centre[1]),
                theta_deg: pose.theta_deg,
                mirrored: pose.mirrored,
            }
        })
        .collect()
}

fn depth_after(
    sources: &[PieceSource],
    poses: &[Pose],
    contract: &Contract,
    factor: f64,
) -> f64 {
    let compressed = compressed(sources, poses, contract, factor);
    let mut deepest = f64::NEG_INFINITY;
    for (source, pose) in sources.iter().zip(&compressed) {
        let (sin, cos) = super::state::pose_sin_cos(pose.theta_deg);
        for point in &source.decomposition.ring {
            let placed = super::state::apply_pose(
                *point,
                pose.mirrored,
                sin,
                cos,
                pose.tx_mm,
                pose.ty_mm,
            );
            deepest = deepest.max(placed[1]);
        }
    }
    deepest + contract.sheet_edge_clearance_mm
}
