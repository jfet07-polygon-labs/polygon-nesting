#![recursion_limit = "512"]

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use polygon_nesting_core::domain::ImportedPiece;
use polygon_nesting_core::geometry::general_polygon::PolygonSet;
use polygon_nesting_core::geometry::general_source::polygon_set_from_imported_piece;
use polygon_nesting_core::parallel::JobPool;
use polygon_nesting_core::search::general_fast::GeneralFastPlacement;
use polygon_nesting_core::search::general_fast::{
    construct_short_side_first, diagnose_congruent_pair_constructor,
    diagnose_congruent_pair_templates, GeneralFastPiece, GeneralFastSettings,
    GeneralPairClusterArmDiagnostics,
};
use polygon_nesting_core::search::general_relaxed::{
    improve_complete_layout_with_pinned_vacancy_parent, GeneralAngularRepairSettings,
    GeneralPersistentVacancyPinnedParent, GeneralRelaxedAngleSeedPolicy,
    GeneralRelaxedCollisionBackend, GeneralRelaxedDiagnostics, GeneralRelaxedPressureModel,
    GeneralRelaxedSettings,
};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args().skip(1);
    let request_path = arguments.next().ok_or(
        "usage: general_request_benchmark REQUEST.json [runs] [order-variants] [exploratory-evaluations-per-piece] [repair-targets] [repair-evaluations-per-piece] [local-angle-evaluations-per-piece] [catalog-variants] [catalog-evaluations-per-piece] [pairing-evaluations-per-piece] [pairing-band-variants] [partial-layouts] [beam-evaluations-per-state] [angle-seed-count] [max-angles-per-piece] [threads] [sheet-long-axis-override-mm] [tightening-passes] [sheet-edge-clearance-mm] [pair-clearance-mm] [relaxed-epochs] [relaxed-lanes] [relaxed-sweeps] [relaxed-global-samples] [relaxed-focused-samples] [relaxed-refinement-rounds] [relaxed-seed] [relaxed-initial-shrink-ratio] [relaxed-minimum-shrink-ratio] [relaxed-failed-attempts-per-depth] [relaxed-infeasible-pool-size] [relaxed-synchronize-lanes] [relaxed-dynamic-hazard] [relaxed-continuous-seeds] [relaxed-pressure-model] [relaxed-angular-repair] [relaxed-repair-neighborhood] [coupled-dynamic-separator] [pair-template-diagnostics] [pair-constructor-diagnostics] [precompression-frontier-vacancy] [exact-pair-terminal] [persistent-vacancy] [persistent-vacancy-parent-fixture] [persistent-vacancy-target-depth-mm]",
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
    if persistent_vacancy_mode > 21 {
        return Err("persistent vacancy mode must be between 0 and 21".into());
    }
    let persistent_vacancy_parent_fixture = arguments.next();
    let persistent_vacancy_target_depth_mm = arguments
        .next()
        .map(|value| value.parse::<f64>())
        .transpose()
        .map_err(|error| format!("persistent vacancy target depth: {error}"))?;
    if runs == 0 || arguments.next().is_some() {
        return Err("runs must be positive and no extra arguments are accepted".into());
    }

    let bytes = fs::read(Path::new(&request_path))?;
    let request_sha256 = format!("{:x}", Sha256::digest(&bytes));
    let pinned_vacancy_parent = persistent_vacancy_parent_fixture
        .as_deref()
        .map(|path| load_pinned_vacancy_parent(path, &request_sha256))
        .transpose()?;
    let request: Request = serde_json::from_slice(&bytes)?;
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
    let pair_template_probe = if pair_template_diagnostics {
        let started = Instant::now();
        let diagnostics = diagnose_congruent_pair_templates(&pieces, settings)?;
        Some(json!({
            "elapsedMs": started.elapsed().as_secs_f64() * 1_000.0,
            "eligiblePairs": diagnostics.eligible_pairs,
            "pairsWithTemplates": diagnostics.pairs_with_templates,
            "fallbackPairs": diagnostics.fallback_pairs,
            "orientationTuples": diagnostics.orientation_tuples,
            "contactAttempts": diagnostics.contact_attempts,
            "exactPairRows": diagnostics.exact_pair_rows,
            "retainedTemplates": diagnostics.retained_templates,
            "transformedSourceVertices": diagnostics.transformed_source_vertices,
            "offsetOutputVertices": diagnostics.offset_output_vertices,
            "intersectionInputVertices": diagnostics.intersection_input_vertices,
            "intersectionOutputVertices": diagnostics.intersection_output_vertices,
            "transientRejectedOutputVertices": diagnostics.transient_rejected_output_vertices,
        }))
    } else {
        None
    };
    let pair_constructor_probe = if pair_constructor_diagnostics {
        let started = Instant::now();
        let experiment = diagnose_congruent_pair_constructor(&pieces, settings)?;
        Some(json!({
            "elapsedMs": started.elapsed().as_secs_f64() * 1_000.0,
            "templates": {
                "eligiblePairs": experiment.templates.eligible_pairs,
                "pairsWithTemplates": experiment.templates.pairs_with_templates,
                "fallbackPairs": experiment.templates.fallback_pairs,
                "retainedTemplates": experiment.templates.retained_templates,
            },
            "control": pair_cluster_arm_json(&experiment.control),
            "treatment": pair_cluster_arm_json(&experiment.treatment),
        }))
    } else {
        None
    };

    let mut elapsed_ms = Vec::with_capacity(runs);
    let mut result = None;
    let mut relaxed_diagnostics = None::<GeneralRelaxedDiagnostics>;
    let mut constructed_depth_mm = None;
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
                relaxed_settings.persistent_vacancy_target_depth_mm =
                    persistent_vacancy_target_depth_mm;
                let outcome = improve_complete_layout_with_pinned_vacancy_parent(
                    &pieces,
                    settings,
                    relaxed_settings,
                    &constructed,
                    pinned_vacancy_parent.as_ref(),
                )?;
                Ok((
                    outcome.result,
                    Some(outcome.diagnostics),
                    constructed_depth_mm,
                ))
            })?;
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
    let collision_expansion_mm =
        settings.total_padding_mm / 2.0 + settings.clearance_safety_margin_mm + 0.002;
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
            "rustflags": env::var("RUSTFLAGS").ok(),
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
            "quota": {
                "orderVariants": order_variants,
                "exploratoryEvaluationsPerPiece": exploratory_evaluations,
                "repairTargets": repair_targets,
                "repairEvaluationsPerPiece": repair_evaluations,
                "localAngleEvaluationsPerPiece": local_angle_evaluations,
                "catalogVariants": catalog_variants,
                "catalogEvaluationsPerPiece": catalog_evaluations,
                "pairingEvaluationsPerPiece": pairing_evaluations,
                "pairingBandVariants": pairing_band_variants,
                "partialLayouts": partial_layouts,
                "beamEvaluationsPerState": beam_evaluations,
                "angleSeedCount": angle_seed_count,
                "maxAnglesPerPiece": max_angles_per_piece,
                "tighteningPasses": tightening_passes,
                "relaxedEpochs": relaxed_epochs,
                "relaxedLanes": relaxed_lanes,
                "relaxedSweepsPerEpoch": relaxed_sweeps,
                "relaxedGlobalSamplesPerMove": relaxed_global_samples,
                "relaxedFocusedSamplesPerMove": relaxed_focused_samples,
                "relaxedRefinementRounds": relaxed_refinement_rounds,
                "relaxedSeed": relaxed_seed,
                "relaxedInitialShrinkRatio": relaxed_initial_shrink_ratio,
                "relaxedMinimumShrinkRatio": relaxed_minimum_shrink_ratio,
                "relaxedFailedAttemptsPerDepth": relaxed_failed_attempts_per_depth,
                "relaxedInfeasiblePoolSize": relaxed_infeasible_pool_size,
                "relaxedInfeasiblePoolArgumentsIgnored": true,
                "relaxedSynchronizeLanes": relaxed_synchronize_lanes,
                "relaxedDynamicHazard": relaxed_dynamic_hazard,
                "relaxedAngleSeedPolicy": if relaxed_continuous_seeds { "continuousUniform" } else { "structuredGrid" },
                "relaxedPressureModel": pressure_model_name(relaxed_pressure_model),
                "relaxedAngularRepair": relaxed_angular_repair,
                "relaxedRepairNeighborhood": relaxed_repair_neighborhood,
                "pairTemplateDiagnostics": pair_template_diagnostics,
                "pairConstructorDiagnostics": pair_constructor_diagnostics,
                "precompressionFrontierVacancyMode": precompression_frontier_vacancy_mode,
                "exactPairTerminalMode": retired_exact_pair_terminal_mode,
                "persistentVacancyMode": persistent_vacancy_mode,
            },
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
    if let Some(pinned) = &pinned_vacancy_parent {
        output["quota"]["persistentVacancyParentFixture"] = json!({
            "path": pinned.source,
            "sha256": pinned.source_sha256,
        });
    }
    if let Some(target) = persistent_vacancy_target_depth_mm {
        output["quota"]["persistentVacancyTargetDepthMm"] = json!(target);
    }
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PinnedVacancyParentFixture {
    #[serde(rename = "schemaVersion")]
    _schema_version: u64,
    #[serde(rename = "description")]
    _description: String,
    request_sha256: String,
    #[serde(rename = "expectedPlacementFingerprint")]
    _expected_placement_fingerprint: String,
    #[serde(rename = "reportedDepthMm")]
    _reported_depth_mm: f64,
    #[serde(rename = "independentDepthMm")]
    _independent_depth_mm: f64,
    #[serde(rename = "provenance")]
    _provenance: serde_json::Value,
    placements: Vec<PinnedVacancyPlacementFixture>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PinnedVacancyPlacementFixture {
    piece_id: String,
    rotation_deg: f64,
    mirrored: bool,
    translate_short_axis: f64,
    translate_long_axis: f64,
}

/// Loads a committed persistent-vacancy parent fixture. The fixture only
/// supplies parent placements; the engine's compiled-in frozen fingerprint and
/// depth checks remain the acceptance authority for the loaded layout.
fn load_pinned_vacancy_parent(
    path: &str,
    request_sha256: &str,
) -> Result<GeneralPersistentVacancyPinnedParent, Box<dyn std::error::Error>> {
    let bytes = fs::read(Path::new(path))?;
    let source_sha256 = format!("{:x}", Sha256::digest(&bytes));
    let fixture: PinnedVacancyParentFixture = serde_json::from_slice(&bytes)?;
    if fixture.request_sha256 != request_sha256 {
        return Err(format!(
            "persistent vacancy parent fixture {} pins request {}, but the current request hashes to {}",
            path, fixture.request_sha256, request_sha256
        )
        .into());
    }
    Ok(GeneralPersistentVacancyPinnedParent {
        placements: fixture
            .placements
            .into_iter()
            .map(|placement| GeneralFastPlacement {
                piece_id: placement.piece_id,
                rotation_deg: placement.rotation_deg,
                mirrored: placement.mirrored,
                translate_short_axis: placement.translate_short_axis,
                translate_long_axis: placement.translate_long_axis,
            })
            .collect(),
        source: path.to_owned(),
        source_sha256,
    })
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

fn pair_cluster_arm_json(diagnostics: &GeneralPairClusterArmDiagnostics) -> serde_json::Value {
    json!({
        "placed": diagnostics.result.as_ref().map(|result| result.placements.len()),
        "usedLongAxisDepthMm": diagnostics.result.as_ref().map(|result| result.used_long_axis_depth_mm),
        "bandVariantsAttempted": diagnostics.band_variants_attempted,
        "completedBands": diagnostics.completed_bands,
        "bandFailures": diagnostics.band_failures,
        "proposalAttempts": diagnostics.proposal_attempts,
        "generatedProposals": diagnostics.generated_proposals,
        "exactChildFixedVisits": diagnostics.exact_child_fixed_visits,
        "exactCandidateRows": diagnostics.exact_candidate_rows,
    })
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
