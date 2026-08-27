//! The work vector and the anytime trace.
//!
//! Two rules from the spec of record, and they are the reason this module is
//! separate from the solver at all:
//!
//! * **the work currency is a vector, not an exchange rate.** The old engine's
//!   265 ns "candidate evaluation" does not transfer; one signed cell-contact
//!   update is different work. The six names below are Sol R2 §4's, and every
//!   evidence document reports all six.
//! * **the only quality series is exact-valid raw source depth.** `phi`, `T`,
//!   `max_g` and the guided energy appear on the same timeline as
//!   *diagnostics* and are never plotted as progress. A run that never
//!   publishes has an empty quality series and says so, rather than showing a
//!   falling proxy.
//!
//! There is no `Instant` anywhere in the trajectory. Wall is read by the
//! driver, around whole phases, and never by a solver decision.

use std::collections::BTreeMap;

/// The six counters of the work vector, the piece-visit currency the work quota
/// is written in, and the **new, separately named** relocate economics.
///
/// Arbitration 4 of docs/cutclose-relocate-spec.md, which is Sol review 17
/// Round 2 §2's correction: the committed cold-Φ, row-rebuild and cell-gap
/// thresholds keep their literal meaning and their literal numbers, and the
/// relocate gets a **new metric version** rather than a rename. A sample
/// evaluation is not the old piece proposal - one relocate is hundreds of them
/// - and quietly re-denominating the 100 K pin would manufacture a continuity
/// that does not exist. So `sampleEvaluations`, `relocates`, `containerWinners`,
/// `focusedWinners`, `stayPutWinners` and `containerCommits` are new names, and
/// the two ratios the gate reads (`sampleEvaluationsPerRelocate`,
/// `relocatesPerSecond`) are derived from them here rather than in a driver.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorkVector {
    pub pair_row_probes: u64,
    pub convex_cell_gap_queries: u64,
    /// Cell-pair box tests inside `measure_pair`: `|A cells| x |B cells|` for
    /// every pair the piece-level broad phase let through. The evaluation-cost
    /// work says the surviving pairs are 60-70 % of a candidate evaluation, and
    /// this is the number that says whether that is the cell scan or the SAT
    /// inside it.
    pub cell_pair_box_tests: u64,
    /// Of the `convex_cell_gap_queries`, how many ended on the **separated**
    /// branch and therefore paid the `O(|a| * |b|)` `closest_feature` segment
    /// scan rather than stopping inside the axis loop.
    pub sat_separated_calls: u64,
    /// Of the `convex_cell_gap_queries`, how many returned a gap at or above
    /// the pair clearance. Their answer cannot beat any `worst >= 0`, so the
    /// exact query was pure waste: this is the size of the prize a separating
    /// axis prune can take, and nothing else in the vector measures it.
    pub sat_discarded_calls: u64,
    pub pose_transforms: u64,
    /// Vestigial: candidates of the retired topology jump. Always zero under
    /// `CutCloseRelocate`.
    pub jump_proposals: u64,
    pub exact_checkpoints: u64,
    pub repair_rows: u64,
    /// One piece **slot** of a sweep: `n` per sweep, whatever the colliding set
    /// held.
    ///
    /// **It is no longer a count of operator invocations, and it is not the
    /// unit of the old 100 K-in-8-seconds proposal pin.** It used to be "a
    /// gradient formed and a backtracking ladder walked", one per piece per
    /// sweep. It is now a slot, most of which are empty once a layout is close
    /// to feasible, because the member only relocates the colliding set. The
    /// operator's own count is `relocates`, its cost is `sampleEvaluations`,
    /// and nothing here claims parity with the retired pin.
    ///
    /// **It equals `Descent::proposals` under one worker and is `workers`
    /// times it under the Algorithm-10 tournament.** On the locked-strip
    /// trajectory (`Engine::run`: S0, S1, triangle-20, the corpus, the
    /// throughput cell) there is one sweep per master iteration and the two are
    /// numerically identical, which is what lets a reader divide
    /// `IcsConfig::proposal_budget` by it. Under `Engine::run_cutclose` eight
    /// workers each sweep the same master state, so eight times the slots are
    /// really visited, while `Descent::proposals` - the *trajectory's* ordinal,
    /// taken from the winner - advances by `n`. Both numbers are true and they
    /// are not the same number; charging only the winner would report one
    /// eighth of the machine's work.
    pub piece_proposals: u64,
    /// Piece visits that committed a pose different from the entry pose.
    pub accepted_moves: u64,
    /// Algorithm-8 passes: one per master iteration, over every row.
    pub weight_updates: u64,
    /// Pair rows the piece-box proof zeroed without a cell query.
    pub broad_phase_rejects: u64,

    // ------------------------------------------- the relocate metric version --
    /// One incremental incident-Φ evaluation of one candidate pose - a pool
    /// sample or a coordinate-descent candidate. **The member's work currency.**
    pub sample_evaluations: u64,
    /// Relocates that actually ran, i.e. piece visits whose piece was in the
    /// colliding set.
    pub relocates: u64,
    /// Pool draws inside the piece's own current AABB.
    pub focused_samples: u64,
    /// Pool draws across the usable strip at the current `T`.
    pub container_samples: u64,
    /// Relocates whose winning finalist descended from a container-wide draw.
    pub container_winners: u64,
    /// ... from a focused draw.
    pub focused_winners: u64,
    /// ... from the pose the piece already had.
    pub stay_put_winners: u64,
    /// Container winners that **moved the piece**. This is the neutered-relocate
    /// tripwire's counter: `containerSamples >= 50` with `containerCommits == 0`
    /// on a fixture with a distant vacancy is the defect, not a preference.
    pub container_commits: u64,
    /// Algorithm-12 disruptions fired.
    pub disruptions: u64,
    /// Pieces moved by a disruption, the two swapped ones included.
    pub disruption_moves: u64,
}

