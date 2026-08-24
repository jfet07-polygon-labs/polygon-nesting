//! The overlap-ICS vertical slice's only driver, and Gate 0's battery.
//!
//! ```text
//! overlap_ics_benchmark --cell=CELL --request=REQUEST.json [key=value ...]
//! ```
//!
//! One JSON document on stdout, nothing written in place. Every cell reports
//! the same skeleton - request identity, contract, work vector, exact
//! checkpoints - so two cells can be diffed as documents.
//!
//! **Wall fields are confined to one object, `wall`.** The two-process
//! fixed-work smoke strips exactly that key and requires the rest to be
//! byte-identical; a wall number anywhere else would silently pass that
//! comparison and make the determinism claim worthless.
//!
//! The request loader below is `sparrow_import_gate`'s, which is the benchmark
//! example's reduced to the fields a pose set needs: the same
//! `polygon_set_from_imported_piece`, the same
//! `GeneralFastSettings::deterministic_test` seed, the same
//! `sheet.width >= sheet.height` axis-normalisation rule.
//!
//! Chinese wall: the Sparrow pose fixture is read by the `s0`, `s1` and `s2`
//! cells and by nothing else. It is a correctness pin - never a seed, never a
//! parameter source - and no constant in `search::overlap_ics` was chosen by
//! looking at it.

#![recursion_limit = "256"]

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::time::Instant;

