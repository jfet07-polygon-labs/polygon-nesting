//! The constructor's **inner** overlap certificate.
//!
//! [`construction_confirm_shield`] removes exact pair queries whose answer is
//! "clean". The census that sized it also measured the ceiling of that idea and
//! found it low: 77.3% of the pair questions the constructor takes to Clipper
//! are genuine overlaps, and no *outer* approximation can ever remove one of
//! those. Proving an overlap needs a certificate in the opposite direction.
//!
//! This module is that certificate, and it attacks the larger of the two
//! numbers the census produced. The candidate stream offers 432,710
//! confirmation rows on the gate-1 stream and accepts 2.6% of them; 78.66% of
//! all collision-polygon builds are spent on a pose the row discards two lines
//! later, and `collisionPolygonBuild` is 20% of leaf. A proof that a pose
//! **does** overlap, evaluated *before* the build, removes the build and every
//! pair question behind it.
//!
//! # The certificate
//!
//! Let `S` be a piece's source polygon and `C(S, p, e) = offset(snap(T_p(S)), e)`
//! the collision polygon the constructor builds at pose `p` for expansion `e` —
//! one rigid transform, one grid snap, one Clipper miter offset.
//!
//! For two poses to be *rejected* it is enough that their collision polygons
//! intersect in positive area. So: cover each polygon from the inside with
//! discs, and two discs that overlap are a proof.
//!
//! * **The fixed side needs no lemma at all.** The parent's collision polygons
//!   already exist — the row is being confirmed *against* them — so the discs
//!   are inscribed directly in `C`, by measuring the distance from a sample
//!   point to every ring of `C`. `disc(c, r) ⊆ C` is then a computation, not an
//!   assumption.
//! * **The candidate side is the whole geometric content**, because its `C` is
//!   exactly what this module exists not to build. Its discs are inscribed in
//!   the *source* `S` and then transformed rigidly and inflated by `e`. That
//!   step is sound iff `C(S, p, e) ⊇ snap(T_p(S)) ⊕ disc(e)`, which is the
//!   containment the census entry recorded as "believable for Clipper's miter
//!   join and not proved here". It is proved now; see the ledger chapter
//!   "The inner certificate" in `docs/next-generation-engine-plan.md`, and the
//!   sketch under [`CANDIDATE_SLACK_MM`] for the numeric slack it needs.
//!
//! Every erosion the two discretisations cost is charged explicitly and
//! generously, and the resulting test is
//!
//! ```text
//! |c_cand - c_fixed| < (r_cand + e - CANDIDATE_SLACK) + (r_fixed - FIXED_SLACK) - LENS_MARGIN
//! ```
//!
//! which answers "provably overlapping" or "no information", never "probably".
//!
//! # What it is allowed to decide
//!
//! Exactly one thing: that a confirmation row returns `None`. That is the same
//! value the row returns when the exact tier finds an overlap, produced without
//! running it — so, like the separation shield, the mechanism is a proof that
//! substitutes a verdict rather than a heuristic that changes one. The poses the
//! constructor accepts are unchanged, in the same order, at the same row
//! charges.
//!
//! A `debug_assert` on the reject path closes the loop empirically: in a debug
//! build every certified row still builds its collision polygon and asks
//! Clipper, and the exact area must be positive.
//!
//! [`construction_confirm_shield`]: crate::search::construction_confirm_shield

#[cfg(any(feature = "constructor-census", feature = "fast-constructor-reject"))]
mod armed {
    use std::sync::Arc;

    use crate::domain::IrregularPoint;
    use crate::geometry::general_polygon::{PolygonRing, PolygonSet};

    /// How many inscribed discs one cover carries.
    ///
    /// The census prices prefixes of this cover — one, two, four and eight
    /// discs — because the greedy that builds it is nested, so its first `k`
    /// picks *are* the `k`-disc cover.
    pub(crate) const COVER_DISCS: usize = 8;

    /// How many of those discs the *armed* query uses.
    ///
    /// The census priced the ladder on the mode-20 gate-1 stream: the candidate
    /// stream's certified rows are 288,693 at one disc, 320,216 at two, 330,260
    /// at four and 330,746 at eight. Four reaches 99.85% of what eight reaches
    /// for a quarter of the pair arithmetic, so four is where the knee is and
    /// the cover is still built at eight so the census can keep pricing past it.
    pub(crate) const REJECT_DISCS: usize = 4;