impl WorkVector {
    pub fn saturating_sub(self, earlier: Self) -> Self {
        Self {
            pair_row_probes: self.pair_row_probes.saturating_sub(earlier.pair_row_probes),
            convex_cell_gap_queries: self
                .convex_cell_gap_queries
                .saturating_sub(earlier.convex_cell_gap_queries),
            cell_pair_box_tests: self
                .cell_pair_box_tests
                .saturating_sub(earlier.cell_pair_box_tests),
            sat_separated_calls: self
                .sat_separated_calls
                .saturating_sub(earlier.sat_separated_calls),
            sat_discarded_calls: self
                .sat_discarded_calls
                .saturating_sub(earlier.sat_discarded_calls),
            pose_transforms: self.pose_transforms.saturating_sub(earlier.pose_transforms),
            jump_proposals: self.jump_proposals.saturating_sub(earlier.jump_proposals),
            exact_checkpoints: self
                .exact_checkpoints
                .saturating_sub(earlier.exact_checkpoints),
            repair_rows: self.repair_rows.saturating_sub(earlier.repair_rows),
            piece_proposals: self.piece_proposals.saturating_sub(earlier.piece_proposals),
            accepted_moves: self.accepted_moves.saturating_sub(earlier.accepted_moves),
            weight_updates: self.weight_updates.saturating_sub(earlier.weight_updates),
            broad_phase_rejects: self
                .broad_phase_rejects
                .saturating_sub(earlier.broad_phase_rejects),
            sample_evaluations: self
                .sample_evaluations
                .saturating_sub(earlier.sample_evaluations),
            relocates: self.relocates.saturating_sub(earlier.relocates),
            focused_samples: self.focused_samples.saturating_sub(earlier.focused_samples),
            container_samples: self
                .container_samples
                .saturating_sub(earlier.container_samples),
            container_winners: self
                .container_winners
                .saturating_sub(earlier.container_winners),
            focused_winners: self.focused_winners.saturating_sub(earlier.focused_winners),
            stay_put_winners: self
                .stay_put_winners
                .saturating_sub(earlier.stay_put_winners),
            container_commits: self
                .container_commits
                .saturating_sub(earlier.container_commits),
            disruptions: self.disruptions.saturating_sub(earlier.disruptions),
            disruption_moves: self
                .disruption_moves
                .saturating_sub(earlier.disruption_moves),
        }
    }

