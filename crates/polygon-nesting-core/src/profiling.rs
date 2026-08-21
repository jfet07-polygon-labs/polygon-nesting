//! Fixed-stream search profiling: phase spans, phase counters, and optional
//! allocator measurements.
//!
//! This module exists to answer one question with evidence rather than
//! intuition: *where do the milliseconds go* in a fixed benchmark stream.
//! Everything here is diagnostic. Nothing in this module may ever be read by
//! the optimizer, enter a score, seed an RNG, or otherwise reach a search
//! decision — the recorded values are wall-clock and allocation quantities,
//! which are not reproducible, so any search that consulted them would stop
//! being deterministic.
//!
//! # Design constraints
//!
//! * **Off by default, and cheap when off.** Every recording entry point is
//!   guarded by one relaxed load of [`ENABLED`]. When profiling is disabled no
//!   clock is read, no thread-local is touched, and no counter is written. The
//!   roadmap gate for this stage is that the diagnostics-off overhead stays
//!   below 2% of the fixed-stream wall time. One module — the deep-operator
//!   constructor — could not meet that with a runtime branch and is compiled
//!   out instead; see [`deep`] for the measurement that forced it.
//! * **No hot atomics across threads.** Each thread owns its own counter
//!   block; the blocks are only summed when a snapshot is taken, which happens
//!   once per process at a barrier. The per-block counters are `AtomicU64`
//!   purely so the block can be shared with the aggregator, never because two
//!   threads contend on one.
//! * **Deterministic reporting order.** A snapshot is emitted in phase/counter
//!   declaration order, not thread-registration or completion order, so two
//!   runs of the same stream produce the same field sequence.
//!
//! # Semantics of the recorded quantities
//!
//! Phase spans are **inclusive**: a span that encloses another span counts the
//! inner span's time too. The declared phases are chosen so that the leaf
//! phases (the geometry kernels and the per-candidate scorers) partition the
//! interesting work, and the few enclosing phases are marked as such in
//! [`Phase::is_enclosing`]. A cost table should be read off the leaf phases;
//! the enclosing phases exist to show how much of a run is inside the search
//! at all.
//!
//! The counters carry the "precise candidate/move semantics" the roadmap asks
//! for. They are defined once, here, so that a throughput number quoted from
//! this crate can be compared with an external one:
//!
//! * [`Counter::CandidateQueries`] — one per candidate *pose* handed to a
//!   scorer, whether or not it survives pruning. This is the unit that
//!   corresponds to "evaluation" in external strip-packing literature.
//! * [`Counter::NeighborTests`] — one per ordered (candidate, fixed piece)
//!   collision question asked of the proxy. Several of these usually happen
//!   per candidate query.
//! * [`Counter::EffectivePieceMoves`] — one per piece pose that actually
//!   changes in a search state. This is the unit Sparrow-class "moves/s"
//!   numbers use, and it is deliberately *not* the same as a candidate query.
//! * [`Counter::AcceptedMoves`] — one per sweep decision that installs a new
//!   incumbent placement. Every accepted move is an effective piece move; the
//!   converse does not hold, because construction and repair also move pieces.
//! * [`Counter::FullRescores`] — one per whole-layout rescore. A search that
//!   scales will drive this to zero per accepted move.
//! * [`Counter::PublicationAttempts`] — one per invocation of the exact
//!   publication path (contract validation plus the independent validator).
//! * [`Counter::ExactPairTests`] — one per exact Clipper overlap query that
//!   actually reached Clipper. Pairs rejected by the bounds prefilter in front
//!   of it are deliberately *not* counted: they are broad-phase work, they
//!   outnumber the real queries by two orders of magnitude, and instrumenting
//!   them costs more than they take.
//! * [`Counter::CollisionPolygonBuilds`] — one per transformed-and-offset
//!   collision polygon materialised from a source ring.
//!
//! # Allocator measurements
//!
//! [`CountingAllocator`] is a `GlobalAlloc` wrapper that tallies allocation
//! count and bytes while profiling is enabled. It is *not* installed by this
//! crate: a binary that wants heap numbers installs it itself, which keeps the
//! library's own allocation behaviour untouched for every other consumer. The
//! benchmark example installs it under the `profiling-allocator` feature.
//!
//! Its two counters are the one exception to the per-thread rule above: they
//! live in process-global atomics rather than in a thread block. A global
//! allocator may not touch a lazily initialised thread-local, because the
//! initialisation itself allocates — the first counted allocation would call
//! back into the allocator, which would try to initialise the same
//! thread-local again, and the thread would either recurse until its stack
//! overflows or deadlock on the block registry. Global relaxed adds have no
//! such re-entrancy, and paying for their contention is acceptable in exactly
//! this build: heap numbers are collected in a separate, deliberately slower
//! run whose wall time is not the measurement.

use std::alloc::{GlobalAlloc, Layout};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

/// A timed span of search work.
///
/// Variants are declared in reporting order. Adding one is a diagnostics-only
/// change; the numeric discriminants are not persisted anywhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(usize)]
pub enum Phase {
    // ---- constructor (general_fast) ----
    /// Generating candidate translations for one oriented piece.
    ConstructorProposals,
    /// Transforming a source ring and offsetting it into a collision polygon.
    CollisionPolygonBuild,
    /// Testing a collision polygon against the sheet rectangle.
    SheetFitTest,
    /// One exact Clipper overlap query between two collision polygons.
    ExactOverlapTest,
    /// Rebuilding and re-testing the winning candidate before publication.
    PublicationConfirm,
    /// Scoring a confirmed constructor candidate.
    ConstructorScore,