    /// How many discs a *row* poses. The counting build has to pose the whole
    /// cover, because it prices prefixes it does not act on; every other build
    /// poses only the discs its query will read.
    const POSED_DISCS: usize = if cfg!(feature = "constructor-census") {
        COVER_DISCS
    } else {
        REJECT_DISCS
    };

    /// Millimetres eroded from a candidate disc's certified radius.
    ///
    /// The candidate's chain is `source ring (exact mm) -> rigid transform ->
    /// grid snap -> Clipper miter offset -> integer rounding of the emitted
    /// offset vertices`, and each discretisation moves a boundary point by a
    /// bounded amount. A polygon whose boundary moves by at most `d` in
    /// Hausdorff distance still contains `disc(c, r - d)` for any
    /// `disc(c, r)` it contained, so the erosions simply add:
    ///
    /// | step | bound | mm |
    /// |---|---|---|
    /// | grid snap of the transformed ring | half a grid unit per axis | 0.000708 |
    /// | rounding of the offset distance itself | half a grid unit | 0.000500 |
    /// | `math_round` on each emitted offset vertex | half a grid unit per axis | 0.000708 |
    /// | `f64` in the rotation and the distance evaluation | ~1e-12 relative | negligible |
    ///
    /// The sum is 0.001916 mm. This constant is 0.005 mm — a 2.6x margin over
    /// the derivation, which costs nothing: the quantity it is subtracted from
    /// is `r + e`, and `e` alone is millimetres on any request that has a
    /// clearance at all.
    const CANDIDATE_SLACK_MM: f64 = 0.005;

    /// Millimetres eroded from a fixed disc's certified radius.
    ///
    /// The fixed side is measured on the collision polygon itself, whose
    /// vertices are the exact grid points the Clipper query is executed on, so
    /// the only error is `f64` in the distance evaluation — of order 1e-12 mm.
    /// A thousandth of a millimetre is one whole grid unit and is charged for
    /// tidiness rather than necessity.
    const FIXED_SLACK_MM: f64 = 0.001;

    /// Millimetres of penetration the certificate insists on beyond contact.
    ///
    /// Two discs at `d < R1 + R2` overlap in a lens of positive area for any
    /// positive penetration, but the exact query answers on the integer grid,
    /// and a lens thinner than the grid could in principle round away. At
    /// 0.02 mm the lens is 0.02 mm deep and, for the smallest inscribed radius
    /// this constant is worth applying to, over 0.001 mm^2 — a thousand grid
    /// area units. The `debug_assert` on the reject path is the empirical half
    /// of this claim.
    const LENS_MARGIN_MM: f64 = 0.02;

    /// Samples along the longer bounding-box axis when covering a polygon.
    const COVER_SAMPLES: usize = 28;

    /// Hill-climbing rounds each picked disc centre is refined for.
    const REFINE_ROUNDS: usize = 12;

    #[derive(Clone, Copy, Debug, Default)]
    struct Disc {
        x: f64,
        y: f64,
        r: f64,
    }

    /// A set of discs proved to lie inside one polygon, plus their extent.
    #[derive(Clone, Debug, Default)]
    pub(crate) struct InnerCover {
        discs: Vec<Disc>,
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
    }

    impl InnerCover {
        fn seal(&mut self) {
            self.min_x = f64::INFINITY;
            self.min_y = f64::INFINITY;
            self.max_x = f64::NEG_INFINITY;
            self.max_y = f64::NEG_INFINITY;
            for disc in &self.discs {
                self.min_x = self.min_x.min(disc.x - disc.r);
                self.min_y = self.min_y.min(disc.y - disc.r);
                self.max_x = self.max_x.max(disc.x + disc.r);
                self.max_y = self.max_y.max(disc.y + disc.r);
            }
        }

        #[inline]
        fn disjoint_extent(&self, other: &Self) -> bool {
            self.max_x <= other.min_x
                || other.max_x <= self.min_x
                || self.max_y <= other.min_y
                || other.max_y <= self.min_y
        }
    }

