//! The constructor's proxy-first separation prefilter.
//!
//! The skyline constructor confirms a candidate pose by building its exact
//! collision polygon and asking Clipper, for every active piece of the parent,
//! whether the two overlap with positive area. After the bit-grid redesign that
//! confirmation *is* mode 20: `exactOverlapTest` 33.1% of leaf plus
//! `collisionPolygonBuild` 20.1%.
//!
//! This module removes exact pair queries whose answer is already known — and it
//! removes only those. Nothing here estimates, approximates, or tolerates
//! anything.
//!
//! # What the census said, and what it permits
//!
//! The counting build in [`crate::constructor_census`] measured the mode-20
//! gate-1 stream: of the 1,290,352 pair questions that reach Clipper, 997,826
//! (77.3%) are genuine overlaps. Only the remaining 292,526 are queries a
//! prefilter could ever remove, and that is this stage's ceiling — a fact worth
//! stating before the mechanism, because it is the reason the mechanism is a
//! *proof* rather than a heuristic. A test that skipped a query it could not
//! decide would be trading a 22.7% saving against a wrong constructor.
//!
//! # The two tiers, and why each one is sound
//!
//! Both tiers run on the **integer Clipper path** — the same `Path64` the exact
//! intersection is executed on, not a re-derivation of it and not a surrogate
//! built at a different pose. There is therefore no representation gap to
//! bound: the only question is whether the arithmetic is exact, and it is,
//! because grid coordinates are integer-valued `f64` and every quantity below
//! stays inside the exactly representable range under an explicit guard.
//!
//! 1. **Slabs.** [`GridSlabs`](crate::geometry::general_polygon::GridSlabs) —
//!    the axis-aligned box plus the two diagonals. A non-overlapping slab is a
//!    separating line. Four comparisons.
//! 2. **Hull.** A separating-axis test over both convex hulls' edge normals. A
//!    hull contains its polygon, so a separation of the hulls is a separation of
//!    the polygons.
//!
//! Both answer "provably separated" or "no information". A separated pair has
//! zero intersection area, so `intersection_area_mm2(..) > 0.0` is `false`, so
//! the verdict this prefilter substitutes is the verdict the exact query would
//! have returned. That is the whole soundness argument, and it is why the
//! constructor's *decisions* are untouched by this flag: the poses it accepts
//! are exactly the poses it accepted before, confirmed by exactly the same
//! Clipper queries — there are simply fewer queries whose answer was never in
//! doubt.
//!
//! A `debug_assert` in the caller closes the loop empirically: in a debug build
//! every pair this module skips is still handed to Clipper, and the exact answer
//! must be "no overlap".
//!
//! # What it does not do
//!
//! It cannot prove an overlap, so it can never reject a pose. Every accepted
//! pose is still exactly confirmed against every active piece by the exact
//! tier. And it is not a *proxy* in the [`ExplorationKernel`] sense: the
//! constructor has no surrogate catalogue, and building one per pose would cost
//! more than the query it replaced. The census measured the alternative — the
//! surrogate tier's own broad phase is an axis-aligned box, which is tier one
//! here.
//!
//! [`ExplorationKernel`]: crate::search::kernel::ExplorationKernel

#[cfg(feature = "fast-constructor-confirm")]
mod armed {
    use std::sync::Arc;

    use crate::geometry::general_polygon::{GridSlabs, PolygonSet};

    /// The largest grid coordinate magnitude for which the hull test's
    /// arithmetic is exact.
    ///
    /// The hull test forms `nx * x + ny * y`, where `nx`/`ny` are differences of
    /// two coordinates. With every coordinate bounded by `C` the widest product
    /// is `2C * C` and the sum of two of them is `4C^2`, so exactness needs
    /// `4C^2 <= 2^53`, i.e. `C <= 2^25.5`; `2^25` is the power of two below it.
    /// That is 33.5 metres on the 0.001 mm contractual grid. A request whose
    /// coordinates exceed it keeps tier one, whose projections are the stored
    /// coordinates themselves and are exact for anything `to_grid_mm` admits.
    const HULL_EXACT_LIMIT: f64 = 33_554_432.0; // 2^25

