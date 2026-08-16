#![recursion_limit = "512"]

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use polygon_nesting_core::domain::ImportedPiece;
use polygon_nesting_core::geometry::general_polygon::PolygonSet;
use polygon_nesting_core::geometry::general_source::polygon_set_from_imported_piece;
use polygon_nesting_core::parallel::JobPool;
use polygon_nesting_core::search::general_fast::{
    construct_short_side_first, diagnose_congruent_pair_constructor,
    diagnose_congruent_pair_templates, GeneralFastPiece, GeneralFastSettings,
    GeneralPairClusterArmDiagnostics,
};
use polygon_nesting_core::search::general_relaxed::{
    improve_complete_layout, improve_complete_layout_with_persistent_vacancy_parent,
    GeneralAngularRepairSettings, GeneralCoupledSeparatorPlacementDiagnostics,
    GeneralRelaxedAngleSeedPolicy, GeneralRelaxedCollisionBackend, GeneralRelaxedDiagnostics,
    GeneralRelaxedPressureModel, GeneralRelaxedSettings,
};
use serde::ser::{SerializeSeq, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

const PERSISTENT_VACANCY_PARENT_FIXTURE_SHA256: &str =
    "18e0b052997d1251573fa35679c9fcf1d5e796acf771ec48f320ce4e9bf0081d";
const VACANCY_ARTICULATION_JSON_CAP_BYTES: usize = 4 * 1024 * 1024;
const VACANCY_BRIDGE_JSON_CAP_BYTES: usize = 4 * 1024 * 1024;

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
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
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
struct PersistentVacancyParentFixture {
    schema_version: u32,
    request_sha256: String,
    settings: PersistentVacancyParentSettings,
    placements: Vec<GeneralCoupledSeparatorPlacementDiagnostics>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct PersistentVacancyParentSettings {
    sheet_short_axis_mm: f64,
    sheet_long_axis_mm: f64,
    total_padding_mm: f64,
    sheet_edge_clearance_mm: Option<f64>,
    clearance_safety_margin_mm: f64,
    flattening_sag_tolerance_mm: f64,
}

impl From<GeneralFastSettings> for PersistentVacancyParentSettings {
    fn from(settings: GeneralFastSettings) -> Self {
        Self {
            sheet_short_axis_mm: settings.sheet_short_axis_mm,
            sheet_long_axis_mm: settings.sheet_long_axis_mm,
            total_padding_mm: settings.total_padding_mm,
            sheet_edge_clearance_mm: settings.sheet_edge_clearance_mm,
            clearance_safety_margin_mm: settings.clearance_safety_margin_mm,
            flattening_sag_tolerance_mm: settings.flattening_sag_tolerance_mm,
        }
    }
}

struct LoadedPersistentVacancyParent {
    schema_version: u32,
    sha256: String,
    settings: PersistentVacancyParentSettings,
    placements: Vec<GeneralCoupledSeparatorPlacementDiagnostics>,
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
    #[serde(default)]
    padding: f64,
    allow_rotation: bool,
    #[serde(default = "default_true")]
    allow_mirror: bool,
}

struct OwnedPiece {
    id: String,
    polygon: PolygonSet,
    allow_rotation: bool,
    allow_mirror: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PairTemplateProbeOutput {
    elapsed_ms: f64,
    eligible_pairs: usize,
    pairs_with_templates: usize,
    fallback_pairs: usize,
    orientation_tuples: usize,
    contact_attempts: usize,
    exact_pair_rows: usize,
    retained_templates: usize,
    transformed_source_vertices: usize,
    offset_output_vertices: usize,
    intersection_input_vertices: usize,
    intersection_output_vertices: usize,
    transient_rejected_output_vertices: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PairTemplateSummaryOutput {
    eligible_pairs: usize,
    pairs_with_templates: usize,
    fallback_pairs: usize,
    retained_templates: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PairClusterArmOutput {
    placed: Option<usize>,
    used_long_axis_depth_mm: Option<f64>,
    band_variants_attempted: usize,
    completed_bands: usize,
    band_failures: Vec<String>,
    proposal_attempts: usize,
    generated_proposals: usize,
    exact_child_fixed_visits: usize,
    exact_candidate_rows: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PairConstructorProbeOutput {
    elapsed_ms: f64,
    templates: PairTemplateSummaryOutput,
    control: PairClusterArmOutput,
    treatment: PairClusterArmOutput,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct QuotaOutput {
    order_variants: usize,
    exploratory_evaluations_per_piece: usize,
    repair_targets: usize,
    repair_evaluations_per_piece: usize,
    local_angle_evaluations_per_piece: usize,
    catalog_variants: usize,
    catalog_evaluations_per_piece: usize,
    pairing_evaluations_per_piece: usize,
    pairing_band_variants: usize,
    partial_layouts: usize,
    beam_evaluations_per_state: usize,
    angle_seed_count: usize,
    max_angles_per_piece: usize,
    tightening_passes: usize,
    relaxed_epochs: usize,
    relaxed_lanes: usize,
    relaxed_sweeps_per_epoch: usize,
    relaxed_global_samples_per_move: usize,
    relaxed_focused_samples_per_move: usize,
    relaxed_refinement_rounds: usize,
    relaxed_seed: u64,
    relaxed_initial_shrink_ratio: f64,
    relaxed_minimum_shrink_ratio: f64,
    relaxed_failed_attempts_per_depth: usize,
    relaxed_infeasible_pool_size: usize,
    relaxed_infeasible_pool_arguments_ignored: bool,
    relaxed_synchronize_lanes: bool,
    relaxed_dynamic_hazard: bool,
    relaxed_angle_seed_policy: &'static str,
    relaxed_pressure_model: &'static str,
    relaxed_angular_repair: bool,
    relaxed_repair_neighborhood: usize,
    pair_template_diagnostics: bool,
    pair_constructor_diagnostics: bool,
    precompression_frontier_vacancy_mode: usize,
    exact_pair_terminal_mode: usize,
    persistent_vacancy_mode: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ParentFixtureOutput<'a> {
    schema_version: u32,
    sha256: &'a str,
    placement_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlacementOutput<'a> {
    piece_id: &'a str,
    rotation_deg: f64,
    mirrored: bool,
    translate_short_axis: f64,
    translate_long_axis: f64,
}

struct PlacementList<'a>(&'a [polygon_nesting_core::search::general_fast::GeneralFastPlacement]);

impl Serialize for PlacementList<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for placement in self.0 {
            sequence.serialize_element(&PlacementOutput {
                piece_id: &placement.piece_id,
                rotation_deg: placement.rotation_deg,
                mirrored: placement.mirrored,
                translate_short_axis: placement.translate_short_axis,
                translate_long_axis: placement.translate_long_axis,
            })?;
        }
        sequence.end()
    }
}

struct MetadataKeys<'a>(&'a BTreeMap<String, serde_json::Value>);

impl Serialize for MetadataKeys<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for key in self.0.keys() {
            sequence.serialize_element(key)?;
        }
        sequence.end()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Mode25Output<'a> {
    request: &'a str,
    request_sha256: &'a str,
    engine_commit: &'a Option<String>,
    engine_worktree_dirty: &'a Option<bool>,
    engine_worktree_status: &'a Option<String>,
    executable_sha256: &'a Option<String>,
    relevant_source_tree_sha256: &'a Option<String>,
    profile: &'static str,
    build_profile: &'static str,
    target_architecture: &'static str,
    target_operating_system: &'static str,
    machine_architecture: &'a Option<String>,
    cpu_model: &'a Option<String>,
    rustc_version: &'a Option<String>,
    rustflags: &'a Option<String>,
    budget_mode: &'static str,
    seed: (),
    piece_count: usize,
    source_piece_count: usize,
    total_vertices: usize,
    concave_piece_count: usize,
    sheet_short_axis_mm: f64,
    sheet_long_axis_mm: f64,
    request_total_padding_mm: f64,
    pair_clearance_mm: f64,
    sheet_edge_clearance_mm: f64,
    flattening_sag_tolerance_mm: f64,
    clearance_safety_margin_mm: f64,
    requested_threads: usize,
    actual_threads: usize,
    quota: &'a QuotaOutput,
    pair_template_probe: &'a Option<PairTemplateProbeOutput>,
    pair_constructor_probe: &'a Option<PairConstructorProbeOutput>,
    relaxed_diagnostics: Option<GeneralRelaxedDiagnostics>,
    placed: usize,
    unplaced: usize,
    constructed_long_axis_depth_mm: Option<f64>,
    used_long_axis_depth_mm: f64,
    independent_used_long_axis_depth_mm: f64,
    coupled_treatment_independent_used_long_axis_depth_mm: Option<f64>,
    placed_material_area_mm2: f64,
    expanded_collision_area_mm2: f64,
    area_lower_bound_depth_mm: f64,
    depth_over_area_lower_bound: f64,
    used_strip_area_mm2: f64,
    used_strip_utilization_percent: f64,
    exact_evaluations: usize,
    primary_exact_evaluations: usize,
    order_portfolio_exact_evaluations: usize,
    catalog_portfolio_exact_evaluations: usize,
    pairing_exact_evaluations: usize,
    beam_exact_evaluations: usize,
    tightening_exact_evaluations: usize,
    tightening_passes_attempted: usize,
    tightening_passes_improved: usize,
    catalog_candidate_placed_count: Option<usize>,
    catalog_candidate_depth_mm: Option<f64>,
    pairing_candidate_placed_count: Option<usize>,
    pairing_candidate_depth_mm: Option<f64>,
    beam_candidate_placed_count: Option<usize>,
    beam_candidate_depth_mm: Option<f64>,
    exploratory_exact_evaluations: usize,
    repair_exact_evaluations: usize,
    local_angle_refinement_exact_evaluations: usize,
    order_variants_attempted: usize,
    catalog_variants_attempted: usize,
    repair_targets_considered: usize,
    order_portfolio_failed: bool,
    catalog_portfolio_failed: bool,
    pairing_failed: bool,
    beam_failed: bool,
    exploratory_failed: bool,
    repair_failed: bool,
    median_elapsed_ms: f64,
    first_quartile_elapsed_ms: f64,
    third_quartile_elapsed_ms: f64,
    interquartile_range_elapsed_ms: f64,
    min_elapsed_ms: f64,
    max_elapsed_ms: f64,
    elapsed_ms: &'a [f64],
    ignored_request_metadata_fields: MetadataKeys<'a>,
    placements: PlacementList<'a>,
    persistent_vacancy_parent_fixture: Option<ParentFixtureOutput<'a>>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args().skip(1);
    let request_path = arguments.next().ok_or(
        "usage: general_request_benchmark REQUEST.json [runs] [order-variants] [exploratory-evaluations-per-piece] [repair-targets] [repair-evaluations-per-piece] [local-angle-evaluations-per-piece] [catalog-variants] [catalog-evaluations-per-piece] [pairing-evaluations-per-piece] [pairing-band-variants] [partial-layouts] [beam-evaluations-per-state] [angle-seed-count] [max-angles-per-piece] [threads] [sheet-long-axis-override-mm] [tightening-passes] [sheet-edge-clearance-mm] [pair-clearance-mm] [relaxed-epochs] [relaxed-lanes] [relaxed-sweeps] [relaxed-global-samples] [relaxed-focused-samples] [relaxed-refinement-rounds] [relaxed-seed] [relaxed-initial-shrink-ratio] [relaxed-minimum-shrink-ratio] [relaxed-failed-attempts-per-depth] [relaxed-infeasible-pool-size] [relaxed-synchronize-lanes] [relaxed-dynamic-hazard] [relaxed-continuous-seeds] [relaxed-pressure-model] [relaxed-angular-repair] [relaxed-repair-neighborhood] [coupled-dynamic-separator] [pair-template-diagnostics] [pair-constructor-diagnostics] [precompression-frontier-vacancy] [exact-pair-terminal] [persistent-vacancy] [persistent-vacancy-parent.json (required iff persistent-vacancy > 0)]",
    )?;
    let runs = parse_optional(&mut arguments, 1)?;
    let order_variants = parse_optional(&mut arguments, 1)?;
    let exploratory_evaluations = parse_optional(&mut arguments, 0)?;
    let repair_targets = parse_optional(&mut arguments, 0)?;
    let repair_evaluations = parse_optional(&mut arguments, 0)?;
    let local_angle_evaluations = parse_optional(&mut arguments, 0)?;
    let catalog_variants = parse_optional(&mut arguments, 1)?;
    let catalog_evaluations = parse_optional(&mut arguments, 0)?;
    let pairing_evaluations = parse_optional(&mut arguments, 0)?;
    let pairing_band_variants = parse_optional(&mut arguments, 1)?;
    let partial_layouts = parse_optional(&mut arguments, 1)?;
    let beam_evaluations = parse_optional(&mut arguments, 0)?;
    let angle_seed_count = parse_optional(&mut arguments, 4)?;
    let max_angles_per_piece = parse_optional(&mut arguments, 8)?;
    let threads = parse_optional(&mut arguments, 1)?;
    let sheet_long_axis_override_mm = parse_optional_f64(&mut arguments, 0.0)?;
    let tightening_passes = parse_optional(&mut arguments, 0)?;
    let sheet_edge_clearance_mm = arguments
        .next()
        .map(|value| value.parse::<f64>())
        .transpose()?;
    let pair_clearance_mm = arguments
        .next()
        .map(|value| value.parse::<f64>())
        .transpose()?;
    let relaxed_epochs = parse_optional(&mut arguments, 0)?;
    let relaxed_lanes = parse_optional(&mut arguments, threads)?;
    let relaxed_sweeps = parse_optional(&mut arguments, 12)?;
    let relaxed_global_samples = parse_optional(&mut arguments, 36)?;
    let relaxed_focused_samples = parse_optional(&mut arguments, 36)?;
    let relaxed_refinement_rounds = parse_optional(&mut arguments, 3)?;
    let relaxed_seed = arguments
        .next()
        .map(|value| value.parse::<u64>())
        .transpose()?
        .unwrap_or(0);
    let relaxed_initial_shrink_ratio = parse_optional_f64(&mut arguments, 0.02)?;
    let relaxed_minimum_shrink_ratio = parse_optional_f64(&mut arguments, 0.001)?;
    let relaxed_failed_attempts_per_depth = parse_optional(&mut arguments, 1)?;
    let relaxed_infeasible_pool_size = parse_optional(&mut arguments, 6)?;
    let relaxed_synchronize_lanes = parse_optional(&mut arguments, 0)? != 0;
    let relaxed_dynamic_hazard = parse_optional(&mut arguments, 0)? != 0;
    let relaxed_continuous_seeds = parse_optional(&mut arguments, 0)? != 0;
    let relaxed_pressure_model = parse_optional_pressure_model(
        &mut arguments,
        GeneralRelaxedPressureModel::StructuredTrianglePoles,
    )?;
    let relaxed_angular_repair = parse_optional(&mut arguments, 0)? != 0;
    let relaxed_repair_neighborhood = parse_optional(&mut arguments, 10)?;
    let coupled_dynamic_separator = parse_optional(&mut arguments, 0)? != 0;
    let pair_template_diagnostics = parse_optional(&mut arguments, 0)? != 0;
    let pair_constructor_diagnostics = parse_optional(&mut arguments, 0)? != 0;
    let precompression_frontier_vacancy_mode = parse_optional(&mut arguments, 0)?;
    if precompression_frontier_vacancy_mode > 3 {
        return Err("precompression frontier vacancy mode must be 0, 1, 2, or 3".into());
    }
    let retired_exact_pair_terminal_mode = parse_optional(&mut arguments, 0)?;
    if retired_exact_pair_terminal_mode != 0 {
        return Err("exact pair terminal diagnostics have been retired; mode must be 0".into());
    }
    let persistent_vacancy_mode = parse_optional(&mut arguments, 0)?;
    if !matches!(persistent_vacancy_mode, 0..=6 | 8 | 9 | 10 | 14..=19 | 25 | 26) {
        return Err(
            "persistent vacancy mode must be 0 through 6, 8, 9, 10, 14 through 19, or 25 through 26; retired modes 7 and 11 through 13 are unavailable"
                .into(),
        );
    }
    let persistent_vacancy_parent_path = arguments.next();
    if runs == 0 || arguments.next().is_some() {
        return Err("runs must be positive and no extra arguments are accepted".into());
    }
    if persistent_vacancy_mode > 0 && persistent_vacancy_parent_path.is_none() {
        return Err(
            "a persistent-vacancy parent fixture path is required when persistent vacancy mode is nonzero"
                .into(),
        );
    }
    if persistent_vacancy_mode == 0 && persistent_vacancy_parent_path.is_some() {
        return Err(
            "a persistent-vacancy parent fixture path is accepted only when persistent vacancy mode is nonzero"
                .into(),
        );
    }

    let bytes = fs::read(Path::new(&request_path))?;
    let request_sha256 = format!("{:x}", Sha256::digest(&bytes));
    let request: Request = serde_json::from_slice(&bytes)?;
    let persistent_vacancy_parent = persistent_vacancy_parent_path
        .map(|path| {
            let fixture_bytes = fs::read(Path::new(&path))?;
            let fixture_sha256 = format!("{:x}", Sha256::digest(&fixture_bytes));
            if fixture_sha256 != PERSISTENT_VACANCY_PARENT_FIXTURE_SHA256 {
                return Err(format!(
                    "persistent-vacancy parent fixture hash mismatch: expected {PERSISTENT_VACANCY_PARENT_FIXTURE_SHA256}, got {fixture_sha256}"
                )
                .into());
            }
            let fixture: PersistentVacancyParentFixture =
                serde_json::from_slice(&fixture_bytes)?;
            if fixture.schema_version != 1 {
                return Err(format!(
                    "persistent-vacancy parent fixture schema must be 1, got {}",
                    fixture.schema_version
                )
                .into());
            }
            if fixture.request_sha256 != request_sha256 {
                return Err(format!(
                    "persistent-vacancy parent fixture request mismatch: expected {request_sha256}, got {}",
                    fixture.request_sha256
                )
                .into());
            }
            Ok::<_, Box<dyn std::error::Error>>(LoadedPersistentVacancyParent {
                schema_version: fixture.schema_version,
                sha256: fixture_sha256,
                settings: fixture.settings,
                placements: fixture.placements,
            })
        })
        .transpose()?;
    let (request_total_padding_mm, allow_global_rotation, allow_global_mirror, geometry) =
        effective_request_settings(&request)?;
    let total_padding_mm = pair_clearance_mm.unwrap_or(request_total_padding_mm);
    let flattening_sag_tolerance_mm = geometry.flattening_sag_tolerance_mm;
    let clearance_safety_margin_mm = geometry.clearance_safety_margin_mm;
    if pair_clearance_mm.is_none()
        && request
            .pieces
            .iter()
            .any(|piece| (piece.padding * 2.0 - total_padding_mm).abs() > f64::EPSILON)
    {
        return Err("the internal benchmark requires one total padding value".into());
    }
    let source_by_id = unique_sources(&request.source_pieces)?;
    reject_duplicate_piece_ids(&request.pieces)?;
    let normalize_axes = request.sheet.width >= request.sheet.height;
    let owned = request
        .pieces
        .iter()
        .map(|piece| {
            let source = source_by_id
                .get(piece.source_piece_id.as_str())
                .ok_or_else(|| format!("missing source piece {}", piece.source_piece_id))?;
            Ok(OwnedPiece {
                id: piece.id.clone(),
                polygon: normalize_polygon_axes(
                    polygon_set_from_imported_piece(source, flattening_sag_tolerance_mm)?,
                    normalize_axes,
                )?,
                allow_rotation: allow_global_rotation && piece.allow_rotation,
                allow_mirror: allow_global_mirror && piece.allow_mirror,
            })
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    let pieces = owned
        .iter()
        .map(|piece| GeneralFastPiece {
            id: &piece.id,
            polygon: &piece.polygon,
            allow_rotation: piece.allow_rotation,
            allow_mirror: piece.allow_mirror,
        })
        .collect::<Vec<_>>();
    let mut settings = GeneralFastSettings::deterministic_test(
        request.sheet.width.min(request.sheet.height),
        request.sheet.width.max(request.sheet.height),
    );
    settings.total_padding_mm = total_padding_mm;
    settings.sheet_edge_clearance_mm = sheet_edge_clearance_mm;
    settings.clearance_safety_margin_mm = clearance_safety_margin_mm;
    settings.flattening_sag_tolerance_mm = flattening_sag_tolerance_mm;
    settings.angle_seed_count = angle_seed_count;
    settings.max_angles_per_piece = max_angles_per_piece;
    settings.max_order_variants = order_variants;
    settings.max_catalog_variants = catalog_variants;
    settings.max_catalog_evaluations_per_piece = catalog_evaluations;
    settings.max_pairing_evaluations_per_piece = pairing_evaluations;
    settings.max_pairing_band_variants = pairing_band_variants;
    settings.max_partial_layouts = partial_layouts;
    settings.max_beam_evaluations_per_state = beam_evaluations;
    settings.max_tightening_passes = tightening_passes;
    settings.max_exploratory_evaluations_per_piece = exploratory_evaluations;
    settings.max_repair_targets = repair_targets;
    settings.max_repair_evaluations_per_piece = repair_evaluations;
    settings.max_local_angle_refinement_evaluations_per_piece = local_angle_evaluations;
    if sheet_long_axis_override_mm > 0.0 {
        settings.sheet_long_axis_mm = sheet_long_axis_override_mm;
    }
    if let Some(parent) = &persistent_vacancy_parent {
        let effective_settings = PersistentVacancyParentSettings::from(settings);
        if parent.settings != effective_settings {
            return Err(format!(
                "persistent-vacancy parent fixture settings mismatch: expected {:?}, got {:?}",
                parent.settings, effective_settings
            )
            .into());
        }
    }
    let pair_template_probe = if pair_template_diagnostics {
        let started = Instant::now();
        let diagnostics = diagnose_congruent_pair_templates(&pieces, settings)?;
        Some(PairTemplateProbeOutput {
            elapsed_ms: started.elapsed().as_secs_f64() * 1_000.0,
            eligible_pairs: diagnostics.eligible_pairs,
            pairs_with_templates: diagnostics.pairs_with_templates,
            fallback_pairs: diagnostics.fallback_pairs,
            orientation_tuples: diagnostics.orientation_tuples,
            contact_attempts: diagnostics.contact_attempts,
            exact_pair_rows: diagnostics.exact_pair_rows,
            retained_templates: diagnostics.retained_templates,
            transformed_source_vertices: diagnostics.transformed_source_vertices,
            offset_output_vertices: diagnostics.offset_output_vertices,
            intersection_input_vertices: diagnostics.intersection_input_vertices,
            intersection_output_vertices: diagnostics.intersection_output_vertices,
            transient_rejected_output_vertices: diagnostics.transient_rejected_output_vertices,
        })
    } else {
        None
    };
    let pair_constructor_probe = if pair_constructor_diagnostics {
        let started = Instant::now();
        let experiment = diagnose_congruent_pair_constructor(&pieces, settings)?;
        Some(PairConstructorProbeOutput {
            elapsed_ms: started.elapsed().as_secs_f64() * 1_000.0,
            templates: PairTemplateSummaryOutput {
                eligible_pairs: experiment.templates.eligible_pairs,
                pairs_with_templates: experiment.templates.pairs_with_templates,
                fallback_pairs: experiment.templates.fallback_pairs,
                retained_templates: experiment.templates.retained_templates,
            },
            control: pair_cluster_arm_output(&experiment.control),
            treatment: pair_cluster_arm_output(&experiment.treatment),
        })
    } else {
        None
    };

    let mut elapsed_ms = Vec::with_capacity(runs);
    let mut result = None;
    let mut relaxed_diagnostics = None::<GeneralRelaxedDiagnostics>;
    let mut constructed_depth_mm = None;
    let mut post_output_failure = None::<String>;
    let job_pool = JobPool::new(Some(threads));
    for _ in 0..runs {
        let started = Instant::now();
        let (current, current_relaxed_diagnostics, current_constructed_depth_mm) = job_pool
            .run_scoped(|| {
                let constructed = construct_short_side_first(&pieces, settings)?;
                let constructed_depth_mm = constructed.used_long_axis_depth_mm;
                if relaxed_epochs == 0 {
                    return Ok::<_, polygon_nesting_core::search::general_fast::GeneralFastError>(
                        (constructed, None, constructed_depth_mm),
                    );
                }
                let mut relaxed_settings =
                    GeneralRelaxedSettings::mixed_61_probe(relaxed_seed, relaxed_lanes);
                relaxed_settings.epochs = relaxed_epochs;
                relaxed_settings.sweeps_per_epoch = relaxed_sweeps;
                relaxed_settings.global_samples_per_move = relaxed_global_samples;
                relaxed_settings.focused_samples_per_move = relaxed_focused_samples;
                relaxed_settings.refinement_rounds = relaxed_refinement_rounds;
                relaxed_settings.initial_shrink_ratio = relaxed_initial_shrink_ratio;
                relaxed_settings.minimum_shrink_ratio = relaxed_minimum_shrink_ratio;
                relaxed_settings.synchronize_lanes = relaxed_synchronize_lanes;
                relaxed_settings.collision_backend = if relaxed_dynamic_hazard {
                    GeneralRelaxedCollisionBackend::DynamicHazard
                } else {
                    GeneralRelaxedCollisionBackend::RollbackTriangle
                };
                relaxed_settings.angle_seed_policy = if relaxed_continuous_seeds {
                    GeneralRelaxedAngleSeedPolicy::ContinuousUniform
                } else {
                    GeneralRelaxedAngleSeedPolicy::StructuredGrid
                };
                relaxed_settings.pressure_model = relaxed_pressure_model;
                relaxed_settings.angular_repair = if relaxed_angular_repair {
                    let mut repair = GeneralAngularRepairSettings::bounded_probe();
                    repair.neighborhood_size = relaxed_repair_neighborhood;
                    repair
                } else {
                    GeneralAngularRepairSettings::disabled()
                };
                relaxed_settings.coupled_dynamic_separator = coupled_dynamic_separator;
                relaxed_settings.precompression_frontier_vacancy_mode =
                    precompression_frontier_vacancy_mode;
                relaxed_settings.persistent_vacancy_mode = persistent_vacancy_mode;
                let outcome = if let Some(parent) = &persistent_vacancy_parent {
                    improve_complete_layout_with_persistent_vacancy_parent(
                        &pieces,
                        settings,
                        relaxed_settings,
                        &constructed,
                        &parent.placements,
                    )?
                } else {
                    improve_complete_layout(&pieces, settings, relaxed_settings, &constructed)?
                };
                Ok((
                    outcome.result,
                    Some(outcome.diagnostics),
                    constructed_depth_mm,
                ))
            })?;
        if persistent_vacancy_mode > 0 {
            let persistent = current_relaxed_diagnostics
                .as_ref()
                .and_then(|diagnostics| diagnostics.coupled_dynamic_separator.as_ref())
                .and_then(|diagnostics| diagnostics.persistent_vacancy_population.as_ref())
                .ok_or("requested persistent-vacancy diagnostics are missing")?;
            if !persistent.attempted {
                return Err(format!(
                    "requested persistent-vacancy experiment did not run: {}",
                    persistent
                        .failure_reason
                        .as_deref()
                        .unwrap_or("no failure reason was recorded")
                )
                .into());
            }
            if persistent_vacancy_mode == 19 {
                let probe = persistent
                    .vacancy_topology_probe
                    .as_ref()
                    .ok_or("requested vacancy-topology probe diagnostics are missing")?;
                if let Some(reason) = probe.failure_reason.as_deref() {
                    return Err(format!("requested vacancy-topology probe failed: {reason}").into());
                }
                if !probe.attempted || probe.snapshots.len() != 18 {
                    return Err(
                        "requested vacancy-topology probe did not reach its bounded terminal"
                            .into(),
                    );
                }
            }
            if persistent_vacancy_mode == 25 {
                let probe = persistent
                    .vacancy_articulation_probe
                    .as_ref()
                    .ok_or("requested vacancy-articulation probe diagnostics are missing")?;
                if let Some(reason) = probe.failure_reason.as_deref() {
                    post_output_failure = Some(format!(
                        "requested vacancy-articulation probe failed: {reason}"
                    ));
                } else if !probe.attempted
                    || probe.rows.len() != 765
                    || probe.baselines.len() != 15
                    || probe.states.len() != 3
                    || probe.work.topology_calls != 780
                    || probe.work.component_graph_node_pairs == 0
                    || probe.work.component_graph_node_pairs
                        > probe.work.component_graph_node_pair_cap
                    || probe.work.component_graph_edge_checks
                        > probe.work.component_graph_edge_check_cap
                    || probe.work.component_graph_scratch_peak_bytes
                        > probe.work.component_graph_scratch_cap_bytes
                {
                    post_output_failure = Some(
                        "requested vacancy-articulation probe did not reach its bounded terminal"
                            .to_owned(),
                    );
                }
            }
            if persistent_vacancy_mode == 26 {
                let probe = persistent
                    .vacancy_bridge_relocation
                    .as_ref()
                    .ok_or("requested vacancy-bridge relocation diagnostics are missing")?;
                if probe.inconclusive {
                    if !probe.attempted
                        || probe.failure_reason.is_some()
                        || probe.terminal_status != "generatorInconclusive"
                        || probe.generated_candidates > probe.candidate_cap
                        || probe.legal_candidates > probe.generated_candidates
                        || probe.control.is_some()
                        || probe.treatment.is_some()
                        || probe.promotion_gate_passed
                    {
                        post_output_failure = Some(
                            "requested vacancy-bridge generator inconclusive terminal is invalid"
                                .to_owned(),
                        );
                    }
                } else if let Some(reason) = probe.failure_reason.as_deref() {
                    post_output_failure = Some(format!(
                        "requested vacancy-bridge relocation failed: {reason}"
                    ));
                } else if !probe.attempted
                    || probe.generated_candidates > probe.candidate_cap
                    || probe.legal_candidates > probe.generated_candidates
                    || probe.control.is_none()
                    || probe.treatment.is_none()
                    || !probe
                        .candidates
                        .iter()
                        .any(|candidate| candidate.selected_control)
                    || !probe
                        .candidates
                        .iter()
                        .any(|candidate| candidate.selected_treatment)
                {
                    post_output_failure = Some(
                        "requested vacancy-bridge relocation did not reach its bounded terminal"
                            .to_owned(),
                    );
                } else if !probe.promotion_gate_passed {
                    post_output_failure = Some(
                        "requested vacancy-bridge relocation failed its strict promotion gate"
                            .to_owned(),
                    );
                }
            }
        }
        elapsed_ms.push(started.elapsed().as_secs_f64() * 1_000.0);
        constructed_depth_mm.get_or_insert(current_constructed_depth_mm);
        if let Some(reference) = &result {
            if reference != &current {
                return Err("deterministic replay produced different results".into());
            }
        } else {
            result = Some(current);
        }
        if let Some(reference) = &relaxed_diagnostics {
            if Some(reference) != current_relaxed_diagnostics.as_ref() {
                return Err("deterministic relaxed replay produced different diagnostics".into());
            }
        } else {
            relaxed_diagnostics = current_relaxed_diagnostics;
        }
    }
    elapsed_ms.sort_by(f64::total_cmp);
    let result = result.expect("positive run count produces a result");
    let placed_area_mm2 = result
        .placements
        .iter()
        .map(|placement| {
            owned
                .iter()
                .find(|piece| piece.id == placement.piece_id)
                .expect("placements reference benchmark pieces")
                .polygon
                .area_mm2()
        })
        .sum::<f64>();
    // keep the search envelope equal to the requested pair clearance. Curve
    // flattening and grid safety are internal and must not add visible gap.
    let collision_expansion_mm = settings.total_padding_mm / 2.0;
    let expanded_collision_area_mm2 = owned.iter().try_fold(0.0, |area, piece| {
        Ok::<_, Box<dyn std::error::Error>>(
            area + piece.polygon.offset(collision_expansion_mm)?.area_mm2(),
        )
    })?;
    let effective_edge_clearance_mm = settings
        .sheet_edge_clearance_mm
        .unwrap_or(settings.total_padding_mm / 2.0);
    let collision_sheet_inset_mm = effective_edge_clearance_mm - settings.total_padding_mm / 2.0;
    let collision_sheet_width_mm = settings.sheet_short_axis_mm - 2.0 * collision_sheet_inset_mm;
    let area_lower_bound_depth_mm =
        expanded_collision_area_mm2 / collision_sheet_width_mm + 2.0 * collision_sheet_inset_mm;
    let strip_area_mm2 = settings.sheet_short_axis_mm * result.used_long_axis_depth_mm;
    let independent_used_long_axis_depth_mm = result
        .placements
        .iter()
        .map(|placement| -> Result<f64, Box<dyn std::error::Error>> {
            let piece = owned
                .iter()
                .find(|piece| piece.id == placement.piece_id)
                .expect("placements reference benchmark pieces");
            let transformed = piece.polygon.transformed(
                placement.rotation_deg,
                placement.mirrored,
                placement.translate_short_axis,
                placement.translate_long_axis,
            )?;
            let bounds = transformed
                .bounds()
                .ok_or("a placed benchmark polygon must be non-empty")?;
            Ok(bounds.max_y + effective_edge_clearance_mm)
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .max_by(f64::total_cmp)
        .unwrap_or(0.0);
    let coupled_treatment_independent_used_long_axis_depth_mm = relaxed_diagnostics
        .as_ref()
        .and_then(|diagnostics| diagnostics.coupled_dynamic_separator.as_ref())
        .filter(|diagnostics| {
            diagnostics.treatment.attempted && !diagnostics.treatment.final_placements.is_empty()
        })
        .map(|diagnostics| {
            independently_measure_coupled_depth(
                &diagnostics.treatment.final_placements,
                &owned,
                effective_edge_clearance_mm,
            )
        })
        .transpose()?;
    if let (Some(reported), Some(independent)) = (
        relaxed_diagnostics
            .as_ref()
            .and_then(|diagnostics| diagnostics.coupled_dynamic_separator.as_ref())
            .and_then(|diagnostics| diagnostics.treatment.independently_measured_final_depth_mm),
        coupled_treatment_independent_used_long_axis_depth_mm,
    ) {
        if ordered_f64_bits(reported).abs_diff(ordered_f64_bits(independent)) > 1 {
            return Err(format!(
                "coupled treatment depth disagrees with independent source reconstruction: reported={reported}, independent={independent}"
            )
            .into());
        }
    }
    let first_quartile_elapsed_ms = percentile_nearest_rank(&elapsed_ms, 0.25);
    let third_quartile_elapsed_ms = percentile_nearest_rank(&elapsed_ms, 0.75);
    let git_commit = command_output("git", &["rev-parse", "HEAD"]);
    let git_status = command_output_allow_empty(
        "git",
        &["status", "--porcelain=v1", "--untracked-files=normal"],
    );
    let git_dirty = git_status.as_ref().map(|status| !status.is_empty());
    let executable_sha256 = env::current_exe()
        .ok()
        .and_then(|path| fs::read(path).ok())
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)));
    let relevant_source_tree_sha256 = relevant_source_tree_sha256();
    let rustc_version = command_output("rustc", &["-Vv"]);
    let machine_architecture = command_output("uname", &["-m"]);
    let cpu_model = command_output("sysctl", &["-n", "machdep.cpu.brand_string"])
        .or_else(|| command_output("sh", &["-c", "grep -m1 'model name' /proc/cpuinfo"]));
    let rustflags = env::var("RUSTFLAGS").ok();
    let quota = QuotaOutput {
        order_variants,
        exploratory_evaluations_per_piece: exploratory_evaluations,
        repair_targets,
        repair_evaluations_per_piece: repair_evaluations,
        local_angle_evaluations_per_piece: local_angle_evaluations,
        catalog_variants,
        catalog_evaluations_per_piece: catalog_evaluations,
        pairing_evaluations_per_piece: pairing_evaluations,
        pairing_band_variants,
        partial_layouts,
        beam_evaluations_per_state: beam_evaluations,
        angle_seed_count,
        max_angles_per_piece,
        tightening_passes,
        relaxed_epochs,
        relaxed_lanes,
        relaxed_sweeps_per_epoch: relaxed_sweeps,
        relaxed_global_samples_per_move: relaxed_global_samples,
        relaxed_focused_samples_per_move: relaxed_focused_samples,
        relaxed_refinement_rounds,
        relaxed_seed,
        relaxed_initial_shrink_ratio,
        relaxed_minimum_shrink_ratio,
        relaxed_failed_attempts_per_depth,
        relaxed_infeasible_pool_size,
        relaxed_infeasible_pool_arguments_ignored: true,
        relaxed_synchronize_lanes,
        relaxed_dynamic_hazard,
        relaxed_angle_seed_policy: if relaxed_continuous_seeds {
            "continuousUniform"
        } else {
            "structuredGrid"
        },
        relaxed_pressure_model: pressure_model_name(relaxed_pressure_model),
        relaxed_angular_repair,
        relaxed_repair_neighborhood,
        pair_template_diagnostics,
        pair_constructor_diagnostics,
        precompression_frontier_vacancy_mode,
        exact_pair_terminal_mode: retired_exact_pair_terminal_mode,
        persistent_vacancy_mode,
    };
    let (serialized, serialization_fallback_used) = if matches!(persistent_vacancy_mode, 25 | 26) {
        let parent_fixture = persistent_vacancy_parent
            .as_ref()
            .map(|parent| ParentFixtureOutput {
                schema_version: parent.schema_version,
                sha256: parent.sha256.as_str(),
                placement_count: parent.placements.len(),
            });
        let mut output = Mode25Output {
            request: &request_path,
            request_sha256: &request_sha256,
            engine_commit: &git_commit,
            engine_worktree_dirty: &git_dirty,
            engine_worktree_status: &git_status,
            executable_sha256: &executable_sha256,
            relevant_source_tree_sha256: &relevant_source_tree_sha256,
            profile: "general-fast-experimental",
            build_profile: if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            },
            target_architecture: std::env::consts::ARCH,
            target_operating_system: std::env::consts::OS,
            machine_architecture: &machine_architecture,
            cpu_model: &cpu_model,
            rustc_version: &rustc_version,
            rustflags: &rustflags,
            budget_mode: "deterministic-work-quota",
            seed: (),
            piece_count: pieces.len(),
            source_piece_count: source_by_id.len(),
            total_vertices: owned
                .iter()
                .map(|piece| piece.polygon.vertex_count())
                .sum::<usize>(),
            concave_piece_count: owned
                .iter()
                .filter(|piece| {
                    piece
                        .polygon
                        .regions()
                        .iter()
                        .any(|region| !region.outer.is_convex())
                })
                .count(),
            sheet_short_axis_mm: settings.sheet_short_axis_mm,
            sheet_long_axis_mm: settings.sheet_long_axis_mm,
            request_total_padding_mm,
            pair_clearance_mm: settings.total_padding_mm,
            sheet_edge_clearance_mm: settings
                .sheet_edge_clearance_mm
                .unwrap_or(settings.total_padding_mm / 2.0),
            flattening_sag_tolerance_mm: settings.flattening_sag_tolerance_mm,
            clearance_safety_margin_mm: settings.clearance_safety_margin_mm,
            requested_threads: job_pool.requested_thread_count(),
            actual_threads: job_pool.actual_thread_count(),
            quota: &quota,
            pair_template_probe: &pair_template_probe,
            pair_constructor_probe: &pair_constructor_probe,
            relaxed_diagnostics,
            placed: result.placements.len(),
            unplaced: result.unplaced_piece_ids.len(),
            constructed_long_axis_depth_mm: constructed_depth_mm,
            used_long_axis_depth_mm: result.used_long_axis_depth_mm,
            independent_used_long_axis_depth_mm,
            coupled_treatment_independent_used_long_axis_depth_mm,
            placed_material_area_mm2: placed_area_mm2,
            expanded_collision_area_mm2,
            area_lower_bound_depth_mm,
            depth_over_area_lower_bound: result.used_long_axis_depth_mm / area_lower_bound_depth_mm,
            used_strip_area_mm2: strip_area_mm2,
            used_strip_utilization_percent: if strip_area_mm2 > 0.0 {
                placed_area_mm2 / strip_area_mm2 * 100.0
            } else {
                0.0
            },
            exact_evaluations: result.exact_evaluations,
            primary_exact_evaluations: result.primary_exact_evaluations,
            order_portfolio_exact_evaluations: result.order_portfolio_exact_evaluations,
            catalog_portfolio_exact_evaluations: result.catalog_portfolio_exact_evaluations,
            pairing_exact_evaluations: result.pairing_exact_evaluations,
            beam_exact_evaluations: result.beam_exact_evaluations,
            tightening_exact_evaluations: result.tightening_exact_evaluations,
            tightening_passes_attempted: result.tightening_passes_attempted,
            tightening_passes_improved: result.tightening_passes_improved,
            catalog_candidate_placed_count: result.catalog_candidate_placed_count,
            catalog_candidate_depth_mm: result.catalog_candidate_depth_mm,
            pairing_candidate_placed_count: result.pairing_candidate_placed_count,
            pairing_candidate_depth_mm: result.pairing_candidate_depth_mm,
            beam_candidate_placed_count: result.beam_candidate_placed_count,
            beam_candidate_depth_mm: result.beam_candidate_depth_mm,
            exploratory_exact_evaluations: result.exploratory_exact_evaluations,
            repair_exact_evaluations: result.repair_exact_evaluations,
            local_angle_refinement_exact_evaluations: result
                .local_angle_refinement_exact_evaluations,
            order_variants_attempted: result.order_variants_attempted,
            catalog_variants_attempted: result.catalog_variants_attempted,
            repair_targets_considered: result.repair_targets_considered,
            order_portfolio_failed: result.order_portfolio_failed,
            catalog_portfolio_failed: result.catalog_portfolio_failed,
            pairing_failed: result.pairing_failed,
            beam_failed: result.beam_failed,
            exploratory_failed: result.exploratory_failed,
            repair_failed: result.repair_failed,
            median_elapsed_ms: elapsed_ms[elapsed_ms.len() / 2],
            first_quartile_elapsed_ms,
            third_quartile_elapsed_ms,
            interquartile_range_elapsed_ms: third_quartile_elapsed_ms - first_quartile_elapsed_ms,
            min_elapsed_ms: elapsed_ms[0],
            max_elapsed_ms: elapsed_ms[elapsed_ms.len() - 1],
            elapsed_ms: &elapsed_ms,
            ignored_request_metadata_fields: MetadataKeys(&request.extra),
            placements: PlacementList(&result.placements),
            persistent_vacancy_parent_fixture: parent_fixture,
        };
        if persistent_vacancy_mode == 25 {
            let bounded =
                serialize_mode25_output(&mut output, VACANCY_ARTICULATION_JSON_CAP_BYTES)?;
            (bounded.bytes, bounded.fallback_used)
        } else {
            (
                serialize_pretty_bounded(&output, VACANCY_BRIDGE_JSON_CAP_BYTES)?,
                false,
            )
        }
    } else {
        let mut output = json!({
            "request": request_path,
            "requestSha256": request_sha256,
            "engineCommit": git_commit,
            "engineWorktreeDirty": git_dirty,
            "engineWorktreeStatus": git_status,
            "executableSha256": executable_sha256,
            "relevantSourceTreeSha256": relevant_source_tree_sha256,
            "profile": "general-fast-experimental",
            "buildProfile": if cfg!(debug_assertions) { "debug" } else { "release" },
            "targetArchitecture": std::env::consts::ARCH,
            "targetOperatingSystem": std::env::consts::OS,
            "machineArchitecture": machine_architecture,
            "cpuModel": cpu_model,
            "rustcVersion": rustc_version,
            "rustflags": rustflags,
            "budgetMode": "deterministic-work-quota",
            "seed": serde_json::Value::Null,
            "pieceCount": pieces.len(),
            "sourcePieceCount": source_by_id.len(),
            "totalVertices": owned.iter().map(|piece| piece.polygon.vertex_count()).sum::<usize>(),
            "concavePieceCount": owned.iter().filter(|piece| piece.polygon.regions().iter().any(|region| !region.outer.is_convex())).count(),
            "sheetShortAxisMm": settings.sheet_short_axis_mm,
            "sheetLongAxisMm": settings.sheet_long_axis_mm,
            "requestTotalPaddingMm": request_total_padding_mm,
            "pairClearanceMm": settings.total_padding_mm,
            "sheetEdgeClearanceMm": settings.sheet_edge_clearance_mm.unwrap_or(settings.total_padding_mm / 2.0),
            "flatteningSagToleranceMm": settings.flattening_sag_tolerance_mm,
            "clearanceSafetyMarginMm": settings.clearance_safety_margin_mm,
            "requestedThreads": job_pool.requested_thread_count(),
            "actualThreads": job_pool.actual_thread_count(),
            "quota": quota,
            "pairTemplateProbe": pair_template_probe,
            "pairConstructorProbe": pair_constructor_probe,
            "relaxedDiagnostics": relaxed_diagnostics,
            "placed": result.placements.len(),
            "unplaced": result.unplaced_piece_ids.len(),
            "constructedLongAxisDepthMm": constructed_depth_mm,
            "usedLongAxisDepthMm": result.used_long_axis_depth_mm,
            "independentUsedLongAxisDepthMm": independent_used_long_axis_depth_mm,
            "coupledTreatmentIndependentUsedLongAxisDepthMm": coupled_treatment_independent_used_long_axis_depth_mm,
            "placedMaterialAreaMm2": placed_area_mm2,
            "expandedCollisionAreaMm2": expanded_collision_area_mm2,
            "areaLowerBoundDepthMm": area_lower_bound_depth_mm,
            "depthOverAreaLowerBound": result.used_long_axis_depth_mm / area_lower_bound_depth_mm,
            "usedStripAreaMm2": strip_area_mm2,
            "usedStripUtilizationPercent": if strip_area_mm2 > 0.0 { placed_area_mm2 / strip_area_mm2 * 100.0 } else { 0.0 },
            "exactEvaluations": result.exact_evaluations,
            "primaryExactEvaluations": result.primary_exact_evaluations,
            "orderPortfolioExactEvaluations": result.order_portfolio_exact_evaluations,
            "catalogPortfolioExactEvaluations": result.catalog_portfolio_exact_evaluations,
            "pairingExactEvaluations": result.pairing_exact_evaluations,
            "beamExactEvaluations": result.beam_exact_evaluations,
            "tighteningExactEvaluations": result.tightening_exact_evaluations,
            "tighteningPassesAttempted": result.tightening_passes_attempted,
            "tighteningPassesImproved": result.tightening_passes_improved,
            "catalogCandidatePlacedCount": result.catalog_candidate_placed_count,
            "catalogCandidateDepthMm": result.catalog_candidate_depth_mm,
            "pairingCandidatePlacedCount": result.pairing_candidate_placed_count,
            "pairingCandidateDepthMm": result.pairing_candidate_depth_mm,
            "beamCandidatePlacedCount": result.beam_candidate_placed_count,
            "beamCandidateDepthMm": result.beam_candidate_depth_mm,
            "exploratoryExactEvaluations": result.exploratory_exact_evaluations,
            "repairExactEvaluations": result.repair_exact_evaluations,
            "localAngleRefinementExactEvaluations": result.local_angle_refinement_exact_evaluations,
            "orderVariantsAttempted": result.order_variants_attempted,
            "catalogVariantsAttempted": result.catalog_variants_attempted,
            "repairTargetsConsidered": result.repair_targets_considered,
            "orderPortfolioFailed": result.order_portfolio_failed,
            "catalogPortfolioFailed": result.catalog_portfolio_failed,
            "pairingFailed": result.pairing_failed,
            "beamFailed": result.beam_failed,
            "exploratoryFailed": result.exploratory_failed,
            "repairFailed": result.repair_failed,
            "medianElapsedMs": elapsed_ms[elapsed_ms.len() / 2],
            "firstQuartileElapsedMs": first_quartile_elapsed_ms,
            "thirdQuartileElapsedMs": third_quartile_elapsed_ms,
            "interquartileRangeElapsedMs": third_quartile_elapsed_ms - first_quartile_elapsed_ms,
            "minElapsedMs": elapsed_ms[0],
            "maxElapsedMs": elapsed_ms[elapsed_ms.len() - 1],
            "elapsedMs": elapsed_ms,
            "ignoredRequestMetadataFields": request.extra.keys().collect::<Vec<_>>(),
            "placements": result.placements.iter().map(|placement| json!({
                "pieceId": placement.piece_id,
                "rotationDeg": placement.rotation_deg,
                "mirrored": placement.mirrored,
                "translateShortAxis": placement.translate_short_axis,
                "translateLongAxis": placement.translate_long_axis,
            })).collect::<Vec<_>>(),
        });
        if let Some(parent) = &persistent_vacancy_parent {
            output
                .as_object_mut()
                .expect("the benchmark output is an object")
                .insert(
                    "persistentVacancyParentFixture".to_owned(),
                    json!({
                        "schemaVersion": parent.schema_version,
                        "sha256": parent.sha256,
                        "placementCount": parent.placements.len(),
                    }),
                );
        }
        (serde_json::to_vec_pretty(&output)?, false)
    };
    let mut stdout = io::stdout().lock();
    stdout.write_all(&serialized)?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    if serialization_fallback_used {
        post_output_failure.get_or_insert_with(|| {
            "vacancy-articulation output exceeded its bounded JSON serialization cap".to_owned()
        });
    }
    if let Some(reason) = post_output_failure {
        return Err(reason.into());
    }
    Ok(())
}

struct FixedCapacityJsonBuffer {
    bytes: Vec<u8>,
    capacity: usize,
    overflowed: bool,
}

impl FixedCapacityJsonBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(capacity),
            capacity,
            overflowed: false,
        }
    }

    fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

