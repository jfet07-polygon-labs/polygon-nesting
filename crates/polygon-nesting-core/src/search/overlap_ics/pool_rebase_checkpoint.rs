//! Canonical fresh-process checkpoint for the first exploration-pool retry.
//!
//! This is intentionally a small local codec rather than `serde_json`: every
//! float is its raw `u64` bit pattern, every collection is length-delimited,
//! and field order is fixed by the encoder. The decoder must consume the whole
//! payload and canonical re-encoding must reproduce the input bytes exactly.

use sha2::{Digest, Sha256};

use crate::search::general_fast::{GeneralFastPiece, GeneralFastPlacement, GeneralFastSettings};
use crate::search::overlap_ics_meter::currency::{Currency, WorkTerms};
use crate::search::overlap_ics_meter::pacer::{NoClock, WorkPlanPacer, WorkPlanPacerSnapshotV1};
use crate::search::overlap_ics_meter::strike_meter::{ShadowCounters, StrikeConfig};

use super::contact::Contact;
use super::decomposition::Cell;
use super::descent::{Descent, DescentConfig, DescentSnapshotV1, RejectionCensus};
use super::diagnostics::{
    ExactCheckpoint, JumpEvent, ProxySample, QualityPoint, Trace, WorkVector,
};
use super::homotopy::Bite;
use super::icscal::{BinaryKey, CurrencyVersion, Executor, PlanKey};
use super::profile::PhaseProfile;
use super::publish::PublicationLimits;
use super::relocate::{CoordDescentStage, RelocateConfig};
use super::state::{
    Contract, EdgeRow, ExactIncumbent, Geometry, IcsState, PairRow, PieceSource, Pose,
};
use super::{
    BiteRecord, Engine, IcsConfig, IterationFingerprint, Pacer, Phase, PoolEntry, PublishedBite,
    ScheduleConfig, WorkOrdinal,
};

const DOMAIN: &[u8] = b"pool-retry-tracker-rebase/checkpoint/v2";
const ENVELOPE_DOMAIN: &[u8] = b"pool-retry-tracker-rebase/checkpoint-envelope/v2";
const SEAM: &[u8] = b"explore-after-first-pool-rank-before-install";
const MAX_SEQUENCE: usize = 20_000_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckpointBindings {
    pub spec_sha256: String,
    pub request_sha256: String,
    pub plan_sha256: String,
    pub executable_sha256: String,
    pub source_commit: String,
    pub features: Vec<String>,
    pub immutable_input_sha256: String,
}

#[derive(Clone, Debug)]
struct PacerSnapshotV1 {
    pub plan: WorkPlanPacerSnapshotV1,
    pub attempts_per_bite: u64,
    pub cursor: WorkTerms,
    pub opened_at: WorkTerms,
    pub charged: WorkTerms,
    pub explore_crossing_batch_units: u64,
    pub compress_crossing_batch_units: u64,
}

#[derive(Clone, Debug)]
struct EngineSnapshotV1 {
    pub state: IcsState,
    pub incumbent: ExactIncumbent,
    pub trace: Trace,
    pub config: IcsConfig,
    pub descent: DescentSnapshotV1,
    pub last_attempt_pose_digest: Option<[u8; 32]>,
}

#[derive(Clone, Debug)]
struct FirstPoolRetryCheckpointV1 {
    pub workers: usize,
    pub strikes: StrikeConfig,
    pub explore_time_ratio: f64,
    pub seed: u64,
    pub engine: EngineSnapshotV1,
    pub pacer: PacerSnapshotV1,
    pub start_depth_mm: f64,
    pub depth_mm: f64,
    pub width_mm: f64,
    pub parent_poses: Vec<Pose>,
    pub parent_fingerprint: String,
    pub bite_ordinal: u64,
    pub explore_bites: u64,
    pub bites: Vec<BiteRecord>,
    pub publications: Vec<PublishedBite>,
    pub fingerprints: Vec<IterationFingerprint>,
    pub record: BiteRecord,
    pub attempt: u64,
    pub pool: Vec<PoolEntry>,
    pub selected_rank: usize,
}

pub(super) struct RestoredFirstPoolRetry<'a> {
    pub engine: Engine<'a>,
    pub pacer: Pacer,
    pub workers: usize,
    pub strikes: StrikeConfig,
    pub seed: u64,
    pub start_depth_mm: f64,
    pub depth_mm: f64,
    pub width_mm: f64,
    pub parent_poses: Vec<Pose>,
    pub parent_fingerprint: String,
    pub bite_ordinal: u64,
    pub explore_bites: u64,
    pub bites: Vec<BiteRecord>,
    pub publications: Vec<PublishedBite>,
    pub fingerprints: Vec<IterationFingerprint>,
    pub record: BiteRecord,
    pub attempt: u64,
    pub pool: Vec<PoolEntry>,
    pub selected_rank: usize,
}

struct Encoder {
    bytes: Vec<u8>,
}

impl Encoder {
    fn new(domain: &[u8]) -> Self {
        let mut out = Self { bytes: Vec::new() };
        out.raw_bytes(domain);
        out
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn usize(&mut self, value: usize) {
        self.u64(value as u64);
    }

    fn f64(&mut self, value: f64) {
        self.u64(value.to_bits());
    }

    fn raw_bytes(&mut self, value: &[u8]) {
        self.u64(value.len() as u64);
        self.bytes.extend_from_slice(value);
    }

    fn string(&mut self, value: &str) {
        self.raw_bytes(value.as_bytes());
    }

    fn digest(&mut self, value: &[u8; 32]) {
        self.bytes.extend_from_slice(value);
    }
}

struct Decoder<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Decoder<'a> {
    fn new(bytes: &'a [u8], domain: &[u8]) -> Result<Self, String> {
        let mut input = Self { bytes, at: 0 };
        if input.raw_bytes()? != domain {
            return Err("pool-retry checkpoint domain mismatch".to_owned());
        }
        Ok(input)
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], String> {
        let end = self
            .at
            .checked_add(len)
            .ok_or_else(|| "checkpoint length overflow".to_owned())?;
        let value = self
            .bytes
            .get(self.at..end)
            .ok_or_else(|| "truncated pool-retry checkpoint".to_owned())?;
        self.at = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn bool(&mut self) -> Result<bool, String> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(format!("invalid checkpoint boolean {other}")),
        }
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn usize(&mut self) -> Result<usize, String> {
        usize::try_from(self.u64()?).map_err(|_| "checkpoint usize overflow".to_owned())
    }

    fn len(&mut self) -> Result<usize, String> {
        let len = self.usize()?;
        if len > MAX_SEQUENCE {
            return Err(format!("checkpoint sequence length {len} exceeds guard"));
        }
        Ok(len)
    }

    fn f64(&mut self) -> Result<f64, String> {
        Ok(f64::from_bits(self.u64()?))
    }

    fn raw_bytes(&mut self) -> Result<&'a [u8], String> {
        let len = self.len()?;
        self.take(len)
    }

    fn string(&mut self) -> Result<String, String> {
        String::from_utf8(self.raw_bytes()?.to_vec())
            .map_err(|_| "checkpoint string is not UTF-8".to_owned())
    }

    fn digest(&mut self) -> Result<[u8; 32], String> {
        Ok(self.take(32)?.try_into().unwrap())
    }

    fn finish(self) -> Result<(), String> {
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err(format!(
                "checkpoint has {} trailing bytes",
                self.bytes.len() - self.at
            ))
        }
    }
}

fn encode_pose(out: &mut Encoder, piece: usize, pose: Pose) {
    out.usize(piece);
    out.f64(pose.tx_mm);
    out.f64(pose.ty_mm);
    out.f64(pose.theta_deg);
    out.bool(pose.mirrored);
}

fn decode_pose(input: &mut Decoder<'_>, expected_piece: usize) -> Result<Pose, String> {
    if input.usize()? != expected_piece {
        return Err("checkpoint pose ID/order mismatch".to_owned());
    }
    Ok(Pose {
        tx_mm: input.f64()?,
        ty_mm: input.f64()?,
        theta_deg: input.f64()?,
        mirrored: input.bool()?,
    })
}

fn encode_poses(out: &mut Encoder, poses: &[Pose]) {
    out.usize(poses.len());
    for (piece, pose) in poses.iter().copied().enumerate() {
        encode_pose(out, piece, pose);
    }
}