    // ---- relaxed search (general_relaxed) ----
    /// Scoring one candidate pose against the current layout.
    ScorePlacement,
    /// The part of [`Phase::ScorePlacement`] before the neighbour scan:
    /// resolving the candidate's oriented shape and querying the broad-phase
    /// index for the pieces whose bounds it overlaps. Recorded only under
    /// `relaxed-lane-census`.
    ScoreProbe,
    /// The neighbour scan of [`Phase::ScorePlacement`]: encloses the catalogue
    /// descents and the pair rows. Recorded only under `relaxed-lane-census`.
    ScoreScan,
    /// The part of [`Phase::ScorePlacement`] after the neighbour scan: sorting
    /// the moved piece's rows into pair order and returning the delta.
    /// Recorded only under `relaxed-lane-census`.
    ScoreFinalize,
    /// The boundary-overflow penalty of one pose.
    BoundaryPenalty,
    /// One proxy collision question between two poses.
    PairCollide,
    /// Quantifying an already-reported collision into a pressure value.
    PairPressure,
    /// Whole-layout rescore.
    FullRescore,
    /// Rebuilding the broad-phase piece index.
    PieceIndexBuild,
    /// One move sweep over the active pieces.
    MoveSweep,
    /// Installing an accepted move into the incumbent score.
    UpdateAfterMove,
    /// The coupled auditor's independent whole-layout score.
    AuditorScore,

    // ---- dynamic hazard adapter (general_hazard) ----
    /// A complete or fail-fast hazard query.
    HazardQuery,
    /// A hazard-derived pressure quantification.
    HazardPressure,
    /// Transformed exploration bounds for one pose.
    HazardPoseBounds,
    /// Installing a pose into the dynamic hazard index.
    HazardCommit,

    // ---- deep operators (general_persistent_vacancy) ----
    /// Proposal generation inside a deep operator.
    VacancyProposals,
    /// Proxy ranking of deep-operator proposals.
    VacancyProxyRank,
    /// Exact finalist rows inside a deep operator.
    VacancyExactRows,

    // ---- publication ----
    /// Contract validation plus the independent validator.
    PublicationValidate,
}

impl Phase {
    /// Number of declared phases.
    pub const COUNT: usize = Phase::PublicationValidate as usize + 1;

    /// Every phase, in reporting order.
    pub const ALL: [Phase; Phase::COUNT] = [
        Phase::ConstructorProposals,
        Phase::CollisionPolygonBuild,
        Phase::SheetFitTest,
        Phase::ExactOverlapTest,
        Phase::PublicationConfirm,
        Phase::ConstructorScore,
        Phase::ScorePlacement,
        Phase::ScoreProbe,
        Phase::ScoreScan,
        Phase::ScoreFinalize,
        Phase::BoundaryPenalty,
        Phase::PairCollide,
        Phase::PairPressure,
        Phase::FullRescore,
        Phase::PieceIndexBuild,
        Phase::MoveSweep,
        Phase::UpdateAfterMove,
        Phase::AuditorScore,
        Phase::HazardQuery,
        Phase::HazardPressure,
        Phase::HazardPoseBounds,
        Phase::HazardCommit,
        Phase::VacancyProposals,
        Phase::VacancyProxyRank,
        Phase::VacancyExactRows,
        Phase::PublicationValidate,
    ];

    /// The stable reporting name of this phase.
    pub const fn name(self) -> &'static str {
        match self {
            Phase::ConstructorProposals => "constructorProposals",
            Phase::CollisionPolygonBuild => "collisionPolygonBuild",
            Phase::SheetFitTest => "sheetFitTest",
            Phase::ExactOverlapTest => "exactOverlapTest",
            Phase::PublicationConfirm => "publicationConfirm",
            Phase::ConstructorScore => "constructorScore",
            Phase::ScorePlacement => "scorePlacement",
            Phase::ScoreProbe => "scoreProbe",
            Phase::ScoreScan => "scoreScan",
            Phase::ScoreFinalize => "scoreFinalize",
            Phase::BoundaryPenalty => "boundaryPenalty",
            Phase::PairCollide => "pairCollide",
            Phase::PairPressure => "pairPressure",
            Phase::FullRescore => "fullRescore",
            Phase::PieceIndexBuild => "pieceIndexBuild",
            Phase::MoveSweep => "moveSweep",
            Phase::UpdateAfterMove => "updateAfterMove",
            Phase::AuditorScore => "auditorScore",
            Phase::HazardQuery => "hazardQuery",
            Phase::HazardPressure => "hazardPressure",
            Phase::HazardPoseBounds => "hazardPoseBounds",
            Phase::HazardCommit => "hazardCommit",
            Phase::VacancyProposals => "vacancyProposals",
            Phase::VacancyProxyRank => "vacancyProxyRank",
            Phase::VacancyExactRows => "vacancyExactRows",
            Phase::PublicationValidate => "publicationValidate",
        }
    }

    /// Whether this phase encloses other declared phases.
    ///
    /// Enclosing phases must be excluded from a "where did the time go" table
    /// or the percentages sum past 100.
    pub const fn is_enclosing(self) -> bool {
        matches!(
            self,
            Phase::PublicationConfirm
                | Phase::ScorePlacement
                | Phase::ScoreScan
                | Phase::FullRescore
                | Phase::MoveSweep
                | Phase::AuditorScore
                | Phase::VacancyProposals
                | Phase::VacancyExactRows
        )
    }
}

/// A counted search event.
///
/// See the module documentation for the exact semantics of each unit; those
/// definitions are the contract that makes a throughput number comparable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(usize)]
pub enum Counter {
    CandidateQueries,
    NeighborTests,
    EffectivePieceMoves,
    AcceptedMoves,
    FullRescores,
    PublicationAttempts,
    ExactPairTests,
    CollisionPolygonBuilds,
    /// Pieces the broad-phase index returned for one candidate scan, summed.
    /// Recorded only under `relaxed-lane-census`.
    ScanNeighborsReturned,
    /// Neighbours the scan actually reached before an upper-bound cutoff,
    /// summed. Recorded only under `relaxed-lane-census`.
    ScanNeighborsVisited,
    /// Scans that stopped early on the caller's upper bound.
    /// Recorded only under `relaxed-lane-census`.
    ScanUpperBoundCutoffs,
    /// Colliding rows a scan emitted, summed.
    /// Recorded only under `relaxed-lane-census`.
    ScanCollisionRows,
    /// Ordered-catalogue descents the lane's scoring path performed.
    /// Recorded only under `relaxed-lane-census`.
    ScanCatalogDescents,
    AllocationCount,
    AllocationBytes,
}