    /// One polygon's separation certificate material.
    #[derive(Clone, Debug, Default)]
    pub(crate) struct PairShield {
        slabs: Option<GridSlabs>,
        /// Counter-clockwise convex hull in grid units; empty when the polygon
        /// is degenerate or its coordinates exceed [`HULL_EXACT_LIMIT`], in
        /// which case tier two is skipped rather than approximated.
        hull: Vec<(f64, f64)>,
    }

    impl PairShield {
        /// Refills this certificate from `polygon`, reusing both its own hull
        /// buffer and the caller's point buffer.
        ///
        /// The constructor derives one of these per confirmation row — 747,521
        /// of them on the gate-1 stream — so the two allocations a freshly
        /// built certificate would need are the difference between a prefilter
        /// that pays for itself and one that does not.
        fn rebuild(&mut self, polygon: &PolygonSet, points: &mut Vec<(f64, f64)>) {
            self.hull.clear();
            self.slabs = polygon.grid_slabs();
            if self.slabs.is_none() {
                return;
            }
            polygon.grid_points_into(points);
            if points
                .iter()
                .all(|(x, y)| x.abs() <= HULL_EXACT_LIMIT && y.abs() <= HULL_EXACT_LIMIT)
            {
                convex_hull_into(points, &mut self.hull);
            }
        }

        /// Whether the two polygons **provably** have zero intersection area.
        ///
        /// `true` is a proof; `false` carries no information. See the module
        /// documentation for why each tier is a proof.
        pub(crate) fn separated_from(&self, other: &Self) -> bool {
            if self.slabs_separated_from(other) {
                return true;
            }
            if self.hull.len() < 3 || other.hull.len() < 3 {
                return false;
            }
            axis_separates(&self.hull, &other.hull) || axis_separates(&other.hull, &self.hull)
        }

        /// A standalone certificate, for the tests that check the property the
        /// engine relies on. The engine itself never builds one this way.
        #[cfg(test)]
        pub(super) fn for_test(polygon: &PolygonSet) -> Self {
            let mut shield = Self::default();
            shield.rebuild(polygon, &mut Vec::new());
            shield
        }

        /// Tier one alone, so a test can show tier two earning its place.
        #[inline(always)]
        pub(crate) fn slabs_separated_from(&self, other: &Self) -> bool {
            match (self.slabs.as_ref(), other.slabs.as_ref()) {
                (Some(first), Some(second)) => first.separated(second),
                _ => false,
            }
        }
    }

    /// Whether some outward edge normal of `hull` separates it from `other`.
    fn axis_separates(hull: &[(f64, f64)], other: &[(f64, f64)]) -> bool {
        for index in 0..hull.len() {
            let (x0, y0) = hull[index];
            let (x1, y1) = hull[(index + 1) % hull.len()];
            let (normal_x, normal_y) = (y1 - y0, x0 - x1);
            let mut own = f64::NEG_INFINITY;
            for (x, y) in hull {
                own = own.max(normal_x * x + normal_y * y);
            }
            let mut theirs = f64::INFINITY;
            for (x, y) in other {
                theirs = theirs.min(normal_x * x + normal_y * y);
                if theirs < own {
                    break;
                }
            }
            if own <= theirs {
                return true;
            }
        }
        false
    }

