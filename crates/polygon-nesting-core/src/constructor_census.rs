//! The constructor's exact-confirmation census.
//!
//! A **counting build**, in the sense the pole-loop and collider stages of this
//! project use the term: an opt-in feature whose only job is to answer a
//! question about a stream before anybody writes code that assumes an answer.
//! It changes no search decision, is compiled to nothing when off, and its
//! numbers are counts, never times — a build that carries it runs the extra
//! prefilter arithmetic on every pair it observes, so its clock is not the
//! production clock and nothing here may be quoted as one.
//!
//! # The question
//!
//! After the bit-grid redesign the mode-20 constructor's leaf time is
//! `exactOverlapTest` 33.1% plus `collisionPolygonBuild` 20.1% — the exact
//! confirmation *inside* construction. Three things had to be established
//! before a prefilter could be designed:
//!
//! 1. **How much of the exact work is spent proving a negative.** A pair query
//!    that reports "no overlap" is a query a cheap conservative test could have
//!    answered, if one exists that is *sound*.
//! 2. **How much of the collision-polygon build is spent on candidates that
//!    then fail** — a build whose pose is rejected two lines later.
//! 3. **Where the queries come from.** The constructor asks its exact question
//!    from three structurally different places (a station/anchor/shelf
//!    candidate's first confirmation, a contact slide's geometric ladder, and
//!    that slide's bisection refinement), and they have very different hit
//!    rates, so an ordering change and a pruning change are not the same
//!    change.
//!
//! # The prefilter ladder this prices
//!
//! For every pair that reaches Clipper, three conservative separation tests are
//! evaluated alongside the exact answer:
//!
//! | test | what it is | soundness |
//! |---|---|---|
//! | `aabb` | the axis-aligned box test already in the code | exact on the grid |
//! | `slabs` | [`GridSlabs`](crate::geometry::general_polygon) — box plus the two diagonals | exact on the grid |
//! | `hull` | separating-axis over both convex hulls | exact on the grid |
//!
//! All three are computed on the integer Clipper path, so all three are
//! *proofs* rather than estimates: coordinates are integer-valued `f64`, and the
//! projections and cross products below stay far inside the exactly
//! representable range for any sheet a request can describe. That is the
//! property the census is really measuring — not "does a cheap test usually
//! agree", which would be worthless, but "how much of the exact work can be
//! removed by a test that cannot disagree".
//!
//! Every observation additionally asserts the implication in the direction that
//! matters: a pair the exact query reports as *overlapping* must not be reported
//! separated by any of the three. Those counters are published as
//! `soundnessViolations` and are expected to be zero; a non-zero value falsifies
//! the whole design and is meant to be read before anything else.

#[cfg(feature = "constructor-census")]
mod armed {
    use std::cell::Cell;
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::geometry::general_polygon::PolygonSet;

