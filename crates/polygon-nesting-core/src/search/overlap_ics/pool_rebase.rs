//! Diagnostics and runtime arm for the pool-retry tracker-rebase experiment.
//!
//! The only behavioral intervention lives at one call site in `mod.rs`: after
//! a least-infeasible pool entry's poses have been cold-installed and before
//! the unchanged disruption, [`PoolRebaseArm::Rebase`] returns every pair and
//! edge GLS weight to the exact floor. Rollbacks inside a separation retain
//! their weights and every other search seam is outside this module.

use sha2::{Digest, Sha256};

use super::diagnostics::WorkVector;
use super::disrupt::DisruptOutcome;
use super::state::{IcsState, Pose, GLS_WEIGHT_FLOOR};

const WEIGHT_DOMAIN: &[u8] = b"pool-retry-tracker-rebase/weights/v1";
const POSE_DOMAIN: &[u8] = b"pool-retry-tracker-rebase/poses/v1";
const POSE_TRANSFORM_DOMAIN: &[u8] = b"pool-retry-tracker-rebase/pose-transforms/v1";
const RAW_ROW_DOMAIN: &[u8] = b"pool-retry-tracker-rebase/raw-rows/v1";

/// The one-retry Gate-0 cap frozen by section 5.2 of the specification.
pub const FIRST_RETRY_ITERATION_CAP: u64 = 400;

/// The three runtime arms frozen by the exact specification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoolRebaseArm {
    /// Historical behavior: restore the selected pool entry's saved weights.
    Saved,
    /// Reset every weight to exactly one before disruption.
    Rebase,
    /// Exercise reset and telemetry, then restore the historical vector.
    ComputeIgnore,
}

impl PoolRebaseArm {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Saved => "saved",
            Self::Rebase => "rebase",
            Self::ComputeIgnore => "compute-ignore",
        }
    }
}

/// Stable, fully auditable reading of every GLS weight.
#[derive(Clone, Debug, PartialEq)]
pub struct WeightSnapshot {
    pub bits: Vec<u64>,
    pub digest_sha256: [u8; 32],
    pub count_above_floor: usize,
    pub minimum: f64,
    pub maximum: f64,
    pub all_finite: bool,
    pub all_exactly_one: bool,
}

impl WeightSnapshot {
    pub fn of(state: &IcsState) -> Self {
        let mut bits = Vec::with_capacity(state.pair_rows.len() + state.edge_rows.len() * 4);
        for row in &state.pair_rows {
            bits.push(row.weight.to_bits());
        }
        for rows in &state.edge_rows {
            for row in rows {
                bits.push(row.weight.to_bits());
            }
        }
        Self::from_bits(bits)
    }

    pub fn of_saved(pair_weights: &[f64], edge_weights: &[[f64; 4]]) -> Self {
        let mut bits = Vec::with_capacity(pair_weights.len() + edge_weights.len() * 4);
        bits.extend(pair_weights.iter().map(|value| value.to_bits()));
        for weights in edge_weights {
            bits.extend(weights.iter().map(|value| value.to_bits()));
        }
        Self::from_bits(bits)
    }

    fn from_bits(bits: Vec<u64>) -> Self {
        let mut digest = Sha256::new();
        digest.update((WEIGHT_DOMAIN.len() as u64).to_le_bytes());
        digest.update(WEIGHT_DOMAIN);
        digest.update((bits.len() as u64).to_le_bytes());
        for value in &bits {
            digest.update(value.to_le_bytes());
        }
        let mut minimum = f64::INFINITY;
        let mut maximum = f64::NEG_INFINITY;
        let mut count_above_floor = 0usize;
        let mut all_finite = true;
        let mut all_exactly_one = true;
        for value in bits.iter().map(|bits| f64::from_bits(*bits)) {
            all_finite &= value.is_finite();
            all_exactly_one &= value.to_bits() == GLS_WEIGHT_FLOOR.to_bits();
            count_above_floor += usize::from(value > GLS_WEIGHT_FLOOR);
            minimum = minimum.min(value);
            maximum = maximum.max(value);
        }
        if bits.is_empty() {
            minimum = GLS_WEIGHT_FLOOR;
            maximum = GLS_WEIGHT_FLOOR;
        }
        Self {
            bits,
            digest_sha256: digest.finalize().into(),
            count_above_floor,
            minimum,
            maximum,
            all_finite,
            all_exactly_one,
        }
    }
}

/// Applies exactly the section-2 policy to a cold-installed state.
///
/// `saved_*` are the selected pool entry's historical weights. Rebase never
/// writes them into the live rows. ComputeIgnore performs the reset first and
/// then restores them so its trajectory is the Saved control.
pub fn apply_weight_policy(
    state: &mut IcsState,
    arm: PoolRebaseArm,
    saved_pair: &[f64],
    saved_edges: &[[f64; 4]],
) -> Option<WeightSnapshot> {
    let restore = |state: &mut IcsState| {
        assert_eq!(state.pair_rows.len(), saved_pair.len());
        assert_eq!(state.edge_rows.len(), saved_edges.len());
        for (row, weight) in state.pair_rows.iter_mut().zip(saved_pair) {
            row.weight = *weight;
        }
        for (rows, weights) in state.edge_rows.iter_mut().zip(saved_edges) {
            for (row, weight) in rows.iter_mut().zip(weights) {
                row.weight = *weight;
            }
        }
    };
    match arm {
        PoolRebaseArm::Saved => {
            restore(state);
            None
        }
        PoolRebaseArm::Rebase => {
            super::energy::reset_weights(state);
            Some(WeightSnapshot::of(state))
        }
        PoolRebaseArm::ComputeIgnore => {
            super::energy::reset_weights(state);
            let reset = WeightSnapshot::of(state);
            restore(state);
            Some(reset)
        }
    }
}

