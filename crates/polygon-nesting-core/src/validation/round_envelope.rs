//! The certified round-envelope kernel: `P (+) disc(r)` as an exact integer
//! predicate on the canonical grid.
//!
//! # What this replaces, and what it does not
//!
//! The engine publishes on a conjunction. One half is the **material contract**
//! ([`crate::validation::general_polygon::validate_publication`]) — untouched
//! `f64` source rings, pairwise boundary distance against `total_padding + 2 *
//! sag`, sheet-edge clearance against `sheet_edge_clearance + sag`. The other
//! half is the **canonical envelope**: every placement's source ring
//! canonicalized to the 1 µm grid, offset outward by
//! [`crate::search::general_fast::collision_expansion_mm`] with
//! `JoinType::Miter` at `CLIPPER_MITER_LIMIT = 2.0`, and required to fit the
//! inset sheet and to be pairwise disjoint.
//!
//! This module is a second implementation of *that second half only*, with the
//! miter join replaced by a disc. The material contract half is not touched by
//! anything here and remains the final authority on publication.
//!
//! docs/experiments/gate-a-sparrow-import/ measured what the join costs: on one
//! contract-legal 61-piece layout the miter envelope refuses 31 of 1830 pairs
//! and 2 of 61 boundaries at the contract radius, **all 31 and both 2 caused by
//! the join shape and none by the radius**, at a median price of 0.5057 mm of
//! material clearance on a refused pair and up to 2.3343 mm.
//!
//! # Why this is exact rather than discretized
//!
//! Sol review 11 asked for "discretizzazione soltanto outward, con errore
//! formalmente incluso nel margine". Gate A then measured the case that answer
//! cannot serve: pair 38·39 of the Sparrow import has **0.42 µm** of radius
//! margin at `r = 2.5`, which is *below* the 1 µm canonical grid step. Any
//! polygonal outward approximation of the disc with the error charged to the
//! margin refuses that pair; the layout is legal and the refusal would be an
//! artefact of the approximation.
//!
//! So there is no approximating polygon here at all. The canonicalized rings
//! are integers on a 1 µm grid, and on integers the two questions the envelope
//! half asks have exact integer answers:
//!
//! * **pair**: `P_i (+) disc(r)` and `P_j (+) disc(r)` have disjoint interiors
//!   iff the material sets are disjoint and their minimum boundary distance is
//!   at least `2r`. Squared distances between integer points and integer
//!   segments are *rationals* with integer numerator and denominator, so
//!   `d^2 >= (2r)^2` is one `i128` comparison after cross-multiplication. No
//!   `f64` appears in the decision.
//! * **boundary**: the inset sheet rectangle is axis-aligned, and a disc
//!   reaches exactly `r` past the material in every direction, so
//!   `P (+) disc(r)` fits it iff the material's integer bounding box, grown by
//!   `r` on each side, fits it. Four integer comparisons.
//!
//! The consequence is that the kernel's verdict is a function of the integer
//! grid alone: it is bit-identical across platforms, it has no rounding mode,
//! and it needs no error budget. That is the "certified" in the name, and it is
//! why [`i128`] and not `f64` is the arithmetic everywhere below.
//!
//! # Containment is a separate question and is asked separately
//!
//! Minimum *boundary* distance is not a legality test on its own: a small piece
//! strictly inside a large one has a large positive boundary distance and is an
//! overlap. The pair predicate therefore refuses on containment before it
//! credits any distance, exactly as
//! [`crate::validation::general_polygon::validate_publication`]'s
//! `material_sets_overlap` does — and for the same reason.
//!
//! # Economy
//!
//! Same broad phase as the exact-clearance contract validator's
//! `ClearanceSlabs`, in integers: an axis-aligned bounding-box gap of at least
//! `2r` is a proof of legality, and it certifies the great majority of pairs.
//! Below it, a ring-level box test and then a segment-level box test, both
//! exact and both in the same integers as the decision they guard. The narrow
//! path runs only on segment pairs no box test could separate.
//!
//! # Arming
//!
//! Off by default at compile time (`round-envelope-kernel`), and off by default
//! at run time even when compiled: [`set_kernel_mode`] follows the
//! `fast-contract-validator` promotion architecture from Sol review 8, so the
//! kernel is armed by the v3 coordinator for the duration of one run through an
//! RAII guard and by nothing else. With the feature off, none of this compiles
//! and the composite is HEAD's miter authority exactly — which is what the four
//! pinned regression gates prove.
//!
//! There are two armed modes and the difference between them is the round's
//! main finding. [`KernelMode::Exclusive`] makes the kernel the envelope half
//! outright; it is exact, and it is **one canonical grid step stricter than the
//! shipped miter authority at contact**, because Clipper re-quantizes its offset
//! output and the short-side-first constructor places pieces exactly there.
//! [`KernelMode::Union`] admits what either half admits, which cannot lose a
//! canonical-valid layout and is the mode a promotion would be asked for.

use crate::geometry::general_polygon::{PolygonRing, PolygonSet};

/// The canonical grid step, in millimetres. One micrometre.
pub const GRID_STEP_MM: f64 = 0.001;

/// The largest absolute canonical-grid coordinate, in micrometres, this kernel
/// will accept.
///
/// `2^28` µm is 268.4 metres, about a hundred times the long axis of the
/// largest sheet the engine has ever been run on (2700 mm) and far above any
/// translation a search proposes. Its job is not to be tight, it is to be a
/// *proof*: every intermediate the exact predicates below compute is bounded by
/// a fixed power of two of this constant, and
/// [`tests::the_domain_bound_keeps_every_intermediate_inside_i128`] evaluates
/// those bounds literally rather than by argument.
///
/// A coordinate outside it gets no certificate: [`GridSet::of`] returns `None`
/// and the caller must fall back to the miter authority. Fail-closed, like
/// `CLEARANCE_SLAB_MAX_COORDINATE_MM` in the contract validator's broad phase.
pub const DOMAIN_MAX_MICRON: i64 = 1 << 28;