fn decode_poses(input: &mut Decoder<'_>) -> Result<Vec<Pose>, String> {
    let len = input.len()?;
    (0..len).map(|piece| decode_pose(input, piece)).collect()
}

fn encode_work(out: &mut Encoder, work: WorkVector) {
    for value in [
        work.pair_row_probes,
        work.convex_cell_gap_queries,
        work.pose_transforms,
        work.jump_proposals,
        work.exact_checkpoints,
        work.repair_rows,
        work.piece_proposals,
        work.accepted_moves,
        work.weight_updates,
        work.broad_phase_rejects,
        work.sample_evaluations,
        work.relocates,
        work.focused_samples,
        work.container_samples,
        work.container_winners,
        work.focused_winners,
        work.stay_put_winners,
        work.container_commits,
        work.disruptions,
        work.disruption_moves,
    ] {
        out.u64(value);
    }
}

fn decode_work(input: &mut Decoder<'_>) -> Result<WorkVector, String> {
    Ok(WorkVector {
        pair_row_probes: input.u64()?,
        convex_cell_gap_queries: input.u64()?,
        pose_transforms: input.u64()?,
        jump_proposals: input.u64()?,
        exact_checkpoints: input.u64()?,
        repair_rows: input.u64()?,
        piece_proposals: input.u64()?,
        accepted_moves: input.u64()?,
        weight_updates: input.u64()?,
        broad_phase_rejects: input.u64()?,
        sample_evaluations: input.u64()?,
        relocates: input.u64()?,
        focused_samples: input.u64()?,
        container_samples: input.u64()?,
        container_winners: input.u64()?,
        focused_winners: input.u64()?,
        stay_put_winners: input.u64()?,
        container_commits: input.u64()?,
        disruptions: input.u64()?,
        disruption_moves: input.u64()?,
    })
}

fn encode_terms(out: &mut Encoder, terms: WorkTerms) {
    out.u64(terms.sample_evaluations);
    out.u64(terms.master_batches);
    out.u64(terms.actual_publication_attempt_calls);
    out.u64(terms.repair_rows);
    out.u64(terms.disruption_moves);
}

fn decode_terms(input: &mut Decoder<'_>) -> Result<WorkTerms, String> {
    Ok(WorkTerms {
        sample_evaluations: input.u64()?,
        master_batches: input.u64()?,
        actual_publication_attempt_calls: input.u64()?,
        repair_rows: input.u64()?,
        disruption_moves: input.u64()?,
    })
}

fn encode_string_vec(out: &mut Encoder, values: &[String]) {
    out.usize(values.len());
    for value in values {
        out.string(value);
    }
}

fn decode_string_vec(input: &mut Decoder<'_>) -> Result<Vec<String>, String> {
    let len = input.len()?;
    (0..len).map(|_| input.string()).collect()
}

fn encode_point<const N: usize>(out: &mut Encoder, values: [f64; N]) {
    for value in values {
        out.f64(value);
    }
}

fn decode_point<const N: usize>(input: &mut Decoder<'_>) -> Result<[f64; N], String> {
    let mut values = [0.0; N];
    for value in &mut values {
        *value = input.f64()?;
    }
    Ok(values)
}

fn encode_geometry(out: &mut Encoder, geometry: &Geometry) {
    out.usize(geometry.cell_points.len());
    for point in &geometry.cell_points {
        encode_point(out, *point);
    }
    out.usize(geometry.cells.len());
    for (id, cell) in geometry.cells.iter().enumerate() {
        out.usize(id);
        out.usize(cell.start);
        out.usize(cell.len);
    }
    out.usize(geometry.cell_bounds.len());
    for bounds in &geometry.cell_bounds {
        encode_point(out, *bounds);
    }
    out.usize(geometry.piece_cells.len());
    for (piece, (start, end)) in geometry.piece_cells.iter().enumerate() {
        out.usize(piece);
        out.usize(*start);
        out.usize(*end);
    }
    out.usize(geometry.ring_points.len());
    for point in &geometry.ring_points {
        encode_point(out, *point);
    }
    out.usize(geometry.piece_rings.len());
    for (piece, (start, end)) in geometry.piece_rings.iter().enumerate() {
        out.usize(piece);
        out.usize(*start);
        out.usize(*end);
    }
    out.usize(geometry.piece_bounds.len());
    for bounds in &geometry.piece_bounds {
        encode_point(out, *bounds);
    }
    out.usize(geometry.centroids.len());
    for point in &geometry.centroids {
        encode_point(out, *point);
    }
}

fn decode_geometry(input: &mut Decoder<'_>) -> Result<Geometry, String> {
    let cell_points = (0..input.len()?)
        .map(|_| decode_point(input))
        .collect::<Result<Vec<[f64; 2]>, _>>()?;
    let cell_len = input.len()?;
    let mut cells = Vec::with_capacity(cell_len);
    for id in 0..cell_len {
        if input.usize()? != id {
            return Err("checkpoint cell ID/order mismatch".to_owned());
        }
        cells.push(Cell {
            start: input.usize()?,
            len: input.usize()?,
        });
    }
    let cell_bounds = (0..input.len()?)
        .map(|_| decode_point(input))
        .collect::<Result<Vec<[f64; 4]>, _>>()?;
    let piece_cell_len = input.len()?;
    let mut piece_cells = Vec::with_capacity(piece_cell_len);
    for piece in 0..piece_cell_len {
        if input.usize()? != piece {
            return Err("checkpoint piece-cell ID/order mismatch".to_owned());
        }
        piece_cells.push((input.usize()?, input.usize()?));
    }
    let ring_points = (0..input.len()?)
        .map(|_| decode_point(input))
        .collect::<Result<Vec<[f64; 2]>, _>>()?;
    let piece_ring_len = input.len()?;
    let mut piece_rings = Vec::with_capacity(piece_ring_len);
    for piece in 0..piece_ring_len {
        if input.usize()? != piece {
            return Err("checkpoint piece-ring ID/order mismatch".to_owned());
        }
        piece_rings.push((input.usize()?, input.usize()?));
    }
    let piece_bounds = (0..input.len()?)
        .map(|_| decode_point(input))
        .collect::<Result<Vec<[f64; 4]>, _>>()?;
    let centroids = (0..input.len()?)
        .map(|_| decode_point(input))
        .collect::<Result<Vec<[f64; 2]>, _>>()?;
    let cell_total = cell_points.len();
    let cell_count = cells.len();
    Ok(Geometry {
        cell_axes: vec![[0.0; 2]; cell_total],
        cell_own: vec![[0.0; 2]; cell_total],
        cell_axes_valid: vec![false; cell_count],
        cell_points,
        cells,
        cell_bounds,
        piece_cells,
        ring_points,
        piece_rings,
        piece_bounds,
        centroids,
    })
}

fn encode_contact(out: &mut Encoder, contact: Contact) {
    out.f64(contact.signed_gap_mm);
    encode_point(out, contact.normal);
    encode_point(out, contact.witness_a);
    encode_point(out, contact.witness_b);
}

fn decode_contact(input: &mut Decoder<'_>) -> Result<Contact, String> {
    Ok(Contact {
        signed_gap_mm: input.f64()?,
        normal: decode_point(input)?,
        witness_a: decode_point(input)?,
        witness_b: decode_point(input)?,
    })
}

fn encode_state(out: &mut Encoder, state: &IcsState) {
    encode_poses(out, &state.poses);
    encode_geometry(out, &state.geometry);
    out.usize(state.pair_rows.len());
    let mut pair = 0usize;
    for first in 0..state.poses.len() {
        for second in first + 1..state.poses.len() {
            let row = &state.pair_rows[pair];
            out.usize(pair);
            out.usize(first);
            out.usize(second);
            out.f64(row.violation_mm);
            out.f64(row.weight);
            encode_contact(out, row.contact);
            pair += 1;
        }
    }
    assert_eq!(pair, state.pair_rows.len());
    out.usize(state.edge_rows.len());
    for (piece, rows) in state.edge_rows.iter().enumerate() {
        out.usize(piece);
        for (side, row) in rows.iter().enumerate() {
            out.usize(side);
            out.f64(row.violation_mm);
            out.f64(row.weight);
            encode_point(out, row.witness);
        }
    }
    out.f64(state.target_depth_mm);
}

