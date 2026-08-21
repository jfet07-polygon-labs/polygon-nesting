//! The lane-owned compression schedule: the depth clock a relaxed lane runs
//! its sweeps against.
//!
//! This is the port the mode-26 rung anatomy designed from measurement (see
//! `docs/experiments/mode26-rung-anatomy/README.md` §2-3). Mode 26 buys depth
//! by rebuilding a whole clamped-sheet mode-0 pipeline per rung: 32.25M
//! candidate queries and 4.7-13.8 s to move one bound by 0.159 mm, of which
//! 75.5% is thrown away by a rollback comparison the rung inherits from the
//! coupled separator. The anatomy's finding is that the *clamp itself* is
//! already expressible in the proxy tier at zero additional geometry -
//! `boundary_penalty` takes the depth as a parameter at all 11 of its call
//! sites, costs 84.9 ns whether the piece protrudes or not, and the sampling
//! boxes of every candidate generator are derived from the same scalar - so
//! what is missing is not geometry but a *clock*: something that owns the
//! depth, lowers it on a schedule the lane can afford, and knows which of the
//! layouts it has walked through was the last one an exact validator accepted.
//!
//! This module is that clock, and only that clock. It holds no geometry, no
//! placements and no lane state; it answers `depth_mm()`, it is told when a
//! confirmation succeeded or failed, and it decides when to step, when to ask
//! for a confirmation and when the frontier has to be given back. The lane
//! integration - the one write per sweep, the boundary-row refresh, the exact
//! confirmation and the deepest-confirmed slot - lives in
//! [`crate::search::general_relaxed`], which is where the private state it
//! touches lives.
//!
//! # The five invariants
//!
//! 1. **The step is a canonical grid unit.** [`canonical_grid_step_mm`] is
//!    derived from the pose grid the engine snaps every translation to, not
//!    chosen. A step below it is a step the layout cannot express.
//! 2. **The floor is monotone.** [`CompressionSchedule::floor_mm`] - the depth
//!    of the deepest *confirmed* layout - only ever decreases. Nothing in the
//!    lane can restore a looser one, which is the memory `boundary_penalty`
//!    does not have and the reason mode 26 had to rebuild a clamped pipeline
//!    per rung.
//! 3. **The frontier is never looser than the floor.** `depth_mm <= floor_mm`
//!    at every point in the schedule's life, including immediately after a
//!    rollback. This is asserted, not argued.
//! 4. **The frontier may be infeasible; the floor may not.** The floor moves
//!    only on an accepted exact confirmation. That is Sol's incumbent/frontier
//!    asymmetry, kept at the finer grain.
//! 5. **A rollback restores a (layout, depth) pair.** The schedule state is
//!    part of the snapshot, so a moving depth cannot desynchronise a restore.
//!    See the module's `rollback` note below.
//!
//! Compiled only under the `compression-schedule` feature. With the feature
//! off, this module does not exist, `GeneralRelaxedSettings` has no field
//! naming it, and the relaxed lane has neither the schedule slot nor the write.

use serde::Serialize;

use crate::canonical_grid::{from_grid, to_grid_mm};

/// Millimetres to canonical grid units, through the same rounding the pose
/// lattice uses. A value the grid cannot represent falls back to the naive
/// scaling rather than to a panic; the schedule then still runs, on a lattice
/// that is one rounding away from the canonical one, which is a degradation
/// rather than a failure.
fn to_grid(value_mm: f64) -> f64 {
    to_grid_mm(value_mm).unwrap_or(value_mm * 1_000.0)
}

/// One canonical grid unit, in millimetres.
///
/// Every translation the engine accepts is snapped to this lattice by
/// `snap_mm`, which routes through `to_grid_mm`/`from_grid`; the same lattice
/// is what `grid_key` compares depths on. So it is the finest depth change a
/// layout can *express*, and therefore the schedule's step: at a 159 mm parent
/// one mode-26 rung is `parent * 0.001 = 0.159 mm`, which is 159 of these, and
/// spreading that rung over the 739-1,478 sweeps a 0.5-1.0 s slice affords
/// would otherwise ask for 0.11-0.22 µm per sweep - below the grid, and
/// therefore a no-op.
///
/// It is derived rather than written down: `from_grid(1.0)` is one grid unit
/// through the canonical-grid authority itself, so a request whose grid scale
/// ever changes moves this with it.
pub fn canonical_grid_step_mm() -> f64 {
    from_grid(1.0)
}

/// What one exact pair test costs in the portfolio's work currency.
///
/// Deliberately a local copy of `portfolio::WORK_UNITS_PER_EXACT_PAIR_TEST`
/// rather than a `use` of it: that module is compiled only with the deep
/// operators, and the schedule has to be able to price its own confirmations in
/// the same currency in a build that does not carry the coordinator at all. It
/// is the same number and it must stay the same number; if the coordinator's
/// ever moves, this is the second place to change.
pub const WORK_UNITS_PER_EXACT_PAIR_TEST: usize = 5;

/// How the lane should behave when a scheduled confirmation is refused.
///
/// Named rather than a bare `bool` because the two halves are different
/// experiments: the anatomy measured `micro_legalize` at 0.83 ms publishing in
/// **0 of 25** mode-26 arms, but those arms were legalizing the residue of a
/// 0.159 mm bound move. The whole premise of a 1 µm step is that its residue
/// stays inside the micro-legalizer's translation-only model, and that premise
/// is exactly what a run with this on and a run with it off separate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompressionRepairPolicy {
    /// The sweeps are the whole repair. A refused confirmation is recorded and
    /// the frontier carries on.
    SweepsOnly,
    /// A refused confirmation is offered to `micro_legalize` once, and the
    /// layout it returns - which the authoritative validator has already
    /// accepted - is eligible for the deepest-confirmed slot.
    MicroLegalizeOnReject,
}

