//! The kernel the engine has always used.
//!
//! [`LegacyKernel`] is the reference implementation of [`ExplorationKernel`]
//! and the default binding everywhere. Its proxy tier is the relaxed triangle
//! surrogate — a triangulation of the exact expanded collision ring, queried
//! through a per-shape bin index with a strict separating-axis test. The exact
//! tier is `f64` Clipper, unchanged, and it is reached through
//! [`LegacyKernel::exact_authority`] rather than through [`ExplorationKernel`].
//!
//! Nothing here is new geometry. Every method forwards to the function that
//! already implemented it, so this file is a *naming* of the seam rather than a
//! reimplementation of anything behind it. That is what makes the boundary
//! provable: the pinned regression gates are bit-identical across its
//! introduction because the instruction stream is the same one.

use crate::search::general_relaxed::{
    pole_overlap_pressure, surrogate_pair_collides, OrientedSurrogate,
};

use super::exact::ExactAuthority;
use super::{ExplorationKernel, KernelProbes, PosedShape};

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
/// documentation of [`crate::search::kernel::exact`].
pub const LEGACY: LegacyKernel = LegacyKernel;

impl LegacyKernel {
    /// The crate's one grant of exact-tier authority.
    ///
    /// This is an *inherent* method on the concrete kernel, not a trait method,
    /// and that is the whole mechanism: a function generic over
    /// `K: ExplorationKernel` cannot call it, because `K` is not this type and
    /// the trait does not declare it. Reaching the `f64` Clipper answers
    /// therefore requires naming [`LegacyKernel`] — usually through [`LEGACY`]
    /// — at the call site, which is what makes the exact call sites
    /// enumerable by grepping one symbol.
    ///
    /// See [`crate::search::kernel::exact`] for what the token does and does
    /// not enforce.
    #[inline(always)]
    pub(crate) fn exact_authority(&self) -> ExactAuthority {
        ExactAuthority::grant()
    }
}

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
}