    /// Monotone-chain convex hull, counter-clockwise, on grid coordinates.
    ///
    /// Collinear points are dropped (`cross <= 0.0`), which keeps the hull
    /// minimal and every edge normal non-zero. `points` is sorted in place and
    /// `hull` is overwritten; both are the caller's reused buffers.
    fn convex_hull_into(points: &mut Vec<(f64, f64)>, hull: &mut Vec<(f64, f64)>) {
        points.sort_by(|first, second| {
            first
                .0
                .total_cmp(&second.0)
                .then_with(|| first.1.total_cmp(&second.1))
        });
        points.dedup();
        hull.clear();
        if points.len() < 3 {
            return;
        }
        let cross = |origin: (f64, f64), first: (f64, f64), second: (f64, f64)| {
            (first.0 - origin.0) * (second.1 - origin.1)
                - (first.1 - origin.1) * (second.0 - origin.0)
        };
        for &point in points.iter() {
            while hull.len() >= 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], point) <= 0.0
            {
                hull.pop();
            }
            hull.push(point);
        }
        let lower = hull.len() + 1;
        for &point in points.iter().rev() {
            while hull.len() >= lower
                && cross(hull[hull.len() - 2], hull[hull.len() - 1], point) <= 0.0
            {
                hull.pop();
            }
            hull.push(point);
        }
        hull.pop();
    }

    /// The parent's certificate material, one entry per piece.
    ///
    /// A parent is fixed for the whole expansion of one beam slot — every
    /// confirmation row of that slot asks about the same active pieces — so this
    /// is built once per [`construct_candidate_poses`] call and reused by every
    /// row the slot generates. Rebuilding is keyed on the stored `Arc` identity,
    /// so a parent that carries the same collision object as the previous one
    /// costs nothing.
    ///
    /// [`construct_candidate_poses`]: crate::search::general_persistent_vacancy
    #[derive(Default)]
    pub(crate) struct ConfirmShields {
        shields: Vec<Option<(*const PolygonSet, PairShield)>>,
        /// The certificate of the confirmation row in progress, refilled in
        /// place rather than allocated per row.
        candidate: PairShield,
        /// The point buffer every certificate builds its hull through.
        points: Vec<(f64, f64)>,
    }

    // The raw pointer is an *identity* token and is never dereferenced: it is
    // compared against the address of a live `Arc` the caller is holding, purely
    // to decide whether a cached shield still describes it. `ConfirmShields` is
    // owned by `RunWork`, which never crosses a thread boundary.
    unsafe impl Send for ConfirmShields {}

    impl ConfirmShields {
        /// Refreshes the cache for a new expansion parent.
        pub(crate) fn begin_parent(&mut self, collisions: &[Option<Arc<PolygonSet>>]) {
            if self.shields.len() != collisions.len() {
                self.shields.clear();
                self.shields.resize_with(collisions.len(), || None);
            }
            for (slot, collision) in self.shields.iter_mut().zip(collisions.iter()) {
                let Some(collision) = collision else {
                    *slot = None;
                    continue;
                };
                let identity = Arc::as_ptr(collision);
                if slot.as_ref().is_some_and(|(cached, _)| *cached == identity) {
                    continue;
                }
                let mut shield = slot.take().map_or_else(PairShield::default, |(_, s)| s);
                shield.rebuild(collision, &mut self.points);
                *slot = Some((identity, shield));
            }
        }

        /// Derives the certificate of the confirmation row about to run.
        pub(crate) fn begin_candidate(&mut self, collision: &PolygonSet) {
            let mut candidate = std::mem::take(&mut self.candidate);
            candidate.rebuild(collision, &mut self.points);
            self.candidate = candidate;
        }

        /// Whether the parent's piece at `index` is provably separated from the
        /// candidate of the row in progress.
        pub(crate) fn separated(&self, index: usize) -> bool {
            let Some(Some((_, fixed))) = self.shields.get(index) else {
                return false;
            };
            self.candidate.separated_from(fixed)
        }
    }
}

/// The forwarder compiled when `fast-constructor-confirm` is off: a zero-sized
/// value whose one query always answers "no information", so every pair reaches
/// the exact tier exactly as it did before.
#[cfg(not(feature = "fast-constructor-confirm"))]
mod armed {
    use std::sync::Arc;

    use crate::geometry::general_polygon::PolygonSet;

    #[derive(Default)]
    pub(crate) struct ConfirmShields;

    impl ConfirmShields {
        #[inline(always)]
        pub(crate) fn begin_parent(&mut self, collisions: &[Option<Arc<PolygonSet>>]) {
            let _ = collisions;
        }