/// The schedule's knobs, as a caller supplies them.
///
/// Every field is a budget or a cadence. None of them is a quality knob: the
/// numbers a caller should use are the anatomy's measured ones, and the
/// defaults here are those numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompressionScheduleSettings {
    /// Sweeps of repair per depth step.
    ///
    /// The anatomy's slice arithmetic: a 0.5 s slice affords ~739 move sweeps
    /// and a 1.0 s slice ~1,478, against the 159 steps one mode-26 rung of
    /// depth costs - so 4.6 to 9.3 sweeps per step. The default is the middle
    /// of that band.
    pub sweeps_per_step: usize,
    /// How many steps must pass before the lane asks the exact tier whether
    /// the layout it has reached is publishable.
    ///
    /// The mode-26 anatomy budgeted this at 0.491 ms per confirmation and
    /// therefore 2.0% of a 1.0 s slice at every fourth step. **That budget is
    /// wrong by an order of magnitude, and this port measured it.** A
    /// confirmation the validator *accepts* asks all `n * (n - 1) / 2` pair
    /// questions; on the 61-piece fixture that is 1,830 of them at the
    /// anatomy's own 1,904.8 ns, which is 3.49 ms before the collision-polygon
    /// builds, and the measurement here is **4.18-5.65 ms**. The 0.491 ms
    /// figure is the cost of a confirmation that *fails*, which exits at the
    /// first violating pair.
    ///
    /// The cadence is kept at the anatomy's four because the port's other cost
    /// control - refusing to confirm a layout the proxy tier already calls
    /// infeasible - suppresses 96-99% of the confirmations this cadence makes
    /// due, and the achieved cost is then 0.2-11% of the arm rather than the
    /// 20-45% the raw cadence would imply. Both numbers are reported, so a
    /// reader can see that the budget holds for a reason the design did not
    /// name.
    pub confirm_every: usize,
    /// How many steps the frontier may run without an accepted confirmation
    /// before it is given back to the deepest-confirmed layout.
    ///
    /// **`0` - never roll back - is the default, and it is a measurement
    /// rather than a preference.** The mechanism below is correct and its
    /// invariants are pinned by tests, but on twelve matched cells at
    /// 174-179 mm parents, arming it at 32 steps cost a median of 11.75 mm of
    /// published depth: 12 of 12 cells publish without it at a median 12.110 mm
    /// below their parent, and 8 of 12 publish with it at a median 0.359 mm.
    /// The reason is visible in the same evidence - a schedule that gives its
    /// frontier back every 32 steps cannot sustain a descent, and the frontier
    /// is proxy-infeasible for 96-99% of its steps by construction, so the
    /// rollback fires almost every time it can.
    ///
    /// That is the mode-26 anatomy's own finding reproduced one level down: the
    /// rollback was 75.5% of a legacy arm's wall, and here it is 97% of the
    /// port's depth. The knob stays because a future frontier that *can* run
    /// away needs it, and because "we measured it and it was harmful" is a
    /// stronger statement when the thing measured is still there.
    pub rollback_after_steps: usize,
    /// A hard cap on the schedule's spend, in the portfolio's work units.
    ///
    /// `None` means the step budget is the only stop. See
    /// [`CompressionSchedule::work_units`] for how the two halves of that unit
    /// are counted here, and why neither of them is read from the profiling
    /// counters.
    pub work_cap_queries: Option<usize>,
    /// Whether the schedule keeps stepping after it reaches the requested
    /// bound.
    ///
    /// `false` - the default - makes the schedule ask for exactly the drop it
    /// was given, which is what a matched-arm comparison against a mode-26
    /// ladder asking for the same drop requires. `true` turns it into an
    /// anytime operator that spends whatever budget it is given.
    pub continue_past_bound: bool,
    /// What to do with a refused confirmation.
    pub repair_policy: CompressionRepairPolicy,
    /// How far one step lowers the frontier, **in canonical grid units**.
    ///
    /// `1.0` - one grid unit, 1 µm on this request - is the default and is the
    /// module's first invariant: it is the finest depth change a *layout* can
    /// express, because `snap_mm` rounds every translation onto that lattice.
    ///
    /// A value below `1.0` is therefore not a finer layout, it is a finer
    /// *clamp*: `strip_depth_mm` is a proxy-tier scalar that `boundary_penalty`
    /// reads as a continuous number, so a sub-grid frontier is a smaller
    /// increment of pressure per step rather than a smaller move. What it buys
    /// is cadence, not resolution - `confirm_every` counts steps, so a quarter
    /// step at the same `sweeps_per_step` asks the exact tier four times as
    /// often per micron of descent, and spends four times as many repair sweeps
    /// getting there.
    ///
    /// Kept as a knob rather than a default because it is a budget, and because
    /// the invariant above is worth stating in the type: the caller has to ask
    /// for a sub-grid frontier by name.
    pub step_grid: f64,
    /// Whether the lane tries to make its *parent* proxy-feasible, by
    /// translation alone, before it takes step 0.
    ///
    /// The port's §7.1 is the reason this exists: **0 of 12** coordinator
    /// parents at 171-179 mm are feasible in the proxy tier when they arrive,
    /// at 26-38 colliding pairs, because `initialize_complete_state` snaps
    /// every rotation onto the structured surrogate's 2.5-degree grid and the
    /// parents' rotations are continuous. A lane that starts infeasible spends
    /// a fraction of every step repairing the *parent* rather than compressing
    /// it - the slice is part regrid - and the fraction is not reported by any
    /// number the schedule publishes.
    ///
    /// `false` is the default and it keeps every pinned m34 number in this
    /// repository reproducible: the port's twelve cells, the coordinator's
    /// nine-rung slice and the v4 curves were all measured with the lane
    /// entering on whatever `initialize_complete_state` handed it.
    ///
    /// When `true`, the lane runs one
    /// [`crate::search::general_micro_legalization::global_legalize`] on the
    /// snapped parent, bounded at the parent's own depth. That solver is
    /// translation-only - two variables per piece, no rotation - so it cannot
    /// undo the snap; what it can do is redistribute the layout until the
    /// collision envelopes the snap pushed together come apart again. Nothing
    /// is published from it: its output is only ever a starting state for the
    /// schedule, and the schedule still publishes only what its own exact
    /// confirmation accepts.
    pub legalize_entry: bool,
    /// Whether a lane whose parent is *still* proxy-infeasible after
    /// [`Self::legalize_entry`] gives the slice back instead of running it.
    ///
    /// Grok review 1 §2b item 3's rule, and the arithmetic behind it: a slice
    /// that enters infeasible costs 1.1-2.3 seconds of a ten-second budget on
    /// the three requests measured here, and on two of them it has never
    /// published anything at all. Refusing it returns that wall to the classes
    /// that do publish.
    ///
    /// Requires [`Self::legalize_entry`]: skipping on the *unrepaired* entry
    /// verdict would refuse every slice on every request, including the one
    /// where the class publishes on nine of nine.
    pub skip_infeasible_entry: bool,
    /// Whether a lane whose entry is already deeper than the parent by more
    /// than the drop it was asked for gives the slice back instead of running
    /// it.
    ///
    /// This is [`Self::skip_infeasible_entry`]'s rule with the discriminator
    /// the measurement chose rather than the one the review proposed. The lane
    /// publishes only a layout whose *source* depth beats the parent's, and it
    /// may walk only the requested drop of clamp to get there; so a slice whose
    /// entry already sits a drop or more above its parent cannot publish even
    /// if every step and every confirmation goes perfectly. It is the slice's
    /// own arithmetic, decided before step 0 for the price of one depth
    /// measurement, and it carries no request in it.
    ///
    /// Independent of [`Self::legalize_entry`]: it needs no repair, only the
    /// entry's own depth.
    pub skip_unpublishable_entry: bool,
    /// The denominator of the *probe*: the lane abandons the slice once it has
    /// taken `steps_planned / n` steps without having published anything below
    /// its parent. `0` disables it, which is every measurement before this one.
    ///
    /// # Why the price of the first slice is a budget and not a prior
    ///
    /// Grok review 1 §2b item 1 asks for the first slice to be wall-priced,
    /// because the first one has no ratchet and costs 1.1-2.3 s of a
    /// ten-second budget. A prior can only decide *whether* to buy it, and
    /// this round measured that no prior can: the entry numbers that were
    /// expected to discriminate - proxy feasibility, the entry's own depth
    /// loss - are the same on the request where the class publishes on nine of
    /// nine as on the two where it has never published at all.
    ///
    /// What does discriminate is visible *inside* the slice, from its first
    /// steps, and it is what the lane is being paid for: whether anything it
    /// has reached beats the parent yet. Measured over nine ten-second cells,
    /// where a slice publishes it publishes early, and where it does not the
    /// whole slice is repair sweeps against a frontier that never becomes
    /// publishable - **98-100%** of triangle-20's wall and 76-82% of
    /// shapes-17's, against 21-47% of mixed-61's, whose wall is 50-76% accepted
    /// exact confirmations instead.
    ///
    /// So the price of the first slice is charged as an anytime budget: spend a
    /// fraction, and go on only on evidence. It is the same shape as the
    /// coordinator's own audition rule, one level down.
    pub barren_probe_denominator: usize,
    /// How many workers repair one step, under this one clock.
    ///
    /// `1` - the shipped serial schedule - is the default, and everything above
    /// it is the `parallel-compression-schedule` lever. A step whose proxy tier
    /// is infeasible dispatches `lanes` workers, each repairing a private clone
    /// of the frontier at the *same* depth from a seed derived from
    /// `(base, step, worker)`, and a reduce in worker-index order adopts one of
    /// them. No worker owns the clock: a worker never calls
    /// [`CompressionSchedule::step_down`], never confirms and never moves the
    /// floor, so the five invariants above are untouched by the fan-out.
    ///
    /// It is a *budget* knob and not a free one. Every worker's candidate
    /// queries are charged, so `lanes=8` spends about eight times the candidate
    /// half of a repair step - which on the measured mixed-61 band is only 11%
    /// of the schedule's self-charged work, the derived exact tier being the
    /// other 89%, so the honest multiplier on a work-capped arm is well under
    /// eight. See docs/experiments/parallel-compression-schedule/.
    #[cfg(feature = "parallel-compression-schedule")]
    pub lanes: usize,
    /// Whether one whole-layout exact confirmation is spread over the job pool.
    ///
    /// `false` - the shipped serial validator - is the default. The lever
    /// exists because the measurement that motivated the *repair* fan-out found
    /// the confirmation to be the larger half: on the 174-179 mm band at the
    /// design slice the exact tier is 41-77% of the schedule's own wall, and it
    /// is one 61-piece build pass plus 1,830 pair questions executed one at a
    /// time. Parallelising it does not touch the cadence, the floor or the
    /// deepest-confirmed slot; it makes one confirmation cost less wall.
    #[cfg(feature = "parallel-compression-schedule")]
    pub parallel_confirm: bool,
}

