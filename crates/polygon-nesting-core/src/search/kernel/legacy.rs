//! The kernel the engine has always used.
//!
//! [`LegacyKernel`] is the reference implementation of [`ExplorationKernel`]
//! and the default binding everywhere. Its proxy tier is the relaxed triangle
//! surrogate — a triangulation of the exact expanded collision ring, queried
//! through a per-shape bin index with a strict separating-axis test. Its exact
//! tier is `f64` Clipper, unchanged.
//!
//! Nothing here is new geometry. Every method forwards to the function that
//! already implemented it, so this file is a *naming* of the seam rather than a
//! reimplementation of anything behind it. That is what makes the boundary
//! provable: the two pinned regression gates are bit-identical across its
//! introduction because the instruction stream is the same one.

use crate::domain::IrregularBounds;
use crate::geometry::general_polygon::{GeneralPolygonError, PolygonSet};
use crate::search::general_fast::{build_collision_polygon, exact_pair_overlaps_within};
use crate::search::general_relaxed::{
    pole_overlap_pressure, surrogate_pair_collides, OrientedSurrogate,
};

use super::{ExplorationKernel, KernelPose, KernelProbes, PosedShape};

/// The current geometry kernel: relaxed triangle surrogates for the proxy tier,
/// `f64` Clipper for the exact tier.
///
/// Zero-sized, so a lane that owns one owns nothing, and every method is
/// `#[inline(always)]`, so a monomorphised call is the same direct call the
/// search used to make.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LegacyKernel;

/// The legacy kernel as a constant, for the exact-tier call sites.
///
/// Publication-adjacent code names this instead of taking a kernel parameter.
/// The point is that there is no type substitution which can reroute an exact
/// overlap or a collision-polygon build: those call sites are not generic, so a
/// faster or `f32` kernel cannot reach them even by mistake. See the module
/// documentation of [`crate::search::kernel`].
pub const LEGACY: LegacyKernel = LegacyKernel;

impl ExplorationKernel for LegacyKernel {
    type Shape = OrientedSurrogate;

    #[inline(always)]
    fn pair_collides(
        &mut self,
        first: PosedShape<'_, Self::Shape>,
        second: PosedShape<'_, Self::Shape>,
        probes: &mut KernelProbes,
    ) -> bool {
        surrogate_pair_collides(
            first.shape,
            first.translate_x,
            first.translate_y,
            second.shape,
            second.translate_x,
            second.translate_y,
            probes,
        )
    }

    #[inline(always)]
    fn pair_pressure(
        &self,
        first: PosedShape<'_, Self::Shape>,
        second: PosedShape<'_, Self::Shape>,
    ) -> f64 {
        pole_overlap_pressure(
            first.shape,
            first.translate_x,
            first.translate_y,
            second.shape,
            second.translate_x,
            second.translate_y,
        )
    }

    #[inline(always)]
    fn collision_polygon(
        &self,
        source: &PolygonSet,
        pose: KernelPose,
        expansion_mm: f64,
    ) -> Result<PolygonSet, GeneralPolygonError> {
        build_collision_polygon(source, pose, expansion_mm)
    }

    #[inline(always)]
    fn exact_pair_overlaps(
        &self,
        first: &PolygonSet,
        first_bounds: Option<IrregularBounds>,
        second: &PolygonSet,
        second_bounds: Option<IrregularBounds>,
    ) -> Result<bool, GeneralPolygonError> {
        exact_pair_overlaps_within(first, first_bounds, second, second_bounds)
    }
}