impl Counter {
    /// Number of declared counters.
    pub const COUNT: usize = Counter::AllocationBytes as usize + 1;

    /// Every counter, in reporting order.
    pub const ALL: [Counter; Counter::COUNT] = [
        Counter::CandidateQueries,
        Counter::NeighborTests,
        Counter::EffectivePieceMoves,
        Counter::AcceptedMoves,
        Counter::FullRescores,
        Counter::PublicationAttempts,
        Counter::ExactPairTests,
        Counter::CollisionPolygonBuilds,
        Counter::ScanNeighborsReturned,
        Counter::ScanNeighborsVisited,
        Counter::ScanUpperBoundCutoffs,
        Counter::ScanCollisionRows,
        Counter::ScanCatalogDescents,
        Counter::AllocationCount,
        Counter::AllocationBytes,
    ];

    /// The stable reporting name of this counter.
    pub const fn name(self) -> &'static str {
        match self {
            Counter::CandidateQueries => "candidateQueries",
            Counter::NeighborTests => "neighborTests",
            Counter::EffectivePieceMoves => "effectivePieceMoves",
            Counter::AcceptedMoves => "acceptedMoves",
            Counter::FullRescores => "fullRescores",
            Counter::PublicationAttempts => "publicationAttempts",
            Counter::ExactPairTests => "exactPairTests",
            Counter::CollisionPolygonBuilds => "collisionPolygonBuilds",
            Counter::ScanNeighborsReturned => "scanNeighborsReturned",
            Counter::ScanNeighborsVisited => "scanNeighborsVisited",
            Counter::ScanUpperBoundCutoffs => "scanUpperBoundCutoffs",
            Counter::ScanCollisionRows => "scanCollisionRows",
            Counter::ScanCatalogDescents => "scanCatalogDescents",
            Counter::AllocationCount => "allocationCount",
            Counter::AllocationBytes => "allocationBytes",
        }
    }
}

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Process-global allocator tallies. See the module's allocator section for
/// why these two counters may not live in a thread block.
static ALLOCATION_COUNT: AtomicU64 = AtomicU64::new(0);
static ALLOCATION_BYTES: AtomicU64 = AtomicU64::new(0);

/// Whether profiling is currently recording.
///
/// This is the single guard every recording entry point consults. It is a
/// relaxed load of a process-global flag that is written once, before the
/// measured stream starts.
#[inline(always)]
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Turns recording on or off for the whole process.
///
/// Call this before the measured stream starts. Toggling it mid-stream is
/// allowed but produces a partial profile, not a wrong search: nothing here
/// feeds a search decision.
pub fn set_enabled(value: bool) {
    ENABLED.store(value, Ordering::Relaxed);
}

/// One thread's private counter block.
///
/// The atomics exist so the aggregator can read a block that its owning thread
/// may still be writing; they are never contended, because only the owning
/// thread writes.
#[derive(Debug)]
struct ThreadProfile {
    ordinal: usize,
    nanos: [AtomicU64; Phase::COUNT],
    calls: [AtomicU64; Phase::COUNT],
    counters: [AtomicU64; Counter::COUNT],
}

impl ThreadProfile {
    fn new(ordinal: usize) -> Self {
        Self {
            ordinal,
            nanos: std::array::from_fn(|_| AtomicU64::new(0)),
            calls: std::array::from_fn(|_| AtomicU64::new(0)),
            counters: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }

    #[inline]
    fn add(slot: &AtomicU64, amount: u64) {
        // Relaxed is sufficient: only the owning thread writes this slot, and
        // the aggregate is read after the workers have joined.
        slot.store(
            slot.load(Ordering::Relaxed).wrapping_add(amount),
            Ordering::Relaxed,
        );
    }
}

fn registry() -> &'static Mutex<Vec<Arc<ThreadProfile>>> {
    static REGISTRY: OnceLock<Mutex<Vec<Arc<ThreadProfile>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Vec::new()))
}

thread_local! {
    static LOCAL: Arc<ThreadProfile> = {
        static NEXT_ORDINAL: AtomicUsize = AtomicUsize::new(0);
        let block = Arc::new(ThreadProfile::new(
            NEXT_ORDINAL.fetch_add(1, Ordering::Relaxed),
        ));
        if let Ok(mut blocks) = registry().lock() {
            blocks.push(Arc::clone(&block));
        }
        block
    };
}

/// Adds `amount` to `counter` when profiling is enabled.
#[inline(always)]
pub fn count(counter: Counter, amount: u64) {
    if enabled() {
        record_count(counter, amount);
    }
}

#[inline(never)]
fn record_count(counter: Counter, amount: u64) {
    let _ = LOCAL.try_with(|block| {
        ThreadProfile::add(&block.counters[counter as usize], amount);
    });
}

/// An open phase span. Dropping it records the elapsed time.
#[derive(Debug)]
pub struct PhaseSpan {
    phase: Phase,
    started: Instant,
}

impl Drop for PhaseSpan {
    fn drop(&mut self) {
        record_span(
            self.phase,
            self.started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64,
        );
    }
}

/// Opens a phase span, or returns `None` when profiling is disabled.
///
/// Bind the result to a local: the span closes when that local is dropped.
///
/// ```ignore
/// let _span = profiling::span(Phase::ExactOverlapTest);
/// ```
///
/// Prefer [`start`]/[`finish`] in functions that are called tens of millions
/// of times per stream — see those functions for why.
#[inline(always)]
pub fn span(phase: Phase) -> Option<PhaseSpan> {
    if enabled() {
        Some(PhaseSpan {
            phase,
            started: Instant::now(),
        })
    } else {
        None
    }
}

