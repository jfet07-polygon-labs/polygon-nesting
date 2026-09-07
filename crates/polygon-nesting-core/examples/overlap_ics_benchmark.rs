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
//! looking at it. The one other door is `--start` on the `cutclose` cell
//! (`StartLayout` below): a diagnostic-only start from a caller-named layout
//! that stamps the document with a `startedFrom` tripwire so it can never be
//! scored. `--bitemicroscope=1` (`overlap_ics::microscope`) is a diagnostic
//! of the same kind in the other direction - it *emits* replay capsules, which
//! are known-good layouts - and stamps `biteMicroscope.tripwire` for the same
//! reason.

#![recursion_limit = "256"]

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
#[cfg(feature = "pool-retry-tracker-rebase")]
use std::fs::OpenOptions;
#[cfg(feature = "pool-retry-tracker-rebase")]
use std::io::Write;
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
#[cfg(feature = "pool-retry-tracker-rebase")]
use polygon_nesting_core::search::overlap_ics::pool_rebase::{
    apply_weight_policy, raw_row_digest as pool_raw_row_digest, PoolRebaseArm, PoolRebaseTrace,
    WeightSnapshot, FIRST_RETRY_ITERATION_CAP,
};
#[cfg(feature = "pool-retry-tracker-rebase")]
use polygon_nesting_core::search::overlap_ics::pool_rebase_checkpoint::{
    envelope_body as pool_checkpoint_body, envelope_bytes as pool_checkpoint_envelope,
    immutable_input_sha256 as pool_immutable_input_sha256, CheckpointBindings,
};
use polygon_nesting_core::search::overlap_ics::profile::PhaseProfile;
use polygon_nesting_core::search::overlap_ics::publish;
use polygon_nesting_core::search::overlap_ics::publish::{
    placement_fingerprint, raw_depth_of, PublicationLimits,
};
use polygon_nesting_core::search::overlap_ics::replay::{
    ReplayParams, ReplayProbe, TracedRelocate, TracedSweep,
};
use polygon_nesting_core::search::overlap_ics::state::{
    piece_sources, Contract, ExactIncumbent, PieceSource, Pose,
};
use polygon_nesting_core::search::overlap_ics::{
    poses_of, Budget, CalibratedSummary, Engine, IcsConfig, IcsOutcome, InitialLayoutProvider,
    Phase, ScheduleConfig, ScheduleOutcome,
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

// ------------------------------------------------------- the start layout ---

/// `--start=<placements.json>`: begin the `cutclose` trajectory from a given
/// contract-valid layout instead of the constructor's. **Diagnostic only.**
///
/// WHY. `docs/experiments/overlap-ics/sparrow-warm-start/README.md` warm-starts
/// Sparrow from our constructor's own layout and watches it reach 149.195 mm
/// in 8 s, where our engine reaches ~165 mm from the identical layout; and
/// from the layouts our engine leaves behind at 164-166 mm even Sparrow gets
/// no deeper than 152.7-157.3. The mirror experiment - our engine started from
/// Sparrow's layouts, converted by that directory's
/// `tools/from-sparrow-solution.py` - needs this driver to start a cutclose
/// trajectory from a layout it did not construct. That is the whole purpose
/// of the flag: to ask whether our separator, given Sparrow's basin, holds it,
/// loses it, or deepens it.
///
/// WHAT. The file is JSON with a top-level `placements` array in exactly the
/// shape [`placements_json`] writes (`pieceId`, `rotationDeg`, `mirrored`,
/// `translateShortAxis`, `translateLongAxis`; extra keys such as `itemId`,
/// `source` or `stripWidth` are ignored). The constructor still runs exactly
/// as today, so `constructor` accounting and the wall clocks stay comparable;
/// its layout is then **replaced** by the loaded one before the ICS state is
/// built, and the initial target depth, the incumbent and its fingerprint are
/// derived from the loaded placements by the very same calls that derive them
/// from the constructor's. The loaded layout must place exactly the request's
/// piece ids, each once, and must pass the untouched contract validator
/// (`validate_placements_against_contract`, reached through
/// `publish::independently_revalidate`) with the cell's own contract;
/// otherwise the run is a hard error naming the first failure. There is no
/// fallback to the constructor's layout.
///
/// FORBIDDEN AS A RESULT. `docs/grok-review-12-reading-sparrow.md` §5.2, row
/// "fixture as a seed", forbids starting a scored cell from a known-good
/// layout, and this flag is exactly that door. So it is never a default, it
/// is refused on every cell but `cutclose`, and whenever it is on the
/// document carries a loud tripwire: top-level
/// `startedFrom: {path, sha256, rawSourceDepthMm, placementFingerprint, ...}`
/// and `constructor.startedFrom = true`. With the flag absent neither key
/// exists and the document is byte-identical to today's.
/// `/var/lib/t3/tmp/astra/score.py` must refuse any document carrying
/// `startedFrom` (that change is out of this instrument's scope; the key is
/// what makes it possible).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartLayout {
    placements: Vec<StartPlacement>,
}

/// One start placement. Every field is required - unlike `FixturePose`, whose
/// `mirrored` defaults - because a start layout is a claim about a full pose
/// and a silently defaulted mirror bit would be a different layout.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartPlacement {
    piece_id: String,
    rotation_deg: f64,
    mirrored: bool,
    translate_short_axis: f64,
    translate_long_axis: f64,
}