fn decode_state(input: &mut Decoder<'_>) -> Result<IcsState, String> {
    let poses = decode_poses(input)?;
    let geometry = decode_geometry(input)?;
    let pair_len = input.len()?;
    let expected_pairs = poses.len() * poses.len().saturating_sub(1) / 2;
    if pair_len != expected_pairs {
        return Err("checkpoint pair-row count does not match piece count".to_owned());
    }
    let mut pair_rows = Vec::with_capacity(pair_len);
    let mut pair = 0usize;
    for first in 0..poses.len() {
        for second in first + 1..poses.len() {
            if input.usize()? != pair || input.usize()? != first || input.usize()? != second {
                return Err("checkpoint pair row ID/endpoints mismatch".to_owned());
            }
            pair_rows.push(PairRow {
                violation_mm: input.f64()?,
                weight: input.f64()?,
                contact: decode_contact(input)?,
            });
            pair += 1;
        }
    }
    let edge_len = input.len()?;
    if edge_len != poses.len() {
        return Err("checkpoint edge-row piece count mismatch".to_owned());
    }
    let mut edge_rows = Vec::with_capacity(edge_len);
    for piece in 0..edge_len {
        if input.usize()? != piece {
            return Err("checkpoint edge piece ID/order mismatch".to_owned());
        }
        let mut rows = [EdgeRow::default(); 4];
        for (side, row) in rows.iter_mut().enumerate() {
            if input.usize()? != side {
                return Err("checkpoint edge side ID/order mismatch".to_owned());
            }
            *row = EdgeRow {
                violation_mm: input.f64()?,
                weight: input.f64()?,
                witness: decode_point(input)?,
            };
        }
        edge_rows.push(rows);
    }
    Ok(IcsState {
        poses,
        geometry,
        pair_rows,
        edge_rows,
        target_depth_mm: input.f64()?,
        near: vec![Vec::new(); count],
    })
}

fn encode_placement(out: &mut Encoder, placement: &GeneralFastPlacement) {
    out.string(&placement.piece_id);
    out.f64(placement.rotation_deg);
    out.bool(placement.mirrored);
    out.f64(placement.translate_short_axis);
    out.f64(placement.translate_long_axis);
}

fn decode_placement(input: &mut Decoder<'_>) -> Result<GeneralFastPlacement, String> {
    Ok(GeneralFastPlacement {
        piece_id: input.string()?,
        rotation_deg: input.f64()?,
        mirrored: input.bool()?,
        translate_short_axis: input.f64()?,
        translate_long_axis: input.f64()?,
    })
}

fn encode_incumbent(out: &mut Encoder, incumbent: &ExactIncumbent) {
    out.usize(incumbent.placements.len());
    for placement in &incumbent.placements {
        encode_placement(out, placement);
    }
    out.f64(incumbent.raw_source_depth_mm);
    out.bool(incumbent.from_constructor);
    out.string(&incumbent.placement_fingerprint);
}

fn decode_incumbent(input: &mut Decoder<'_>) -> Result<ExactIncumbent, String> {
    let placements = (0..input.len()?)
        .map(|_| decode_placement(input))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ExactIncumbent {
        placements,
        raw_source_depth_mm: input.f64()?,
        from_constructor: input.bool()?,
        placement_fingerprint: input.string()?,
    })
}

fn encode_relocate(out: &mut Encoder, config: RelocateConfig) {
    out.usize(config.focused_samples);
    out.usize(config.container_samples);
    out.usize(config.sampled_orientations);
    out.usize(config.finalists);
    out.f64(config.unique_translation_ratio);
    out.f64(config.unique_angle_deg);
    for stage in [config.coarse, config.fine] {
        out.f64(stage.translation_init_ratio);
        out.f64(stage.translation_limit_ratio);
        out.f64(stage.rotation_init_deg);
        out.f64(stage.rotation_limit_deg);
    }
    out.f64(config.step_success);
    out.f64(config.step_fail);
}

fn decode_relocate(input: &mut Decoder<'_>) -> Result<RelocateConfig, String> {
    let focused_samples = input.usize()?;
    let container_samples = input.usize()?;
    let sampled_orientations = input.usize()?;
    let finalists = input.usize()?;
    let unique_translation_ratio = input.f64()?;
    let unique_angle_deg = input.f64()?;
    let stage = |input: &mut Decoder<'_>| -> Result<CoordDescentStage, String> {
        Ok(CoordDescentStage {
            translation_init_ratio: input.f64()?,
            translation_limit_ratio: input.f64()?,
            rotation_init_deg: input.f64()?,
            rotation_limit_deg: input.f64()?,
        })
    };
    Ok(RelocateConfig {
        focused_samples,
        container_samples,
        sampled_orientations,
        finalists,
        unique_translation_ratio,
        unique_angle_deg,
        coarse: stage(input)?,
        fine: stage(input)?,
        step_success: input.f64()?,
        step_fail: input.f64()?,
    })
}

fn encode_descent_config(out: &mut Encoder, config: DescentConfig) {
    out.f64(config.ladder_top_mm);
    encode_relocate(out, config.relocate);
    out.u64(config.seed);
    out.u32(config.jump_allowance);
    out.u32(config.stalls_before_jump);
    out.usize(config.rejection_census_samples);
    out.bool(config.jump_commits_unconditionally);
}

fn decode_descent_config(input: &mut Decoder<'_>) -> Result<DescentConfig, String> {
    Ok(DescentConfig {
        ladder_top_mm: input.f64()?,
        relocate: decode_relocate(input)?,
        seed: input.u64()?,
        jump_allowance: input.u32()?,
        stalls_before_jump: input.u32()?,
        rejection_census_samples: input.usize()?,
        jump_commits_unconditionally: input.bool()?,
    })
}

fn encode_limits(out: &mut Encoder, limits: PublicationLimits) {
    out.f64(limits.band_mm);
    out.f64(limits.epsilon_grid_mm);
    out.f64(limits.max_piece_displacement_mm);
    out.usize(limits.repair_rows_per_piece);
    out.f64(limits.minimum_improvement_mm);
}

fn decode_limits(input: &mut Decoder<'_>) -> Result<PublicationLimits, String> {
    Ok(PublicationLimits {
        band_mm: input.f64()?,
        epsilon_grid_mm: input.f64()?,
        max_piece_displacement_mm: input.f64()?,
        repair_rows_per_piece: input.usize()?,
        minimum_improvement_mm: input.f64()?,
    })
}

fn encode_config(out: &mut Encoder, config: IcsConfig) {
    out.f64(config.target_depth_mm);
    out.u64(config.proposal_budget);
    out.u64(config.relocate_eval_budget);
    out.u64(config.checkpoint_every_sweeps);
    encode_descent_config(out, config.descent);
    encode_limits(out, config.limits);
}

fn decode_config(input: &mut Decoder<'_>) -> Result<IcsConfig, String> {
    Ok(IcsConfig {
        target_depth_mm: input.f64()?,
        proposal_budget: input.u64()?,
        relocate_eval_budget: input.u64()?,
        checkpoint_every_sweeps: input.u64()?,
        descent: decode_descent_config(input)?,
        limits: decode_limits(input)?,
    })
}

fn encode_profile(out: &mut Encoder, profile: PhaseProfile) {
    for value in [
        profile.iterations,
        profile.barrier_to_barrier_ns,
        profile.prep_ns,
        profile.dispatch_ns,
        profile.sweep_critical_ns,
        profile.sweep_total_ns,
        profile.merge_gls_ns,
        profile.exact_ns,
        profile.band_fold_ns,
        profile.snapshot_ns,
        profile.band_entries,
        profile.exact_calls,
        profile.sample_evaluations,
        profile.repair_rows,
        profile.disruption_moves,
    ] {
        out.u64(value);
    }
}

fn decode_profile(input: &mut Decoder<'_>) -> Result<PhaseProfile, String> {
    Ok(PhaseProfile {
        iterations: input.u64()?,
        barrier_to_barrier_ns: input.u64()?,
        prep_ns: input.u64()?,
        dispatch_ns: input.u64()?,
        sweep_critical_ns: input.u64()?,
        sweep_total_ns: input.u64()?,
        merge_gls_ns: input.u64()?,
        exact_ns: input.u64()?,
        band_fold_ns: input.u64()?,
        snapshot_ns: input.u64()?,
        band_entries: input.u64()?,
        exact_calls: input.u64()?,
        sample_evaluations: input.u64()?,
        repair_rows: input.u64()?,
        disruption_moves: input.u64()?,
    })
}