/// The largest expansion radius, in micrometres, this kernel will accept.
///
/// Same role as [`DOMAIN_MAX_MICRON`]: `2^28` µm bounds `(2r)^2 * |v|^2` in the
/// segment predicate. Production radii are ~2500 µm.
pub const MAX_RADIUS_MICRON: i64 = 1 << 28;

/// One canonicalized ring, in integer micrometres, with its integer box.
#[derive(Clone, Debug)]
pub struct GridRing {
    points: Vec<(i64, i64)>,
    min_x: i64,
    min_y: i64,
    max_x: i64,
    max_y: i64,
}

impl GridRing {
    fn of(ring: &PolygonRing) -> Option<Self> {
        let path = ring.grid_path();
        if path.len() < 3 {
            return None;
        }
        let mut points = Vec::with_capacity(path.len());
        let (mut min_x, mut min_y) = (i64::MAX, i64::MAX);
        let (mut max_x, mut max_y) = (i64::MIN, i64::MIN);
        for point in path {
            // The canonical path is integer-valued by construction:
            // `PolygonRing::new` fills it from `to_grid_mm`, which returns a
            // safe integer or nothing. This re-derives the integer rather than
            // assuming it, and refuses anything that is not one.
            if point.x.fract() != 0.0 || point.y.fract() != 0.0 {
                return None;
            }
            if !(point.x.abs() <= DOMAIN_MAX_MICRON as f64)
                || !(point.y.abs() <= DOMAIN_MAX_MICRON as f64)
            {
                return None;
            }
            let x = point.x as i64;
            let y = point.y as i64;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            points.push((x, y));
        }
        Some(Self {
            points,
            min_x,
            min_y,
            max_x,
            max_y,
        })
    }

    /// This ring's vertices, in integer micrometres, in canonical order.
    pub fn points(&self) -> &[(i64, i64)] {
        &self.points
    }
}

/// One canonicalized material region: an outer ring and the rings it removes.
#[derive(Clone, Debug)]
pub struct GridRegion {
    outer: GridRing,
    holes: Vec<GridRing>,
}

/// One placement's canonicalized material, in integer micrometres.
///
/// This is the *source* material on the canonical grid — the operand
/// `PolygonSet::offset` is given — and never an offset of it. The expansion
/// radius enters as a number in the predicates below, not as geometry.
#[derive(Clone, Debug)]
pub struct GridSet {
    regions: Vec<GridRegion>,
    min_x: i64,
    min_y: i64,
    max_x: i64,
    max_y: i64,
}

impl GridSet {
    /// The canonical integer rings of a transformed [`PolygonSet`].
    ///
    /// `None` when the set is empty, when a ring is degenerate, or when any
    /// coordinate leaves [`DOMAIN_MAX_MICRON`] — the fail-closed cases, where
    /// the caller must use the miter authority instead of this one.
    pub fn of(polygon: &PolygonSet) -> Option<Self> {
        if polygon.regions().is_empty() {
            return None;
        }
        let mut regions = Vec::with_capacity(polygon.regions().len());
        let (mut min_x, mut min_y) = (i64::MAX, i64::MAX);
        let (mut max_x, mut max_y) = (i64::MIN, i64::MIN);
        for region in polygon.regions() {
            let outer = GridRing::of(&region.outer)?;
            min_x = min_x.min(outer.min_x);
            min_y = min_y.min(outer.min_y);
            max_x = max_x.max(outer.max_x);
            max_y = max_y.max(outer.max_y);
            let holes = region
                .holes
                .iter()
                .map(GridRing::of)
                .collect::<Option<Vec<_>>>()?;
            regions.push(GridRegion { outer, holes });
        }
        Some(Self {
            regions,
            min_x,
            min_y,
            max_x,
            max_y,
        })
    }

    /// The material's integer bounding box, `(min x, min y, max x, max y)`, in
    /// micrometres, over outer rings only.
    ///
    /// Outer rings only because that is what the box means: a hole is interior
    /// and cannot extend it. `PolygonSet::fits_rect`, the function this
    /// kernel's boundary half replaces, reads exactly the same rings.
    pub fn bounds_micron(&self) -> (i64, i64, i64, i64) {
        (self.min_x, self.min_y, self.max_x, self.max_y)
    }

    /// Every ring of every region — outer rings and holes both.
    ///
    /// The pair predicate walks all of them, for the same reason
    /// `minimum_boundary_distance` in the contract validator walks
    /// `region_rings`: a piece can sit inside another's hole, and the boundary
    /// that separates them is the hole.
    fn rings(&self) -> impl Iterator<Item = &GridRing> {
        self.regions
            .iter()
            .flat_map(|region| std::iter::once(&region.outer).chain(region.holes.iter()))
    }

    fn total_vertices(&self) -> usize {
        self.rings().map(|ring| ring.points.len()).sum()
    }
}

/// The squared box gap between two integer boxes, or `None` when the boxes
/// overlap on both axes.
///
/// Sound as a *lower bound* on the true minimum distance between anything
/// inside the two boxes, which is all a prune may rely on.
#[inline]
fn box_gap_squared(
    a: (i64, i64, i64, i64),
    b: (i64, i64, i64, i64),
) -> i128 {
    let gap_x = (a.0 - b.2).max(b.0 - a.2).max(0) as i128;
    let gap_y = (a.1 - b.3).max(b.1 - a.3).max(0) as i128;
    gap_x * gap_x + gap_y * gap_y
}