/// Decodes a `--start` document into placements for exactly `expected_ids`,
/// each once. Geometry is not checked here; that is the contract validator's
/// job at the call site, and it runs on every load.
fn start_layout_placements(
    bytes: &[u8],
    expected_ids: &[&str],
) -> Result<Vec<GeneralFastPlacement>, String> {
    let layout: StartLayout =
        serde_json::from_slice(bytes).map_err(|error| format!("--start: {error}"))?;
    let expected = expected_ids.iter().copied().collect::<BTreeSet<&str>>();
    let mut seen = BTreeSet::new();
    for placement in &layout.placements {
        if !expected.contains(placement.piece_id.as_str()) {
            return Err(format!(
                "--start: placement for `{}` names a piece that is not in this request",
                placement.piece_id
            ));
        }
        if !seen.insert(placement.piece_id.as_str()) {
            return Err(format!(
                "--start: piece `{}` is placed twice",
                placement.piece_id
            ));
        }
    }
    if let Some(missing) = expected_ids.iter().find(|id| !seen.contains(*id)) {
        return Err(format!("--start: piece `{missing}` has no placement"));
    }
    Ok(layout
        .placements
        .into_iter()
        .map(|placement| GeneralFastPlacement {
            piece_id: placement.piece_id,
            rotation_deg: placement.rotation_deg,
            mirrored: placement.mirrored,
            translate_short_axis: placement.translate_short_axis,
            translate_long_axis: placement.translate_long_axis,
        })
        .collect())
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

    /// `--guidedexponent=<p>`: the exponent the guided objective ranks on
    /// (`sum w v^p`; `overlap_ics::set_guided_exponent`). Absent means the
    /// frozen engine's `2`. A value that is not a number, or outside
    /// `0 < p <= 2`, is refused here with a clear reason, so a typo never
    /// runs a cell silently at the default.
    fn guided_exponent(&self) -> Result<f64, String> {
        match self.get("guidedexponent") {
            Some(value) => {
                let exponent: f64 = value.trim().parse().map_err(|error| {
                    format!("--guidedexponent: `{value}` is not a number ({error})")
                })?;
                if !exponent.is_finite() || exponent <= 0.0 || exponent > 2.0 {
                    return Err(format!(
                        "--guidedexponent: `{value}` must satisfy 0 < p <= 2 (2 is the frozen engine)"
                    ));
                }
                Ok(exponent)
            }
            None => Ok(polygon_nesting_core::search::overlap_ics::DEFAULT_GUIDED_EXPONENT),
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

#[cfg(feature = "pool-retry-tracker-rebase")]
fn pool_rebase_arm(options: &Options) -> Result<PoolRebaseArm, String> {
    match options.get("poolrebase").unwrap_or("saved") {
        "saved" => Ok(PoolRebaseArm::Saved),
        "rebase" => Ok(PoolRebaseArm::Rebase),
        "compute-ignore" => Ok(PoolRebaseArm::ComputeIgnore),
        other => Err(format!(
            "--poolrebase must be saved|rebase|compute-ignore, not `{other}`"
        )),
    }
}

#[cfg(feature = "pool-retry-tracker-rebase")]
fn pool_checkpoint_bindings(
    options: &Options,
    request_sha256: &str,
    pieces: &[GeneralFastPiece<'_>],
    sources: &[PieceSource],
    settings: GeneralFastSettings,
    contract: Contract,
    plan_bytes: &[u8],
) -> Result<CheckpointBindings, String> {
    Ok(CheckpointBindings {
        spec_sha256: options.required("specsha")?.to_owned(),
        request_sha256: request_sha256.to_owned(),
        plan_sha256: format!("{:x}", Sha256::digest(plan_bytes)),
        executable_sha256: executable_sha256()
            .ok_or_else(|| "cannot hash the running executable".to_owned())?,
        source_commit: options.required("sourcecommit")?.to_owned(),
        features: build_features(),
        immutable_input_sha256: pool_immutable_input_sha256(pieces, sources, settings, contract),
    })
}

#[cfg(feature = "pool-retry-tracker-rebase")]
fn weight_snapshot_json(snapshot: &WeightSnapshot) -> Value {
    json!({
        "bits": snapshot.bits,
        "digestSha256": hex_bytes(&snapshot.digest_sha256),
        "count": snapshot.bits.len(),
        "countAboveFloor": snapshot.count_above_floor,
        "minimum": if snapshot.minimum.is_finite() { json!(snapshot.minimum) } else { Value::Null },
        "maximum": if snapshot.maximum.is_finite() { json!(snapshot.maximum) } else { Value::Null },
        "allFinite": snapshot.all_finite,
        "allExactlyOne": snapshot.all_exactly_one,
    })
}

#[cfg(feature = "pool-retry-tracker-rebase")]
fn retry_pacer_json(summary: &CalibratedSummary) -> Value {
    json!({
        "exploreAllocationUnits": summary.explore_allocation,
        "compressAllocationUnits": summary.compress_allocation,
        "exploreConsumedUnits": summary.explore_consumed,
        "compressConsumedUnits": summary.compress_consumed,
        "exploreBatches": summary.explore_batches,
        "compressBatches": summary.compress_batches,
        "exploreCrossingBatchUnits": summary.explore_crossing_batch_units,
        "compressCrossingBatchUnits": summary.compress_crossing_batch_units,
        "charged": work_terms_json(&summary.charged),
        "unchargedTail": work_terms_json(&summary.uncharged_tail),
        "trajectory": work_terms_json(&summary.trajectory),
        "chargeIdentityHolds": summary.charge_identity_holds,
        "consumedUnits": summary.consumed_units,
        "consumedUnitsMatchCharged": summary.consumed_units_match_charged,
        "currencyVersion": summary.currency_version.as_str(),
        "budgetSeconds": summary.budget_seconds,
        "exploreRatio": summary.explore_ratio,
        "planKey": serde_json::to_value(&summary.plan_key).unwrap_or(Value::Null),
    })
}

#[cfg(feature = "pool-retry-tracker-rebase")]
fn retry_exact_checkpoint_json(
    row: &polygon_nesting_core::search::overlap_ics::diagnostics::ExactCheckpoint,
) -> Value {
    json!({
        "proposalOrdinal": row.proposal_ordinal,
        "targetDepthMm": row.target_depth_mm,
        "targetDepthBits": row.target_depth_mm.to_bits(),
        "maxViolationMm": row.max_violation_mm,
        "proxyRawDepthMm": row.proxy_raw_depth_mm,
        "firstScanFailingPairs": row.first_scan_failing_pairs,
        "firstScanFailingBoundaries": row.first_scan_failing_boundaries,
        "blockedOn": row.blocked_on,
        "blockingShortfallUm": row.blocking_shortfall_um,
        "firstPair": row.first_pair,
        "firstPairKernelShortfallUm": row.first_pair_kernel_shortfall_um,
        "firstPairProxyViolationUm": row.first_pair_proxy_violation_um,
        "kernelExclusiveValid": row.kernel_exclusive_valid,
        "contractValid": row.contract_valid,
        "repairRows": row.repair_rows,
        "repairMaxDisplacementMm": row.repair_max_displacement_mm,
        "repairDepthGivebackMm": row.repair_depth_giveback_mm,
        "publishedRawDepthMm": row.published_raw_depth_mm,
        "refusal": row.refusal,
    })
}

#[cfg(feature = "pool-retry-tracker-rebase")]
fn pool_rebase_json(
    trace: &PoolRebaseTrace,
    fingerprints: &[polygon_nesting_core::search::overlap_ics::IterationFingerprint],
) -> Value {
    json!({
        "arm": trace.arm.as_str(),
        "invalidRetries": trace.invalid_retries,
        "decisions": trace.decisions.iter().map(|row| json!({
            "key": {
                "requestSeed": row.request_seed,
                "exploreBiteOrdinal": row.explore_bite_ordinal,
                "attemptOrdinal": row.attempt_ordinal,
            },
            "widthMm": row.width_mm,
            "widthBits": row.width_mm.to_bits(),
            "poolLength": row.pool_length,
            "selectedRank": row.selected_rank,
            "poolEntryRawPhi": row.pool_entry_raw_phi,
            "poolEntryRawPhiBits": row.pool_entry_raw_phi.to_bits(),
            "selectedPoseDigestSha256": hex_bytes(&row.selected_pose_digest_sha256),
            "savedWeights": weight_snapshot_json(&row.saved_weights),
            "postInstallPoseDigestSha256": hex_bytes(&row.post_install_pose_digest_sha256),
            "postInstallRawRowDigestSha256": hex_bytes(&row.post_install_raw_row_digest_sha256),
            "resetWeights": row.reset_weights.as_ref().map(weight_snapshot_json),
            "postPolicyWeights": weight_snapshot_json(&row.post_policy_weights),
            "disruption": {
                "fired": row.disruption.fired,
                "swapped": row.disruption.swapped,
                "distinct": row.disruption.distinct,
                "followers": row.disruption.followers,
                "followersCapped": row.disruption.followers_capped,
                "work": work_json(&row.disruption_work_delta),
                "poseTransformDigestSha256":
                    hex_bytes(&row.disruption_pose_transform_digest_sha256),
            },
            "postDisruptionPoseDigestSha256": hex_bytes(&row.post_disruption_pose_digest_sha256),
            "postDisruptionRawRowDigestSha256": hex_bytes(&row.post_disruption_raw_row_digest_sha256),
            "coldPostDisruptionRawRowDigestSha256":
                hex_bytes(&row.cold_post_disruption_raw_row_digest_sha256),
            "incrementalColdRawRowsIdentical": row.post_disruption_raw_row_digest_sha256
                == row.cold_post_disruption_raw_row_digest_sha256,
            "postDisruptionWeights": weight_snapshot_json(&row.post_disruption_weights),
            "fingerprintStart": row.fingerprint_start,
            "fingerprintEnd": row.fingerprint_end,
            "downstreamFingerprints": fingerprints[row.fingerprint_start..row.fingerprint_end]
                .iter().map(|fingerprint| json!({
                    "bite": fingerprint.bite,
                    "attempt": fingerprint.attempt,
                    "iteration": fingerprint.iteration,
                    "winner": fingerprint.winner,
                    "winnerGuided": fingerprint.winner_guided,
                    "contested": fingerprint.contested,
                    "state": fingerprint.state,
                    "committedPoseDigestSha256": fingerprint
                        .committed_pose_digest_sha256
                        .as_ref()
                        .map(|digest| hex_bytes(digest)),
                })).collect::<Vec<_>>(),
            "retryIterations": row.retry_iterations,
            "retryStop": row.retry_stop,
            "retryPublished": row.retry_published,
            "retryWorkBefore": work_json(&row.retry_work_before),
            "retryWorkAfter": work_json(&row.retry_work_after),
            "pathWork": work_json(&row.path_work_delta),
            "retryStrike": {
                "strikes": row.retry_strikes,
                "batches": row.retry_strike_shadow.batches,
                "chargedWorkSampleEvaluations": row.retry_strike_shadow.charged_work,
                "substantial": row.retry_strike_shadow.substantial,
                "marginal": row.retry_strike_shadow.marginal,
                "none": row.retry_strike_shadow.none,
                "strikeAccumulated": row.retry_strike_accumulated,
                "strikeOvershoot": row.retry_strike_overshoot,
            },
            "retryBand": {
                "minimumRawPhi": if row.retry_min_raw_phi.is_finite() {
                    json!(row.retry_min_raw_phi)
                } else {
                    Value::Null
                },
                "reached": row.retry_band_reached,
                "entries": row.retry_exact_band_entries,
                "exactCheckpointCalls": row.retry_exact_checkpoint_calls,
            },
            "retryExactCheckpoints": row.retry_exact_checkpoints
                .iter().map(retry_exact_checkpoint_json).collect::<Vec<_>>(),
            "retryPublication": row.retry_publication.as_ref().map(|publication| json!({
                "ordinal": {
                    "bite": publication.ordinal.bite,
                    "attempt": publication.ordinal.attempt,
                    "iteration": publication.ordinal.iteration,
                    "proposals": publication.ordinal.proposals,
                },
                "phase": publication.phase.label(),
                "targetDepthMm": publication.target_depth_mm,
                "publishedRawDepthMm": publication.published_raw_depth_mm,
                "repairRows": publication.repair_rows,
                "repairMaxDisplacementMm": publication.repair_max_displacement_mm,
                "repairDepthGivebackMm": publication.repair_depth_giveback_mm,
                "parentFingerprint": publication.parent_fingerprint,
                "placementFingerprint": publication.placement_fingerprint,
                "improvedIncumbent": publication.improved_incumbent,
                "poses": poses_json(&publication.poses),
            })),
            "authorityParentFingerprint": row.authority_parent_fingerprint,
            "pacerBefore": row.pacer_before.as_ref().map(retry_pacer_json),
            "pacerAfter": row.pacer_after.as_ref().map(retry_pacer_json),
            "pathSeconds": row.path_seconds,
            "failureReasons": row.failure_reasons,
            "valid": row.valid,
        })).collect::<Vec<_>>(),
    })
}

#[cfg(feature = "minimum-conflict-binary-close")]
fn binary_close_json(trace: &BinaryCloseTrace) -> Value {
    json!({
        "arm": trace.arm.as_str(),
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
                "rowBits": term.row_bits.map(|states| states.map(|row| json!({
                    "violation": row.violation_mm,
                    "weight": row.weight,
                    "signedGap": row.signed_gap_mm,
                    "normal": row.normal,
                    "witnessA": row.witness_a,
                    "witnessB": row.witness_b,
                }))),
                "finiteNonnegative": term.finite_nonnegative,
                "zeroDiagonal": term.zero_diagonal,
                "submodular": term.submodular,
            })).collect::<Vec<_>>(),
            "unaries": decision.unary_terms.iter().map(|term| json!({
                "piece": term.piece,
                "violationBits": term.violations_mm.map(|row| row.map(f64::to_bits)),
                "rowCostBits": term.row_costs.map(|row| row.map(f64::to_bits)),
                "rowBits": term.row_bits.map(|states| states.map(|row| json!({
                    "violation": row.violation_mm,
                    "weight": row.weight,
                    "witness": row.witness,
                }))),
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
            "selectedTotalsFiniteNonnegative": decision.selected_totals_finite_nonnegative,
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
        "rejectsAggregateOverflow": report.rejects_aggregate_overflow,
        "rejectsNonnegativeDelta": report.rejects_nonnegative_delta,
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
    settings: GeneralFastSettings,
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
    if cfg!(feature = "pool-retry-tracker-rebase") {
        features.push("pool-retry-tracker-rebase".to_owned());
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
        "firstScanFailingPairs": row.first_scan_failing_pairs,
        "firstScanFailingBoundaries": row.first_scan_failing_boundaries,
        "blockedOn": row.blocked_on,
        "blockingShortfallUm": row.blocking_shortfall_um,
        "firstPair": row.first_pair,
        "firstPairKernelShortfallUm": row.first_pair_kernel_shortfall_um,
        "firstPairProxyViolationUm": row.first_pair_proxy_violation_um,
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
    let independent_revalidations = layouts.revalidate.then(|| {
        outcome
            .publications
            .iter()
            .map(|row| {
                let placements = publish::placements_of(layouts.sources, &row.poses);
                publish::independently_revalidate(
                    layouts.pieces,
                    &placements,
                    layouts.settings,
                    layouts.contract,
                )
            })
            .collect::<Vec<_>>()
    });
    let independent_invalid_publications = independent_revalidations.as_ref().map(|rows| {
        rows.iter()
            .filter(|row| !(row.kernel_exclusive_valid && row.contract_valid))
            .count()
    });
    let publications = outcome
        .publications
        .iter()
        .enumerate()
        .map(|(index, row)| {
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
                let independent = &independent_revalidations
                    .as_ref()
                    .expect("revalidation rows exist when requested")[index];
                value["revalidation"] = json!({
                    "recomputedPlacementFingerprint": placement_fingerprint(&placements),
                    "fingerprintMatches":
                        placement_fingerprint(&placements) == row.placement_fingerprint,
                    "recomputedRawDepthMm": depth,
                    "depthMatchesBitwise": depth.to_bits()
                        == row.published_raw_depth_mm.to_bits(),
                    "exclusiveKernel": {
                        "mode": independent.kernel_mode,
                        "radiusMm": independent.radius_mm,
                        "twoRMicron": independent.two_r_micron,
                        "searchOffsetAllowanceMm": independent.search_offset_allowance_mm,
                        "valid": independent.kernel_exclusive_valid,
                        "error": independent.kernel_error,
                    },
                    "contract": {
                        "valid": independent.contract_valid,
                        "error": independent.contract_error,
                    },
                    "allAuthoritiesValid": independent.kernel_exclusive_valid
                        && independent.contract_valid,
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
    let band_reached = outcome
        .bites
        .iter()
        .filter(|row| row.proxy_band_reached)
        .count();
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
        "firstScanFailingPairs": row.first_scan_failing_pairs,
        "firstScanFailingBoundaries": row.first_scan_failing_boundaries,
        "blockedOn": row.blocked_on,
        "blockingShortfallUm": row.blocking_shortfall_um,
        "firstPair": row.first_pair,
        "firstPairKernelShortfallUm": row.first_pair_kernel_shortfall_um,
        "firstPairProxyViolationUm": row.first_pair_proxy_violation_um,
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
    if let Some(count) = independent_invalid_publications {
        document["independentInvalidPublications"] = json!(count);
    }
    #[cfg(feature = "conflict-cluster-budget")]
    if outcome.trace.partition.partition_decisions > 0 {
        document["partition"] = partition_json(&outcome.trace.partition);
    }
    #[cfg(feature = "minimum-conflict-binary-close")]
    if let Some(digest) = &outcome.consumed_order_digest_sha256 {
        document["consumedWorkerOrders"] = json!({
            "digestSha256": digest,
            "sweeps": outcome.consumed_order_sweeps,
            "slots": outcome.consumed_order_slots,
            "completeStateDigestSha256": outcome.complete_state_digest_sha256,
        });
    }
    #[cfg(feature = "minimum-conflict-binary-close")]
    if !outcome.binary_close.decisions.is_empty() {
        document["binaryClose"] = binary_close_json(&outcome.binary_close);
    }
    #[cfg(feature = "pool-retry-tracker-rebase")]
    if !outcome.pool_rebase.decisions.is_empty() {
        document["poolRetryRebase"] = pool_rebase_json(&outcome.pool_rebase, &outcome.fingerprints);
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
    // `--start` is a cutclose-only diagnostic (see `StartLayout`); on any
    // other cell it would be silently ignored, which is worse than refused.
    if options.get("start").is_some() && cell != "cutclose" {
        return Err(format!("--start is a cutclose-only diagnostic, not a `{cell}` option").into());
    }
    // `--bitemicroscope=1` is the other cutclose-only diagnostic
    // (`overlap_ics::microscope`): a buffered per-relocate trace of the first
    // hard explore bite and the three after it, with replay capsules. Off by
    // default and byte-identical off; on, the document carries the top-level
    // `biteMicroscope` block whose `tripwire` a scorer must refuse - a replay
    // capsule is a known-good layout, and the forbidden-rescue table
    // (`docs/grok-review-12-reading-sparrow.md` §5.2, "fixture as a seed")
    // forbids starting a scored cell from one.
    if options.integer("bitemicroscope", 0)? != 0 && cell != "cutclose" {
        return Err(
            format!("--bitemicroscope is a cutclose-only diagnostic, not a `{cell}` option").into(),
        );
    }
    // `--microscopetarget=<mm>[,<mm>]` is the same microscope with the depth
    // trigger of GPT-6 Astra review 7 Q18 (`overlap_ics::microscope`, "the
    // depth trigger"): the first explore cut targeting at most the first
    // depth, followed to publication or its live stop, and if it publishes
    // the next cut targeting at most the second. Cutclose only, never
    // beside `--bitemicroscope`, and the same tripwire.
    if options.get("microscopetarget").is_some() && cell != "cutclose" {
        return Err(
            format!("--microscopetarget is a cutclose-only diagnostic, not a `{cell}` option").into(),
        );
    }
    let microscope_config = polygon_nesting_core::search::overlap_ics::microscope::resolve_flags(
        options.integer("bitemicroscope", 0)? != 0,
        options.get("microscopetarget"),
    )?;
    // `--capsule`, `--bite`, `--probe`, `--capsuleindex`, `--maxiters` and
    // `--horizon` belong to the `replay` cell alone (`overlap_ics::replay`):
    // a replay starts from a microscope capsule, which is a known-good
    // layout, and the forbidden-rescue table forbids that on any scored cell.
    for key in ["capsule", "bite", "probe", "capsuleindex", "maxiters", "fork", "certify", "horizon"] {
        if options.get(key).is_some() && cell != "replay" {
            return Err(format!("--{key} is a replay-only option, not a `{cell}` option").into());
        }
    }
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
    // The `--start` tripwire, filled by the cutclose arm exactly when the flag
    // is on and emitted at the tail. `None` means the constructor's own layout.
    let mut started_from: Option<Value> = None;
    // The `--bitemicroscope` report, filled by the cutclose arm exactly when
    // the flag is on and emitted at the tail with its tripwire. `None` means
    // the frozen document.
    let mut bite_microscope: Option<Value> = None;
    // The `--cell=replay` report, filled by the replay arm only and emitted
    // at the tail with its tripwire (`overlap_ics::replay`). `None` on every
    // other cell.
    let mut replay_document: Option<Value> = None;

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
        "pool-rebase-vectors" => {
            #[cfg(feature = "pool-retry-tracker-rebase")]
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
                let mut engine = Engine::from_constructor_at_depth(
                    &pieces,
                    settings,
                    &placements,
                    constructor_depth,
                    config,
                )?;
                if let Some(row) = engine.state.pair_rows.first_mut() {
                    row.weight = 2.0;
                }
                if let Some(row) = engine.state.pair_rows.last_mut() {
                    row.weight = 17.0;
                }
                if let Some(rows) = engine.state.edge_rows.first_mut() {
                    rows[3].weight = 1_024.0;
                }
                let saved_pair = engine
                    .state
                    .pair_rows
                    .iter()
                    .map(|row| row.weight)
                    .collect::<Vec<_>>();
                let saved_edges = engine
                    .state
                    .edge_rows
                    .iter()
                    .map(|rows| rows.map(|row| row.weight))
                    .collect::<Vec<_>>();
                let saved = WeightSnapshot::of_saved(&saved_pair, &saved_edges);
                let raw_before = pool_raw_row_digest(&engine.state);

                let lifecycle = |arm: PoolRebaseArm, nonfinite: bool| {
                    let mut vector_engine = Engine::from_constructor_at_depth(
                        &pieces,
                        settings,
                        &placements,
                        constructor_depth,
                        config,
                    )?;
                    if let Some(row) = vector_engine.state.pair_rows.first_mut() {
                        row.weight = if nonfinite { f64::NAN } else { 2.0 };
                    }
                    if let Some(row) = vector_engine.state.pair_rows.last_mut() {
                        row.weight = 17.0;
                    }
                    if let Some(rows) = vector_engine.state.edge_rows.first_mut() {
                        rows[3].weight = 1_024.0;
                    }
                    Ok::<_, String>(vector_engine.pool_rebase_lifecycle_vector(arm, seed))
                };
                let lifecycle_saved = lifecycle(PoolRebaseArm::Saved, false)?;
                let lifecycle_rebase = lifecycle(PoolRebaseArm::Rebase, false)?;
                let lifecycle_compute = lifecycle(PoolRebaseArm::ComputeIgnore, false)?;
                let lifecycle_nonfinite = lifecycle(PoolRebaseArm::Rebase, true)?;
                let rollback = engine.pool_rebase_rollback_weight_vector();
                let mut saved_state = engine.state.clone();
                let saved_reset = apply_weight_policy(
                    &mut saved_state,
                    PoolRebaseArm::Saved,
                    &saved_pair,
                    &saved_edges,
                );
                let mut rebase_state = engine.state.clone();
                let rebase_reset = apply_weight_policy(
                    &mut rebase_state,
                    PoolRebaseArm::Rebase,
                    &saved_pair,
                    &saved_edges,
                );
                let mut compute_state = engine.state.clone();
                let compute_reset = apply_weight_policy(
                    &mut compute_state,
                    PoolRebaseArm::ComputeIgnore,
                    &saved_pair,
                    &saved_edges,
                );
                let new_width =
                    engine.pool_rebase_new_width_reset_vector(constructor_depth * 0.999);
                let nonfinite = WeightSnapshot::of_saved(&[f64::NAN], &[[1.0; 4], [1.0; 4]]);
                let disruption_identity = |row: &polygon_nesting_core::search::overlap_ics::pool_rebase::PoolRetryRecord| {
                    (
                        row.disruption.clone(),
                        row.disruption_work_delta,
                        row.disruption_pose_transform_digest_sha256,
                        row.post_disruption_pose_digest_sha256,
                        row.post_disruption_raw_row_digest_sha256,
                    )
                };
                document["poolRetryRebaseVectors"] = json!({
                    "savedInput": weight_snapshot_json(&saved),
                    "saved": {
                        "reset": saved_reset.as_ref().map(weight_snapshot_json),
                        "post": weight_snapshot_json(&WeightSnapshot::of(&saved_state)),
                        "rawRowDigestSha256": hex_bytes(&pool_raw_row_digest(&saved_state)),
                    },
                    "rebase": {
                        "reset": rebase_reset.as_ref().map(weight_snapshot_json),
                        "post": weight_snapshot_json(&WeightSnapshot::of(&rebase_state)),
                        "rawRowDigestSha256": hex_bytes(&pool_raw_row_digest(&rebase_state)),
                    },
                    "computeIgnore": {
                        "reset": compute_reset.as_ref().map(weight_snapshot_json),
                        "post": weight_snapshot_json(&WeightSnapshot::of(&compute_state)),
                        "rawRowDigestSha256": hex_bytes(&pool_raw_row_digest(&compute_state)),
                    },
                    "nonfiniteSavedVisible": !nonfinite.all_finite,
                    "nonfiniteLifecycleInvalid": !lifecycle_nonfinite.valid
                        && !lifecycle_nonfinite.saved_weights.all_finite,
                    "rawRowDigestSha256": hex_bytes(&raw_before),
                    "rawRowsUnchanged": raw_before == pool_raw_row_digest(&saved_state)
                        && raw_before == pool_raw_row_digest(&rebase_state)
                        && raw_before == pool_raw_row_digest(&compute_state),
                    "savedRestoredExactly": WeightSnapshot::of(&saved_state).bits == saved.bits,
                    "rebaseAllExactlyOne": WeightSnapshot::of(&rebase_state).all_exactly_one,
                    "computeIgnoreRestoredExactly":
                        WeightSnapshot::of(&compute_state).bits == saved.bits,
                    "lifecycle": {
                        "savedValid": lifecycle_saved.valid,
                        "rebaseValid": lifecycle_rebase.valid,
                        "computeIgnoreValid": lifecycle_compute.valid,
                        "savedPostPolicy": weight_snapshot_json(
                            &lifecycle_saved.post_policy_weights),
                        "rebasePostPolicy": weight_snapshot_json(
                            &lifecycle_rebase.post_policy_weights),
                        "computeIgnorePostPolicy": weight_snapshot_json(
                            &lifecycle_compute.post_policy_weights),
                        "savedRawRows": hex_bytes(
                            &lifecycle_saved.post_disruption_raw_row_digest_sha256),
                        "rebaseRawRows": hex_bytes(
                            &lifecycle_rebase.post_disruption_raw_row_digest_sha256),
                        "computeIgnoreRawRows": hex_bytes(
                            &lifecycle_compute.post_disruption_raw_row_digest_sha256),
                    },
                    "lifecycleDisruptionIdentical":
                        disruption_identity(&lifecycle_saved)
                            == disruption_identity(&lifecycle_rebase)
                        && disruption_identity(&lifecycle_saved)
                            == disruption_identity(&lifecycle_compute),
                    "inSeparationRollback": {
                        "snapshotWeights": weight_snapshot_json(&rollback.snapshot_weights),
                        "evolvedWeights": weight_snapshot_json(&rollback.evolved_weights),
                        "restoredWeights": weight_snapshot_json(&rollback.restored_weights),
                        "snapshotRawRowDigestSha256":
                            hex_bytes(&rollback.snapshot_raw_row_digest_sha256),
                        "preRollbackRawRowDigestSha256":
                            hex_bytes(&rollback.pre_rollback_raw_row_digest_sha256),
                        "postRollbackRawRowDigestSha256":
                            hex_bytes(&rollback.post_rollback_raw_row_digest_sha256),
                        "posesRestored": rollback.poses_restored,
                        "valid": rollback.valid,
                    },
                    "newWidthReset": {
                        "widthMm": new_width.width_mm,
                        "weights": weight_snapshot_json(&new_width.weights),
                        "coldWeights": weight_snapshot_json(&new_width.cold_weights),
                        "liveRawRowDigestSha256":
                            hex_bytes(&new_width.live_raw_row_digest_sha256),
                        "coldRawRowDigestSha256":
                            hex_bytes(&new_width.cold_raw_row_digest_sha256),
                        "valid": new_width.valid,
                    },
                });
            }
            #[cfg(not(feature = "pool-retry-tracker-rebase"))]
            return Err("pool-rebase-vectors requires pool-retry-tracker-rebase".into());
        }
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
                    arm: BinaryCloseArm::MinCut,
                    invalid_decisions: u64::from(!decision.valid),
                    decisions: vec![decision],
                };
                let geometry_vector = geometry_gate0_vector_report();
                let synthetic_geometry_trace = BinaryCloseTrace {
                    arm: BinaryCloseArm::MinCut,
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
        #[cfg(feature = "pool-retry-tracker-rebase")]
        "pool-rebase-resume" => {
            let checkpoint_path = options.required("checkpointin")?;
            let plan_path = options.required("plan")?;
            let checkpoint_bytes = fs::read(checkpoint_path)
                .map_err(|error| format!("reading checkpoint `{checkpoint_path}`: {error}"))?;
            let plan_bytes = fs::read(plan_path)
                .map_err(|error| format!("reading plan `{plan_path}`: {error}"))?;
            let plan = plan_from_bytes(&plan_bytes)?;
            let bindings = pool_checkpoint_bindings(
                &options,
                &request_sha256,
                &pieces,
                &sources,
                settings,
                contract,
                &plan_bytes,
            )?;
            let wanted = PlanKey {
                request_sha256: request_sha256.clone(),
                currency_version: CurrencyVersion::U0Samples,
                binary_key: BinaryKey {
                    executable_sha256: bindings.executable_sha256.clone(),
                    features: bindings.features.clone(),
                },
                workers: 8,
                executor: Executor::EphemeralScope,
            };
            if let PlanMatch::Miss(reason) = match_plan(&wanted, &plan.key) {
                return Err(format!("checkpoint resume plan mismatch: {reason}").into());
            }
            let body = pool_checkpoint_body(&checkpoint_bytes, &bindings)?;
            let arm = pool_rebase_arm(&options)?;
            if arm == PoolRebaseArm::ComputeIgnore && options.integer("poolrebasetiming", 0)? == 0 {
                return Err("compute-ignore resume is reserved for timed G0.3 cells".into());
            }
            let solver_started = Instant::now();
            let outcome = Engine::resume_first_pool_retry_from_checkpoint(
                &pieces,
                sources.clone(),
                settings,
                contract,
                &body,
                arm,
                options.integer("poolrebasetiming", 0)? != 0,
            )?;
            wall.insert(
                "searchSeconds".to_owned(),
                json!(solver_started.elapsed().as_secs_f64()),
            );
            let checkpoint_sha256 = format!("{:x}", Sha256::digest(&checkpoint_bytes));
            document["checkpoint"] = json!({
                "schema": "pool-retry-tracker-rebase/checkpoint-envelope/v2",
                "inputSha256": checkpoint_sha256,
                "byteLength": checkpoint_bytes.len(),
                "canonicalReencodeIdentical":
                    pool_checkpoint_envelope(&bindings, &body) == checkpoint_bytes,
                "bodySha256": format!("{:x}", Sha256::digest(&body)),
                "bindings": {
                    "specSha256": bindings.spec_sha256,
                    "requestSha256": bindings.request_sha256,
                    "planSha256": bindings.plan_sha256,
                    "executableSha256": bindings.executable_sha256,
                    "sourceCommit": bindings.source_commit,
                    "features": bindings.features,
                    "immutableInputSha256": bindings.immutable_input_sha256,
                },
            });
            document["schedule"] = json!({
                "mode": "checkpoint-resume",
                "workers": 8,
                "arm": "control",
                "armLabel": StrikeConfig::CONTROL.arm(),
                "poolRetryArm": arm.as_str(),
                "recordPoolRetryRebase": true,
                "stopAfterFirstPoolRetry": true,
                "firstPoolRetryIterationCap": FIRST_RETRY_ITERATION_CAP,
                "recordFingerprints": true,
                "recordPoolRetryTiming": options.integer("poolrebasetiming", 0)? != 0,
                "calibratedPlan": {
                    "path": plan_path,
                    "sourceSha256": format!("{:x}", Sha256::digest(&plan_bytes)),
                    "match": "hit",
                    "wantedKey": serde_json::to_value(&wanted)?,
                },
            });
            let authority_parent = outcome
                .bites
                .first()
                .and_then(|row| row.published.as_ref())
                .map(|row| row.parent_fingerprint.clone())
                .unwrap_or_else(|| outcome.incumbent.placement_fingerprint.clone());
            document["outcome"] = schedule_json(
                &outcome,
                &authority_parent,
                &LayoutContext {
                    sources: &sources,
                    pieces: &pieces,
                    settings,
                    contract: &contract,
                    revalidate: true,
                },
            );
            document["finalPoseDigest"] = json!(pose_digest(&outcome.final_poses));
        }
        #[cfg(not(feature = "pool-retry-tracker-rebase"))]
        "pool-rebase-resume" => {
            return Err("pool-rebase-resume requires pool-retry-tracker-rebase".into());
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
            // `--start`: the constructor has run and been accounted for; now
            // its layout is replaced by the loaded one, and the three values
            // the trajectory is built from - placements, initial target depth
            // `T`, incumbent fingerprint - are re-derived from the loaded
            // layout by the same two calls above. See `StartLayout` for why
            // this exists and why it is diagnostic only.
            let (placements, constructor_depth, constructor_fingerprint) = match options
                .get("start")
            {
                None => (placements, constructor_depth, constructor_fingerprint),
                Some(start_path) => {
                    let load_started = Instant::now();
                    let start_bytes = fs::read(start_path)
                        .map_err(|error| format!("--start: reading `{start_path}`: {error}"))?;
                    let start_sha256 = format!("{:x}", Sha256::digest(&start_bytes));
                    let piece_ids = pieces.iter().map(|piece| piece.id).collect::<Vec<_>>();
                    let loaded = start_layout_placements(&start_bytes, &piece_ids)?;
                    // The cell's own contract, through the untouched
                    // validator. A refusal is the answer, never a fallback.
                    let revalidation =
                        publish::independently_revalidate(&pieces, &loaded, settings, &contract);
                    if !revalidation.contract_valid {
                        return Err(format!(
                                "--start: `{start_path}` is not contract-valid at edge {} / pair {}: {}",
                                contract.sheet_edge_clearance_mm,
                                settings.total_padding_mm,
                                revalidation
                                    .contract_error
                                    .as_deref()
                                    .unwrap_or("the contract validator refused without a message")
                            )
                            .into());
                    }
                    let loaded_depth = raw_depth_of(&pieces, &loaded, &contract);
                    let loaded_fingerprint = placement_fingerprint(&loaded);
                    wall.insert(
                        "startLoadSeconds".to_owned(),
                        json!(load_started.elapsed().as_secs_f64()),
                    );
                    started_from = Some(json!({
                        "path": start_path,
                        "sha256": start_sha256,
                        "rawSourceDepthMm": loaded_depth,
                        "placementFingerprint": loaded_fingerprint,
                        "placementCount": loaded.len(),
                        "contractValid": revalidation.contract_valid,
                        "kernelExclusiveValid": revalidation.kernel_exclusive_valid,
                        "kernelError": revalidation.kernel_error,
                        // The layout this run did NOT start from.
                        "replacedConstructor": {
                            "rawSourceDepthMm": constructor_depth,
                            "placementFingerprint": constructor_fingerprint,
                        },
                    }));
                    (loaded, loaded_depth, loaded_fingerprint)
                }
            };
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
            // Only when named. Absent, the engine's own default applies, so a
            // bare cell measures the shipped pacer; `--itercap=0` is still the
            // explicit unbounded arm every pre-ratification replay needs.
            if let Some(value) = options.get("itercap") {
                polygon_nesting_core::search::overlap_ics::set_wall_iteration_cap(
                    value
                        .parse()
                        .map_err(|_| format!("--itercap: `{value}`"))?,
                );
            }
            // The only thing the quorum wrote. `legacy` is the default and is
            // the engine as it was; `wall10s` is `ICS-10s-coarse-v1` and
            // refuses any request that is not a caller-named ten-second wall.
            let profile = match options.get("profile").unwrap_or("legacy") {
                "legacy" => polygon_nesting_core::search::overlap_ics::ScheduleProfile::Legacy,
                "wall10s" => polygon_nesting_core::search::overlap_ics::ScheduleProfile::Wall10s,
                other => return Err(format!("--profile: `{other}`").into()),
            };
            profile.validate_for(mode == "wall", options.number("wall", 10.0)?)?;
            polygon_nesting_core::search::overlap_ics::set_schedule_profile(profile);
            // H1, publish at the achieved depth: `--publishachieved=1` replaces
            // both `> target_depth_mm` refusals in `publish::attempt` with the
            // improvement gate. Off by default; the default path is the frozen
            // engine to the bit.
            polygon_nesting_core::search::overlap_ics::set_publish_achieved(
                options.integer("publishachieved", 0)? != 0,
            );
            // Proxy margin: `--proxymargin=<um>` measures every proxy pair row
            // against `pair + m` and every boundary against `edge + m` (the top
            // aims `m` under `T`), so a proxy-zero state lies strictly inside
            // the Exclusive kernel's region. Default 0; the default path is the
            // frozen engine to the bit.
            polygon_nesting_core::search::overlap_ics::set_proxy_margin_um(
                options.integer("proxymargin", 0)?,
            );
            // Guided exponent: `--guidedexponent=<p>` ranks the relocate's
            // candidates and the tournament's winner on `sum w v^p` instead of
            // `sum w v^2` (the bite microscope's pinned column, GPT-6 Astra
            // review 5 Q3). Default 2; the default path is the frozen engine
            // to the bit.
            polygon_nesting_core::search::overlap_ics::set_guided_exponent(
                options.guided_exponent()?,
            )?;
            homotopy::set_explore_shrink_step(options.number("shrinkstep", 0.0)?);
            homotopy::set_adaptive_step_ceiling(options.number("adaptivestep", 0.0)?);
            homotopy::set_adaptive_step_floor(options.number("adaptivefloor", 0.0)?);
            homotopy::set_compress_start_step(options.number("compressstep", 0.0)?);
            homotopy::set_pool_spread(options.number("poolspread", 0.0)?);
            #[cfg(feature = "t-row-repair")]
            {
                use polygon_nesting_core::search::overlap_ics::publish::{
                    set_t_row_arm, TRowArm,
                };
                set_t_row_arm(match options.get("trow").unwrap_or("off") {
                    "off" => TRowArm::Off,
                    "repair" => TRowArm::Repair,
                    "computeignore" => TRowArm::ComputeIgnore,
                    other => {
                        return Err(format!(
                            "--trow must be off|repair|computeignore, not `{other}`"
                        )
                        .into())
                    }
                });
            }
            polygon_nesting_core::search::overlap_ics::set_explore_patience(
                options.integer("patience", 0)? as u64,
            );
            let strikes = match arm.as_str() {
                "control" => StrikeConfig::control_live(),
                "treatment" => StrikeConfig::TREATMENT,
                other => {
                    return Err(format!("--arm must be control|treatment, not `{other}`").into())
                }
            };
            #[cfg(feature = "minimum-conflict-binary-close")]
            let binary_close_arm = binary_close_arm(&options, "binaryclose")?;
            #[cfg(feature = "pool-retry-tracker-rebase")]
            let pool_rebase_arm = pool_rebase_arm(&options)?;
            let schedule = ScheduleConfig {
                workers,
                strikes,
                record_fingerprints: options.integer("fingerprints", 0)? != 0,
                #[cfg(feature = "minimum-conflict-binary-close")]
                binary_close_arm,
                #[cfg(feature = "pool-retry-tracker-rebase")]
                pool_rebase_arm,
                #[cfg(feature = "pool-retry-tracker-rebase")]
                record_pool_rebase: options.integer("poolrebasetrace", 0)? != 0,
                #[cfg(feature = "pool-retry-tracker-rebase")]
                stop_after_first_pool_retry: options.integer("firstretry", 0)? != 0,
                #[cfg(feature = "pool-retry-tracker-rebase")]
                capture_first_pool_retry_checkpoint: options.get("checkpointout").is_some(),
                #[cfg(feature = "pool-retry-tracker-rebase")]
                record_pool_rebase_timing: options.integer("poolrebasetiming", 0)? != 0,
                // **The explore/compress split, exposed rather than changed.**
                // `ScheduleConfig::default()` is Sparrow's 0.8, which
                // `consts.rs` sets and the paper's Table 1 tuned for
                // twenty-minute runs. At ten seconds it hands 20 % of the
                // budget to a phase whose steps are 0.05 % decaying to 0.001 %,
                // against explore's flat 0.1 %. Whether that is the right
                // division at this budget is a measurement, and this flag is
                // how it gets measured. The default is unchanged.
                explore_time_ratio: options
                    .number("exploreratio", homotopy::EXPLORE_TIME_RATIO)?,
                // The bite microscope, at its prospectively fixed trigger
                // (34 master iterations, three bites after) or at the depth
                // trigger `--microscopetarget` names. Diagnostic only.
                bite_microscope: microscope_config,
                ..ScheduleConfig::default()
            };
            #[cfg(feature = "pool-retry-tracker-rebase")]
            if schedule.stop_after_first_pool_retry
                && (!schedule.record_pool_rebase || !schedule.record_fingerprints)
            {
                return Err(
                    "--firstretry=1 requires --poolrebasetrace=1 and --fingerprints=1".into(),
                );
            }
            #[cfg(feature = "pool-retry-tracker-rebase")]
            if schedule.stop_after_first_pool_retry && mode == "wall" {
                return Err("--firstretry=1 requires fixed or calibrated work, not wall".into());
            }
            #[cfg(feature = "pool-retry-tracker-rebase")]
            if schedule.capture_first_pool_retry_checkpoint {
                if mode != "calibrated"
                    || options.get("poolrebase").is_some()
                    || schedule.stop_after_first_pool_retry
                    || !schedule.record_fingerprints
                    || schedule.record_pool_rebase
                    || schedule.record_pool_rebase_timing
                {
                    return Err(
                        "--checkpointout requires calibrated mode and --fingerprints=1, with no \
                         --poolrebase, --poolrebasetrace, --poolrebasetiming or --firstretry"
                            .into(),
                    );
                }
            }
            let wall_budget_s = options.number("wall", 10.0)?;
            // Built before the budget, because a calibrated budget moves into
            // `run_cutclose` and a plan is worth printing whether or not the
            // trajectory that spends it succeeds.
            let mut plan_document = Value::Null;
            #[cfg(feature = "pool-retry-tracker-rebase")]
            let mut plan_source_bytes: Option<Vec<u8>> = None;
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
                    #[cfg(feature = "pool-retry-tracker-rebase")]
                    {
                        plan_source_bytes = Some(bytes.clone());
                    }
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
            #[cfg(feature = "pool-retry-tracker-rebase")]
            if let Some(path) = options.get("checkpointout") {
                if let Some(error) = &outcome.pool_rebase.checkpoint_error {
                    return Err(format!("capturing pool-retry checkpoint: {error}").into());
                }
                let body = outcome
                    .pool_rebase
                    .checkpoint_bytes
                    .as_ref()
                    .ok_or("trajectory ended before the first pool-rank checkpoint")?;
                let plan_bytes = plan_source_bytes
                    .as_deref()
                    .ok_or("checkpoint capture has no calibrated plan bytes")?;
                let bindings = pool_checkpoint_bindings(
                    &options,
                    &request_sha256,
                    &pieces,
                    &sources,
                    settings,
                    contract,
                    plan_bytes,
                )?;
                let artifact = pool_checkpoint_envelope(&bindings, body);
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .map_err(|error| {
                        format!("creating checkpoint `{path}` exclusively: {error}")
                    })?;
                file.write_all(&artifact)?;
                file.sync_all()?;
                document["checkpoint"] = json!({
                    "schema": "pool-retry-tracker-rebase/checkpoint-envelope/v2",
                    "outputSha256": format!("{:x}", Sha256::digest(&artifact)),
                    "byteLength": artifact.len(),
                    "bodySha256": format!("{:x}", Sha256::digest(body)),
                    "captureSeam": "explore-after-first-pool-rank-before-install",
                    "armNeutral": true,
                    "bindings": {
                        "specSha256": bindings.spec_sha256,
                        "requestSha256": bindings.request_sha256,
                        "planSha256": bindings.plan_sha256,
                        "executableSha256": bindings.executable_sha256,
                        "sourceCommit": bindings.source_commit,
                        "features": bindings.features,
                        "immutableInputSha256": bindings.immutable_input_sha256,
                    },
                });
            }
            document["constructor"] = json!({
                "rawSourceDepthMm": constructor_depth,
                "placementFingerprint": constructor_fingerprint,
                "placementCount": placements.len(),
                "lowerScaleMm": lower_scale_mm,
            });
            // The `--start` tripwire's second half: present exactly when the
            // constructor's layout was replaced. Absence means today's path.
            if started_from.is_some() {
                document["constructor"]["startedFrom"] = json!(true);
            }
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
            #[cfg(feature = "pool-retry-tracker-rebase")]
            if schedule.pool_rebase_arm != PoolRebaseArm::Saved
                || schedule.record_pool_rebase
                || schedule.stop_after_first_pool_retry
                || schedule.capture_first_pool_retry_checkpoint
                || schedule.record_pool_rebase_timing
            {
                document["schedule"]["poolRetryArm"] = json!(schedule.pool_rebase_arm.as_str());
                document["schedule"]["recordPoolRetryRebase"] = json!(schedule.record_pool_rebase);
                document["schedule"]["stopAfterFirstPoolRetry"] =
                    json!(schedule.stop_after_first_pool_retry);
                document["schedule"]["firstPoolRetryIterationCap"] = schedule
                    .stop_after_first_pool_retry
                    .then_some(FIRST_RETRY_ITERATION_CAP)
                    .into();
                document["schedule"]["captureFirstPoolRetryCheckpoint"] =
                    json!(schedule.capture_first_pool_retry_checkpoint);
                document["schedule"]["recordPoolRetryTiming"] =
                    json!(schedule.record_pool_rebase_timing);
            }
            document["outcome"] = schedule_json(
                &outcome,
                &constructor_fingerprint,
                &LayoutContext {
                    sources: &sources,
                    pieces: &pieces,
                    settings,
                    contract: &contract,
                    revalidate: options.integer("revalidate", 0)? != 0,
                },
            );
            document["finalPoseDigest"] = json!(pose_digest(&outcome.final_poses));
            // The microscope's report, serialized after the timed region with
            // `f64` as-is (`float_roundtrip`), and the schedule block's half
            // of the tripwire. Present exactly when the flag was on.
            if let Some(report) = &outcome.bite_microscope {
                bite_microscope = Some(serde_json::to_value(report)?);
                document["schedule"]["biteMicroscope"] = json!(true);
            }

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
            // The corpus cell's force-correlation clause
            // (`corpus::gradient_probe_step`) walks the gradient of
            // `sum w v^2` and accepts a rung on the incident guided energy,
            // which under `--guidedexponent != 2` is `sum w v^p`. Rather than
            // audit a `v^2` direction against a `v^p` acceptance, the cell
            // refuses the knob (the knob commit's verifier named this).
            let guided_exponent = options.guided_exponent()?;
            if guided_exponent != polygon_nesting_core::search::overlap_ics::DEFAULT_GUIDED_EXPONENT {
                return Err(format!(
                    "--cell=corpus: the force-correlation clause is defined for the quadratic \
                     guided objective; --guidedexponent={guided_exponent} is refused here"
                )
                .into());
            }
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
            let record_consumed_orders = options.integer("consumedorders", 0)? != 0;
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
                    record_consumed_orders,
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
            let prefix_legacy_proposals = engine.proposals();
            wall.insert("prefixSeconds".to_owned(), json!(prefix_seconds));
            let search_started = Instant::now();
            let outcome = engine.run_cutclose(
                ScheduleConfig {
                    workers,
                    record_fingerprints,
                    #[cfg(feature = "minimum-conflict-binary-close")]
                    record_consumed_orders,
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
            let total_legacy_proposals = engine.proposals();
            wall.insert("searchSeconds".to_owned(), json!(search_seconds));
            wall.insert(
                "totalSearchSeconds".to_owned(),
                json!(prefix_seconds + search_seconds),
            );

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
                "prefixPoseDigestSha256": pose_digest(&prefix_outcome.final_poses),
                "prefixPoses": poses_json(&prefix_outcome.final_poses),
                "prefixLegacyProposals": prefix_legacy_proposals,
                "probeLegacyProposals": total_legacy_proposals - prefix_legacy_proposals,
                "totalLegacyProposals": total_legacy_proposals,
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
            #[cfg(feature = "minimum-conflict-binary-close")]
            if record_consumed_orders {
                document["spawnTax"]["prefixCompleteStateDigestSha256"] =
                    json!(prefix_outcome.complete_state_digest_sha256);
                document["spawnTax"]["prefixConsumedOrderDigestSha256"] =
                    json!(prefix_outcome.consumed_order_digest_sha256);
                document["spawnTax"]["prefixConsumedOrderSweeps"] =
                    json!(prefix_outcome.consumed_order_sweeps);
                document["spawnTax"]["prefixConsumedOrderSlots"] =
                    json!(prefix_outcome.consumed_order_slots);
            }
            document["outcome"] = schedule_json(
                &outcome,
                &constructor_fingerprint,
                &LayoutContext {
                    sources: &sources,
                    pieces: &pieces,
                    settings,
                    contract: &contract,
                    revalidate: options.integer("revalidate", 0)? != 0,
                },
            );
            document["prefixOutcome"] = schedule_json(
                &prefix_outcome,
                &constructor_fingerprint,
                &LayoutContext {
                    sources: &sources,
                    pieces: &pieces,
                    settings,
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
        // **The evaluation-cost bench.** `ics-profile` says 95.6 % of the
        // engine is one worker sweep and the counters say a sweep costs 2.7 us
        // per candidate evaluation. This times the three parts of one
        // evaluation with the clock *outside* the loop, so the measurement does
        // not pay for itself: the transform of the whole decomposition, the
        // rebuild of the piece's rows against every other piece, and the fold.
        "evalcost" => {
            use polygon_nesting_core::search::overlap_ics::energy::{
                incident_totals, rebuild_piece_rows,
            };
            use polygon_nesting_core::search::overlap_ics::state::transform_piece;
            let placements = ShortSideFirst.layout(&pieces, settings)?;
            let constructor_depth = raw_depth_of(&pieces, &placements, &contract);
            let mut engine = Engine::from_constructor_at_depth(
                &pieces,
                settings,
                &placements,
                constructor_depth,
                IcsConfig {
                    target_depth_mm: constructor_depth,
                    proposal_budget: 0,
                    relocate_eval_budget: u64::MAX,
                    checkpoint_every_sweeps: u64::MAX,
                    descent: descent_config(&options, &contract, &sources, seed)?,
                    limits: publication_limits(&options)?,
                },
            )?;
            let rounds: u64 = options.integer("rounds", 200_000)? as u64;
            let piece = options.integer("piece", 0)? as usize;
            let mut work = WorkVector::default();
            let contract = engine.contract;
            let base = engine.state.poses[piece];

            // Warm the caches with the same access pattern the timed loops use.
            for _ in 0..1_000 {
                transform_piece(&engine.sources, &mut engine.state.geometry,
                                &engine.state.poses, piece);
            }

            let started = std::time::Instant::now();
            for step in 0..rounds {
                engine.state.poses[piece].tx_mm = base.tx_mm + (step % 7) as f64 * 1e-6;
                transform_piece(&engine.sources, &mut engine.state.geometry,
                                &engine.state.poses, piece);
            }
            let transform_ns = started.elapsed().as_nanos() as f64 / rounds as f64;

            let started = std::time::Instant::now();
            for step in 0..rounds {
                engine.state.poses[piece].tx_mm = base.tx_mm + (step % 7) as f64 * 1e-6;
                transform_piece(&engine.sources, &mut engine.state.geometry,
                                &engine.state.poses, piece);
                rebuild_piece_rows(&mut engine.state, &contract, piece, &mut work);
            }
            let plus_rows_ns = started.elapsed().as_nanos() as f64 / rounds as f64;

            let started = std::time::Instant::now();
            let mut sink = 0.0f64;
            for step in 0..rounds {
                engine.state.poses[piece].tx_mm = base.tx_mm + (step % 7) as f64 * 1e-6;
                transform_piece(&engine.sources, &mut engine.state.geometry,
                                &engine.state.poses, piece);
                rebuild_piece_rows(&mut engine.state, &contract, piece, &mut work);
                let (raw, weighted) = incident_totals(&engine.state, piece);
                sink += raw + weighted;
            }
            let full_ns = started.elapsed().as_nanos() as f64 / rounds as f64;

            // **The fixed floor.** Move the piece far outside the sheet so every
            // one of the `n-1` box tests rejects and no pair survives. What is
            // left is the `O(n)` scan itself, which a maintained near-set would
            // remove and which no amount of geometry pruning can.
            engine.state.poses[piece].tx_mm = base.tx_mm + 100_000.0;
            engine.state.poses[piece].ty_mm = base.ty_mm + 100_000.0;
            transform_piece(&engine.sources, &mut engine.state.geometry,
                            &engine.state.poses, piece);
            let far = engine.state.poses[piece];
            let started = std::time::Instant::now();
            for step in 0..rounds {
                engine.state.poses[piece].tx_mm = far.tx_mm + (step % 7) as f64 * 1e-6;
                transform_piece(&engine.sources, &mut engine.state.geometry,
                                &engine.state.poses, piece);
                rebuild_piece_rows(&mut engine.state, &contract, piece, &mut work);
            }
            let floor_ns = started.elapsed().as_nanos() as f64 / rounds as f64;
            engine.state.poses[piece] = base;
            transform_piece(&engine.sources, &mut engine.state.geometry,
                            &engine.state.poses, piece);

            // **Box tests without the row writes.** `rebuild_piece_rows` does
            // two things per other piece: a cheap box test, and a scattered
            // write into the triangular `pair_rows` array - 1,830 rows, about
            // 100 kB, indexed by `pair_index`. This times the tests alone, so
            // the two are told apart before anything is redesigned around the
            // wrong one.
            let count = engine.state.poses.len();
            let clearance = contract.pair_clearance_mm();
            let started = std::time::Instant::now();
            let mut near_count = 0u64;
            for step in 0..rounds {
                engine.state.poses[piece].tx_mm = base.tx_mm + (step % 7) as f64 * 1e-6;
                transform_piece(&engine.sources, &mut engine.state.geometry,
                                &engine.state.poses, piece);
                let mine = engine.state.geometry.piece_bounds[piece];
                for other in 0..count {
                    if other != piece
                        && polygon_nesting_core::search::overlap_ics::broad_phase::pair_is_near(
                            mine,
                            engine.state.geometry.piece_bounds[other],
                            clearance,
                        )
                    {
                        near_count += 1;
                    }
                }
            }
            let box_tests_ns = started.elapsed().as_nanos() as f64 / rounds as f64;
            engine.state.poses[piece] = base;
            transform_piece(&engine.sources, &mut engine.state.geometry,
                            &engine.state.poses, piece);

            document["evalCost"] = json!({
                "boxTestsOnlyNs": box_tests_ns - transform_ns,
                "nearPerRound": near_count as f64 / rounds as f64,
                "rounds": rounds,
                "piece": piece,
                "transformNs": transform_ns,
                "transformPlusRowsNs": plus_rows_ns,
                "fullEvaluateNs": full_ns,
                "rowsNs": plus_rows_ns - transform_ns,
                "foldNs": full_ns - plus_rows_ns,
                "transformShare": transform_ns / full_ns,
                "rowsShare": (plus_rows_ns - transform_ns) / full_ns,
                "foldShare": (full_ns - plus_rows_ns) / full_ns,
                "sink": sink,
                "scanFloorNs": floor_ns,
                "scanFloorShare": floor_ns / full_ns,
            });
        }
        // **The replay probes** (`overlap_ics::replay`). DIAGNOSTIC ONLY.
        //
        // WHY: `docs/experiments/overlap-ics/sparrow-warm-start/README.md`
        // shows the hard bite costing us 37-43 master iterations against
        // Sparrow's 17 passes from the identical layout, and the bite
        // microscope traced that bite (339/339 fine-CD exits by `limits`,
        // 282 with residual; 176/492 changed rows on a `visited` endpoint).
        // Astra review 4 Q3/Q4 prescribe two detached replays of that traced
        // bite before any live mechanism is built. This cell is those
        // replays: it rebuilds the state from a capsule, re-runs the same
        // separation loop from it, and reports whether a probe clears the
        // persistent blockers and enters the band, at what evaluation cost.
        // The third probe, `--probe=exponent:<p>`, is the microscope
        // README's addendum "how the column broke": neither detached probe
        // broke the pinned column early because the escape is a weight
        // race under `w v^2` (weights ~1e5, 36 updates) where Sparrow's
        // ~sqrt(penetration) needs ~20; the probe ranks the candidates and
        // the tournament on `w v^p` and nothing else. `exponent:2` must
        // reproduce `none` bit for bit (the identity count printed below).
        // The committed-geometry instrument (Astra review 5b Q6-Q8,
        // `docs/astra-review-5b-the-exponent.md`) adds to every replay
        // document the winner's committed relocates, the column read off
        // the blocking graph (`replay.column`, `replay.columnLongestLived`),
        // pose/weight/stream fingerprints in the identity gate, the
        // resolved seed, and two flags: `--fork=<sweep>` (re-score sweep
        // <sweep>+1's candidates under the four exponents, then stop) and
        // `--certify=1` (call the unchanged live publication path once at
        // band entry and record its answer).
        //
        // FORBIDDEN AS A RESULT. A capsule is a known-good layout;
        // `docs/grok-review-12-reading-sparrow.md` §5.2 (row "fixture as a
        // seed") forbids starting a scored cell from one. So this cell
        // installs nothing, is never a default, and its document carries
        // `replay.tripwire`. It stops at band entry; the exact call the
        // live loop would make there is made only under `--certify=1`,
        // once, after the stop, and its answer - a publication included -
        // is recorded in `replay.certification` of this refused document
        // and nowhere else.
        "replay" => {
            let capsule_path = options.required("capsule")?.to_owned();
            let capsule_bytes = fs::read(&capsule_path)
                .map_err(|error| format!("--capsule: reading `{capsule_path}`: {error}"))?;
            let capsule_sha256 = format!("{:x}", Sha256::digest(&capsule_bytes));
            let capsule_document: Value = serde_json::from_slice(&capsule_bytes)
                .map_err(|error| format!("--capsule: `{capsule_path}`: {error}"))?;
            let bite_ordinal = options.integer("bite", 0)?;
            if bite_ordinal == 0 {
                return Err("--bite=<ordinal> names the traced bite to replay".into());
            }
            let probe = ReplayProbe::parse(options.get("probe").unwrap_or("none"))?;
            // `--horizon=live`: the cap is the traced attempt's own sweep
            // count, read below once the attempt is known (Astra review 7
            // Q18: the replayed objective gets exactly the live attempt's
            // budget). `--maxiters=<n>` is the fixed cap as before; naming
            // both is refused rather than resolved.
            let live_horizon = match options.get("horizon") {
                None => false,
                Some("live") => true,
                Some(other) => {
                    return Err(format!("--horizon must be `live` (or absent), not `{other}`").into())
                }
            };
            if live_horizon && options.get("maxiters").is_some() {
                return Err("--horizon=live and --maxiters name two caps; pass one of them".into());
            }
            let mut max_iterations = options.integer("maxiters", 200)?;
            let workers = options.integer("workers", 8)? as usize;
            // `--fork=<sweep>`: the fixed diagnostic fork of Astra 5b Q8, in
            // sweep <sweep> + 1; `--certify=1`: the detached publication check
            // of Astra 5b Q6 at band entry (`overlap_ics::replay`).
            let fork = match options.get("fork") {
                None => None,
                Some(value) => {
                    let sweep: u64 = value
                        .parse()
                        .map_err(|_| format!("--fork=<sweep>: `{value}` is not a sweep number"))?;
                    if !matches!(probe, ReplayProbe::None | ReplayProbe::Exponent(_)) {
                        return Err(format!(
                            "--fork runs only with --probe=none or --probe=exponent:<p>, not \
                             `{}` (the fork re-scores the exponent path's candidates)",
                            probe.label()
                        )
                        .into());
                    }
                    Some(sweep)
                }
            };
            let certify = match options.get("certify") {
                None | Some("0") => false,
                Some("1") => true,
                Some(other) => return Err(format!("--certify must be 0 or 1, not `{other}`").into()),
            };

            // The document must be a trace of THIS request under THIS
            // contract: every mismatch is a refusal, never a fallback.
            let document_request_sha = capsule_document["request"]["sha256"]
                .as_str()
                .ok_or("--capsule: the document has no request.sha256")?;
            if document_request_sha != request_sha256 {
                return Err(format!(
                    "--capsule: the document traced request {document_request_sha}, not this \
                     request ({request_sha256})"
                )
                .into());
            }
            let document_margin = capsule_document["proxyMarginUm"].as_u64().unwrap_or(0);
            let proxy_margin = options.integer("proxymargin", 0)?;
            if document_margin != proxy_margin {
                return Err(format!(
                    "--capsule: the document was traced at --proxymargin={document_margin}; \
                     this run named --proxymargin={proxy_margin}. They must agree."
                )
                .into());
            }
            polygon_nesting_core::search::overlap_ics::set_proxy_margin_um(proxy_margin);
            // Same rule for the guided exponent: the control replay folds at
            // the process knob (`energy::fold` / `incident_totals`), so the
            // reconstruction reproduces the trace only when the two agree.
            // `--probe=exponent:<p>` names its own `p` on top of this and is
            // unchanged.
            let document_exponent = capsule_document["guidedExponent"]
                .as_f64()
                .unwrap_or(polygon_nesting_core::search::overlap_ics::DEFAULT_GUIDED_EXPONENT);
            let guided_exponent = options.guided_exponent()?;
            if document_exponent != guided_exponent {
                return Err(format!(
                    "--capsule: the document was traced at --guidedexponent={document_exponent}; \
                     this run named --guidedexponent={guided_exponent}. They must agree."
                )
                .into());
            }
            polygon_nesting_core::search::overlap_ics::set_guided_exponent(guided_exponent)?;
            let document_seed = capsule_document["seed"]
                .as_u64()
                .ok_or("--capsule: the document has no seed")?;
            if options.get("seed").is_some() && seed != document_seed {
                return Err(format!(
                    "--capsule: the document was traced at --seed={document_seed}; this run \
                     named --seed={seed}"
                )
                .into());
            }
            let seed = document_seed;
            let document_workers = capsule_document["schedule"]["workers"].as_u64();
            if let Some(traced_workers) = document_workers {
                if traced_workers as usize != workers {
                    return Err(format!(
                        "--capsule: the document was traced with {traced_workers} workers; \
                         this run named --workers={workers}. Identity needs the same tournament."
                    )
                    .into());
                }
            }
            let document_pair = capsule_document["contract"]["pairClearanceMm"].as_f64();
            let document_edge = capsule_document["contract"]["physicalEdgeClearanceMm"].as_f64();
            if document_pair != Some(contract.pair_clearance_mm())
                || document_edge != Some(contract.physical_edge_clearance_mm())
            {
                return Err(format!(
                    "--capsule: the document's contract (pair {:?}, edge {:?}) is not this run's \
                     (pair {}, edge {})",
                    document_pair,
                    document_edge,
                    contract.pair_clearance_mm(),
                    contract.physical_edge_clearance_mm()
                )
                .into());
            }
            let profile = match options.get("profile").unwrap_or("legacy") {
                "legacy" => polygon_nesting_core::search::overlap_ics::ScheduleProfile::Legacy,
                "wall10s" => polygon_nesting_core::search::overlap_ics::ScheduleProfile::Wall10s,
                other => return Err(format!("--profile: `{other}`").into()),
            };
            polygon_nesting_core::search::overlap_ics::set_schedule_profile(profile);
            polygon_nesting_core::search::overlap_ics::set_explore_patience(
                options.integer("patience", 0)? as u64,
            );

            let microscope = &capsule_document["biteMicroscope"];
            if microscope.is_null() {
                return Err("--capsule: the document carries no biteMicroscope block".into());
            }
            let document_pieces = microscope["pieces"].as_u64().unwrap_or(0) as usize;
            if document_pieces != pieces.len() {
                return Err(format!(
                    "--capsule: the trace has {document_pieces} pieces; this request has {}",
                    pieces.len()
                )
                .into());
            }
            let bites = microscope["bites"]
                .as_array()
                .ok_or("--capsule: biteMicroscope.bites is not an array")?;
            let bite = bites
                .iter()
                .find(|bite| bite["ordinal"].as_u64() == Some(bite_ordinal))
                .ok_or_else(|| {
                    format!(
                        "--capsule: bite {bite_ordinal} is not retained; retained: {:?}",
                        bites
                            .iter()
                            .filter_map(|bite| bite["ordinal"].as_u64())
                            .collect::<Vec<_>>()
                    )
                })?;
            let capsules = microscope["capsules"]
                .as_array()
                .ok_or("--capsule: biteMicroscope.capsules is not an array")?;
            let default_index = bite["capsule"].as_u64().unwrap_or(0);
            let capsule_index = options.integer("capsuleindex", default_index)?;
            let capsule = capsules.get(capsule_index as usize).ok_or_else(|| {
                format!(
                    "--capsuleindex={capsule_index}: the document has {} capsules",
                    capsules.len()
                )
            })?;
            if capsule["bite"].as_u64() != Some(bite_ordinal) {
                return Err(format!(
                    "--capsuleindex={capsule_index} belongs to bite {:?}, not bite {bite_ordinal}",
                    capsule["bite"]
                )
                .into());
            }
            // The exponent the capture ran under: the capsule's own
            // `guidedExponent` (the live capture writes it only when the
            // knob was on) and the document's top-level field are the same
            // process knob, so they must agree with each other as they were
            // just made to agree with this run's; absence is the frozen
            // engine's 2.
            let captured_exponent = capsule["guidedExponent"]
                .as_f64()
                .unwrap_or(document_exponent);
            if captured_exponent != document_exponent {
                return Err(format!(
                    "--capsuleindex={capsule_index}: the capsule was captured at \
                     guidedExponent={captured_exponent} but the document says \
                     {document_exponent}; a document does not disagree with its own capsule"
                )
                .into());
            }
            let capsule_label = capsule["label"].as_str().unwrap_or("").to_owned();
            // The separation this capsule opens: the bite-entry capsule opens
            // attempt 0; an after-disruption capsule opens the attempt after
            // the reset that captured it.
            let separation_attempt = if capsule_label == "bite-entry" {
                0
            } else {
                bite["resets"]
                    .as_array()
                    .and_then(|resets| {
                        resets
                            .iter()
                            .find(|reset| reset["capsule"].as_u64() == Some(capsule_index))
                            .and_then(|reset| reset["afterAttempt"].as_u64())
                    })
                    .map_or(0, |after| after + 1)
            };
            let separations = bite["separations"].as_array().cloned().unwrap_or_default();
            let separation = separations
                .iter()
                .find(|call| call["attempt"].as_u64() == Some(separation_attempt));
            let traced_sweeps: Vec<Value> = separation
                .and_then(|call| call["sweeps"].as_array().cloned())
                .unwrap_or_default();
            if live_horizon {
                max_iterations = traced_sweeps.len() as u64;
            }
            let traced: Vec<TracedSweep> = traced_sweeps
                .iter()
                .map(|sweep| {
                    // The extended identity gate's reference: the sweep's
                    // committed relocates (piece, displacement bits, changed
                    // rows), its evaluation counts and its stream key.
                    let relocates = sweep["relocates"]
                        .as_array()
                        .map(|relocates| {
                            relocates
                                .iter()
                                .map(|relocate| {
                                    Ok(TracedRelocate {
                                        piece: relocate["piece"]
                                            .as_u64()
                                            .ok_or("--capsule: a relocate has no piece")?
                                            as u32,
                                        dx_mm: relocate["dxMm"]
                                            .as_f64()
                                            .ok_or("--capsule: a relocate has no dxMm")?,
                                        dy_mm: relocate["dyMm"]
                                            .as_f64()
                                            .ok_or("--capsule: a relocate has no dyMm")?,
                                        dtheta_deg: relocate["dthetaDeg"]
                                            .as_f64()
                                            .ok_or("--capsule: a relocate has no dthetaDeg")?,
                                        rows: relocate["rows"]
                                            .as_array()
                                            .into_iter()
                                            .flatten()
                                            .map(|row| {
                                                Ok((
                                                    row[0].as_u64().ok_or(
                                                        "--capsule: a row change has no rowId",
                                                    )? as u32,
                                                    row[1].as_f64().ok_or(
                                                        "--capsule: a row change has no before",
                                                    )?,
                                                    row[2].as_f64().ok_or(
                                                        "--capsule: a row change has no after",
                                                    )?,
                                                ))
                                            })
                                            .collect::<Result<Vec<_>, String>>()?,
                                    })
                                })
                                .collect::<Result<Vec<_>, String>>()
                        })
                        .transpose()?
                        .unwrap_or_default();
                    let stream = sweep["stream"].as_object().map(|stream| {
                        polygon_nesting_core::search::overlap_ics::microscope::StreamKey {
                            seed: stream.get("seed").and_then(Value::as_u64).unwrap_or(0),
                            bite: stream.get("bite").and_then(Value::as_u64).unwrap_or(0),
                            iteration: stream.get("iteration").and_then(Value::as_u64).unwrap_or(0),
                            worker: stream.get("worker").and_then(Value::as_u64).unwrap_or(0),
                        }
                    });
                    Ok(TracedSweep {
                        iteration: sweep["iteration"]
                            .as_u64()
                            .ok_or("--capsule: a sweep has no iteration")?,
                        raw_after: sweep["rawAfter"]
                            .as_f64()
                            .ok_or("--capsule: a sweep has no rawAfter")?,
                        max_after_mm: sweep["maxAfterMm"]
                            .as_f64()
                            .ok_or("--capsule: a sweep has no maxAfterMm")?,
                        winner: sweep["winner"]
                            .as_u64()
                            .ok_or("--capsule: a sweep has no winner")?
                            as u32,
                        evaluations_all_workers: sweep["evaluationsAllWorkers"]
                            .as_u64()
                            .ok_or("--capsule: a sweep has no evaluationsAllWorkers")?,
                        evaluations_winner: sweep["evaluationsWinner"]
                            .as_u64()
                            .ok_or("--capsule: a sweep has no evaluationsWinner")?,
                        stream,
                        relocates,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            // The control's persistent blocking rows: the five rows most often
            // in the traced sweeps' end-of-sweep blocking set, ties by row id.
            let mut persistence: BTreeMap<u32, (u64, u64, f64)> = BTreeMap::new();
            for sweep in &traced_sweeps {
                let iteration = sweep["iteration"].as_u64().unwrap_or(0);
                for row in sweep["blocking"].as_array().into_iter().flatten() {
                    let (Some(id), Some(residual)) = (row[0].as_u64(), row[1].as_f64()) else {
                        continue;
                    };
                    let entry = persistence.entry(id as u32).or_insert((0, 0, 0.0));
                    entry.0 += 1;
                    entry.1 = iteration;
                    entry.2 = entry.2.max(residual);
                }
            }
            let mut ranked: Vec<(u32, (u64, u64, f64))> = persistence.into_iter().collect();
            ranked.sort_by(|left, right| right.1 .0.cmp(&left.1 .0).then(left.0.cmp(&right.0)));
            let watched: Vec<(u32, (u64, u64, f64))> = ranked.into_iter().take(5).collect();
            let watch_rows: Vec<u32> = watched.iter().map(|row| row.0).collect();
            // The control, read off the trace: its band entry and the
            // evaluations it had spent by then.
            let traced_samples: Vec<Value> = separation
                .and_then(|call| call["samples"].as_array().cloned())
                .unwrap_or_default();
            let traced_band_entry = traced_samples
                .iter()
                .find(|sample| sample[5].as_bool() == Some(true))
                .and_then(|sample| sample[0].as_u64());
            let traced_evaluations_to_band = traced_band_entry.map(|at| {
                traced_sweeps
                    .iter()
                    .filter(|sweep| sweep["iteration"].as_u64().unwrap_or(u64::MAX) <= at)
                    .map(|sweep| sweep["evaluationsAllWorkers"].as_u64().unwrap_or(0))
                    .sum::<u64>()
            });

            // The state: the request's sources at the capsule's poses, the
            // rows rebuilt cold, the weights restored bit for bit, the
            // stream put back. `IcsConfig::target_depth_mm` is the capsule's.
            let capsule_poses = capsule["poses"]
                .as_array()
                .ok_or("--capsule: the capsule has no poses")?;
            let capsule_mirrored = capsule["mirrored"]
                .as_array()
                .ok_or("--capsule: the capsule has no mirrored")?;
            if capsule_poses.len() != pieces.len() || capsule_mirrored.len() != pieces.len() {
                return Err(format!(
                    "--capsule: the capsule holds {} poses / {} mirror bits for {} pieces",
                    capsule_poses.len(),
                    capsule_mirrored.len(),
                    pieces.len()
                )
                .into());
            }
            let poses: Vec<Pose> = capsule_poses
                .iter()
                .zip(capsule_mirrored)
                .map(|(pose, mirrored)| {
                    Ok(Pose {
                        tx_mm: pose[0].as_f64().ok_or("--capsule: a pose has no tx")?,
                        ty_mm: pose[1].as_f64().ok_or("--capsule: a pose has no ty")?,
                        theta_deg: pose[2].as_f64().ok_or("--capsule: a pose has no theta")?,
                        mirrored: mirrored
                            .as_bool()
                            .ok_or("--capsule: a mirror bit is not a bool")?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            let pair_weights: Vec<f64> = capsule["pairWeights"]
                .as_array()
                .ok_or("--capsule: the capsule has no pairWeights")?
                .iter()
                .map(|weight| weight.as_f64().ok_or("--capsule: a pair weight is not a number"))
                .collect::<Result<Vec<_>, _>>()?;
            let edge_weights: Vec<[f64; 4]> = capsule["edgeWeights"]
                .as_array()
                .ok_or("--capsule: the capsule has no edgeWeights")?
                .iter()
                .map(|weights| {
                    let mut out = [0.0f64; 4];
                    for (slot, weight) in out.iter_mut().zip(weights.as_array().into_iter().flatten())
                    {
                        *slot = weight
                            .as_f64()
                            .ok_or("--capsule: an edge weight is not a number")?;
                    }
                    Ok(out)
                })
                .collect::<Result<Vec<_>, String>>()?;
            let target_depth_mm = capsule["targetDepthMm"]
                .as_f64()
                .ok_or("--capsule: the capsule has no targetDepthMm")?;
            let stream_iteration = capsule["stream"]["iteration"]
                .as_u64()
                .ok_or("--capsule: the capsule's stream has no iteration")?;
            let stream_seed = capsule["stream"]["seed"].as_u64().unwrap_or(seed);
            if stream_seed != seed {
                return Err(format!(
                    "--capsule: the capsule's stream seed {stream_seed} is not the document's \
                     seed {seed}"
                )
                .into());
            }
            let proposals = capsule["proposals"].as_u64().unwrap_or(0);
            // The capsule doc: the worker coordinate a replay sets from the
            // selected sweep's winner (the tournament re-sets it per slot
            // anyway; `iteration` is the coordinate that matters).
            let stream_worker = traced.first().map_or(0, |sweep| u64::from(sweep.winner));
            let config = IcsConfig {
                target_depth_mm,
                proposal_budget: 0,
                relocate_eval_budget: u64::MAX,
                checkpoint_every_sweeps: u64::MAX,
                descent: descent_config(&options, &contract, &sources, seed)?,
                limits: publication_limits(&options)?,
            };
            // The replay never publishes, so the incumbent is a placeholder
            // that no comparison ever reads.
            let incumbent = ExactIncumbent {
                placements: Vec::new(),
                raw_source_depth_mm: f64::INFINITY,
                from_constructor: false,
                placement_fingerprint: "replay-capsule".to_owned(),
            };
            let mut engine = Engine::from_poses(
                &pieces,
                settings,
                sources.clone(),
                contract,
                poses,
                incumbent,
                config,
            );
            engine.restore_capsule_weights(&pair_weights, &edge_weights)?;
            engine.restore_replay_stream(bite_ordinal, stream_worker, stream_iteration, proposals);
            // The reconstruction check: the rebuilt state's fold against the
            // capsule's own reading, bit for bit.
            let rebuilt = engine.totals();
            let capsule_raw = capsule["raw"].as_f64().unwrap_or(f64::NAN);
            let capsule_guided = capsule["guided"].as_f64().unwrap_or(f64::NAN);
            let capsule_max = capsule["maxMm"].as_f64().unwrap_or(f64::NAN);
            let reconstruction = json!({
                "rawEqual": rebuilt.raw.to_bits() == capsule_raw.to_bits(),
                "guidedEqual": rebuilt.guided.to_bits() == capsule_guided.to_bits(),
                "maxEqual": rebuilt.max_violation_mm.to_bits() == capsule_max.to_bits(),
                "raw": rebuilt.raw,
                "guided": rebuilt.guided,
                "maxMm": rebuilt.max_violation_mm,
                "capsuleRaw": capsule_raw,
                "capsuleGuided": capsule_guided,
                "capsuleMaxMm": capsule_max,
            });
            eprintln!(
                "replay: capsule {capsule_index} ({capsule_label}) of bite {bite_ordinal}, \
                 attempt {separation_attempt}, {} traced sweeps; horizon {} ({} iterations); \
                 captured exponent {captured_exponent}, trajectory exponent {}; reconstruction \
                 raw {} guided {} max {}",
                traced.len(),
                if live_horizon { "live" } else { "maxiters" },
                max_iterations,
                match probe {
                    ReplayProbe::Exponent(exponent) => exponent,
                    _ => guided_exponent,
                },
                if reconstruction["rawEqual"] == json!(true) { "EQUAL" } else { "DIFFERS" },
                if reconstruction["guidedEqual"] == json!(true) { "EQUAL" } else { "DIFFERS" },
                if reconstruction["maxEqual"] == json!(true) { "EQUAL" } else { "DIFFERS" },
            );

            let params = ReplayParams {
                workers,
                bite: bite_ordinal,
                max_iterations,
                live_horizon,
                probe,
                strikes: StrikeConfig::control_live(),
                traced,
                watch_rows,
                fork,
                certify,
            };
            let search_started = Instant::now();
            let report = engine.replay_separation(&params);
            wall.insert(
                "searchSeconds".to_owned(),
                json!(search_started.elapsed().as_secs_f64()),
            );
            // The identity gate, printed per iteration. The control must pass
            // every iteration; a probe is expected to diverge.
            for row in &report.identity {
                eprintln!(
                    "replay identity iteration {:>3}: {} raw {:e} vs {:e}, max {:e} vs {:e}, \
                     winner {} vs {}; scalars {}, relocates {} ({} vs {}{}), evaluations {} \
                     ({}/{} vs {}/{}), stream {:?}; poses {} weights {} stream {}",
                    row.iteration,
                    if row.equal { "PASS" } else { "FAIL" },
                    row.raw_trace,
                    row.raw_replay,
                    row.max_trace,
                    row.max_replay,
                    row.winner_trace,
                    row.winner_replay,
                    row.scalars_equal,
                    row.relocates_equal,
                    row.relocates_replay,
                    row.relocates_trace,
                    row.relocates_first_difference
                        .as_ref()
                        .map_or(String::new(), |difference| format!("; {difference}")),
                    row.evaluations_equal,
                    row.evaluations_all_workers,
                    row.evaluations_winner,
                    row.evaluations_all_workers_trace,
                    row.evaluations_winner_trace,
                    row.stream_equal,
                    row.poses_fingerprint,
                    row.weights_fingerprint,
                    row.stream_fingerprint,
                );
            }
            let relocates_equal_rows =
                report.identity.iter().filter(|row| row.relocates_equal).count();
            let evaluations_equal_rows =
                report.identity.iter().filter(|row| row.evaluations_equal).count();
            let stream_equal_rows = report
                .identity
                .iter()
                .filter(|row| row.stream_equal == Some(true))
                .count();
            eprintln!(
                "replay identity ({}): {}/{} PASS, {} FAIL{} (extended gate: scalars, relocates \
                 {}/{}, evaluations {}/{}; stream {}/{})",
                probe.label(),
                report.identity_pass,
                report.identity.len(),
                report.identity_fail,
                report
                    .diverges_from_trace_at_iteration
                    .map_or(String::new(), |at| format!(", first divergence at iteration {at}")),
                relocates_equal_rows,
                report.identity.len(),
                evaluations_equal_rows,
                report.identity.len(),
                stream_equal_rows,
                report.identity.len(),
            );
            eprintln!("{}", report.column.summary("column"));
            eprintln!("{}", report.column_longest_lived.summary("column longest-lived"));
            if let Some(fork) = &report.fork {
                eprintln!("{}", fork.summary());
            }
            if let Some(certification) = &report.certification {
                eprintln!(
                    "replay certification (attempted {}, published {}, refusal {:?}, depthMm {:?}, \
                     proxyDepthMm {:.6}, targetDepthMm {:.6}, exactCalls {}, reason {:?})",
                    certification.attempted,
                    certification.published,
                    certification.refusal,
                    certification.depth_mm,
                    certification.proxy_depth_mm,
                    certification.target_depth_mm,
                    certification.exact_calls,
                    certification.reason,
                );
            }
            eprintln!(
                "replay ({}): stop {} after {} iterations; bandEnteredAtIteration {:?}; \
                 evaluationsToBand {:?}; evaluationsTotal {}; continuationEvaluations {}; \
                 queuedRelocates {} (winner) / {} (all workers); control band entry {:?} at {:?} evaluations",
                probe.label(),
                report.stop,
                report.iterations.len(),
                report.band_entered_at_iteration,
                report.evaluations_to_band,
                report.evaluations_total,
                report.continuation_evaluations_total,
                report.queued_relocates_total,
                report.queued_relocates_all_workers_total,
                traced_band_entry,
                traced_evaluations_to_band,
            );
            for row in &report.watched_rows {
                eprintln!(
                    "replay ({}): persistent row {} -> {:?}, cleared at {:?} (blocking in {} of {} \
                     replay states, entry included; entry residual {:.6} mm; end residual {:.6} mm)",
                    probe.label(),
                    row.row_id,
                    row.status,
                    row.cleared_at_iteration,
                    row.blocking_iterations,
                    report.iterations.len() + 1,
                    row.entry_residual_mm,
                    row.end_residual_mm,
                );
            }

            let mut replay = serde_json::to_value(&report)?;
            replay["capsule"] = json!({
                "path": capsule_path,
                "sha256": capsule_sha256,
                "index": capsule_index,
                "label": capsule_label,
                "bite": bite_ordinal,
                "separationAttempt": separation_attempt,
                "targetDepthMm": target_depth_mm,
                "stream": capsule["stream"].clone(),
                "proposals": proposals,
                "raw": capsule_raw,
                "guided": capsule_guided,
                "maxMm": capsule_max,
                "seed": seed,
                "workers": document_workers,
                "proxyMarginUm": document_margin,
                "wallIterationCap": capsule_document["wallIterationCap"].clone(),
                "explorePatience": capsule_document["explorePatience"].clone(),
                // The exponent the capture ran under, read from the capsule
                // (or the document's top level; absent means the frozen
                // engine's 2), never assumed: a capsule captured under
                // `--guidedexponent=<p>` replays only at that `p`.
                "capturedExponent": captured_exponent,
                "pieces": pieces.len(),
            });
            replay["reconstruction"] = reconstruction;
            replay["params"] = json!({
                "probe": probe.label(),
                "maxIterations": max_iterations,
                "horizon": report.horizon,
                // The objective the trajectory ran under: the probe's `p`,
                // or the live knob (the document's own) for the control.
                "trajectoryExponent": match probe {
                    ReplayProbe::Exponent(exponent) => exponent,
                    _ => guided_exponent,
                },
                "workers": workers,
                "bandMm": report.band_mm,
                "continuation": report.continuation,
                "revisit": report.revisit,
                "exponent": report.exponent,
                "strikes": StrikeConfig::control_live().arm(),
                "explorePatience": polygon_nesting_core::search::overlap_ics::explore_patience(),
                "stopsAtBandEntry": true,
                "exactCalls": report.certification.as_ref().map_or(0, |c| c.exact_calls),
                "fork": fork,
                "certify": certify,
                "revisitStream": "RelocateKey::revisit: the sweep's key with the worker ordinal tagged",
            });
            // Astra 5b Q7 item 5: the trajectory actually replayed is the
            // capsule's seed, whatever `--seed` the top level prints.
            document["resolvedSeed"] = json!(seed);
            replay["control"] = json!({
                "tracedMasterIterations": bite["masterIterations"].clone(),
                "tracedPublished": bite["published"].clone(),
                "tracedStop": separation.map(|call| call["stop"].clone()).unwrap_or(Value::Null),
                "tracedIterations": separation.map(|call| call["iterations"].clone()).unwrap_or(Value::Null),
                "bandEnteredAtIteration": traced_band_entry,
                "evaluationsToBand": traced_evaluations_to_band,
                "evaluationsTotal": traced_sweeps
                    .iter()
                    .map(|sweep| sweep["evaluationsAllWorkers"].as_u64().unwrap_or(0))
                    .sum::<u64>(),
            });
            replay["persistentRows"] = json!(watched
                .iter()
                .map(|(id, (count, last, residual))| json!({
                    "rowId": id,
                    "tracedBlockingSweeps": count,
                    "tracedLastBlockingIteration": last,
                    "tracedMaxResidualMm": residual,
                }))
                .collect::<Vec<_>>());
            replay_document = Some(replay);
        }
        other => return Err(format!("unknown cell `{other}`").into()),
    }

    wall.insert(
        "totalSeconds".to_owned(),
        json!(started.elapsed().as_secs_f64()),
    );
    document["wall"] = Value::Object(wall);
    document["exploreShrinkStep"] = json!(homotopy::explore_shrink_step());
    document["scheduleProfile"] = json!(match polygon_nesting_core::search::overlap_ics::schedule_profile() {
        polygon_nesting_core::search::overlap_ics::ScheduleProfile::Legacy => "legacy",
        polygon_nesting_core::search::overlap_ics::ScheduleProfile::Wall10s => "wall10s",
    });
    document["adaptiveStepCeiling"] = json!(homotopy::adaptive_step_ceiling());
    document["adaptiveStepFloor"] = json!(homotopy::adaptive_step_floor());
    document["compressStartStep"] = json!(homotopy::compress_start_step());
    document["poolSpread"] = json!(homotopy::pool_spread());
    document["explorePatience"] =
        json!(polygon_nesting_core::search::overlap_ics::explore_patience());
    document["wallIterationCap"] =
        json!(polygon_nesting_core::search::overlap_ics::wall_iteration_cap());
    // Present exactly when the mechanism is on. The frozen binary's document
    // has no such key and the bit-identity check hashes the whole document, so
    // a `false` written unconditionally would fail identity on key presence
    // alone while changing no trajectory. Absence means off.
    if polygon_nesting_core::search::overlap_ics::publish_achieved() {
        document["publishAchieved"] = json!(true);
    }
    // Same rule: present exactly when the margin is on. Absence means zero.
    let proxy_margin_um = polygon_nesting_core::search::overlap_ics::proxy_margin_um();
    if proxy_margin_um != 0 {
        document["proxyMarginUm"] = json!(proxy_margin_um);
    }
    // Same rule: present exactly when the guided exponent is not the frozen
    // engine's 2. Absence means `sum w v^2`.
    let guided_exponent = polygon_nesting_core::search::overlap_ics::guided_exponent();
    if guided_exponent != polygon_nesting_core::search::overlap_ics::DEFAULT_GUIDED_EXPONENT {
        document["guidedExponent"] = json!(guided_exponent);
    }
    // Same rule, and the loudest of the three: present exactly when the
    // trajectory did not start from the constructor's layout. A scorer must
    // refuse any document that carries this key (`StartLayout`).
    if let Some(started_from) = started_from.take() {
        document["startedFrom"] = started_from;
    }
    // Same rule, same loudness: present exactly when `--bitemicroscope=1`
    // ran. The `tripwire` field is what a scorer refuses on; the rest is the
    // trace (`overlap_ics::microscope`).
    if let Some(mut report) = bite_microscope.take() {
        let flag = match options.get("microscopetarget") {
            Some(value) => format!("--microscopetarget={value}"),
            None => "--bitemicroscope=1".to_owned(),
        };
        report["tripwire"] = json!(format!(
            "DIAGNOSTIC ONLY: {flag} ran; this document carries replay capsules \
             (known-good layouts) and must never be scored (forbidden-rescue row: fixture as a seed)"
        ));
        report["flag"] = json!(flag);
        document["biteMicroscope"] = report;
    }
    // Same rule, and louder still: present exactly when `--cell=replay` ran.
    // The replay started from a capsule's known-good layout and installed
    // nothing; its band entry is a diagnostic reading, never a depth, and
    // under `--certify=1` the one live publication call made after that
    // stop is recorded in `replay.certification` of this document only
    // (`overlap_ics::replay`).
    if let Some(mut report) = replay_document.take() {
        let probe_label = report["probe"].as_str().unwrap_or("?").to_owned();
        let certify = report["params"]["certify"] == json!(true);
        let exact_calls = report["params"]["exactCalls"].as_u64().unwrap_or(0);
        report["tripwire"] = json!(format!(
            "DIAGNOSTIC ONLY: --cell=replay ran (probe {probe_label}, certify {certify}, \
             {exact_calls} exact calls); this trajectory started from a bite-microscope \
             capsule (a known-good layout) and installed nothing - under --certify=1 the live \
             publication call is made once after band entry and its answer, a publication \
             included, is recorded in replay.certification of this refused document only - \
             and must never be scored (forbidden-rescue row: fixture as a seed)"
        ));
        report["flag"] = json!("--cell=replay");
        document["replay"] = report;
    }
    document["executableSha256"] = json!(executable_sha256());
    document["buildFeatures"] = json!(build_features());
    // Instrument only, and present only on a census build: which publication
    // gate refused, counted at the band entry.
    #[cfg(feature = "t-row-repair")]
    {
        let census =
            polygon_nesting_core::search::overlap_ics::publish::t_row_census::snapshot();
        document["tRowCensus"] = json!({
            "arm": match polygon_nesting_core::search::overlap_ics::publish::t_row_arm() {
                polygon_nesting_core::search::overlap_ics::publish::TRowArm::Off => "off",
                polygon_nesting_core::search::overlap_ics::publish::TRowArm::Repair => "repair",
                polygon_nesting_core::search::overlap_ics::publish::TRowArm::ComputeIgnore =>
                    "compute-ignore",
            },
            "eligible": census.eligible,
            "eligibleWithTRow": census.eligible_with_t_row,
            "firstScanBoundaryRows": census.first_scan_boundary_rows,
            "published": census.published,
            "refused": census.refused,
            "publishedMaxExcessMm": census.published_max_excess_mm,
            "publishedMaxDisplacementMm": census.published_max_displacement_mm,
            "blockedOnBoundary": census.blocked_on_boundary,
            "blockedOnPair": census.blocked_on_pair,
            "blockedNoNormal": census.blocked_no_normal,
            "blockedSaturated": census.blocked_saturated,
            "blockedDisplacementCap": census.blocked_displacement_cap,
            "blockedRowBudget": census.blocked_row_budget,
            "blockingShortfallMicrometreBuckets": census.blocking_shortfall_um,
            "giveUpFailingPairs": census.give_up_failing_pairs,
            "giveUpFailingBoundaries": census.give_up_failing_boundaries,
            "giveUpTotalShortfallMicrometre": census.give_up_total_shortfall_um,
            "repeatSkipped": census.repeat_skipped,
        });
    }
    #[cfg(feature = "ics-publish-census")]
    {
        let census = polygon_nesting_core::search::overlap_ics::publish_census::snapshot();
        document["publishCensus"] = json!({
            "bandEntries": census.band_entries,
            "digestRepeat": census.digest_repeat,
            "aboveTarget": census.above_target,
            "notImproving": census.not_improving,
            "called": census.called,
            "aboveTargetMinMm": census.above_target_min_mm.is_finite()
                .then_some(census.above_target_min_mm),
            "aboveTargetMaxMm": census.above_target_max_mm.is_finite()
                .then_some(census.above_target_max_mm),
            "aboveTargetBetterThanIncumbent": census.above_target_better_than_incumbent,
            "aboveTargetBestGainMm": census.above_target_best_gain_mm,
            "excessHistogramHalfMicrometre": census.excess_histogram_half_um,
            "frontWithinMicrometre": census.front_within_um,
            "frontSamples": census.front_sampled,
            "aboveTargetUniqueDigests": census.above_target_unique,
        });
    }
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

#[cfg(test)]
mod start_layout_tests {
    use super::*;

    fn layout() -> Vec<GeneralFastPlacement> {
        vec![
            GeneralFastPlacement {
                piece_id: "b".to_owned(),
                rotation_deg: 179.93750000000003,
                mirrored: true,
                translate_short_axis: 159.1695963788811,
                translate_long_axis: 109.11815942459275,
            },
            GeneralFastPlacement {
                piece_id: "a".to_owned(),
                rotation_deg: 0.0,
                mirrored: false,
                translate_short_axis: 12.5,
                translate_long_axis: -3.25,
            },
        ]
    }

    fn bytes_of(placements: &[GeneralFastPlacement]) -> Vec<u8> {
        serde_json::to_vec(&json!({ "placements": placements_json(placements) })).unwrap()
    }

    #[test]
    fn round_trips_through_placements_json() {
        let original = layout();
        let decoded = start_layout_placements(&bytes_of(&original), &["a", "b"]).unwrap();
        assert_eq!(decoded, original);
        assert_eq!(
            placement_fingerprint(&decoded),
            placement_fingerprint(&original)
        );
    }

    #[test]
    fn ignores_extra_keys_at_both_levels() {
        let bytes = br#"{
            "source": "sparrow", "stripWidth": 164.36841,
            "placements": [
                {"pieceId": "a", "itemId": 38, "rotationDeg": 90.0, "mirrored": false,
                 "translateShortAxis": 1.0, "translateLongAxis": 2.0, "extra": [1, 2]}
            ]
        }"#;
        let decoded = start_layout_placements(bytes, &["a"]).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].piece_id, "a");
        assert_eq!(decoded[0].rotation_deg, 90.0);
        assert!(!decoded[0].mirrored);
        assert_eq!(decoded[0].translate_short_axis, 1.0);
        assert_eq!(decoded[0].translate_long_axis, 2.0);
    }

    #[test]
    fn refuses_a_missing_piece() {
        let mut short = layout();
        short.pop();
        let error = start_layout_placements(&bytes_of(&short), &["a", "b"]).unwrap_err();
        assert!(error.contains("`a` has no placement"), "{error}");
    }

    #[test]
    fn refuses_a_duplicate_piece() {
        let mut doubled = layout();
        doubled.push(doubled[0].clone());
        let error = start_layout_placements(&bytes_of(&doubled), &["a", "b"]).unwrap_err();
        assert!(error.contains("`b` is placed twice"), "{error}");
    }

    #[test]
    fn refuses_a_piece_the_request_does_not_have() {
        let error = start_layout_placements(&bytes_of(&layout()), &["a"]).unwrap_err();
        assert!(
            error.contains("`b` names a piece that is not in this request"),
            "{error}"
        );
    }

    #[test]
    fn refuses_a_placement_without_its_mirror_bit() {
        let bytes = br#"{"placements": [{"pieceId": "a", "rotationDeg": 0.0,
            "translateShortAxis": 1.0, "translateLongAxis": 2.0}]}"#;
        let error = start_layout_placements(bytes, &["a"]).unwrap_err();
        assert!(error.contains("mirrored"), "{error}");
    }
}