fn encode_shadow(out: &mut Encoder, shadow: ShadowCounters) {
    out.u64(shadow.batches);
    out.u64(shadow.charged_work);
    out.u64(shadow.substantial);
    out.u64(shadow.marginal);
    out.u64(shadow.none);
}

fn decode_shadow(input: &mut Decoder<'_>) -> Result<ShadowCounters, String> {
    Ok(ShadowCounters {
        batches: input.u64()?,
        charged_work: input.u64()?,
        substantial: input.u64()?,
        marginal: input.u64()?,
        none: input.u64()?,
    })
}

fn encode_trace(out: &mut Encoder, trace: &Trace) {
    encode_work(out, trace.work);
    out.usize(trace.checkpoints.len());
    for row in &trace.checkpoints {
        out.u64(row.proposal_ordinal);
        out.f64(row.target_depth_mm);
        out.f64(row.max_violation_mm);
        out.f64(row.proxy_raw_depth_mm);
        out.bool(row.kernel_exclusive_valid);
        out.bool(row.contract_valid);
        out.u64(row.repair_rows);
        out.f64(row.repair_max_displacement_mm);
        out.f64(row.repair_depth_giveback_mm);
        out.bool(row.published_raw_depth_mm.is_some());
        if let Some(value) = row.published_raw_depth_mm {
            out.f64(value);
        }
        out.bool(row.refusal.is_some());
        if let Some(value) = &row.refusal {
            out.string(value);
        }
    }
    out.usize(trace.quality.len());
    for row in &trace.quality {
        out.u64(row.proposal_ordinal);
        out.f64(row.raw_source_depth_mm);
        out.bool(row.strict_child);
    }
    out.usize(trace.proxy_samples.len());
    for row in &trace.proxy_samples {
        out.u64(row.proposal_ordinal);
        out.f64(row.target_depth_mm);
        out.f64(row.raw_phi);
        out.f64(row.guided_phi);
        out.f64(row.max_violation_mm);
        out.f64(row.raw_source_depth_mm);
    }
    for value in [
        trace.sweeps,
        trace.guided_stalls,
        trace.jumps,
        trace.jump_attempted,
        trace.jump_committed,
        trace.jumps_improving_guided,
    ] {
        out.u64(value);
    }
    out.usize(trace.jump_events.len());
    for row in &trace.jump_events {
        out.u64(row.proposal_ordinal);
        out.usize(row.piece);
        out.string(row.kind);
        out.f64(row.radius_mm);
        out.f64(row.max_violation_mm);
        out.f64(row.baseline_guided);
        out.f64(row.best_guided);
        out.bool(row.installed);
        out.bool(row.improved_guided);
    }
}

fn decode_trace(input: &mut Decoder<'_>) -> Result<Trace, String> {
    let work = decode_work(input)?;
    let checkpoint_len = input.len()?;
    let mut checkpoints = Vec::with_capacity(checkpoint_len);
    for _ in 0..checkpoint_len {
        let proposal_ordinal = input.u64()?;
        let target_depth_mm = input.f64()?;
        let max_violation_mm = input.f64()?;
        let proxy_raw_depth_mm = input.f64()?;
        let kernel_exclusive_valid = input.bool()?;
        let contract_valid = input.bool()?;
        let repair_rows = input.u64()?;
        let repair_max_displacement_mm = input.f64()?;
        let repair_depth_giveback_mm = input.f64()?;
        let published_raw_depth_mm = input.bool()?.then(|| input.f64()).transpose()?;
        let refusal = input.bool()?.then(|| input.string()).transpose()?;
        checkpoints.push(ExactCheckpoint {
            proposal_ordinal,
            target_depth_mm,
            max_violation_mm,
            proxy_raw_depth_mm,
            kernel_exclusive_valid,
            contract_valid,
            repair_rows,
            repair_max_displacement_mm,
            repair_depth_giveback_mm,
            published_raw_depth_mm,
            refusal,
        });
    }
    let quality_len = input.len()?;
    let mut quality = Vec::with_capacity(quality_len);
    for _ in 0..quality_len {
        quality.push(QualityPoint {
            proposal_ordinal: input.u64()?,
            raw_source_depth_mm: input.f64()?,
            strict_child: input.bool()?,
        });
    }
    let proxy_len = input.len()?;
    let mut proxy_samples = Vec::with_capacity(proxy_len);
    for _ in 0..proxy_len {
        proxy_samples.push(ProxySample {
            proposal_ordinal: input.u64()?,
            target_depth_mm: input.f64()?,
            raw_phi: input.f64()?,
            guided_phi: input.f64()?,
            max_violation_mm: input.f64()?,
            raw_source_depth_mm: input.f64()?,
        });
    }
    let sweeps = input.u64()?;
    let guided_stalls = input.u64()?;
    let jumps = input.u64()?;
    let jump_attempted = input.u64()?;
    let jump_committed = input.u64()?;
    let jumps_improving_guided = input.u64()?;
    let event_len = input.len()?;
    let mut jump_events = Vec::with_capacity(event_len);
    for _ in 0..event_len {
        let proposal_ordinal = input.u64()?;
        let piece = input.usize()?;
        let kind = match input.string()?.as_str() {
            "strip" => "strip",
            "ball" => "ball",
            other => return Err(format!("unknown checkpoint jump kind `{other}`")),
        };
        jump_events.push(JumpEvent {
            proposal_ordinal,
            piece,
            kind,
            radius_mm: input.f64()?,
            max_violation_mm: input.f64()?,
            baseline_guided: input.f64()?,
            best_guided: input.f64()?,
            installed: input.bool()?,
            improved_guided: input.bool()?,
        });
    }
    Ok(Trace {
        work,
        checkpoints,
        quality,
        proxy_samples,
        sweeps,
        guided_stalls,
        jumps,
        jump_attempted,
        jump_committed,
        jumps_improving_guided,
        jump_events,
    })
}

fn encode_census(out: &mut Encoder, census: &RejectionCensus) {
    out.u64(census.accepted);
    out.u64(census.rejected);
    out.u64(census.zero_energy);
    for value in census.accepted_by_class {
        out.u64(value);
    }
    for value in census.rejected_by_class {
        out.u64(value);
    }
    out.bool(census.armed);
    debug_assert!(
        census.records.is_empty(),
        "retired rejection-rung records cannot enter a pool-retry checkpoint"
    );
    out.usize(0);
    for value in census.accepted_by_origin {
        out.u64(value);
    }
    for value in census.rejected_by_origin {
        out.u64(value);
    }
    out.f64(census.max_displacement_mm);
}

fn decode_census(input: &mut Decoder<'_>) -> Result<RejectionCensus, String> {
    let accepted = input.u64()?;
    let rejected = input.u64()?;
    let zero_energy = input.u64()?;
    let mut accepted_by_class = [0; 3];
    for value in &mut accepted_by_class {
        *value = input.u64()?;
    }
    let mut rejected_by_class = [0; 3];
    for value in &mut rejected_by_class {
        *value = input.u64()?;
    }
    let armed = input.bool()?;
    if input.len()? != 0 {
        return Err("retired rejection-rung record in checkpoint".to_owned());
    }
    let mut accepted_by_origin = [0; 3];
    for value in &mut accepted_by_origin {
        *value = input.u64()?;
    }
    let mut rejected_by_origin = [0; 3];
    for value in &mut rejected_by_origin {
        *value = input.u64()?;
    }
    Ok(RejectionCensus {
        accepted,
        rejected,
        zero_energy,
        accepted_by_class,
        rejected_by_class,
        armed,
        records: Vec::new(),
        accepted_by_origin,
        rejected_by_origin,
        max_displacement_mm: input.f64()?,
    })
}

fn encode_descent(out: &mut Encoder, descent: &DescentSnapshotV1) {
    encode_descent_config(out, descent.config);
    out.usize(descent.order.len());
    for piece in &descent.order {
        out.usize(*piece);
    }
    out.usize(descent.allow_rotation.len());
    for allowed in &descent.allow_rotation {
        out.bool(*allowed);
    }
    out.u64(descent.proposals);
    out.u64(descent.bite);
    out.u64(descent.worker);
    out.u64(descent.iteration);
    encode_census(out, &descent.census);
}