    /// Scratch buffers the cover builder reuses across polygons.
    #[derive(Default)]
    struct CoverScratch {
        samples: Vec<(f64, f64, f64)>,
    }

    /// Squared distance from `(x, y)` to the segment `(ax, ay)-(bx, by)`.
    #[inline]
    fn segment_distance_squared(x: f64, y: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
        let (dx, dy) = (bx - ax, by - ay);
        let length_squared = dx * dx + dy * dy;
        let t = if length_squared <= 0.0 {
            0.0
        } else {
            (((x - ax) * dx + (y - ay) * dy) / length_squared).clamp(0.0, 1.0)
        };
        let (px, py) = (ax + t * dx - x, ay + t * dy - y);
        px * px + py * py
    }

    /// One ring's vertices, in millimetres.
    ///
    /// `source` selects the *pre-snap* ring a transform is applied to over the
    /// snapped one the Clipper path carries. The candidate side needs the
    /// former, because `PolygonSet::transformed` transforms `source_points`;
    /// the fixed side needs the latter, because that is the polygon the exact
    /// query is executed on.
    #[inline]
    fn ring_points(ring: &PolygonRing, source: bool) -> &[IrregularPoint] {
        if source {
            ring.source_points()
        } else {
            ring.points()
        }
    }

    /// Every ring of `set`, outer rings and holes alike.
    fn rings(set: &PolygonSet, source: bool) -> impl Iterator<Item = &[IrregularPoint]> {
        set.regions().iter().flat_map(move |region| {
            std::iter::once(region.outer_ring())
                .chain(region.hole_rings())
                .map(move |ring| ring_points(ring, source))
        })
    }

    /// Whether `(x, y)` is inside the material of `set`, holes removed.
    fn inside(set: &PolygonSet, source: bool, x: f64, y: f64) -> bool {
        set.regions().iter().any(|region| {
            ring_contains(ring_points(region.outer_ring(), source), x, y)
                && !region
                    .hole_rings()
                    .iter()
                    .any(|hole| ring_contains(ring_points(hole, source), x, y))
        })
    }

    /// Crossing-number point-in-ring, on a closed ring given in order.
    fn ring_contains(points: &[IrregularPoint], x: f64, y: f64) -> bool {
        if points.len() < 3 {
            return false;
        }
        let mut inside = false;
        let mut previous = points[points.len() - 1];
        for current in points {
            if (current.y > y) != (previous.y > y) {
                let t = (y - current.y) / (previous.y - current.y);
                if x < current.x + t * (previous.x - current.x) {
                    inside = !inside;
                }
            }
            previous = *current;
        }
        inside
    }

    /// Distance from `(x, y)` to the nearest boundary point of `set`.
    fn boundary_distance(set: &PolygonSet, source: bool, x: f64, y: f64) -> f64 {
        let mut best = f64::INFINITY;
        for points in rings(set, source) {
            if points.len() < 2 {
                continue;
            }
            let mut previous = points[points.len() - 1];
            for current in points {
                best = best.min(segment_distance_squared(
                    x, y, previous.x, previous.y, current.x, current.y,
                ));
                previous = *current;
            }
        }
        best.sqrt()
    }