    pub fn saturating_add(&mut self, other: &Self) {
        self.pair_row_probes += other.pair_row_probes;
        self.convex_cell_gap_queries += other.convex_cell_gap_queries;
        self.cell_pair_box_tests += other.cell_pair_box_tests;
        self.sat_separated_calls += other.sat_separated_calls;
        self.sat_discarded_calls += other.sat_discarded_calls;
        self.pose_transforms += other.pose_transforms;
        self.jump_proposals += other.jump_proposals;
        self.exact_checkpoints += other.exact_checkpoints;
        self.repair_rows += other.repair_rows;
        self.piece_proposals += other.piece_proposals;
        self.accepted_moves += other.accepted_moves;
        self.weight_updates += other.weight_updates;
        self.broad_phase_rejects += other.broad_phase_rejects;
        self.sample_evaluations += other.sample_evaluations;
        self.relocates += other.relocates;
        self.focused_samples += other.focused_samples;
        self.container_samples += other.container_samples;
        self.container_winners += other.container_winners;
        self.focused_winners += other.focused_winners;
        self.stay_put_winners += other.stay_put_winners;
        self.container_commits += other.container_commits;
        self.disruptions += other.disruptions;
        self.disruption_moves += other.disruption_moves;
    }

    /// `sampleEvaluationsPerRelocate`: how many candidate poses one relocate
    /// actually paid for. A relocate that reports 76 has not run its coordinate
    /// descents; one that reports 0 has not run at all.
    pub fn sample_evaluations_per_relocate(&self) -> f64 {
        if self.relocates == 0 {
            0.0
        } else {
            self.sample_evaluations as f64 / self.relocates as f64
        }
    }

    /// `relocatesPerSecond`, given the wall the caller measured around whole
    /// sweeps. The seconds are the driver's; this module never reads a clock.
    pub fn relocates_per_second(&self, seconds: f64) -> f64 {
        if seconds <= 0.0 {
            0.0
        } else {
            self.relocates as f64 / seconds
        }
    }

    pub fn to_map(self) -> BTreeMap<&'static str, u64> {
        let mut map = BTreeMap::new();
        map.insert("pairRowProbes", self.pair_row_probes);
        map.insert("convexCellGapQueries", self.convex_cell_gap_queries);
        map.insert("cellPairBoxTests", self.cell_pair_box_tests);
        map.insert("satSeparatedCalls", self.sat_separated_calls);
        map.insert("satDiscardedCalls", self.sat_discarded_calls);
        map.insert("poseTransforms", self.pose_transforms);
        map.insert("jumpProposals", self.jump_proposals);
        map.insert("exactCheckpoints", self.exact_checkpoints);
        map.insert("repairRows", self.repair_rows);
        map.insert("pieceProposals", self.piece_proposals);
        map.insert("acceptedMoves", self.accepted_moves);
        map.insert("weightUpdates", self.weight_updates);
        map.insert("broadPhaseRejects", self.broad_phase_rejects);
        map.insert("sampleEvaluations", self.sample_evaluations);
        map.insert("relocates", self.relocates);
        map.insert("focusedSamples", self.focused_samples);
        map.insert("containerSamples", self.container_samples);
        map.insert("containerWinners", self.container_winners);
        map.insert("focusedWinners", self.focused_winners);
        map.insert("stayPutWinners", self.stay_put_winners);
        map.insert("containerCommits", self.container_commits);
        map.insert("disruptions", self.disruptions);
        map.insert("disruptionMoves", self.disruption_moves);
        map
    }
}