fn decode_descent(input: &mut Decoder<'_>) -> Result<DescentSnapshotV1, String> {
    let config = decode_descent_config(input)?;
    let order = (0..input.len()?)
        .map(|_| input.usize())
        .collect::<Result<Vec<_>, _>>()?;
    let allow_rotation = (0..input.len()?)
        .map(|_| input.bool())
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DescentSnapshotV1 {
        config,
        order,
        allow_rotation,
        proposals: input.u64()?,
        bite: input.u64()?,
        worker: input.u64()?,
        iteration: input.u64()?,
        census: decode_census(input)?,
    })
}

fn encode_plan_key(out: &mut Encoder, key: &PlanKey) {
    out.string(&key.request_sha256);
    out.u8(match key.currency_version {
        CurrencyVersion::U0Samples => 0,
        CurrencyVersion::U1Weighted => 1,
    });
    out.string(&key.binary_key.executable_sha256);
    encode_string_vec(out, &key.binary_key.features);
    out.usize(key.workers);
    out.u8(match key.executor {
        Executor::EphemeralScope => 0,
        Executor::PersistentPool => 1,
    });
}

fn decode_plan_key(input: &mut Decoder<'_>) -> Result<PlanKey, String> {
    let request_sha256 = input.string()?;
    let currency_version = match input.u8()? {
        0 => CurrencyVersion::U0Samples,
        1 => CurrencyVersion::U1Weighted,
        other => return Err(format!("unknown checkpoint currency version {other}")),
    };
    let executable_sha256 = input.string()?;
    let features = decode_string_vec(input)?;
    let workers = input.usize()?;
    let executor = match input.u8()? {
        0 => Executor::EphemeralScope,
        1 => Executor::PersistentPool,
        other => return Err(format!("unknown checkpoint executor {other}")),
    };
    Ok(PlanKey {
        request_sha256,
        currency_version,
        binary_key: BinaryKey {
            executable_sha256,
            features,
        },
        workers,
        executor,
    })
}

fn encode_pacer(out: &mut Encoder, pacer: &PacerSnapshotV1) {
    encode_plan_key(out, &pacer.plan.key);
    // The signed Gate uses U0. Encoding a U1 coefficient table here would
    // widen the reviewed surface for a path the exact spec forbids.
    assert_eq!(pacer.plan.currency, Currency::U0);
    out.u8(0);
    out.u64(pacer.plan.explore_allocation);
    out.u64(pacer.plan.compress_allocation);
    out.u64(pacer.plan.explore_consumed);
    out.u64(pacer.plan.compress_consumed);
    out.u64(pacer.plan.explore_batches);
    out.u64(pacer.plan.compress_batches);
    out.f64(pacer.plan.budget_seconds);
    out.f64(pacer.plan.explore_ratio);
    out.u64(pacer.attempts_per_bite);
    encode_terms(out, pacer.cursor);
    encode_terms(out, pacer.opened_at);
    encode_terms(out, pacer.charged);
    out.u64(pacer.explore_crossing_batch_units);
    out.u64(pacer.compress_crossing_batch_units);
}

fn decode_pacer(input: &mut Decoder<'_>) -> Result<PacerSnapshotV1, String> {
    let key = decode_plan_key(input)?;
    if input.u8()? != 0 || key.currency_version != CurrencyVersion::U0Samples {
        return Err("pool-retry Gate checkpoint must use U0".to_owned());
    }
    Ok(PacerSnapshotV1 {
        plan: WorkPlanPacerSnapshotV1 {
            key,
            currency: Currency::U0,
            explore_allocation: input.u64()?,
            compress_allocation: input.u64()?,
            explore_consumed: input.u64()?,
            compress_consumed: input.u64()?,
            explore_batches: input.u64()?,
            compress_batches: input.u64()?,
            budget_seconds: input.f64()?,
            explore_ratio: input.f64()?,
        },
        attempts_per_bite: input.u64()?,
        cursor: decode_terms(input)?,
        opened_at: decode_terms(input)?,
        charged: decode_terms(input)?,
        explore_crossing_batch_units: input.u64()?,
        compress_crossing_batch_units: input.u64()?,
    })
}

fn encode_strikes(out: &mut Encoder, strikes: StrikeConfig) {
    match strikes {
        StrikeConfig::IterationStrikes { explore, compress } => {
            out.u8(0);
            out.u64(explore.iterations_without_improvement);
            out.u32(explore.strikes);
            out.u64(compress.iterations_without_improvement);
            out.u32(compress.strikes);
        }
        StrikeConfig::WorkStrikes {
            explore_quantum,
            compress_quantum,
            explore_strikes,
            compress_strikes,
        } => {
            out.u8(1);
            out.u64(explore_quantum);
            out.u64(compress_quantum);
            out.u32(explore_strikes);
            out.u32(compress_strikes);
        }
    }
}

fn decode_strikes(input: &mut Decoder<'_>) -> Result<StrikeConfig, String> {
    Ok(match input.u8()? {
        0 => StrikeConfig::IterationStrikes {
            explore: super::SeparateLimits {
                iterations_without_improvement: input.u64()?,
                strikes: input.u32()?,
            },
            compress: super::SeparateLimits {
                iterations_without_improvement: input.u64()?,
                strikes: input.u32()?,
            },
        },
        1 => StrikeConfig::WorkStrikes {
            explore_quantum: input.u64()?,
            compress_quantum: input.u64()?,
            explore_strikes: input.u32()?,
            compress_strikes: input.u32()?,
        },
        other => return Err(format!("unknown checkpoint strike arm {other}")),
    })
}

fn encode_phase(out: &mut Encoder, phase: Phase) {
    out.u8(match phase {
        Phase::Explore => 0,
        Phase::Compress => 1,
    });
}

fn decode_phase(input: &mut Decoder<'_>) -> Result<Phase, String> {
    match input.u8()? {
        0 => Ok(Phase::Explore),
        1 => Ok(Phase::Compress),
        other => Err(format!("unknown checkpoint phase {other}")),
    }
}

fn encode_work_ordinal(out: &mut Encoder, row: WorkOrdinal) {
    out.u64(row.bite);
    out.u64(row.attempt);
    out.u64(row.iteration);
    out.u64(row.proposals);
}

fn decode_work_ordinal(input: &mut Decoder<'_>) -> Result<WorkOrdinal, String> {
    Ok(WorkOrdinal {
        bite: input.u64()?,
        attempt: input.u64()?,
        iteration: input.u64()?,
        proposals: input.u64()?,
    })
}

fn encode_published(out: &mut Encoder, row: &PublishedBite) {
    encode_work_ordinal(out, row.ordinal);
    encode_phase(out, row.phase);
    out.f64(row.target_depth_mm);
    out.f64(row.published_raw_depth_mm);
    out.u64(row.repair_rows);
    out.f64(row.repair_max_displacement_mm);
    out.f64(row.repair_depth_giveback_mm);
    out.string(&row.parent_fingerprint);
    out.string(&row.placement_fingerprint);
    out.bool(row.improved_incumbent);
    out.bool(row.wall_seconds.is_some());
    if let Some(value) = row.wall_seconds {
        out.f64(value);
    }
    encode_poses(out, &row.poses);
}

fn decode_published(input: &mut Decoder<'_>) -> Result<PublishedBite, String> {
    Ok(PublishedBite {
        ordinal: decode_work_ordinal(input)?,
        phase: decode_phase(input)?,
        target_depth_mm: input.f64()?,
        published_raw_depth_mm: input.f64()?,
        repair_rows: input.u64()?,
        repair_max_displacement_mm: input.f64()?,
        repair_depth_giveback_mm: input.f64()?,
        parent_fingerprint: input.string()?,
        placement_fingerprint: input.string()?,
        improved_incumbent: input.bool()?,
        wall_seconds: input.bool()?.then(|| input.f64()).transpose()?,
        poses: decode_poses(input)?,
    })
}

fn encode_bite(out: &mut Encoder, bite: &Bite) {
    out.f64(bite.width_before_mm);
    out.f64(bite.width_after_mm);
    out.f64(bite.delta_mm);
    out.f64(bite.split_y_mm);
    out.usize(bite.moved_pieces);
    out.f64(bite.step);
}

