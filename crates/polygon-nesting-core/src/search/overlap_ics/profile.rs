//! **The master-iteration phase census. Measurement only, and off by default.**
//!
//! docs/economics-round-spec.md funds one thing here and only one: the
//! *measured gate* in front of the persistent executor. "Profile easy +
//! bite-22 hard states, workers 1/2/4/8, identical fixed work (prep,
//! dispatch/join, sweeps, merge+GLS, exact/repair separately). Build iff
//! prep+dispatch >= 10 % of hard-state wall." Nothing in this module may
//! change a trajectory, and the feature gate is how that is enforced rather
//! than promised.
//!
//! # The clock rule, and how this module does not break it
//!
//! The evidence audit's clean finding is load-bearing for every determinism
//! claim the campaign has made:
//!
//! > **No clock inside a sweep.** `Instant` appears in exactly one place under
//! > `search/overlap_ics/` — `Pacer::Wall` … a fixed-work trajectory cannot
//! > read a clock at all — which is what makes the two-process bit-identity a
//! > proof rather than a coincidence.
//!
//! [`ics_time!`] and every write below are compiled **only** under the
//! `ics-profile` cargo feature, which is off in the default build, off in the
//! `overlap-ics` feature set the gates are measured on, and never enabled in a
//! binary that publishes a number to a gate. With the feature off this file
//! contributes one `u64` field per worker slot, zero clock reads and zero
//! branches, and the sentence above stays literally true of the shipped
//! binary. With the feature on the clock reads are still *outside* every
//! decision - no field of [`PhaseProfile`] is ever read by the engine - so a
//! profiling build takes the same trajectory as the shipped one. That is
//! asserted, not asserted-by-comment: the census battery compares a
//! `--features ics-profile` build's whole fixed-work document against the
//! default build's, and the census document records the verdict.
//!
//! # What the phases mean
//!
//! One *master iteration* is one turn of `Engine::separate`'s loop, and it is
//! the barrier-to-barrier unit the spec's 10 % clause is denominated in.
//! Inside it:
//!
//! * `prep_ns` — cloning the master state, the descent and a fresh work vector
//!   into `workers` slots, and keying each stream. **This is the half of the
//!   spawn tax a persistent executor removes by keeping the slots alive and
//!   using `clone_from`.**
//! * `dispatch_ns` — `std::thread::scope`'s wall **minus the critical-path
//!   sweep**: thread creation, the scheduler getting eight threads onto eight
//!   cores, and the join. **This is the other half.** At `workers == 1` there
//!   are no threads and it is zero by construction, which is what makes the
//!   1/2/4/8 ladder a measurement of the tax rather than of the machine.
//! * `sweep_critical_ns` — the longest single worker sweep, i.e. the parallel
//!   work on the critical path. `sweep_total_ns` is the sum over all workers:
//!   the CPU the iteration really spent, which is what the currency has to be
//!   denominated in.
//! * `merge_gls_ns` — the ordinal winner scan, installing the winner, the one
//!   Algorithm-8 pass and the fold that follows it.
//! * `exact_ns` — `Engine::attempt_publication`: the exact authorities and the
//!   repair loop, whether or not they published.
//! * `band_fold_ns` — the top-of-loop `energy::fold` that decides whether the
//!   band was entered at all.
//! * `snapshot_ns` — `snapshot.clone_from(&self.state)` on a new minimum, plus
//!   the strike rollback. A state copy that a persistent executor does **not**
//!   remove, which is why it is named separately from `prep_ns` instead of
//!   being folded into it.
//!
//! `barrier_to_barrier_ns` is the whole turn. The six named phases do not have
//! to sum to it and are not made to: the residual is reported as a residual,
//! because a decomposition that always adds up is usually a decomposition with
//! a fudge term in it.