impl Write for FixedCapacityJsonBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(required) = self.bytes.len().checked_add(bytes.len()) else {
            self.overflowed = true;
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "JSON serialization size overflow",
            ));
        };
        if required > self.capacity {
            self.overflowed = true;
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "JSON serialization cap exceeded",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn serialize_pretty_bounded<T: Serialize>(
    value: &T,
    capacity: usize,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut buffer = FixedCapacityJsonBuffer::new(capacity);
    match serde_json::to_writer_pretty(&mut buffer, value) {
        Ok(()) => Ok(buffer.into_bytes()),
        Err(_error) if buffer.overflowed => {
            Err(format!("JSON serialization exceeded its {capacity}-byte cap").into())
        }
        Err(error) => Err(error.into()),
    }
}

#[derive(Debug)]
struct BoundedMode25Output {
    bytes: Vec<u8>,
    fallback_used: bool,
}

fn serialize_mode25_output(
    output: &mut Mode25Output<'_>,
    capacity: usize,
) -> Result<BoundedMode25Output, Box<dyn std::error::Error>> {
    match serialize_pretty_bounded(output, capacity) {
        Ok(bytes) => Ok(BoundedMode25Output {
            bytes,
            fallback_used: false,
        }),
        Err(primary_error)
            if primary_error
                .to_string()
                .contains("serialization exceeded its") =>
        {
            let reason = primary_error.to_string();
            replace_vacancy_articulation_sidecar(
                &mut output.relaxed_diagnostics,
                capacity,
                &reason,
            )?;
            match serialize_pretty_bounded(output, capacity) {
                Ok(bytes) => Ok(BoundedMode25Output {
                    bytes,
                    fallback_used: true,
                }),
                Err(fallback_error) => Err(format!(
                    "the intact base result exceeded the {capacity}-byte cap even after replacing the vacancy-articulation sidecar: {fallback_error}"
                )
                .into()),
            }
        }
        Err(error) => Err(error),
    }
}

fn replace_vacancy_articulation_sidecar(
    diagnostics: &mut Option<GeneralRelaxedDiagnostics>,
    capacity: usize,
    primary_error: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let probe = diagnostics
        .as_mut()
        .and_then(|diagnostics| diagnostics.coupled_dynamic_separator.as_mut())
        .and_then(|diagnostics| diagnostics.persistent_vacancy_population.as_mut())
        .and_then(|population| population.vacancy_articulation_probe.as_mut())
        .ok_or_else(|| {
            format!(
                "the vacancy-articulation sidecar was missing after primary serialization overflow: {primary_error}"
            )
        })?;
    *probe = Default::default();
    probe.attempted = true;
    probe.failure_reason = Some(format!(
        "primary output serialization exceeded the {capacity}-byte cap: {primary_error}"
    ));
    probe.serialization_cap_bytes = Some(capacity);
    Ok(())
}

#[cfg(test)]
mod bounded_json_tests {
    use super::*;
    use polygon_nesting_core::search::general_relaxed::{
        GeneralCoupledSeparatorDiagnostics, GeneralPersistentVacancyArticulationProbeDiagnostics,
        GeneralPersistentVacancyDiagnostics,
    };

    #[test]
    fn bounded_pretty_serialization_matches_unbounded_output() {
        let value = json!({
            "message": "fixed-capacity output",
            "values": [1, 2, 3, 4],
            "nested": {"enabled": true},
        });

        assert_eq!(
            serialize_pretty_bounded(&value, 1024).unwrap(),
            serde_json::to_vec_pretty(&value).unwrap()
        );
    }

    #[test]
    fn bounded_writer_rejects_without_growing_past_the_cap() {
        let mut buffer = FixedCapacityJsonBuffer::new(3);
        buffer.write_all(b"abc").unwrap();
        assert!(buffer.write_all(b"d").is_err());
        assert!(buffer.overflowed);
        assert_eq!(buffer.bytes.len(), 3);
        assert!(buffer.bytes.capacity() <= 3);
    }

    #[test]
    fn bounded_pretty_serialization_rejects_overflow() {
        let value = json!({"payload": "x".repeat(128)});
        let error = serialize_pretty_bounded(&value, 32).unwrap_err();

        assert_eq!(
            error.to_string(),
            "JSON serialization exceeded its 32-byte cap"
        );
    }

    #[test]
    fn mode25_overflow_replaces_only_the_additive_typed_sidecar() {
        let mut probe = GeneralPersistentVacancyArticulationProbeDiagnostics::default();
        probe.attempted = true;
        probe.rows = vec![Default::default(); 765];
        let mut population = GeneralPersistentVacancyDiagnostics::default();
        population.mode = 25;
        population.vacancy_articulation_probe = Some(probe);
        let mut coupled = GeneralCoupledSeparatorDiagnostics {
            seed_domain: 0,
            control: Default::default(),
            treatment: Default::default(),
            boundary_projection_treatment: None,
            conflict_ruin_recreate: None,
            precompression_frontier_vacancy: None,
            persistent_vacancy_population: Some(population),
        };
        let mut diagnostics = Some(GeneralRelaxedDiagnostics {
            coupled_dynamic_separator: Some(coupled.clone()),
            ..Default::default()
        });

        replace_vacancy_articulation_sidecar(&mut diagnostics, 4 * 1024, "cap").unwrap();
        coupled = diagnostics
            .as_ref()
            .unwrap()
            .coupled_dynamic_separator
            .as_ref()
            .unwrap()
            .clone();
        let population = coupled.persistent_vacancy_population.as_ref().unwrap();
        let sidecar = population.vacancy_articulation_probe.as_ref().unwrap();
        assert_eq!(population.mode, 25);
        assert!(sidecar.attempted);
        assert!(sidecar.rows.is_empty());
        assert_eq!(sidecar.serialization_cap_bytes, Some(4 * 1024));
        assert!(sidecar.failure_reason.as_deref().unwrap().contains("cap"));

        let bytes = serialize_pretty_bounded(sidecar, 4 * 1024).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed["rows"], json!([]));
        assert_eq!(parsed["serializationCapBytes"], 4 * 1024);
    }

    #[test]
    fn mode25_typed_failure_sidecar_stays_bounded_after_replacement() {
        let mut probe = GeneralPersistentVacancyArticulationProbeDiagnostics::default();
        probe.rows = vec![Default::default(); 765];
        let before = serialize_pretty_bounded(&probe, 4 * 1024).unwrap_err();
        assert!(before.to_string().contains("exceeded"));

        let mut population = GeneralPersistentVacancyDiagnostics::default();
        population.vacancy_articulation_probe = Some(probe);
        let mut diagnostics = Some(GeneralRelaxedDiagnostics {
            coupled_dynamic_separator: Some(GeneralCoupledSeparatorDiagnostics {
                seed_domain: 0,
                control: Default::default(),
                treatment: Default::default(),
                boundary_projection_treatment: None,
                conflict_ruin_recreate: None,
                precompression_frontier_vacancy: None,
                persistent_vacancy_population: Some(population),
            }),
            ..Default::default()
        });
        replace_vacancy_articulation_sidecar(&mut diagnostics, 4 * 1024, "cap").unwrap();
        let sidecar = diagnostics
            .as_ref()
            .unwrap()
            .coupled_dynamic_separator
            .as_ref()
            .unwrap()
            .persistent_vacancy_population
            .as_ref()
            .unwrap()
            .vacancy_articulation_probe
            .as_ref()
            .unwrap();
        assert!(serialize_pretty_bounded(sidecar, 4 * 1024).is_ok());
    }
}