impl Default for CompressionScheduleSettings {
    fn default() -> Self {
        Self {
            sweeps_per_step: 6,
            confirm_every: 4,
            rollback_after_steps: 0,
            work_cap_queries: None,
            continue_past_bound: false,
            repair_policy: CompressionRepairPolicy::MicroLegalizeOnReject,
            step_grid: 1.0,
            legalize_entry: false,
            skip_infeasible_entry: false,
            skip_unpublishable_entry: false,
            barren_probe_denominator: 0,
            // Both levers default to the shipped serial behaviour, so an armed
            // *build* that is not asked for them is the shipped schedule.
            #[cfg(feature = "parallel-compression-schedule")]
            lanes: 1,
            #[cfg(feature = "parallel-compression-schedule")]
            parallel_confirm: false,
        }
    }
}

/// Why a schedule stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompressionScheduleExit {
    /// The requested bound was reached.
    Bound,
    /// The candidate-query cap was reached.
    WorkCap,
    /// The step budget was exhausted.
    StepBudget,
    /// The depth could not be lowered any further without leaving the sheet.
    DepthFloor,
}

impl CompressionScheduleExit {
    pub fn name(self) -> &'static str {
        match self {
            Self::Bound => "bound",
            Self::WorkCap => "workCap",
            Self::StepBudget => "stepBudget",
            Self::DepthFloor => "depthFloor",
        }
    }
}

/// The live schedule.
///
/// # Rollback, and why a moving depth does not break it
///
/// The coupled separator's rollback compares a from-scratch rescore against an
/// incrementally tracked minimum and aborts the arm when the two disagree; the
/// anatomy measured 146 of 171 arms dying there on a 0-6 f32-ulp disagreement,
/// 75.5% of all arm wall. A moving depth makes the boundary term of that
/// comparison depth-dependent, so inheriting it would mean restoring a depth as
/// well as a layout and would put the schedule squarely inside the mechanism it
/// exists to avoid.
///
/// So the schedule does not inherit it. It keeps mode 0's own accept/reject
/// discipline - which has no such rescore - and its rollback is a different
/// contract: the *only* restorable state is the deepest-confirmed slot, and
/// that slot is a `(layout, depth)` **pair** written at the same instant by the
/// same confirmation. Restoring it restores both halves together, so there is
/// no window in which a layout is held at a depth it was never confirmed at.
/// Because [`Self::floor_mm`] is monotone non-increasing, the depth a rollback
/// restores is never looser than any depth already confirmed, which is what
/// makes the restore safe to perform at any point in the schedule's life
/// rather than only at a step boundary.
#[derive(Clone, Debug)]
pub struct CompressionSchedule {
    settings: CompressionScheduleSettings,
    /// Every depth here is carried in **canonical grid units**, not in
    /// millimetres, and each of these is an exact integer held in an `f64`.
    ///
    /// That is not a micro-optimisation, it is the invariant. A schedule takes
    /// thousands of steps, and subtracting `0.001` from a millimetre value
    /// thousands of times accumulates error until the walk stops one step early
    /// and the depths it reports are not on the lattice the poses live on.
    /// Counting in grid units makes every depth the schedule ever names a
    /// number the layout can express, exactly, and makes the bound a
    /// termination condition on integers.
    ///
    /// The frontier: what the lane writes into the state at every sweep entry.
    depth_grid: f64,
    /// The deepest layout an exact validator has accepted. Monotone
    /// non-increasing; `depth_grid <= floor_grid` always.
    floor_grid: f64,
    /// The requested bound, where [`CompressionScheduleSettings::continue_past_bound`]
    /// decides whether the schedule stops.
    target_grid: f64,
    /// The hard lower limit any depth must stay above, whatever the bound says.
    limit_grid: f64,
    steps_planned: usize,
    steps_taken: usize,
    steps_since_confirmation: usize,
    steps_since_accepted_confirmation: usize,
    sweeps_run: usize,
    work_spent: usize,
    exact_pairs_per_confirmation: usize,
    confirmations_attempted: usize,
    confirmations_accepted: usize,
    confirmations_refused: usize,
    confirmations_skipped_infeasible: usize,
    micro_legalizations_attempted: usize,
    micro_legalizations_accepted: usize,
    rollbacks: usize,
    exit: CompressionScheduleExit,
}

impl CompressionSchedule {
    /// Builds a schedule that walks `start_depth_mm` down to `target_depth_mm`
    /// one canonical grid unit at a time.
    ///
    /// `depth_limit_mm` is the depth below which no clamp may ever be set - the
    /// sheet inset, in practice - so a schedule handed an absurd bound degrades
    /// to a shorter walk rather than to a negative sheet.
    pub fn new(
        settings: CompressionScheduleSettings,
        start_depth_mm: f64,
        target_depth_mm: f64,
        depth_limit_mm: f64,
    ) -> Self {
        let start_grid = to_grid(start_depth_mm);
        let target_grid = to_grid(target_depth_mm);
        // A non-finite or non-positive step would make the walk either infinite
        // or backwards, and both are worse than degrading to the canonical
        // unit; the parser rejects them first, so this is the second wall.
        let settings = CompressionScheduleSettings {
            step_grid: if settings.step_grid.is_finite() && settings.step_grid > 0.0 {
                settings.step_grid
            } else {
                1.0
            },
            ..settings
        };
        let steps_planned =
            ((start_grid - target_grid).max(0.0) / settings.step_grid) as usize;
        Self {
            settings,
            depth_grid: start_grid,
            floor_grid: start_grid,
            target_grid,
            limit_grid: to_grid(depth_limit_mm),
            steps_planned,
            steps_taken: 0,
            steps_since_confirmation: 0,
            steps_since_accepted_confirmation: 0,
            sweeps_run: 0,
            work_spent: 0,
            exact_pairs_per_confirmation: 0,
            confirmations_attempted: 0,
            confirmations_accepted: 0,
            confirmations_refused: 0,
            confirmations_skipped_infeasible: 0,
            micro_legalizations_attempted: 0,
            micro_legalizations_accepted: 0,
            rollbacks: 0,
            exit: CompressionScheduleExit::StepBudget,
        }
    }

    /// The depth the lane must be running against right now.
    pub fn depth_mm(&self) -> f64 {
        from_grid(self.depth_grid)
    }

    /// The monotone floor: the depth of the deepest confirmed layout.
    pub fn floor_mm(&self) -> f64 {
        from_grid(self.floor_grid)
    }