/// Opens a phase span that has **no destructor**, or returns `None` when
/// profiling is disabled. Close it with [`finish`].
///
/// # Why this exists alongside [`span`]
///
/// [`PhaseSpan`] implements `Drop`, so holding one across a function body
/// gives that body a drop obligation: the compiler must keep the span alive in
/// a stack slot and emit an unwind cleanup path at every `?`. In a coarse
/// function that is free. In the deep-operator geometry leaves — which run
/// tens of millions of times per stream and are threaded with `?` — it was
/// measurably not: arming them through [`span`] cost about 4% of a mode-20
/// stream even with recording switched off, which is over this stage's budget.
///
/// `Option<Instant>` is `Copy` and has no destructor, so this form adds a
/// predictable branch and nothing else. The trade is that a span opened this
/// way is *not* recorded if the function returns early through `?`; every
/// caller here treats that as acceptable because those paths are fatal
/// geometry errors that end the run.
#[inline(always)]
pub fn start(_phase: Phase) -> Option<Instant> {
    if enabled() {
        Some(Instant::now())
    } else {
        None
    }
}

/// Closes a span opened by [`start`].
#[inline(always)]
pub fn finish(phase: Phase, started: Option<Instant>) {
    if let Some(started) = started {
        record_span(
            phase,
            started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64,
        );
    }
}

#[inline(never)]
fn record_span(phase: Phase, elapsed: u64) {
    let _ = LOCAL.try_with(|block| {
        ThreadProfile::add(&block.nanos[phase as usize], elapsed);
        ThreadProfile::add(&block.calls[phase as usize], 1);
    });
}

/// Instrumentation for the deep-operator module, compiled out by default.
///
/// # Why this one module is compiled out rather than branched
///
/// Everywhere else, a disabled recording site costs one predictable branch and
/// measures below 1% of a fixed stream. The deep-operator constructor is the
/// exception, and it was measured, not assumed: arming *any* subset of sites
/// inside `general_persistent_vacancy` — the geometry leaves, their enclosing
/// confirm-row, or neither of those but the proposal generator — cost the same
/// 4.3-4.5% of a mode-20 stream, while removing all of them returned it to
/// -0.14%. A cost that does not scale with the number of armed sites is not
/// per-call work; it is an inlining/layout cliff in a very large, very hot
/// generated function. No placement of a runtime branch avoids it.
///
/// So the deep operators are instrumented behind `search-profiling`. Without
/// the feature these entry points are literally empty and take a `()` token,
/// so nothing survives into the generated code and the default build is the
/// one the gate measures. With the feature the full phase breakdown is
/// available, at a known and stated ~4.5% uniform distortion that does not
/// reorder the cost centres it reports.
pub mod deep {
    use super::{Counter, Phase};

    /// The token returned by [`start`]; carries no data when compiled out.
    #[cfg(feature = "search-profiling")]
    pub type DeepSpan = Option<std::time::Instant>;

    /// The token returned by [`start`]; carries no data when compiled out.
    #[cfg(not(feature = "search-profiling"))]
    pub type DeepSpan = ();

    /// Opens a deep-operator span.
    #[cfg(feature = "search-profiling")]
    #[inline(always)]
    pub fn start(phase: Phase) -> DeepSpan {
        super::start(phase)
    }

    /// Opens a deep-operator span. Compiled out.
    #[cfg(not(feature = "search-profiling"))]
    #[inline(always)]
    pub fn start(_phase: Phase) -> DeepSpan {}

    /// Closes a deep-operator span.
    #[cfg(feature = "search-profiling")]
    #[inline(always)]
    pub fn finish(phase: Phase, span: DeepSpan) {
        super::finish(phase, span);
    }

    /// Closes a deep-operator span. Compiled out.
    #[cfg(not(feature = "search-profiling"))]
    #[inline(always)]
    pub fn finish(_phase: Phase, _span: DeepSpan) {}

    /// Adds to a deep-operator counter.
    #[cfg(feature = "search-profiling")]
    #[inline(always)]
    pub fn count(counter: Counter, amount: u64) {
        super::count(counter, amount);
    }

    /// Adds to a deep-operator counter. Compiled out.
    #[cfg(not(feature = "search-profiling"))]
    #[inline(always)]
    pub fn count(_counter: Counter, _amount: u64) {}

    /// Whether the deep-operator sites are compiled in.
    pub const COMPILED_IN: bool = cfg!(feature = "search-profiling");
}

/// Instrumentation for the relaxed lane's candidate scorer, compiled out by
/// default.
///
/// # Why this is a separate switch from [`deep`]
///
/// The lane's generic `score_placement` runs about four million times per
/// mode-20 stream and its neighbour loop about sixteen million, so this is the
/// one place where the *measurement* is a serious fraction of the thing being
/// measured: three spans per call is six `Instant::now` reads against a call
/// that costs around a microsecond. That is an acceptable price for a
/// decomposition and an unacceptable one for a wall-clock claim, so the sites
/// are compiled out unless `relaxed-lane-census` is on, exactly like the deep
/// operators, and the census build is never quoted as a time.
///
/// The counters are the durable half of what this feature produces: they are
/// exact call structure rather than sampled time, they cost one relaxed add on
/// a thread-local block, and they are what sizes a lever. The spans only say
/// which third of the function to look in.
pub mod census {
    use super::{Counter, Phase};

    /// The token returned by [`start`]; carries no data when compiled out.
    #[cfg(feature = "relaxed-lane-census")]
    pub type CensusSpan = Option<std::time::Instant>;

    /// The token returned by [`start`]; carries no data when compiled out.
    #[cfg(not(feature = "relaxed-lane-census"))]
    pub type CensusSpan = ();

    /// Opens a census span.
    #[cfg(feature = "relaxed-lane-census")]
    #[inline(always)]
    pub fn start(phase: Phase) -> CensusSpan {
        super::start(phase)
    }