/// One publication attempt, successful or not. Attempts are expected; invalid
/// *outputs* are the invariant violation, and there is no such row here because
/// `publish.rs` cannot produce one.
#[derive(Clone, Debug, PartialEq)]
pub struct ExactCheckpoint {
    pub proposal_ordinal: u64,
    /// The strip this attempt was made inside.
    pub target_depth_mm: f64,
    /// `max_g` at the moment of the attempt.
    pub max_violation_mm: f64,
    /// The state's raw source depth before any repair.
    pub proxy_raw_depth_mm: f64,
    pub kernel_exclusive_valid: bool,
    pub contract_valid: bool,
    pub repair_rows: u64,
    pub repair_max_displacement_mm: f64,
    pub repair_depth_giveback_mm: f64,
    /// `Some` only on a dual-valid publication.
    pub published_raw_depth_mm: Option<f64>,
    pub refusal: Option<String>,
}

/// One entry of the anytime quality series. Exact-valid raw source depth only.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QualityPoint {
    pub proposal_ordinal: u64,
    pub raw_source_depth_mm: f64,
    /// `false` while the incumbent is still the constructor's own layout.
    pub strict_child: bool,
}

/// The whole trajectory's diagnostics.
#[derive(Clone, Debug, Default)]
pub struct Trace {
    pub work: WorkVector,
    pub checkpoints: Vec<ExactCheckpoint>,
    pub quality: Vec<QualityPoint>,
    /// Present only in the conflict-cluster experiment build and never emitted
    /// by runtime Off.
    #[cfg(feature = "conflict-cluster-budget")]
    pub partition: super::cluster_budget::PartitionTrace,
    /// Raw/guided Φ and `max_g` at epoch boundaries. Diagnostics, never quality.
    pub proxy_samples: Vec<ProxySample>,
    pub sweeps: u64,
    pub guided_stalls: u64,
    /// Jumps whose relocation was **installed**: the trajectory's spent
    /// allowance.
    pub jumps: u64,
    /// Jumps whose 16 candidates were evaluated at all, installed or not.
    pub jump_attempted: u64,
    /// Jumps that adopted a candidate state. Equal to `jumps`; kept beside
    /// `jump_attempted` because the pair is what tells a no-op from a
    /// relocation, and the previous round's documents could not.
    pub jump_committed: u64,
    /// Jumps whose **best candidate beat the pre-jump guided Φ**.
    ///
    /// This is the counter Grok review 10 asks to be named for what it is: it
    /// is not "a relocation was installed" and never was. Under the default
    /// commit rule a jump installs whether or not this is true, so a `0` here
    /// beside a nonzero `jumpCommitted` means the jump was a deliberate step
    /// backwards - which is the point of a topology change - and not a no-op.
    pub jumps_improving_guided: u64,
    /// One row per attempted jump: the scale it fired at and what it did.
    pub jump_events: Vec<JumpEvent>,
}

/// One attempted jump, recorded whatever it did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JumpEvent {
    pub proposal_ordinal: u64,
    pub piece: usize,
    /// `"strip"` above the 0.100 mm gate, `"ball"` at or below it.
    pub kind: &'static str,
    /// The ball's translational radius; infinite for a strip relocation.
    pub radius_mm: f64,
    /// `max_g` at the moment the scale was chosen.
    pub max_violation_mm: f64,
    pub baseline_guided: f64,
    pub best_guided: f64,
    pub installed: bool,
    pub improved_guided: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProxySample {
    pub proposal_ordinal: u64,
    pub target_depth_mm: f64,
    pub raw_phi: f64,
    pub guided_phi: f64,
    pub max_violation_mm: f64,
    pub raw_source_depth_mm: f64,
}