    pub fn step_mm(&self) -> f64 {
        from_grid(self.settings.step_grid)
    }

    pub fn steps_planned(&self) -> usize {
        self.steps_planned
    }

    pub fn sweeps_per_step(&self) -> usize {
        self.settings.sweeps_per_step
    }

    pub fn repair_policy(&self) -> CompressionRepairPolicy {
        self.settings.repair_policy
    }

    /// The lane's candidate-query count at the last time the schedule was told.
    ///
    /// The schedule is charged in the lane's *own* `surrogate_evaluations`
    /// counter rather than in `profiling::Counter::CandidateQueries`, and the
    /// two are the same number on this backend: `score_placement` increments
    /// both on the same line, and the profiling one is additionally gated on a
    /// process-global recording flag that a production run leaves off. Using
    /// the lane's counter makes the budget deterministic and load-independent
    /// whether or not anything is recording - which is the property the whole
    /// work-budget comparison rests on.
    pub fn work_spent(&self) -> usize {
        self.work_spent
    }

    /// The schedule's spend in the portfolio's own work currency: candidate
    /// queries plus [`WORK_UNITS_PER_EXACT_PAIR_TEST`] per exact pair test.
    ///
    /// The exact half is *derived* rather than sampled. Every confirmation
    /// validates the whole layout, which is exactly `n * (n - 1) / 2` pair
    /// questions for `n` pieces, so the schedule can charge itself for the
    /// exact tier without reading a counter and without a profiling build. That
    /// keeps a work-capped arm bit-reproducible: two runs of the same arm stop
    /// at the same step whether or not anything is recording.
    ///
    /// **It is deliberately a conservative over-estimate.** The process-wide
    /// `Counter::ExactPairTests` the portfolio's meter reads is incremented
    /// *past* the broad-phase bounds reject (`kernel::exact`), so it counts the
    /// narrow phase only: on the 61-piece fixture a confirmation asks 1,830
    /// pairs and reaches the narrow phase on about 99 of them, so this charges
    /// roughly 18x what the coordinator's own meter would. A schedule capped
    /// here therefore stops **earlier** than the same cap would stop it inside
    /// the coordinator - measured on the gate band at 19.5M of the
    /// coordinator's units against a 33.4M self-cap - and every cost number
    /// this port reports is on the coordinator's counter rather than on this
    /// one, so the two never get mixed.
    pub fn work_units(&self) -> usize {
        self.work_spent.saturating_add(
            WORK_UNITS_PER_EXACT_PAIR_TEST.saturating_mul(self.exact_pair_tests()),
        )
    }

    /// The exact pair tests the schedule's own confirmations have cost.
    pub fn exact_pair_tests(&self) -> usize {
        self.exact_pairs_per_confirmation
            .saturating_mul(self.confirmations_attempted)
    }

    /// Tells the schedule how many pair questions one whole-layout validation
    /// asks, so it can price its own exact tier.
    pub fn set_exact_pairs_per_confirmation(&mut self, pairs: usize) {
        self.exact_pairs_per_confirmation = pairs;
    }

    pub fn steps_taken(&self) -> usize {
        self.steps_taken
    }

    pub fn exit(&self) -> CompressionScheduleExit {
        self.exit
    }

    /// The lowest depth the next step may set, given the bound policy, in grid
    /// units.
    fn lower_limit_grid(&self) -> f64 {
        if self.settings.continue_past_bound {
            self.limit_grid
        } else {
            self.target_grid.max(self.limit_grid)
        }
    }

    /// Whether the schedule may take another step, and records why not.
    pub fn may_step(&mut self, work_spent: usize) -> bool {
        self.work_spent = work_spent;
        if let Some(cap) = self.settings.work_cap_queries {
            if self.work_units() >= cap {
                self.exit = CompressionScheduleExit::WorkCap;
                return false;
            }
        }
        if self.depth_grid - self.settings.step_grid < self.lower_limit_grid() {
            self.exit = if self.settings.continue_past_bound {
                CompressionScheduleExit::DepthFloor
            } else {
                CompressionScheduleExit::Bound
            };
            return false;
        }
        true
    }

    /// Lowers the clamp by one step - one canonical grid unit unless the caller
    /// asked for a sub-grid frontier through
    /// [`CompressionScheduleSettings::step_grid`].
    ///
    /// The invariant `depth_mm <= floor_mm` is preserved by construction here:
    /// the depth only ever decreases in this function, and the floor only ever
    /// decreases in [`Self::note_confirmed`].
    pub fn step_down(&mut self) {
        self.depth_grid -= self.settings.step_grid;
        self.steps_taken += 1;
        self.steps_since_confirmation += 1;
        self.steps_since_accepted_confirmation += 1;
        debug_assert!(
            self.depth_grid <= self.floor_grid,
            "compression schedule frontier relaxed past its monotone floor"
        );
    }

    pub fn note_sweep(&mut self) {
        self.sweeps_run += 1;
    }

    /// Whether the exact tier should be asked about the current layout.
    ///
    /// Two clauses, and both are cost control. The cadence clause is the
    /// anatomy's budget. The feasibility clause is what actually keeps the
    /// exact tier affordable: a layout the proxy tier already calls infeasible
    /// is one the exact validator will refuse, and asking anyway costs
    /// milliseconds the schedule would rather spend on sweeps. On the mixed-61
    /// band it suppresses 96-99% of the confirmations the cadence makes due,
    /// so the skips are counted and reported - the cadence a run *achieved* is
    /// a measurement, and it is not the cadence the design asked for.
    pub fn due_for_confirmation(&mut self, proxy_feasible: bool) -> bool {
        if self.steps_since_confirmation < self.settings.confirm_every {
            return false;
        }
        if !proxy_feasible {
            self.confirmations_skipped_infeasible += 1;
            return false;
        }
        true
    }

    pub fn note_confirmation_attempt(&mut self) {
        self.confirmations_attempted += 1;
        self.steps_since_confirmation = 0;
    }

    /// Records an accepted confirmation *at the current frontier*, which is the
    /// only depth a confirmation can be about, and moves the monotone floor
    /// onto it.
    ///
    /// It takes no depth argument on purpose. A floor set from a number the
    /// caller supplies is a floor that can be set wrongly; a floor set from the
    /// frontier the confirmation actually ran at cannot, and the monotonicity
    /// is then a consequence of `depth_grid <= floor_grid` rather than a check.
    pub fn note_confirmed(&mut self) {
        self.confirmations_accepted += 1;
        self.steps_since_accepted_confirmation = 0;
        if self.depth_grid < self.floor_grid {
            self.floor_grid = self.depth_grid;
        }
        // The pair invariant, checkable rather than argued: after an accepted
        // confirmation the floor *is* the frontier, so the layout the caller
        // snapshots alongside it was confirmed at exactly the depth a later
        // rollback will restore.
        debug_assert_eq!(
            self.depth_grid, self.floor_grid,
            "an accepted confirmation leaves the floor on the frontier"
        );
    }

    pub fn note_refused(&mut self) {
        self.confirmations_refused += 1;
    }

    pub fn note_micro_legalization(&mut self, accepted: bool) {
        self.micro_legalizations_attempted += 1;
        if accepted {
            self.micro_legalizations_accepted += 1;
        }
    }

    /// Whether the frontier has run long enough without an accepted
    /// confirmation that it should be given back to the deepest-confirmed
    /// layout.
    pub fn due_for_rollback(&self) -> bool {
        self.settings.rollback_after_steps > 0
            && self.steps_since_accepted_confirmation >= self.settings.rollback_after_steps
    }

