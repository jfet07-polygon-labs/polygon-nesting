use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};

use serde::de::{Error as DeError, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{ArchiveIneligibilityReason, EngineError, ProtocolError, SourcePiece};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ExactDecimalString(String);

impl ExactDecimalString {
    pub fn new(value: impl Into<String>) -> Result<Self, ProtocolError> {
        let value = value.into();
        if is_canonical_decimal(&value) {
            Ok(Self(value))
        } else {
            Err(ProtocolError::InvalidDecimalString {
                field: "decimal".to_owned(),
                value,
            })
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl<'de> Deserialize<'de> for ExactDecimalString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct DecimalVisitor;

        impl Visitor<'_> for DecimalVisitor {
            type Value = ExactDecimalString;

            fn expecting(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a canonical base-10 integer string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                if is_canonical_decimal(value) {
                    Ok(ExactDecimalString(value.to_owned()))
                } else {
                    Err(E::custom("expected a canonical base-10 integer string"))
                }
            }
        }

        deserializer.deserialize_str(DecimalVisitor)
    }
}

impl Display for ExactDecimalString {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

pub(crate) fn is_canonical_decimal(value: &str) -> bool {
    if value == "0" {
        return true;
    }
    let digits = value.strip_prefix('-').unwrap_or(value);
    !digits.is_empty()
        && !digits.starts_with('0')
        && digits.as_bytes().iter().all(u8::is_ascii_digit)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrthogonalRotation {
    Deg0,
    Deg90,
}

impl Serialize for OrthogonalRotation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f64(match self {
            Self::Deg0 => 0.0,
            Self::Deg90 => 90.0,
        })
    }
}

impl<'de> Deserialize<'de> for OrthogonalRotation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct RotationVisitor;

        impl Visitor<'_> for RotationVisitor {
            type Value = OrthogonalRotation;

            fn expecting(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("the numeric rotation 0 or 90")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                match value {
                    0 => Ok(OrthogonalRotation::Deg0),
                    90 => Ok(OrthogonalRotation::Deg90),
                    _ => Err(E::custom("expected rotation 0 or 90")),
                }
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                self.visit_u64(value.try_into().map_err(E::custom)?)
            }

            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                if value == 0.0 {
                    Ok(OrthogonalRotation::Deg0)
                } else if value == 90.0 {
                    Ok(OrthogonalRotation::Deg90)
                } else {
                    Err(E::custom("expected rotation 0 or 90"))
                }
            }
        }

        deserializer.deserialize_any(RotationVisitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapacityRouting {
    PreflightProvenImpossible,
    BoundedCompleteArchiveMiss,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityPreflightMeasurements {
    pub piece_count: f64,
    pub sheet_width_grid: f64,
    pub sheet_height_grid: f64,
    pub sheet_doubled_area_grid2: ExactDecimalString,
    pub minimum_doubled_collision_area_sum_grid2: ExactDecimalString,
    pub minimum_collision_area_pressure_ppm: ExactDecimalString,
    pub maximum_singleton_span_pressure_ppm: ExactDecimalString,
    pub singleton_infeasible_piece_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProvenImpossibleKind {
    #[serde(rename = "proven_impossible")]
    ProvenImpossible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InconclusiveKind {
    #[serde(rename = "inconclusive")]
    Inconclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SingletonTransformSetDoesNotFitReason {
    #[serde(rename = "singleton-transform-set-does-not-fit")]
    SingletonTransformSetDoesNotFit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MinimumCollisionAreaExceedsSheetAreaReason {
    #[serde(rename = "minimum-collision-area-exceeds-sheet-area")]
    MinimumCollisionAreaExceedsSheetArea,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SingletonTransformSetDoesNotFitPreflight {
    pub kind: ProvenImpossibleKind,
    pub reason: SingletonTransformSetDoesNotFitReason,
    pub piece_id: String,
    pub measurements: CapacityPreflightMeasurements,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MinimumCollisionAreaExceedsSheetAreaPreflight {
    pub kind: ProvenImpossibleKind,
    pub reason: MinimumCollisionAreaExceedsSheetAreaReason,
    pub measurements: CapacityPreflightMeasurements,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InconclusivePreflight {
    pub kind: InconclusiveKind,
    pub measurements: CapacityPreflightMeasurements,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CapacityPreflightOutcome {
    SingletonTransformSetDoesNotFit(SingletonTransformSetDoesNotFitPreflight),
    MinimumCollisionAreaExceedsSheetArea(MinimumCollisionAreaExceedsSheetAreaPreflight),
    Inconclusive(InconclusivePreflight),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapacityEndpointOrigin {
    ColdSearch,
    PrefixIncumbent,
    WarmPrefixContinuation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityObjective {
    pub placed_count: f64,
    pub placed_doubled_material_area_grid2: ExactDecimalString,
    pub enclosed_cavity_count: f64,
    pub total_enclosed_cavity_area_mm2: f64,
    pub total_enclosed_cavity_doubled_area_grid2: ExactDecimalString,
    pub envelope_maximum_side_mm: f64,
    pub envelope_area_mm2: f64,
    pub envelope_span_mm: f64,
    pub envelope_maximum_side_grid: f64,
    pub envelope_area_grid2: ExactDecimalString,
    pub envelope_span_grid: f64,
    pub canonical_geometry_hash: String,
    pub origin: CapacityEndpointOrigin,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_depth: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_role: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityCavityMetrics {
    pub count: f64,
    pub total_area_mm2: f64,
    pub total_doubled_area_grid2: ExactDecimalString,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityGridSpan {
    pub width_grid: f64,
    pub height_grid: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalLayoutTopology {
    pub enclosed_cavity_count: f64,
    pub largest_occupied_hull_gap_ratio: f64,
    pub occupied_envelope_aspect_ratio: f64,
    pub positive_contact_component_count: f64,
    pub isolated_piece_count: f64,
    pub largest_positive_contact_component_size: f64,
    pub largest_positive_contact_component_ratio: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalLayoutTopologyExact {
    pub topology: CanonicalLayoutTopology,
    pub hull_gap_doubled_area_grid2: f64,
    pub hull_doubled_area_grid2: f64,
    pub exact_hull_gap_doubled_area_grid2: ExactDecimalString,
    pub exact_hull_doubled_area_grid2: ExactDecimalString,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapacityTopologyRepresentativeRole {
    TerminalObjective,
    MinimumComponents,
    MinimumIsolated,
    MaximumLargestComponent,
    MinimumHullWaste,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CapacityDecision {
    Place,
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CapacityProposalRole {
    Compactness,
    Contact,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityTopologyRepresentative {
    pub role: CapacityTopologyRepresentativeRole,
    pub decision_identity: String,
    pub parent_decision_identity: String,
    pub decision: CapacityDecision,
    pub proposal_role: CapacityProposalRole,
    pub piece_id: String,
    pub anchored_occupied_key: String,
    pub placed_count: f64,
    pub placed_doubled_material_area_grid2: ExactDecimalString,
    pub cavities: CapacityCavityMetrics,
    pub grid_span: CapacityGridSpan,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topology: Option<CanonicalLayoutTopologyExact>,
    pub retained: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityTopologyRetentionDepthTrace {
    pub depth: f64,
    pub piece_id: String,
    pub measured_survivor_count: f64,
    pub retained_count: f64,
    pub best_accounting_stratum_count: f64,
    pub topology_measurement_count: f64,
    pub topology_measurement_ms: f64,
    pub legal_candidate_count: f64,
    pub contact_measured_candidate_count: f64,
    pub positive_contact_candidate_count: f64,
    pub contact_measurement_ms: f64,
    pub contact_selected_successor_count: f64,
    pub contact_deduplicated_successor_count: f64,
    pub contact_retained_successor_count: f64,
    pub representatives: Vec<CapacityTopologyRepresentative>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapacitySearchSettlement {
    Exhausted,
    EvaluationCap,
    Paused,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacitySearchTrace {
    pub beam_width: f64,
    pub local_legal_placement_fanout: f64,
    pub placement_evaluation_cap: f64,
    pub placement_evaluation_quota_per_depth: f64,
    pub consumed_placement_evaluations: f64,
    pub auxiliary_placement_evaluations: f64,
    pub pruned_by_attainable_count: f64,
    pub pruned_by_attainable_material: f64,
    pub deduplicated_successors: f64,
    pub fit_rejected_candidates: f64,
    pub invalid_candidates: f64,
    pub endpoint_fit_rejections: f64,
    pub completed_depths: f64,
    pub depth_quota_exhaustions: f64,
    pub piece_count: f64,
    pub settlement: CapacitySearchSettlement,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topology_retention_depths: Option<Vec<CapacityTopologyRetentionDepthTrace>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityPrefixDescriptor {
    pub role: String,
    pub depth: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityPrefixTrace {
    pub captured_count: f64,
    pub fitting_count: f64,
    pub rejected_count: f64,
    pub terminalized_count: f64,
    pub descriptors: Vec<CapacityPrefixDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityIncumbentTrace {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_depth: Option<f64>,
    pub placed_count: f64,
    pub placed_material_area_mm2: f64,
    pub selected_rotation_deg: OrthogonalRotation,
    pub canonical_geometry_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapacityWarmPrefixStatus {
    Settled,
    CheckpointedCensored,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityWarmPrefixLaneTrace {
    pub source_role: String,
    pub prefix_depth: f64,
    pub reused_placed_count: f64,
    pub status: CapacityWarmPrefixStatus,
    pub selected_for_continuation: bool,
    pub checkpoint_retained: bool,
    pub consumed_placement_evaluations: f64,
    pub completed_depths: f64,
    pub elapsed_ms: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<CapacityObjective>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapacityCohesionShadowProducerRole {
    #[serde(rename = "capacity-cohesion-shadow")]
    CapacityCohesionShadow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SettledStatus {
    Settled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoOutputInfluence {
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityCohesionShadowTrace {
    pub producer_role: CapacityCohesionShadowProducerRole,
    pub status: SettledStatus,
    pub output_influence: NoOutputInfluence,
    pub consumed_placement_evaluations: f64,
    pub completed_depths: f64,
    pub elapsed_ms: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<CapacityObjective>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retention_depths: Option<Vec<CapacityTopologyRetentionDepthTrace>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapacityQualityWarmPrefixVersion {
    #[serde(rename = "intrinsic-capacity-quality-warm-prefix-v1")]
    V1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapacityQualityWarmPrefixProducerRole {
    #[serde(rename = "capacity-quality-warm-prefix")]
    CapacityQualityWarmPrefix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapacityQualityWarmPrefixPolicy {
    #[serde(rename = "quality-frontier")]
    QualityFrontier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapacityQualityWarmPrefixStatus {
    SkippedBelowMinimumPieceCount,
    SkippedNoFittingCanonicalPrefix,
    Settled,
    EvaluationCap,
    CheckpointedCensored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapacityQualityWarmPrefixOutputInfluence {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "strict-count-improvement")]
    StrictCountImprovement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CanonicalGridSourceRole {
    #[serde(rename = "canonical-grid")]
    CanonicalGrid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityQualityWarmPrefixTrace {
    pub version: CapacityQualityWarmPrefixVersion,
    pub producer_role: CapacityQualityWarmPrefixProducerRole,
    pub policy: CapacityQualityWarmPrefixPolicy,
    pub status: CapacityQualityWarmPrefixStatus,
    pub output_influence: CapacityQualityWarmPrefixOutputInfluence,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_role: Option<CanonicalGridSourceRole>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_depth: Option<f64>,
    pub reused_placed_count: f64,
    pub request_piece_count: f64,
    pub minimum_piece_count: f64,
    pub placement_evaluation_cap: f64,
    pub consumed_placement_evaluations: f64,
    pub completed_depths: f64,
    pub checkpoint_retained: bool,
    pub elapsed_ms: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<CapacityObjective>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "kebab-case")]
pub enum CapacityContinuedProducer {
    CapacityCold,
    CapacityWarmPrefix {
        source_role: String,
        prefix_depth: f64,
    },
    CapacityQualityWarmPrefix {
        source_role: CanonicalGridSourceRole,
        prefix_depth: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapacityCoordinatorProducerRole {
    CapacityCold,
    CapacityQualityWarmPrefix,
    CapacityWarmPrefix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CapacityCoordinatorPhase {
    Initial,
    Resume,
    Censor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CapacityCoordinatorOutcome {
    Checkpointed,
    Settled,
    Censored,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityLaneCoordinatorQuantum {
    pub ordinal: f64,
    pub producer_role: CapacityCoordinatorProducerRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_depth: Option<f64>,
    pub phase: CapacityCoordinatorPhase,
    pub from_depth: f64,
    pub to_depth: f64,
    pub placement_evaluation_delta: f64,
    pub outcome: CapacityCoordinatorOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapacityLaneCoordinatorVersion {
    #[serde(rename = "intrinsic-capacity-lane-coordinator-v3")]
    V3,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityLaneCoordinatorTrace {
    pub version: CapacityLaneCoordinatorVersion,
    pub aggregate_placement_evaluation_cap: f64,
    pub aggregate_consumed_placement_evaluations: f64,
    pub warm_pilot_depth_boundaries: f64,
    pub continued_producers: Vec<CapacityContinuedProducer>,
    pub retained_checkpoint_count: f64,
    pub censored_lane_count: f64,
    pub quanta: Vec<CapacityLaneCoordinatorQuantum>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacitySelectionTrace {
    #[serde(flatten)]
    pub objective: CapacityObjective,
    pub unplaced_count: f64,
    pub placed_material_area_mm2: f64,
    pub selected_rotation_deg: OrthogonalRotation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapacityTrace {
    pub routing: CapacityRouting,
    pub preflight: CapacityPreflightOutcome,
    pub prefixes: CapacityPrefixTrace,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_incumbent: Option<CapacityIncumbentTrace>,
    pub cold_search: CapacitySearchTrace,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warm_prefix_lanes: Option<Vec<CapacityWarmPrefixLaneTrace>>,
    pub warm_prefix_endpoints_admitted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cohesion_shadow: Option<CapacityCohesionShadowTrace>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality_warm_prefix: Option<CapacityQualityWarmPrefixTrace>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lane_coordinator: Option<CapacityLaneCoordinatorTrace>,
    pub selected: CapacitySelectionTrace,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preflight_runtime_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complete_archive_runtime_ms: Option<f64>,
    pub prefix_terminalization_ms: f64,
    pub cold_search_ms: f64,
    pub runtime_ms: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntrinsicAnytimeSchedulerVersion {
    #[serde(rename = "intrinsic-anytime-scheduler-v1")]
    V1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IntrinsicAnytimeSchedulerColdStartStatus {
    Paused,
    Settled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IntrinsicAnytimeSchedulerCancellationReason {
    CompleteEndpointFitted,
    CompleteCohortMiss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IntrinsicAnytimeSchedulerCohort {
    Partial,
    Complete,
    ExperimentalComplete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IntrinsicAnytimeSchedulerProducerRole {
    CapacityCold,
    CapacityQualityWarmPrefix,
    LegacyComplete,
    CapacityWarmPrefix,
    ExperimentalPlaceDeferComplete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IntrinsicAnytimeSchedulerOutcome {
    Checkpointed,
    Settled,
    Cancelled,
    Censored,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntrinsicAnytimeSchedulerQuantum {
    pub ordinal: usize,
    pub cohort: IntrinsicAnytimeSchedulerCohort,
    pub producer_role: IntrinsicAnytimeSchedulerProducerRole,
    pub outcome: IntrinsicAnytimeSchedulerOutcome,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntrinsicAnytimeSchedulerTrace {
    pub version: IntrinsicAnytimeSchedulerVersion,
    pub cold_quantum_depths: f64,
    pub cold_start_status: IntrinsicAnytimeSchedulerColdStartStatus,
    pub cold_start_completed_depths: f64,
    pub cold_start_consumed_placement_evaluations: f64,
    pub cold_checkpoint_reused: bool,
    pub warm_prefix_endpoints_admitted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancellation_reason: Option<IntrinsicAnytimeSchedulerCancellationReason>,
    pub quanta: Vec<IntrinsicAnytimeSchedulerQuantum>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FocusedCompleteReconstructionVersion {
    #[serde(rename = "intrinsic-focused-complete-reconstruction-v1")]
    V1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FocusedCompleteReconstructionStatus {
    Completed,
    DuplicateOrder,
    EvaluationCap,
    Deadline,
    Incomplete,
    FailedProtectedFallback,
    SkippedPreflightProvenImpossible,
    SkippedNoFittingProtectedEndpoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FocusedCompleteReconstructionOutputInfluence {
    Selected,
    ProtectedFallback,
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusedCompleteReconstructionTrace {
    pub version: FocusedCompleteReconstructionVersion,
    pub status: FocusedCompleteReconstructionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_canonical_geometry_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_canonical_geometry_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_canonical_geometry_hash: Option<String>,
    pub consumed_candidate_evaluations: f64,
    pub candidate_evaluation_accounting_complete: bool,
    pub runtime_ms: f64,
    pub output_influence: FocusedCompleteReconstructionOutputInfluence,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntrinsicShortSideObserverVersion {
    #[serde(rename = "intrinsic-short-side-observer-v6")]
    V6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShortSideObserverStatus {
    Observed,
    ObservedNoLegalOrientation,
    ObservedNoGuardEligibleEndpoint,
    ObservedNoDirectionalImprovement,
    SkippedNoSettledCompleteEndpoints,
    RuntimeBudgetExceeded,
    TraceBudgetExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShortSideOutputInfluence {
    None,
    Selected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SheetAxis {
    Width,
    Height,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ShortSideComparisonValue {
    Number(f64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortSideOrientationObservation {
    pub rotation_deg: OrthogonalRotation,
    pub exact_legal: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_geometry_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_width_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_height_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_long_axis_used_span_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_short_axis_shortfall_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_long_axis_used_span_grid: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_short_axis_shortfall_grid: Option<f64>,
    pub cavity_count: f64,
    pub hull_gap_ratio: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hull_gap_doubled_area_grid2: Option<ExactDecimalString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occupied_hull_doubled_area_grid2: Option<ExactDecimalString>,
    pub cohesion_passes: bool,
    pub cohesion_deficit: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cohesion_deficit_numerator: Option<ExactDecimalString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cohesion_deficit_denominator: Option<ExactDecimalString>,
    pub intrinsic_envelope_area_mm2: f64,
    pub intrinsic_envelope_maximum_side_mm: f64,
    pub intrinsic_envelope_span_mm: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intrinsic_envelope_area_grid2: Option<ExactDecimalString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intrinsic_envelope_maximum_side_grid: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intrinsic_envelope_span_grid: Option<f64>,
    pub dominant_structural_contacts: f64,
    pub total_structural_contacts: f64,
    pub contact_units: f64,
    pub shared_boundary_length_mm: f64,
    pub comparison_tuple: Vec<ShortSideComparisonValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortSideEndpointObservation {
    pub archive_index: f64,
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    pub canonical_geometry_hash: String,
    pub q0: ShortSideOrientationObservation,
    pub q90: ShortSideOrientationObservation,
    pub selected_rotation_deg: OrthogonalRotation,
    pub selected: ShortSideOrientationObservation,
    pub cavity_hull_guard_eligible: bool,
    pub geometric_pareto_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortSideDirectionalAdmissionTerms {
    pub short_edge_fill_admitted: bool,
    pub shortfall_halved: bool,
    pub depth_within_production_maximum_side: bool,
    pub envelope_area_cost_within_production_bound: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntrinsicShortSideObserverTrace {
    pub version: IntrinsicShortSideObserverVersion,
    pub status: ShortSideObserverStatus,
    pub output_influence: ShortSideOutputInfluence,
    pub requested_sheet_width_mm: f64,
    pub requested_sheet_height_mm: f64,
    pub requested_long_axis_mm: f64,
    pub requested_short_axis_mm: f64,
    pub requested_long_axis: SheetAxis,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub production_short_axis_span_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub production_maximum_side_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub production_envelope_area_mm2: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub production_short_axis_span_grid: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub production_maximum_side_grid: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub production_envelope_area_grid2: Option<ExactDecimalString>,
    pub settled_endpoint_count: f64,
    pub evaluated_orientation_count: f64,
    pub cavity_hull_guard_eligible_endpoint_count: f64,
    pub geometric_pareto_eligible_endpoint_count: f64,
    pub placement_evaluations: f64,
    pub candidate_evaluations: f64,
    pub runtime_ms: f64,
    pub runtime_budget_exceeded: bool,
    pub serialized_trace_bytes: f64,
    pub endpoints: Vec<ShortSideEndpointObservation>,
    pub ranked_canonical_geometry_hashes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directional_admission_terms: Option<ShortSideDirectionalAdmissionTerms>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observer_winner_canonical_geometry_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observer_winner_rotation_deg: Option<OrthogonalRotation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShortSideConstructionKind {
    PairFold,
    MultiRowShelf,
    ContactStrip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShortSidePairFoldStatus {
    Accepted,
    SkippedSquareSheet,
    NoPair,
    NoFittingPair,
    RejectedAdmission,
    Deadline,
    MemoryCap,
    TraceCap,
    FailedProtectedFallback,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortSidePairFoldAdmission {
    pub exact_legal: bool,
    pub all_pieces_placed: bool,
    pub fill_ratio: f64,
    pub depth_within_production_maximum_side: bool,
    pub projection_coverage_ratio: f64,
    pub projection_component_count: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enclosed_cavity_count: Option<f64>,
    pub collision_envelope_density: f64,
    pub short_axis_span_gain_factor: f64,
    pub envelope_area_cost_factor: f64,
    pub directionally_efficient: bool,
    pub envelope_area_cost_within_production_bound: bool,
    pub accepted: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortSideEnvelopeAreaCostVeto {
    pub construction_kind: ShortSideConstructionKind,
    pub admission: ShortSidePairFoldAdmission,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortSideInterlockingMetrics {
    pub largest_occupied_hull_gap_ratio: f64,
    pub largest_occupied_hull_gap_doubled_area_grid2: ExactDecimalString,
    pub occupied_hull_doubled_area_grid2: ExactDecimalString,
    pub isolated_piece_count: f64,
    pub positive_contact_component_count: f64,
    pub largest_positive_contact_component_size: f64,
    pub shared_boundary_length_mm: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShortSideContactStripStatus {
    Constructed,
    NoLegalPlacement,
    Deadline,
    MemoryCap,
    EvaluationCap,
    FailedProtectedFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShortSideContactStripVersion {
    #[serde(rename = "intrinsic-short-side-contact-strip-v3")]
    V3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShortSideExecutionModel {
    #[serde(rename = "single-process-sequential")]
    SingleProcessSequential,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShortSideSelectionPolicy {
    DepthFirst,
    ContactFirst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShortSideOrderPolicy {
    Prepared,
    Reverse,
    PieceIdAscending,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortSideContactStripTrace {
    pub version: ShortSideContactStripVersion,
    pub status: ShortSideContactStripStatus,
    pub execution_model: ShortSideExecutionModel,
    pub selection_policy: ShortSideSelectionPolicy,
    pub order_policy: ShortSideOrderPolicy,
    pub strip_short_axis_mm: f64,
    pub strip_long_axis_mm: f64,
    pub transform_evaluations: f64,
    pub candidate_evaluations: f64,
    pub backtrack_count: f64,
    pub reused_prefix_placements: f64,
    pub placed_count: f64,
    pub requested_count: f64,
    pub runtime_ms: f64,
    pub peak_rss_delta_bytes: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortSideConstructionSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_short_axis_span_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_long_axis_depth_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub envelope_area_mm2: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_short_axis_span_grid: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_long_axis_depth_grid: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub envelope_area_grid2: Option<ExactDecimalString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collision_material_doubled_area_grid2: Option<ExactDecimalString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admission: Option<ShortSidePairFoldAdmission>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interlocking: Option<ShortSideInterlockingMetrics>,
    pub status: ShortSidePairFoldStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortSideContactStripPromotion {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incumbent_construction_kind: Option<ShortSideConstructionKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_strip_summary: Option<ShortSideConstructionSummary>,
    pub contact_strip_admitted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_not_regressed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub envelope_area_not_regressed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth_not_regressed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub density_not_regressed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hull_gap_not_regressed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isolated_pieces_not_regressed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub positive_contact_components_not_regressed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub largest_contact_component_not_regressed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strictly_improved: Option<bool>,
    pub promoted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntrinsicShortSidePairFoldVersion {
    #[serde(rename = "intrinsic-short-side-terminal-observer-v6")]
    V6,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntrinsicShortSidePairFoldTrace {
    pub version: IntrinsicShortSidePairFoldVersion,
    pub status: ShortSidePairFoldStatus,
    pub output_influence: ShortSideOutputInfluence,
    pub execution_model: ShortSideExecutionModel,
    pub requested_short_axis_mm: f64,
    pub requested_long_axis_mm: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prescribed_rotation_deg: Option<OrthogonalRotation>,
    pub production_short_axis_span_mm: f64,
    pub production_maximum_side_mm: f64,
    pub production_envelope_area_mm2: f64,
    pub production_short_axis_span_grid: f64,
    pub production_maximum_side_grid: f64,
    pub production_envelope_area_grid2: ExactDecimalString,
    pub transform_evaluations: f64,
    pub expected_pair_count: f64,
    pub evaluated_pair_count: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub construction_kind: Option<ShortSideConstructionKind>,
    pub row_count: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_bottom_piece_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_upper_piece_id: Option<String>,
    pub placed_count: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_short_axis_span_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_long_axis_depth_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub envelope_area_mm2: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_short_axis_span_grid: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_long_axis_depth_grid: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub envelope_area_grid2: Option<ExactDecimalString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collision_material_doubled_area_grid2: Option<ExactDecimalString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_geometry_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admission: Option<ShortSidePairFoldAdmission>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interlocking: Option<ShortSideInterlockingMetrics>,
    pub envelope_area_cost_veto_observed: bool,
    pub envelope_area_cost_vetoes: Vec<ShortSideEnvelopeAreaCostVeto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_strip: Option<ShortSideContactStripTrace>,
    pub contact_strip_lanes: Vec<ShortSideContactStripTrace>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_strip_promotion: Option<ShortSideContactStripPromotion>,
    pub runtime_ms: f64,
    pub peak_rss_delta_bytes: f64,
    pub serialized_trace_bytes: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum EngineOutcome {
    Success {
        result: EngineResult,
        #[serde(default, skip_serializing_if = "ExecutionDiagnostics::is_empty")]
        diagnostics: ExecutionDiagnostics,
    },
    Failure {
        error: EngineError,
        #[serde(default, skip_serializing_if = "ExecutionDiagnostics::is_empty")]
        diagnostics: ExecutionDiagnostics,
    },
    ArchiveIneligible {
        reason: ArchiveIneligibilityReason,
        error: EngineError,
        #[serde(default, skip_serializing_if = "ExecutionDiagnostics::is_empty")]
        diagnostics: ExecutionDiagnostics,
    },
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionDiagnostics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_workers: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_workers: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub counters: BTreeMap<String, u64>,
}

impl ExecutionDiagnostics {
    pub fn is_empty(&self) -> bool {
        self.requested_workers.is_none()
            && self.actual_workers.is_none()
            && self.elapsed_ms.is_none()
            && self.counters.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineResult {
    pub placed_collision_geometries: Vec<PlacedCollisionGeometry>,
    pub score: LayoutScore,
    pub unplaced_piece_ids: Vec<String>,
    pub diagnostics: Vec<ResultDiagnostic>,
    pub sorted_piece_ids: Vec<String>,
    pub state_snapshots: Vec<StateSnapshot>,
    pub beam_width: f64,
    pub portfolio: PortfolioResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capacity_trace: Option<CapacityTrace>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intrinsic_anytime_scheduler_trace: Option<IntrinsicAnytimeSchedulerTrace>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_complete_reconstruction_trace: Option<FocusedCompleteReconstructionTrace>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intrinsic_short_side_observer_trace: Option<IntrinsicShortSideObserverTrace>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intrinsic_short_side_pair_fold_trace: Option<IntrinsicShortSidePairFoldTrace>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Polygon {
    pub points: Vec<Point>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FreeMaterialSheet {
    pub width: f64,
    pub height: f64,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FreeMaterialRegion {
    pub boundary: Polygon,
    pub holes: Vec<Polygon>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FreeMaterialSnapshot {
    pub sheet: FreeMaterialSheet,
    pub regions: Vec<FreeMaterialRegion>,
    pub diagnostics: Vec<ResultDiagnostic>,
}

impl Default for FreeMaterialSnapshot {
    fn default() -> Self {
        Self {
            sheet: FreeMaterialSheet {
                width: 0.0,
                height: 0.0,
                label: String::new(),
            },
            regions: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacementReference {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacementTransform {
    pub translate_x: f64,
    pub translate_y: f64,
    pub rotation_deg: f64,
    pub mirrored: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Placement {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub piece_id: Option<String>,
    pub source_piece_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placement_reference: Option<PlacementReference>,
    pub transform: PlacementTransform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IrregularTransformReason {
    Orthogonal,
    EdgeAlignment,
    Configured,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollisionTransform {
    pub index: f64,
    pub rotation_deg: f64,
    pub mirrored: bool,
    pub reason: IrregularTransformReason,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollisionGeometry {
    pub source_piece_id: String,
    pub transform: CollisionTransform,
    pub polygon: Polygon,
    pub bounds: Bounds,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedCollisionGeometry {
    pub source_piece_id: String,
    pub source_bounds: Bounds,
    pub sampled_points: Vec<Point>,
    pub convex_hull: Polygon,
    pub collision_polygon: Polygon,
    pub placement_reference: Point,
    pub diagnostics: Vec<ResultDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriorityOrderKey {
    pub long_side_mm: f64,
    pub area_mm2: f64,
    pub imbalance_mm: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotPreparedPiece {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub piece_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interchangeability_key: Option<String>,
    pub source: SourcePiece,
    pub allow_mirror: bool,
    pub collision_geometry: PreparedCollisionGeometry,
    pub transforms: Vec<CollisionTransform>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority_order_key: Option<PriorityOrderKey>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacedCollisionGeometry {
    pub placement: Placement,
    pub collision_geometry: CollisionGeometry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultDiagnostic {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub piece_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutScoreSummary {
    pub unplaced_count: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared_collision_boundary_length_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared_collision_boundary_contact_units: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared_collision_boundary_contact_band: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub near_complete_structural_contact_count: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dominant_near_complete_structural_contact_count: Option<f64>,
    pub largest_net_free_material_region_area_mm2: f64,
    pub free_material_region_count: f64,
    pub free_material_hole_count: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_enclosed_cavity_count: Option<f64>,
    pub free_material_sliver_metric: f64,
    pub collision_bounds_worst_normalized_sheet_consumption: f64,
    pub collision_bounds_normalized_span_sum: f64,
    pub collision_bounds_area_mm2: f64,
    pub collision_bounds_span_mm: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutScore {
    pub unplaced_count: f64,
    pub shared_collision_boundary_length_mm: f64,
    pub shared_collision_boundary_contact_units: f64,
    pub shared_collision_boundary_contact_band: f64,
    pub near_complete_structural_contact_count: f64,
    pub dominant_near_complete_structural_contact_count: f64,
    pub largest_net_free_material_region_area_mm2: f64,
    pub free_material_region_count: f64,
    pub free_material_hole_count: f64,
    pub free_material_sliver_metric: f64,
    pub collision_bounds_worst_normalized_sheet_consumption: f64,
    pub collision_bounds_normalized_span_sum: f64,
    pub collision_bounds_area_mm2: f64,
    pub collision_bounds_span_mm: f64,
    pub occupied_hull_waste_ratio: f64,
    pub collision_bounds_bottom_mm: f64,
    pub collision_bounds_left_mm: f64,
    pub free_material_snapshot: FreeMaterialSnapshot,
    pub placement_order: Vec<String>,
    pub unplaced_source_piece_ids: Vec<String>,
}

impl Default for LayoutScore {
    fn default() -> Self {
        Self {
            unplaced_count: 0.0,
            shared_collision_boundary_length_mm: 0.0,
            shared_collision_boundary_contact_units: 0.0,
            shared_collision_boundary_contact_band: 0.0,
            near_complete_structural_contact_count: 0.0,
            dominant_near_complete_structural_contact_count: 0.0,
            largest_net_free_material_region_area_mm2: 0.0,
            free_material_region_count: 0.0,
            free_material_hole_count: 0.0,
            free_material_sliver_metric: 0.0,
            collision_bounds_worst_normalized_sheet_consumption: 0.0,
            collision_bounds_normalized_span_sum: 0.0,
            collision_bounds_area_mm2: 0.0,
            collision_bounds_span_mm: 0.0,
            occupied_hull_waste_ratio: 0.0,
            collision_bounds_bottom_mm: 0.0,
            collision_bounds_left_mm: 0.0,
            free_material_snapshot: FreeMaterialSnapshot::default(),
            placement_order: Vec::new(),
            unplaced_source_piece_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StateSnapshotSource {
    Beam,
    SharedArchive,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateSnapshot {
    pub step_index: f64,
    pub beam_rank: f64,
    pub candidate_count: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<StateSnapshotSource>,
    pub placements: Vec<PlacedCollisionGeometry>,
    pub remaining_prepared_pieces: Vec<SnapshotPreparedPiece>,
    pub unplaced_piece_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortfolioStatus {
    #[default]
    Completed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortfolioTerminationReason {
    CapacitySubsetSettled,
    #[default]
    SharedArchiveCompleted,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SearchSource {
    #[default]
    SharedArchive,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioResult {
    pub status: PortfolioStatus,
    pub termination_reason: PortfolioTerminationReason,
    pub source: SearchSource,
    pub placements: Vec<Placement>,
    pub unplaced_piece_ids: Vec<String>,
    pub score: LayoutScoreSummary,
    pub diagnostics: Vec<ResultDiagnostic>,
}