        #[inline(always)]
        pub(crate) fn begin_candidate(&mut self, collision: &PolygonSet) {
            let _ = collision;
        }

        #[inline(always)]
        pub(crate) fn separated(&self, index: usize) -> bool {
            let _ = index;
            false
        }
    }
}

pub(crate) use armed::ConfirmShields;

#[cfg(all(test, feature = "fast-constructor-confirm"))]
mod tests {
    use super::armed::PairShield;
    use crate::domain::IrregularPoint;
    use crate::geometry::general_polygon::{PolygonRegion, PolygonSet};

    fn polygon(points: &[(f64, f64)]) -> PolygonSet {
        PolygonSet::new(vec![PolygonRegion::new(
            points
                .iter()
                .map(|(x, y)| IrregularPoint::new(*x, *y))
                .collect(),
            Vec::new(),
        )
        .expect("valid region")])
        .expect("valid set")
    }

    fn shield(points: &[(f64, f64)]) -> PairShield {
        PairShield::for_test(&polygon(points))
    }

    /// The property the whole design rests on, checked by brute force: over a
    /// dense grid of relative placements of two non-convex polygons, the shield
    /// may never report "separated" for a pair whose exact intersection is
    /// positive.
    #[test]
    fn a_separation_certificate_is_never_issued_for_an_overlapping_pair() {
        // An L, which no convex test can describe, and a bar that fits into its
        // notch.
        let ell = [
            (0.0, 0.0),
            (30.0, 0.0),
            (30.0, 10.0),
            (10.0, 10.0),
            (10.0, 30.0),
            (0.0, 30.0),
        ];
        let bar = [(0.0, 0.0), (14.0, 0.0), (14.0, 6.0), (0.0, 6.0)];
        let first = polygon(&ell);
        let first_shield = shield(&ell);
        let mut certificates = 0;
        let mut overlaps = 0;
        for step_x in -40..=40 {
            for step_y in -40..=40 {
                let (dx, dy) = (step_x as f64, step_y as f64);
                let moved: Vec<(f64, f64)> =
                    bar.iter().map(|(x, y)| (x + dx, y + dy)).collect();
                let second = polygon(&moved);
                let separated = first_shield.separated_from(&shield(&moved));
                let overlapping = first
                    .intersection_area_mm2(&second)
                    .expect("a small pair query succeeds")
                    > 0.0;
                assert!(
                    !(separated && overlapping),
                    "a separation certificate was issued for an overlapping pair at ({dx}, {dy})"
                );
                certificates += usize::from(separated);
                overlaps += usize::from(overlapping);
            }
        }
        // The corpus has to contain both populations for the assertion above to
        // mean anything.
        assert!(certificates > 100, "expected many separated placements");
        assert!(overlaps > 100, "expected many overlapping placements");
    }

    /// Touching is not overlapping, and the certificate says so — which is the
    /// case the `<=` comparison in the slab test exists for.
    #[test]
    fn edge_contact_is_reported_separated() {
        let left = shield(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]);
        let right = shield(&[(10.0, 0.0), (20.0, 0.0), (20.0, 10.0), (10.0, 10.0)]);
        assert!(left.separated_from(&right));
    }

    /// The second tier earns its place: a pair the slabs cannot separate and
    /// the hulls can.
    #[test]
    fn the_hull_tier_separates_what_the_slabs_cannot() {
        // Two slivers along direction (2, -1), offset along (1, 2). Every one
        // of the four slab directions overlaps, because none of them is (1, 2);
        // the slivers' own long edge is the separating axis.
        let lower = shield(&[(0.0, 0.0), (40.0, -20.0), (40.0, -19.0), (0.0, 1.0)]);
        let upper = shield(&[(2.0, 4.0), (42.0, -16.0), (42.0, -15.0), (2.0, 5.0)]);
        assert!(
            !lower.slabs_separated_from(&upper),
            "this pair is the one tier one cannot decide"
        );
        assert!(lower.separated_from(&upper));
    }
}