    /// Restores the frontier depth to the monotone floor.
    ///
    /// The caller restores the layout half of the same snapshot in the same
    /// statement; see the type's rollback note for why the two halves cannot
    /// come apart.
    pub fn rollback_to_floor(&mut self) {
        self.depth_grid = self.floor_grid;
        self.steps_since_accepted_confirmation = 0;
        self.rollbacks += 1;
    }

    /// The schedule's own report.
    pub fn report(&self) -> GeneralCompressionScheduleDiagnostics {
        GeneralCompressionScheduleDiagnostics {
            step_mm: self.step_mm(),
            steps_planned: self.steps_planned,
            steps_taken: self.steps_taken,
            sweeps_per_step: self.settings.sweeps_per_step,
            sweeps_run: self.sweeps_run,
            confirm_every: self.settings.confirm_every,
            rollback_after_steps: self.settings.rollback_after_steps,
            continue_past_bound: self.settings.continue_past_bound,
            work_cap_queries: self.settings.work_cap_queries,
            candidate_queries: self.work_spent,
            exact_pair_tests: self.exact_pair_tests(),
            work_units: self.work_units(),
            start_depth_mm: f64::NAN,
            parent_boundary_violations: 0,
            parent_collision_pairs: 0,
            parent_proxy_feasible: false,
            parent_entry_loss: 0.0,
            entry_legalization_armed: false,
            entry_legalization_run: false,
            entry_legalization_resolved: false,
            entry_legalization_accepted: false,
            entry_legalization_ms: 0.0,
            entry_proxy_feasible: false,
            entry_collision_pairs: 0,
            entry_boundary_violations: 0,
            entry_loss: 0.0,
            entry_source_depth_mm: None,
            entry_depth_loss_mm: None,
            requested_drop_mm: 0.0,
            entry_legalization_reason: None,
            entry_legalization_violating_pairs_before: 0,
            entry_legalization_violating_pairs_after: 0,
            entry_legalization_boundary_pieces_before: 0,
            entry_legalization_boundary_pieces_after: 0,
            aborted_barren_probe: false,
            probe_steps: 0,
            skipped_infeasible_entry: false,
            current_pose_overlay: false,
            current_pose_overlay_entries: 0,
            continuous_rotation: false,
            rotation_rungs_proposed: 0,
            rotation_rungs_improved: 0,
            mirror_toggles_proposed: 0,
            mirror_toggles_improved: 0,
            rotation_accepted_moves: 0,
            accepted_moves: 0,
            rotation_loss_bought_mm: 0.0,
            translation_loss_bought_mm: 0.0,
            rotation_surrogate_builds: 0,
            rotation_surrogate_hits: 0,
            rotation_surrogate_evictions: 0,
            rotation_surrogate_build_ms: 0.0,
            rotation_surrogate_cells: 0,
            rotation_builds_refused: 0,
            sparse_rotation: false,
            rotation_equivariant_offset: false,
            rotation_equivariant_builds: 0,
            rotation_equivariant_fallbacks: 0,
            sparse_rotation_episodes: 0,
            sparse_rotation_pieces_armed: 0,
            sparse_rotation_sweeps: 0,
            sparse_rotation_rungs_proposed: 0,
            sparse_rotation_rung_winners: 0,
            sparse_rotation_committed_moves: 0,
            sparse_rotation_committed_episodes: 0,
            se2_witness_adoptions: 0,
            se2_witness_calls: 0,
            se2_witness_accepted: 0,
            se2_witness_ms: 0.0,
            se2_witness_bought_mm: 0.0,
            current_pose_overlay_off_grid_pieces: 0,
            current_pose_overlay_setup_ms: 0.0,
            parent_pair_classification: Vec::new(),
            target_depth_mm: from_grid(self.target_grid),
            final_depth_mm: self.depth_mm(),
            floor_depth_mm: self.floor_mm(),
            confirmations_attempted: self.confirmations_attempted,
            confirmations_accepted: self.confirmations_accepted,
            confirmations_refused: self.confirmations_refused,
            confirmations_skipped_infeasible: self.confirmations_skipped_infeasible,
            micro_legalizations_attempted: self.micro_legalizations_attempted,
            micro_legalizations_accepted: self.micro_legalizations_accepted,
            rollbacks: self.rollbacks,
            exit_cause: self.exit.name().to_owned(),
            confirmation_ms: 0.0,
            repair_ms: 0.0,
            #[cfg(feature = "parallel-compression-schedule")]
            parallel: GeneralCompressionScheduleParallelDiagnostics::default(),
            steps: Vec::new(),
        }
    }
}

/// One parent pair, classified against both proxy resolutions and the exact
/// tier.
///
/// Written only by a diagnostic run (see
/// `GeneralCompressionScheduleDiagnostics::parent_pair_classification`). The
/// question it answers, per pair: when the continuous surrogate calls a pair
/// colliding and the grid surrogate does not, is the continuous one *right*
/// (the exact envelope really does conflict, so the grid proxy was missing a
/// real conflict, and the overlay is more conservative *and* more accurate) or
/// *wrong* (the exact envelope is clear, so the continuous proxy invented a
/// conflict, and the overlay is simply more inaccurate at the parent's own
/// rotations)?
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralCompressionSchedulePairClassification {
    pub first_piece_id: String,
    pub second_piece_id: String,
    /// The parent's own continuous rotations for the two pieces, and the grid
    /// angles the `StructuredGrid` snap would have used instead.
    pub first_rotation_deg: f64,
    pub second_rotation_deg: f64,
    pub first_snapped_rotation_deg: f64,
    pub second_snapped_rotation_deg: f64,
    /// The proxy tier's penalty for this pair with both pieces resolved at the
    /// parent's exact poses. `> 0` is the proxy calling the pair colliding.
    pub continuous_proxy_penalty: f64,
    /// The same, with both pieces resolved at their grid-snapped poses - the
    /// verdict today's default arm reaches.
    pub grid_proxy_penalty: f64,
    /// The exact tier on the parent's true geometry: do the two pieces'
    /// *material* polygons overlap? On a valid parent this is always `false`,
    /// and it is recorded so that stays visible rather than assumed.
    pub material_overlap: bool,
    /// The exact tier on the parent's true geometry: do the two pieces'
    /// *envelope* (material offset by half the pair clearance) polygons
    /// overlap? This is the verdict the authoritative validator uses, and it
    /// is what "the pair really does conflict" means.
    pub envelope_overlap: bool,
    /// Signed distance from the envelope-feasibility boundary, in millimetres,
    /// measured by bisecting the symmetric offset at which the two material
    /// polygons first touch. Positive is clearance to spare, negative is
    /// penetration; `0` is exactly on the boundary.
    ///
    /// This is the "margine dal confine" the review asks for, and it is what
    /// distinguishes a pair the proxy was right to flag from one it invented.
    ///
    /// **One saturating case, stated rather than hidden:** the bisection only
    /// searches *outward* offsets, so a pair whose material polygons already
    /// overlap has no bracket and reports the floor value
    /// `-2 * collision_expansion_mm` regardless of how deep the penetration is.
    /// [`Self::material_overlap`] is the flag that says the number saturated;
    /// on an exact-valid parent it is always `false` and the margin is always
    /// a real measurement.
    pub envelope_margin_mm: f64,
    /// The gap between the two *material* polygons, in millimetres.
    pub material_gap_mm: f64,
}