fn independently_measure_coupled_depth(
    placements: &[polygon_nesting_core::search::general_relaxed::GeneralCoupledSeparatorPlacementDiagnostics],
    owned: &[OwnedPiece],
    edge_clearance_mm: f64,
) -> Result<f64, Box<dyn std::error::Error>> {
    placements
        .iter()
        .map(|placement| -> Result<f64, Box<dyn std::error::Error>> {
            let piece = owned
                .iter()
                .find(|piece| piece.id == placement.piece_id)
                .ok_or_else(|| format!("unknown coupled placement {}", placement.piece_id))?;
            let transformed = piece.polygon.transformed(
                placement.rotation_deg,
                placement.mirrored,
                placement.translate_short_axis,
                placement.translate_long_axis,
            )?;
            let bounds = transformed
                .bounds()
                .ok_or("a coupled diagnostic polygon must be non-empty")?;
            Ok(bounds.max_y + edge_clearance_mm)
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .max_by(f64::total_cmp)
        .ok_or_else(|| "coupled diagnostics must retain at least one placement".into())
}

fn pair_cluster_arm_output(diagnostics: &GeneralPairClusterArmDiagnostics) -> PairClusterArmOutput {
    PairClusterArmOutput {
        placed: diagnostics
            .result
            .as_ref()
            .map(|result| result.placements.len()),
        used_long_axis_depth_mm: diagnostics
            .result
            .as_ref()
            .map(|result| result.used_long_axis_depth_mm),
        band_variants_attempted: diagnostics.band_variants_attempted,
        completed_bands: diagnostics.completed_bands,
        band_failures: diagnostics.band_failures.clone(),
        proposal_attempts: diagnostics.proposal_attempts,
        generated_proposals: diagnostics.generated_proposals,
        exact_child_fixed_visits: diagnostics.exact_child_fixed_visits,
        exact_candidate_rows: diagnostics.exact_candidate_rows,
    }
}

fn ordered_f64_bits(value: f64) -> u64 {
    let bits = value.to_bits();
    if bits & (1 << 63) == 0 {
        bits | (1 << 63)
    } else {
        !bits
    }
}

fn parse_optional(
    arguments: &mut impl Iterator<Item = String>,
    default: usize,
) -> Result<usize, Box<dyn std::error::Error>> {
    Ok(arguments
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(default))
}

fn parse_optional_pressure_model(
    arguments: &mut impl Iterator<Item = String>,
    default: GeneralRelaxedPressureModel,
) -> Result<GeneralRelaxedPressureModel, Box<dyn std::error::Error>> {
    let Some(value) = arguments.next() else {
        return Ok(default);
    };
    match value.as_str() {
        "0" | "structured" | "structured-triangle-poles" => {
            Ok(GeneralRelaxedPressureModel::StructuredTrianglePoles)
        }
        "directional" | "directional-penetration" => {
            Ok(GeneralRelaxedPressureModel::DirectionalPenetration)
        }
        "continuous" | "continuous-triangle-poles" => {
            Ok(GeneralRelaxedPressureModel::ContinuousTrianglePoles)
        }
        "1" | "dynamic" | "dynamic-poles" => Ok(GeneralRelaxedPressureModel::DynamicPoles),
        _ => Err(format!(
            "unsupported relaxed pressure model {value}; expected structured, directional, continuous, or dynamic"
        )
        .into()),
    }
}

fn pressure_model_name(model: GeneralRelaxedPressureModel) -> &'static str {
    match model {
        GeneralRelaxedPressureModel::StructuredTrianglePoles => "structuredTrianglePoles",
        GeneralRelaxedPressureModel::DirectionalPenetration => "directionalPenetration",
        GeneralRelaxedPressureModel::ContinuousTrianglePoles => "continuousTrianglePoles",
        GeneralRelaxedPressureModel::DynamicPoles => "dynamicPoles",
    }
}

fn percentile_nearest_rank(sorted: &[f64], percentile: f64) -> f64 {
    let index = ((sorted.len() - 1) as f64 * percentile).round() as usize;
    sorted[index]
}

fn command_output(program: &str, arguments: &[&str]) -> Option<String> {
    command_output_allow_empty(program, arguments).filter(|value| !value.is_empty())
}

fn command_output_allow_empty(program: &str, arguments: &[&str]) -> Option<String> {
    let output = Command::new(program).args(arguments).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn relevant_source_tree_sha256() -> Option<String> {
    let output = Command::new("git")
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
            "Cargo.toml",
            "Cargo.lock",
            "crates/polygon-nesting-core",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let mut paths = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8(path.to_vec()).ok())
        .collect::<Option<Vec<_>>>()?;
    paths.sort();
    let mut hasher = Sha256::new();
    for path in paths {
        let bytes = fs::read(&path).ok()?;
        hasher.update((path.len() as u64).to_le_bytes());
        hasher.update(path.as_bytes());
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    Some(format!("{:x}", hasher.finalize()))
}

fn parse_optional_f64(
    arguments: &mut impl Iterator<Item = String>,
    default: f64,
) -> Result<f64, Box<dyn std::error::Error>> {
    Ok(arguments
        .next()
        .map(|value| value.parse::<f64>())
        .transpose()?
        .unwrap_or(default))
}

fn default_true() -> bool {
    true
}

fn effective_request_settings(
    request: &Request,
) -> Result<(f64, bool, bool, GeometrySettings), Box<dyn std::error::Error>> {
    match (&request.settings, &request.options) {
        (Some(settings), None) => Ok((
            settings.padding,
            settings.allow_global_rotation,
            settings.allow_global_mirror,
            settings.geometry,
        )),
        (None, Some(options)) => Ok((
            request
                .padding
                .ok_or("legacy requests require top-level padding")?,
            options.allow_global_rotation,
            options.allow_global_mirror,
            options.irregular_settings.geometry,
        )),
        (Some(_), Some(_)) => {
            Err("a request must not mix current settings with legacy options".into())
        }
        (None, None) => Err("a request must contain settings or legacy options".into()),
    }
}

fn unique_sources(
    sources: &[ImportedPiece],
) -> Result<BTreeMap<&str, &ImportedPiece>, Box<dyn std::error::Error>> {
    let mut by_id = BTreeMap::new();
    for source in sources {
        if by_id.insert(source.id.as_str(), source).is_some() {
            return Err(format!("duplicate source piece ID: {}", source.id.as_str()).into());
        }
    }
    Ok(by_id)
}

fn reject_duplicate_piece_ids(pieces: &[RequestPiece]) -> Result<(), Box<dyn std::error::Error>> {
    let mut ids = std::collections::BTreeSet::new();
    for piece in pieces {
        if !ids.insert(piece.id.as_str()) {
            return Err(format!("duplicate prepared piece ID: {}", piece.id).into());
        }
    }
    Ok(())
}

fn normalize_polygon_axes(
    polygon: PolygonSet,
    rotate_physical_to_normalized: bool,
) -> Result<PolygonSet, Box<dyn std::error::Error>> {
    if !rotate_physical_to_normalized {
        return Ok(polygon);
    }
    let rotated = polygon.transformed(270.0, false, 0.0, 0.0)?;
    let bounds = rotated
        .bounds()
        .ok_or("cannot normalize empty source geometry")?;
    Ok(rotated.translated(-bounds.min_x, -bounds.min_y)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polygon_nesting_core::domain::IrregularPoint;
    use polygon_nesting_core::validation::general_polygon::{
        validate_publication, GeneralPlacement, PublicationValidationSettings,
    };

    #[test]
    fn pressure_model_argument_is_independent_from_angle_seed_policy() {
        for (value, expected) in [
            (
                "structured",
                GeneralRelaxedPressureModel::StructuredTrianglePoles,
            ),
            (
                "continuous",
                GeneralRelaxedPressureModel::ContinuousTrianglePoles,
            ),
            ("dynamic", GeneralRelaxedPressureModel::DynamicPoles),
        ] {
            let mut arguments = [value.to_owned()].into_iter();
            assert_eq!(
                parse_optional_pressure_model(
                    &mut arguments,
                    GeneralRelaxedPressureModel::StructuredTrianglePoles,
                )
                .unwrap(),
                expected
            );
        }
    }

    fn rectangle(width: f64, height: f64) -> PolygonSet {
        PolygonSet::from_outer(vec![
            IrregularPoint::new(0.0, 0.0),
            IrregularPoint::new(width, 0.0),
            IrregularPoint::new(width, height),
            IrregularPoint::new(0.0, height),
        ])
        .unwrap()
    }

    #[test]
    fn physical_height_short_axis_is_rotated_into_normalized_x() {
        let normalized = normalize_polygon_axes(rectangle(3.0, 1.0), true).unwrap();
        let bounds = normalized.bounds().unwrap();
        assert_eq!(bounds.max_x - bounds.min_x, 1.0);
        assert_eq!(bounds.max_y - bounds.min_y, 3.0);
        assert_eq!(bounds.min_x, 0.0);
        assert_eq!(bounds.min_y, 0.0);
    }

    #[test]
    fn physical_axis_normalization_keeps_subgrid_source_violations_visible() {
        let source = PolygonSet::from_outer(vec![
            IrregularPoint::new(0.0004, 0.0004),
            IrregularPoint::new(2.0004, 0.0004),
            IrregularPoint::new(2.0004, 1.0004),
            IrregularPoint::new(0.0004, 1.0004),
        ])
        .unwrap();
        let normalized = normalize_polygon_axes(source, true).unwrap();

        let error = validate_publication(
            &[GeneralPlacement {
                piece_id: "subgrid",
                polygon: &normalized,
                rotation_deg: 0.0,
                mirrored: false,
                translate_x: 0.0,
                translate_y: 0.0,
            }],
            PublicationValidationSettings {
                sheet_width_mm: 1.0,
                sheet_height_mm: 2.0,
                total_padding_mm: 0.0,
                sheet_edge_clearance_mm: None,
                flattening_sag_tolerance_mm: 0.0,
            },
        )
        .unwrap_err();

        assert_eq!(
            error.message(),
            "piece subgrid crosses the sheet clearance boundary"
        );
    }

    #[test]
    fn current_request_settings_are_authoritative() {
        let request: Request = serde_json::from_value(json!({
            "sheet": { "width": 20.0, "height": 10.0 },
            "pieces": [],
            "sourcePieces": [],
            "settings": {
                "padding": 7.0,
                "allowGlobalRotation": false,
                "allowGlobalMirror": false,
                "geometry": {
                    "flatteningSagToleranceMm": 0.1,
                    "clearanceSafetyMarginMm": 0.2
                }
            }
        }))
        .unwrap();

        let (padding, rotation, mirror, geometry) = effective_request_settings(&request).unwrap();
        assert_eq!(padding, 7.0);
        assert!(!rotation);
        assert!(!mirror);
        assert_eq!(geometry.flattening_sag_tolerance_mm, 0.1);
        assert_eq!(geometry.clearance_safety_margin_mm, 0.2);
    }
}