/// Whether the distance from integer point `p` to integer segment `a`-`b` is
/// strictly below `threshold` micrometres. Exact.
///
/// The three branches are the three ways the nearest point of a segment is
/// reached: before `a`, after `b`, or at the interior projection. The interior
/// case is where an `f64` implementation loses the answer — the projected
/// distance is `|cross| / |v|`, a ratio of integers — and it is done here by
/// cross-multiplying into `cross^2 < threshold^2 * |v|^2`, which is an
/// `i128` comparison with no division and no rounding at all.
#[inline]
fn point_segment_closer_than(
    p: (i64, i64),
    a: (i64, i64),
    b: (i64, i64),
    threshold: i64,
) -> bool {
    let threshold_squared = (threshold as i128) * (threshold as i128);
    let vx = (b.0 - a.0) as i128;
    let vy = (b.1 - a.1) as i128;
    let wx = (p.0 - a.0) as i128;
    let wy = (p.1 - a.1) as i128;
    let dot = wx * vx + wy * vy;
    if dot <= 0 {
        return wx * wx + wy * wy < threshold_squared;
    }
    let length_squared = vx * vx + vy * vy;
    if dot >= length_squared {
        let ux = (p.0 - b.0) as i128;
        let uy = (p.1 - b.1) as i128;
        return ux * ux + uy * uy < threshold_squared;
    }
    let cross = vx * wy - vy * wx;
    cross * cross < threshold_squared * length_squared
}

/// Whether two integer segments come closer than `threshold` micrometres.
/// Exact.
///
/// For **disjoint** segments the minimum is attained at an endpoint of one
/// against the other, which is the four calls below. Crossing segments are at
/// distance zero and the four endpoint tests do **not** see them — two long
/// segments meeting in an X have four large endpoint distances — so the caller
/// runs [`segments_intersect`] first whenever the two segments' integer boxes
/// touch, which is the only way they can cross. `box_touches` carries that
/// decision in rather than recomputing it.
#[inline]
fn segments_closer_than(
    a0: (i64, i64),
    a1: (i64, i64),
    b0: (i64, i64),
    b1: (i64, i64),
    threshold: i64,
    box_touches: bool,
) -> bool {
    if box_touches && segments_intersect(a0, a1, b0, b1) {
        return threshold > 0;
    }
    point_segment_closer_than(a0, b0, b1, threshold)
        || point_segment_closer_than(a1, b0, b1, threshold)
        || point_segment_closer_than(b0, a0, a1, threshold)
        || point_segment_closer_than(b1, a0, a1, threshold)
}

/// The sign of the cross product `(b - a) x (c - a)`, exactly.
#[inline]
fn orientation_sign(a: (i64, i64), b: (i64, i64), c: (i64, i64)) -> i32 {
    let value = ((b.0 - a.0) as i128) * ((c.1 - a.1) as i128)
        - ((b.1 - a.1) as i128) * ((c.0 - a.0) as i128);
    match value.cmp(&0) {
        std::cmp::Ordering::Greater => 1,
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
    }
}

#[inline]
fn on_segment(a: (i64, i64), b: (i64, i64), p: (i64, i64)) -> bool {
    orientation_sign(a, b, p) == 0
        && p.0 >= a.0.min(b.0)
        && p.0 <= a.0.max(b.0)
        && p.1 >= a.1.min(b.1)
        && p.1 <= a.1.max(b.1)
}

/// Whether two integer segments share at least one point. Exact, including the
/// collinear and touching cases.
#[inline]
fn segments_intersect(a0: (i64, i64), a1: (i64, i64), b0: (i64, i64), b1: (i64, i64)) -> bool {
    let d1 = orientation_sign(a0, a1, b0);
    let d2 = orientation_sign(a0, a1, b1);
    let d3 = orientation_sign(b0, b1, a0);
    let d4 = orientation_sign(b0, b1, a1);
    if d1 != d2 && d3 != d4 {
        return true;
    }
    on_segment(a0, a1, b0)
        || on_segment(a0, a1, b1)
        || on_segment(b0, b1, a0)
        || on_segment(b0, b1, a1)
}

/// Whether integer point `p` is strictly inside integer ring `ring`. Exact.
///
/// Crossing number with the horizontal ray to `+x`. Every comparison is an
/// `i128` cross-multiplication, so a vertex exactly on the ray is decided the
/// same way twice and the parity is right. The callers below only ever ask this
/// for points known to be off the ring's boundary, which is the case the
/// algorithm is exact for.
fn point_strictly_inside(ring: &GridRing, p: (i64, i64)) -> bool {
    if p.0 < ring.min_x || p.0 > ring.max_x || p.1 < ring.min_y || p.1 > ring.max_y {
        return false;
    }
    let mut inside = false;
    let count = ring.points.len();
    for index in 0..count {
        let a = ring.points[index];
        let b = ring.points[(index + 1) % count];
        if (a.1 > p.1) != (b.1 > p.1) {
            let delta_y = (b.1 - a.1) as i128;
            let left = ((p.0 - a.0) as i128) * delta_y;
            let right = ((b.0 - a.0) as i128) * ((p.1 - a.1) as i128);
            let crosses = if delta_y > 0 { left < right } else { left > right };
            if crosses {
                inside = !inside;
            }
        }
    }
    inside
}