    /// Builds a nested greedy inner cover of `set` into `cover`.
    ///
    /// Deterministic by construction: a fixed sample lattice, a fixed greedy
    /// rule with the sample index as the tie-break, and a fixed refinement
    /// schedule. No randomness and no iteration over a hash container.
    fn build_cover(
        set: &PolygonSet,
        source: bool,
        cover: &mut InnerCover,
        scratch: &mut CoverScratch,
    ) {
        cover.discs.clear();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for points in rings(set, source) {
            for point in points {
                min_x = min_x.min(point.x);
                min_y = min_y.min(point.y);
                max_x = max_x.max(point.x);
                max_y = max_y.max(point.y);
            }
        }
        let (width, height) = (max_x - min_x, max_y - min_y);
        if !(width > 0.0 && height > 0.0) {
            cover.seal();
            return;
        }
        let longest = width.max(height);
        let samples_x =
            (((COVER_SAMPLES as f64) * width / longest).round() as usize).clamp(3, COVER_SAMPLES);
        let samples_y =
            (((COVER_SAMPLES as f64) * height / longest).round() as usize).clamp(3, COVER_SAMPLES);
        let (step_x, step_y) = (width / samples_x as f64, height / samples_y as f64);

        scratch.samples.clear();
        for ix in 0..samples_x {
            let x = min_x + (ix as f64 + 0.5) * step_x;
            for iy in 0..samples_y {
                let y = min_y + (iy as f64 + 0.5) * step_y;
                if !inside(set, source, x, y) {
                    continue;
                }
                let radius = boundary_distance(set, source, x, y);
                if radius > 0.0 {
                    scratch.samples.push((x, y, radius));
                }
            }
        }
        if scratch.samples.is_empty() {
            cover.seal();
            return;
        }

        let refine_step = step_x.max(step_y) * 0.5;
        for _ in 0..COVER_DISCS {
            // The best sample not already inside a chosen disc. Ties go to the
            // earlier sample, which is a fixed lattice order.
            let mut best: Option<(usize, f64)> = None;
            for (index, &(x, y, radius)) in scratch.samples.iter().enumerate() {
                let covered = cover.discs.iter().any(|disc| {
                    let (dx, dy) = (x - disc.x, y - disc.y);
                    dx * dx + dy * dy <= disc.r * disc.r
                });
                if covered {
                    continue;
                }
                if best.is_none_or(|(_, best_radius)| radius > best_radius) {
                    best = Some((index, radius));
                }
            }
            let Some((index, _)) = best else { break };
            let (mut x, mut y, mut radius) = scratch.samples[index];
            // Hill-climb the centre for a larger inscribed radius. Every probe
            // is verified inside, so the result is still a proof.
            let mut step = refine_step;
            for _ in 0..REFINE_ROUNDS {
                let mut improved = false;
                for (dx, dy) in [
                    (1.0, 0.0),
                    (-1.0, 0.0),
                    (0.0, 1.0),
                    (0.0, -1.0),
                    (0.7071067811865476, 0.7071067811865476),
                    (-0.7071067811865476, 0.7071067811865476),
                    (0.7071067811865476, -0.7071067811865476),
                    (-0.7071067811865476, -0.7071067811865476),
                ] {
                    let (px, py) = (x + dx * step, y + dy * step);
                    if !inside(set, source, px, py) {
                        continue;
                    }
                    let probe = boundary_distance(set, source, px, py);
                    if probe > radius {
                        x = px;
                        y = py;
                        radius = probe;
                        improved = true;
                    }
                }
                if !improved {
                    step *= 0.5;
                }
            }
            cover.discs.push(Disc { x, y, r: radius });
        }
        cover.seal();
    }

    /// The constructor's inner-certificate material.
    ///
    /// Two caches with different lifetimes, both keyed on pointer identity in
    /// exactly the sense [`ConfirmShields`] uses: an address compared against a
    /// live borrow the caller is holding, never dereferenced.
    ///
    /// [`ConfirmShields`]: crate::search::construction_confirm_shield::ConfirmShields
    #[derive(Default)]
    pub(crate) struct RejectCertificates {
        /// Source-frame cover per piece index. A piece's source polygon is
        /// fixed for the whole run, so this is built once each.
        sources: Vec<Option<(*const PolygonSet, InnerCover)>>,
        /// Cover of each parent collision polygon, rebuilt when the slot's
        /// stored `Arc` changes.
        fixed: Vec<Option<(*const PolygonSet, InnerCover)>>,
        /// The row in progress: the candidate's source cover, transformed to
        /// the pose and inflated by the expansion.
        candidate: InnerCover,
        /// How much of that inflation came from the expansion, so the counting
        /// build can price the certificate with the inflation taken back off.
        inflation: f64,
        scratch: CoverScratch,
    }

    // See `ConfirmShields`: the raw pointers are identity tokens, never
    // dereferenced, and `RunWork` never crosses a thread boundary.
    unsafe impl Send for RejectCertificates {}