/// What one depth step cost and what residue it made.
///
/// This is the measurement the anatomy's second-biggest risk asks for: "residue
/// magnitude per step, against `micro_legalize`'s acceptance". The residue is
/// read off the tracker the lane already maintains, so the row costs nothing
/// the sweep was not already paying.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralCompressionScheduleStepRow {
    pub step: usize,
    pub depth_mm: f64,
    /// Pieces protruding past the new clamp, immediately after the step and
    /// before any repair sweep.
    pub boundary_violations_before: usize,
    /// Colliding pairs immediately after the step.
    pub collision_pairs_before: usize,
    /// The tracker's boundary loss immediately after the step: the residue's
    /// magnitude, in the lane's own units.
    pub boundary_loss_before: f64,
    /// The same three after the step's repair sweeps.
    pub boundary_violations_after: usize,
    pub collision_pairs_after: usize,
    pub boundary_loss_after: f64,
    /// Sweeps this step actually ran: a step whose residue cleared early stops
    /// short of `sweeps_per_step`.
    pub sweeps_run: usize,
    /// Candidate queries this step cost.
    pub candidate_queries: usize,
    /// Whether the step ended feasible in the proxy tier.
    pub proxy_feasible: bool,
    /// Whether a confirmation ran, and what it decided.
    pub confirmed: bool,
    /// The raw source depth an accepted confirmation measured, in millimetres.
    ///
    /// This is the only number the mode publishes on, and it is recorded per
    /// step because the schedule's clamp and the layout's depth are in
    /// different coordinates: the clamp bounds the *collision* polygons and the
    /// depth measures the *material*, so a clamp that has come down by `k`
    /// microns does not by itself say the layout has.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_depth_mm: Option<f64>,
    pub confirmation_refused: bool,
    pub micro_legalized: bool,
    pub rolled_back: bool,
}