/// Whether integer point `p` is strictly inside the material of `set`.
fn point_in_material(set: &GridSet, p: (i64, i64)) -> bool {
    set.regions.iter().any(|region| {
        point_strictly_inside(&region.outer, p)
            && !region
                .holes
                .iter()
                .any(|hole| point_strictly_inside(hole, p))
    })
}

/// Whether either set's material contains the other's, given that their
/// boundaries are known to be disjoint.
///
/// With disjoint boundaries every region is wholly inside or wholly outside the
/// other set's material, so one vertex per outer ring decides it.
fn either_contains_the_other(a: &GridSet, b: &GridSet) -> bool {
    a.regions
        .iter()
        .any(|region| point_in_material(b, region.outer.points[0]))
        || b.regions
            .iter()
            .any(|region| point_in_material(a, region.outer.points[0]))
}

/// Whether any boundary segment of `a` comes closer than `threshold` to any of
/// `b`, with the box prunes in front of it.
///
/// `steps` accumulates the number of segment-pair *narrow* evaluations, which
/// is the quantity the economy section of the evidence reports. It is a
/// counter, never a decision.
fn any_boundary_closer_than(
    a: &GridSet,
    b: &GridSet,
    threshold: i64,
    steps: &mut u64,
) -> bool {
    let threshold_squared = (threshold as i128) * (threshold as i128);
    for ring_a in a.rings() {
        let box_a = (ring_a.min_x, ring_a.min_y, ring_a.max_x, ring_a.max_y);
        for ring_b in b.rings() {
            let box_b = (ring_b.min_x, ring_b.min_y, ring_b.max_x, ring_b.max_y);
            if box_gap_squared(box_a, box_b) >= threshold_squared {
                continue;
            }
            let count_a = ring_a.points.len();
            let count_b = ring_b.points.len();
            for index_a in 0..count_a {
                let a0 = ring_a.points[index_a];
                let a1 = ring_a.points[(index_a + 1) % count_a];
                let seg_box_a = (
                    a0.0.min(a1.0),
                    a0.1.min(a1.1),
                    a0.0.max(a1.0),
                    a0.1.max(a1.1),
                );
                if box_gap_squared(seg_box_a, box_b) >= threshold_squared {
                    continue;
                }
                for index_b in 0..count_b {
                    let b0 = ring_b.points[index_b];
                    let b1 = ring_b.points[(index_b + 1) % count_b];
                    let seg_box_b = (
                        b0.0.min(b1.0),
                        b0.1.min(b1.1),
                        b0.0.max(b1.0),
                        b0.1.max(b1.1),
                    );
                    let gap = box_gap_squared(seg_box_a, seg_box_b);
                    if gap >= threshold_squared {
                        continue;
                    }
                    *steps += 1;
                    if segments_closer_than(a0, a1, b0, b1, threshold, gap == 0) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// How much work one pair verdict cost, for the economy evidence.
#[derive(Clone, Copy, Debug, Default)]
pub struct PairWork {
    /// `true` when the piece-level box gap alone decided the pair.
    pub certified_by_box: bool,
    /// Segment pairs that reached the exact narrow predicate.
    pub narrow_segment_pairs: u64,
}

/// Whether `P_a (+) disc(r)` and `P_b (+) disc(r)` have disjoint interiors,
/// where `two_r` is `2r` in integer micrometres. **Exact.**
///
/// The verdict is the conjunction of the two things the composite's envelope
/// half means by "these two placements do not collide":
///
/// 1. the material sets do not overlap — including the containment case, which
///    no boundary distance can see; and
/// 2. their minimum boundary distance is at least `2r`.
///
/// `two_r` rather than `r` because `2r` is what the comparison needs and
/// halving an odd integer radius is the one place this predicate could have
/// acquired a rounding.
///
/// # Domain: `two_r >= 1`
///
/// The kernel does not certify at **zero** expansion, and that is a deliberate
/// fail-closed restriction rather than an oversight. At `2r = 0` the question
/// stops being "are these at least `2r` apart" and becomes "do these two
/// polygons overlap with positive area", which boundary distance cannot answer:
/// two unit squares sharing an edge are legal, two overlapping squares whose
/// every vertex lies on the other's boundary are not, and both have minimum
/// boundary distance zero and no strictly-contained vertex. Deciding that needs
/// an area authority, which is what `polygons_overlap_exact` is for and what
/// this module deliberately is not.
///
/// It never binds in production: `collision_expansion_mm` is `total_padding/2 +
/// clearance_safety_margin + search_offset_allowance`, which is 2.500-2.502 mm
/// on every configuration this campaign has run, so `2r` is 5000-5004 µm. The
/// wire point in `general_fast` checks [`certifies`] and falls back to the
/// miter authority when it is false, so a zero-padding request gets HEAD's
/// answer rather than this one.
///
/// [`certifies`] is the predicate; a `debug_assert` here catches a caller that
/// ignored it.
pub fn pair_admissible(a: &GridSet, b: &GridSet, two_r: i64) -> bool {
    pair_admissible_measured(a, b, two_r).0
}

/// Whether the kernel will certify at this doubled radius at all. See
/// [`pair_admissible`]'s domain note.
pub fn certifies(two_r: i64) -> bool {
    two_r >= 1 && two_r <= 2 * MAX_RADIUS_MICRON
}

/// [`pair_admissible`] with the work it cost.
pub fn pair_admissible_measured(a: &GridSet, b: &GridSet, two_r: i64) -> (bool, PairWork) {
    let mut work = PairWork::default();
    debug_assert!(
        certifies(two_r),
        "the round-envelope kernel does not certify at 2r = {two_r}"
    );
    let two_r = two_r.max(1);
    let threshold_squared = (two_r as i128) * (two_r as i128);
    // The broad phase, in integers: a box gap of at least `2r` proves both
    // clauses at once - the sets cannot overlap and cannot be closer than the
    // gap. This is `ClearanceSlabs::provably_clear` with the axis-aligned
    // directions and without the floating-point margin, which integers do not
    // need.
    if box_gap_squared(
        (a.min_x, a.min_y, a.max_x, a.max_y),
        (b.min_x, b.min_y, b.max_x, b.max_y),
    ) >= threshold_squared
    {
        work.certified_by_box = true;
        return (true, work);
    }
    if any_boundary_closer_than(a, b, two_r, &mut work.narrow_segment_pairs) {
        return (false, work);
    }
    // Boundaries are now known to be at least `2r >= 1` apart, so they are
    // disjoint and the only remaining way to overlap is containment: one set
    // entirely inside the other, at a distance. That is the clause a pure
    // minimum-boundary-distance kernel gets wrong, in the false-accept
    // direction.
    (!either_contains_the_other(a, b), work)
}

/// Whether `P (+) disc(r)` fits the inset sheet rectangle, in integer
/// micrometres. **Exact.**
///
/// The rectangle is axis-aligned and a disc reaches exactly `r` past the
/// material in every direction, so this is the material's own integer box grown
/// by `r`. `PolygonSet::fits_rect`, which the miter authority calls with the
/// offset polygon, refuses an empty set; so does [`GridSet::of`], one level up.
pub fn boundary_admissible(
    set: &GridSet,
    radius: i64,
    low_x: i64,
    low_y: i64,
    high_x: i64,
    high_y: i64,
) -> bool {
    if low_x > high_x || low_y > high_y {
        return false;
    }
    set.min_x - radius >= low_x
        && set.min_y - radius >= low_y
        && set.max_x + radius <= high_x
        && set.max_y + radius <= high_y
}

/// The largest integer `2r` in micrometres at which [`pair_admissible`] still
/// holds, searched in `[0, ceiling]`, or `None` when it fails at zero.
///
/// This is the exact analogue of Gate A's bisected `2 * r*` — with the
/// difference that Gate A bisected a *Clipper offset*, so its answer carried
/// the offset's own output quantization, and this one bisects a predicate that
/// is exact at every radius. `Some((ceiling, true))` means the search
/// saturated and the number is a floor rather than the answer, the same
/// labelling `import_gate::largest_micron_radius` uses.
///
/// Diagnostic. No acceptance path calls it.
pub fn critical_two_r_micron(a: &GridSet, b: &GridSet, ceiling: i64) -> Option<(i64, bool)> {
    if !pair_admissible(a, b, 1) {
        return None;
    }
    let (mut low, mut high) = (1i64, ceiling.max(2));
    if pair_admissible(a, b, high) {
        return Some((high, true));
    }
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        if pair_admissible(a, b, middle) {
            low = middle;
        } else {
            high = middle;
        }
    }
    Some((low, false))
}

/// The largest integer radius in micrometres at which [`boundary_admissible`]
/// still holds. Closed form, not a bisection: the binding side is whichever of
/// the four is tightest.
///
/// `None` when the material is already outside the inset rectangle.
pub fn critical_boundary_radius_micron(
    set: &GridSet,
    low_x: i64,
    low_y: i64,
    high_x: i64,
    high_y: i64,
) -> Option<i64> {
    let slack = (set.min_x - low_x)
        .min(set.min_y - low_y)
        .min(high_x - set.max_x)
        .min(high_y - set.max_y);
    if slack < 0 {
        return None;
    }
    Some(slack)
}

/// The number of ring vertices this kernel would walk for one placement.
pub fn vertex_count(set: &GridSet) -> usize {
    set.total_vertices()
}

/// How the round-envelope kernel participates in the composite's envelope half.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KernelMode {
    /// Not consulted. The composite is HEAD's miter authority exactly.
    Off,
    /// **The hybrid.** The envelope half admits a layout when the kernel admits
    /// it *or* the miter authority does.
    ///
    /// This is Sol review 12 §3.2's "serve un ibrido" and Sol review 8's
    /// internal-filter architecture, and it is the mode a promotion would be
    /// asked for. Two properties follow from the disjunction and neither needs
    /// a measurement to establish:
    ///
    /// * **no canonical-valid layout is lost.** Whatever the miter admits, the
    ///   union admits. The battery measured why that matters: the shipped miter
    ///   authority re-quantizes its offset output to the 1 µm grid, so at
    ///   *contact* it admits pairs whose canonical separation is `2r - 1` µm -
    ///   one grid step permissive of its own declared envelope - and the
    ///   short-side-first constructor places pieces exactly at contact. In
    ///   [`KernelMode::Exclusive`] the constructor's own `validate_result` then
    ///   refuses the layout the constructor just built.
    /// * **no new false-accept surface beyond HEAD's own.** The union is
    ///   bounded by the two halves it is made of, and the material contract
    ///   validator is untouched and still final.
    ///
    /// The kernel is asked *first*, because it is the cheap one, so the miter
    /// runs only on the rows the kernel refuses.
    Union,
    /// The kernel alone is the envelope half. The certified-exact arm, and a
    /// measurement mode: it is one grid step stricter than HEAD at contact and
    /// is not backward-compatible with layouts placed there.
    Exclusive,
}

impl KernelMode {
    fn code(self) -> u8 {
        match self {
            KernelMode::Off => 0,
            KernelMode::Union => 1,
            KernelMode::Exclusive => 2,
        }
    }

    fn from_code(code: u8) -> Self {
        match code {
            1 => KernelMode::Union,
            2 => KernelMode::Exclusive,
            _ => KernelMode::Off,
        }
    }

    /// `"0"`, `"1"`/`"union"`, `"2"`/`"exclusive"`. `None` for anything else, so
    /// a driver that mistypes the mode gets a refusal rather than a different
    /// arm under its label.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "0" | "off" => Some(KernelMode::Off),
            "1" | "union" => Some(KernelMode::Union),
            "2" | "exclusive" => Some(KernelMode::Exclusive),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            KernelMode::Off => "off",
            KernelMode::Union => "union",
            KernelMode::Exclusive => "exclusive",
        }
    }
}

/// The process-wide arming of the round-envelope kernel.
///
/// [`KernelMode::Off`], and this is the difference from
/// `fast-contract-validator`'s `CERTIFICATE_ARMED`, which defaults to armed.
/// That one is verdict-preserving - the certificate only ever skips work the
/// exact loop would have agreed with - so arming it changes nothing a document
/// can see. This one **changes the acceptance authority**: a round envelope
/// accepts layouts the miter envelope refuses, so an armed run is a different
/// engine and must be asked for.
///
/// A process-wide atomic rather than a parameter on the composite because the
/// composite's callers are the whole acceptance path - `general_fast`,
/// `general_relaxed`, `general_persistent_vacancy`,
/// `general_micro_legalization` - and threading an engine preference through
/// `GeneralFastSettings` would put it inside the type that means "what the
/// request asked for". The coordinator sets it once, before any search thread
/// exists, and puts it back on the way out; see `RoundEnvelopeArming` in
/// `search::portfolio`.
static KERNEL_MODE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// How the round-envelope kernel is armed in this process.
pub fn kernel_mode() -> KernelMode {
    KernelMode::from_code(KERNEL_MODE.load(std::sync::atomic::Ordering::Relaxed))
}

/// Arms or disarms the round-envelope kernel process-wide, returning what it
/// was.
pub fn set_kernel_mode(mode: KernelMode) -> KernelMode {
    KernelMode::from_code(KERNEL_MODE.swap(mode.code(), std::sync::atomic::Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::IrregularPoint;
    use crate::geometry::general_polygon::PolygonRegion;

    fn point(x: f64, y: f64) -> IrregularPoint {
        IrregularPoint::new(x, y)
    }

    fn rectangle(x: f64, y: f64, width: f64, height: f64) -> PolygonSet {
        PolygonSet::from_outer(vec![
            point(x, y),
            point(x + width, y),
            point(x + width, y + height),
            point(x, y + height),
        ])
        .expect("a rectangle is a valid ring")
    }

    fn grid(set: &PolygonSet) -> GridSet {
        GridSet::of(set).expect("the fixture is inside the kernel's domain")
    }

    /// The `i128` bound, evaluated rather than argued.
    ///
    /// Every product the exact predicates form is one of five shapes, and each
    /// is bounded by a fixed power of two of [`DOMAIN_MAX_MICRON`]. This
    /// computes each bound literally, at the domain's own extreme, and requires
    /// it to fit `i128` with room to spare. It also states the actual sheet the
    /// campaign runs on, 2000 x 2700 mm, so the margin between "what the engine
    /// does" and "what the kernel proves" is visible.
    #[test]
    fn the_domain_bound_keeps_every_intermediate_inside_i128() {
        let domain = DOMAIN_MAX_MICRON as i128;
        // A coordinate difference between two points of the domain.
        let delta = 2 * domain;
        // `wx*wx + wy*wy`, the endpoint branch.
        let squared_length = 2 * delta * delta;
        // `cross = vx*wy - vy*wx`, the interior branch's numerator.
        let cross = 2 * delta * delta;
        // `cross * cross`, the interior branch's left-hand side. This is the
        // largest quantity the kernel ever forms.
        let cross_squared = cross * cross;
        // `threshold^2 * |v|^2`, the interior branch's right-hand side. The
        // threshold is the doubled radius, and `certifies()` admits it up to
        // `2 * MAX_RADIUS_MICRON` — the bound must be evaluated there, not at
        // the bare radius (sol-review-13 caught the earlier mismatch).
        let two_r = 2 * (MAX_RADIUS_MICRON as i128);
        let right_hand_side = two_r * two_r * squared_length;
        // `box_gap_squared`, and the point-in-ring cross-multiplications.
        let box_gap = 2 * delta * delta;
        let in_ring = delta * delta;

        for (name, value) in [
            ("|w|^2", squared_length),
            ("cross", cross),
            ("cross^2", cross_squared),
            ("(2r)^2 * |v|^2", right_hand_side),
            ("box gap^2", box_gap),
            ("point-in-ring product", in_ring),
        ] {
            assert!(
                value > 0 && value <= i128::MAX / 16,
                "{name} = {value} leaves less than four bits of i128 headroom"
            );
        }

        // The sheet the campaign actually runs on, with a generous allowance
        // for negative translations: the kernel's domain is 99x its long axis.
        let sheet_long_axis_micron: i64 = 2_700_000;
        assert!(
            DOMAIN_MAX_MICRON / sheet_long_axis_micron >= 99,
            "the domain guard is not comfortably above the campaign's sheet"
        );
        // And the production expansion radius, ~2502 µm, against the radius
        // guard.
        assert!(MAX_RADIUS_MICRON / 2_502 >= 100_000);
    }

    #[test]
    fn a_coordinate_outside_the_domain_gets_no_certificate() {
        let far = (DOMAIN_MAX_MICRON as f64) * GRID_STEP_MM + 1.0;
        let outside = rectangle(far, 0.0, 10.0, 10.0);
        assert!(
            GridSet::of(&outside).is_none(),
            "a set outside the domain must fail closed"
        );
    }

    /// The predicate is `>= 2r`, not `> 2r`: the composite's envelope half asks
    /// `polygons_overlap_exact`, which is a positive-area test, so two
    /// envelopes that touch on their boundary do not overlap.
    #[test]
    fn envelopes_that_exactly_touch_are_admissible_and_one_micron_closer_is_not() {
        let left = grid(&rectangle(0.0, 0.0, 10.0, 10.0));
        let right = grid(&rectangle(15.0, 0.0, 10.0, 10.0));
        // The gap is 5.000 mm = 5000 µm.
        assert!(pair_admissible(&left, &right, 5_000));
        assert!(!pair_admissible(&left, &right, 5_001));
        assert_eq!(
            critical_two_r_micron(&left, &right, 40_000),
            Some((5_000, false))
        );
    }

    /// The diagonal case, where an axis-aligned reading of the gap would be
    /// wrong in the *permissive* direction and a false accept would follow.
    #[test]
    fn a_diagonal_gap_is_measured_as_a_distance_and_not_per_axis() {
        let lower = grid(&rectangle(0.0, 0.0, 10.0, 10.0));
        let upper = grid(&rectangle(13.0, 14.0, 10.0, 10.0));
        // Corner to corner: (3, 4) mm, exactly 5 mm apart.
        assert!(pair_admissible(&lower, &upper, 5_000));
        assert!(!pair_admissible(&lower, &upper, 5_001));
    }

    /// The interior-projection branch: the nearest point is in the middle of an
    /// edge, so the answer is a ratio of integers and an `f64` implementation
    /// would have to round it.
    #[test]
    fn the_interior_projection_branch_is_exact_at_the_micron() {
        // A tilted segment from (0,0) to (4000, 3000) µm has length 5000 µm.
        // The point (3000, -4000) µm projects to the origin end; the point
        // (-3000, 4000) µm is at perpendicular distance 5000 µm.
        assert!(!point_segment_closer_than(
            (-3_000, 4_000),
            (0, 0),
            (4_000, 3_000),
            5_000
        ));
        assert!(point_segment_closer_than(
            (-3_000, 4_000),
            (0, 0),
            (4_000, 3_000),
            5_001
        ));
        // One micrometre nearer along the same perpendicular.
        assert!(point_segment_closer_than(
            (-2_999, 3_999),
            (0, 0),
            (4_000, 3_000),
            5_000
        ));
    }

    /// A containment is an overlap however far the boundaries are apart. This
    /// is the clause a pure minimum-boundary-distance kernel would get wrong,
    /// in the false-accept direction.
    #[test]
    fn a_contained_piece_is_refused_at_every_radius() {
        let outer = grid(&rectangle(0.0, 0.0, 100.0, 100.0));
        let inner = grid(&rectangle(40.0, 40.0, 5.0, 5.0));
        // 35 mm of boundary distance on every side, and still an overlap.
        assert!(!pair_admissible(&outer, &inner, 5_000));
        assert!(!pair_admissible(&outer, &inner, 1));
        assert_eq!(critical_two_r_micron(&outer, &inner, 40_000), None);
    }

    /// A piece inside another's *hole* is legal when the hole is wide enough:
    /// the hole ring is the boundary that separates them, which is why the pair
    /// predicate walks holes and not only outer rings.
    #[test]
    fn a_piece_inside_a_wide_hole_is_admissible_and_a_narrow_hole_is_not() {
        let frame = PolygonSet::new(vec![PolygonRegion::new(
            vec![
                point(0.0, 0.0),
                point(100.0, 0.0),
                point(100.0, 100.0),
                point(0.0, 100.0),
            ],
            vec![vec![
                point(20.0, 20.0),
                point(80.0, 20.0),
                point(80.0, 80.0),
                point(20.0, 80.0),
            ]],
        )
        .expect("a framed region is valid")])
        .expect("a framed set is valid");
        let frame = grid(&frame);
        let small = grid(&rectangle(45.0, 45.0, 10.0, 10.0));
        // 25 mm from the hole edge on every side.
        assert!(pair_admissible(&frame, &small, 25_000));
        assert!(!pair_admissible(&frame, &small, 25_001));
        let wide = grid(&rectangle(25.0, 25.0, 50.0, 50.0));
        assert!(pair_admissible(&frame, &wide, 5_000));
        assert!(!pair_admissible(&frame, &wide, 5_001));
    }

    /// Two rings that cross without either owning a vertex of the other — the
    /// plus sign — overlap, and no endpoint-to-segment distance sees it. The
    /// transversal test in [`segments_intersect`] is what catches it, and this
    /// is the false accept that would follow from leaving it out.
    #[test]
    fn crossing_rings_are_refused_at_the_smallest_certified_radius() {
        let horizontal = grid(&rectangle(0.0, 40.0, 100.0, 20.0));
        let vertical = grid(&rectangle(40.0, 0.0, 20.0, 100.0));
        assert!(!pair_admissible(&horizontal, &vertical, 1));
        assert!(!pair_admissible(&horizontal, &vertical, 5_004));
        assert_eq!(critical_two_r_micron(&horizontal, &vertical, 40_000), None);
    }

    /// Material that touches is refused from one micrometre up, which is where
    /// the kernel's domain starts.
    #[test]
    fn touching_material_is_refused_from_one_micron_up() {
        let left = grid(&rectangle(0.0, 0.0, 10.0, 10.0));
        let right = grid(&rectangle(10.0, 0.0, 10.0, 10.0));
        assert!(!pair_admissible(&left, &right, 1));
    }

    /// The domain restriction, stated as a test rather than only as prose: zero
    /// expansion is not a question this kernel answers, and
    /// [`certifies`] is the guard the wire point reads before it uses one of
    /// these verdicts.
    #[test]
    fn the_kernel_does_not_certify_at_zero_expansion() {
        assert!(!certifies(0));
        assert!(!certifies(-1));
        assert!(certifies(1));
        // The production doubled radii: 2.500, 2.5005 and 2.502 mm of
        // expansion, doubled and on the grid.
        assert!(certifies(5_000));
        assert!(certifies(5_002));
        assert!(certifies(5_004));
        assert!(!certifies(2 * MAX_RADIUS_MICRON + 1));
    }

    /// The boundary half is the material box grown by the radius, and the flip
    /// is at one micrometre.
    #[test]
    fn the_boundary_half_flips_at_one_micron() {
        let set = grid(&rectangle(5.0, 5.0, 100.0, 100.0));
        // Inset 2.5 mm, sheet 2000 x 2700 mm: the rectangle is
        // [2500, 1997500] x [2500, 2697500] µm.
        assert!(boundary_admissible(&set, 2_500, 2_500, 2_500, 1_997_500, 2_697_500));
        assert!(!boundary_admissible(&set, 2_501, 2_500, 2_500, 1_997_500, 2_697_500));
        assert_eq!(
            critical_boundary_radius_micron(&set, 2_500, 2_500, 1_997_500, 2_697_500),
            Some(2_500)
        );
    }

    /// The broad phase is a *proof*, never an estimate: whenever it certifies a
    /// pair, the full narrow scan must agree. Checked over a lattice of offsets
    /// that straddles the threshold from every direction.
    #[test]
    fn the_box_certificate_never_disagrees_with_the_narrow_scan() {
        let base = rectangle(0.0, 0.0, 12.0, 7.0);
        let anchor = grid(&base);
        let mut certified = 0u32;
        let mut checked = 0u32;
        for dx in -30i64..=30 {
            for dy in -30i64..=30 {
                let moved = base
                    .transformed(0.0, false, dx as f64 * 0.7, dy as f64 * 0.7)
                    .expect("translation stays in the domain");
                let moved = grid(&moved);
                for two_r in [1i64, 999, 1_000, 5_000, 12_345] {
                    let (verdict, work) = pair_admissible_measured(&anchor, &moved, two_r);
                    checked += 1;
                    if !work.certified_by_box {
                        continue;
                    }
                    certified += 1;
                    // Re-decide without the certificate: run the narrow scan
                    // and the containment clause directly.
                    let mut steps = 0;
                    let narrow = !any_boundary_closer_than(&anchor, &moved, two_r, &mut steps)
                        && !either_contains_the_other(&anchor, &moved);
                    assert!(
                        verdict && narrow,
                        "the box certificate and the narrow scan disagree at \
                         ({dx}, {dy}) and 2r = {two_r}"
                    );
                }
            }
        }
        // The counts are asserted so that a future edit which silently stops
        // certifying - or stops sweeping - fails here rather than passing
        // vacuously.
        assert_eq!(checked, 61 * 61 * 5);
        assert!(
            certified * 4 > checked,
            "the box certificate answered only {certified} of {checked} probes; \
             a broad phase that never fires is not being tested"
        );
    }

    /// The predicate is monotone in the radius: once refused it stays refused.
    /// Everything that bisects it - `critical_two_r_micron` here, and the
    /// battery's sweeps - depends on that and nothing proves it but a sweep.
    #[test]
    fn the_pair_predicate_is_monotone_in_the_radius() {
        let base = rectangle(0.0, 0.0, 12.0, 7.0);
        let anchor = grid(&base);
        for shift in [3.0, 8.5, 17.25, 40.0] {
            let moved = grid(
                &base
                    .transformed(37.0, false, shift, shift / 2.0)
                    .expect("a rotated translate stays in the domain"),
            );
            let mut previous = true;
            for two_r in 1i64..=30_000 {
                let now = pair_admissible(&anchor, &moved, two_r);
                assert!(
                    previous || !now,
                    "the predicate went false then true again at 2r = {two_r}"
                );
                previous = now;
            }
            // And the bisection agrees with the sweep's own flip point.
            let flip = (1i64..=30_000)
                .take_while(|two_r| pair_admissible(&anchor, &moved, *two_r))
                .last();
            assert_eq!(
                critical_two_r_micron(&anchor, &moved, 30_000).map(|(value, _)| value),
                flip
            );
        }
    }

    #[test]
    fn the_kernel_is_disarmed_by_default_and_the_mode_round_trips() {
        assert_eq!(kernel_mode(), KernelMode::Off, "the kernel defaults to off");
        let previous = set_kernel_mode(KernelMode::Union);
        assert_eq!(previous, KernelMode::Off);
        assert_eq!(kernel_mode(), KernelMode::Union);
        assert_eq!(set_kernel_mode(KernelMode::Exclusive), KernelMode::Union);
        assert_eq!(kernel_mode(), KernelMode::Exclusive);
        set_kernel_mode(previous);
        assert_eq!(kernel_mode(), KernelMode::Off);
        assert_eq!(KernelMode::parse("0"), Some(KernelMode::Off));
        assert_eq!(KernelMode::parse("1"), Some(KernelMode::Union));
        assert_eq!(KernelMode::parse("2"), Some(KernelMode::Exclusive));
        assert_eq!(KernelMode::parse("union"), Some(KernelMode::Union));
        assert_eq!(KernelMode::parse("exclusive"), Some(KernelMode::Exclusive));
        assert_eq!(KernelMode::parse("yes"), None);
        assert_eq!(KernelMode::parse(""), None);
    }
}