    impl RejectCertificates {
        /// Refreshes the fixed covers for a new expansion parent.
        pub(crate) fn begin_parent(&mut self, collisions: &[Option<Arc<PolygonSet>>]) {
            if self.fixed.len() != collisions.len() {
                self.fixed.clear();
                self.fixed.resize_with(collisions.len(), || None);
            }
            for (slot, collision) in self.fixed.iter_mut().zip(collisions.iter()) {
                let Some(collision) = collision else {
                    *slot = None;
                    continue;
                };
                let identity = Arc::as_ptr(collision);
                if slot.as_ref().is_some_and(|(cached, _)| *cached == identity) {
                    continue;
                }
                let mut cover = slot.take().map_or_else(InnerCover::default, |(_, c)| c);
                build_cover(collision, false, &mut cover, &mut self.scratch);
                for disc in &mut cover.discs {
                    disc.r -= FIXED_SLACK_MM;
                }
                cover.discs.retain(|disc| disc.r > 0.0);
                cover.seal();
                *slot = Some((identity, cover));
            }
        }

        /// Poses the candidate piece's source cover for the row about to run.
        pub(crate) fn begin_candidate(
            &mut self,
            source: &PolygonSet,
            piece_index: usize,
            rotation_deg: f64,
            mirrored: bool,
            translate_x: f64,
            translate_y: f64,
            expansion_mm: f64,
        ) {
            if self.sources.len() <= piece_index {
                self.sources.resize_with(piece_index + 1, || None);
            }
            let identity = source as *const PolygonSet;
            let stale = !self.sources[piece_index]
                .as_ref()
                .is_some_and(|(cached, _)| *cached == identity);
            if stale {
                let mut cover = self.sources[piece_index]
                    .take()
                    .map_or_else(InnerCover::default, |(_, c)| c);
                build_cover(source, true, &mut cover, &mut self.scratch);
                self.sources[piece_index] = Some((identity, cover));
            }
            // The rotation is the one `PolygonSet::transformed` applies:
            // mirror in x, rotate counter-clockwise, then translate.
            let (sin, cos) = rotation_deg.to_radians().sin_cos();
            let inflation = expansion_mm - CANDIDATE_SLACK_MM;
            self.inflation = expansion_mm.max(0.0);
            self.candidate.discs.clear();
            let Some((_, cover)) = self.sources[piece_index].as_ref() else {
                self.candidate.seal();
                return;
            };
            for disc in cover.discs.iter().take(POSED_DISCS) {
                let radius = disc.r + inflation;
                if radius <= 0.0 {
                    continue;
                }
                let mirror_x = if mirrored { -disc.x } else { disc.x };
                self.candidate.discs.push(Disc {
                    x: mirror_x * cos - disc.y * sin + translate_x,
                    y: mirror_x * sin + disc.y * cos + translate_y,
                    r: radius,
                });
            }
            self.candidate.seal();
        }

        /// The deepest overlap this row can **prove** against the parent's
        /// active pieces, in millimetres, or `None` when it can prove none.
        ///
        /// `discs` bounds how many discs per cover participate; the cover is
        /// nested, so a prefix is a smaller cover rather than a different one.
        pub(crate) fn proven_overlap(&self, active: &[bool], discs: usize) -> Option<f64> {
            if self.candidate.discs.is_empty() {
                return None;
            }
            let mut deepest = 0.0f64;
            for (index, slot) in self.fixed.iter().enumerate() {
                if !active.get(index).copied().unwrap_or(false) {
                    continue;
                }
                let Some((_, fixed)) = slot else { continue };
                if fixed.discs.is_empty() || self.candidate.disjoint_extent(fixed) {
                    continue;
                }
                for candidate in self.candidate.discs.iter().take(discs) {
                    for other in fixed.discs.iter().take(discs) {
                        let reach = candidate.r + other.r - LENS_MARGIN_MM;
                        if reach <= 0.0 {
                            continue;
                        }
                        let (dx, dy) = (candidate.x - other.x, candidate.y - other.y);
                        let distance_squared = dx * dx + dy * dy;
                        if distance_squared < reach * reach {
                            deepest = deepest.max(reach - distance_squared.sqrt());
                        }
                    }
                }
            }
            (deepest > 0.0).then_some(deepest)
        }