    /// Opens a census span. Compiled out.
    #[cfg(not(feature = "relaxed-lane-census"))]
    #[inline(always)]
    pub fn start(_phase: Phase) -> CensusSpan {}

    /// Closes a census span.
    #[cfg(feature = "relaxed-lane-census")]
    #[inline(always)]
    pub fn finish(phase: Phase, span: CensusSpan) {
        super::finish(phase, span);
    }

    /// Closes a census span. Compiled out.
    #[cfg(not(feature = "relaxed-lane-census"))]
    #[inline(always)]
    pub fn finish(_phase: Phase, _span: CensusSpan) {}

    /// Adds to a census counter.
    #[cfg(feature = "relaxed-lane-census")]
    #[inline(always)]
    pub fn count(counter: Counter, amount: u64) {
        super::count(counter, amount);
    }

    /// Adds to a census counter. Compiled out.
    #[cfg(not(feature = "relaxed-lane-census"))]
    #[inline(always)]
    pub fn count(_counter: Counter, _amount: u64) {}

    /// Whether the census sites are compiled in.
    pub const COMPILED_IN: bool = cfg!(feature = "relaxed-lane-census");
}

/// The continuous-rotation operator's wall decomposition, compiled out by
/// default.
///
/// # What this is for, and why it is not [`census`]
///
/// docs/experiments/continuous-rotation/ §3 established the shape of the
/// operator's negative — 0.87 s per mode-34 slice becomes 1.94 s — and could
/// account for only 0.32 s of it, the surrogate builds, which
/// `WorkCounters::rotation_surrogate_build_nanos` times directly. The rest was
/// *reasoned* about rather than measured, and its §6 named two suspects: the
/// double ordered-map descent every off-grid neighbour resolution pays, and
/// the linear scans of a 48-entry deque in `touch_rotation_probe` and
/// `acquire_rotation_key`.
///
/// This module is what settles that. It is deliberately **not** built on the
/// [`census`] machinery, for one reason: `resolve_surrogate` runs tens of
/// millions of times per slice, so a decomposition of it may not carry a
/// clock. What it carries instead is an exact **count per outcome** — which of
/// the two maps answered — and the wall is then obtained by ablation, one fix
/// at a time, at equal work. The two regions that *are* timed here
/// (`ROTATION_PROBE_NANOS`, `ENSURE_STATE_NANOS`) are called about a million
/// and about ten thousand times a run respectively, where an `Instant` pair is
/// a sub-percent distortion and says something a count cannot.
///
/// # Why process-global atomics are acceptable here and nowhere else
///
/// A search decision may never read any of this — nothing does; the counters
/// are write-only until the process prints them. Contended atomics across
/// eight fan-out lanes would be a real cost and are exactly why this feature
/// is never compiled into a binary that carries a wall claim. The wall battery
/// in docs/experiments/rotation-tax/ §4 runs on a build without it.
pub mod rotation_tax {
    #[cfg(feature = "rotation-tax-census")]
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Which slot of the totals block a site adds to.
    ///
    /// Declaration order is reporting order, as everywhere else in this
    /// module, so two runs print the same field sequence.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Tax {
        /// Pose resolutions the surrogate catalogue answered — the cheap case,
        /// one ordered-map descent, and the only case an unarmed lane has.
        ResolveCatalogHit,
        /// Pose resolutions that **missed the catalogue and hit the lane's
        /// overflow map**: two ordered-map descents, one of them a full-depth
        /// failed search. This is the resolution tax of the continuous-rotation
        /// round's §1.2, counted rather than inferred.
        ResolveOverflowHit,
        /// Pose resolutions the overflow map answered on a remembered route,
        /// without descending the catalogue first — the ones the tax fix
        /// actually caught. `ResolveOverflowHit` is then the residue: off-grid
        /// resolutions still paying both descents because no route was
        /// remembered for them.
        ResolveHintedOverflowHit,
        /// Pose resolutions neither map answered. Non-zero means a lane raised
        /// `missing_orientation`.
        ResolveMiss,
        /// Calls into the two deque linear scans (`touch_rotation_probe` and
        /// `acquire_rotation_key`).
        ProbeScanCalls,
        /// Deque entries those calls compared, summed. This is the "tens of
        /// millions of comparisons" of the continuous-rotation round's §6, as a
        /// number.
        ProbeScanSteps,
        /// Wall inside those calls.
        ProbeScanNanos,
        /// Calls into `ensure_state_surrogates`, and the wall inside them. One
        /// per whole-state entry point, each `O(pieces)` ensures.
        EnsureStateCalls,
        EnsureStateNanos,
        /// Calls into `ensure_rotation_surrogate` — every rung the operator
        /// proposed, plus every pose a state pinned.
        EnsureRotationCalls,
        /// Surrogate builds and the wall inside `build_oriented_surrogate`,
        /// **process-wide**.
        ///
        /// `WorkCounters::rotation_surrogate_builds` already counts these, but
        /// it is only ever *reported* on a mode-34 schedule slice, and the
        /// coordinator arms the operator on modes 22 **and** 34
        /// (`portfolio.rs`: `matches!(mode, 22 | 34)`). Every build a mode-22
        /// lane paid for was therefore invisible to
        /// docs/experiments/continuous-rotation/ §4, which is what made "only a
        /// sixth of the slowdown is the surrogate builds" a statement about
        /// mode 34 rather than about the run.
        RotationBuildCalls,
        RotationBuildNanos,
        /// The four stages of one `build_oriented_surrogate`, timed separately,
        /// so that "the builds are the tax" becomes "*this part* of the build
        /// is the tax". `Transform` is `PolygonSet::transformed`, `Offset` is
        /// the Clipper miter offset, `Triangulate` is the ring triangulation,
        /// and `Index` is the pole series, the cell axes and the `CellIndex`
        /// bin masks.
        ///
        /// Transform and offset are split apart rather than timed together
        /// because they are attacked by completely different things: a ring
        /// transform is `O(points)` trigonometry and an offset is a Minkowski
        /// sum on Clipper's integer grid. The split is the whole reason
        /// docs/experiments/rotation-tax/ §5 can name a specific next lever
        /// instead of "make builds cheaper".
        BuildTransformNanos,
        BuildOffsetNanos,
        BuildTriangulateNanos,
        BuildIndexNanos,
        /// Calls into `prepare_continuous_candidate` that were armed.
        PrepareCandidateCalls,
        /// Lookups in the lane's pair-NFP cache, and those that missed. Both
        /// are expected to be **zero** on an armed lane: the operator refuses
        /// the directional pressure model, which is that cache's only consumer.
        /// They are counted so that "the pair-NFP cache is not on this path" is
        /// a measurement rather than a code reading.
        PairNfpLookups,
        PairNfpMisses,
    }

    impl Tax {
        /// Number of declared slots.
        pub const COUNT: usize = Tax::PairNfpMisses as usize + 1;

        /// Every slot, in reporting order.
        pub const ALL: [Tax; Tax::COUNT] = [
            Tax::ResolveCatalogHit,
            Tax::ResolveOverflowHit,
            Tax::ResolveHintedOverflowHit,
            Tax::ResolveMiss,
            Tax::ProbeScanCalls,
            Tax::ProbeScanSteps,
            Tax::ProbeScanNanos,
            Tax::EnsureStateCalls,
            Tax::EnsureStateNanos,
            Tax::EnsureRotationCalls,
            Tax::RotationBuildCalls,
            Tax::RotationBuildNanos,
            Tax::BuildTransformNanos,
            Tax::BuildOffsetNanos,
            Tax::BuildTriangulateNanos,
            Tax::BuildIndexNanos,
            Tax::PrepareCandidateCalls,
            Tax::PairNfpLookups,
            Tax::PairNfpMisses,
        ];

        /// The stable reporting name of this slot.
        pub const fn name(self) -> &'static str {
            match self {
                Tax::ResolveCatalogHit => "resolveCatalogHit",
                Tax::ResolveOverflowHit => "resolveOverflowHit",
                Tax::ResolveHintedOverflowHit => "resolveHintedOverflowHit",
                Tax::ResolveMiss => "resolveMiss",
                Tax::ProbeScanCalls => "probeScanCalls",
                Tax::ProbeScanSteps => "probeScanSteps",
                Tax::ProbeScanNanos => "probeScanNanos",
                Tax::EnsureStateCalls => "ensureStateCalls",
                Tax::EnsureStateNanos => "ensureStateNanos",
                Tax::EnsureRotationCalls => "ensureRotationCalls",
                Tax::RotationBuildCalls => "rotationBuildCalls",
                Tax::RotationBuildNanos => "rotationBuildNanos",
                Tax::BuildTransformNanos => "buildTransformNanos",
                Tax::BuildOffsetNanos => "buildOffsetNanos",
                Tax::BuildTriangulateNanos => "buildTriangulateNanos",
                Tax::BuildIndexNanos => "buildIndexNanos",
                Tax::PrepareCandidateCalls => "prepareCandidateCalls",
                Tax::PairNfpLookups => "pairNfpLookups",
                Tax::PairNfpMisses => "pairNfpMisses",
            }
        }
    }

    /// One thread's private block, registered so the aggregator can read it
    /// without the owning thread having exited.
    ///
    /// **Per-thread rather than one process-global array, and the difference is
    /// not a detail.** The first version of this instrument used a single
    /// `[AtomicU64]` and `fetch_add`, and the eight fan-out lanes turned
    /// `ResolveCatalogHit` — 160 million calls in a ten-second run — into 160
    /// million contended read-modify-writes on one cache line. That build was
    /// slow enough that the coordinator never reached a mode-34 slice at all,
    /// so the instrument destroyed the phase structure it was there to
    /// describe. A thread-owned line written with relaxed loads and stores has
    /// no coherence traffic and costs about a nanosecond.
    #[cfg(feature = "rotation-tax-census")]
    struct TaxBlock {
        slots: [AtomicU64; Tax::COUNT],
    }

    #[cfg(feature = "rotation-tax-census")]
    fn registry() -> &'static std::sync::Mutex<Vec<std::sync::Arc<TaxBlock>>> {
        static REGISTRY: std::sync::OnceLock<std::sync::Mutex<Vec<std::sync::Arc<TaxBlock>>>> =
            std::sync::OnceLock::new();
        REGISTRY.get_or_init(|| std::sync::Mutex::new(Vec::new()))
    }

    #[cfg(feature = "rotation-tax-census")]
    thread_local! {
        static LOCAL: std::sync::Arc<TaxBlock> = {
            let block = std::sync::Arc::new(TaxBlock {
                slots: std::array::from_fn(|_| AtomicU64::new(0)),
            });
            if let Ok(mut blocks) = registry().lock() {
                blocks.push(std::sync::Arc::clone(&block));
            }
            block
        };
    }

    /// Adds `amount` to one slot of the calling thread's block.
    #[cfg(feature = "rotation-tax-census")]
    #[inline(always)]
    pub fn add(slot: Tax, amount: u64) {
        let _ = LOCAL.try_with(|block| {
            let cell = &block.slots[slot as usize];
            // Relaxed load-then-store rather than `fetch_add`: only the owning
            // thread writes this slot and the aggregate is read after the
            // workers are idle, so no read-modify-write is needed.
            cell.store(
                cell.load(Ordering::Relaxed).wrapping_add(amount),
                Ordering::Relaxed,
            );
        });
    }

    /// Adds `amount` to one slot. Compiled out.
    #[cfg(not(feature = "rotation-tax-census"))]
    #[inline(always)]
    pub fn add(_slot: Tax, _amount: u64) {}

    /// Every slot's total across every thread that recorded anything, in
    /// [`Tax::ALL`] order.
    pub fn totals() -> [u64; Tax::COUNT] {
        #[cfg(feature = "rotation-tax-census")]
        {
            let mut out = [0u64; Tax::COUNT];
            if let Ok(blocks) = registry().lock() {
                for block in blocks.iter() {
                    for (slot, total) in out.iter_mut().enumerate() {
                        *total = total.wrapping_add(block.slots[slot].load(Ordering::Relaxed));
                    }
                }
            }
            out
        }
        #[cfg(not(feature = "rotation-tax-census"))]
        {
            [0; Tax::COUNT]
        }
    }

    /// Whether the decomposition sites are compiled in.
    pub const COMPILED_IN: bool = cfg!(feature = "rotation-tax-census");
}