/// The schedule's report.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralCompressionScheduleDiagnostics {
    pub step_mm: f64,
    pub steps_planned: usize,
    pub steps_taken: usize,
    pub sweeps_per_step: usize,
    pub sweeps_run: usize,
    pub confirm_every: usize,
    pub rollback_after_steps: usize,
    pub continue_past_bound: bool,
    pub work_cap_queries: Option<usize>,
    /// The lane's candidate-query count: the first half of the work unit.
    pub candidate_queries: usize,
    /// The exact pair tests the confirmations cost: the second half.
    pub exact_pair_tests: usize,
    /// `candidate_queries + 5 * exact_pair_tests` - the portfolio's own
    /// currency, so a schedule arm and a mode-26 arm can be priced against each
    /// other without a conversion.
    pub work_units: usize,
    pub start_depth_mm: f64,
    /// What the *proxy* tier thinks of the parent, before the schedule has
    /// taken a single step.
    ///
    /// A parent the exact validator accepts is not automatically a parent the
    /// surrogate tier calls feasible: the structured backend can only represent
    /// a pose on its 2.5-degree angle grid, so a layout whose rotations are
    /// continuous arrives at the lane already perturbed. When these are not
    /// zero, every number below describes a run that was repairing the
    /// *parent* rather than the schedule's own steps, and must be read that
    /// way.
    pub parent_boundary_violations: usize,
    pub parent_collision_pairs: usize,
    pub parent_proxy_feasible: bool,
    /// The magnitude behind the two counts above: boundary loss plus every
    /// colliding pair's raw penalty, in the proxy tier's own units, measured
    /// on the parent before the schedule takes a single step. Two runs can
    /// report the same violation and collision-pair counts while this
    /// differs, because a piece a continuous rotation away from its nearest
    /// `StructuredGrid` angle can still clear the same neighbours by a
    /// smaller margin. This is the number Sol review 5's entry-damage claim
    /// is about.
    pub parent_entry_loss: f64,
    /// Whether [`CompressionScheduleSettings::legalize_entry`] armed the
    /// translation-only repair of the parent, and whether it ran.
    ///
    /// It is armed-and-not-run whenever the parent arrived proxy-feasible,
    /// which is the state this whole mechanism exists to *produce*.
    pub entry_legalization_armed: bool,
    pub entry_legalization_run: bool,
    /// Whether the translation-only program came back with a layout at all,
    /// and whether the lane accepted it.
    ///
    /// The two differ: a program that closes onto a state the proxy tier likes
    /// no better than the one it started from is refused, because the entry
    /// repair may not hand the schedule a worse entry than the one it replaces.
    pub entry_legalization_resolved: bool,
    pub entry_legalization_accepted: bool,
    /// What the entry repair cost, in milliseconds, whether or not its result
    /// was accepted. `0.0` when it never ran, and no `Instant` is read then.
    pub entry_legalization_ms: f64,
    /// The proxy tier's verdict on the state the schedule actually starts from,
    /// after any entry repair. Equal to [`Self::parent_proxy_feasible`] when
    /// the repair did not run or was refused.
    pub entry_proxy_feasible: bool,
    pub entry_collision_pairs: usize,
    pub entry_boundary_violations: usize,
    pub entry_loss: f64,
    /// The **material** depth of the state the lane is actually holding when it
    /// arrives, measured on the untouched source rings by the same function
    /// every publication is measured by.
    ///
    /// This is the number the port's §7.2 entry-loss median is about, reported
    /// per slice rather than pooled, and it is the one the schedule's whole
    /// arithmetic turns on: the lane publishes only a layout whose source depth
    /// beats the *parent's*, and it may only walk `requested_drop_mm` of clamp
    /// to get there. A slice that arrives more than its own drop above the
    /// parent is asking to buy back the snap before it compresses anything.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_source_depth_mm: Option<f64>,
    /// `entry_source_depth_mm - parent_depth_mm`: what the 2.5-degree snap and
    /// the lane's own initialisation cost, in millimetres of published depth,
    /// before the schedule takes a step.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_depth_loss_mm: Option<f64>,
    /// The drop the slice was asked for, so `entry_depth_loss_mm` can be read
    /// against it without a second document.
    pub requested_drop_mm: f64,
    /// Why the translation-only entry repair did not produce a layout, when it
    /// ran and did not. The program's own `skippedReason` / `rejectionReason`,
    /// or `"unresolved"` when it simply did not converge.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_legalization_reason: Option<String>,
    /// The entry repair's own before/after violation counts, in the *exact*
    /// tier's terms rather than the proxy tier's. Reported because the two
    /// tiers disagree about this parent by construction and a repair that
    /// closed the exact tier's books while the proxy tier still refuses is a
    /// different finding from a repair that closed neither.
    pub entry_legalization_violating_pairs_before: usize,
    pub entry_legalization_violating_pairs_after: usize,
    pub entry_legalization_boundary_pieces_before: usize,
    pub entry_legalization_boundary_pieces_after: usize,
    /// Whether the lane abandoned the slice because its probe expired with
    /// nothing published below the parent, and at which step the probe ended.
    ///
    /// A lane that aborts here has spent `probe_steps` of its `steps_planned`
    /// and given the rest back. See
    /// [`CompressionScheduleSettings::barren_probe_denominator`].
    pub aborted_barren_probe: bool,
    pub probe_steps: usize,
    /// Whether the lane gave the slice back because its entry was still
    /// infeasible - see
    /// [`CompressionScheduleSettings::skip_infeasible_entry`]. A skipped slice
    /// takes no steps, asks the exact tier nothing and publishes nothing; the
    /// parent comes back unchanged, and the wall it did not spend is returned
    /// to the classes that do publish.
    pub skipped_infeasible_entry: bool,
    /// Whether `CurrentPoseOverlay` was armed for this run. See
    /// `GeneralRelaxedSettings::current_pose_overlay`.
    pub current_pose_overlay: bool,
    /// How many *surrogates* the overlay had to build.
    ///
    /// This is a catalogue size, not a piece count: entries are keyed by
    /// `(geometry_class, angle, mirror)`, so two instances of the same
    /// geometry class at the same continuous pose share one entry. Use it to
    /// price the overlay's setup. Zero whenever the overlay is off, and zero
    /// on an overlay run whose parent happened to be grid-native.
    ///
    /// The v5 round reported this alone and described it as a piece count;
    /// Sol review 6 §2.4 caught the conflation. See
    /// [`Self::current_pose_overlay_off_grid_pieces`].
    pub current_pose_overlay_entries: usize,
    /// How many of the parent's *placements* arrived off the 2.5-degree grid -
    /// that is, how many pieces the `StructuredGrid` snap would have moved.
    ///
    /// Always `>= current_pose_overlay_entries`; strictly greater exactly when
    /// a repetitive parent puts two instances of one geometry class at the
    /// same continuous pose. This is the damage measure; the entry count is
    /// the cost measure.
    pub current_pose_overlay_off_grid_pieces: usize,
    /// What the overlay cost to set up: building its surrogates plus
    /// installing them into the schedule's catalogue, in milliseconds.
    ///
    /// Sol review 6 §2.2 rejected the v5 round's installation - a deep clone of
    /// every grid orientation of every geometry class, to add a handful of
    /// entries - as setup cost the ten-second path cannot absorb, and asked for
    /// the price to be measured before and after. This is that measurement,
    /// reported by the run itself rather than inferred from process wall.
    /// `0.0` whenever the overlay is off, and no `Instant` is read then.
    pub current_pose_overlay_setup_ms: f64,
    /// Whether the continuous-rotation operator was armed for this slice, and
    /// what it did. See `GeneralRelaxedSettings::continuous_rotation`.
    ///
    /// The six counters are the slice's own answer to the three questions the
    /// operator is judged on, and they are reported together because no one of
    /// them means anything alone:
    ///
    /// * **acceptance** - `rotation_rungs_improved` against
    ///   `rotation_rungs_proposed`, and `rotation_accepted_moves` against
    ///   `accepted_moves`;
    /// * **yield** - `rotation_loss_bought_mm` against
    ///   `translation_loss_bought_mm`, the same quantity measured the same way
    ///   for both families, so "rotation bought millimetres" is a comparison
    ///   and not an assertion;
    /// * **price** - `rotation_surrogate_builds` and the milliseconds they
    ///   took, against `rotation_surrogate_hits`, which is what the cache
    ///   saved.
    ///
    /// All zero when the operator is off, which is every run that does not
    /// pass `crot=1` to a build with the feature.
    pub continuous_rotation: bool,
    pub rotation_rungs_proposed: usize,
    pub rotation_rungs_improved: usize,
    pub mirror_toggles_proposed: usize,
    pub mirror_toggles_improved: usize,
    pub rotation_accepted_moves: usize,
    pub accepted_moves: usize,
    pub rotation_loss_bought_mm: f64,
    pub translation_loss_bought_mm: f64,
    pub rotation_surrogate_builds: usize,
    pub rotation_surrogate_hits: usize,
    pub rotation_surrogate_evictions: usize,
    pub rotation_surrogate_build_ms: f64,
    /// Cells the operator built, and rungs it refused to build because its
    /// residency guard would have been breached. A non-zero refusal count is
    /// the first thing to read if an armed slice under-performs.
    pub rotation_surrogate_cells: usize,
    pub rotation_builds_refused: usize,
    /// The sparse operator's own account of the slice, and it answers a
    /// different question from the six above: those price *rotation*, these
    /// price **sparsity** and the construction it depends on.
    ///
    /// * **coverage** - `rotation_equivariant_builds` against
    ///   `rotation_surrogate_builds`. Equal means every rung took the offset-once
    ///   path; a gap is `rotation_equivariant_fallbacks` pieces whose offset ring
    ///   did not survive rotation onto the grid;
    /// * **sparsity** - `sparse_rotation_episodes` is the number of stalls that
    ///   armed anything at all, `sparse_rotation_pieces_armed / episodes` is the
    ///   mean episode width against the piece count, and
    ///   `sparse_rotation_sweeps` is how many sweeps ran with an episode open.
    ///   If the last is close to the slice's whole sweep count, design B has
    ///   quietly become design A and the depth delta must be read that way;
    /// * **witness** - the design-C counters. `se2_witness_ms` is the wall the
    ///   certificate took *out of this slice's repair budget*, and it is
    ///   reported whether or not the witness bought anything, because a
    ///   proposal source that costs a slice and returns nothing is the finding.
    ///
    /// All zero without `sparse-rotation` compiled in.
    pub sparse_rotation: bool,
    pub rotation_equivariant_offset: bool,
    pub rotation_equivariant_builds: usize,
    pub rotation_equivariant_fallbacks: usize,
    pub sparse_rotation_episodes: usize,
    pub sparse_rotation_pieces_armed: usize,
    pub sparse_rotation_sweeps: usize,
    /// The operator's own chain, and the column a disarm bit must read.
    ///
    /// `rotation_accepted_moves` counts any accepted move whose pose changed -
    /// including a random catalogue start that happened to win at a different
    /// angle - so it is not the operator's attribution and the control arm
    /// proves it: zero rungs offered, 11,523 accepted "rotation" moves
    /// (Sol review 8 §2 P0). These four are keyed to the operator's episodes:
    /// rungs proposed inside an open episode, iterations one of them won and
    /// moved the pose with, accepted moves whose committed pose is still the
    /// one the rung produced, and the number of distinct episodes that
    /// produced at least one of those. `committed_episodes / episodes` is the
    /// fraction of stalls the operator actually converted, and it is zero on a
    /// lane where the operator never fired regardless of what the sweeps did.
    pub sparse_rotation_rungs_proposed: usize,
    pub sparse_rotation_rung_winners: usize,
    pub sparse_rotation_committed_moves: usize,
    pub sparse_rotation_committed_episodes: usize,
    pub se2_witness_calls: usize,
    pub se2_witness_accepted: usize,
    /// Accepted witnesses that were adopted as the child frontier. Zero unless
    /// `Se2WitnessSettings::adopt` armed it, and it is reported separately from
    /// `se2_witness_accepted` so "the witness published" and "the search
    /// continued from the witness" are two readings and not one.
    pub se2_witness_adoptions: usize,
    pub se2_witness_ms: f64,
    pub se2_witness_bought_mm: f64,
    /// Per-pair classification of the parent's proxy collisions, empty unless
    /// `current_pose_overlay_classify_pairs` armed it.
    ///
    /// Sol review 6 §2's "Interpretazione dei numeri" asks whether the
    /// continuous surrogate is *more conservative* or *more inaccurate* at the
    /// parent's own rotations, and says the `+9` collision-pair delta cannot
    /// be called "the expected price" until each new pair is classified. Each
    /// row here carries the four things that decide it for one pair: the exact
    /// material verdict, the exact envelope verdict, both proxy verdicts, and
    /// the pair's signed distance from the envelope-feasibility boundary.
    ///
    /// Off by default and gated behind its own setting because it runs an
    /// exact-tier bisection per pair: a run that emits this is a *diagnostic*
    /// run and its wall and work numbers must not be quoted against a
    /// measured arm.
    pub parent_pair_classification: Vec<GeneralCompressionSchedulePairClassification>,
    pub target_depth_mm: f64,
    /// The clamp the frontier ended at.
    pub final_depth_mm: f64,
    /// The monotone floor: the clamp of the deepest confirmed layout.
    pub floor_depth_mm: f64,
    pub confirmations_attempted: usize,
    pub confirmations_accepted: usize,
    pub confirmations_refused: usize,
    pub confirmations_skipped_infeasible: usize,
    pub micro_legalizations_attempted: usize,
    pub micro_legalizations_accepted: usize,
    pub rollbacks: usize,
    pub exit_cause: String,
    /// Wall-clock milliseconds inside the exact confirmations, so the
    /// anatomy's 0.491 ms/confirmation and its 2%-of-a-slice budget can be
    /// checked rather than assumed. A wall number in a shared-box measurement
    /// is a decomposition, never a claim.
    pub confirmation_ms: f64,
    /// Wall-clock milliseconds inside the repair sweeps, on the same terms.
    pub repair_ms: f64,
    /// The intra-arm parallel schedule's own report. See
    /// [`GeneralCompressionScheduleParallelDiagnostics`]. Absent - not merely
    /// zeroed - in a build without `parallel-compression-schedule`, so an
    /// armed-feature document and a `compression-schedule`-only one differ by
    /// this key alone.
    #[cfg(feature = "parallel-compression-schedule")]
    pub parallel: GeneralCompressionScheduleParallelDiagnostics,
    pub steps: Vec<GeneralCompressionScheduleStepRow>,
}