        /// The certificate with the whole expansion inflation taken back off
        /// the candidate's discs.
        ///
        /// A counting-build query, and the one that prices the *risk* rather
        /// than the prize. A disc inscribed in the source is inside the
        /// collision polygon under nothing more than `offset(P, e) ⊇ P` for
        /// `e >= 0`; the extra `+ e` needs the Minkowski containment `(★)`
        /// argued in the ledger. This counts how many rows the certificate
        /// would still prove if `(★)` were abandoned entirely — the fallback
        /// the design can retreat to without changing a line of its structure.
        #[cfg(feature = "constructor-census")]
        pub(crate) fn proven_overlap_without_inflation(
            &self,
            active: &[bool],
            discs: usize,
        ) -> bool {
            for (index, slot) in self.fixed.iter().enumerate() {
                if !active.get(index).copied().unwrap_or(false) {
                    continue;
                }
                let Some((_, fixed)) = slot else { continue };
                for candidate in self.candidate.discs.iter().take(discs) {
                    for other in fixed.discs.iter().take(discs) {
                        let reach =
                            candidate.r - self.inflation + other.r - LENS_MARGIN_MM;
                        if reach <= 0.0 {
                            continue;
                        }
                        let (dx, dy) = (candidate.x - other.x, candidate.y - other.y);
                        if dx * dx + dy * dy < reach * reach {
                            return true;
                        }
                    }
                }
            }
            false
        }

        /// The **signed** proximity of this row to the parent's active pieces:
        /// positive is the proven overlap depth [`Self::proven_overlap`]
        /// returns, negative is the closest approach the certificate could not
        /// close.
        ///
        /// A counting-build query only. It is the graded version of the
        /// certificate — a ranking signal rather than a proof — and it exists so
        /// the census can price *ordering* the candidate stream by it against
        /// pruning it, which are different changes with different soundness.
        /// It takes no extent shortcut, because a shortcut would corrupt the
        /// negative branch it is measured for.
        #[cfg(feature = "constructor-census")]
        pub(crate) fn signed_pressure(&self, active: &[bool], discs: usize) -> f64 {
            let mut best = f64::NEG_INFINITY;
            for (index, slot) in self.fixed.iter().enumerate() {
                if !active.get(index).copied().unwrap_or(false) {
                    continue;
                }
                let Some((_, fixed)) = slot else { continue };
                for candidate in self.candidate.discs.iter().take(discs) {
                    for other in fixed.discs.iter().take(discs) {
                        let reach = candidate.r + other.r - LENS_MARGIN_MM;
                        let (dx, dy) = (candidate.x - other.x, candidate.y - other.y);
                        best = best.max(reach - (dx * dx + dy * dy).sqrt());
                    }
                }
            }
            if best.is_finite() {
                best
            } else {
                f64::NEG_INFINITY
            }
        }
    }
}

/// The forwarder compiled when neither the census nor the reject flag is on: a
/// zero-sized value whose one query always answers "no proof", so every row
/// builds and confirms exactly as it did before.
#[cfg(not(any(feature = "constructor-census", feature = "fast-constructor-reject")))]
mod armed {
    use std::sync::Arc;

    use crate::geometry::general_polygon::PolygonSet;

    #[derive(Default)]
    pub(crate) struct RejectCertificates;

    impl RejectCertificates {
        #[inline(always)]
        pub(crate) fn begin_parent(&mut self, collisions: &[Option<Arc<PolygonSet>>]) {
            let _ = collisions;
        }

        #[allow(clippy::too_many_arguments)]
        #[inline(always)]
        pub(crate) fn begin_candidate(
            &mut self,
            source: &PolygonSet,
            piece_index: usize,
            rotation_deg: f64,
            mirrored: bool,
            translate_x: f64,
            translate_y: f64,
            expansion_mm: f64,
        ) {
            let _ = (
                source,
                piece_index,
                rotation_deg,
                mirrored,
                translate_x,
                translate_y,
                expansion_mm,
            );
        }

        #[inline(always)]
        pub(crate) fn proven_overlap(&self, active: &[bool], discs: usize) -> Option<f64> {
            let _ = (active, discs);
            None
        }
    }
}

pub(crate) use armed::RejectCertificates;