/// Nanosecond accumulators for one measured region of the trajectory.
///
/// Every field is zero in a build without `ics-profile`, and no field is ever
/// read by the engine. It is `Copy` and 12 words, so a `BiteRecord` carrying
/// one costs nothing to clone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PhaseProfile {
    /// Master iterations, i.e. tournaments run. **Counted in every build** -
    /// it is a counter, not a clock - so it reconciles with
    /// `BiteRecord::master_iterations` and gives the currency its
    /// `master_batches` term per bite. The denominator of every
    /// per-iteration number the census prints.
    pub iterations: u64,
    /// The whole turn of `Engine::separate`'s loop.
    pub barrier_to_barrier_ns: u64,
    /// Slot/state/descent preparation.
    pub prep_ns: u64,
    /// Thread creation and join, off the critical path's sweep.
    pub dispatch_ns: u64,
    /// The longest single worker sweep.
    pub sweep_critical_ns: u64,
    /// Every worker's sweep, summed.
    pub sweep_total_ns: u64,
    /// Ordinal merge, install, Algorithm-8 pass and fold.
    pub merge_gls_ns: u64,
    /// `attempt_publication`: exact authorities and repair.
    pub exact_ns: u64,
    /// The top-of-loop fold that decides the band test.
    pub band_fold_ns: u64,
    /// Minimum-snapshot copies and strike rollbacks.
    pub snapshot_ns: u64,
    /// Turns that entered the 4 µm band. Reconciles with
    /// `BiteRecord::exact_band_entries`.
    pub band_entries: u64,
    /// Turns whose band entry reached exact geometry. Reconciles with
    /// `BiteRecord::exact_checkpoint_calls`.
    pub exact_calls: u64,

    // ------------------------------ the currency's terms, per bite, exactly --
    //
    // The spec's calibrated-work currency is
    // `U = sample_evaluations + B*master_batches
    //      + E*actual_publication_attempt_calls + R*repair_rows
    //      + D*disruption_moves`.
    //
    // Every one of its five terms is a **counter**, so all five are counted in
    // every build, feature or no feature, and none of them costs a clock read.
    // They are here rather than in `WorkVector` because `WorkVector` is
    // trajectory-global and the currency has to be denominated per bite: a
    // rate calibrated on bites 1-21 and spent on the 179 shelf is the spec's
    // pre-named defect (3), and the only way to see it is to hold the work of
    // the two windows apart. `iterations` above is `master_batches` and
    // `exact_calls` above is `actual_publication_attempt_calls`; these three
    // complete the vector.
    /// Incremental incident-Φ evaluations charged to this bite, all workers.
    pub sample_evaluations: u64,
    /// Repair rows this bite's publication attempts spent.
    pub repair_rows: u64,
    /// Pieces moved by this bite's disruptions.
    pub disruption_moves: u64,
}

impl PhaseProfile {
    pub fn add(&mut self, other: &Self) {
        self.iterations += other.iterations;
        self.barrier_to_barrier_ns += other.barrier_to_barrier_ns;
        self.prep_ns += other.prep_ns;
        self.dispatch_ns += other.dispatch_ns;
        self.sweep_critical_ns += other.sweep_critical_ns;
        self.sweep_total_ns += other.sweep_total_ns;
        self.merge_gls_ns += other.merge_gls_ns;
        self.exact_ns += other.exact_ns;
        self.band_fold_ns += other.band_fold_ns;
        self.snapshot_ns += other.snapshot_ns;
        self.band_entries += other.band_entries;
        self.exact_calls += other.exact_calls;
        self.sample_evaluations += other.sample_evaluations;
        self.repair_rows += other.repair_rows;
        self.disruption_moves += other.disruption_moves;
    }

    /// `prep + dispatch`, the numerator of the spec's pre-committed executor
    /// clause. **The verdict is the caller's**; this function only adds two
    /// numbers, so nothing here can quietly re-pick the threshold.
    pub fn prep_plus_dispatch_ns(&self) -> u64 {
        self.prep_ns + self.dispatch_ns
    }

    /// The share of barrier-to-barrier wall spent on preparation and dispatch,
    /// or `None` when nothing was measured. `None` is not zero: a build
    /// without `ics-profile` measured nothing and must not be read as a build
    /// that measured no tax.
    pub fn prep_plus_dispatch_share(&self) -> Option<f64> {
        if self.barrier_to_barrier_ns == 0 {
            return None;
        }
        Some(self.prep_plus_dispatch_ns() as f64 / self.barrier_to_barrier_ns as f64)
    }

    /// Whatever the six named phases did not account for: the loop's own
    /// bookkeeping, `observe_raw`, the strike ladder's comparisons, the
    /// fingerprint row when it is recorded, and the measurement's own
    /// overhead. Saturating, because a residual is never negative and a
    /// negative one would mean the regions overlapped.
    pub fn residual_ns(&self) -> u64 {
        let named = self.prep_ns
            + self.dispatch_ns
            + self.sweep_critical_ns
            + self.merge_gls_ns
            + self.exact_ns
            + self.band_fold_ns
            + self.snapshot_ns;
        self.barrier_to_barrier_ns.saturating_sub(named)
    }

    /// True when this profile carries a measurement at all.
    pub fn measured(&self) -> bool {
        self.barrier_to_barrier_ns > 0
    }
}

/// Times `$body` into `$profile.$field`, or compiles to `$body` alone.
///
/// The `let _ = &$profile;` in the inactive arm is what keeps a build without
/// the feature free of an `unused_variables` warning without an `allow` that
/// would also hide a real one.
#[cfg(feature = "ics-profile")]
macro_rules! ics_time {
    ($profile:expr, $field:ident, $body:expr) => {{
        let ics_profile_started = std::time::Instant::now();
        let ics_profile_value = $body;
        $profile.$field += ics_profile_started.elapsed().as_nanos() as u64;
        ics_profile_value
    }};
}

#[cfg(not(feature = "ics-profile"))]
macro_rules! ics_time {
    ($profile:expr, $field:ident, $body:expr) => {{
        let _ = &$profile;
        $body
    }};
}

pub(crate) use ics_time;