/// One phase's aggregated sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhaseSample {
    pub phase: Phase,
    pub nanos: u64,
    pub calls: u64,
}

/// One counter's aggregated sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CounterSample {
    pub counter: Counter,
    pub value: u64,
}

/// An aggregated profile across every thread that recorded anything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileSnapshot {
    pub phases: Vec<PhaseSample>,
    pub counters: Vec<CounterSample>,
    pub threads: usize,
}

impl ProfileSnapshot {
    /// Total nanoseconds attributed to leaf (non-enclosing) phases.
    ///
    /// This is the denominator a share-of-time table should use.
    pub fn leaf_nanos(&self) -> u64 {
        self.phases
            .iter()
            .filter(|sample| !sample.phase.is_enclosing())
            .map(|sample| sample.nanos)
            .sum()
    }

    /// Looks up one counter's aggregated value.
    pub fn counter(&self, counter: Counter) -> u64 {
        self.counters
            .iter()
            .find(|sample| sample.counter == counter)
            .map_or(0, |sample| sample.value)
    }
}

/// Clears every registered thread block.
///
/// Blocks belonging to threads that have already exited stay registered and
/// are cleared too, so a reset is a true zero for the next stream.
pub fn reset() {
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    ALLOCATION_BYTES.store(0, Ordering::Relaxed);
    let Ok(blocks) = registry().lock() else {
        return;
    };
    for block in blocks.iter() {
        for slot in block.nanos.iter().chain(block.calls.iter()) {
            slot.store(0, Ordering::Relaxed);
        }
        for slot in block.counters.iter() {
            slot.store(0, Ordering::Relaxed);
        }
    }
}