/// What the intra-arm fan-out did, so its multiplier is a measurement rather
/// than an assumption.
///
/// Every field here is a count the run made, not a rate derived from a clock:
/// the wall numbers this experiment reports live in `repair_ms` and
/// `confirmation_ms`, and the box these were measured on was shared.
#[cfg(feature = "parallel-compression-schedule")]
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralCompressionScheduleParallelDiagnostics {
    /// Workers per repair step, as asked for. `1` is the serial schedule.
    pub lanes: usize,
    /// Whether the exact confirmation was spread over the job pool.
    pub parallel_confirm: bool,
    /// Steps that actually fanned out. A step the proxy tier already calls
    /// feasible repairs nothing and dispatches nothing, and on the measured
    /// band that is 55-77% of all steps - which is the first reason the
    /// occupancy multiplier is not the lane multiplier.
    pub fanned_out_steps: usize,
    /// Candidate queries the winning worker of each fanned-out step spent:
    /// what a serial schedule would have paid for the same step.
    pub winner_queries: usize,
    /// Candidate queries the *losing* workers spent. This is the price of the
    /// fan-out in the work currency, and it is charged in full.
    pub discarded_queries: usize,
    /// How often each worker ordinal won its step, lowest ordinal first. A
    /// distribution concentrated on worker 0 means the fan-out is buying
    /// nothing the serial lane would not have found.
    pub lane_wins: Vec<usize>,
    /// Fanned-out steps where the winning worker reached proxy feasibility and
    /// worker 0 - the one whose seed matches the serial lane's - did not.
    /// These are the steps the fan-out actually bought.
    pub steps_won_off_lane_zero: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schedule(settings: CompressionScheduleSettings) -> CompressionSchedule {
        CompressionSchedule::new(settings, 100.0, 99.7, 1.0)
    }

    #[test]
    fn step_is_one_canonical_grid_unit() {
        assert_eq!(canonical_grid_step_mm(), 0.001);
        let plan = schedule(CompressionScheduleSettings::default());
        assert_eq!(plan.step_mm(), 0.001);
        // 0.3 mm of drop at one micron a step.
        assert_eq!(plan.steps_planned(), 300);
    }

    #[test]
    fn a_sub_grid_step_walks_the_same_drop_in_proportionally_more_steps() {
        let mut settings = CompressionScheduleSettings::default();
        settings.step_grid = 0.25;
        let mut plan = schedule(settings);
        assert_eq!(plan.step_mm(), 0.00025);
        // The same 0.3 mm of drop, at a quarter of a micron a step.
        assert_eq!(plan.steps_planned(), 1_200);
        let mut steps = 0;
        while plan.may_step(0) {
            plan.step_down();
            steps += 1;
        }
        assert_eq!(steps, 1_200);
        assert!((plan.depth_mm() - 99.7).abs() < 1e-9, "{}", plan.depth_mm());
    }

    #[test]
    fn a_non_positive_step_degrades_to_the_canonical_unit() {
        for bad in [0.0, -1.0, f64::NAN] {
            let mut settings = CompressionScheduleSettings::default();
            settings.step_grid = bad;
            assert_eq!(schedule(settings).step_mm(), canonical_grid_step_mm());
        }
    }

    #[test]
    fn frontier_never_relaxes_past_the_monotone_floor() {
        let mut plan = schedule(CompressionScheduleSettings::default());
        for _ in 0..10 {
            assert!(plan.may_step(0));
            plan.step_down();
        }
        assert!(plan.depth_mm() < plan.floor_mm());
        // A confirmation at the current frontier lowers the floor onto it.
        plan.note_confirmed();
        assert_eq!(plan.depth_mm(), plan.floor_mm());
        // The floor is set from the frontier, so it cannot be set to anything
        // looser; stepping on and confirming again only lowers it further.
        let deeper = plan.floor_mm();
        plan.may_step(0);
        plan.step_down();
        plan.note_confirmed();
        assert!(plan.floor_mm() < deeper);
        assert_eq!(plan.floor_mm(), 100.0 - 11.0 * 0.001);
    }

    #[test]
    fn rollback_restores_exactly_the_floor() {
        let mut settings = CompressionScheduleSettings::default();
        settings.rollback_after_steps = 4;
        let mut plan = schedule(settings);
        for _ in 0..3 {
            plan.may_step(0);
            plan.step_down();
        }
        plan.note_confirmed();
        let floor = plan.floor_mm();
        for _ in 0..4 {
            plan.may_step(0);
            plan.step_down();
        }
        assert!(plan.due_for_rollback());
        plan.rollback_to_floor();
        assert_eq!(plan.depth_mm(), floor);
        assert_eq!(plan.floor_mm(), floor);
        assert!(!plan.due_for_rollback());
    }

    #[test]
    fn rollback_is_disabled_at_zero() {
        let mut settings = CompressionScheduleSettings::default();
        settings.rollback_after_steps = 0;
        let mut plan = schedule(settings);
        for _ in 0..200 {
            plan.may_step(0);
            plan.step_down();
        }
        assert!(!plan.due_for_rollback());
    }

    #[test]
    fn the_bound_stops_the_schedule_and_names_itself() {
        let mut plan = schedule(CompressionScheduleSettings::default());
        let mut taken = 0;
        while plan.may_step(0) {
            plan.step_down();
            taken += 1;
            assert!(taken <= 400, "schedule ran past its bound");
        }
        assert_eq!(taken, 300);
        assert_eq!(plan.exit(), CompressionScheduleExit::Bound);
        assert!(plan.depth_mm() >= 99.7 - 1e-9);
    }

    #[test]
    fn continuing_past_the_bound_stops_at_the_depth_limit() {
        let mut settings = CompressionScheduleSettings::default();
        settings.continue_past_bound = true;
        let mut plan = CompressionSchedule::new(settings, 1.010, 1.005, 1.0);
        let mut taken = 0;
        while plan.may_step(0) {
            plan.step_down();
            taken += 1;
            assert!(taken <= 64);
        }
        assert_eq!(taken, 10);
        assert_eq!(plan.exit(), CompressionScheduleExit::DepthFloor);
    }

    #[test]
    fn the_work_cap_stops_the_schedule_and_names_itself() {
        let mut settings = CompressionScheduleSettings::default();
        settings.work_cap_queries = Some(1_000);
        let mut plan = schedule(settings);
        assert!(plan.may_step(999));
        plan.step_down();
        assert!(!plan.may_step(1_000));
        assert_eq!(plan.exit(), CompressionScheduleExit::WorkCap);
    }

    #[test]
    fn confirmation_cadence_counts_its_own_skips() {
        let mut plan = schedule(CompressionScheduleSettings::default());
        for _ in 0..3 {
            plan.step_down();
            assert!(!plan.due_for_confirmation(true));
        }
        plan.step_down();
        // Due on cadence, but the proxy tier already refuses it.
        assert!(!plan.due_for_confirmation(false));
        assert_eq!(plan.report().confirmations_skipped_infeasible, 1);
        assert!(plan.due_for_confirmation(true));
        plan.note_confirmation_attempt();
        assert!(!plan.due_for_confirmation(true));
    }
}