    /// Where in the constructor an exact confirmation was asked for.
    ///
    /// The two slide sites are separated because they answer different design
    /// questions: the ladder is a *speculative* descent that expects to fail at
    /// its last rung, while the bisection is a refinement between one known-good
    /// and one known-bad offset.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Site {
        /// A candidate's first confirmation in the ranked stream — station
        /// hints, the shelf ladder, anchor-local poses, orientation variants.
        Candidate,
        /// A contact slide's geometric drop ladder.
        SlideLadder,
        /// A contact slide's bisection refinement.
        SlideBisect,
        /// The short-side-first constructor in `general_fast` — the mode-0
        /// stream's own constructor, which shares the exact tier but not this
        /// operator.
        ShortSideFirst,
        /// Anything else that reached the exact tier.
        Other,
    }

    impl Site {
        const COUNT: usize = 5;

        const fn index(self) -> usize {
            match self {
                Site::Candidate => 0,
                Site::SlideLadder => 1,
                Site::SlideBisect => 2,
                Site::ShortSideFirst => 3,
                Site::Other => 4,
            }
        }

        const fn name(index: usize) -> &'static str {
            match index {
                0 => "candidate",
                1 => "slideLadder",
                2 => "slideBisect",
                3 => "shortSideFirst",
                _ => "other",
            }
        }
    }

    thread_local! {
        static CURRENT: Cell<Site> = const { Cell::new(Site::Other) };
    }

    /// Sets the current site for the lifetime of the returned guard.
    pub fn site(site: Site) -> SiteGuard {
        let previous = CURRENT.with(|cell| cell.replace(site));
        SiteGuard { previous }
    }

    /// Restores the enclosing site.
    pub struct SiteGuard {
        previous: Site,
    }

    impl Drop for SiteGuard {
        fn drop(&mut self) {
            let previous = self.previous;
            CURRENT.with(|cell| cell.set(previous));
        }
    }

    fn current() -> usize {
        CURRENT.with(Cell::get).index()
    }

    macro_rules! counters {
        ($($field:ident),* $(,)?) => {
            #[derive(Default)]
            struct Counters {
                $($field: [AtomicU64; Site::COUNT],)*
            }
            impl Counters {
                fn rows(&self) -> Vec<(&'static str, &[AtomicU64; Site::COUNT])> {
                    vec![$((stringify!($field), &self.$field),)*]
                }
            }
        };
    }

    counters! {
        // Confirmation rows: one candidate pose, built and checked.
        rows,
        rows_rejected_by_containment,
        rows_rejected_by_overlap,
        rows_accepted,
        // Pair questions inside those rows.
        pairs_offered,
        pairs_rejected_by_aabb,
        pairs_reaching_clipper,
        pairs_clipper_overlapping,
        // The prefilter ladder, over the pairs Clipper answered "no overlap".
        clean_separated_by_slabs,
        clean_separated_by_hull,
        // Soundness: a conservative test must never separate an overlapping
        // pair. Every one of these is a falsification.
        soundness_violations_slabs,
        soundness_violations_hull,
        // Collision-polygon builds, and how many were spent on a pose that the
        // row then rejected.
        collision_builds,
        collision_builds_wasted,
        // Input size, for cost context: Clipper's cost is superlinear in these.
        clipper_input_vertices,
    }

    static COUNTERS: std::sync::LazyLock<Counters> = std::sync::LazyLock::new(Counters::default);

    #[inline]
    fn bump(counter: &[AtomicU64; Site::COUNT], amount: u64) {
        counter[current()].fetch_add(amount, Ordering::Relaxed);
    }

    /// Records one confirmation row that built a collision polygon.
    pub fn row_started() {
        bump(&COUNTERS.rows, 1);
        bump(&COUNTERS.collision_builds, 1);
    }

    /// Records a row rejected before any pair question was asked.
    pub fn row_rejected_by_containment() {
        bump(&COUNTERS.rows_rejected_by_containment, 1);
        bump(&COUNTERS.collision_builds_wasted, 1);
    }

    /// Records a row rejected by an exact pair overlap.
    pub fn row_rejected_by_overlap() {
        bump(&COUNTERS.rows_rejected_by_overlap, 1);
        bump(&COUNTERS.collision_builds_wasted, 1);
    }

    /// Records a row whose pose was accepted.
    pub fn row_accepted() {
        bump(&COUNTERS.rows_accepted, 1);
    }

    /// Records a collision-polygon build outside a confirmation row.
    pub fn build_recorded() {
        bump(&COUNTERS.collision_builds, 1);
    }

    /// Records that the candidate a build was made for was then rejected.
    pub fn build_wasted() {
        bump(&COUNTERS.collision_builds_wasted, 1);
    }

    /// Records one offered pair question and prices the prefilter ladder on it.
    ///
    /// `reached_clipper` says whether the caller's own broad phase let the pair
    /// through; `overlapping` is the exact verdict, and is only meaningful when
    /// it did.
    pub fn pair(first: &PolygonSet, second: &PolygonSet, reached_clipper: bool, overlapping: bool) {
        bump(&COUNTERS.pairs_offered, 1);
        if !reached_clipper {
            bump(&COUNTERS.pairs_rejected_by_aabb, 1);
            return;
        }
        bump(&COUNTERS.pairs_reaching_clipper, 1);
        bump(
            &COUNTERS.clipper_input_vertices,
            first.vertex_count().saturating_add(second.vertex_count()) as u64,
        );
        let slabs = match (first.grid_slabs(), second.grid_slabs()) {
            (Some(first), Some(second)) => first.separated(&second),
            _ => false,
        };
        let hull = hulls_separated(first, second);
        if overlapping {
            bump(&COUNTERS.pairs_clipper_overlapping, 1);
            if slabs {
                bump(&COUNTERS.soundness_violations_slabs, 1);
            }
            if hull {
                bump(&COUNTERS.soundness_violations_hull, 1);
            }
            return;
        }
        if slabs {
            bump(&COUNTERS.clean_separated_by_slabs, 1);
        }
        if hull {
            bump(&COUNTERS.clean_separated_by_hull, 1);
        }
    }

    /// Whether the two sets' convex hulls are separated, in exact grid
    /// arithmetic.
    ///
    /// Separating-axis over both hulls' edge normals. A convex hull contains its
    /// set, so a separation of the hulls is a separation of the sets; the
    /// projections are integer-valued `f64` cross products of grid coordinates,
    /// which is exact for any sheet a request can describe.
    fn hulls_separated(first: &PolygonSet, second: &PolygonSet) -> bool {
        let first = convex_hull(first.grid_points());
        let second = convex_hull(second.grid_points());
        if first.len() < 2 || second.len() < 2 {
            return false;
        }
        axis_separates(&first, &second) || axis_separates(&second, &first)
    }

    fn axis_separates(edges_of: &[(f64, f64)], other: &[(f64, f64)]) -> bool {
        for index in 0..edges_of.len() {
            let (x0, y0) = edges_of[index];
            let (x1, y1) = edges_of[(index + 1) % edges_of.len()];
            // Outward normal of a counter-clockwise hull edge.
            let (normal_x, normal_y) = (y1 - y0, x0 - x1);
            let own = edges_of
                .iter()
                .map(|(x, y)| normal_x * x + normal_y * y)
                .fold(f64::NEG_INFINITY, f64::max);
            let theirs = other
                .iter()
                .map(|(x, y)| normal_x * x + normal_y * y)
                .fold(f64::INFINITY, f64::min);
            if own <= theirs {
                return true;
            }
        }
        false
    }

    /// Monotone-chain convex hull, counter-clockwise, on grid coordinates.
    fn convex_hull(mut points: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
        points.sort_by(|first, second| {
            first
                .0
                .total_cmp(&second.0)
                .then_with(|| first.1.total_cmp(&second.1))
        });
        points.dedup();
        if points.len() < 3 {
            return points;
        }
        let cross = |origin: (f64, f64), first: (f64, f64), second: (f64, f64)| {
            (first.0 - origin.0) * (second.1 - origin.1)
                - (first.1 - origin.1) * (second.0 - origin.0)
        };
        let mut hull: Vec<(f64, f64)> = Vec::with_capacity(points.len() + 1);
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
        hull
    }

    /// The census so far, as a JSON value.
    pub fn snapshot() -> serde_json::Value {
        let mut sites = serde_json::Map::new();
        let mut totals = serde_json::Map::new();
        for index in 0..Site::COUNT {
            let mut row = serde_json::Map::new();
            for (name, counter) in COUNTERS.rows() {
                let value = counter[index].load(Ordering::Relaxed);
                row.insert(camel(name), serde_json::json!(value));
            }
            sites.insert(Site::name(index).to_owned(), serde_json::Value::Object(row));
        }
        for (name, counter) in COUNTERS.rows() {
            let value: u64 = counter.iter().map(|c| c.load(Ordering::Relaxed)).sum();
            totals.insert(camel(name), serde_json::json!(value));
        }
        serde_json::json!({
            "totals": totals,
            "bySite": sites,
        })
    }

    fn camel(name: &str) -> String {
        let mut out = String::with_capacity(name.len());
        let mut upper = false;
        for character in name.chars() {
            if character == '_' {
                upper = true;
            } else if upper {
                out.extend(character.to_uppercase());
                upper = false;
            } else {
                out.push(character);
            }
        }
        out
    }
}

#[cfg(feature = "constructor-census")]
pub use armed::{
    build_recorded, build_wasted, pair, row_accepted, row_rejected_by_containment,
    row_rejected_by_overlap, row_started, site, snapshot, Site, SiteGuard,
};

/// Whether the census sites are compiled into this build.
pub const COMPILED_IN: bool = cfg!(feature = "constructor-census");
