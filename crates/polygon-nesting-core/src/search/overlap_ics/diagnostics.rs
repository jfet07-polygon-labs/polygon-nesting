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

/// The six counters of the work vector, plus the piece-proposal currency the
/// throughput gate is written in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorkVector {
    pub pair_row_probes: u64,
    pub convex_cell_gap_queries: u64,
    pub pose_transforms: u64,
    pub jump_proposals: u64,
    pub exact_checkpoints: u64,
    pub repair_rows: u64,
    /// One complete piece proposal: a piece selected, its gradient formed, and
    /// the backtracking ladder walked to accept or reject. This is the unit the
    /// 100K-in-8-seconds kill is stated in.
    pub piece_proposals: u64,
    /// Proposals whose ladder found a strict decrease.
    pub accepted_moves: u64,
    /// Guided weight increments.
    pub weight_updates: u64,
    /// Pair rows the piece-box proof zeroed without a cell query.
    pub broad_phase_rejects: u64,
}

impl WorkVector {
    pub fn saturating_add(&mut self, other: &Self) {
        self.pair_row_probes += other.pair_row_probes;
        self.convex_cell_gap_queries += other.convex_cell_gap_queries;
        self.pose_transforms += other.pose_transforms;
        self.jump_proposals += other.jump_proposals;
        self.exact_checkpoints += other.exact_checkpoints;
        self.repair_rows += other.repair_rows;
        self.piece_proposals += other.piece_proposals;
        self.accepted_moves += other.accepted_moves;
        self.weight_updates += other.weight_updates;
        self.broad_phase_rejects += other.broad_phase_rejects;
    }

    pub fn to_map(self) -> BTreeMap<&'static str, u64> {
        let mut map = BTreeMap::new();
        map.insert("pairRowProbes", self.pair_row_probes);
        map.insert("convexCellGapQueries", self.convex_cell_gap_queries);
        map.insert("poseTransforms", self.pose_transforms);
        map.insert("jumpProposals", self.jump_proposals);
        map.insert("exactCheckpoints", self.exact_checkpoints);
        map.insert("repairRows", self.repair_rows);
        map.insert("pieceProposals", self.piece_proposals);
        map.insert("acceptedMoves", self.accepted_moves);
        map.insert("weightUpdates", self.weight_updates);
        map.insert("broadPhaseRejects", self.broad_phase_rejects);
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