/// Aggregates every thread block into one snapshot.
///
/// Thread blocks are summed in registration-ordinal order so that the
/// floating-point-free integer sums are identical regardless of which thread
/// finished first.
pub fn snapshot() -> ProfileSnapshot {
    let Ok(blocks) = registry().lock() else {
        return ProfileSnapshot {
            phases: Vec::new(),
            counters: Vec::new(),
            threads: 0,
        };
    };
    let mut ordered = blocks.iter().map(Arc::clone).collect::<Vec<_>>();
    ordered.sort_by_key(|block| block.ordinal);
    let mut phases = Vec::with_capacity(Phase::COUNT);
    for phase in Phase::ALL {
        let index = phase as usize;
        phases.push(PhaseSample {
            phase,
            nanos: ordered
                .iter()
                .map(|block| block.nanos[index].load(Ordering::Relaxed))
                .sum(),
            calls: ordered
                .iter()
                .map(|block| block.calls[index].load(Ordering::Relaxed))
                .sum(),
        });
    }
    let mut counters = Vec::with_capacity(Counter::COUNT);
    for counter in Counter::ALL {
        let index = counter as usize;
        // The two allocator counters are tallied process-globally rather than
        // per thread; the per-thread slots stay zero and the global is added
        // in, so a caller reads one value per counter either way.
        let global = match counter {
            Counter::AllocationCount => ALLOCATION_COUNT.load(Ordering::Relaxed),
            Counter::AllocationBytes => ALLOCATION_BYTES.load(Ordering::Relaxed),
            _ => 0,
        };
        counters.push(CounterSample {
            counter,
            value: ordered
                .iter()
                .map(|block| block.counters[index].load(Ordering::Relaxed))
                .sum::<u64>()
                .wrapping_add(global),
        });
    }
    let threads = ordered
        .iter()
        .filter(|block| {
            block
                .nanos
                .iter()
                .chain(block.calls.iter())
                .chain(block.counters.iter())
                .any(|slot| slot.load(Ordering::Relaxed) != 0)
        })
        .count();
    ProfileSnapshot {
        phases,
        counters,
        threads,
    }
}

/// Every counter's current total, without allocating.
///
/// [`snapshot`] is the reporting path: it builds the phase table too, and it
/// allocates two `Vec`s to do so. This is the *sampling* path, for a caller
/// that wants the work ordinals at a moment inside a run rather than the whole
/// profile at the end of one — the quality frontier trace reads it once per
/// event. Blocks are summed in registration-ordinal order for the same reason
/// [`snapshot`] sorts them: integer sums of the same set are order-independent,
/// but reading them in a stable order keeps the two functions comparable.
pub fn counter_totals() -> [u64; Counter::COUNT] {
    let mut totals = [0u64; Counter::COUNT];
    totals[Counter::AllocationCount as usize] = ALLOCATION_COUNT.load(Ordering::Relaxed);
    totals[Counter::AllocationBytes as usize] = ALLOCATION_BYTES.load(Ordering::Relaxed);
    let Ok(blocks) = registry().lock() else {
        return totals;
    };
    let mut ordered = blocks.iter().map(Arc::clone).collect::<Vec<_>>();
    drop(blocks);
    ordered.sort_by_key(|block| block.ordinal);
    for block in &ordered {
        for (index, total) in totals.iter_mut().enumerate() {
            *total = total.wrapping_add(block.counters[index].load(Ordering::Relaxed));
        }
    }
    totals
}

