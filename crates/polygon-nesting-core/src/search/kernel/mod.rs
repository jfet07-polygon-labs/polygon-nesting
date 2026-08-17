//! The exploration-kernel seam.
//!
//! Every geometric service the exploration hot loop consumes is declared here
//! as one trait, [`ExplorationKernel`], so that a faster implementation can be
//! measured against the current one without editing a single line of search
//! logic. The current code path is [`LegacyKernel`]; it is the default and the
//! only implementation any production route binds.
//!
//! # Why a trait, and why here
//!
//! The fixed-stream profile taken in PR1 named the cost centres this boundary
//! has to enclose. In the mode-0 and mode-22 streams the leaf phases are
//! dominated by three families:
//!
//! | Cost centre | Phase / counter | Scale on the pinned streams |
//! |---|---|---|
//! | proxy pair collision test | [`Phase::PairCollide`] | ~22.8M calls |
//! | exact overlap evaluation | [`Phase::ExactOverlapTest`] | deep-operator dominated |
//! | collision polygon construction | [`Phase::CollisionPolygonBuild`] | one Clipper transform+offset each |
//!
//! [`Phase::PairCollide`]: crate::profiling::Phase::PairCollide
//! [`Phase::ExactOverlapTest`]: crate::profiling::Phase::ExactOverlapTest
//! [`Phase::CollisionPolygonBuild`]: crate::profiling::Phase::CollisionPolygonBuild
//!
//! Those three, plus the proxy scoring hook that turns a reported collision
//! into a magnitude, are exactly the trait's method set.
//!
//! # Two tiers, and why they are not interchangeable
//!
//! The trait declares two tiers, and the difference between them is a
//! *contract*, not a performance note.
//!
//! * **Proxy tier** — [`ExplorationKernel::pair_collides`] and
//!   [`ExplorationKernel::pair_pressure`]. These rank and prune candidates.
//!   They are allowed to be approximate, and a replacement kernel is expected
//!   to change their numeric representation. Search binds this tier
//!   *generically*: [`crate::search::general_relaxed`]'s lane search carries a
//!   `K: ExplorationKernel` type parameter that defaults to [`LegacyKernel`],
//!   so swapping a kernel in is a type substitution at the entry point.
//!
//! * **Exact tier** — [`ExplorationKernel::collision_polygon`] and
//!   [`ExplorationKernel::exact_pair_overlaps`]. These are the `f64`
//!   Clipper answers that publication-adjacent code consults. Every call site
//!   that can reach a published placement binds this tier to the *named*
//!   [`LegacyKernel`] through [`LEGACY`], never through a generic parameter.
//!   That is deliberate and is the structural form of the roadmap's refusal to
//!   "put `f32`, a tolerance, or Jagua in the publication authority": there is
//!   no type substitution that can reroute an exact answer, because no exact
//!   call site is generic. The independent validator in
//!   [`crate::validation::general_polygon`] remains the sole publisher and is
//!   not part of this seam at all.
//!
//!   The exact tier is still declared on the trait because a kernel has to be
//!   able to *ask* the exact question — the deep operators confirm proxy
//!   verdicts before they commit — and because a kernel that answered it
//!   differently would be a bug we want the type system to make visible. The
//!   experimental [`JaguaKernel`] therefore forwards its whole exact tier to
//!   [`LegacyKernel`] verbatim.
//!
//! # Cost of the boundary
//!
//! The legacy path pays nothing for it. [`LegacyKernel`] is a zero-sized type,
//! every method is `#[inline(always)]`, and the generic lane search has exactly
//! one instantiation in every build that does not opt into another kernel, so
//! monomorphisation reproduces the previous direct calls. There is no `dyn` on
//! any hot path. The two pinned regression gates (mode-20 anchor, mode-22
//! record replay) are bit-identical across this change, which is the evidence
//! that the boundary is free rather than the claim that it should be.
//!
//! # What is *not* behind the seam yet
//!
//! PR3 opens the *query* seam. The oriented-shape representation the proxy
//! tier consumes is still the legacy surrogate, and the catalogue that owns it
//! is still concrete, so the lane search binds `K::Shape = OrientedSurrogate`.
//! A kernel that accelerates the query over that representation — an `f32` SoA
//! BVH, a quadtree, a bitmask index — is swappable today. A kernel with its own
//! shape representation additionally needs the catalogue, the NFP builder, and
//! the pose-bounds helper moved behind [`ExplorationKernel::Shape`], which is
//! PR4/PR6 work.

use crate::domain::IrregularBounds;
use crate::geometry::general_polygon::{GeneralPolygonError, PolygonSet};

pub mod legacy;

pub use legacy::{LegacyKernel, LEGACY};

/// One oriented shape placed at a translation.
///
/// The exploration tier never rotates a shape at query time: the rotation and
/// mirror are baked into the prepared shape and only the translation varies per
/// candidate. Keeping that split explicit in the argument type is what lets a
/// kernel precompute whatever it wants per orientation.
#[derive(Clone, Copy)]
pub struct PosedShape<'a, S> {
    /// The prepared, already-oriented shape.
    pub shape: &'a S,
    /// Translation along the sheet short axis, in millimetres.
    pub translate_x: f64,
    /// Translation along the sheet long axis, in millimetres.
    pub translate_y: f64,
}