use polygon_nesting_core::domain::ImportedPiece;
use polygon_nesting_core::geometry::general_polygon::PolygonSet;
use polygon_nesting_core::geometry::general_source::polygon_set_from_imported_piece;
use polygon_nesting_core::search::general_fast::{
    construct_short_side_first, GeneralFastPiece, GeneralFastPlacement, GeneralFastSettings,
};
#[cfg(feature = "minimum-conflict-binary-close")]
use polygon_nesting_core::search::overlap_ics::binary_close::{
    gate0_vector_report as binary_close_gate0_vector_report, geometry_gate0_vector_report,
    BinaryCloseArm, BinaryCloseTrace, Gate0VectorReport as BinaryCloseGate0VectorReport,
};
#[cfg(feature = "conflict-cluster-budget")]
use polygon_nesting_core::search::overlap_ics::cluster_budget::{
    gate0_vector_report as partition_gate0_vector_report, FallbackKind, Gate0VectorReport,
    PartitionArm, PartitionCostArmSample, PartitionTrace,
};
use polygon_nesting_core::search::overlap_ics::contact::convex_cell_gap;
use polygon_nesting_core::search::overlap_ics::corpus;
use polygon_nesting_core::search::overlap_ics::descent::{
    counter_hash, DescentConfig, RejectionCensus,
};
use polygon_nesting_core::search::overlap_ics::diagnostics::WorkVector;
use polygon_nesting_core::search::overlap_ics::homotopy;
use polygon_nesting_core::search::overlap_ics::icscal::{
    BinaryKey, CurrencyVersion, Executor, PhasePlan, PlanKey, PlanPhase, WorkPlan,
};
use polygon_nesting_core::search::overlap_ics::icscal_read::plan_from_bytes;
use polygon_nesting_core::search::overlap_ics::profile::PhaseProfile;
use polygon_nesting_core::search::overlap_ics::publish;
use polygon_nesting_core::search::overlap_ics::publish::{
    placement_fingerprint, raw_depth_of, PublicationLimits,
};
use polygon_nesting_core::search::overlap_ics::state::{
    piece_sources, Contract, ExactIncumbent, PieceSource, Pose,
};
use polygon_nesting_core::search::overlap_ics::{
    poses_of, Budget, Engine, IcsConfig, IcsOutcome, InitialLayoutProvider, Phase, ScheduleConfig,
    ScheduleOutcome,
};
use polygon_nesting_core::search::overlap_ics_meter::currency::{Currency, WorkTerms};
use polygon_nesting_core::search::overlap_ics_meter::pacer::{
    match_plan, NoClock, PlanMatch, WorkPlanPacer,
};
use polygon_nesting_core::search::overlap_ics_meter::strike_meter::{
    frozen_literals_intact, Patience, StrikeConfig,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

// ------------------------------------------------------------- the request ---

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Request {
    sheet: Sheet,
    #[serde(default)]
    padding: Option<f64>,
    pieces: Vec<RequestPiece>,
    source_pieces: Vec<ImportedPiece>,
    #[serde(default)]
    settings: Option<RequestSettings>,
    #[serde(default)]
    options: Option<LegacyOptions>,
}

#[derive(Deserialize)]
struct Sheet {
    width: f64,
    height: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestPiece {
    id: String,
    source_piece_id: String,
    allow_rotation: bool,
    #[serde(default = "default_true")]
    allow_mirror: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestSettings {
    padding: f64,
    allow_global_rotation: bool,
    #[serde(default = "default_true")]
    allow_global_mirror: bool,
    geometry: GeometrySettings,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyOptions {
    allow_global_rotation: bool,
    #[serde(default = "default_true")]
    allow_global_mirror: bool,
    irregular_settings: LegacyIrregularSettings,
}

#[derive(Deserialize)]
struct LegacyIrregularSettings {
    geometry: GeometrySettings,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeometrySettings {
    flattening_sag_tolerance_mm: f64,
    clearance_safety_margin_mm: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PoseFixture {
    placements: Vec<FixturePose>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixturePose {
    piece_id: String,
    rotation_deg: f64,
    #[serde(default)]
    mirrored: bool,
    translate_short_axis: f64,
    translate_long_axis: f64,
}

fn default_true() -> bool {
    true
}

struct OwnedPiece {
    id: String,
    polygon: PolygonSet,
    allow_rotation: bool,
    allow_mirror: bool,
}

/// The constructor, behind the engine's own adapter. This is the only place in
/// the whole overlap-ICS tree that names `construct_short_side_first`.
struct ShortSideFirst;

impl InitialLayoutProvider for ShortSideFirst {
    fn layout(
        &self,
        pieces: &[GeneralFastPiece<'_>],
        settings: GeneralFastSettings,
    ) -> Result<Vec<GeneralFastPlacement>, String> {
        let result = construct_short_side_first(pieces, settings)
            .map_err(|error| format!("constructor: {error}"))?;
        if !result.unplaced_piece_ids.is_empty() {
            return Err(format!(
                "the constructor left {} pieces unplaced; the ICS state needs a complete layout",
                result.unplaced_piece_ids.len()
            ));
        }
        Ok(result.placements)
    }
}

// --------------------------------------------------------------- arguments ---

struct Options {
    map: BTreeMap<String, String>,
}

impl Options {
    fn parse() -> Result<Self, String> {
        let mut map = BTreeMap::new();
        for argument in env::args().skip(1) {
            let trimmed = argument.trim_start_matches("--");
            let (key, value) = trimmed
                .split_once('=')
                .ok_or_else(|| format!("argument `{argument}` is not key=value"))?;
            map.insert(key.to_owned(), value.to_owned());
        }
        Ok(Self { map })
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.map.get(key).map(String::as_str)
    }

    fn required(&self, key: &str) -> Result<&str, String> {
        self.get(key).ok_or_else(|| format!("missing --{key}"))
    }

    fn number(&self, key: &str, fallback: f64) -> Result<f64, String> {
        match self.get(key) {
            Some(value) => value.parse().map_err(|_| format!("--{key}: `{value}`")),
            None => Ok(fallback),
        }
    }

    fn integer(&self, key: &str, fallback: u64) -> Result<u64, String> {
        match self.get(key) {
            Some(value) => value.parse().map_err(|_| format!("--{key}: `{value}`")),
            None => Ok(fallback),
        }
    }
}

// ------------------------------------------------------------------ output ---

fn work_json(work: &WorkVector) -> Value {
    let map = work.to_map();
    let mut object = serde_json::Map::new();
    for (key, value) in map {
        object.insert(key.to_owned(), json!(value));
    }
    Value::Object(object)
}

#[cfg(feature = "conflict-cluster-budget")]
fn partition_json(trace: &PartitionTrace) -> Value {
    let fallback = |kind: FallbackKind| match kind {
        FallbackKind::None => "none",
        FallbackKind::ZeroSignal => "zero-signal",
        FallbackKind::Invalid => "invalid",
    };
    json!({
        "partitionDecisions": trace.partition_decisions,
        "eligibleDecisions": trace.eligible_decisions,
        "eligibleDisagreementDecisions": trace.eligible_disagreement_decisions,
        "eligibleDisagreementRate": trace.eligible_disagreement_rate(),
        "entryCollidingPieces": trace.entry_colliding_pieces,
        "componentCount": trace.component_count,
        "positivePairEdges": trace.positive_pair_edges,
        "partitionSlots": trace.partition_slots,
        "executedSlots": trace.executed_slots,
        "fullRelocateSlots": trace.full_relocate_slots,
        "zeroEnergySlots": trace.zero_energy_slots,
        "pairDiscTerms": trace.pair_disc_terms,
        "positiveBoundaryRows": trace.positive_boundary_rows,
        "zeroSignalFallbackDecisions": trace.zero_signal_fallback_decisions,
        "invalidFallbackDecisions": trace.invalid_fallback_decisions,
        "planIdentityFailureDecisions": trace.plan_identity_failure_decisions,
        "executionIdentityFailureDecisions": trace.execution_identity_failure_decisions,
        "slotIdentitiesHold": trace.slot_identities_hold(),
        "graphDigestSha256": hex_bytes(&trace.graph_digest_sha256),
        "allocationDigestSha256": hex_bytes(&trace.allocation_digest_sha256),
        "scheduleDigestSha256": hex_bytes(&trace.schedule_digest_sha256),
        "decisions": trace.decisions.iter().map(|row| json!({
            "seed": row.key.seed,
            "bite": row.key.bite,
            "iteration": row.key.iteration,
            "worker": row.key.worker,
            "Q": row.q(),
            "entry": row.entry,
            "components": row.components.iter().map(|component| json!({
                "id": component.id,
                "members": component.members,
            })).collect::<Vec<_>>(),
            "positivePairEdges": row.positive_pair_edges.iter().map(|edge| json!({
                "pairId": edge.pair_id,
                "first": edge.first,
                "second": edge.second,
            })).collect::<Vec<_>>(),
            "pairDiscTerms": row.pair_disc_terms,
            "positiveBoundaryRows": row.positive_boundary_rows,
            "massBits": row.mass_bits,
            "maxViolationBits": row.max_violation_bits,
            "massQuotas": row.mass_quotas,
            "shuffledQuotas": row.shuffled_quotas,
            "maxViolationQuotas": row.max_violation_quotas,
            "massDiffersFromMaxViolation": row.mass_quotas != row.max_violation_quotas,
            "memberPermutations": row.member_permutations,
            "massSchedule": row.mass_schedule,
            "shuffledSchedule": row.shuffled_schedule,
            "maxViolationSchedule": row.max_violation_schedule,
            "massFallback": fallback(row.mass_fallback),
            "shuffledFallback": fallback(row.shuffled_fallback),
            "maxViolationFallback": fallback(row.max_violation_fallback),
            "placeboOffset": row.placebo_offset,
            "planIdentitiesHold": row.plan_identities_hold(),
            "spearmanFieldMassMaxViolation": row.spearman_field_mass_max,
            "spearmanQuotaMassMaxViolation": row.spearman_quota_mass_max,
        })).collect::<Vec<_>>(),
    })
}

fn hex_bytes(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut out, "{byte:02x}").expect("writing to a String cannot fail");
    }
    out
}

#[cfg(feature = "minimum-conflict-binary-close")]
fn binary_close_arm(options: &Options, key: &str) -> Result<BinaryCloseArm, String> {
    match options.get(key).unwrap_or("centre") {
        "centre" => Ok(BinaryCloseArm::Centre),
        "mincut" => Ok(BinaryCloseArm::MinCut),
        "compute-ignore" => Ok(BinaryCloseArm::ComputeIgnore),
        other => Err(format!(
            "--{key} must be centre|mincut|compute-ignore, not `{other}`"
        )),
    }
}

#[cfg(feature = "minimum-conflict-binary-close")]
fn binary_close_json(trace: &BinaryCloseTrace) -> Value {
    json!({
        "invalidDecisions": trace.invalid_decisions,
        "decisions": trace.decisions.iter().map(|decision| json!({
            "key": {
                "requestSeed": decision.request_seed,
                "exploreBiteOrdinal": decision.explore_bite_ordinal,
            },
            "depthBeforeMm": decision.depth_before_mm,
            "depthBeforeBits": decision.depth_before_mm.to_bits(),
            "targetDepthMm": decision.target_depth_mm,
            "targetDepthBits": decision.target_depth_mm.to_bits(),
            "deltaMm": decision.delta_mm,
            "deltaBits": decision.delta_mm.to_bits(),
            "poseStateBitsValid": decision.pose_state_bits_valid,
            "parentProxyPairLegal": decision.parent_proxy_pair_legal,
            "parentPoseDigestSha256": hex_bytes(&decision.parent_pose_digest_sha256),
            "pairs": decision.pair_terms.iter().map(|term| json!({
                "pairId": term.pair_id,
                "first": term.first,
                "second": term.second,
                "violationBits": term.violations_mm.map(|row| row.map(f64::to_bits)),
                "costBits": term.costs.map(|row| row.map(f64::to_bits)),
                "finiteNonnegative": term.finite_nonnegative,
                "zeroDiagonal": term.zero_diagonal,
                "submodular": term.submodular,
            })).collect::<Vec<_>>(),
            "unaries": decision.unary_terms.iter().map(|term| json!({
                "piece": term.piece,
                "violationBits": term.violations_mm.map(|row| row.map(f64::to_bits)),
                "rowCostBits": term.row_costs.map(|row| row.map(f64::to_bits)),
                "sumBits": term.sums.map(f64::to_bits),
                "finiteNonnegative": term.finite_nonnegative,
            })).collect::<Vec<_>>(),
            "allFiniteNonnegative": decision.all_finite_nonnegative,
            "allZeroDiagonal": decision.all_zero_diagonal,
            "allSubmodular": decision.all_submodular,
            "graphEdges": decision.graph_edges.iter().map(|edge| json!({
                "from": edge.from,
                "to": edge.to,
                "capacityBits": edge.capacity.to_bits(),
            })).collect::<Vec<_>>(),
            "residualSourceReachable": decision.residual_source_reachable,
            "labels": decision.labels,
            "centreLabels": decision.centre_labels,
            "hammingDisagreement": decision.hamming_disagreement,
            "movedPieces": decision.moved_pieces,
            "centreMovedPieces": decision.centre_moved_pieces,
            "digests": {
                "termTableSha256": hex_bytes(&decision.term_table_digest_sha256),
                "graphSha256": hex_bytes(&decision.graph_digest_sha256),
                "residualSha256": hex_bytes(&decision.residual_digest_sha256),
                "labelsSha256": hex_bytes(&decision.label_digest_sha256),
                "installedPosesSha256": hex_bytes(&decision.installed_pose_digest_sha256),
                "installedRowsSha256": hex_bytes(&decision.installed_row_digest_sha256),
            },
            "selectedCutCapacity": decision.selected_cut_capacity.is_finite()
                .then_some(decision.selected_cut_capacity),
            "selectedCutCapacityBits": decision.selected_cut_capacity.to_bits(),
            "selectedTableEnergy": decision.selected_table_energy.is_finite()
                .then_some(decision.selected_table_energy),
            "selectedTableEnergyBits": decision.selected_table_energy.to_bits(),
            "coldRawPhi": decision.cold_raw_phi.is_finite().then_some(decision.cold_raw_phi),
            "coldRawPhiBits": decision.cold_raw_phi.to_bits(),
            "cutTableBitsEqual": decision.cut_table_bits_equal,
            "tableColdBitsEqual": decision.table_cold_bits_equal,
            "installedRowsMatchTable": decision.installed_rows_match_table,
            "fieldWork": work_json(&decision.field_work),
            "valid": decision.valid,
            "invalidReason": decision.invalid_reason,
        })).collect::<Vec<_>>(),
    })
}

#[cfg(feature = "minimum-conflict-binary-close")]
fn binary_close_vectors_json(report: &BinaryCloseGate0VectorReport) -> Value {
    json!({
        "expectedLabels": report.expected_labels,
        "solverLabels": report.solver_labels,
        "exhaustiveEnergyBits": report.exhaustive_energies.iter()
            .map(|value| value.to_bits()).collect::<Vec<_>>(),
        "uniqueNontrivialMinimum": report.unique_nontrivial_minimum,
        "everyLabelCutEnergyIdentity": report.every_label_cut_energy_identity,
        "acceptsZeroDiagonalSubmodular": report.accepts_zero_diagonal_submodular,
        "rejectsNonfinite": report.rejects_nonfinite,
        "rejectsNegative": report.rejects_negative,
        "rejectsNonzeroDiagonal": report.rejects_nonzero_diagonal,
        "rejectsNonsubmodular": report.rejects_nonsubmodular,
        "allZeroLabels": report.all_zero_labels,
        "allOneLabels": report.all_one_labels,
        "tieLabelsFirst": report.tie_labels_first,
        "tieLabelsSecond": report.tie_labels_second,
        "tieStable": report.tie_labels_first == report.tie_labels_second,
        "graphDigestSha256": hex_bytes(&report.graph_digest_sha256),
    })
}

#[cfg(feature = "conflict-cluster-budget")]
fn partition_cost_json(sample: &PartitionCostArmSample) -> Value {
    json!({
        "arm": sample.arm.as_str(),
        "warmupSweeps": sample.warmup_sweeps,
        "measuredSweeps": sample.measured_sweeps,
        "pieceCount": sample.piece_count,
        "entryCollidingPieces": sample.entry_colliding_pieces,
        "expectedAtomicSlots": sample.expected_atomic_slots,
        "completedAtomicSlots": sample.completed_atomic_slots,
        "legacyProposals": sample.legacy_proposals,
        "elapsedSeconds": sample.elapsed_seconds,
        "slotsPerSecond": sample.slots_per_second,
        "poseSequenceDigestSha256": sample.pose_sequence_digest_sha256,
        "consumedOrderDigestSha256": sample.consumed_order_digest_sha256,
        "work": work_json(&sample.work),
        "partition": partition_json(&sample.partition),
    })
}

#[cfg(feature = "conflict-cluster-budget")]
fn partition_vectors_json(report: &Gate0VectorReport) -> Value {
    let fallback = |kind: FallbackKind| match kind {
        FallbackKind::None => "none",
        FallbackKind::ZeroSignal => "zero-signal",
        FallbackKind::Invalid => "invalid",
    };
    json!({
        "unitSquare": {
            "center": report.unit_square_center,
            "radius": report.unit_square_radius,
        },
        "transformedDisc": {
            "mirror": true,
            "sin": 1.0,
            "cos": 0.0,
            "translation": [10.0, 5.0],
            "center": report.transformed_center,
            "radius": report.transformed_radius,
        },
        "pairInversion": {
            "kind": "pure-frozen-row-field-vector",
            "callsMeasurePair": !report.inversion_is_pure_frozen_row_field_vector,
            "massTermsMm2": report.pair_mass_terms,
            "massQuotas": report.mass_inversion_quotas,
            "maxViolationWeightsMm": [2.0, 1.0],
            "maxViolationQuotas": report.max_violation_inversion_quotas,
        },
        "boundaryV3TermMm2": report.boundary_term,
        "largestRemainder": {
            "componentIds": report.largest_remainder_component_ids,
            "quotas": report.largest_remainder_quotas,
        },
        "mixedZeroQuotas": report.mixed_zero_quotas,
        "zeroSignalQuotas": report.zero_signal_quotas,
        "zeroSignalFallback": fallback(report.zero_signal_fallback),
        "placebo": {
            "offset": report.placebo_offset,
            "input": report.placebo_input,
            "rotated": report.placebo_rotated,
            "quotas": report.placebo_quotas,
            "multisetPreserved": report.placebo_multiset_preserved,
            "nonIdentity": report.placebo_non_identity,
        },
        "memberPermutation": report.member_permutation,
        "roundRobinSchedule": report.round_robin_schedule,
        "invalidQuotas": report.invalid_quotas,
        "invalidFallback": fallback(report.invalid_fallback),
        "nonfiniteSourceRejected": report.nonfinite_source_rejected,
        "nonfinitePairRejected": report.nonfinite_pair_rejected,
        "accountingIdentities": {
            "quotaSumEqualsQ": report.quota_sum_identity,
            "scheduleLengthEqualsQ": report.schedule_length_identity,
            "executedSlotsEqualsQ": report.executed_slots_identity,
            "fullPlusZeroEqualsQ": report.full_plus_zero_identity,
        },
    })
}

/// The rejection census both reviews require before any statement about the
/// move set: the whole population's accept/reject split by direction class, and
/// a bounded rung-by-rung decomposition of the rejections **at the stall**.
fn rejection_census_json(census: &RejectionCensus) -> Value {
    json!({
        "armed": census.armed,
        "acceptedProposals": census.accepted,
        "rejectedProposals": census.rejected,
        "zeroEnergyProposals": census.zero_energy,
        "acceptedByDirectionClass": {
            "translation": census.accepted_by_class[0],
            "rotation": census.accepted_by_class[1],
            "combined": census.accepted_by_class[2],
        },
        "rejectedByDirectionClass": {
            "translation": census.rejected_by_class[0],
            "rotation": census.rejected_by_class[1],
            "combined": census.rejected_by_class[2],
        },
        "sampledRejections": census.records.len(),
        "rejections": census.records.iter().map(|row| json!({
            "proposalOrdinal": row.proposal_ordinal,
            "piece": row.piece,
            "directionClass": row.direction_class,
            "translationShare": row.translation_share,
            "rotationShare": row.rotation_share,
            "incidentGuidedBefore": row.incident_guided_before,
            "rawPhiBefore": row.raw_before,
            "guidedPhiBefore": row.guided_before,
            "maxViolationBeforeMm": row.max_violation_before_mm,
            "activeIncidentRows": row.active_incident_rows,
            "activeIncidentPenaltyMax": row.active_incident_penalty_max,
            "activeIncidentPenaltySum": row.active_incident_penalty_sum,
            "rungs": row.rungs.iter().map(|rung| json!({
                "stepMm": rung.step_mm,
                "deltaIncidentGuided": rung.delta_incident_guided,
                "deltaRawPhi": rung.delta_raw,
                "deltaMaxViolationMm": rung.delta_max_violation_mm,
                "newlyActivatedRows": rung.newly_activated_rows,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

/// What `schedule_json` needs to make a layout re-validatable by a reader.
///
/// It is a borrowed bundle rather than four more parameters because the poses
/// have to be turned back into request-coordinate placements, and the pieces
/// and contract are only reached at all when `--revalidate=1` asks the emitter
/// to recompute the depth it is printing.
struct LayoutContext<'a> {
    sources: &'a [PieceSource],
    pieces: &'a [GeneralFastPiece<'a>],
    contract: &'a Contract,
    revalidate: bool,
}

/// The engine's own continuous poses. `mirrored` first-class, `thetaDeg`
/// unwrapped: this is the array the engine installs, not a presentation of it.
fn poses_json(poses: &[Pose]) -> Value {
    Value::Array(
        poses
            .iter()
            .map(|pose| {
                json!({
                    "txMm": pose.tx_mm,
                    "tyMm": pose.ty_mm,
                    "thetaDeg": pose.theta_deg,
                    "mirrored": pose.mirrored,
                })
            })
            .collect(),
    )
}

/// The same layout in the request's own coordinates, in the shape a pose
/// fixture is read in (`PoseFixture` above, and
/// `docs/experiments/gate-a-sparrow-import/fixture/`). This is the form an
/// external validator can push straight back through `raw_depth_of` and the
/// contract validator.
fn placements_json(placements: &[GeneralFastPlacement]) -> Value {
    Value::Array(
        placements
            .iter()
            .map(|placement| {
                json!({
                    "pieceId": placement.piece_id,
                    "rotationDeg": placement.rotation_deg,
                    "mirrored": placement.mirrored,
                    "translateShortAxis": placement.translate_short_axis,
                    "translateLongAxis": placement.translate_long_axis,
                })
            })
            .collect(),
    )
}

/// One bite's master-iteration phase census. All zeros - and `measured: false`
/// - in a build without `ics-profile`, which is every build a gate reads.
fn profile_json(profile: &PhaseProfile) -> Value {
    json!({
        "measured": profile.measured(),
        "iterations": profile.iterations,
        "barrierToBarrierNs": profile.barrier_to_barrier_ns,
        "prepNs": profile.prep_ns,
        "dispatchNs": profile.dispatch_ns,
        "sweepCriticalNs": profile.sweep_critical_ns,
        "sweepTotalNs": profile.sweep_total_ns,
        "mergeGlsNs": profile.merge_gls_ns,
        "exactNs": profile.exact_ns,
        "bandFoldNs": profile.band_fold_ns,
        "snapshotNs": profile.snapshot_ns,
        "residualNs": profile.residual_ns(),
        "bandEntries": profile.band_entries,
        "exactCalls": profile.exact_calls,
        "sampleEvaluations": profile.sample_evaluations,
        "repairRows": profile.repair_rows,
        "disruptionMoves": profile.disruption_moves,
        "prepPlusDispatchNs": profile.prep_plus_dispatch_ns(),
        "prepPlusDispatchShare": profile.prep_plus_dispatch_share(),
    })
}

/// One profile, decomposed **as fractions of barrier-to-barrier**, which is
/// the denominator the spec's 10 % clause names.
///
/// The seven named regions are not forced to sum to one. `residualShare` is
/// whatever is left - `observe_raw`, the strike ladder's comparisons, the loop
/// bookkeeping and the measurement's own overhead - and it is printed rather
/// than distributed, because a decomposition that always adds up is usually a
/// decomposition with a fudge term in it. `prepPlusDispatchShare` is the one
/// number the executor gate reads, and this function does not compare it to
/// anything: the threshold lives in the census driver, quoted from the spec.
fn phase_census_json(profile: &PhaseProfile) -> Value {
    let total = profile.barrier_to_barrier_ns;
    let share = |value: u64| -> Value {
        if total == 0 {
            Value::Null
        } else {
            json!(value as f64 / total as f64)
        }
    };
    let per_iteration = |value: u64| -> Value {
        if profile.iterations == 0 {
            Value::Null
        } else {
            json!(value as f64 / profile.iterations as f64)
        }
    };
    json!({
        "measured": profile.measured(),
        "iterations": profile.iterations,
        "barrierToBarrierNs": total,
        "barrierToBarrierNsPerIteration": per_iteration(total),
        "ns": {
            "prep": profile.prep_ns,
            "dispatch": profile.dispatch_ns,
            "sweepCritical": profile.sweep_critical_ns,
            "sweepTotal": profile.sweep_total_ns,
            "mergeGls": profile.merge_gls_ns,
            "exact": profile.exact_ns,
            "bandFold": profile.band_fold_ns,
            "snapshot": profile.snapshot_ns,
            "residual": profile.residual_ns(),
        },
        "share": {
            "prep": share(profile.prep_ns),
            "dispatch": share(profile.dispatch_ns),
            "sweepCritical": share(profile.sweep_critical_ns),
            "mergeGls": share(profile.merge_gls_ns),
            "exact": share(profile.exact_ns),
            "bandFold": share(profile.band_fold_ns),
            "snapshot": share(profile.snapshot_ns),
            "residual": share(profile.residual_ns()),
        },
        "prepPlusDispatchNs": profile.prep_plus_dispatch_ns(),
        "prepPlusDispatchShare": profile.prep_plus_dispatch_share(),
        "bandEntries": profile.band_entries,
        "exactCalls": profile.exact_calls,
        // The five terms of the spec's currency, for this window alone. All
        // counters, so they are populated in every build.
        "currencyTerms": {
            "sampleEvaluations": profile.sample_evaluations,
            "masterBatches": profile.iterations,
            "actualPublicationAttemptCalls": profile.exact_calls,
            "repairRows": profile.repair_rows,
            "disruptionMoves": profile.disruption_moves,
        },
        "sampleEvaluationsPerSecond": if total == 0 {
            Value::Null
        } else {
            json!(profile.sample_evaluations as f64 / (total as f64 / 1e9))
        },
    })
}

/// The currency's five counted terms, in the currency's own field names.
///
/// One writer for all three of `charged`, `unchargedTail` and `trajectory`, so
/// the three cannot be printed under names that do not line up - which is
/// exactly the comparison the double-debit identity is read off.
fn work_terms_json(terms: &WorkTerms) -> Value {
    json!({
        "sampleEvaluations": terms.sample_evaluations,
        "masterBatches": terms.master_batches,
        "actualPublicationAttemptCalls": terms.actual_publication_attempt_calls,
        "repairRows": terms.repair_rows,
        "disruptionMoves": terms.disruption_moves,
    })
}

/// This executable's own sha256, the `binaryKey` half of an icscal file.
///
/// `None` rather than an empty string when the executable cannot be read, so
/// the document keeps saying `null` exactly where it always has and
/// `WorkPlan::validate` refuses a plan keyed to a binary nobody can name.
fn executable_sha256() -> Option<String> {
    env::current_exe()
        .ok()
        .and_then(|path| fs::read(path).ok())
        .map(|bytes| format!("{:x}", Sha256::digest(&bytes)))
}

/// The features this binary was built with, in a fixed order, for the same key.
fn build_features() -> Vec<String> {
    let mut features = vec!["overlap-ics".to_owned()];
    if cfg!(feature = "conflict-cluster-budget") {
        features.push("conflict-cluster-budget".to_owned());
    }
    if cfg!(feature = "minimum-conflict-binary-close") {
        features.push("minimum-conflict-binary-close".to_owned());
    }
    if cfg!(feature = "ics-profile") {
        features.push("ics-profile".to_owned());
    }
    features
}

/// An `icscal/v1` plan derived from the **shelf** bite alone.
///
/// The rate is `sampleEvaluations / seconds` measured on bite 22, never on the
/// cheap prefix: pre-named defect (3). The seconds are the shelf's own
/// barrier-to-barrier wall when the profile measured one, and the driver's
/// bracket around the shelf probe otherwise. In both cases the numerator is
/// the shelf bite's own counter. Mixing the cumulative trajectory numerator
/// with the probe-only denominator over-promises the non-profile rate.
///
/// `compress` is Wave 3's addition and it is **additive**: passing `None`
/// produces exactly the bytes Wave 1 wrote, which is why the census's
/// committed plan is still the plan it was. A pacer needs both phases (a
/// trajectory runs both), so a plan meant to be *spent* rather than merely
/// recorded carries a compress rate measured on compress bites.
#[allow(clippy::too_many_arguments)]
fn shelf_work_plan(
    request_sha256: &str,
    executable_sha256: &str,
    workers: usize,
    outcome: &ScheduleOutcome,
    shelf_index: usize,
    shelf_ordinal: u64,
    search_seconds: f64,
    safety_factor: f64,
    compress: Option<PhasePlan>,
) -> Result<WorkPlan, String> {
    let shelf = outcome
        .bites
        .get(shelf_index)
        .ok_or_else(|| format!("no bite {} to calibrate on", shelf_index + 1))?;
    let (seconds, units, derivation) = if shelf.profile.measured() {
        (
            shelf.profile.barrier_to_barrier_ns as f64 / 1e9,
            // The shelf's OWN sample evaluations, not the trajectory's:
            // `PhaseProfile` carries the currency's five terms per bite for
            // exactly this reason. Nothing here is apportioned.
            shelf.profile.sample_evaluations,
            format!(
                "trajectory bite {shelf_ordinal} (the 179 shelf) alone, {} master iterations, \
                 barrier-to-barrier wall from the ics-profile timers, sampleEvaluations \
                 charged to that bite. NOT the cheap 0.1 % prefix: spec defect (3).",
                shelf.master_iterations
            ),
        )
    } else {
        (
            search_seconds,
            // `search_seconds` brackets this probe, not the prefix that built
            // its parent. Its numerator must therefore be this bite's own
            // counter too. The former cumulative numerator made the plain
            // writer about 16% fast while its provenance claimed the opposite.
            shelf.profile.sample_evaluations,
            format!(
                "trajectory bite {shelf_ordinal} (the 179 shelf) alone, {} master iterations, \
                 the driver's probe-only wall and sampleEvaluations charged to that bite; \
                 no ics-profile phase timer. NOT the cheap 0.1 % prefix: spec defect (3).",
                shelf.master_iterations
            ),
        )
    };
    let mut phases = vec![PhasePlan::from_measurement(
        PlanPhase::Explore,
        units,
        seconds,
        safety_factor,
        derivation,
    )?];
    phases.extend(compress);
    Ok(WorkPlan::new(
        PlanKey {
            request_sha256: request_sha256.to_owned(),
            currency_version: CurrencyVersion::U0Samples,
            binary_key: BinaryKey {
                executable_sha256: executable_sha256.to_owned(),
                features: build_features(),
            },
            workers,
            executor: Executor::EphemeralScope,
        },
        phases,
        // **Not updated, on purpose.** This sentence is stale as a statement
        // about the round - Wave 3 built both a reader and a pacer - but it is
        // baked into `census/evidence/mixed61-w8-seed0.icscal.json`, and the
        // committed bytes of a measurement are the measurement. Editing it
        // would mean a re-run of `spawntax.py --icscal=` no longer reproduces
        // the file the census recorded the sha256 of, which is a worse thing
        // to be wrong about than a provenance line that names its own wave.
        // The plan a pacer actually spends is written by `calibration_plan`
        // below, and its provenance is current.
        "docs/experiments/overlap-ics/economics-round/census/, the spawntax cell. \
         Schema and writer only: this round builds no reader and no pacer.",
    ))
}

/// **A two-phase plan measured on one wall trajectory: the calibration entry
/// point a `--mode=calibrated` run spends.**
///
/// Each phase's rate is that phase's own `sampleEvaluations` - counters,
/// charged per bite by `PhaseProfile`, so nothing is apportioned - over that
/// phase's own wall, which the loop reports as `exploreSeconds` and
/// `searchSeconds - exploreSeconds`. One blended rate would be the
/// probe-on-cheap-bites defect wearing a different hat, which is why
/// `PlanPhase` exists at all.
///
/// It is deliberately a **wall-mode** measurement, and that is not a leak: a
/// rate is a statement about seconds and cannot be measured without a clock.
/// The separation the spec asks for is between *this* process and the gated
/// one, and it is total - the gated run reads bytes and constructs no
/// `Instant` at all.
fn calibration_plan(
    request_sha256: &str,
    executable_sha256: &str,
    workers: usize,
    outcome: &ScheduleOutcome,
    safety_factor: f64,
) -> Result<WorkPlan, String> {
    let explore_seconds = outcome
        .explore_seconds
        .ok_or("a calibration needs a wall-mode trajectory: --mode=wall")?;
    let search_seconds = outcome
        .search_seconds
        .ok_or("a calibration needs a wall-mode trajectory: --mode=wall")?;
    let phase_units = |phase: &str| -> u64 {
        outcome
            .bites
            .iter()
            .filter(|row| row.phase.label() == phase)
            .map(|row| row.profile.sample_evaluations)
            .sum()
    };
    let phases = vec![
        PhasePlan::from_measurement(
            PlanPhase::Explore,
            phase_units("explore"),
            explore_seconds,
            safety_factor,
            format!(
                "the explore phase of one wall trajectory: {} bites, sampleEvaluations charged \
                 per bite by PhaseProfile, over the loop's own exploreSeconds",
                outcome.explore_bites
            ),
        )?,
        PhasePlan::from_measurement(
            PlanPhase::Compress,
            phase_units("compress"),
            (search_seconds - explore_seconds).max(f64::MIN_POSITIVE),
            safety_factor,
            format!(
                "the compress phase of the same wall trajectory: {} bites, over \
                 searchSeconds - exploreSeconds",
                outcome.compress_bites
            ),
        )?,
    ];
    Ok(WorkPlan::new(
        PlanKey {
            request_sha256: request_sha256.to_owned(),
            currency_version: CurrencyVersion::U0Samples,
            binary_key: BinaryKey {
                executable_sha256: executable_sha256.to_owned(),
                features: build_features(),
            },
            workers,
            executor: Executor::EphemeralScope,
        },
        phases,
        "overlap_ics_benchmark --cell=cutclose --mode=wall --icscal=<path>: the calibration \
         entry point. Spend it with --mode=calibrated --plan=<path>, in a different process, \
         which reads no clock.",
    ))
}

fn outcome_json(outcome: &IcsOutcome, constructor_fingerprint: &str) -> Value {
    let mut document = json!({
        "incumbent": {
            "rawSourceDepthMm": outcome.incumbent.raw_source_depth_mm,
            "fromConstructor": outcome.incumbent.from_constructor,
            "placementFingerprint": outcome.incumbent.placement_fingerprint,
            "constructorFingerprint": constructor_fingerprint,
            "fingerprintDiffersFromConstructor":
                outcome.incumbent.placement_fingerprint != constructor_fingerprint,
            "placementCount": outcome.incumbent.placements.len(),
        },
        "publications": outcome.publications,
        "firstStrictChildProposal": outcome.first_strict_child_proposal,
        "proxy": {
            "rawPhi": outcome.final_raw_phi,
            "guidedPhi": outcome.final_guided_phi,
            "maxViolationMm": outcome.final_max_violation_mm,
            "rawSourceDepthMm": outcome.final_raw_depth_mm,
        },
        "census": {
            "activePairRows": outcome.final_census.active_pairs,
            "activeEdgeRows": outcome.final_census.active_edges,
            "maxPairViolationMm": outcome.final_census.max_pair_violation_mm,
            "maxEdgeViolationMm": outcome.final_census.max_edge_violation_mm,
            "maxGuidedPenalty": outcome.final_census.max_penalty,
            // The per-side split. Two rows on opposite sides of one piece mean
            // no single rigid translation legalizes the layout, which is the
            // claim the previous round's README could not settle.
            "activeEdgeRowsBySide": {
                "left": outcome.final_census.active_edges_by_side[0],
                "right": outcome.final_census.active_edges_by_side[1],
                "bottom": outcome.final_census.active_edges_by_side[2],
                "top": outcome.final_census.active_edges_by_side[3],
            },
            "maxEdgeViolationBySideMm": {
                "left": outcome.final_census.max_edge_violation_by_side_mm[0],
                "right": outcome.final_census.max_edge_violation_by_side_mm[1],
                "bottom": outcome.final_census.max_edge_violation_by_side_mm[2],
                "top": outcome.final_census.max_edge_violation_by_side_mm[3],
            },
            "piecesSqueezedOnOppositeSides":
                outcome.final_census.pieces_squeezed_on_opposite_sides,
        },
        "rejectionCensus": rejection_census_json(&outcome.rejection_census),
        "sweeps": outcome.trace.sweeps,
        "guidedStalls": outcome.trace.guided_stalls,
        "jumps": outcome.trace.jumps,
        "jumpAttempted": outcome.trace.jump_attempted,
        "jumpCommitted": outcome.trace.jump_committed,
        // Named for what it is: "the best candidate beat the pre-jump guided
        // Φ", not "a relocation was installed". Read it beside `jumpCommitted`.
        "jumpsImprovingGuided": outcome.trace.jumps_improving_guided,
        "jumpEvents": outcome.trace.jump_events.iter().map(|row| json!({
            "proposalOrdinal": row.proposal_ordinal,
            "piece": row.piece,
            "kind": row.kind,
            "radiusMm": if row.radius_mm.is_finite() { json!(row.radius_mm) } else { json!("strip") },
            "maxViolationMm": row.max_violation_mm,
            "baselineGuidedPhi": row.baseline_guided,
            "bestGuidedPhi": row.best_guided,
            "installed": row.installed,
            "improvedGuided": row.improved_guided,
        })).collect::<Vec<_>>(),
        "work": work_json(&outcome.trace.work),
        "qualitySeries": outcome.trace.quality.iter().map(|point| json!({
            "proposalOrdinal": point.proposal_ordinal,
            "rawSourceDepthMm": point.raw_source_depth_mm,
            "strictChild": point.strict_child,
        })).collect::<Vec<_>>(),
        "exactCheckpoints": outcome.trace.checkpoints.iter().map(|row| json!({
            "proposalOrdinal": row.proposal_ordinal,
            "targetDepthMm": row.target_depth_mm,
            "maxViolationMm": row.max_violation_mm,
            "proxyRawDepthMm": row.proxy_raw_depth_mm,
            "kernelExclusiveValid": row.kernel_exclusive_valid,
            "contractValid": row.contract_valid,
            "repairRows": row.repair_rows,
            "repairMaxDisplacementMm": row.repair_max_displacement_mm,
            "repairDepthGivebackMm": row.repair_depth_giveback_mm,
            "publishedRawDepthMm": row.published_raw_depth_mm,
            "refusal": row.refusal,
        })).collect::<Vec<_>>(),
        "proxySamples": outcome.trace.proxy_samples.iter().map(|row| json!({
            "proposalOrdinal": row.proposal_ordinal,
            "targetDepthMm": row.target_depth_mm,
            "rawPhi": row.raw_phi,
            "guidedPhi": row.guided_phi,
            "maxViolationMm": row.max_violation_mm,
            "rawSourceDepthMm": row.raw_source_depth_mm,
        })).collect::<Vec<_>>(),
        "boundaryEdgeViolations": {
            "activeEdgeRows": outcome.final_census.active_edges,
            "maxEdgeViolationMm": outcome.final_census.max_edge_violation_mm,
        },
        "invalidPublications": outcome.trace.checkpoints.iter().filter(|row|
            row.published_raw_depth_mm.is_some()
                && !(row.kernel_exclusive_valid && row.contract_valid)).count(),
        "repairMaxDisplacementMm": outcome.trace.checkpoints.iter()
            .map(|row| row.repair_max_displacement_mm).fold(0.0f64, f64::max),
        "repairMaxGivebackMm": outcome.trace.checkpoints.iter()
            .filter(|row| row.published_raw_depth_mm.is_some())
            .map(|row| row.repair_depth_giveback_mm).fold(0.0f64, f64::max),
    });
    #[cfg(feature = "conflict-cluster-budget")]
    if outcome.trace.partition.partition_decisions > 0 {
        document["partition"] = partition_json(&outcome.trace.partition);
    }
    document
}

/// **The `CutCloseRelocate` trajectory, as a document.**
///
/// Everything the pre-committed gate, the funnel autopsy and the two-process
/// replay read, and nothing a clock touches outside `wall`:
///
/// * `publications` - the **only** quality series. One row per dual-valid
///   publication, with its fixed-work ordinal `(bite, attempt, iteration,
///   proposals)` so a wall run and its fixed-work replay can be lined up, its
///   `wallSeconds` so the 3/10/30 staircase is a *filter* rather than an
///   interpolation, and the exact parent fingerprint before and after.
/// * `bites` - the funnel row the failure license names, per bite:
///   `bitesStarted -> proxyBandReached -> exactAttempted -> dualValidPublished`.
/// * `fingerprints` - present only under `--fingerprints=1`; the eight-worker
///   merge-determinism vector's whole subject.
///
/// **Every publication and the incumbent now carry their layout.** The
/// evidence audit's revalidation chapter closes on "no pose is recorded for any
/// of the 1,701 publications ... re-validatable only by the process that
/// produced them" (RV2). `placements` is the pose set in the request's own
/// coordinates - the shape `sparrow_import_gate` and the `s0`/`s1`/`s2` cells
/// read - so `raw_depth_of` and both exact authorities can be re-run on it by
/// anyone. Under `--revalidate=1` this function re-runs the depth itself and
/// prints whether the recomputation matched bit for bit; it is off by default
/// because it happens between the loop's last clock read and `totalSeconds`,
/// and `wall.py` brackets a publication's age with that difference.
fn schedule_json(
    outcome: &ScheduleOutcome,
    constructor_fingerprint: &str,
    layouts: &LayoutContext<'_>,
) -> Value {
    let publications = outcome
        .publications
        .iter()
        .map(|row| {
            let placements = publish::placements_of(layouts.sources, &row.poses);
            let mut value = json!({
                "ordinal": {
                    "bite": row.ordinal.bite,
                    "attempt": row.ordinal.attempt,
                    "iteration": row.ordinal.iteration,
                    "proposals": row.ordinal.proposals,
                },
                "phase": row.phase.label(),
                "targetDepthMm": row.target_depth_mm,
                "publishedRawDepthMm": row.published_raw_depth_mm,
                "repairRows": row.repair_rows,
                "repairMaxDisplacementMm": row.repair_max_displacement_mm,
                "repairDepthGivebackMm": row.repair_depth_giveback_mm,
                "parentFingerprint": row.parent_fingerprint,
                "placementFingerprint": row.placement_fingerprint,
                "improvedIncumbent": row.improved_incumbent,
                "wallSeconds": row.wall_seconds,
                // RV2. The repaired poses, and the placements they denote.
                "poses": poses_json(&row.poses),
                "placements": placements_json(&placements),
            });
            if layouts.revalidate {
                let depth = raw_depth_of(layouts.pieces, &placements, layouts.contract);
                value["revalidation"] = json!({
                    "recomputedPlacementFingerprint": placement_fingerprint(&placements),
                    "fingerprintMatches":
                        placement_fingerprint(&placements) == row.placement_fingerprint,
                    "recomputedRawDepthMm": depth,
                    "depthMatchesBitwise": depth.to_bits()
                        == row.published_raw_depth_mm.to_bits(),
                });
            }
            value
        })
        .collect::<Vec<_>>();
    let bites = outcome
        .bites
        .iter()
        .map(|row| {
            json!({
                "ordinal": row.ordinal,
                "phase": row.phase.label(),
                "widthBeforeMm": row.bite.width_before_mm,
                "widthAfterMm": row.bite.width_after_mm,
                "deltaMm": row.bite.delta_mm,
                "splitYMm": row.bite.split_y_mm,
                "movedPieces": row.bite.moved_pieces,
                "step": row.bite.step,
                "attempts": row.attempts,
                "disruptions": row.disruptions,
                "masterIterations": row.master_iterations,
                "strikes": row.strikes,
                "minRawPhi": if row.min_raw_phi.is_finite() { json!(row.min_raw_phi) } else { Value::Null },
                "proxyBandReached": row.proxy_band_reached,
                // **Unchanged value, unchanged name.** Every committed
                // document and every audit script reads this key as the count
                // of 4 um band entries, and it still is one. The two keys
                // below are the split, not a replacement.
                "exactAttempts": row.exact_band_entries,
                // Audit F4. `exactBandEntries` is `exactAttempts` under the
                // name of the thing it counts; `exactCheckpointCalls` is the
                // number the funnel never had - how many times the exact
                // authorities were actually asked.
                "exactBandEntries": row.exact_band_entries,
                "exactCheckpointCalls": row.exact_checkpoint_calls,
                "profile": profile_json(&row.profile),
                // **The two-arm gate's per-bite terms.** Both arms carry both
                // patience counters, so the paired comparison is term by term
                // rather than shape by shape. `strikeAccumulated` is the
                // patience that had run out at each strike, summed;
                // `strikeOvershoot` is the crossing batch's own cost, and
                // `strikeAccumulated - strikeOvershoot` is what the spec's
                // "overshoot <= one batch" clause bounds.
                "strikeMeter": {
                    "batches": row.strike_shadow.batches,
                    "chargedWorkSampleEvaluations": row.strike_shadow.charged_work,
                    "substantial": row.strike_shadow.substantial,
                    "marginal": row.strike_shadow.marginal,
                    "none": row.strike_shadow.none,
                    "strikeAccumulated": row.strike_accumulated,
                    "strikeOvershoot": row.strike_overshoot,
                },
                "published": row.published.is_some(),
            })
        })
        .collect::<Vec<_>>();
    // The funnel, summed. The failure license asks for exactly this row.
    let band_reached = outcome.bites.iter().filter(|row| row.proxy_band_reached).count();
    let exact_attempted = outcome
        .bites
        .iter()
        .filter(|row| row.exact_band_entries > 0)
        .count();
    let band_entries: u64 = outcome.bites.iter().map(|row| row.exact_band_entries).sum();
    let checkpoint_calls: u64 = outcome
        .bites
        .iter()
        .map(|row| row.exact_checkpoint_calls)
        .sum();
    let mut document = json!({
        "startDepthMm": outcome.start_depth_mm,
        "depthMm": outcome.depth_mm,
        "finalWidthMm": outcome.final_width_mm,
        "exploreBites": outcome.explore_bites,
        "compressBites": outcome.compress_bites,
        "publications": publications,
        "publicationCount": outcome.publications.len(),
        "bites": bites,
        // **The funnel, with the rung the audit says it never had.**
        //
        // F4: "the failure license's funnel `bitesStarted -> proxyBandReached
        // -> exactAttempted -> dualValidPublished` has no rung that answers
        // 'how many times were the exact authorities asked', the true number
        // is `work.exactCheckpoints`, and `wall.py`'s reduction drops `work`
        // entirely. The autopsy the failure license buys is being read off two
        // numbers that are 0.6x and 3.7x the one it wants."
        //
        // `exactAttempted` keeps its value and its place, so every committed
        // document stays comparable, and `bitesWithBandEntry` is the same
        // number under the name of what it counts: BITES that entered the
        // band, not attempts. The two sums beside it are the attempts and the
        // calls. `exactCheckpointCallsReconcile` is the identity that makes
        // the split checkable from the document alone.
        "funnel": {
            "bitesStarted": outcome.bites.len(),
            "proxyBandReached": band_reached,
            "exactAttempted": exact_attempted,
            "bitesWithBandEntry": exact_attempted,
            "exactBandEntries": band_entries,
            "exactCheckpointCalls": checkpoint_calls,
            "exactCheckpointCallsReconcile":
                checkpoint_calls == outcome.trace.work.exact_checkpoints,
            "workExactCheckpoints": outcome.trace.work.exact_checkpoints,
            "dualValidPublished": outcome.publications.len(),
        },
        "fingerprints": outcome.fingerprints.iter().map(|row| json!({
            "bite": row.bite,
            "attempt": row.attempt,
            "iteration": row.iteration,
            "winner": row.winner,
            "winnerGuided": row.winner_guided,
            "contested": row.contested,
            "state": row.state,
        })).collect::<Vec<_>>(),
        "fingerprintCount": outcome.fingerprints.len(),
        "contestedIterations": outcome.fingerprints.iter().filter(|row| row.contested).count(),
        "incumbent": {
            "rawSourceDepthMm": outcome.incumbent.raw_source_depth_mm,
            "fromConstructor": outcome.incumbent.from_constructor,
            "placementFingerprint": outcome.incumbent.placement_fingerprint,
            "constructorFingerprint": constructor_fingerprint,
            "fingerprintDiffersFromConstructor":
                outcome.incumbent.placement_fingerprint != constructor_fingerprint,
            "placementCount": outcome.incumbent.placements.len(),
            // RV2, for the number the README prints as the cell's answer.
            "placements": placements_json(&outcome.incumbent.placements),
        },
        "proxy": {
            "rawPhi": outcome.final_raw_phi,
            "guidedPhi": outcome.final_guided_phi,
            "maxViolationMm": outcome.final_max_violation_mm,
            "rawSourceDepthMm": outcome.final_raw_depth_mm,
        },
        "census": {
            "activePairRows": outcome.final_census.active_pairs,
            "activeEdgeRows": outcome.final_census.active_edges,
            "maxPairViolationMm": outcome.final_census.max_pair_violation_mm,
            "maxEdgeViolationMm": outcome.final_census.max_edge_violation_mm,
            "maxGuidedPenalty": outcome.final_census.max_penalty,
        },
        "work": work_json(&outcome.trace.work),
        "relocateEconomics": relocate_economics(&outcome.trace.work),
        "sweeps": outcome.trace.sweeps,
        "strikeArm": outcome.strike_arm.arm(),
        // **The calibrated plan's closing ledger**, or `null` when no plan was
        // spending. `chargeIdentityHolds` is the spec's worst-ranked defect
        // class as one boolean: the sum of the per-batch deltas plus the tail
        // the last barrier did not see equals the trajectory's own five
        // counters, so nothing was charged twice and nothing was charged to
        // nobody.
        "calibrated": match &outcome.calibrated {
            None => Value::Null,
            Some(row) => json!({
                "exploreAllocationUnits": row.explore_allocation,
                "compressAllocationUnits": row.compress_allocation,
                "exploreConsumedUnits": row.explore_consumed,
                "compressConsumedUnits": row.compress_consumed,
                "exploreBatches": row.explore_batches,
                "compressBatches": row.compress_batches,
                // The overshoot clause's own numerator: `consumed -
                // allocation` cannot exceed the batch that crossed.
                "exploreCrossingBatchUnits": row.explore_crossing_batch_units,
                "compressCrossingBatchUnits": row.compress_crossing_batch_units,
                "charged": work_terms_json(&row.charged),
                "unchargedTail": work_terms_json(&row.uncharged_tail),
                "trajectory": work_terms_json(&row.trajectory),
                "chargeIdentityHolds": row.charge_identity_holds,
                "consumedUnits": row.consumed_units,
                "consumedUnitsMatchCharged": row.consumed_units_match_charged,
                "currencyVersion": row.currency_version.as_str(),
                "budgetSeconds": row.budget_seconds,
                "exploreRatio": row.explore_ratio,
                "planKey": serde_json::to_value(&row.plan_key).unwrap_or(Value::Null),
            }),
        },
        "exactCheckpoints": outcome.trace.checkpoints.iter().map(|row| json!({
            "proposalOrdinal": row.proposal_ordinal,
            "targetDepthMm": row.target_depth_mm,
            "maxViolationMm": row.max_violation_mm,
            "proxyRawDepthMm": row.proxy_raw_depth_mm,
            "kernelExclusiveValid": row.kernel_exclusive_valid,
            "contractValid": row.contract_valid,
            "repairRows": row.repair_rows,
            "repairMaxDisplacementMm": row.repair_max_displacement_mm,
            "repairDepthGivebackMm": row.repair_depth_giveback_mm,
            "publishedRawDepthMm": row.published_raw_depth_mm,
            "refusal": row.refusal,
        })).collect::<Vec<_>>(),
        // The two invariant clauses of the gate, computed here so no reader has
        // to re-derive them: a single invalid publication is a FAIL for the
        // whole round, whatever any depth says.
        "invalidPublications": outcome.trace.checkpoints.iter().filter(|row|
            row.published_raw_depth_mm.is_some()
                && !(row.kernel_exclusive_valid && row.contract_valid)).count(),
        "repairMaxDisplacementMm": outcome.trace.checkpoints.iter()
            .filter(|row| row.published_raw_depth_mm.is_some())
            .map(|row| row.repair_max_displacement_mm).fold(0.0f64, f64::max),
        "repairMaxGivebackMm": outcome.trace.checkpoints.iter()
            .filter(|row| row.published_raw_depth_mm.is_some())
            .map(|row| row.repair_depth_giveback_mm).fold(0.0f64, f64::max),
    });
    #[cfg(feature = "conflict-cluster-budget")]
    if outcome.trace.partition.partition_decisions > 0 {
        document["partition"] = partition_json(&outcome.trace.partition);
    }
    #[cfg(feature = "minimum-conflict-binary-close")]
    if !outcome.binary_close.decisions.is_empty() {
        document["binaryClose"] = binary_close_json(&outcome.binary_close);
    }
    document
}

/// **The relocate metric version**, arbitration 4 / Sol review 17 Round 2 §2.
///
/// New names for new economics. The committed cold-Φ, row-rebuild and cell-gap
/// thresholds stay literal in [`throughput`] under their original meaning; these
/// counters describe the operator that replaced the proposal ladder, and none of
/// them is allowed to be read as the retired 100 K proposal pin.
fn relocate_economics(work: &WorkVector) -> Value {
    json!({
        "sampleEvaluations": work.sample_evaluations,
        "sampleEvaluationsPerRelocate": work.sample_evaluations_per_relocate(),
        "relocates": work.relocates,
        "focusedSamples": work.focused_samples,
        "containerSamples": work.container_samples,
        "containerWinners": work.container_winners,
        "focusedWinners": work.focused_winners,
        "stayPutWinners": work.stay_put_winners,
        // The neutered-relocate tripwire's counter. `containerSamples >= 50`
        // beside `containerCommits == 0` is the pre-named defect.
        "containerCommits": work.container_commits,
        "acceptedMoves": work.accepted_moves,
        "disruptions": work.disruptions,
        "disruptionMoves": work.disruption_moves,
    })
}

/// **What the trajectory was allowed to spend, and what it spent.**
///
/// The locked-strip regressions are now denominated in relocate-evals, per Grok
/// review 12 Round 1 §4.3 ("Work quota for S1: 200,000 **relocate-evals** (not
/// PGS proposals)"). Both currencies are printed, because they are not
/// interchangeable and the whole point of arbitration 4 is that no reader is
/// asked to convert one into the other:
///
/// * `pieceProposals` is a **slot** - `n` per sweep, most of them empty once
///   the colliding set has shrunk;
/// * `sampleEvaluations` is what the operator actually paid for.
///
/// `stopReason` names which quota bound the run, so a cell that stopped early
/// for the *other* reason cannot be read as one that spent its budget. The
/// proposal test is "one more sweep would not fit", not "the counter reached the
/// number", because `Engine::run`'s condition is `proposals + n <= budget` and a
/// 61-piece sweep therefore stops at 199,958 of 200,000.
///
/// **Both quotas are kept, and that is a finding rather than belt-and-braces.**
/// A relocate-eval quota alone does *not* terminate a locked-strip trajectory:
/// once the layout converges the colliding set is empty, every further sweep
/// relocates nothing and spends **zero** relocate-evals, so the quota is never
/// reached and the loop spins until something else stops it. Measured, on S1:
/// with the proposal backstop removed the cell ran 10^9 empty slots in 155 s and
/// still finished 116,406 relocate-evals short of a 200,000 cap. The relocate-eval
/// budget is the *work* the operator is licensed to spend; the proposal budget is
/// what makes a converged cell stop.
fn quota_json(config: &IcsConfig, work: &WorkVector, pieces: usize) -> Value {
    let proposals_bound = config.proposal_budget != u64::MAX
        && work.piece_proposals + pieces.max(1) as u64 > config.proposal_budget;
    let relocate_bound = config.relocate_eval_budget != u64::MAX
        && work.sample_evaluations >= config.relocate_eval_budget;
    json!({
        "proposalBudget": config.proposal_budget,
        "relocateEvalBudget": if config.relocate_eval_budget == u64::MAX {
            Value::Null
        } else {
            json!(config.relocate_eval_budget)
        },
        "pieceProposalsSpent": work.piece_proposals,
        "sampleEvaluationsSpent": work.sample_evaluations,
        "relocatesSpent": work.relocates,
        "sampleEvaluationsPerRelocate": work.sample_evaluations_per_relocate(),
        "stopReason": match (relocate_bound, proposals_bound) {
            (true, _) => "relocateEvalBudget",
            (false, true) => "proposalBudget",
            (false, false) => "converged-or-cadence",
        },
    })
}

// -------------------------------------------------------------------- main ---

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = Options::parse()?;
    let cell = options.required("cell")?.to_owned();
    let request_path = options.required("request")?.to_owned();
    let request_bytes = fs::read(&request_path)?;
    let request_sha256 = format!("{:x}", Sha256::digest(&request_bytes));
    let request: Request = serde_json::from_slice(&request_bytes)?;

    let (request_total_padding_mm, allow_global_rotation, allow_global_mirror, geometry) =
        match (&request.settings, &request.options) {
            (Some(settings), None) => (
                settings.padding,
                settings.allow_global_rotation,
                settings.allow_global_mirror,
                settings.geometry,
            ),
            (None, Some(legacy)) => (
                request
                    .padding
                    .ok_or("legacy requests require top-level padding")?,
                legacy.allow_global_rotation,
                legacy.allow_global_mirror,
                legacy.irregular_settings.geometry,
            ),
            _ => return Err("a request must contain settings or legacy options, not both".into()),
        };
    let source_by_id = request
        .source_pieces
        .iter()
        .map(|source| (source.id.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    let normalize_axes = request.sheet.width >= request.sheet.height;
    let owned = request
        .pieces
        .iter()
        .map(|piece| {
            let source = *source_by_id
                .get(piece.source_piece_id.as_str())
                .ok_or_else(|| format!("missing source piece {}", piece.source_piece_id))?;
            let polygon =
                polygon_set_from_imported_piece(source, geometry.flattening_sag_tolerance_mm)?;
            let polygon = if normalize_axes {
                let rotated = polygon.transformed(270.0, false, 0.0, 0.0)?;
                let bounds = rotated.bounds().ok_or("cannot normalize empty geometry")?;
                rotated.translated(-bounds.min_x, -bounds.min_y)?
            } else {
                polygon
            };
            Ok(OwnedPiece {
                id: piece.id.clone(),
                polygon,
                allow_rotation: allow_global_rotation && piece.allow_rotation,
                allow_mirror: allow_global_mirror && piece.allow_mirror,
            })
        })
        .collect::<Result<Vec<OwnedPiece>, Box<dyn std::error::Error>>>()?;
    // `--rotation=off` freezes theta for every piece. It is a **diagnostic**,
    // not a configuration: the converged spec's whole point about the search
    // coordinate is that theta is continuous from the first sweep, so a cell
    // run with rotation off is a probe of what the rotation axis is
    // contributing, never a gate result.
    let rotation_frozen = matches!(options.get("rotation"), Some("off"));
    let pieces = owned
        .iter()
        .map(|piece| GeneralFastPiece {
            id: &piece.id,
            polygon: &piece.polygon,
            allow_rotation: piece.allow_rotation && !rotation_frozen,
            allow_mirror: piece.allow_mirror,
        })
        .collect::<Vec<_>>();

    let mut settings = GeneralFastSettings::deterministic_test(
        request.sheet.width.min(request.sheet.height),
        request.sheet.width.max(request.sheet.height),
    );
    settings.total_padding_mm = options.number("pair", request_total_padding_mm)?;
    settings.sheet_edge_clearance_mm =
        Some(options.number("edge", settings.total_padding_mm / 2.0)?);
    settings.clearance_safety_margin_mm = geometry.clearance_safety_margin_mm;
    settings.flattening_sag_tolerance_mm = geometry.flattening_sag_tolerance_mm;
    // The search-offset allowance reaches **one** consumer: the constructor
    // arm that produces the anytime floor, where it is the campaign's pinned
    // 0.002 mm. It reaches nothing else, and it cannot:
    //
    // * Φ's clearance is `total_padding + 2 * sag`, read off the material
    //   contract, and `Contract` has no allowance field at all;
    // * the round kernel's radius is `total_padding / 2 + safety`, allowance
    //   excluded by construction;
    // * `publish::publication_settings` forces it to zero before the contract
    //   validator ever sees the settings.
    //
    // It is not zero here because the constructor's own envelope *is* the
    // exact contract at zero, and a coincident envelope refuses its own legal
    // layouts on exact contact.
    settings.search_offset_allowance_mm = options.number("allowance", 0.002)?;
    // The constructor's own portfolio, matching the campaign's pinned tail:
    // order variants 4, catalogue 1, angle seeds 16, max angles 4.
    settings.max_order_variants = options.integer("orders", 4)? as usize;
    settings.angle_seed_count = options.integer("angleseeds", 16)? as usize;
    settings.max_angles_per_piece = options.integer("maxangles", 4)? as usize;

    let contract = Contract::from_settings(settings);
    let sources = piece_sources(&pieces)?;
    let lower_scale_mm = homotopy::lower_scale_mm(&sources, &contract);
    let seed = options.integer("seed", 0)?;

    let mut wall = serde_json::Map::new();
    let started = Instant::now();

    let mut document = json!({
        "experiment": "overlap-ics",
        "cell": cell,
        "instrument": "crates/polygon-nesting-core/src/search/overlap_ics/",
        "request": {
            "path": request_path,
            "sha256": request_sha256,
            "sheetShortAxisMm": settings.sheet_short_axis_mm,
            "sheetLongAxisMm": settings.sheet_long_axis_mm,
            "normalizeAxes": normalize_axes,
            "pieceCount": pieces.len(),
        },
        "contract": {
            "pairClearanceMm": contract.pair_clearance_mm(),
            // `sheetEdgeClearanceMm` keeps its previous meaning and value -
            // `edge + sag`, the physical sheet rule - so the previous round's
            // documents and `residual_split.py` stay readable. The two names
            // beside it are the split this round introduced.
            "sheetEdgeClearanceMm": contract.physical_edge_clearance_mm(),
            "physicalEdgeClearanceMm": contract.physical_edge_clearance_mm(),
            "depthTopInsetMm": contract.depth_top_inset_mm(),
            "expansionMm": contract.expansion_mm(),
            "twoRMicron": (contract.expansion_mm() * 2000.0).round(),
            "sheetInsetMm": contract.sheet_inset_mm(),
            "searchOffsetAllowanceMm": settings.search_offset_allowance_mm,
            "flatteningSagToleranceMm": settings.flattening_sag_tolerance_mm,
        },
        "lowerScaleMm": lower_scale_mm,
        "seed": seed,
        "rotationFrozen": rotation_frozen,
    });

    match cell.as_str() {
        "partition-vectors" => {
            #[cfg(feature = "conflict-cluster-budget")]
            {
                document["partitionVectors"] =
                    partition_vectors_json(&partition_gate0_vector_report());
            }
            #[cfg(not(feature = "conflict-cluster-budget"))]
            return Err("partition-vectors requires conflict-cluster-budget".into());
        }
        "binary-close-vectors" => {
            #[cfg(feature = "minimum-conflict-binary-close")]
            {
                let placements = ShortSideFirst.layout(&pieces, settings)?;
                let constructor_depth = raw_depth_of(&pieces, &placements, &contract);
                let config = IcsConfig {
                    target_depth_mm: constructor_depth,
                    proposal_budget: 0,
                    relocate_eval_budget: u64::MAX,
                    checkpoint_every_sweeps: u64::MAX,
                    descent: descent_config(&options, &contract, &sources, seed)?,
                    limits: publication_limits(&options)?,
                };
                let engine = Engine::from_constructor_at_depth(
                    &pieces,
                    settings,
                    &placements,
                    constructor_depth,
                    config,
                )?;
                let decision = engine.binary_close_vector(1);
                let real_trace = BinaryCloseTrace {
                    invalid_decisions: u64::from(!decision.valid),
                    decisions: vec![decision],
                };
                let geometry_vector = geometry_gate0_vector_report();
                let synthetic_geometry_trace = BinaryCloseTrace {
                    invalid_decisions: u64::from(!geometry_vector.decision.valid),
                    decisions: vec![geometry_vector.decision],
                };
                document["binaryCloseVectors"] = json!({
                    "synthetic": binary_close_vectors_json(&binary_close_gate0_vector_report()),
                    "syntheticGeometry": {
                        "poseStates": geometry_vector.pose_states.iter().map(|state| json!({
                            "piece": state.piece,
                            "zeroBits": state.zero,
                            "oneBits": state.one,
                            "mirrored": state.mirrored,
                        })).collect::<Vec<_>>(),
                        "incrementalPoseDigestSha256":
                            hex_bytes(&geometry_vector.incremental_pose_digest_sha256),
                        "incrementalRowDigestSha256":
                            hex_bytes(&geometry_vector.incremental_row_digest_sha256),
                        "incrementalRawPhiBits": geometry_vector.incremental_raw_phi.to_bits(),
                        "incrementalMatchesCold": geometry_vector.incremental_matches_cold,
                        "decision": binary_close_json(&synthetic_geometry_trace),
                    },
                    "realGeometry": binary_close_json(&real_trace),
                });
            }
            #[cfg(not(feature = "minimum-conflict-binary-close"))]
            return Err("binary-close-vectors requires minimum-conflict-binary-close".into());
        }
        "s0" | "s1" | "s2" => {
            let poses_path = options.required("poses")?.to_owned();
            let poses_bytes = fs::read(&poses_path)?;
            let poses_sha256 = format!("{:x}", Sha256::digest(&poses_bytes));
            let fixture: PoseFixture = serde_json::from_slice(&poses_bytes)?;
            let placements = fixture
                .placements
                .iter()
                .map(|pose| GeneralFastPlacement {
                    piece_id: pose.piece_id.clone(),
                    rotation_deg: pose.rotation_deg,
                    mirrored: pose.mirrored,
                    translate_short_axis: pose.translate_short_axis,
                    translate_long_axis: pose.translate_long_axis,
                })
                .collect::<Vec<_>>();
            let imported = poses_of(&pieces, &sources, &placements)?;
            let imported_depth = raw_depth_of(&pieces, &placements, &contract);
            let target = options.number("target", imported_depth)?;
            let (magnitude_mm, magnitude_deg) = match cell.as_str() {
                "s0" => (0.0, 0.0),
                "s1" => (
                    options.number("perturbmm", 0.5)?,
                    options.number("perturbdeg", 2.0)?,
                ),
                _ => (
                    options.number("perturbmm", 2.0)?,
                    options.number("perturbdeg", 10.0)?,
                ),
            };
            let poses = perturb(&imported, seed, magnitude_mm, magnitude_deg);
            let perturbation_digest = pose_digest(&poses);
            let config = IcsConfig {
                target_depth_mm: target,
                proposal_budget: options.integer("budget", 0)?,
                relocate_eval_budget: options.integer("relocateevals", u64::MAX)?,
                checkpoint_every_sweeps: options.integer("checkpointevery", 1)?,
                descent: descent_config(&options, &contract, &sources, seed)?,
                limits: publication_limits(&options)?,
            };
            let incumbent = ExactIncumbent {
                placements: Vec::new(),
                raw_source_depth_mm: f64::INFINITY,
                from_constructor: true,
                placement_fingerprint: placement_fingerprint(&placements),
            };
            let solver_started = Instant::now();
            let mut engine = Engine::from_poses(
                &pieces,
                settings,
                sources.clone(),
                contract,
                poses,
                incumbent,
                config,
            );
            let entry_totals = engine.totals();
            let entry_depth = engine.raw_depth_mm();
            let outcome = engine.run();
            wall.insert(
                "solverSeconds".to_owned(),
                json!(solver_started.elapsed().as_secs_f64()),
            );
            document["poses"] = json!({
                "path": poses_path,
                "sha256": poses_sha256,
                "placementCount": placements.len(),
                "importedRawSourceDepthMm": imported_depth,
                "perturbationMm": magnitude_mm,
                "perturbationDeg": magnitude_deg,
                "perturbedPoseDigest": perturbation_digest,
            });
            document["entry"] = json!({
                "rawPhi": entry_totals.raw,
                "rawPhiBits": entry_totals.raw.to_bits(),
                "guidedPhi": entry_totals.guided,
                "maxViolationMm": entry_totals.max_violation_mm,
                "rawSourceDepthMm": entry_depth,
                "lockedTargetMm": target,
            });
            document["outcome"] = outcome_json(&outcome, &placement_fingerprint(&placements));
            document["quota"] = quota_json(&config, &outcome.trace.work, pieces.len());
        }
        "constructor" | "c175" | "c168" | "triangle" | "run" | "partition-cost" => {
            let constructor_started = Instant::now();
            let placements = ShortSideFirst.layout(&pieces, settings)?;
            wall.insert(
                "constructorSeconds".to_owned(),
                json!(constructor_started.elapsed().as_secs_f64()),
            );
            let constructor_depth = raw_depth_of(&pieces, &placements, &contract);
            let constructor_fingerprint = placement_fingerprint(&placements);
            let parent = poses_of(&pieces, &sources, &placements)?;
            let target = match cell.as_str() {
                "c175" | "partition-cost" => {
                    constructor_depth - 0.10 * (constructor_depth - lower_scale_mm)
                }
                _ => options.number("target", constructor_depth)?,
            };
            document["constructor"] = json!({
                "rawSourceDepthMm": constructor_depth,
                "placementFingerprint": constructor_fingerprint,
                "placementCount": placements.len(),
                "lowerScaleMm": lower_scale_mm,
                "shockResidual": 0.10,
                "shockMm": constructor_depth - target,
                "halfShockMm": 0.05 * (constructor_depth - lower_scale_mm),
            });
            if cell == "constructor" {
                document["lockedTargetMm"] = json!(target);
            } else {
                // The shock, written out rather than hidden inside the engine:
                // the constructor's poses displaced by a seed-keyed SE(2)
                // vector, and *then* affinely compressed onto the locked
                // target.
                //
                // The displacement is what makes three seeds three
                // *trajectories*. Without it the descent is seed-independent
                // (the ladder, the sweep order and the weight rule are all
                // deterministic functions of the state), so "three fixed seeds"
                // would be one run reported three times - exactly the
                // "three seeds repeated three times are not nine seeds"
                // objection Sol review 14 §3 raises. Sol R2 §4 sanctions the
                // construction: distinct workers use "distinct deterministic
                // affine perturbations/jump streams" from the same constructor.
                //
                // **The order was wrong and it changed the cell.** Compressing
                // first and perturbing after put the entry state up to one
                // shock magnitude *outside* the locked strip - about 0.8 mm on
                // C175 - so what ran was "affine shock plus a random throw past
                // the target", not the cell the arbitration named (Sol review
                // 15 §A.4). Perturbing the parent and compressing each
                // perturbed parent onto the same `T` gives three distinct
                // trajectories that all start inside their own target, which is
                // what the assertion below now requires of every seed.
                let perturbed_parent = perturb(
                    &parent,
                    seed,
                    options.number("shockmm", 0.25)?,
                    options.number("shockdeg", 1.0)?,
                );
                let factor = homotopy::affine_compression_factor(
                    &sources,
                    &perturbed_parent,
                    &contract,
                    target,
                );
                let shocked =
                    homotopy::compressed(&sources, &perturbed_parent, &contract, factor);
                let config = IcsConfig {
                    target_depth_mm: target,
                    proposal_budget: options.integer("budget", 100_000)?,
                    relocate_eval_budget: options.integer("relocateevals", u64::MAX)?,
                    checkpoint_every_sweeps: options.integer("checkpointevery", 1)?,
                    descent: descent_config(&options, &contract, &sources, seed)?,
                    limits: publication_limits(&options)?,
                };
                let incumbent = ExactIncumbent {
                    placements: placements.clone(),
                    raw_source_depth_mm: constructor_depth,
                    from_constructor: true,
                    placement_fingerprint: constructor_fingerprint.clone(),
                };
                let solver_started = Instant::now();
                let mut engine = Engine::from_poses(
                    &pieces,
                    settings,
                    sources.clone(),
                    contract,
                    shocked.clone(),
                    incumbent,
                    config,
                );
                let entry_totals = engine.totals();
                let entry_depth = engine.raw_depth_mm();
                // A hard failure, not a warning. The whole point of the shock
                // is that the trajectory starts *at* a target it cannot yet
                // satisfy; a state that starts outside the strip is a different
                // cell and must not be reported as this one.
                if entry_depth > target + 1e-9 {
                    return Err(format!(
                        "cell `{cell}` entered at {entry_depth} mm, outside its locked target \
                         {target} mm: the shock must be applied to the parent and compressed \
                         onto the target, never applied after the compression"
                    )
                    .into());
                }
                #[cfg(feature = "conflict-cluster-budget")]
                if cell == "partition-cost" {
                    let sequence = options.get("sequence").unwrap_or("AB");
                    if sequence != "AB" && sequence != "BA" {
                        return Err("--sequence must be AB|BA".into());
                    }
                    let warmups = options.integer("warmups", 32)? as usize;
                    let measured = options.integer("measured", 256)? as usize;
                    let samples = sequence
                        .chars()
                        .map(|label| {
                            let arm = match label {
                                'A' => PartitionArm::Off,
                                'B' => PartitionArm::ComputeIgnore,
                                _ => unreachable!(),
                            };
                            engine.partition_cost_arm(arm, warmups, measured)
                        })
                        .collect::<Vec<_>>();
                    let by_arm = |arm: PartitionArm| {
                        samples
                            .iter()
                            .find(|sample| sample.arm == arm)
                            .expect("both cost arms ran")
                    };
                    let off = by_arm(PartitionArm::Off);
                    let compute = by_arm(PartitionArm::ComputeIgnore);
                    document["partitionCost"] = json!({
                        "sequence": sequence,
                        "warmups": warmups,
                        "measured": measured,
                        "samples": samples.iter().map(partition_cost_json).collect::<Vec<_>>(),
                        "ratioComputeIgnoreOverOff":
                            compute.slots_per_second / off.slots_per_second,
                        "poseIdentity": compute.pose_sequence_digest_sha256
                            == off.pose_sequence_digest_sha256,
                        "orderIdentity": compute.consumed_order_digest_sha256
                            == off.consumed_order_digest_sha256,
                        "workIdentity": compute.work == off.work,
                        "actualSlotsIdentity": compute.completed_atomic_slots
                            == off.completed_atomic_slots,
                        "offActualMatchesExpected": off.completed_atomic_slots
                            == off.expected_atomic_slots,
                        "computeActualMatchesExpected": compute.completed_atomic_slots
                            == compute.expected_atomic_slots,
                        "computePartitionSlotsMatchActual": compute.partition.partition_slots
                            == compute.completed_atomic_slots,
                        "legacyProposalIdentity": compute.legacy_proposals
                            == off.legacy_proposals,
                        "computeSlotIdentitiesHold": compute.partition.slot_identities_hold(),
                        "computeInvalidFallbacks":
                            compute.partition.invalid_fallback_decisions,
                    });
                    wall.insert(
                        "solverSeconds".to_owned(),
                        json!(solver_started.elapsed().as_secs_f64()),
                    );
                    document["shock"] = json!({
                        "affineFactor": factor,
                        "entryDepthWithinTarget": entry_depth <= target + 1e-9,
                        "entryDepthSlackMm": target - entry_depth,
                        "shockMm": options.number("shockmm", 0.25)?,
                        "shockDeg": options.number("shockdeg", 1.0)?,
                        "shockedPoseDigest": pose_digest(&shocked),
                    });
                    document["entry"] = json!({
                        "rawPhi": entry_totals.raw,
                        "guidedPhi": entry_totals.guided,
                        "maxViolationMm": entry_totals.max_violation_mm,
                        "rawSourceDepthMm": entry_depth,
                        "lockedTargetMm": target,
                    });
                    document["finalPoseDigest"] = json!(pose_digest(engine.state().poses.as_slice()));
                }
                #[cfg(not(feature = "conflict-cluster-budget"))]
                if cell == "partition-cost" {
                    return Err("partition-cost requires conflict-cluster-budget".into());
                }
                if cell != "partition-cost" {
                    let outcome = engine.run();
                    wall.insert(
                        "solverSeconds".to_owned(),
                        json!(solver_started.elapsed().as_secs_f64()),
                    );
                    document["shock"] = json!({
                        "affineFactor": factor,
                        "entryDepthWithinTarget": entry_depth <= target + 1e-9,
                        "entryDepthSlackMm": target - entry_depth,
                        "shockMm": options.number("shockmm", 0.25)?,
                        "shockDeg": options.number("shockdeg", 1.0)?,
                        "shockedPoseDigest": pose_digest(&shocked),
                    });
                    document["entry"] = json!({
                        "rawPhi": entry_totals.raw,
                        "guidedPhi": entry_totals.guided,
                        "maxViolationMm": entry_totals.max_violation_mm,
                        "rawSourceDepthMm": entry_depth,
                        "lockedTargetMm": target,
                    });
                    document["outcome"] = outcome_json(&outcome, &constructor_fingerprint);
                    document["quota"] = quota_json(&config, &outcome.trace.work, pieces.len());
                    document["finalPoseDigest"] = json!(pose_digest(&outcome.final_poses));
                }
            }
        }
        // ---------------------------------------------------- CutCloseRelocate --
        //
        // **The live loop, and the only cell any gate verdict rests on.**
        //
        //   --cell=cutclose --mode=wall  --wall=10.0 --workers=8 --seed=S
        //   --cell=cutclose --mode=fixed --bites=8 --attempts=2 --iters=400
        //                   --compressbites=2 --workers=8 --seed=S
        //
        // **The economics round adds one flag and one mode**, and neither
        // changes anything unless it is named:
        //
        //   --arm=control|treatment       default `control`, the closed member
        //   --mode=calibrated --plan=P    spend an `icscal/v1` plan, no clock
        //   --icscal=P                    (on `--mode=wall`) write one
        //
        // `--arm` selects the strike policy and nothing else: the control is
        // the frozen `200 / 3 / 100 / 5 / 0.98` read off `SeparateLimits`, the
        // treatment is the KNOB quanta `1_630_000` / `815_000`. The default
        // invocation - no `--arm` - is the control, so every committed cell
        // and every existing driver call runs exactly the trajectory it
        // always ran; `economics-round/integration/armgate.py` measures that
        // against the round's base binary rather than asserting it.
        //
        // `--mode=calibrated` is the third budget. It reads a plan, refuses it
        // by name if any key field disagrees with what this process is asking
        // (request, currency, binary sha and features, workers, executor), and
        // otherwise spends `--wall` seconds *of calibrated work* at the plan's
        // own previously measured rate. It constructs no `Instant` at all - a
        // miss is an exit status and never a fallback to measuring, because
        // that fallback is the live probe the spec forbids. The extra guards:
        //
        //   --currency=U0|U1       what the runner is asking for; default U0
        //   --calattempts=N        failed separations per width, 0 = unlimited
        //
        // The clock starts on the **decoded bare request** (`started`, at the top
        // of `main`) and the constructor is charged against it but never capped -
        // arbitration 3, "a load-dependent start would break the determinism
        // contract; the ~1.4 s is charged, not enforced". So the wall handed to
        // the loop is `--wall` minus whatever the constructor actually spent, and
        // a constructor that overran the whole budget leaves the loop zero
        // seconds and the constructor's own layout as the anytime floor. That is
        // the honest degenerate case and it is reported, not hidden: Grok review
        // 12 Round 2 §6.6, "constructor-only at 3 s is allowed and expected".
        //
        // `--mode=fixed` constructs no `Instant` inside the trajectory at all.
        "cutclose" => {
            let constructor_started = Instant::now();
            let placements = ShortSideFirst.layout(&pieces, settings)?;
            let constructor_seconds = constructor_started.elapsed().as_secs_f64();
            wall.insert("constructorSeconds".to_owned(), json!(constructor_seconds));
            let constructor_depth = raw_depth_of(&pieces, &placements, &contract);
            let constructor_fingerprint = placement_fingerprint(&placements);
            let mode = options.get("mode").unwrap_or("fixed").to_owned();
            let workers = options.integer("workers", 8)? as usize;
            // **The arm, and nothing else about the arm.** `--arm=control` is
            // the closed member: the frozen `200 / 3 / 100 / 5 / 0.98` read
            // off `SeparateLimits` rather than restated here.
            // `--arm=treatment` is the spec's work-denominated impatient
            // policy at the KNOB quanta. The default is the control, so every
            // committed cell and every existing driver invocation runs the
            // trajectory it always ran.
            let arm = options.get("arm").unwrap_or("control").to_owned();
            let strikes = match arm.as_str() {
                "control" => StrikeConfig::CONTROL,
                "treatment" => StrikeConfig::TREATMENT,
                other => {
                    return Err(format!("--arm must be control|treatment, not `{other}`").into())
                }
            };
            #[cfg(feature = "minimum-conflict-binary-close")]
            let binary_close_arm = binary_close_arm(&options, "binaryclose")?;
            let schedule = ScheduleConfig {
                workers,
                strikes,
                record_fingerprints: options.integer("fingerprints", 0)? != 0,
                #[cfg(feature = "minimum-conflict-binary-close")]
                binary_close_arm,
                ..ScheduleConfig::default()
            };
            let wall_budget_s = options.number("wall", 10.0)?;
            // Built before the budget, because a calibrated budget moves into
            // `run_cutclose` and a plan is worth printing whether or not the
            // trajectory that spends it succeeds.
            let mut plan_document = Value::Null;
            let budget = match mode.as_str() {
                "wall" => {
                    // The clock started at the decoded request, not here.
                    let remaining = wall_budget_s - started.elapsed().as_secs_f64();
                    Budget::Wall {
                        remaining_seconds: remaining.max(0.0),
                    }
                }
                "fixed" => Budget::FixedWork {
                    explore_bites: options.integer("bites", 8)?,
                    compress_bites: options.integer("compressbites", 0)?,
                    attempts_per_bite: options.integer("attempts", 1)?,
                    iterations_per_separation: options.integer("iters", 400)?,
                },
                // **The 10-second calibrated work plan.** A file measured by
                // some earlier process, read here, matched against what this
                // process is actually asking, and spent in units. No clock is
                // constructed anywhere on this path - not even to decide the
                // budget, which is the plan's `--wall` seconds converted at
                // the plan's own previously measured rate.
                //
                // A key that does not match is a **hard error**. There is no
                // "measure it now" branch, because that branch is precisely
                // the live probe on a gated trajectory the spec forbids.
                "calibrated" => {
                    let path = options
                        .required("plan")
                        .map_err(|_| "--mode=calibrated needs --plan=<icscal/v1 file>")?;
                    let bytes = fs::read(path)
                        .map_err(|error| format!("reading the plan `{path}`: {error}"))?;
                    let plan = plan_from_bytes(&bytes)?;
                    let wanted_currency = match options.get("currency").unwrap_or("U0") {
                        "U0" => CurrencyVersion::U0Samples,
                        "U1" => CurrencyVersion::U1Weighted,
                        other => {
                            return Err(format!("--currency must be U0|U1, not `{other}`").into())
                        }
                    };
                    let wanted = PlanKey {
                        request_sha256: request_sha256.clone(),
                        currency_version: wanted_currency,
                        binary_key: BinaryKey {
                            executable_sha256: executable_sha256().unwrap_or_default(),
                            features: build_features(),
                        },
                        workers,
                        executor: Executor::EphemeralScope,
                    };
                    let verdict = match_plan(&wanted, &plan.key);
                    if let PlanMatch::Miss(reason) = &verdict {
                        return Err(format!(
                            "the plan at `{path}` is not this run's plan: {reason}. \
                             A miss is an answer, not a licence to measure one now: \
                             calibrate offline and re-run."
                        )
                        .into());
                    }
                    let currency = match plan.currency {
                        Some(coefficients) => Currency::u1(coefficients),
                        None => Currency::U0,
                    };
                    let pacer = WorkPlanPacer::from_plan(
                        &plan,
                        &currency,
                        wall_budget_s,
                        schedule.explore_time_ratio,
                        NoClock,
                    )?;
                    plan_document = json!({
                        "path": path,
                        "sourceSha256": format!("{:x}", Sha256::digest(&bytes)),
                        "summary": plan.summary(),
                        "currency": currency.summary(),
                        "match": "hit",
                        "wantedKey": serde_json::to_value(&wanted)?,
                        "plan": serde_json::to_value(&plan)?,
                        "budgetSeconds": wall_budget_s,
                        "exploreRatio": schedule.explore_time_ratio,
                        "exploreAllocationUnits": pacer.allocation(PlanPhase::Explore),
                        "compressAllocationUnits": pacer.allocation(PlanPhase::Compress),
                    });
                    Budget::CalibratedWork {
                        plan: Box::new(pacer),
                        attempts_per_bite: options.integer("calattempts", 0)?,
                    }
                }
                other => {
                    return Err(format!(
                        "--mode must be wall|fixed|calibrated, not `{other}`"
                    )
                    .into())
                }
            };
            // The budget's own description, taken **before** it is spent: a
            // calibrated budget carries a plan and moves into the trajectory.
            let budget_json = match &budget {
                Budget::FixedWork {
                    explore_bites,
                    compress_bites,
                    attempts_per_bite,
                    iterations_per_separation,
                } => json!({
                    "exploreBites": explore_bites,
                    "compressBites": compress_bites,
                    "attemptsPerBite": attempts_per_bite,
                    "iterationsPerSeparation": iterations_per_separation,
                }),
                Budget::Wall { .. } | Budget::CalibratedWork { .. } => Value::Null,
            };
            let calibrated_attempts_per_bite = match &budget {
                Budget::CalibratedWork {
                    attempts_per_bite, ..
                } => json!(attempts_per_bite),
                _ => Value::Null,
            };
            let config = IcsConfig {
                // Overridden by `from_constructor_at_depth` with `D*`; named here
                // so the record shows what the cell asked for.
                target_depth_mm: constructor_depth,
                proposal_budget: 0,
                relocate_eval_budget: u64::MAX,
                checkpoint_every_sweeps: u64::MAX,
                descent: descent_config(&options, &contract, &sources, seed)?,
                limits: publication_limits(&options)?,
            };
            let mut engine = Engine::from_constructor_at_depth(
                &pieces,
                settings,
                &placements,
                constructor_depth,
                config,
            )?;
            // **The offset between the two clocks, emitted rather than
            // bracketed.**
            //
            // `PublishedBite.wallSeconds` is `Pacer::elapsed_s()` and the
            // `Pacer` is constructed on the next line, inside
            // `Engine::run_cutclose`; `--wall` is measured from the decoded
            // request, which is `started`. The audit's F1/F2 are both about
            // the gap between those two clocks, and until now the only way to
            // bound it from the document was `constructorSeconds` below and
            // `totalSeconds - searchSeconds` above - an upper bound that
            // includes the whole document build, and therefore widens every
            // time this driver emits more evidence.
            //
            // This is the offset itself, read one statement before the pacer
            // exists: constructor, engine construction and nothing else. What
            // it still cannot see is the call prologue and `Pacer::new`, tens
            // of nanoseconds, so `constructorSeconds` stays the conservative
            // LOWER bound and the verdict stays on that side.
            let loop_entry_seconds = started.elapsed().as_secs_f64();
            wall.insert("loopEntrySeconds".to_owned(), json!(loop_entry_seconds));
            let search_started = Instant::now();
            let outcome = engine.run_cutclose(schedule, budget);
            let search_seconds = search_started.elapsed().as_secs_f64();
            wall.insert("searchSeconds".to_owned(), json!(search_seconds));
            wall.insert(
                "loopSearchSeconds".to_owned(),
                json!(outcome.search_seconds),
            );
            wall.insert(
                "loopExploreSeconds".to_owned(),
                json!(outcome.explore_seconds),
            );
            document["constructor"] = json!({
                "rawSourceDepthMm": constructor_depth,
                "placementFingerprint": constructor_fingerprint,
                "placementCount": placements.len(),
                "lowerScaleMm": lower_scale_mm,
            });
            // **The schedule block, and the two-arm gate's cell key.**
            //
            // The four `*IterationsWithoutImprovement` / `*Strikes` keys keep
            // their names and, in the control arm, their exact values, so
            // every committed document and every existing reduction reads
            // them as it always did. In the treatment arm the iteration
            // patience genuinely does not exist and the key is `null` rather
            // than the control's literal: a document that reported `200` on an
            // arm that never counts to 200 would be the one thing a paired
            // comparison cannot survive.
            let arm_rule = |phase: Phase| {
                let rule = schedule.strikes.rule(phase);
                let (kind, iterations, quantum) = match rule.patience {
                    Patience::Iterations(limit) => ("iterations", json!(limit), Value::Null),
                    Patience::Work(quantum) => ("work", Value::Null, json!(quantum)),
                };
                (kind, iterations, quantum, rule.strikes)
            };
            let (explore_kind, explore_iterations, explore_quantum, explore_strikes) =
                arm_rule(Phase::Explore);
            let (compress_kind, compress_iterations, compress_quantum, compress_strikes) =
                arm_rule(Phase::Compress);
            document["schedule"] = json!({
                "mode": mode,
                "workers": workers,
                "wallBudgetSeconds": match mode.as_str() {
                    "wall" | "calibrated" => json!(wall_budget_s),
                    _ => Value::Null,
                },
                "exploreTimeRatio": schedule.explore_time_ratio,
                "exploreIterationsWithoutImprovement": explore_iterations,
                "exploreStrikes": explore_strikes,
                "compressIterationsWithoutImprovement": compress_iterations,
                "compressStrikes": compress_strikes,
                "recordFingerprints": schedule.record_fingerprints,
                "fixedWork": budget_json,
                // ---------------------------------- the two-arm gate's key --
                "arm": arm,
                "armLabel": schedule.strikes.arm(),
                "strikePolicy": {
                    "explore": {
                        "patience": explore_kind,
                        "iterationsWithoutImprovement": explore_iterations,
                        "workQuantumSampleEvaluations": explore_quantum,
                        "strikes": explore_strikes,
                    },
                    "compress": {
                        "patience": compress_kind,
                        "iterationsWithoutImprovement": compress_iterations,
                        "workQuantumSampleEvaluations": compress_quantum,
                        "strikes": compress_strikes,
                    },
                    "improvingResetRatio": 0.98,
                    // The tripwire on all six frozen numbers, evaluated by the
                    // binary that ran rather than asserted by the reader.
                    "frozenLiteralsIntact": frozen_literals_intact(),
                },
                "calibratedPlan": plan_document,
                "calibratedAttemptsPerBite": calibrated_attempts_per_bite,
            });
            document["outcome"] = schedule_json(
                &outcome,
                &constructor_fingerprint,
                &LayoutContext {
                    sources: &sources,
                    pieces: &pieces,
                    contract: &contract,
                    revalidate: options.integer("revalidate", 0)? != 0,
                },
            );
            document["finalPoseDigest"] = json!(pose_digest(&outcome.final_poses));

            // **The calibration entry point.** A wall trajectory measures two
            // per-phase rates and writes them; a `--mode=calibrated` run in a
            // *different process* reads them and constructs no clock. The two
            // halves never meet: this branch cannot spend a plan and the
            // calibrated branch above cannot measure one.
            if let Some(path) = options.get("icscal") {
                let plan = calibration_plan(
                    &request_sha256,
                    &executable_sha256().unwrap_or_default(),
                    workers,
                    &outcome,
                    options.number("icscalsafety", 0.80)?,
                )?;
                let bytes = plan.to_bytes()?;
                fs::write(path, &bytes)?;
                document["icscal"] = json!({
                    "path": path,
                    "summary": plan.summary(),
                    "sha256": format!("{:x}", Sha256::digest(&bytes)),
                    "plan": serde_json::to_value(&plan)?,
                });
            }

            // The first-bite canary's own clause, computed by the driver rather
            // than by the python that reads it, so the binary and the tripwire
            // cannot disagree about what `0.999 * D*` is.
            //
            // FAIL HERE MEANS DO NOT RUN THE 9-SEED WALL: Grok review 12 Round 2
            // §6.3.4, "FAIL here is a member fail" - 0.183 mm is inside the S1
            // basin the member already republishes, so a first 0.1 % bite that
            // cannot publish is not a throughput story.
            let first_bite_target = homotopy::explore_width_mm(constructor_depth);
            let first = outcome
                .publications
                .iter()
                .find(|row| row.ordinal.bite == 1 && row.phase.label() == "explore");
            document["firstBiteCanary"] = json!({
                "constructorDepthMm": constructor_depth,
                "expectedTargetMm": first_bite_target,
                "published": first.is_some(),
                "publishedRawDepthMm": first.map(|row| row.published_raw_depth_mm),
                "targetDepthMm": first.map(|row| row.target_depth_mm),
                "targetMatchesExpected": first
                    .map(|row| row.target_depth_mm == first_bite_target)
                    .unwrap_or(false),
                "withinTarget": first
                    .map(|row| row.published_raw_depth_mm <= row.target_depth_mm)
                    .unwrap_or(false),
                "dualValid": outcome.trace.checkpoints.iter().all(|row|
                    row.published_raw_depth_mm.is_none()
                        || (row.kernel_exclusive_valid && row.contract_valid)),
                "strictChild": first
                    .map(|row| row.placement_fingerprint != constructor_fingerprint)
                    .unwrap_or(false),
            });
        }
        "randomt" => {
            // Diagnostic only, by both designers' arbitration: a uniform dense
            // throw changes initialization *and* separation, so a failure here
            // cannot tell a bad Φ from an erased coarse structure.
            let constructor_started = Instant::now();
            let placements = ShortSideFirst.layout(&pieces, settings)?;
            wall.insert(
                "constructorSeconds".to_owned(),
                json!(constructor_started.elapsed().as_secs_f64()),
            );
            let constructor_depth = raw_depth_of(&pieces, &placements, &contract);
            let constructor_fingerprint = placement_fingerprint(&placements);
            let target = options.number("target", constructor_depth)?;
            let poses = uniform_throw(&sources, &pieces, &contract, target, seed);
            let config = IcsConfig {
                target_depth_mm: target,
                proposal_budget: options.integer("budget", 100_000)?,
                relocate_eval_budget: options.integer("relocateevals", u64::MAX)?,
                checkpoint_every_sweeps: options.integer("checkpointevery", 1)?,
                descent: descent_config(&options, &contract, &sources, seed)?,
                limits: publication_limits(&options)?,
            };
            let incumbent = ExactIncumbent {
                placements: placements.clone(),
                raw_source_depth_mm: constructor_depth,
                from_constructor: true,
                placement_fingerprint: constructor_fingerprint.clone(),
            };
            let solver_started = Instant::now();
            let mut engine = Engine::from_poses(
                &pieces,
                settings,
                sources.clone(),
                contract,
                poses,
                incumbent,
                config,
            );
            let entry_totals = engine.totals();
            let outcome = engine.run();
            wall.insert(
                "solverSeconds".to_owned(),
                json!(solver_started.elapsed().as_secs_f64()),
            );
            document["constructor"] = json!({
                "rawSourceDepthMm": constructor_depth,
                "placementFingerprint": constructor_fingerprint,
            });
            document["entry"] = json!({
                "rawPhi": entry_totals.raw,
                "maxViolationMm": entry_totals.max_violation_mm,
                "lockedTargetMm": target,
            });
            document["outcome"] = outcome_json(&outcome, &constructor_fingerprint);
        }
        "corpus" => {
            let constructor_started = Instant::now();
            let placements = ShortSideFirst.layout(&pieces, settings)?;
            wall.insert(
                "constructorSeconds".to_owned(),
                json!(constructor_started.elapsed().as_secs_f64()),
            );
            let constructor_depth = raw_depth_of(&pieces, &placements, &contract);
            let parent = poses_of(&pieces, &sources, &placements)?;
            let states = options.integer("states", 1_000)?;
            let target = options.number("target", constructor_depth)?;
            let corpus_started = Instant::now();
            let (report, misses) = corpus::run(
                &pieces,
                &sources,
                settings,
                &contract,
                &parent,
                constructor_depth,
                lower_scale_mm,
                states,
                seed,
                target,
            );
            wall.insert(
                "corpusSeconds".to_owned(),
                json!(corpus_started.elapsed().as_secs_f64()),
            );
            // The fatal force-correlation clause is scored on the population
            // the spec defines for it: "states produced from three constructor
            // layouts by 1 %, 3 % and 10 %-residual affine compression plus
            // predeclared SE(2) perturbations" - the `compressed` family.
            //
            // The other two families are this round's additions and they are
            // reported, never folded into the fatal denominator:
            //
            // * `grazing` (0 % compression, micrometre perturbations) exists
            //   because the compression family never produces a Phi-feasible
            //   state, so the "no proxy-feasible state is exact-invalid outside
            //   the 4 um band" clause would pass vacuously without it. Its
            //   force misses are a quadratic-versus-linear artefact near
            //   convergence: Phi is a sum of squares and the independent score
            //   is a sum of violations, so trading one large residual for
            //   several small ones lowers the first and raises the second.
            // * `containment` is a synthetic construction the spec checks with
            //   a *different* clause ("no containment false-feasible case"),
            //   which it passes. Its force rate is low for a named reason: the
            //   minimum translation vector of a small piece deep inside a large
            //   one points the long way out and is not a descent direction for
            //   the deepest-interior-vertex measure.
            //
            // Both are stated in docs/experiments/overlap-ics/README.md with
            // their numbers, so folding them in is one division away for any
            // reader who disagrees with the split.
            let fatal_steps = report.force_steps_by_family[0];
            let active_rate = ratio(report.force_active_improved_by_family[0], fatal_steps);
            let total_rate = ratio(report.force_total_not_worse_by_family[0], fatal_steps);
            let all_active_rate = ratio(report.force_active_improved, report.force_steps);
            let all_total_rate = ratio(report.force_total_not_worse, report.force_steps);
            document["constructor"] = json!({"rawSourceDepthMm": constructor_depth});
            document["corpus"] = json!({
                "states": report.states,
                "lockedTargetMm": target,
                "proxyFeasible": report.proxy_feasible,
                "proxyFeasibleExactInvalid": report.proxy_feasible_exact_invalid,
                "outsideFourMicrometreBand": report.outside_band,
                "worstBandMicron": report.worst_band_micron,
                "containmentStates": report.containment_states,
                "containmentFalseFeasible": report.containment_false_feasible,
                "incrementalMismatches": report.incremental_mismatches,
                "kernelUnmeasurable": report.kernel_unmeasurable,
                "compressedStates": report.compressed_states,
                "grazingStates": report.grazing_states,
                "containmentFamilyStates": report.containment_family_states,
                "forceStepsByFamily": {
                    "compressed": report.force_steps_by_family[0],
                    "grazing": report.force_steps_by_family[1],
                    "containment": report.force_steps_by_family[2],
                },
                "forceActiveImprovedByFamily": {
                    "compressed": report.force_active_improved_by_family[0],
                    "grazing": report.force_active_improved_by_family[1],
                    "containment": report.force_active_improved_by_family[2],
                },
                "forceTotalNotWorseByFamily": {
                    "compressed": report.force_total_not_worse_by_family[0],
                    "grazing": report.force_total_not_worse_by_family[1],
                    "containment": report.force_total_not_worse_by_family[2],
                },
                "forceSteps": report.force_steps,
                "forceStepsScored": fatal_steps,
                "forceActiveImprovedRate": active_rate,
                "forceTotalNotWorseRate": total_rate,
                "forceActiveImprovedRateAllFamilies": all_active_rate,
                "forceTotalNotWorseRateAllFamilies": all_total_rate,
            });
            document["forceMisses"] = json!(misses.iter().map(|miss| json!({
                "ordinal": miss.ordinal,
                "family": miss.family.label(),
                "piece": miss.piece,
                "scaleMm": miss.scale_mm,
                "beforeActiveMm": miss.before_active_mm,
                "afterActiveMm": miss.after_active_mm,
                "beforeTotalMm": miss.before_total_mm,
                "afterTotalMm": miss.after_total_mm,
                "phiBefore": miss.phi_before,
                "phiAfter": miss.phi_after,
                "stepMm": miss.step_mm,
            })).collect::<Vec<_>>());
            document["verdict"] = json!({
                "outsideBandZero": report.outside_band == 0,
                "containmentNeverFalseFeasible": report.containment_false_feasible == 0,
                "incrementalEqualsCold": report.incremental_mismatches == 0,
                "forceScoredOn": "compressed",
                "forceActiveAtLeast95": active_rate >= 0.95,
                "forceTotalAtLeast80": total_rate >= 0.80,
                "forceActiveAtLeast95AllFamilies": all_active_rate >= 0.95,
                "forceTotalAtLeast80AllFamilies": all_total_rate >= 0.80,
                "proxyFeasiblePopulationNonEmpty": report.proxy_feasible > 0,
                "containmentPopulationNonEmpty": report.containment_states > 0,
                "pass": report.outside_band == 0
                    && report.proxy_feasible > 0
                    && report.containment_states > 0
                    && report.containment_false_feasible == 0
                    && report.incremental_mismatches == 0
                    && active_rate >= 0.95
                    && total_rate >= 0.80,
            });
        }
        // ------------------------------------------------ the profile census --
        //
        // **The spawn-tax cell.** docs/economics-round-spec.md funds a persistent
        // executor only behind a measured gate: "profile easy + bite-22 hard
        // states, workers 1/2/4/8, identical fixed work (prep, dispatch/join,
        // sweeps, merge+GLS, exact/repair separately). Build iff prep+dispatch
        // >= 10% of hard-state wall."
        //
        // The density is the whole point, and it is the spec's own pre-named
        // defect (3): "probe-on-cheap-bites (calibrating on bites 1-21
        // overstates iters/s ~1.5x; the probe is 400 iterations AT the 179
        // shelf)". So this cell does not run bites 1-21 and time them. It runs
        // the constructor, takes the 21 published 0.1 % bites that land the
        // trajectory on the 179 shelf, and then spends `--probeiters` master
        // iterations on the 22nd bite - the one that does not publish - with
        // the phase clock running. Both halves are reported: the cheap prefix
        // is the number the defect would have calibrated on, and printing it
        // beside the shelf is how a reader sees the 1.5x rather than being
        // told about it.
        //
        // The phase timers only exist under `--features ics-profile`. Without
        // it this cell still runs and still reports work and iterations, and
        // every duration is zero with `measured: false` beside it - which is a
        // refusal to answer, not an answer of zero.
        "spawntax" => {
            let constructor_started = Instant::now();
            let placements = ShortSideFirst.layout(&pieces, settings)?;
            let constructor_seconds = constructor_started.elapsed().as_secs_f64();
            wall.insert("constructorSeconds".to_owned(), json!(constructor_seconds));
            let constructor_depth = raw_depth_of(&pieces, &placements, &contract);
            let constructor_fingerprint = placement_fingerprint(&placements);
            // **Two quotas, and two worker counts, on purpose.**
            //
            // The PREFIX is the audit's committed fixed-work replay exactly -
            // `bites=21, attempts=1, iters=400` - and it always runs at the
            // frozen eight workers, whatever `--workers` says. That is what
            // makes the ladder a measurement: all four arms enter the shelf
            // from the *identical* state, the 8-worker parent whose depth is
            // 179.16566573285345 on seed 0, and only the probe's worker count
            // differs. A prefix run at the arm's own worker count would enter
            // four different states, and the 1/2/4/8 comparison would be a
            // comparison of four layouts rather than of the machinery.
            //
            // The PROBE is a second `run_cutclose` on the same engine: one
            // explore bite from the published shelf parent, `--probeiters`
            // master iterations, one attempt. It is a fresh 0.1 % cut from the
            // exact-valid incumbent, which is what a 22nd bite is.
            let workers = options.integer("workers", 8)? as usize;
            let prefix_workers = options.integer("prefixworkers", 8)? as usize;
            let shelf_bites = options.integer("shelfbites", 21)?;
            let prefix_iterations = options.integer("prefixiters", 400)?;
            let probe_iterations = options.integer("probeiters", 200)?;
            let record_fingerprints = options.integer("fingerprints", 0)? != 0;
            #[cfg(feature = "minimum-conflict-binary-close")]
            let prefix_binary_close_arm = binary_close_arm(&options, "prefixbinaryclose")?;
            #[cfg(feature = "minimum-conflict-binary-close")]
            let probe_binary_close_arm = binary_close_arm(&options, "binaryclose")?;
            let config = IcsConfig {
                target_depth_mm: constructor_depth,
                proposal_budget: 0,
                relocate_eval_budget: u64::MAX,
                checkpoint_every_sweeps: u64::MAX,
                descent: descent_config(&options, &contract, &sources, seed)?,
                limits: publication_limits(&options)?,
            };
            let mut engine = Engine::from_constructor_at_depth(
                &pieces,
                settings,
                &placements,
                constructor_depth,
                config,
            )?;
            let prefix_started = Instant::now();
            let prefix_outcome = engine.run_cutclose(
                ScheduleConfig {
                    workers: prefix_workers,
                    record_fingerprints,
                    #[cfg(feature = "minimum-conflict-binary-close")]
                    binary_close_arm: prefix_binary_close_arm,
                    ..ScheduleConfig::default()
                },
                Budget::FixedWork {
                    explore_bites: shelf_bites,
                    compress_bites: 0,
                    attempts_per_bite: 1,
                    iterations_per_separation: prefix_iterations,
                },
            );
            let prefix_seconds = prefix_started.elapsed().as_secs_f64();
            wall.insert("prefixSeconds".to_owned(), json!(prefix_seconds));
            let search_started = Instant::now();
            let outcome = engine.run_cutclose(
                ScheduleConfig {
                    workers,
                    record_fingerprints,
                    #[cfg(feature = "minimum-conflict-binary-close")]
                    binary_close_arm: probe_binary_close_arm,
                    ..ScheduleConfig::default()
                },
                Budget::FixedWork {
                    explore_bites: 1,
                    compress_bites: 0,
                    attempts_per_bite: 1,
                    iterations_per_separation: probe_iterations,
                },
            );
            let search_seconds = search_started.elapsed().as_secs_f64();
            wall.insert("searchSeconds".to_owned(), json!(search_seconds));

            let shelf_index = 0usize;
            let prefix: Vec<&_> = prefix_outcome.bites.iter().collect();
            let shelf = outcome.bites.first();
            let mut cheap = PhaseProfile::default();
            for row in &prefix {
                cheap.add(&row.profile);
            }
            let hard = shelf.map(|row| row.profile).unwrap_or_default();
            let published_prefix = prefix.iter().filter(|row| row.published.is_some()).count();
            document["constructor"] = json!({
                "rawSourceDepthMm": constructor_depth,
                "placementFingerprint": constructor_fingerprint,
                "placementCount": placements.len(),
                "lowerScaleMm": lower_scale_mm,
                "seconds": constructor_seconds,
            });
            document["spawnTax"] = json!({
                "workers": workers,
                "prefixWorkers": prefix_workers,
                "shelfBites": shelf_bites,
                "prefixIterations": prefix_iterations,
                "probeIterations": probe_iterations,
                "profileFeature": cfg!(feature = "ics-profile"),
                // The prefix has to have done what it was asked to do before
                // any duration below means anything: 21 bites, 21 publications,
                // and the shelf depth the committed replay records.
                "prefixBites": prefix.len(),
                "prefixPublications": published_prefix,
                "prefixAllPublished": published_prefix == prefix.len()
                    && prefix.len() == shelf_bites as usize,
                "prefixDepthMm": prefix_outcome.depth_mm,
                "prefixFingerprint": prefix_outcome.incumbent.placement_fingerprint,
                "shelfDepthMm": outcome.depth_mm,
                "shelfEntryWidthMm": shelf.map(|row| row.bite.width_after_mm),
                "shelfPublished": shelf.map(|row| row.published.is_some()),
                "shelfIterations": shelf.map(|row| row.master_iterations),
                "shelfStrikes": shelf.map(|row| row.strikes),
                "shelfBandEntries": shelf.map(|row| row.exact_band_entries),
                "shelfCheckpointCalls": shelf.map(|row| row.exact_checkpoint_calls),
                // The two arms of the census. `cheapPrefix` is bites 1-21 -
                // the window the pre-named probe defect would calibrate on -
                // and `hardState` is the 22nd bite alone.
                "cheapPrefix": phase_census_json(&cheap),
                "hardState": phase_census_json(&hard),
                "prefixPerBite": prefix_outcome.bites.iter().map(|row| json!({
                    "ordinal": row.ordinal,
                    "published": row.published.is_some(),
                    "masterIterations": row.master_iterations,
                    "widthAfterMm": row.bite.width_after_mm,
                    "census": phase_census_json(&row.profile),
                })).collect::<Vec<_>>(),
                // The work each arm really did, so a reader can normalise the
                // 1/2/4/8 ladder by work instead of by iteration count. At
                // eight workers one master iteration buys eight sweeps.
                // `work` is the engine's cumulative vector and therefore
                // includes the prefix; the per-window currency terms inside
                // `hardState` and `cheapPrefix` are the ones that do not.
                "work": work_json(&outcome.trace.work),
                "prefixWork": work_json(&prefix_outcome.trace.work),
            });
            document["outcome"] = schedule_json(
                &outcome,
                &constructor_fingerprint,
                &LayoutContext {
                    sources: &sources,
                    pieces: &pieces,
                    contract: &contract,
                    revalidate: options.integer("revalidate", 0)? != 0,
                },
            );
            #[cfg(feature = "minimum-conflict-binary-close")]
            if !prefix_outcome.binary_close.decisions.is_empty() {
                document["binaryClosePrefix"] = binary_close_json(&prefix_outcome.binary_close);
            }
            document["finalPoseDigest"] = json!(pose_digest(&outcome.final_poses));

            // **The icscal write path, exercised on a real measurement.**
            //
            // Schema and writer only, per the spec: no reader exists in this
            // round and no pacer consumes this file. The currency is
            // `U0-sample-evaluations`, which is what Wave 1 can honestly
            // measure - the spec's `U` needs B/E/R/D from microbenchmarks that
            // have not been run - and the rate is taken from the SHELF, never
            // from the cheap prefix.
            if let Some(path) = options.get("icscal") {
                let plan = shelf_work_plan(
                    &request_sha256,
                    &executable_sha256().unwrap_or_default(),
                    workers,
                    &outcome,
                    shelf_index,
                    shelf_bites + 1,
                    search_seconds,
                    options.number("icscalsafety", 0.80)?,
                    // Wave 1's plan, unchanged: the spawn-tax cell measures no
                    // compress phase and its file must stay the bytes the
                    // census committed.
                    None,
                )?;
                fs::write(path, plan.to_bytes()?)?;
                document["icscal"] = json!({
                    "path": path,
                    "summary": plan.summary(),
                    "sha256": format!("{:x}", Sha256::digest(&plan.to_bytes()?)),
                    "plan": serde_json::to_value(&plan)?,
                });
            }
        }
        "throughput" => {
            let constructor_started = Instant::now();
            let placements = ShortSideFirst.layout(&pieces, settings)?;
            wall.insert(
                "constructorSeconds".to_owned(),
                json!(constructor_started.elapsed().as_secs_f64()),
            );
            let constructor_depth = raw_depth_of(&pieces, &placements, &contract);
            let target = options.number("target", constructor_depth * 0.95)?;
            let config = IcsConfig {
                target_depth_mm: target,
                proposal_budget: 0,
                relocate_eval_budget: u64::MAX,
                checkpoint_every_sweeps: u64::MAX,
                descent: descent_config(&options, &contract, &sources, seed)?,
                limits: publication_limits(&options)?,
            };
            let mut engine =
                Engine::from_constructor(&pieces, settings, &placements, constructor_depth, config)?;
            let repeats = options.integer("repeats", 200)? as usize;
            document["throughput"] =
                throughput(&mut engine, repeats, options.integer("proposals", 2_000)?);
            document["throughput"]["lockedTargetMm"] = json!(target);
        }
        other => return Err(format!("unknown cell `{other}`").into()),
    }

    wall.insert(
        "totalSeconds".to_owned(),
        json!(started.elapsed().as_secs_f64()),
    );
    document["wall"] = Value::Object(wall);
    document["executableSha256"] = json!(executable_sha256());
    document["buildFeatures"] = json!(build_features());
    println!("{}", serde_json::to_string_pretty(&document)?);
    Ok(())
}

fn ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        1.0
    } else {
        numerator as f64 / denominator as f64
    }
}

/// The publication limits, with the attempt band overridable **for
/// diagnosis only**.
///
/// The shipped band is `EPSILON_GRID_MM` = 4 µm, and it is derived rather than
/// chosen: `2 * ceil(sqrt(2) * 1 µm)` is the most `GridSet::of` can move two
/// rings toward each other. `--band` exists so a failing cell can be asked
/// *which half* failed - the search, which could not get inside the band, or
/// the publication, which could not legalize once inside it. A widened band is
/// never a verdict; every gate in `cells.py` runs at the derived one.
fn publication_limits(options: &Options) -> Result<PublicationLimits, String> {
    let mut limits = PublicationLimits::default();
    limits.band_mm = options.number("band", limits.band_mm)?;
    Ok(limits)
}

fn descent_config(
    options: &Options,
    contract: &Contract,
    sources: &[PieceSource],
    seed: u64,
) -> Result<DescentConfig, String> {
    let mut config = DescentConfig::derive(contract, sources, seed);
    config.jump_allowance = options.integer("jumps", config.jump_allowance as u64)? as u32;
    config.rejection_census_samples =
        options.integer("rejectioncensus", config.rejection_census_samples as u64)? as usize;
    config.stalls_before_jump =
        options.integer("stalls", config.stalls_before_jump as u64)? as u32;
    // Absent means the *derived* default, which after the Gate-0 autopsy is the
    // spec's literal reading: the best candidate commits. `guided` stays
    // reachable so the A/B is one command, but it is no longer the default and
    // no longer silently overrides `DescentConfig::derive`.
    config.jump_commits_unconditionally = match options.get("jumpcommit") {
        None => config.jump_commits_unconditionally,
        Some("guided") => false,
        Some("always") => true,
        Some(other) => return Err(format!("--jumpcommit must be always|guided, not `{other}`")),
    };
    #[cfg(feature = "conflict-cluster-budget")]
    {
        config.partition_arm = match options.get("partition").unwrap_or("off") {
            "off" => PartitionArm::Off,
            "mass" => PartitionArm::Mass,
            "shuffled-mass" => PartitionArm::ShuffledMass,
            "max-violation" => PartitionArm::MaxViolation,
            "shadow" => PartitionArm::Shadow,
            "compute-ignore" => PartitionArm::ComputeIgnore,
            other => {
                return Err(format!(
                    "--partition must be off|mass|shuffled-mass|max-violation|shadow|compute-ignore, not `{other}`"
                ));
            }
        };
    }
    #[cfg(not(feature = "conflict-cluster-budget"))]
    if let Some(partition) = options.get("partition") {
        if partition != "off" {
            return Err("--partition requires --features conflict-cluster-budget".to_owned());
        }
    }
    Ok(config)
}

/// The committed perturbation: a counter-based SE(2) displacement keyed by
/// `(seed, piece index)` alone, so the vector is a function of the two numbers
/// in the evidence document and can be regenerated from them.
fn perturb(poses: &[Pose], seed: u64, magnitude_mm: f64, magnitude_deg: f64) -> Vec<Pose> {
    poses
        .iter()
        .enumerate()
        .map(|(index, pose)| {
            if magnitude_mm == 0.0 && magnitude_deg == 0.0 {
                return *pose;
            }
            let key = counter_hash(&[seed, index as u64, 0x5011]);
            Pose {
                tx_mm: pose.tx_mm + (unit(key) * 2.0 - 1.0) * magnitude_mm,
                ty_mm: pose.ty_mm + (unit(key >> 17) * 2.0 - 1.0) * magnitude_mm,
                theta_deg: pose.theta_deg + (unit(key >> 34) * 2.0 - 1.0) * magnitude_deg,
                mirrored: pose.mirrored,
            }
        })
        .collect()
}

fn uniform_throw(
    sources: &[PieceSource],
    pieces: &[GeneralFastPiece<'_>],
    contract: &Contract,
    target_mm: f64,
    seed: u64,
) -> Vec<Pose> {
    // The same L/R/B-physical, top-inset split Phi and the jump box use. The
    // circumradius convention is kept here on purpose: random-T is the uniform
    // *throw* diagnostic, its whole point is a dense scatter with no structure,
    // and it is not a cell any verdict rests on.
    let physical = contract.physical_edge_clearance_mm();
    let top = (target_mm - contract.depth_top_inset_mm())
        .min(contract.sheet_long_axis_mm - physical);
    sources
        .iter()
        .enumerate()
        .map(|(index, source)| {
            let key = counter_hash(&[seed, index as u64, 0x7470]);
            let radius = source.max_radius_mm;
            let low_x = physical + radius;
            let high_x = contract.sheet_short_axis_mm - physical - radius;
            let low_y = physical + radius;
            let high_y = top - radius;
            let theta = if pieces[index].allow_rotation {
                unit(key >> 34) * 360.0
            } else {
                0.0
            };
            let centre = [
                low_x + unit(key) * (high_x - low_x).max(0.0),
                low_y + unit(key >> 17) * (high_y - low_y).max(0.0),
            ];
            let (sin, cos) = theta.to_radians().sin_cos();
            let rotated = [
                source.centroid[0] * cos - source.centroid[1] * sin,
                source.centroid[0] * sin + source.centroid[1] * cos,
            ];
            Pose {
                tx_mm: centre[0] - rotated[0],
                ty_mm: centre[1] - rotated[1],
                theta_deg: theta,
                mirrored: false,
            }
        })
        .collect()
}

fn unit(key: u64) -> f64 {
    ((key >> 11) as f64) / ((1u64 << 53) as f64)
}

fn pose_digest(poses: &[Pose]) -> String {
    let mut digest = Sha256::new();
    for pose in poses {
        digest.update(pose.tx_mm.to_bits().to_le_bytes());
        digest.update(pose.ty_mm.to_bits().to_le_bytes());
        digest.update(pose.theta_deg.to_bits().to_le_bytes());
        digest.update([u8::from(pose.mirrored)]);
    }
    format!("{:x}", digest.finalize())
}

/// The four Round-0 performance kills, measured rather than projected where a
/// measurement is possible.
fn throughput(engine: &mut Engine<'_>, repeats: usize, proposals: u64) -> Value {
    // 1. Cold full Φ geometry.
    let started = Instant::now();
    for _ in 0..repeats {
        engine.cold_rebuild();
    }
    let cold_micros = started.elapsed().as_secs_f64() * 1e6 / repeats as f64;

    // 2. One moved-piece row reconstruction.
    let count = engine.state().poses.len();
    let started = Instant::now();
    for index in 0..(repeats * 10) {
        engine.rebuild_piece(index % count);
    }
    let row_micros = started.elapsed().as_secs_f64() * 1e6 / (repeats * 10) as f64;

    // 3. Convex cell gap evaluations per second, on the layout's own cells.
    let cells = engine.geometry().cells.len();
    let mut evaluations = 0u64;
    let started = Instant::now();
    for round in 0..repeats {
        for first in 0..cells {
            let second = (first + 1 + round) % cells;
            if first == second {
                continue;
            }
            let gap = convex_cell_gap(
                engine.geometry().cell_slice(first),
                engine.geometry().cell_slice(second),
            );
            std::hint::black_box(gap);
            evaluations += 1;
        }
    }
    let gap_seconds = started.elapsed().as_secs_f64();

    // 4. Complete piece proposals per second, after incremental rows.
    //
    // Reported with the raw Φ on both sides and the accepted-move count,
    // because a proposal on a piece with no incident energy returns before it
    // forms a gradient and would inflate this rate into a lie. A reader can
    // see from `rawPhiAfter > 0` and `acceptedMoves` that the loop was doing
    // the work the currency is denominated in.
    let phi_before = engine.cold_rebuild().raw;
    let before = engine.work();
    let started = Instant::now();
    let mut done = 0u64;
    while done < proposals {
        engine.propose_once((done as usize) % count);
        done += 1;
    }
    let proposal_seconds = started.elapsed().as_secs_f64();
    let proposals_per_second = done as f64 / proposal_seconds;
    let phi_after = engine.cold_rebuild().raw;
    let after = engine.work();
    let accepted = after.accepted_moves - before.accepted_moves;

    // ------------------------------------- the relocate metric version (§4) --
    //
    // **Arbitration 4, both halves.** The three committed thresholds above stay
    // literal and keep their original meaning, and the fourth pin is
    // *re-denominated* rather than renamed: the retired pin counted a proposal
    // that formed a gradient and walked a backtracking ladder; one `propose_once`
    // is now a whole relocate - 75 pool samples plus four coordinate descents -
    // and a rate in that unit is not the same number and never was.
    //
    // Both are printed. `pieceProposalsPerSecond` is the old counter under the
    // NEW operator and is *expected* to be far below the retired 100 K/8 s pin,
    // because one unit now buys ~76x the work; `relocateEvalsPerSecond` is the
    // member's own currency and is what §4.3's ">= 100 K relocate-evals projected
    // in 8 s" clause is scored on. `rawPhiBefore/After` and `acceptedRelocates`
    // sit beside it so a skip-loop cannot fake the rate - a relocate on a piece
    // with no incident energy returns before it samples anything, and the
    // `relocates` counter (not `pieceProposals`) is what excludes it.
    let sample_evaluations = after.sample_evaluations - before.sample_evaluations;
    let relocates = after.relocates - before.relocates;
    let relocate_evals_per_second = sample_evaluations as f64 / proposal_seconds;
    let projected_relocate_evals = relocate_evals_per_second * 8.0;

    json!({
        "coldPhiMicroseconds": cold_micros,
        "coldPhiUnder200us": cold_micros <= 200.0,
        "movedPieceRowRebuildMicroseconds": row_micros,
        "rowRebuildUnder20us": row_micros <= 20.0,
        "convexCellGapEvaluations": evaluations,
        "convexCellGapEvaluationsPerSecond": evaluations as f64 / gap_seconds,
        "cellGapAtLeast1MPerSecond": (evaluations as f64 / gap_seconds) >= 1.0e6,
        "pieceProposals": done,
        "pieceProposalsPerSecond": proposals_per_second,
        "rawPhiBeforeProposals": phi_before,
        "rawPhiAfterProposals": phi_after,
        "acceptedMovesDuringProposals": accepted,
        "projectedProposalsInEightSeconds": proposals_per_second * 8.0,
        // Kept under its ORIGINAL name and meaning so the previous rounds'
        // documents stay diffable, and explicitly NOT a clause of `pass` any
        // more - see `retiredProposalPinNote`.
        "projectedAtLeast100K": proposals_per_second * 8.0 >= 100_000.0,
        "retiredProposalPinNote":
            "pieceProposals now buys one whole relocate (75 pool samples + 4 \
             coordinate descents), not one gradient + ladder. The 100K/8s pin is \
             re-denominated into relocateEvals per docs/cutclose-relocate-spec.md \
             arbitration 4; this field is reported under its old meaning and is \
             not a pass clause.",
        "relocates": relocates,
        "acceptedRelocates": accepted,
        "sampleEvaluations": sample_evaluations,
        "sampleEvaluationsPerRelocate": if relocates == 0 {
            0.0
        } else {
            sample_evaluations as f64 / relocates as f64
        },
        "relocatesPerSecond": relocates as f64 / proposal_seconds,
        "relocateEvalsPerSecond": relocate_evals_per_second,
        "projectedRelocateEvalsInEightSeconds": projected_relocate_evals,
        "relocateEvalsAtLeast100K": projected_relocate_evals >= 100_000.0,
        "containerWinners": after.container_winners - before.container_winners,
        "focusedWinners": after.focused_winners - before.focused_winners,
        "stayPutWinners": after.stay_put_winners - before.stay_put_winners,
        "containerCommits": after.container_commits - before.container_commits,
        "pass": cold_micros <= 200.0
            && row_micros <= 20.0
            && (evaluations as f64 / gap_seconds) >= 1.0e6
            && projected_relocate_evals >= 100_000.0,
    })
}
