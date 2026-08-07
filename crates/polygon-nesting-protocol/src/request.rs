use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{EngineError, EngineOutcome, ExecutionDiagnostics, ProtocolError, ProtocolVersion};

const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

fn default_zero() -> f64 {
    0.0
}

fn default_local_candidate_fanout() -> f64 {
    4.0
}

fn default_transform_minimum_edge_length_mm() -> f64 {
    1.0
}

fn default_transform_angle_deduplication_tolerance_deg() -> f64 {
    0.01
}

fn default_ga_generation_budget() -> f64 {
    2.0
}

fn default_ga_evaluation_budget() -> f64 {
    24.0
}

fn default_placement_policy_ids() -> Vec<PlacementPolicy> {
    vec![
        PlacementPolicy::BalancedCompactness,
        PlacementPolicy::ShortSideFill,
        PlacementPolicy::EdgeContactThenBalancedCompactness,
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EngineProfile {
    Compact,
    CompactShortSide,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticTraceMode {
    #[default]
    Full,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryMode {
    Stream,
    Final,
    Off,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineRequest {
    pub version: ProtocolVersion,
    pub timeout_ms: f64,
    pub profile: EngineProfile,
    pub sheet: SheetSpec,
    pub pieces: Vec<PreparedPiece>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_pieces: Vec<SourcePiece>,
    pub settings: EngineSettings,
    pub history_mode: HistoryMode,
    #[serde(default)]
    pub diagnostic_trace_mode: DiagnosticTraceMode,
}

impl EngineRequest {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.version != ProtocolVersion::CURRENT {
            return Err(ProtocolError::UnsupportedVersion {
                expected: ProtocolVersion::CURRENT,
                received: self.version.get(),
            });
        }

        require_positive_finite("timeoutMs", self.timeout_ms)?;
        require_positive_safe_integer("sheet.width", self.sheet.width)?;
        require_positive_safe_integer("sheet.height", self.sheet.height)?;

        if self.pieces.is_empty() {
            return Err(ProtocolError::validation(
                "pieces",
                "must contain at least one prepared piece",
            ));
        }

        let mut piece_ids = BTreeSet::new();
        for (index, piece) in self.pieces.iter().enumerate() {
            piece.validate(index)?;
            if !piece_ids.insert(piece.id.as_str()) {
                return Err(ProtocolError::validation(
                    format!("pieces[{index}].id"),
                    "must be unique",
                ));
            }
        }

        let mut source_piece_ids = BTreeSet::new();
        for (index, piece) in self.source_pieces.iter().enumerate() {
            piece.validate(index)?;
            if !source_piece_ids.insert(piece.id.as_str()) {
                return Err(ProtocolError::validation(
                    format!("sourcePieces[{index}].id"),
                    "must be unique",
                ));
            }
        }

        self.settings.validate()?;
        Ok(())
    }

    pub fn archive_ineligibility(&self) -> Option<ArchiveIneligibilityReason> {
        let optimizer = &self.settings.optimizer;
        if !optimizer.intrinsic_shared_archive_enabled {
            return Some(ArchiveIneligibilityReason::ArchiveDisabled);
        }
        if optimizer.placement_policy_id == PlacementPolicy::ShortSideFill {
            return Some(ArchiveIneligibilityReason::ShortSideFill);
        }
        if optimizer.ga_active() {
            return Some(ArchiveIneligibilityReason::GaActive);
        }
        None
    }

    pub fn archive_ineligible_outcome(&self) -> Option<EngineOutcome> {
        self.archive_ineligibility()
            .map(|reason| EngineOutcome::ArchiveIneligible {
                reason,
                error: EngineError::archive_ineligible(reason),
                diagnostics: ExecutionDiagnostics::default(),
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArchiveIneligibilityReason {
    ArchiveDisabled,
    ShortSideFill,
    GaActive,
}

impl ArchiveIneligibilityReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ArchiveDisabled => "archive-disabled",
            Self::ShortSideFill => "short-side-fill",
            Self::GaActive => "ga-active",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetSpec {
    pub width: f64,
    pub height: f64,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RectWithMetrics {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub longest_edge: f64,
    pub area: f64,
    pub imbalance: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CutRowReference {
    pub reference: String,
    pub customer_name: String,
    pub csv_row_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedPiece {
    pub id: String,
    pub source_piece_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interchangeability_key: Option<String>,
    pub real_bounds: Rect,
    pub padded_bounds: RectWithMetrics,
    pub padding: f64,
    pub allow_rotation: bool,
    #[serde(default = "default_true")]
    pub allow_mirror: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cut_row_ref: Option<CutRowReference>,
}

impl PreparedPiece {
    fn validate(&self, index: usize) -> Result<(), ProtocolError> {
        let prefix = format!("pieces[{index}]");
        if self.id.trim().is_empty() {
            return Err(ProtocolError::validation(
                format!("{prefix}.id"),
                "must be a non-empty string",
            ));
        }
        require_non_empty(&format!("{prefix}.sourcePieceId"), &self.source_piece_id)?;
        if let Some(key) = &self.interchangeability_key {
            require_non_empty(&format!("{prefix}.interchangeabilityKey"), key)?;
        }
        require_non_negative_safe_integer(&format!("{prefix}.padding"), self.padding)?;

        for (name, value) in [
            ("realBounds.x", self.real_bounds.x),
            ("realBounds.y", self.real_bounds.y),
            ("paddedBounds.x", self.padded_bounds.x),
            ("paddedBounds.y", self.padded_bounds.y),
            ("paddedBounds.imbalance", self.padded_bounds.imbalance),
        ] {
            require_non_negative_safe_integer(&format!("{prefix}.{name}"), value)?;
        }
        for (name, value) in [
            ("realBounds.width", self.real_bounds.width),
            ("realBounds.height", self.real_bounds.height),
            ("paddedBounds.width", self.padded_bounds.width),
            ("paddedBounds.height", self.padded_bounds.height),
            ("paddedBounds.longestEdge", self.padded_bounds.longest_edge),
            ("paddedBounds.area", self.padded_bounds.area),
        ] {
            require_positive_safe_integer(&format!("{prefix}.{name}"), value)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcePiece {
    pub id: String,
    pub source_file_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_layer: Option<String>,
    pub label: String,
    pub real_bounds: Rect,
    pub geometry: SourceGeometry,
    pub warnings: Vec<SourceWarning>,
}

impl SourcePiece {
    fn validate(&self, index: usize) -> Result<(), ProtocolError> {
        let prefix = format!("sourcePieces[{index}]");
        require_non_empty(&format!("{prefix}.id"), &self.id)?;
        require_non_empty(&format!("{prefix}.sourceFileId"), &self.source_file_id)?;
        for (name, value) in [
            ("realBounds.x", self.real_bounds.x),
            ("realBounds.y", self.real_bounds.y),
        ] {
            require_non_negative_safe_integer(&format!("{prefix}.{name}"), value)?;
        }
        for (name, value) in [
            ("realBounds.width", self.real_bounds.width),
            ("realBounds.height", self.real_bounds.height),
        ] {
            require_positive_safe_integer(&format!("{prefix}.{name}"), value)?;
        }
        self.geometry.validate(&format!("{prefix}.geometry"))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SourceEntityHandle {
    Text(String),
    Number(f64),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceWarning {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_handle: Option<SourceEntityHandle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceGeometryEntityType {
    #[serde(rename = "LINE")]
    Line,
    #[serde(rename = "LWPOLYLINE")]
    Lwpolyline,
    #[serde(rename = "POLYLINE")]
    Polyline,
    #[serde(rename = "CIRCLE")]
    Circle,
    #[serde(rename = "ARC")]
    Arc,
    #[serde(rename = "ELLIPSE")]
    Ellipse,
    #[serde(rename = "DXF_SHAPE")]
    DxfShape,
    #[serde(rename = "PRESET_SHAPE")]
    PresetShape,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceGeometry {
    pub entity_type: SourceGeometryEntityType,
    pub closed: bool,
    pub segments: Vec<SourceGeometrySegment>,
}

impl SourceGeometry {
    fn validate(&self, prefix: &str) -> Result<(), ProtocolError> {
        for (index, segment) in self.segments.iter().enumerate() {
            segment.validate(&format!("{prefix}.segments[{index}]"))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SourceGeometrySegment {
    Line(SourceLineSegment),
    Arc(SourceArcSegment),
}

impl SourceGeometrySegment {
    fn validate(&self, prefix: &str) -> Result<(), ProtocolError> {
        match self {
            Self::Line(line) => line.validate(prefix),
            Self::Arc(arc) => arc.validate(prefix),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceLineSegment {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bulge: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_curve: Option<EllipseSource>,
}

impl SourceLineSegment {
    fn validate(&self, prefix: &str) -> Result<(), ProtocolError> {
        for (name, value) in [
            ("x1", self.x1),
            ("y1", self.y1),
            ("x2", self.x2),
            ("y2", self.y2),
        ] {
            require_finite(&format!("{prefix}.{name}"), value)?;
        }
        if let Some(bulge) = self.bulge {
            require_finite(&format!("{prefix}.bulge"), bulge)?;
        }
        if let Some(curve) = &self.source_curve {
            curve.validate(&format!("{prefix}.sourceCurve"))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceArcSegment {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub cx: f64,
    pub cy: f64,
    pub radius: f64,
    pub start_angle: f64,
    pub end_angle: f64,
}

impl SourceArcSegment {
    fn validate(&self, prefix: &str) -> Result<(), ProtocolError> {
        for (name, value) in [
            ("x1", self.x1),
            ("y1", self.y1),
            ("x2", self.x2),
            ("y2", self.y2),
            ("cx", self.cx),
            ("cy", self.cy),
            ("startAngle", self.start_angle),
            ("endAngle", self.end_angle),
        ] {
            require_finite(&format!("{prefix}.{name}"), value)?;
        }
        require_positive_finite(&format!("{prefix}.radius"), self.radius)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EllipseSourceKind {
    #[serde(rename = "ellipse")]
    Ellipse,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EllipseSource {
    pub kind: EllipseSourceKind,
    pub source_id: String,
    pub cx: f64,
    pub cy: f64,
    pub major_axis_x: f64,
    pub major_axis_y: f64,
    pub axis_ratio: f64,
    pub start_angle: f64,
    pub end_angle: f64,
}

impl EllipseSource {
    fn validate(&self, prefix: &str) -> Result<(), ProtocolError> {
        require_non_empty(&format!("{prefix}.sourceId"), &self.source_id)?;
        for (name, value) in [
            ("cx", self.cx),
            ("cy", self.cy),
            ("majorAxisX", self.major_axis_x),
            ("majorAxisY", self.major_axis_y),
            ("startAngle", self.start_angle),
            ("endAngle", self.end_angle),
        ] {
            require_finite(&format!("{prefix}.{name}"), value)?;
        }
        require_positive_finite(&format!("{prefix}.axisRatio"), self.axis_ratio)?;
        if self.major_axis_x == 0.0 && self.major_axis_y == 0.0 {
            return Err(ProtocolError::validation(
                format!("{prefix}.majorAxis"),
                "must have a non-zero axis vector",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineSettings {
    pub padding: f64,
    pub allow_global_rotation: bool,
    #[serde(default = "default_true")]
    pub allow_global_mirror: bool,
    pub geometry: GeometrySettings,
    pub optimizer: OptimizerSettings,
}

impl EngineSettings {
    fn validate(&self) -> Result<(), ProtocolError> {
        require_non_negative_safe_integer("settings.padding", self.padding)?;
        self.geometry.validate()?;
        self.optimizer.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeometrySettings {
    pub flattening_sag_tolerance_mm: f64,
    pub clearance_safety_margin_mm: f64,
    pub geometry_backend_id: String,
    pub geometry_backend_version: String,
}

impl GeometrySettings {
    fn validate(&self) -> Result<(), ProtocolError> {
        require_positive_finite(
            "settings.geometry.flatteningSagToleranceMm",
            self.flattening_sag_tolerance_mm,
        )?;
        require_non_negative_finite(
            "settings.geometry.clearanceSafetyMarginMm",
            self.clearance_safety_margin_mm,
        )?;
        require_non_empty(
            "settings.geometry.geometryBackendId",
            &self.geometry_backend_id,
        )?;
        require_non_empty(
            "settings.geometry.geometryBackendVersion",
            &self.geometry_backend_version,
        )?;
        if self.clearance_safety_margin_mm < self.flattening_sag_tolerance_mm {
            return Err(ProtocolError::validation(
                "settings.geometry.clearanceSafetyMarginMm",
                "must be greater than or equal to flatteningSagToleranceMm",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlacementPolicy {
    #[default]
    BalancedCompactness,
    ShortSideFill,
    EdgeContactThenBalancedCompactness,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizerSettings {
    pub order_window: f64,
    pub beam_width: f64,
    #[serde(default = "default_local_candidate_fanout")]
    pub local_candidate_fanout: f64,
    #[serde(default = "default_zero")]
    pub local_repair_budget: f64,
    #[serde(default = "default_false")]
    pub intrinsic_shared_archive_enabled: bool,
    pub transform_cap: f64,
    #[serde(default = "default_transform_minimum_edge_length_mm")]
    pub transform_minimum_edge_length_mm: f64,
    #[serde(default = "default_transform_angle_deduplication_tolerance_deg")]
    pub transform_angle_deduplication_tolerance_deg: f64,
    #[serde(default = "default_true")]
    pub configured_rotation_enabled: bool,
    #[serde(default = "default_true")]
    pub edge_alignment_enabled: bool,
    #[serde(default)]
    pub configured_rotation_deg: Vec<f64>,
    #[serde(default = "default_false")]
    pub ga_enabled: bool,
    #[serde(default = "default_true")]
    pub baseline_only: bool,
    pub ga_population: f64,
    #[serde(default = "default_ga_generation_budget")]
    pub ga_generation_budget: f64,
    #[serde(default = "default_ga_evaluation_budget")]
    pub ga_evaluation_budget: f64,
    pub ga_time_budget_ms: f64,
    pub ga_seed: String,
    #[serde(default = "default_true")]
    pub priority_order_mutation_enabled: bool,
    #[serde(default = "default_true")]
    pub transform_preference_mutation_enabled: bool,
    #[serde(default = "default_true")]
    pub placement_policy_mutation_enabled: bool,
    #[serde(default)]
    pub placement_policy_id: PlacementPolicy,
    #[serde(default = "default_placement_policy_ids")]
    pub placement_policy_ids: Vec<PlacementPolicy>,
}

impl OptimizerSettings {
    fn ga_active(&self) -> bool {
        self.ga_enabled
            && !self.baseline_only
            && self.ga_time_budget_ms != 0.0
            && self.ga_generation_budget != 0.0
            && self.ga_evaluation_budget != 0.0
    }

    fn validate(&self) -> Result<(), ProtocolError> {
        for (name, value) in [
            ("orderWindow", self.order_window),
            ("beamWidth", self.beam_width),
            ("localCandidateFanout", self.local_candidate_fanout),
            ("transformCap", self.transform_cap),
            ("gaPopulation", self.ga_population),
        ] {
            require_positive_safe_integer(&format!("settings.optimizer.{name}"), value)?;
        }
        for (name, value) in [
            ("localRepairBudget", self.local_repair_budget),
            ("gaGenerationBudget", self.ga_generation_budget),
            ("gaEvaluationBudget", self.ga_evaluation_budget),
            ("gaTimeBudgetMs", self.ga_time_budget_ms),
        ] {
            require_non_negative_safe_integer(&format!("settings.optimizer.{name}"), value)?;
        }
        require_non_negative_finite(
            "settings.optimizer.transformMinimumEdgeLengthMm",
            self.transform_minimum_edge_length_mm,
        )?;
        require_positive_finite(
            "settings.optimizer.transformAngleDeduplicationToleranceDeg",
            self.transform_angle_deduplication_tolerance_deg,
        )?;
        for (index, value) in self.configured_rotation_deg.iter().enumerate() {
            require_finite(
                &format!("settings.optimizer.configuredRotationDeg[{index}]"),
                *value,
            )?;
        }
        require_non_empty("settings.optimizer.gaSeed", &self.ga_seed)?;

        if self.placement_policy_ids.is_empty() {
            return Err(ProtocolError::validation(
                "settings.optimizer.placementPolicyIds",
                "must not be empty",
            ));
        }
        if !self
            .placement_policy_ids
            .contains(&self.placement_policy_id)
        {
            return Err(ProtocolError::validation(
                "settings.optimizer.placementPolicyId",
                "must be a member of placementPolicyIds",
            ));
        }
        let unique: BTreeSet<_> = self.placement_policy_ids.iter().copied().collect();
        if unique.len() != self.placement_policy_ids.len() {
            return Err(ProtocolError::validation(
                "settings.optimizer.placementPolicyIds",
                "must not contain duplicates",
            ));
        }
        Ok(())
    }
}

fn require_non_empty(field: &str, value: &str) -> Result<(), ProtocolError> {
    if value.is_empty() {
        return Err(ProtocolError::validation(field, "must be non-empty"));
    }
    Ok(())
}

fn require_finite(field: &str, value: f64) -> Result<(), ProtocolError> {
    if !value.is_finite() {
        return Err(ProtocolError::validation(field, "must be finite"));
    }
    Ok(())
}

fn require_positive_finite(field: &str, value: f64) -> Result<(), ProtocolError> {
    require_finite(field, value)?;
    if value <= 0.0 {
        return Err(ProtocolError::validation(
            field,
            "must be greater than zero",
        ));
    }
    Ok(())
}

fn require_non_negative_finite(field: &str, value: f64) -> Result<(), ProtocolError> {
    require_finite(field, value)?;
    if value < 0.0 {
        return Err(ProtocolError::validation(field, "must be non-negative"));
    }
    Ok(())
}

fn require_safe_integer(field: &str, value: f64) -> Result<(), ProtocolError> {
    require_finite(field, value)?;
    if value.fract() != 0.0 || value.abs() > MAX_SAFE_INTEGER {
        return Err(ProtocolError::validation(
            field,
            "must be a JavaScript safe integer",
        ));
    }
    Ok(())
}

fn require_positive_safe_integer(field: &str, value: f64) -> Result<(), ProtocolError> {
    require_safe_integer(field, value)?;
    if value <= 0.0 {
        return Err(ProtocolError::validation(
            field,
            "must be greater than zero",
        ));
    }
    Ok(())
}

fn require_non_negative_safe_integer(field: &str, value: f64) -> Result<(), ProtocolError> {
    require_safe_integer(field, value)?;
    if value < 0.0 {
        return Err(ProtocolError::validation(field, "must be non-negative"));
    }
    Ok(())
}