impl<'a, S> PosedShape<'a, S> {
    /// Poses `shape` at `(translate_x, translate_y)`.
    #[inline(always)]
    pub fn new(shape: &'a S, translate_x: f64, translate_y: f64) -> Self {
        Self {
            shape,
            translate_x,
            translate_y,
        }
    }
}

/// The rigid pose a collision polygon is built at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KernelPose {
    /// Rotation in degrees, unrounded.
    pub rotation_deg: f64,
    /// Whether the source ring is mirrored before rotation.
    pub mirrored: bool,
    /// Translation along the sheet short axis, in millimetres.
    pub translate_x: f64,
    /// Translation along the sheet long axis, in millimetres.
    pub translate_y: f64,
}

impl KernelPose {
    /// The pose that only orients a shape, leaving it at the origin.
    #[inline(always)]
    pub fn oriented(rotation_deg: f64, mirrored: bool) -> Self {
        Self {
            rotation_deg,
            mirrored,
            translate_x: 0.0,
            translate_y: 0.0,
        }
    }
}

/// Deterministic work a proxy query performed.
///
/// These are *quota* quantities, not diagnostics: the relaxed profile charges
/// them against per-job caps, so they have to survive the move behind the trait
/// unchanged. A kernel reports the work it actually did in its own terms; the
/// caller folds the totals into its counters. Both fields are the same units
/// and the same width as the lane counters they feed, so folding a batch total
/// and incrementing one at a time agree exactly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KernelProbes {
    /// Broad-phase probes into a shape's cell/spatial index.
    pub cell_index_probes: usize,
    /// Narrow-phase separating-axis tests actually evaluated.
    pub sat_tests: usize,
}

impl KernelProbes {
    /// Adds `other`'s work to this block.
    #[inline(always)]
    pub fn accumulate(&mut self, other: KernelProbes) {
        self.cell_index_probes = self.cell_index_probes.wrapping_add(other.cell_index_probes);
        self.sat_tests = self.sat_tests.wrapping_add(other.sat_tests);
    }
}

/// The geometric services the exploration hot loop consumes.
///
/// See the module documentation for the two-tier contract. In short: the proxy
/// tier is swappable and approximate, the exact tier is `f64` Clipper truth and
/// is never bound through a generic parameter on a publication-adjacent path.
pub trait ExplorationKernel {
    /// The kernel's own oriented-shape representation.
    ///
    /// The legacy kernel names the relaxed surrogate here. A kernel with a
    /// different representation is buildable and testable today but cannot yet
    /// be handed to the lane search, which still owns a concrete catalogue; see
    /// the module documentation.
    type Shape;

    // ---- proxy tier ----------------------------------------------------

    /// Whether two posed shapes overlap, as the exploration tier sees it.
    ///
    /// This is the hottest call in every measured stream (~22.8M invocations on
    /// the pinned mode-0/mode-22 pair). Implementations must be deterministic:
    /// the same two posed shapes must produce the same verdict and the same
    /// [`KernelProbes`] on every run, on the same platform, because both feed
    /// deterministic quotas.
    fn pair_collides(
        &mut self,
        first: PosedShape<'_, Self::Shape>,
        second: PosedShape<'_, Self::Shape>,
        probes: &mut KernelProbes,
    ) -> bool;

    /// The overlap magnitude of a pair the kernel has already reported as
    /// colliding.
    ///
    /// This is a ranking signal only. It never decides feasibility, and it is
    /// never compared against a clearance: a zero here does not mean legal, and
    /// only [`Self::exact_pair_overlaps`] and the independent validator can say
    /// that a placement is publishable.
    fn pair_pressure(
        &self,
        first: PosedShape<'_, Self::Shape>,
        second: PosedShape<'_, Self::Shape>,
    ) -> f64;

    // ---- exact tier ----------------------------------------------------

    /// Builds the exact, expanded collision polygon for one pose.
    ///
    /// `expansion_mm` is the contract's collision expansion; the result is the
    /// source ring transformed by `pose` and offset outward by it.
    fn collision_polygon(
        &self,
        source: &PolygonSet,
        pose: KernelPose,
        expansion_mm: f64,
    ) -> Result<PolygonSet, GeneralPolygonError>;

    /// Whether two exact collision polygons overlap with positive area.
    ///
    /// `first_bounds`/`second_bounds` are the operands' already-derived
    /// extents. Callers that ask the same polygon about many partners pass them
    /// so the broad-phase reject does not re-walk a ring per pair; an empty
    /// extent is an error, exactly as it is for a caller that derives it here.
    fn exact_pair_overlaps(
        &self,
        first: &PolygonSet,
        first_bounds: Option<IrregularBounds>,
        second: &PolygonSet,
        second_bounds: Option<IrregularBounds>,
    ) -> Result<bool, GeneralPolygonError>;
}
