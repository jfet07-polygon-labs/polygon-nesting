//! The exact tier, off the generic seam.
//!
//! This module holds the two `f64` Clipper services the engine's
//! publication-adjacent code consults — the expanded collision-polygon build
//! and the pair-overlap verdict — and it holds them *outside*
//! [`ExplorationKernel`](super::ExplorationKernel). Nothing here is generic:
//! there is no type parameter, no trait, and no implementation of one, so there
//! is no substitution a caller could perform to change which code answers.
//!
//! # Why the exact tier left the trait
//!
//! PR3 declared both tiers on one trait and relied on a convention — "exact
//! call sites name `LEGACY`" — to keep the exact answers off a generic
//! parameter. Sol's third review named that as the seam's fourth defect: the
//! convention was not enforced, so future generic code could still write
//! `K::exact_pair_overlaps` and reroute a publication-authority answer through
//! whatever kernel happened to be substituted.
//!
//! The trait no longer declares those methods, which turns the misuse into a
//! compile error rather than a review finding, and the services moved here
//! behind a token.
//!
//! # The token, and what it actually enforces
//!
//! [`ExactAuthority`] carries one field whose type is private to this module.
//! Both of Rust's construction forms — the tuple/struct literal and any
//! constructor function — are therefore unavailable outside it, so a value of
//! this type cannot be *forged*: it can only be received. The two exact
//! services are inherent methods on the token, and the functions that
//! implement them are private to this module, so the token is the only door.
//!
//! The one grant in the crate is [`LegacyKernel::exact_authority`], an
//! *inherent* method on the concrete legacy kernel. That is the property worth
//! stating precisely, because it is mechanical rather than conventional:
//!
//! * a function generic over `K: ExplorationKernel` has no `K::collision_polygon`
//!   and no `K::exact_pair_overlaps` to call — they are not on the trait;
//! * it also cannot reach the grant, because `exact_authority` is inherent to
//!   [`LegacyKernel`] and `K` is not that type;
//! * and it cannot construct an [`ExactAuthority`] to hand to a helper, because
//!   the token's field is unnameable outside this module.
//!
//! So the only way any code in this crate reaches exact geometry is by *naming*
//! the legacy kernel. That is Sol's "separate named legacy services", and the
//! token is what makes the naming load-bearing instead of decorative: grep
//! `exact_authority` and the result is the complete list of exact call sites.
//!
//! [`LegacyKernel`]: super::LegacyKernel
//! [`LegacyKernel::exact_authority`]: super::LegacyKernel::exact_authority
//!
//! # What this is not
//!
//! It is not the publisher. The independent validator in
//! [`crate::validation::general_polygon`] remains the sole publication
//! authority and does not consult this module at all; the token's name records
//! that these two answers are the ones a published placement is *measured*
//! with, not that holding one publishes anything.

use crate::domain::IrregularBounds;
use crate::geometry::general_polygon::{GeneralPolygonError, PolygonSet};
use crate::profiling::{self, Counter, Phase};
use crate::search::general_fast::bounds_have_positive_overlap;

use super::KernelPose;

/// The capability to ask the `f64` Clipper truth.
///
/// Zero-sized and unforgeable: see the module documentation. Obtained from
/// [`LegacyKernel::exact_authority`](super::LegacyKernel::exact_authority),
/// which is the crate's only grant.
pub(crate) struct ExactAuthority {
    /// Unnameable outside this module, which is what makes the token
    /// unconstructible outside it. Never read; it exists to deny the literal.
    _grant: Grant,
}

/// The private witness [`ExactAuthority`] is built from.
struct Grant;

impl ExactAuthority {
    /// Mints the token.
    ///
    /// Visible to the kernel module tree only, so the grant that hands one to
    /// the rest of the crate is a single named legacy service rather than a
    /// crate-wide constructor.
    #[inline(always)]
    pub(super) fn grant() -> Self {
        Self { _grant: Grant }
    }

    /// Builds the exact, expanded collision polygon for one pose.
    ///
    /// `expansion_mm` is the contract's collision expansion; the result is the
    /// source ring transformed by `pose` and offset outward by it.
    #[inline(always)]
    pub(crate) fn collision_polygon(
        &self,
        source: &PolygonSet,
        pose: KernelPose,
        expansion_mm: f64,
    ) -> Result<PolygonSet, GeneralPolygonError> {
        build_collision_polygon(source, pose, expansion_mm)
    }

    /// Whether two exact collision polygons overlap with positive area.
    ///
    /// `first_bounds`/`second_bounds` are the operands' already-derived
    /// extents. Callers that ask the same polygon about many partners pass them
    /// so the broad-phase reject does not re-walk a ring per pair; an empty
    /// extent is an error, exactly as it is for a caller that derives it here.
    #[inline(always)]
    pub(crate) fn pair_overlaps(
        &self,
        first: &PolygonSet,
        first_bounds: Option<IrregularBounds>,
        second: &PolygonSet,
        second_bounds: Option<IrregularBounds>,
    ) -> Result<bool, GeneralPolygonError> {
        exact_pair_overlaps_within(first, first_bounds, second, second_bounds)
    }
}

/// The exact collision-polygon build.
///
/// One Clipper transform followed by one Clipper offset. This is the
/// `collisionPolygonBuild` cost centre PR1 measured, and the only place the
/// engine turns a source ring into the expanded ring that every exact overlap
/// question is asked about.
///
/// It carries no instrumentation of its own. Its two callers measure it
/// differently on purpose: the constructor route opens a
/// [`Phase::CollisionPolygonBuild`] span around it, while the deep-operator
/// route uses [`profiling::deep`], which is compiled out by default because a
/// runtime branch there perturbs the surrounding generated function's inlining.
/// Owning a span here would force one of those two contracts onto the other.
fn build_collision_polygon(
    source: &PolygonSet,
    pose: KernelPose,
    expansion_mm: f64,
) -> Result<PolygonSet, GeneralPolygonError> {
    source
        .transformed(
            pose.rotation_deg,
            pose.mirrored,
            pose.translate_x,
            pose.translate_y,
        )?
        .offset(expansion_mm)
}

/// The exact `f64` pair-overlap verdict.
///
/// This is the `exactOverlapTest` cost centre PR1 measured.
fn exact_pair_overlaps_within(
    first: &PolygonSet,
    first_bounds: Option<IrregularBounds>,
    second: &PolygonSet,
    second_bounds: Option<IrregularBounds>,
) -> Result<bool, GeneralPolygonError> {
    let (Some(first_bounds), Some(second_bounds)) = (first_bounds, second_bounds) else {
        return Err(GeneralPolygonError::from_message(
            "an exact overlap query requires non-empty polygons",
        ));
    };
    if !bounds_have_positive_overlap(first_bounds, second_bounds) {
        return Ok(false);
    }
    // Instrumented past the broad-phase reject on purpose. The reject arm runs
    // hundreds of millions of times in a deep-operator stream, and it is not
    // exact-overlap work anyway - it is the bounds filter in front of it.
    // Guarding it measurably slowed the stream; guarding only the narrow phase
    // measures the cost that matters and costs nothing on the common path.
    let _span = profiling::span(Phase::ExactOverlapTest);
    profiling::count(Counter::ExactPairTests, 1);
    Ok(first.intersection_area_mm2(second)? > 0.0)
}