/// Stable pose digest used on both sides of the disruption.
pub fn pose_digest(poses: &[Pose]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((POSE_DOMAIN.len() as u64).to_le_bytes());
    digest.update(POSE_DOMAIN);
    digest.update((poses.len() as u64).to_le_bytes());
    for pose in poses {
        digest.update(pose.tx_mm.to_bits().to_le_bytes());
        digest.update(pose.ty_mm.to_bits().to_le_bytes());
        digest.update(pose.theta_deg.to_bits().to_le_bytes());
        digest.update([u8::from(pose.mirrored)]);
    }
    digest.finalize().into()
}

/// Stable digest of the exact pose transforms applied by one disruption.
pub fn pose_transform_digest(before: &[Pose], after: &[Pose]) -> [u8; 32] {
    assert_eq!(before.len(), after.len());
    let mut digest = Sha256::new();
    digest.update((POSE_TRANSFORM_DOMAIN.len() as u64).to_le_bytes());
    digest.update(POSE_TRANSFORM_DOMAIN);
    digest.update((before.len() as u64).to_le_bytes());
    for (old, new) in before.iter().zip(after) {
        digest.update(old.tx_mm.to_bits().to_le_bytes());
        digest.update(old.ty_mm.to_bits().to_le_bytes());
        digest.update(old.theta_deg.to_bits().to_le_bytes());
        digest.update([u8::from(old.mirrored)]);
        digest.update(new.tx_mm.to_bits().to_le_bytes());
        digest.update(new.ty_mm.to_bits().to_le_bytes());
        digest.update(new.theta_deg.to_bits().to_le_bytes());
        digest.update([u8::from(new.mirrored)]);
    }
    digest.finalize().into()
}

/// Stable digest of authoritative raw rows. GLS weights are deliberately not
/// included: G0.2 requires this digest to agree while the weight digest differs.
pub fn raw_row_digest(state: &IcsState) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((RAW_ROW_DOMAIN.len() as u64).to_le_bytes());
    digest.update(RAW_ROW_DOMAIN);
    digest.update((state.pair_rows.len() as u64).to_le_bytes());
    for row in &state.pair_rows {
        digest.update(row.violation_mm.to_bits().to_le_bytes());
        digest.update(row.contact.signed_gap_mm.to_bits().to_le_bytes());
        for value in row.contact.normal {
            digest.update(value.to_bits().to_le_bytes());
        }
        for value in row.contact.witness_a {
            digest.update(value.to_bits().to_le_bytes());
        }
        for value in row.contact.witness_b {
            digest.update(value.to_bits().to_le_bytes());
        }
    }
    digest.update((state.edge_rows.len() as u64).to_le_bytes());
    for rows in &state.edge_rows {
        for row in rows {
            digest.update(row.violation_mm.to_bits().to_le_bytes());
            for value in row.witness {
                digest.update(value.to_bits().to_le_bytes());
            }
        }
    }
    digest.finalize().into()
}

/// One actual exploration-pool retry and the evidence around its sole branch.
#[derive(Clone, Debug)]
pub struct PoolRetryRecord {
    pub request_seed: u64,
    pub explore_bite_ordinal: u64,
    pub attempt_ordinal: u64,
    pub width_mm: f64,
    pub pool_length: usize,
    pub selected_rank: usize,
    pub pool_entry_raw_phi: f64,
    pub selected_pose_digest_sha256: [u8; 32],
    pub saved_weights: WeightSnapshot,
    pub post_install_pose_digest_sha256: [u8; 32],
    pub post_install_raw_row_digest_sha256: [u8; 32],
    /// The reset vector before ComputeIgnore restores the saved vector. Equal
    /// to `post_policy_weights` for Rebase and `None` for Saved.
    pub reset_weights: Option<WeightSnapshot>,
    pub post_policy_weights: WeightSnapshot,
    pub disruption: DisruptOutcome,
    pub disruption_work_delta: WorkVector,
    pub disruption_pose_transform_digest_sha256: [u8; 32],
    pub post_disruption_pose_digest_sha256: [u8; 32],
    pub post_disruption_raw_row_digest_sha256: [u8; 32],
    pub cold_post_disruption_raw_row_digest_sha256: [u8; 32],
    pub post_disruption_weights: WeightSnapshot,
    pub fingerprint_start: usize,
    pub fingerprint_end: usize,
    pub retry_iterations: u64,
    pub retry_stop: Option<&'static str>,
    pub retry_published: bool,
    pub path_work_delta: WorkVector,
    pub path_seconds: f64,
    pub valid: bool,
}

/// All retries observed by one trajectory. Empty when diagnostics are not
/// armed, even if the behavioral Rebase arm is active in a quality run.
#[derive(Clone, Debug)]
pub struct PoolRebaseTrace {
    pub arm: PoolRebaseArm,
    pub invalid_retries: u64,
    pub decisions: Vec<PoolRetryRecord>,
}

impl PoolRebaseTrace {
    pub fn new(arm: PoolRebaseArm) -> Self {
        Self {
            arm,
            invalid_retries: 0,
            decisions: Vec::new(),
        }
    }
}