/// A `GlobalAlloc` wrapper that tallies allocations while profiling is on.
///
/// Install it from a binary that wants heap numbers:
///
/// ```ignore
/// #[global_allocator]
/// static ALLOCATOR: CountingAllocator<std::alloc::System> =
///     CountingAllocator::new(std::alloc::System);
/// ```
///
/// The wrapper adds one relaxed load per allocation when profiling is off. It
/// is deliberately not installed by this library, so no other consumer pays
/// even that.
///
/// What it reports is *gross* demand, not residency: `dealloc` is not
/// subtracted, and a `realloc` is counted as a fresh request for the whole new
/// size. That is the quantity the roadmap's hot-loop work needs - "how many
/// times did this stream ask the allocator, and for how much" - and it is the
/// only one obtainable without a size-tracking side table inside the allocator.
#[derive(Debug)]
pub struct CountingAllocator<A> {
    inner: A,
}

impl<A> CountingAllocator<A> {
    /// Wraps `inner`.
    pub const fn new(inner: A) -> Self {
        Self { inner }
    }
}

/// Records one allocation request of `bytes`.
///
/// This deliberately touches nothing but two global atomics: it runs *inside*
/// the global allocator, so anything it did that allocated - initialising a
/// thread-local block, locking and pushing to the block registry - would
/// re-enter this function and either recurse until the stack overflows or
/// deadlock against a lock the same thread already holds.
#[inline(always)]
fn record_allocation(bytes: u64) {
    if enabled() {
        ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
        ALLOCATION_BYTES.fetch_add(bytes, Ordering::Relaxed);
    }
}

// SAFETY: every method forwards to the wrapped allocator unchanged; the only
// added work is incrementing counters that no allocation decision reads, and
// that counting is itself allocation-free (see `record_allocation`).
unsafe impl<A: GlobalAlloc> GlobalAlloc for CountingAllocator<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation(layout.size() as u64);
        self.inner.alloc(layout)
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        self.inner.dealloc(pointer, layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_allocation(layout.size() as u64);
        self.inner.alloc_zeroed(layout)
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record_allocation(new_size as u64);
        self.inner.realloc(pointer, layout, new_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_and_counter_tables_are_complete_and_ordered() {
        for (index, phase) in Phase::ALL.iter().enumerate() {
            assert_eq!(*phase as usize, index, "phase {phase:?} is out of order");
        }
        for (index, counter) in Counter::ALL.iter().enumerate() {
            assert_eq!(
                *counter as usize, index,
                "counter {counter:?} is out of order"
            );
        }
    }

    #[test]
    fn phase_and_counter_names_are_unique() {
        let mut phase_names = Phase::ALL.map(Phase::name).to_vec();
        phase_names.sort_unstable();
        let unique = phase_names.len();
        phase_names.dedup();
        assert_eq!(phase_names.len(), unique);
        let mut counter_names = Counter::ALL.map(Counter::name).to_vec();
        counter_names.sort_unstable();
        let unique = counter_names.len();
        counter_names.dedup();
        assert_eq!(counter_names.len(), unique);
    }

    #[test]
    fn recording_is_inert_while_disabled() {
        // The shared flag makes this test order-dependent with any test that
        // enables profiling, so this file keeps exactly one enabling test and
        // runs it in the same test function as its own reset.
        set_enabled(false);
        reset();
        count(Counter::CandidateQueries, 7);
        assert!(span(Phase::ScorePlacement).is_none());
        let snapshot = snapshot();
        assert_eq!(snapshot.counter(Counter::CandidateQueries), 0);
        assert_eq!(snapshot.leaf_nanos(), 0);
    }

    #[test]
    fn enabled_recording_accumulates_and_resets() {
        set_enabled(true);
        reset();
        count(Counter::NeighborTests, 3);
        count(Counter::NeighborTests, 4);
        {
            let _span = span(Phase::ExactOverlapTest);
        }
        let recorded = snapshot();
        set_enabled(false);
        assert_eq!(recorded.counter(Counter::NeighborTests), 7);
        let sample = recorded
            .phases
            .iter()
            .find(|sample| sample.phase == Phase::ExactOverlapTest)
            .copied()
            .expect("every phase is reported");
        assert_eq!(sample.calls, 1);
        reset();
        assert_eq!(snapshot().counter(Counter::NeighborTests), 0);

        // Same enabling window, because the recording flag is process-global
        // and this file keeps exactly one test that turns it on.
        allocator_tallies_gross_demand_without_re_entering_itself();
    }

    /// The counting allocator must report through global atomics only.
    ///
    /// Routing its two counters through the per-thread block is what a first
    /// cut did, and it aborted the process: the first counted allocation
    /// initialised the thread-local block, which allocated, which counted,
    /// which initialised the block. Exercising the wrapper here would not by
    /// itself reproduce that - it is not the installed global allocator in a
    /// test binary - so what this pins is the property that made the fix work:
    /// the tally is visible in a snapshot without any thread block recording
    /// anything at all.
    fn allocator_tallies_gross_demand_without_re_entering_itself() {
        let allocator = CountingAllocator::new(std::alloc::System);
        let layout = Layout::from_size_align(64, 8).expect("a valid test layout");
        set_enabled(true);
        reset();
        // SAFETY: the layout is non-zero-sized and well-formed, the pointer is
        // freed exactly once with the layout it was allocated with, and the
        // block is never read.
        unsafe {
            let pointer = allocator.alloc(layout);
            assert!(!pointer.is_null(), "the system allocator served 64 bytes");
            allocator.dealloc(pointer, layout);
        }
        let recorded = snapshot();
        set_enabled(false);
        assert_eq!(recorded.counter(Counter::AllocationCount), 1);
        assert_eq!(recorded.counter(Counter::AllocationBytes), 64);

        // Disabled means inert here too, and `reset` clears the globals.
        reset();
        // SAFETY: as above.
        unsafe {
            let pointer = allocator.alloc(layout);
            assert!(!pointer.is_null(), "the system allocator served 64 bytes");
            allocator.dealloc(pointer, layout);
        }
        assert_eq!(snapshot().counter(Counter::AllocationCount), 0);
        assert_eq!(snapshot().counter(Counter::AllocationBytes), 0);
    }
}