fn decode_bite(input: &mut Decoder<'_>) -> Result<Bite, String> {
    let width_before_mm = input.f64()?;
    let width_after_mm = input.f64()?;
    let delta_mm = input.f64()?;
    let split_y_mm = input.f64()?;
    let moved_pieces = input.usize()?;
    Ok(Bite {
        width_before_mm,
        width_after_mm,
        delta_mm,
        split_y_mm,
        moved_pieces,
        step: input.f64()?,
    })
}

fn encode_bite_record(out: &mut Encoder, row: &BiteRecord) {
    out.u64(row.ordinal);
    encode_phase(out, row.phase);
    encode_bite(out, &row.bite);
    out.u64(row.attempts);
    out.u64(row.disruptions);
    out.u64(row.master_iterations);
    out.u32(row.strikes);
    out.f64(row.min_raw_phi);
    out.bool(row.proxy_band_reached);
    out.u64(row.exact_band_entries);
    out.u64(row.exact_checkpoint_calls);
    encode_profile(out, row.profile);
    encode_shadow(out, row.strike_shadow);
    out.u64(row.strike_accumulated);
    out.u64(row.strike_overshoot);
    out.bool(row.published.is_some());
    if let Some(value) = &row.published {
        encode_published(out, value);
    }
}

fn decode_bite_record(input: &mut Decoder<'_>) -> Result<BiteRecord, String> {
    Ok(BiteRecord {
        ordinal: input.u64()?,
        phase: decode_phase(input)?,
        bite: decode_bite(input)?,
        attempts: input.u64()?,
        disruptions: input.u64()?,
        master_iterations: input.u64()?,
        strikes: input.u32()?,
        min_raw_phi: input.f64()?,
        proxy_band_reached: input.bool()?,
        exact_band_entries: input.u64()?,
        exact_checkpoint_calls: input.u64()?,
        profile: decode_profile(input)?,
        strike_shadow: decode_shadow(input)?,
        strike_accumulated: input.u64()?,
        strike_overshoot: input.u64()?,
        published: input.bool()?.then(|| decode_published(input)).transpose()?,
    })
}

fn encode_fingerprint(out: &mut Encoder, row: &IterationFingerprint) {
    out.u64(row.bite);
    out.u64(row.attempt);
    out.u64(row.iteration);
    out.usize(row.winner);
    out.f64(row.winner_guided);
    out.bool(row.contested);
    out.string(&row.state);
    out.bool(row.committed_pose_digest_sha256.is_some());
    if let Some(value) = &row.committed_pose_digest_sha256 {
        out.digest(value);
    }
}

fn decode_fingerprint(input: &mut Decoder<'_>) -> Result<IterationFingerprint, String> {
    Ok(IterationFingerprint {
        bite: input.u64()?,
        attempt: input.u64()?,
        iteration: input.u64()?,
        winner: input.usize()?,
        winner_guided: input.f64()?,
        contested: input.bool()?,
        state: input.string()?,
        committed_pose_digest_sha256: input.bool()?.then(|| input.digest()).transpose()?,
    })
}

fn encode_pool_entry(out: &mut Encoder, rank: usize, row: &PoolEntry) {
    out.usize(rank);
    out.f64(row.raw_phi);
    encode_poses(out, &row.poses);
    out.usize(row.pair_weights.len());
    for (pair, weight) in row.pair_weights.iter().enumerate() {
        out.usize(pair);
        out.f64(*weight);
    }
    out.usize(row.edge_weights.len());
    for (piece, weights) in row.edge_weights.iter().enumerate() {
        out.usize(piece);
        for (side, weight) in weights.iter().enumerate() {
            out.usize(side);
            out.f64(*weight);
        }
    }
}

fn decode_pool_entry(input: &mut Decoder<'_>, rank: usize) -> Result<PoolEntry, String> {
    if input.usize()? != rank {
        return Err("checkpoint pool rank/order mismatch".to_owned());
    }
    let raw_phi = input.f64()?;
    let poses = decode_poses(input)?;
    let pair_len = input.len()?;
    let mut pair_weights = Vec::with_capacity(pair_len);
    for pair in 0..pair_len {
        if input.usize()? != pair {
            return Err("checkpoint pool pair ID/order mismatch".to_owned());
        }
        pair_weights.push(input.f64()?);
    }
    let edge_len = input.len()?;
    let mut edge_weights = Vec::with_capacity(edge_len);
    for piece in 0..edge_len {
        if input.usize()? != piece {
            return Err("checkpoint pool edge piece ID/order mismatch".to_owned());
        }
        let mut weights = [0.0; 4];
        for (side, weight) in weights.iter_mut().enumerate() {
            if input.usize()? != side {
                return Err("checkpoint pool edge side ID/order mismatch".to_owned());
            }
            *weight = input.f64()?;
        }
        edge_weights.push(weights);
    }
    if poses.len() != edge_weights.len()
        || pair_weights.len() != poses.len() * poses.len().saturating_sub(1) / 2
    {
        return Err("checkpoint pool entry shape mismatch".to_owned());
    }
    Ok(PoolEntry {
        raw_phi,
        poses,
        pair_weights,
        edge_weights,
    })
}

fn encode_engine(out: &mut Encoder, engine: &EngineSnapshotV1) {
    encode_state(out, &engine.state);
    encode_incumbent(out, &engine.incumbent);
    encode_trace(out, &engine.trace);
    encode_config(out, engine.config);
    encode_descent(out, &engine.descent);
    out.bool(engine.last_attempt_pose_digest.is_some());
    if let Some(value) = &engine.last_attempt_pose_digest {
        out.digest(value);
    }
}

fn decode_engine(input: &mut Decoder<'_>) -> Result<EngineSnapshotV1, String> {
    Ok(EngineSnapshotV1 {
        state: decode_state(input)?,
        incumbent: decode_incumbent(input)?,
        trace: decode_trace(input)?,
        config: decode_config(input)?,
        descent: decode_descent(input)?,
        last_attempt_pose_digest: input.bool()?.then(|| input.digest()).transpose()?,
    })
}

impl FirstPoolRetryCheckpointV1 {
    fn encode(&self) -> Vec<u8> {
        let mut out = Encoder::new(DOMAIN);
        out.raw_bytes(SEAM);
        out.usize(self.workers);
        encode_strikes(&mut out, self.strikes);
        out.f64(self.explore_time_ratio);
        out.u64(self.seed);
        encode_engine(&mut out, &self.engine);
        encode_pacer(&mut out, &self.pacer);
        out.f64(self.start_depth_mm);
        out.f64(self.depth_mm);
        out.f64(self.width_mm);
        encode_poses(&mut out, &self.parent_poses);
        out.string(&self.parent_fingerprint);
        out.u64(self.bite_ordinal);
        out.u64(self.explore_bites);
        out.usize(self.bites.len());
        for row in &self.bites {
            encode_bite_record(&mut out, row);
        }
        out.usize(self.publications.len());
        for row in &self.publications {
            encode_published(&mut out, row);
        }
        out.usize(self.fingerprints.len());
        for row in &self.fingerprints {
            encode_fingerprint(&mut out, row);
        }
        encode_bite_record(&mut out, &self.record);
        out.u64(self.attempt);
        out.usize(self.pool.len());
        for (rank, row) in self.pool.iter().enumerate() {
            encode_pool_entry(&mut out, rank, row);
        }
        out.usize(self.selected_rank);
        out.bytes
    }

    fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut input = Decoder::new(bytes, DOMAIN)?;
        if input.raw_bytes()? != SEAM {
            return Err("checkpoint seam mismatch".to_owned());
        }
        let workers = input.usize()?;
        let strikes = decode_strikes(&mut input)?;
        let explore_time_ratio = input.f64()?;
        let seed = input.u64()?;
        let engine = decode_engine(&mut input)?;
        let pacer = decode_pacer(&mut input)?;
        let start_depth_mm = input.f64()?;
        let depth_mm = input.f64()?;
        let width_mm = input.f64()?;
        let parent_poses = decode_poses(&mut input)?;
        let parent_fingerprint = input.string()?;
        let bite_ordinal = input.u64()?;
        let explore_bites = input.u64()?;
        let bites = (0..input.len()?)
            .map(|_| decode_bite_record(&mut input))
            .collect::<Result<Vec<_>, _>>()?;
        let publications = (0..input.len()?)
            .map(|_| decode_published(&mut input))
            .collect::<Result<Vec<_>, _>>()?;
        let fingerprints = (0..input.len()?)
            .map(|_| decode_fingerprint(&mut input))
            .collect::<Result<Vec<_>, _>>()?;
        let record = decode_bite_record(&mut input)?;
        let attempt = input.u64()?;
        let pool = (0..input.len()?)
            .map(|rank| decode_pool_entry(&mut input, rank))
            .collect::<Result<Vec<_>, _>>()?;
        let selected_rank = input.usize()?;
        input.finish()?;
        let checkpoint = Self {
            workers,
            strikes,
            explore_time_ratio,
            seed,
            engine,
            pacer,
            start_depth_mm,
            depth_mm,
            width_mm,
            parent_poses,
            parent_fingerprint,
            bite_ordinal,
            explore_bites,
            bites,
            publications,
            fingerprints,
            record,
            attempt,
            pool,
            selected_rank,
        };
        checkpoint.validate()?;
        if checkpoint.encode() != bytes {
            return Err("checkpoint body is not canonically encoded".to_owned());
        }
        Ok(checkpoint)
    }

    fn validate(&self) -> Result<(), String> {
        if self.workers != 8
            || self.strikes != StrikeConfig::CONTROL
            || self.engine.config.descent.seed != self.seed
            || self.attempt == 0
            || self.pool.is_empty()
            || self.selected_rank >= self.pool.len()
            || self.record.phase != Phase::Explore
            || self.record.ordinal != self.bite_ordinal
            || self.record.attempts != self.attempt
            || self.width_mm.to_bits() != self.record.bite.width_after_mm.to_bits()
            || self.width_mm.to_bits() != self.engine.state.target_depth_mm.to_bits()
            || self.explore_time_ratio.to_bits() != self.pacer.plan.explore_ratio.to_bits()
            || self.engine.state.poses.len() != self.parent_poses.len()
            || self.engine.descent.allow_rotation.len() != self.parent_poses.len()
            || !self.engine.descent.census.records.is_empty()
        {
            return Err("checkpoint structural invariant failed".to_owned());
        }
        let derived = super::homotopy::normal_biased_rank(
            self.pool.len(),
            self.seed,
            self.bite_ordinal,
            self.attempt,
        );
        if derived != self.selected_rank {
            return Err("checkpoint selected rank does not match frozen rank law".to_owned());
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn capture(
    engine: &Engine<'_>,
    pacer: &Pacer,
    schedule: ScheduleConfig,
    start_depth_mm: f64,
    depth_mm: f64,
    width_mm: f64,
    parent_poses: &[Pose],
    parent_fingerprint: &str,
    bite_ordinal: u64,
    explore_bites: u64,
    bites: &[BiteRecord],
    publications: &[PublishedBite],
    fingerprints: &[IterationFingerprint],
    record: &BiteRecord,
    attempt: u64,
    pool: &[PoolEntry],
    selected_rank: usize,
) -> Result<Vec<u8>, String> {
    let Pacer::Calibrated {
        plan,
        attempts_per_bite,
        cursor,
        opened_at,
        charged,
        explore_crossing_batch_units,
        compress_crossing_batch_units,
    } = pacer
    else {
        return Err("pool-retry checkpoint requires calibrated work".to_owned());
    };
    let checkpoint = FirstPoolRetryCheckpointV1 {
        workers: schedule.workers.max(1),
        strikes: schedule.strikes,
        explore_time_ratio: schedule.explore_time_ratio,
        seed: engine.config.descent.seed,
        engine: EngineSnapshotV1 {
            state: engine.state.clone(),
            incumbent: engine.incumbent.clone(),
            trace: engine.trace.clone(),
            config: engine.config,
            descent: engine.descent.checkpoint_snapshot(),
            last_attempt_pose_digest: engine.last_attempt_pose_digest,
        },
        pacer: PacerSnapshotV1 {
            plan: plan.checkpoint_snapshot(),
            attempts_per_bite: *attempts_per_bite,
            cursor: *cursor,
            opened_at: *opened_at,
            charged: *charged,
            explore_crossing_batch_units: *explore_crossing_batch_units,
            compress_crossing_batch_units: *compress_crossing_batch_units,
        },
        start_depth_mm,
        depth_mm,
        width_mm,
        parent_poses: parent_poses.to_vec(),
        parent_fingerprint: parent_fingerprint.to_owned(),
        bite_ordinal,
        explore_bites,
        bites: bites.to_vec(),
        publications: publications.to_vec(),
        fingerprints: fingerprints.to_vec(),
        record: record.clone(),
        attempt,
        pool: pool.to_vec(),
        selected_rank,
    };
    checkpoint.validate()?;
    Ok(checkpoint.encode())
}

pub(super) fn restore<'a>(
    bytes: &[u8],
    pieces: &'a [GeneralFastPiece<'a>],
    sources: Vec<PieceSource>,
    settings: GeneralFastSettings,
    contract: Contract,
) -> Result<RestoredFirstPoolRetry<'a>, String> {
    let checkpoint = FirstPoolRetryCheckpointV1::decode(bytes)?;
    if pieces.len() != checkpoint.engine.state.poses.len()
        || sources.len() != checkpoint.engine.state.poses.len()
    {
        return Err("checkpoint piece/source count mismatch".to_owned());
    }
    let parent_placements = super::publish::placements_of(&sources, &checkpoint.parent_poses);
    if super::publish::placement_fingerprint(&parent_placements) != checkpoint.parent_fingerprint {
        return Err("checkpoint parent poses do not match parent authority fingerprint".to_owned());
    }
    let parent_depth = super::publish::raw_depth_of(pieces, &parent_placements, &contract);
    if parent_depth.to_bits() != checkpoint.depth_mm.to_bits() {
        return Err("checkpoint parent poses do not match exact parent depth".to_owned());
    }
    let live_raw_rows = super::pool_rebase::raw_row_digest(&checkpoint.engine.state);
    let live_weights = super::pool_rebase::WeightSnapshot::of(&checkpoint.engine.state);
    let mut cold_state = checkpoint.engine.state.clone();
    let mut cold_work = WorkVector::default();
    super::energy::rebuild_all(&mut cold_state, &contract, &mut cold_work);
    if super::pool_rebase::raw_row_digest(&cold_state) != live_raw_rows
        || super::pool_rebase::WeightSnapshot::of(&cold_state).bits != live_weights.bits
    {
        return Err("checkpoint live rows do not match an authoritative cold rebuild".to_owned());
    }
    let descent = Descent::from_checkpoint(checkpoint.engine.descent)?;
    let plan = WorkPlanPacer::from_checkpoint(checkpoint.pacer.plan, NoClock)?;
    let pacer = Pacer::Calibrated {
        plan: Box::new(plan),
        attempts_per_bite: checkpoint.pacer.attempts_per_bite,
        cursor: checkpoint.pacer.cursor,
        opened_at: checkpoint.pacer.opened_at,
        charged: checkpoint.pacer.charged,
        explore_crossing_batch_units: checkpoint.pacer.explore_crossing_batch_units,
        compress_crossing_batch_units: checkpoint.pacer.compress_crossing_batch_units,
    };
    let engine = Engine {
        pieces,
        sources,
        settings,
        contract,
        state: checkpoint.engine.state,
        incumbent: checkpoint.engine.incumbent,
        trace: checkpoint.engine.trace,
        config: checkpoint.engine.config,
        descent,
        last_attempt_pose_digest: checkpoint.engine.last_attempt_pose_digest,
    };
    Ok(RestoredFirstPoolRetry {
        engine,
        pacer,
        workers: checkpoint.workers,
        strikes: checkpoint.strikes,
        seed: checkpoint.seed,
        start_depth_mm: checkpoint.start_depth_mm,
        depth_mm: checkpoint.depth_mm,
        width_mm: checkpoint.width_mm,
        parent_poses: checkpoint.parent_poses,
        parent_fingerprint: checkpoint.parent_fingerprint,
        bite_ordinal: checkpoint.bite_ordinal,
        explore_bites: checkpoint.explore_bites,
        bites: checkpoint.bites,
        publications: checkpoint.publications,
        fingerprints: checkpoint.fingerprints,
        record: checkpoint.record,
        attempt: checkpoint.attempt,
        pool: checkpoint.pool,
        selected_rank: checkpoint.selected_rank,
    })
}