#[cfg(any(feature = "constructor-census", feature = "fast-constructor-reject"))]
pub(crate) use armed::REJECT_DISCS;

/// The full cover size, which only the counting build reads: it prices the
/// prefixes the armed query does not use.
#[cfg(feature = "constructor-census")]
pub(crate) use armed::COVER_DISCS;

#[cfg(all(test, feature = "fast-constructor-reject"))]
mod tests {
    use std::sync::Arc;

    use super::armed::{RejectCertificates, REJECT_DISCS};
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

    /// The property the whole design rests on, checked by brute force in the
    /// direction that matters: over a dense grid of relative placements of two
    /// non-convex pieces, the certificate may never claim an overlap for a pair
    /// whose exact collision polygons are disjoint.
    #[test]
    fn an_overlap_certificate_is_never_issued_for_a_disjoint_pair() {
        let ell = [
            (0.0, 0.0),
            (30.0, 0.0),
            (30.0, 10.0),
            (10.0, 10.0),
            (10.0, 30.0),
            (0.0, 30.0),
        ];
        let bar = [(0.0, 0.0), (14.0, 0.0), (14.0, 6.0), (0.0, 6.0)];
        let expansion = 1.25;
        let fixed_source = polygon(&ell);
        let fixed_collision = Arc::new(
            fixed_source
                .transformed(0.0, false, 0.0, 0.0)
                .expect("transform")
                .offset(expansion)
                .expect("offset"),
        );
        let moving_source = polygon(&bar);

        let mut certificates = RejectCertificates::default();
        certificates.begin_parent(&[Some(Arc::clone(&fixed_collision))]);
        let active = [true];

        let mut proofs = 0;
        let mut overlaps = 0;
        for step_x in -50..=50 {
            for step_y in -50..=50 {
                let (dx, dy) = (step_x as f64 * 0.9, step_y as f64 * 0.9);
                certificates.begin_candidate(&moving_source, 0, 37.0, false, dx, dy, expansion);
                let proved = certificates.proven_overlap(&active, REJECT_DISCS).is_some();
                let moving_collision = moving_source
                    .transformed(37.0, false, dx, dy)
                    .expect("transform")
                    .offset(expansion)
                    .expect("offset");
                let overlapping = moving_collision
                    .intersection_area_mm2(&fixed_collision)
                    .expect("a small pair query succeeds")
                    > 0.0;
                assert!(
                    !(proved && !overlapping),
                    "an overlap certificate was issued for a disjoint pair at ({dx}, {dy})"
                );
                proofs += usize::from(proved);
                overlaps += usize::from(overlapping);
            }
        }
        assert!(proofs > 100, "expected many proved placements, got {proofs}");
        assert!(
            overlaps > proofs,
            "the certificate is conservative, so it cannot prove more than exist"
        );
    }

    /// The certificate holds under a mirror as well as a rotation, because the
    /// disc centres travel through the same transform the polygon does.
    #[test]
    fn a_mirrored_pose_transforms_its_cover_the_same_way() {
        let wedge = [(0.0, 0.0), (20.0, 0.0), (20.0, 4.0), (6.0, 4.0), (6.0, 14.0), (0.0, 14.0)];
        let expansion = 0.75;
        let source = polygon(&wedge);
        let fixed = Arc::new(
            source
                .transformed(0.0, false, 0.0, 0.0)
                .expect("transform")
                .offset(expansion)
                .expect("offset"),
        );
        let mut certificates = RejectCertificates::default();
        certificates.begin_parent(&[Some(Arc::clone(&fixed))]);
        let active = [true];
        for step in -30..=30 {
            let dx = step as f64 * 0.7;
            certificates.begin_candidate(&source, 0, 113.0, true, dx, 2.0, expansion);
            let proved = certificates.proven_overlap(&active, REJECT_DISCS).is_some();
            let moving = source
                .transformed(113.0, true, dx, 2.0)
                .expect("transform")
                .offset(expansion)
                .expect("offset");
            let overlapping = moving
                .intersection_area_mm2(&fixed)
                .expect("a small pair query succeeds")
                > 0.0;
            assert!(
                !(proved && !overlapping),
                "an overlap certificate was issued for a disjoint mirrored pose at {dx}"
            );
        }
    }
}