/// Complete canonical binding of the immutable context reconstructed by a
/// checkpoint-input process. This is compared before any dynamic state is
/// restored; the request SHA remains a separate outer binding.
pub fn immutable_input_sha256(
    pieces: &[GeneralFastPiece<'_>],
    sources: &[PieceSource],
    settings: GeneralFastSettings,
    contract: Contract,
) -> String {
    let mut out = Encoder::new(b"pool-retry-tracker-rebase/immutable-input/v2");
    out.usize(pieces.len());
    for (piece, (input_piece, source)) in pieces.iter().zip(sources).enumerate() {
        out.usize(piece);
        out.string(input_piece.id);
        out.bool(input_piece.allow_rotation);
        out.bool(input_piece.allow_mirror);
        out.string(&source.id);
        out.usize(source.decomposition.points.len());
        for point in &source.decomposition.points {
            encode_point(&mut out, *point);
        }
        out.usize(source.decomposition.cells.len());
        for (cell, range) in source.decomposition.cells.iter().enumerate() {
            out.usize(cell);
            out.usize(range.start);
            out.usize(range.len);
        }
        out.usize(source.decomposition.ring.len());
        for point in &source.decomposition.ring {
            encode_point(&mut out, *point);
        }
        out.bool(source.decomposition.convex);
        encode_point(&mut out, source.centroid);
        out.f64(source.max_radius_mm);
        out.f64(source.area_mm2);
        out.f64(source.min_width_mm);
        out.f64(source.min_bbox_dim_mm);
        encode_point(&mut out, source.interior_witness);
        out.f64(source.convex_hull_area_mm2);
        out.f64(source.diameter_mm);
    }
    for value in [
        settings.sheet_short_axis_mm,
        settings.sheet_long_axis_mm,
        settings.total_padding_mm,
        settings.clearance_safety_margin_mm,
        settings.flattening_sag_tolerance_mm,
        settings.search_offset_allowance_mm,
    ] {
        out.f64(value);
    }
    out.bool(settings.sheet_edge_clearance_mm.is_some());
    if let Some(value) = settings.sheet_edge_clearance_mm {
        out.f64(value);
    }
    for value in [
        settings.angle_seed_count,
        settings.max_angles_per_piece,
        settings.max_evaluations_per_piece,
        settings.max_exploratory_evaluations_per_piece,
        settings.max_order_variants,
        settings.max_catalog_variants,
        settings.max_catalog_evaluations_per_piece,
        settings.max_pairing_evaluations_per_piece,
        settings.max_pairing_band_variants,
        settings.max_partial_layouts,
        settings.max_beam_evaluations_per_state,
        settings.max_tightening_passes,
        settings.max_repair_targets,
        settings.max_repair_evaluations_per_piece,
        settings.max_local_angle_refinement_evaluations_per_piece,
    ] {
        out.usize(value);
    }
    for value in [
        contract.sheet_short_axis_mm,
        contract.sheet_long_axis_mm,
        contract.total_padding_mm,
        contract.sheet_edge_clearance_mm,
        contract.flattening_sag_tolerance_mm,
        contract.clearance_safety_margin_mm,
    ] {
        out.f64(value);
    }
    format!("{:x}", Sha256::digest(out.bytes))
}

pub fn envelope_bytes(bindings: &CheckpointBindings, body: &[u8]) -> Vec<u8> {
    let mut out = Encoder::new(ENVELOPE_DOMAIN);
    out.string(&bindings.spec_sha256);
    out.string(&bindings.request_sha256);
    out.string(&bindings.plan_sha256);
    out.string(&bindings.executable_sha256);
    out.string(&bindings.source_commit);
    encode_string_vec(&mut out, &bindings.features);
    out.string(&bindings.immutable_input_sha256);
    out.raw_bytes(body);
    out.digest(&Sha256::digest(body).into());
    out.bytes
}

pub fn envelope_body(bytes: &[u8], expected: &CheckpointBindings) -> Result<Vec<u8>, String> {
    let mut input = Decoder::new(bytes, ENVELOPE_DOMAIN)?;
    let found = CheckpointBindings {
        spec_sha256: input.string()?,
        request_sha256: input.string()?,
        plan_sha256: input.string()?,
        executable_sha256: input.string()?,
        source_commit: input.string()?,
        features: decode_string_vec(&mut input)?,
        immutable_input_sha256: input.string()?,
    };
    if &found != expected {
        return Err(format!(
            "checkpoint binding mismatch: expected {expected:?}, found {found:?}"
        ));
    }
    let body = input.raw_bytes()?.to_vec();
    let digest = input.digest()?;
    input.finish()?;
    if <[u8; 32]>::from(Sha256::digest(&body)) != digest {
        return Err("checkpoint body SHA-256 mismatch".to_owned());
    }
    if envelope_bytes(&found, &body) != bytes {
        return Err("checkpoint envelope is not canonically encoded".to_owned());
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_body() -> Vec<u8> {
        let mut out = Encoder::new(b"codec-vector/v1");
        out.u8(0xa5);
        out.bool(true);
        out.bool(false);
        out.u32(0x0102_0304);
        out.u64(0x0102_0304_0506_0708);
        out.usize(3);
        out.f64(-0.0);
        out.raw_bytes(&[0x00, 0xff, 0x7e]);
        out.string("ox");
        out.bytes
    }

    #[test]
    fn checkpoint_codec_has_a_fixed_byte_reference_vector() {
        let bytes = reference_body();
        assert_eq!(bytes.len(), 75);
        assert_eq!(
            bytes.iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
            "0f00000000000000636f6465632d766563746f722f7631a5010004030201080706050403020103000000000000000000000000000080030000000000000000ff7e02000000000000006f78"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            "6b9351159558540edc451bf4047909e2d73b897ffd239a8aa766e52a76667b64"
        );

        let mut input = Decoder::new(&bytes, b"codec-vector/v1").expect("domain");
        assert_eq!(input.u8().unwrap(), 0xa5);
        assert!(input.bool().unwrap());
        assert!(!input.bool().unwrap());
        assert_eq!(input.u32().unwrap(), 0x0102_0304);
        assert_eq!(input.u64().unwrap(), 0x0102_0304_0506_0708);
        assert_eq!(input.usize().unwrap(), 3);
        assert_eq!(input.f64().unwrap().to_bits(), (-0.0f64).to_bits());
        assert_eq!(input.raw_bytes().unwrap(), [0x00, 0xff, 0x7e]);
        assert_eq!(input.string().unwrap(), "ox");
        input.finish().expect("the decoder consumed every byte");
    }

    #[test]
    fn checkpoint_envelope_rejects_binding_body_and_length_mutations() {
        let bindings = CheckpointBindings {
            spec_sha256: "spec".to_owned(),
            request_sha256: "request".to_owned(),
            plan_sha256: "plan".to_owned(),
            executable_sha256: "executable".to_owned(),
            source_commit: "source".to_owned(),
            features: vec![
                "overlap-ics".to_owned(),
                "pool-retry-tracker-rebase".to_owned(),
            ],
            immutable_input_sha256: "immutable".to_owned(),
        };
        let body = reference_body();
        let artifact = envelope_bytes(&bindings, &body);
        assert_eq!(envelope_body(&artifact, &bindings).unwrap(), body);

        for field in 0..7 {
            let mut changed = bindings.clone();
            match field {
                0 => changed.spec_sha256.push('x'),
                1 => changed.request_sha256.push('x'),
                2 => changed.plan_sha256.push('x'),
                3 => changed.executable_sha256.push('x'),
                4 => changed.source_commit.push('x'),
                5 => changed.features.push("extra".to_owned()),
                6 => changed.immutable_input_sha256.push('x'),
                _ => unreachable!(),
            }
            assert!(envelope_body(&artifact, &changed).is_err());
        }

        let mut changed_artifact = artifact.clone();
        let body_start = artifact
            .windows(body.len())
            .position(|window| window == body)
            .expect("body occurs in its envelope");
        changed_artifact[body_start + body.len() - 1] ^= 1;
        assert!(envelope_body(&changed_artifact, &bindings).is_err());

        let mut trailing = artifact;
        trailing.push(0);
        assert!(envelope_body(&trailing, &bindings).is_err());
    }
}
