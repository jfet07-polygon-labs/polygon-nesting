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
use polygon_nesting_core::profiling::{self, ProfileSnapshot};
use polygon_nesting_core::quality_trace;
use polygon_nesting_core::search::general_fast::GeneralFastPlacement;
use polygon_nesting_core::search::general_fast::{
    construct_short_side_first, diagnose_congruent_pair_constructor,
    diagnose_congruent_pair_templates, GeneralFastPiece, GeneralFastSettings,
    GeneralPairClusterArmDiagnostics, DEFAULT_SEARCH_OFFSET_ALLOWANCE_MM,
};
use polygon_nesting_core::search::general_relaxed::{
    general_placement_fingerprint, improve_complete_layout_with_pinned_vacancy_parent,
    GeneralAngularRepairSettings, GeneralPersistentVacancyDiagnostics,
    GeneralPersistentVacancyPinnedParent, GeneralRelaxedAngleSeedPolicy,
    GeneralRelaxedCollisionBackend, GeneralRelaxedDiagnostics, GeneralRelaxedPressureModel,
    GeneralRelaxedSettings,
};
use polygon_nesting_core::search::portfolio::{
    self, PortfolioBudget, PortfolioOutcome, PortfolioSettings,
};
use polygon_nesting_core::search::shadow_rescore;
use polygon_nesting_core::validation::general_polygon::{
    raw_source_long_axis_depth_mm, GeneralPlacement,
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

/// A counting allocator, installed only when the `profiling-allocator` feature
/// is on, so that the default benchmark build allocates exactly as it always
/// did and the phase-counter overhead can be measured on its own.
#[cfg(feature = "profiling-allocator")]
#[global_allocator]
static ALLOCATOR: polygon_nesting_core::profiling::CountingAllocator<std::alloc::System> =
    polygon_nesting_core::profiling::CountingAllocator::new(std::alloc::System);

/// Renders a profile snapshot as the benchmark's `searchProfile` block.
///
/// Phase shares are quoted against the leaf-phase total (see
/// [`ProfileSnapshot::leaf_nanos`]) because enclosing phases double-count the
/// spans they contain; both are reported so the reader can check that.
fn search_profile_json(snapshot: &ProfileSnapshot) -> serde_json::Value {
    let leaf_nanos = snapshot.leaf_nanos();
    let phases = snapshot
        .phases
        .iter()
        .map(|sample| {
            json!({
                "phase": sample.phase.name(),
                "enclosing": sample.phase.is_enclosing(),
                "milliseconds": sample.nanos as f64 / 1_000_000.0,
                "calls": sample.calls,
                "leafSharePercent": if sample.phase.is_enclosing() || leaf_nanos == 0 {
                    serde_json::Value::Null
                } else {
                    json!(sample.nanos as f64 * 100.0 / leaf_nanos as f64)
                },
            })
        })
        .collect::<Vec<_>>();
    let counters = snapshot
        .counters
        .iter()
        .map(|sample| (sample.counter.name().to_owned(), json!(sample.value)))
        .collect::<serde_json::Map<_, _>>();
    json!({
        "threads": snapshot.threads,
        "leafMilliseconds": leaf_nanos as f64 / 1_000_000.0,
        "phases": phases,
        "counters": counters,
        "allocatorInstalled": cfg!(feature = "profiling-allocator"),
        "deepOperatorsInstrumented": profiling::deep::COMPILED_IN,
    })
}

/// Whether the harness should record a profile.
///
/// This is read from the environment rather than from a CLI slot on purpose:
/// the positional argument list is a pinned contract that replay drivers
/// depend on, and profiling must never change what a replayed command means.
fn profiling_requested() -> bool {
    env::var("POLYGON_NESTING_PROFILE")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Whether the pinned-parent band may descend from this process's own coupled
/// arm instead of a fixture.
///
/// Read from the environment for the same reason profiling is: the positional
/// argument list is a pinned contract that replay drivers depend on, and the
/// meaning of a replayed command may not change. A run armed this way is a
/// from-request measurement, not a replay, and it must never be quoted against
/// a pinned number - see
/// `GeneralRelaxedSettings::persistent_vacancy_allow_unpinned_parent`.
fn unpinned_vacancy_parent_requested() -> bool {
    env::var("POLYGON_NESTING_UNPINNED_VACANCY_PARENT")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let profiling_armed = profiling_requested();
    let unpinned_vacancy_parent_armed = unpinned_vacancy_parent_requested();
    profiling::set_enabled(profiling_armed);
    let mut arguments = env::args().skip(1);
    let request_path = arguments.next().ok_or(
        "usage: general_request_benchmark REQUEST.json [runs] [order-variants] [exploratory-evaluations-per-piece] [repair-targets] [repair-evaluations-per-piece] [local-angle-evaluations-per-piece] [catalog-variants] [catalog-evaluations-per-piece] [pairing-evaluations-per-piece] [pairing-band-variants] [partial-layouts] [beam-evaluations-per-state] [angle-seed-count] [max-angles-per-piece] [threads] [sheet-long-axis-override-mm] [tightening-passes] [sheet-edge-clearance-mm] [pair-clearance-mm] [relaxed-epochs] [relaxed-lanes] [relaxed-sweeps] [relaxed-global-samples] [relaxed-focused-samples] [relaxed-refinement-rounds] [relaxed-seed] [relaxed-initial-shrink-ratio] [relaxed-minimum-shrink-ratio] [relaxed-failed-attempts-per-depth] [relaxed-infeasible-pool-size] [relaxed-synchronize-lanes] [relaxed-dynamic-hazard] [relaxed-continuous-seeds] [relaxed-pressure-model] [relaxed-angular-repair] [relaxed-repair-neighborhood] [coupled-dynamic-separator] [pair-template-diagnostics] [pair-constructor-diagnostics] [precompression-frontier-vacancy] [exact-pair-terminal] [persistent-vacancy] [persistent-vacancy-parent-fixture] [persistent-vacancy-target-depth-mm] [warm-start-fixture] [search-offset-allowance-mm] [portfolio-spec]",
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
    if persistent_vacancy_mode > 33 {
        return Err("persistent vacancy mode must be between 0 and 33".into());
    }
    // An empty string means "no pinned parent", which is what a from-request
    // run needs: the deep operators then descend from the coupled arm this
    // same process produced instead of from a fixture. The filter follows the
    // warm-start slot's precedent exactly - an empty path was never a loadable
    // fixture, so no previously valid invocation changes meaning - and it is
    // what lets a positional target be supplied without arming this slot.
    let persistent_vacancy_parent_fixture = arguments.next().filter(|value| !value.is_empty());
    // Modes 22 (alternation fixpoint), 23 (recombination), 24 (bounded-depth
    // reinsertion) and 26 (clamped-sheet ladder compression) reinterpret this
    // argument: mode 22 treats it as the starting target depth (mm) for the
    // descent arm; mode 23 treats it as a scale-free cut fraction in (0, 1)
    // of parent A's own measured short-axis span; mode 24 treats it as an
    // absolute depth bound (mm) that no reinserted pose may exceed, which is
    // only meaningful below the parent layout's own depth; mode 26 treats it
    // as the final effective sheet long axis (mm) its bound ladder walks down
    // to, likewise only meaningful below the parent layout's own depth. Every
    // other mode treats it as an absolute target depth (mm), unchanged. Mode
    // 25 is mode 20's skyline constructor with the off-beam best-ever
    // expansion parent armed and takes the same arguments as mode 20. Mode 27
    // (standalone micro-legalization probe) ignores this argument entirely: it
    // measures the parent's residue against the real request and attempts the
    // repair pass on it as-is, with no bound and no ladder. Mode 28
    // (standalone conflict-targeted re-placement) treats it as the clamped
    // sheet long axis (mm) every re-placed pose must fit inside: it ejects the
    // pieces incident to the parent's clearance violations and rebuilds them
    // under that clamp. Mode 29 (standalone joint multi-piece re-placement)
    // reads it the same way as mode 28, but ejects every piece of each
    // pair-bearing violation component rather than a vertex cover of it and
    // searches over the insertion orders of that whole set. Mode 30 (global
    // pressure-balanced legalization) ignores this argument the way mode 27
    // does and solves the parent under the request's own sheet; mode 31 reads
    // it as a hard depth bound that enters the global program as a containment
    // constraint on every piece, which is the tier a mode-26 rung runs. Modes
    // 32 and 33 are modes 28 and 29 with the orientation-perturbation
    // candidate stream armed - each ejected piece's vacated pose is also
    // offered at a continuous ladder of nearby angles, and at the mirror flip
    // where the request allows one - and read this argument exactly as 28 and
    // 29 do.
    //
    // An empty string means "no target", following the two fixture slots'
    // precedent exactly: an empty string was never a parseable depth, so no
    // previously valid invocation changes meaning, and it is what lets a later
    // positional argument be supplied without arming this one.
    let persistent_vacancy_target_depth_mm = arguments
        .next()
        .filter(|value| !value.is_empty())
        .map(|value| value.parse::<f64>())
        .transpose()
        .map_err(|error| format!("persistent vacancy target depth: {error}"))?;
    // Optional warm-start fixture (arg 46): when present, its placements
    // replace the short-side-first construction as the incumbent handed to
    // the relaxed engine, so the legacy continuous separator explores from
    // an externally constructed complete layout. Absent, behavior is
    // byte-identical to the protected default. Mode 23 (recombination) also
    // reuses this same fixture as parent B for the crossover.
    //
    // An empty string means "no warm start", which lets a later positional
    // argument be supplied without arming this one. An empty path was never a
    // loadable fixture, so no previously valid invocation changes meaning.
    let warm_start_fixture_path = arguments.next().filter(|value| !value.is_empty());
    // Optional search-envelope allowance (arg 47), in millimetres. The search
    // envelope offsets every collision polygon by
    // `total_padding / 2 + clearance_safety_margin + allowance`; the allowance
    // is a conservative buffer that makes the envelope a strict superset of
    // the requested clearance contract. Publication is validated separately
    // and exactly, so lowering this only widens the set of legal placements
    // search may visit - it never relaxes what may be published. `0` removes
    // the allowance entirely. Absent, it defaults to the historical
    // `DEFAULT_SEARCH_OFFSET_ALLOWANCE_MM`, so every existing invocation is
    // unchanged.
    let search_offset_allowance_mm =
        parse_optional_f64(&mut arguments, DEFAULT_SEARCH_OFFSET_ALLOWANCE_MM)?;
    if !search_offset_allowance_mm.is_finite() || search_offset_allowance_mm < 0.0 {
        return Err("search offset allowance must be finite and non-negative".into());
    }
    // Optional portfolio-coordinator spec (arg 48): a `key=value` list that
    // arms the PR7 anytime coordinator in place of the single-mode run above.
    // Absent or empty leaves every existing invocation byte-identical - the
    // coordinator is a different entry point, not a different default.
    //
    //   wall=<ms>      wall-clock budget; the demo mode
    //   work=<units>   work-unit budget; the reproducible mode
    //   slots=<n>      salted constructor basin slots
    //   cells=a:b:c    void-grid cell divisor salts, cycled over the slots
    //   states=<n>     distinct archive states the descent phase spends on
    //   cycles=<n>     alternation cycles per descent quantum
    //   epochs=<n>     relaxed epochs inside a descent quantum
    //   archive=<n>    archive capacity
    //   overlap=<f>    piece-assignment overlap at which two layouts are the
    //                  same basin
    let portfolio_spec = arguments.next().filter(|value| !value.is_empty());
    if runs == 0 || arguments.next().is_some() {
        return Err("runs must be positive and no extra arguments are accepted".into());
    }
    // The relaxed configuration, assembled once. It used to be built inside the
    // measured closure; hoisting it changes no field and no order, and it is
    // what lets the portfolio coordinator run its protected mode-0 phase under
    // *this* configuration rather than under a rebuilt approximation of it.
    let relaxed_settings_template = {
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
        relaxed_settings.persistent_vacancy_target_depth_mm = persistent_vacancy_target_depth_mm;
        relaxed_settings.persistent_vacancy_allow_unpinned_parent = unpinned_vacancy_parent_armed;
        relaxed_settings
    };
    let portfolio_settings = portfolio_spec
        .as_deref()
        .map(|spec| parse_portfolio_spec(spec, relaxed_settings_template))
        .transpose()?;
    if portfolio_settings.is_some() && persistent_vacancy_mode != 0 {
        return Err(
            "the portfolio coordinator schedules its own operators; leave the persistent-vacancy mode at 0"
                .into(),
        );
    }

    let bytes = fs::read(Path::new(&request_path))?;
    let request_sha256 = format!("{:x}", Sha256::digest(&bytes));
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
    settings.search_offset_allowance_mm = search_offset_allowance_mm;
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
    let effective_edge_clearance_mm = settings
        .sheet_edge_clearance_mm
        .unwrap_or(settings.total_padding_mm / 2.0);
    let effective_parent_settings = PinnedVacancyEffectiveSettings {
        sheet_short_axis_mm: settings.sheet_short_axis_mm,
        sheet_long_axis_mm: settings.sheet_long_axis_mm,
        total_padding_mm: settings.total_padding_mm,
        sheet_edge_clearance_mm: effective_edge_clearance_mm,
        clearance_safety_margin_mm: settings.clearance_safety_margin_mm,
        flattening_sag_tolerance_mm: settings.flattening_sag_tolerance_mm,
        search_offset_allowance_mm: settings.search_offset_allowance_mm,
    };
    let pinned_vacancy_parent = persistent_vacancy_parent_fixture
        .as_deref()
        .map(|path| {
            load_pinned_vacancy_parent(path, &request_sha256, &effective_parent_settings, &owned)
        })
        .transpose()?;
    let warm_start_incumbent = warm_start_fixture_path
        .as_deref()
        .map(|path| -> Result<_, Box<dyn std::error::Error>> {
            let parent = load_pinned_vacancy_parent(
                path,
                &request_sha256,
                &effective_parent_settings,
                &owned,
            )?;
            let raw: serde_json::Value = serde_json::from_slice(&fs::read(Path::new(path))?)?;
            let depth = raw
                .get("independentDepthMm")
                .and_then(serde_json::Value::as_f64)
                .ok_or("warm-start fixture is missing independentDepthMm")?;
            Ok((parent, depth))
        })
        .transpose()?;
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
    // Everything above is request loading and probe setup; the measured stream
    // starts here, so the profile starts here too.
    profiling::reset();
    shadow_rescore::reset();
    // The quality frontier trace's clock starts on the same line, so `t = 0`
    // in the event stream is the same instant the wall-clock measurement
    // begins. Opening the sink switches profiling recording on, which is why
    // it happens after `profiling::reset()` and not before.
    let quality_trace_armed = quality_trace::init(&format!(
        "\"request\":\"{request_sha256}\",\"pieces\":{},\"relaxedSeed\":{relaxed_seed},\
         \"persistentVacancyMode\":{persistent_vacancy_mode},\"relaxedEpochs\":{relaxed_epochs},\
         \"coupledDynamicSeparator\":{coupled_dynamic_separator},\"threads\":{threads},\
         \"pinnedParent\":{},\"warmStart\":{},\"runs\":{runs},\"portfolio\":{}",
        pieces.len(),
        pinned_vacancy_parent.is_some(),
        warm_start_incumbent.is_some(),
        portfolio_settings.is_some(),
    ));
    let mut portfolio_report = None::<serde_json::Value>;
    for _ in 0..runs {
        let started = Instant::now();
        if let Some(portfolio_settings) = portfolio_settings.as_ref() {
            // The coordinator owns the whole run: it constructs, runs the
            // protected mode-0 phase itself, and schedules the operators. It
            // returns the mode-0 diagnostics unchanged so the rest of this
            // report describes exactly what a mode-0 run's report describes.
            let outcome = job_pool
                .run_scoped(|| portfolio::run_portfolio(&pieces, settings, portfolio_settings))?;
            elapsed_ms.push(started.elapsed().as_secs_f64() * 1_000.0);
            constructed_depth_mm.get_or_insert(outcome.constructed_depth_mm);
            let current = outcome.result.clone();
            let current_relaxed_diagnostics = (*outcome.m0_diagnostics).clone();
            portfolio_report.get_or_insert_with(|| portfolio_report_json(&outcome));
            if let Some(reference) = &result {
                if reference != &current {
                    return Err("deterministic replay produced different results".into());
                }
            } else {
                result = Some(current);
            }
            if let Some(reference) = &relaxed_diagnostics {
                if reference != &current_relaxed_diagnostics {
                    return Err(
                        "deterministic relaxed replay produced different diagnostics".into(),
                    );
                }
            } else {
                relaxed_diagnostics = Some(current_relaxed_diagnostics);
            }
            continue;
        }
        let (current, current_relaxed_diagnostics, current_constructed_depth_mm) = job_pool
            .run_scoped(|| {
                let mut constructed = construct_short_side_first(&pieces, settings)?;
                if let Some((warm, warm_depth)) = warm_start_incumbent.as_ref() {
                    constructed.placements = warm.placements.clone();
                    constructed.unplaced_piece_ids.clear();
                    constructed.used_long_axis_depth_mm = *warm_depth;
                }
                let constructed_depth_mm = constructed.used_long_axis_depth_mm;
                if relaxed_epochs == 0 {
                    return Ok::<_, polygon_nesting_core::search::general_fast::GeneralFastError>(
                        (constructed, None, constructed_depth_mm),
                    );
                }
                let relaxed_settings = relaxed_settings_template;
                let outcome = improve_complete_layout_with_pinned_vacancy_parent(
                    &pieces,
                    settings,
                    relaxed_settings,
                    &constructed,
                    pinned_vacancy_parent.as_ref(),
                    warm_start_incumbent.as_ref().map(|(parent, _)| parent),
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
    // The trace closes before any reporting work, so no event carries a
    // timestamp that includes JSON serialisation of the result document.
    quality_trace::finish();
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
    let collision_expansion_mm = settings.total_padding_mm / 2.0
        + settings.clearance_safety_margin_mm
        + settings.search_offset_allowance_mm;
    let expanded_collision_area_mm2 = owned.iter().try_fold(0.0, |area, piece| {
        Ok::<_, Box<dyn std::error::Error>>(
            area + piece.polygon.offset(collision_expansion_mm)?.area_mm2(),
        )
    })?;
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
    // Computed before `relaxed_diagnostics` is moved into the JSON output
    // below: when a persistent-vacancy mode was requested but the arm
    // declined to run, the process must fail closed even though the JSON is
    // still emitted on stdout (see the exit-code check after printing).
    let persistent_vacancy_unrun_reason = (persistent_vacancy_mode > 0)
        .then(|| {
            persistent_vacancy_unrun_reason(
                relaxed_diagnostics
                    .as_ref()
                    .and_then(|diagnostics| diagnostics.coupled_dynamic_separator.as_ref())
                    .and_then(|diagnostics| diagnostics.persistent_vacancy_population.as_ref()),
            )
        })
        .flatten();
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
                "searchOffsetAllowanceMm": settings.search_offset_allowance_mm,
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
    if let Some((warm, warm_depth)) = &warm_start_incumbent {
        output["quota"]["warmStartFixture"] = json!({
            "path": warm.source,
            "sha256": warm.source_sha256,
            "depthMm": warm_depth,
        });
    }
    // The profile block is emitted only when profiling was armed, so an
    // unprofiled run's report is byte-identical to what it was before this
    // harness existed and every pinned normalization keeps working.
    if profiling_armed {
        output["searchProfile"] = search_profile_json(&profiling::snapshot());
    }
    // A traced run says so in its own report, and says how much of the stream
    // it saw. The block appears only when a sink was actually opened, so an
    // untraced run's report is byte-identical either way - including in a
    // build that carries the feature.
    // A run that descended from an in-process parent says so in its own
    // report, unconditionally, so no artifact of one can be read as a replay.
    if unpinned_vacancy_parent_armed {
        output["unpinnedVacancyParent"] = json!(true);
    }
    // The coordinator's own report. Present only when the coordinator ran, so
    // every existing invocation's document is unchanged.
    if let Some(report) = portfolio_report {
        output["portfolio"] = report;
    }
    if quality_trace_armed {
        output["qualityTrace"] = json!({
            "schemaVersion": quality_trace::SCHEMA_VERSION,
            "sink": env::var(quality_trace::SINK_ENV).unwrap_or_default(),
            "proxySurvivors": quality_trace::proxy_survivor_total(),
            "deepCountersCompiledIn": profiling::deep::COMPILED_IN,
        });
    }
    // The shadow-rescore audit reports unconditionally in a build that carries
    // it, because its whole point is to be read: a run that audited and said
    // nothing would be indistinguishable from a run that never audited. A
    // build without the feature emits nothing, so unprofiled default reports
    // stay byte-identical.
    if shadow_rescore::COMPILED_IN {
        let audit = shadow_rescore::snapshot();
        output["shadowRescore"] = json!({
            "checks": audit.checks,
            "structuralDisagreements": audit.structural_disagreements,
            "magnitudeOnlyAudits": audit.magnitude_only_audits,
            "maxMagnitudeUlps": audit.max_magnitude_ulps,
            "derivedGapAudits": audit.derived_gap_audits,
            "maxDerivedUlps": audit.max_derived_ulps,
            "firstStructuralDisagreement": audit.first_structural_disagreement,
            "firstMagnitudeDisagreement": audit.first_magnitude_disagreement,
            "structuralDetails": audit.structural_details,
        });
    }
    // The constructor census reports unconditionally in a build that carries
    // it, for the reason the shadow-rescore audit does: a counting build that
    // counted and said nothing would be indistinguishable from one that never
    // counted. A build without the feature emits nothing.
    #[cfg(feature = "constructor-census")]
    {
        output["constructorCensus"] = polygon_nesting_core::constructor_census::snapshot();
    }
    println!("{}", serde_json::to_string_pretty(&output)?);
    // Fail closed: a requested persistent-vacancy mode that never ran (the
    // machinery declined the arm, e.g. invalid parent or failed validation)
    // must not exit 0. Callers that only check depth fields for null would
    // otherwise silently treat a skipped arm as a completed one.
    if let Some(reason) = persistent_vacancy_unrun_reason {
        eprintln!(
            "requested persistent-vacancy mode {persistent_vacancy_mode} did not run: {reason}"
        );
        std::process::exit(1);
    }
    Ok(())
}

/// A committed persistent-vacancy parent fixture.
///
/// # What a fixture is allowed to claim, and what is checked
///
/// Writing fixtures is out of scope for this harness - external scripts do
/// that - so every claim a fixture makes is re-derived here, on load, from the
/// fixture's own placements and the current run's request and CLI arguments. A
/// fixture that pins a request but replays under a different *contract* is the
/// failure mode this guards: the request JSON's `padding` is not necessarily
/// the clearance the run uses, because arguments 19/20 override the pair and
/// edge clearances and argument 47 sets the search allowance.
///
/// ```text
/// {
///   "schemaVersion": <u64>,
///   "description": <string>,
///   "requestSha256": <hex>,                 // checked against the request
///   "expectedPlacementFingerprint": <hex>,  // checked when it is a real digest
///   "reportedDepthMm": <f64>,               // checked: may not understate
///   "independentDepthMm": <f64>,            // checked: must land on a
///                                           //   convention in MeasuredDepths
///   "provenance": <any>,
///   "settings": {                           // optional; every field checked
///     "sheetShortAxisMm": <f64>,
///     "sheetLongAxisMm": <f64>,
///     "totalPaddingMm": <f64>,              // pair clearance
///     "sheetEdgeClearanceMm": <f64>,        // edge clearance
///     "clearanceSafetyMarginMm": <f64>,     // margin
///     "flatteningSagToleranceMm": <f64>,    // sag
///     "searchOffsetAllowanceMm": <f64>      // optional within the block
///   },
///   "placements": [ { "pieceId", "rotationDeg", "mirrored",
///                     "translateShortAxis", "translateLongAxis" }, ... ]
/// }
/// ```
///
/// Fixtures that omit `settings` - which is every fixture written before the
/// block existed - load exactly as they always did. Fixture-writing tools
/// should emit it, and should emit `searchOffsetAllowanceMm` inside it, so that
/// a replay under a different allowance is a hard error at load rather than a
/// confusing rejection deep inside the arm.
///
/// The engine's own frozen fingerprint and depth checks remain the acceptance
/// authority for the loaded layout; these checks only establish that the
/// fixture describes the layout it says it does, under the contract it says it
/// was recorded under.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PinnedVacancyParentFixture {
    #[serde(rename = "schemaVersion")]
    _schema_version: u64,
    #[serde(rename = "description")]
    _description: String,
    request_sha256: String,
    expected_placement_fingerprint: String,
    reported_depth_mm: f64,
    independent_depth_mm: f64,
    #[serde(rename = "provenance")]
    _provenance: serde_json::Value,
    #[serde(default)]
    settings: Option<PinnedVacancyParentSettingsFixture>,
    placements: Vec<PinnedVacancyPlacementFixture>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PinnedVacancyParentSettingsFixture {
    sheet_short_axis_mm: f64,
    sheet_long_axis_mm: f64,
    total_padding_mm: f64,
    sheet_edge_clearance_mm: f64,
    clearance_safety_margin_mm: f64,
    flattening_sag_tolerance_mm: f64,
    /// Optional within the block so that fixtures written before the search
    /// envelope was a tunable keep loading. When present it must equal the
    /// run's effective allowance: a layout found under a narrow envelope is
    /// still contract-valid under a wide one, but it is not *reachable* by the
    /// wide one's search, so replaying it there silently measures a different
    /// experiment.
    #[serde(default)]
    search_offset_allowance_mm: Option<f64>,
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

/// The current run's effective geometry-relevant settings, used to validate
/// a persistent-vacancy parent fixture's optional settings block (see
/// `PinnedVacancyParentFixture`) against the geometry contract the current
/// request and CLI arguments actually produce.
struct PinnedVacancyEffectiveSettings {
    sheet_short_axis_mm: f64,
    sheet_long_axis_mm: f64,
    total_padding_mm: f64,
    sheet_edge_clearance_mm: f64,
    clearance_safety_margin_mm: f64,
    flattening_sag_tolerance_mm: f64,
    search_offset_allowance_mm: f64,
}

/// Absolute tolerance (mm) for comparing a fixture's recorded settings
/// against the current run's effective settings. JSON round-trips f64
/// exactly, so this only absorbs floating-point noise from arithmetic (e.g.
/// halving a padding value), not any real geometry drift.
const PARENT_FIXTURE_SETTINGS_TOLERANCE_MM: f64 = 1e-9;

/// Absolute tolerance (mm) between a fixture's recorded depth and the nearest
/// depth its own placements actually measure.
///
/// This absorbs only the rounding a fixture writer applies when printing the
/// value - the corpus records three decimals. It deliberately does *not* absorb
/// the difference between the engine's depth conventions: those are enumerated
/// exactly by [`MeasuredDepths`], so widening this to cover them would have hidden
/// a genuinely wrong claim behind a fudge factor. Two canonical grid steps is
/// comfortably above the printing effect and three orders of magnitude below a
/// real disagreement, which is millimetres, not microns.
const PARENT_FIXTURE_DEPTH_TOLERANCE_MM: f64 = 0.002;

/// Whether a fixture's `expectedPlacementFingerprint` is an actual placement
/// digest rather than a human label.
///
/// Fixture-writing scripts routinely store a provenance tag there instead - the
/// committed corpus carries `alternation`, `crossover`, `hint-only`, `reseed`
/// and `true-exact-native` - and those cannot be checked against anything. A
/// real fingerprint is a SHA-256 in lowercase hex, so the shape alone
/// distinguishes them, and a fixture that *does* claim a digest is always
/// checked.
fn is_placement_fingerprint(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Loads a committed persistent-vacancy parent fixture, re-deriving every claim
/// it makes from its own placements before accepting it.
///
/// The fixture only supplies parent placements; the engine's compiled-in frozen
/// fingerprint and depth checks remain the acceptance authority for the loaded
/// layout. What happens here is narrower and earlier: it establishes that the
/// fixture's *metadata* describes the placements it carries, under the contract
/// the current run is actually configured with. A mismatch is a hard error, not
/// a warning - a fixture that pins the wrong thing produces a plausible-looking
/// trajectory for a different experiment.
fn load_pinned_vacancy_parent(
    path: &str,
    request_sha256: &str,
    effective_settings: &PinnedVacancyEffectiveSettings,
    pieces: &[OwnedPiece],
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
    if let Some(recorded) = &fixture.settings {
        check_parent_fixture_settings(recorded, effective_settings)?;
    }
    let placements = fixture
        .placements
        .into_iter()
        .map(|placement| GeneralFastPlacement {
            piece_id: placement.piece_id,
            rotation_deg: placement.rotation_deg,
            mirrored: placement.mirrored,
            translate_short_axis: placement.translate_short_axis,
            translate_long_axis: placement.translate_long_axis,
        })
        .collect::<Vec<_>>();
    check_parent_fixture_fingerprint(path, &fixture.expected_placement_fingerprint, &placements)?;
    check_parent_fixture_depths(
        path,
        fixture.reported_depth_mm,
        fixture.independent_depth_mm,
        &placements,
        effective_settings,
        pieces,
    )?;
    Ok(GeneralPersistentVacancyPinnedParent {
        placements,
        source: path.to_owned(),
        source_sha256,
    })
}

/// Recomputes the placement fingerprint and hard-errors when the fixture claims
/// a different one. Fixtures carrying a provenance label instead of a digest
/// are left alone; see `is_placement_fingerprint`.
fn check_parent_fixture_fingerprint(
    path: &str,
    claimed: &str,
    placements: &[GeneralFastPlacement],
) -> Result<(), Box<dyn std::error::Error>> {
    if !is_placement_fingerprint(claimed) {
        return Ok(());
    }
    let recomputed = general_placement_fingerprint(placements);
    if recomputed != claimed {
        return Err(format!(
            "parent fixture {path} claims placement fingerprint {claimed}, but its placements fingerprint to {recomputed}"
        )
        .into());
    }
    Ok(())
}

/// A layout's long-axis depth under every convention this codebase reports it
/// in, all taken from the same placements.
///
/// There are two independent axes of variation, and a fixture in the wild may
/// have recorded any combination of them.
///
/// **Quantization.** `source_snapped_mm` is what
/// `independentUsedLongAxisDepthMm` and the engine's own independent depth
/// carry: `PolygonSet::transformed(..)` rotates the canonical *integer-grid*
/// path and re-quantizes it, and `bounds()` then reads that grid.
/// `source_raw_mm` applies the same transform to the untouched `f64` source
/// rings and never quantizes. They differ by at most a couple of grid steps at
/// the deepest vertex.
///
/// **Envelope.** `usedLongAxisDepthMm` is measured on the *collision* polygons
/// rather than the source: each is the source offset by
/// `total_padding / 2 + clearance_safety_margin + search_offset_allowance`, and
/// the metric adds back the sheet inset `edge_clearance - total_padding / 2`.
/// The two halves of the pair clearance cancel, leaving exactly
///
/// ```text
/// usedLongAxisDepthMm = source depth + clearance_safety_margin + search_offset_allowance
/// ```
///
/// which is `envelope_excess_mm`. This is not a fudge factor: it is an identity,
/// and it is why an anchor recorded from `usedLongAxisDepthMm` reads 181.591
/// where its own geometry measures 181.589 at a 0.002 allowance.
///
/// Checking a claim against the nearest of these asks the question that matters
/// - "does this fixture describe the layout it carries" - instead of the one
/// that does not - "which field did its author copy". A fixture describing a
/// *different* layout is out by millimetres and is nowhere near any of them.
struct MeasuredDepths {
    source_snapped_mm: f64,
    source_raw_mm: f64,
    envelope_excess_mm: f64,
}

impl MeasuredDepths {
    /// Every depth this layout can legitimately be reported as.
    fn candidates(&self) -> [f64; 4] {
        [
            self.source_snapped_mm,
            self.source_raw_mm,
            self.source_snapped_mm + self.envelope_excess_mm,
            self.source_raw_mm + self.envelope_excess_mm,
        ]
    }

    /// How far a claimed depth sits from the nearest convention.
    fn distance_from(&self, claimed_mm: f64) -> f64 {
        self.candidates()
            .into_iter()
            .map(|candidate| (claimed_mm - candidate).abs())
            .fold(f64::INFINITY, f64::min)
    }

    /// The shallowest depth the layout can honestly be reported as, which is
    /// the floor `reportedDepthMm` may not sink below.
    fn shallowest_mm(&self) -> f64 {
        self.candidates().into_iter().fold(f64::INFINITY, f64::min)
    }
}

/// Measures a fixture's placements under every convention against the current
/// run's pieces and settings.
fn measure_parent_fixture_depths(
    path: &str,
    placements: &[GeneralFastPlacement],
    effective_settings: &PinnedVacancyEffectiveSettings,
    pieces: &[OwnedPiece],
) -> Result<MeasuredDepths, Box<dyn std::error::Error>> {
    let edge_clearance_mm = effective_settings.sheet_edge_clearance_mm;
    let pieces_by_id = pieces
        .iter()
        .map(|piece| (piece.id.as_str(), piece))
        .collect::<BTreeMap<_, _>>();
    let placed = placements
        .iter()
        .map(|placement| {
            let piece = pieces_by_id
                .get(placement.piece_id.as_str())
                .copied()
                .ok_or_else(|| {
                    format!(
                        "parent fixture {path} places unknown piece {}",
                        placement.piece_id
                    )
                })?;
            Ok::<_, Box<dyn std::error::Error>>((piece, placement))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let snapped_mm = placed
        .iter()
        .map(
            |(piece, placement)| -> Result<f64, Box<dyn std::error::Error>> {
                let transformed = piece.polygon.transformed(
                    placement.rotation_deg,
                    placement.mirrored,
                    placement.translate_short_axis,
                    placement.translate_long_axis,
                )?;
                let bounds = transformed
                    .bounds()
                    .ok_or("a fixture polygon must be non-empty")?;
                Ok(bounds.max_y + edge_clearance_mm)
            },
        )
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .max_by(f64::total_cmp)
        .ok_or("a fixture depth needs at least one placement")?;
    let raw_mm = raw_source_long_axis_depth_mm(
        &placed
            .iter()
            .map(|(piece, placement)| GeneralPlacement {
                piece_id: piece.id.as_str(),
                polygon: &piece.polygon,
                rotation_deg: placement.rotation_deg,
                mirrored: placement.mirrored,
                translate_x: placement.translate_short_axis,
                translate_y: placement.translate_long_axis,
            })
            .collect::<Vec<_>>(),
        edge_clearance_mm,
    )?;
    Ok(MeasuredDepths {
        source_snapped_mm: snapped_mm,
        source_raw_mm: raw_mm,
        envelope_excess_mm: effective_settings.clearance_safety_margin_mm
            + effective_settings.search_offset_allowance_mm,
    })
}

/// Recomputes the layout's depth from the fixture's own placements and checks
/// both recorded depth fields against it.
///
/// The two fields are not the same quantity and are not checked the same way:
///
/// - `independentDepthMm` claims to *be* the measured depth of these
///   placements, so it must land on one of the conventions in
///   [`MeasuredDepths`].
/// - `reportedDepthMm` is the depth the engine reported for the run that found
///   the layout, which is the strip it was found in. That is legitimately
///   deeper than the layout itself - several committed fixtures sit 0.26 mm
///   apart this way - so only the impossible direction is an error: a fixture
///   may not claim to be shallower than its own geometry.
fn check_parent_fixture_depths(
    path: &str,
    reported_depth_mm: f64,
    independent_depth_mm: f64,
    placements: &[GeneralFastPlacement],
    effective_settings: &PinnedVacancyEffectiveSettings,
    pieces: &[OwnedPiece],
) -> Result<(), Box<dyn std::error::Error>> {
    if placements.is_empty() {
        return Ok(());
    }
    let measured = measure_parent_fixture_depths(path, placements, effective_settings, pieces)?;
    if measured.distance_from(independent_depth_mm) > PARENT_FIXTURE_DEPTH_TOLERANCE_MM {
        return Err(format!(
            "parent fixture {path} claims independentDepthMm={independent_depth_mm}, but its placements measure {} (raw source {}, plus a {} mm search envelope)",
            measured.source_snapped_mm, measured.source_raw_mm, measured.envelope_excess_mm
        )
        .into());
    }
    if reported_depth_mm < measured.shallowest_mm() - PARENT_FIXTURE_DEPTH_TOLERANCE_MM {
        return Err(format!(
            "parent fixture {path} claims reportedDepthMm={reported_depth_mm}, which is shallower than the {} its placements measure",
            measured.shallowest_mm()
        )
        .into());
    }
    Ok(())
}

/// Compares a fixture's recorded geometry settings against the current
/// run's effective settings field-by-field, hard-erroring with a clear
/// `<field> fixture=<v> effective=<v>` message on the first mismatch.
fn check_parent_fixture_settings(
    recorded: &PinnedVacancyParentSettingsFixture,
    effective: &PinnedVacancyEffectiveSettings,
) -> Result<(), Box<dyn std::error::Error>> {
    let fields: [(&str, f64, f64); 6] = [
        (
            "sheetShortAxisMm",
            recorded.sheet_short_axis_mm,
            effective.sheet_short_axis_mm,
        ),
        (
            "sheetLongAxisMm",
            recorded.sheet_long_axis_mm,
            effective.sheet_long_axis_mm,
        ),
        (
            "totalPaddingMm",
            recorded.total_padding_mm,
            effective.total_padding_mm,
        ),
        (
            "sheetEdgeClearanceMm",
            recorded.sheet_edge_clearance_mm,
            effective.sheet_edge_clearance_mm,
        ),
        (
            "clearanceSafetyMarginMm",
            recorded.clearance_safety_margin_mm,
            effective.clearance_safety_margin_mm,
        ),
        (
            "flatteningSagToleranceMm",
            recorded.flattening_sag_tolerance_mm,
            effective.flattening_sag_tolerance_mm,
        ),
    ];
    let allowance = recorded.search_offset_allowance_mm.map(|recorded| {
        (
            "searchOffsetAllowanceMm",
            recorded,
            effective.search_offset_allowance_mm,
        )
    });
    for (field, fixture_value, effective_value) in fields.into_iter().chain(allowance) {
        if (fixture_value - effective_value).abs() > PARENT_FIXTURE_SETTINGS_TOLERANCE_MM {
            return Err(format!(
                "parent fixture settings mismatch: {field} fixture={fixture_value} effective={effective_value}"
            )
            .into());
        }
    }
    Ok(())
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

/// Returns the reason a CLI-requested persistent-vacancy mode failed to run
/// when it should have: either the population diagnostics block is absent
/// entirely, or it is present but `attempted` is false (the machinery
/// declined the arm, e.g. an invalid parent or failed validation). Returns
/// `None` when the arm actually ran, in which case its own `exactValid` and
/// depth fields are the authority on whether it *succeeded*, not whether it
/// *ran*; only the latter is this function's concern.
fn persistent_vacancy_unrun_reason(
    population: Option<&GeneralPersistentVacancyDiagnostics>,
) -> Option<String> {
    match population {
        None => Some("persistent-vacancy population diagnostics were not produced".to_owned()),
        Some(diagnostics) if !diagnostics.attempted => Some(
            diagnostics
                .failure_reason
                .clone()
                .unwrap_or_else(|| "no failure reason was recorded".to_owned()),
        ),
        Some(_) => None,
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

/// Parses the portfolio-coordinator spec (argument 48).
///
/// One argument rather than a dozen positional ones on purpose: every replay
/// driver in this repository pins the positional tail by index, and a schedule
/// gains knobs. Exactly one of `wall` and `work` must be given - the budget's
/// *currency* is the one thing the coordinator cannot default, because the two
/// modes make different promises about reproducibility.
fn parse_portfolio_spec(
    spec: &str,
    relaxed_template: GeneralRelaxedSettings,
) -> Result<PortfolioSettings, Box<dyn std::error::Error>> {
    let mut budget = None;
    let mut settings =
        PortfolioSettings::new(relaxed_template, PortfolioBudget::Wall { millis: 0 });
    for entry in spec.split(',').filter(|entry| !entry.is_empty()) {
        let (key, value) = entry
            .split_once('=')
            .ok_or_else(|| format!("portfolio spec entry {entry:?} is not key=value"))?;
        match key {
            "wall" => {
                budget = Some(PortfolioBudget::Wall {
                    millis: value.parse()?,
                })
            }
            "work" => {
                budget = Some(PortfolioBudget::Work {
                    units: value.parse()?,
                })
            }
            "slots" => settings.basin_slots = value.parse()?,
            "states" => settings.descent_states = value.parse()?,
            "cycles" => settings.descent_cycles = value.parse()?,
            "deepen" => settings.descent_iterated_deepening = value != "0",
            "epochs" => settings.descent_relaxed_epochs = value.parse()?,
            "archive" => settings.archive_capacity = value.parse()?,
            "overlap" => settings.similarity_threshold = value.parse()?,
            "cells" => {
                settings.cell_divisor_salts = value
                    .split(':')
                    .filter(|entry| !entry.is_empty())
                    .map(str::parse::<f64>)
                    .collect::<Result<Vec<_>, _>>()?;
            }
            "basinsBy" => settings.schedule.basins_by = value.parse()?,
            "descentBy" => settings.schedule.descent_by = value.parse()?,
            "crossoverBy" => settings.schedule.crossover_by = value.parse()?,
            "compressionBy" => settings.schedule.compression_by = value.parse()?,
            "drainBy" => settings.schedule.drain_by = value.parse()?,
            other => return Err(format!("unknown portfolio spec key {other:?}").into()),
        }
    }
    settings.budget = budget.ok_or("portfolio spec requires wall=<ms> or work=<units>")?;
    Ok(settings)
}

/// The coordinator's report, as JSON.
fn portfolio_report_json(outcome: &PortfolioOutcome) -> serde_json::Value {
    let budget = match outcome.budget {
        PortfolioBudget::Wall { millis } => json!({"kind": "wall", "millis": millis}),
        PortfolioBudget::Work { units } => json!({"kind": "work", "units": units}),
    };
    json!({
        "budget": budget,
        "elapsedSeconds": outcome.elapsed_seconds,
        "workUnits": outcome.work_units,
        "areaLowerBoundDepthMm": outcome.area_lower_bound_depth_mm,
        "constructorClampMm": outcome.constructor_clamp_mm,
        "constructedDepthMm": outcome.constructed_depth_mm,
        "incumbent": {
            "fingerprint": outcome.incumbent.fingerprint(),
            "rawDepthMm": outcome.incumbent.raw_depth_mm(),
            "dualGateValid": outcome.incumbent.dual_gate_valid(),
            "source": outcome.incumbent.source(),
            "publishedSeconds": outcome.incumbent.published_seconds(),
            "publishedWorkUnits": outcome.incumbent.published_work_units(),
        },
        "phases": outcome.phases.iter().map(|phase| json!({
            "name": phase.name,
            "deadlineFraction": phase.deadline_fraction,
            "enteredSeconds": phase.entered_seconds,
            "elapsedSeconds": phase.elapsed_seconds,
            "workUnits": phase.work_units,
            "operatorCalls": phase.operator_calls,
            "publications": phase.publications,
            "skipped": phase.skipped,
        })).collect::<Vec<_>>(),
        "publications": outcome.publications.iter().map(|event| json!({
            "seconds": event.seconds,
            "workUnits": event.work_units,
            "phase": event.phase,
            "source": event.source,
            "rawDepthMm": event.raw_depth_mm,
            "previousRawDepthMm": event.previous_raw_depth_mm,
            "fingerprint": event.fingerprint,
        })).collect::<Vec<_>>(),
        "operatorCalls": outcome.operator_calls.iter().map(|call| json!({
            "phase": call.phase,
            "operator": call.operator,
            "parentFingerprint": call.parent_fingerprint,
            "startedSeconds": call.started_seconds,
            "elapsedSeconds": call.elapsed_seconds,
            "workUnits": call.work_units,
            "exactValid": call.exact_valid,
            "rawDepthMm": call.raw_depth_mm,
            "archiveDisposition": call.archive_disposition,
            "published": call.published,
            "failureReason": call.failure_reason,
        })).collect::<Vec<_>>(),
        "archive": {
            "capacity": outcome.archive.capacity,
            "occupancy": outcome.archive.occupancy,
            "similarityThreshold": outcome.archive.similarity_threshold,
            "admitted": outcome.archive.admitted,
            "duplicates": outcome.archive.duplicates,
            "evicted": outcome.archive.evicted,
            "refusedArchiveFullAllDistinct": outcome.archive.refused_full,
            "refusedIncomplete": outcome.archive.refused_incomplete,
            "byOperator": outcome.archive.by_operator,
            "occupancyOverTime": outcome.archive.occupancy_over_time
                .iter()
                .map(|(seconds, occupancy)| json!([seconds, occupancy]))
                .collect::<Vec<_>>(),
            "members": outcome.archive.members.iter().map(|member| json!({
                "fingerprint": member.fingerprint,
                "rawDepthMm": member.raw_depth_mm,
                "birthSeconds": member.birth_seconds,
                "birthWorkUnits": member.birth_work_units,
                "operator": member.operator,
                "parentFingerprint": member.parent_fingerprint,
                "exactValid": member.exact_valid,
                "descents": member.descents,
            })).collect::<Vec<_>>(),
        },
    })
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

    fn write_temp_fixture(name: &str, contents: &serde_json::Value) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "general_request_benchmark_test_{name}_{}.json",
            std::process::id()
        ));
        std::fs::write(&path, serde_json::to_vec(contents).unwrap()).unwrap();
        path
    }

    fn sample_pinned_vacancy_fixture(settings: Option<serde_json::Value>) -> serde_json::Value {
        let mut fixture = json!({
            "schemaVersion": 1,
            "description": "test fixture",
            "requestSha256": "deadbeef",
            "expectedPlacementFingerprint": "fingerprint",
            "reportedDepthMm": 10.0,
            "independentDepthMm": 10.0,
            "provenance": {},
            "placements": [],
        });
        if let Some(settings) = settings {
            fixture["settings"] = settings;
        }
        fixture
    }

    fn sample_effective_settings() -> PinnedVacancyEffectiveSettings {
        PinnedVacancyEffectiveSettings {
            sheet_short_axis_mm: 300.0,
            sheet_long_axis_mm: 400.0,
            total_padding_mm: 5.0,
            sheet_edge_clearance_mm: 2.5,
            clearance_safety_margin_mm: 0.001,
            flattening_sag_tolerance_mm: 0.005,
            search_offset_allowance_mm: DEFAULT_SEARCH_OFFSET_ALLOWANCE_MM,
        }
    }

    /// One 2x4 mm piece, placed by the fixtures below at a known offset so the
    /// recomputed depth is arithmetic rather than a magic number.
    fn sample_owned_pieces() -> Vec<OwnedPiece> {
        vec![OwnedPiece {
            id: "piece".to_owned(),
            polygon: rectangle(2.0, 4.0),
            allow_rotation: false,
            allow_mirror: false,
        }]
    }

    fn sample_placement(translate_long_axis: f64) -> serde_json::Value {
        json!({
            "pieceId": "piece",
            "rotationDeg": 0.0,
            "mirrored": false,
            "translateShortAxis": 3.0,
            "translateLongAxis": translate_long_axis,
        })
    }

    /// The depth `sample_placement(y)` produces: the piece is 4 mm tall and the
    /// sample edge clearance is 2.5 mm.
    fn sample_depth(translate_long_axis: f64) -> f64 {
        translate_long_axis + 4.0 + 2.5
    }

    fn load_sample(
        path: &std::path::Path,
        effective: &PinnedVacancyEffectiveSettings,
    ) -> Result<GeneralPersistentVacancyPinnedParent, Box<dyn std::error::Error>> {
        let result = load_pinned_vacancy_parent(
            path.to_str().unwrap(),
            "deadbeef",
            effective,
            &sample_owned_pieces(),
        );
        std::fs::remove_file(path).ok();
        result
    }

    #[test]
    fn parent_fixture_without_settings_block_loads_unchanged() {
        let fixture = sample_pinned_vacancy_fixture(None);
        let path = write_temp_fixture("no_settings", &fixture);
        assert!(load_sample(&path, &sample_effective_settings()).is_ok());
    }

    #[test]
    fn parent_fixture_matching_settings_block_loads() {
        let effective = sample_effective_settings();
        let fixture = sample_pinned_vacancy_fixture(Some(json!({
            "sheetShortAxisMm": effective.sheet_short_axis_mm,
            "sheetLongAxisMm": effective.sheet_long_axis_mm,
            "totalPaddingMm": effective.total_padding_mm,
            "sheetEdgeClearanceMm": effective.sheet_edge_clearance_mm,
            "clearanceSafetyMarginMm": effective.clearance_safety_margin_mm,
            "flatteningSagToleranceMm": effective.flattening_sag_tolerance_mm,
        })));
        let path = write_temp_fixture("matching_settings", &fixture);
        assert!(load_sample(&path, &effective).is_ok());
    }

    #[test]
    fn parent_fixture_mismatched_settings_block_hard_errors() {
        let effective = sample_effective_settings();
        let fixture = sample_pinned_vacancy_fixture(Some(json!({
            "sheetShortAxisMm": effective.sheet_short_axis_mm,
            "sheetLongAxisMm": effective.sheet_long_axis_mm,
            "totalPaddingMm": 0.0,
            "sheetEdgeClearanceMm": effective.sheet_edge_clearance_mm,
            "clearanceSafetyMarginMm": effective.clearance_safety_margin_mm,
            "flatteningSagToleranceMm": effective.flattening_sag_tolerance_mm,
        })));
        let path = write_temp_fixture("mismatched_settings", &fixture);
        let error = load_sample(&path, &effective).unwrap_err();
        let expected = format!(
            "parent fixture settings mismatch: totalPaddingMm fixture={} effective={}",
            0.0_f64, effective.total_padding_mm
        );
        assert_eq!(error.to_string(), expected);
    }

    /// The allowance is the field that decides which layouts the search may
    /// visit, so a fixture recorded under a different one is replaying a
    /// different experiment even though every other setting agrees.
    #[test]
    fn parent_fixture_pins_the_search_offset_allowance_when_it_records_one() {
        let effective = sample_effective_settings();
        let settings_block = |allowance: serde_json::Value| {
            let mut block = json!({
                "sheetShortAxisMm": effective.sheet_short_axis_mm,
                "sheetLongAxisMm": effective.sheet_long_axis_mm,
                "totalPaddingMm": effective.total_padding_mm,
                "sheetEdgeClearanceMm": effective.sheet_edge_clearance_mm,
                "clearanceSafetyMarginMm": effective.clearance_safety_margin_mm,
                "flatteningSagToleranceMm": effective.flattening_sag_tolerance_mm,
            });
            if !allowance.is_null() {
                block["searchOffsetAllowanceMm"] = allowance;
            }
            sample_pinned_vacancy_fixture(Some(block))
        };

        // Absent: backward compatible, no opinion.
        let path = write_temp_fixture("allowance_absent", &settings_block(json!(null)));
        assert!(load_sample(&path, &effective).is_ok());

        // Present and equal.
        let path = write_temp_fixture(
            "allowance_match",
            &settings_block(json!(effective.search_offset_allowance_mm)),
        );
        assert!(load_sample(&path, &effective).is_ok());

        // Present and different: this is the record-fixture replay the split
        // exists to describe, and it must not load silently.
        let path = write_temp_fixture("allowance_mismatch", &settings_block(json!(0.0005)));
        let error = load_sample(&path, &effective).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "parent fixture settings mismatch: searchOffsetAllowanceMm fixture={} effective={}",
                0.0005_f64, effective.search_offset_allowance_mm
            )
        );
    }

    #[test]
    fn placement_fingerprint_labels_are_told_apart_from_digests() {
        assert!(is_placement_fingerprint(&"a".repeat(64)));
        assert!(is_placement_fingerprint(
            "9dea3f0d263be2878b7bfca84705118aed9bee39420387a06f42278d3351e69d"
        ));
        for label in [
            "alternation",
            "crossover",
            "hint-only",
            "reseed",
            "true-exact-native",
            "",
        ] {
            assert!(!is_placement_fingerprint(label), "{label}");
        }
        // Right length, wrong alphabet.
        assert!(!is_placement_fingerprint(&"A".repeat(64)));
        assert!(!is_placement_fingerprint(&"g".repeat(64)));
    }

    /// A fixture that claims a real digest is checked against its own
    /// placements; one that carries a provenance label is left alone.
    #[test]
    fn parent_fixture_placement_fingerprint_is_recomputed_when_it_is_a_digest() {
        let effective = sample_effective_settings();
        let mut fixture = sample_pinned_vacancy_fixture(None);
        fixture["placements"] = json!([sample_placement(3.5)]);
        fixture["reportedDepthMm"] = json!(sample_depth(3.5));
        fixture["independentDepthMm"] = json!(sample_depth(3.5));

        let truthful = general_placement_fingerprint(&[GeneralFastPlacement {
            piece_id: "piece".to_owned(),
            rotation_deg: 0.0,
            mirrored: false,
            translate_short_axis: 3.0,
            translate_long_axis: 3.5,
        }]);

        let mut honest = fixture.clone();
        honest["expectedPlacementFingerprint"] = json!(truthful);
        let path = write_temp_fixture("fingerprint_honest", &honest);
        assert!(load_sample(&path, &effective).is_ok());

        let mut lying = fixture.clone();
        lying["expectedPlacementFingerprint"] = json!("f".repeat(64));
        let path = write_temp_fixture("fingerprint_lying", &lying);
        let error = load_sample(&path, &effective).unwrap_err();
        assert!(
            error
                .to_string()
                .contains(&format!("its placements fingerprint to {truthful}")),
            "{error}"
        );

        // A provenance label is unverifiable and must stay loadable.
        let mut labelled = fixture;
        labelled["expectedPlacementFingerprint"] = json!("alternation");
        let path = write_temp_fixture("fingerprint_label", &labelled);
        assert!(load_sample(&path, &effective).is_ok());
    }

    /// `independentDepthMm` claims to be the layout's measured depth, so it is
    /// held to it in both directions.
    #[test]
    fn parent_fixture_independent_depth_is_recomputed_from_its_placements() {
        let effective = sample_effective_settings();
        let mut fixture = sample_pinned_vacancy_fixture(None);
        fixture["placements"] = json!([sample_placement(3.5)]);
        let truthful = sample_depth(3.5);

        for (claimed, ok) in [
            (truthful, true),
            (truthful + 0.0015, true),
            (truthful - 0.0015, true),
            (truthful + 0.5, false),
            (truthful - 0.5, false),
        ] {
            let mut candidate = fixture.clone();
            candidate["independentDepthMm"] = json!(claimed);
            candidate["reportedDepthMm"] = json!(truthful.max(claimed));
            let path = write_temp_fixture("independent_depth", &candidate);
            let result = load_sample(&path, &effective);
            assert_eq!(result.is_ok(), ok, "claimed {claimed}: {result:?}");
        }

        let mut candidate = fixture;
        candidate["independentDepthMm"] = json!(truthful + 0.5);
        candidate["reportedDepthMm"] = json!(truthful + 0.5);
        let path = write_temp_fixture("independent_depth_message", &candidate);
        let error = load_sample(&path, &effective).unwrap_err();
        assert!(
            error
                .to_string()
                .contains(&format!("its placements measure {truthful}")),
            "{error}"
        );
    }

    /// `reportedDepthMm` is the strip the layout was found in, so it may sit
    /// above the layout's own depth - several committed fixtures do. Only the
    /// impossible direction is an error.
    #[test]
    fn parent_fixture_reported_depth_may_exceed_but_never_understate_the_layout() {
        let effective = sample_effective_settings();
        let mut fixture = sample_pinned_vacancy_fixture(None);
        fixture["placements"] = json!([sample_placement(3.5)]);
        fixture["independentDepthMm"] = json!(sample_depth(3.5));
        let truthful = sample_depth(3.5);

        let mut deeper = fixture.clone();
        deeper["reportedDepthMm"] = json!(truthful + 0.264);
        let path = write_temp_fixture("reported_depth_deeper", &deeper);
        assert!(load_sample(&path, &effective).is_ok());

        let mut shallower = fixture;
        shallower["reportedDepthMm"] = json!(truthful - 0.5);
        let path = write_temp_fixture("reported_depth_shallower", &shallower);
        let error = load_sample(&path, &effective).unwrap_err();
        assert!(
            error.to_string().contains("which is shallower than"),
            "{error}"
        );
    }

    /// Every convention the engine reports a depth in is accepted - and nothing
    /// else. The sub-grid placement below separates the snapped and raw
    /// measurements, and the non-zero allowance separates the source and
    /// envelope ones, so the test would not pass if the check collapsed them.
    #[test]
    fn parent_fixture_depth_accepts_every_reporting_convention_and_nothing_else() {
        let effective = sample_effective_settings();
        // A translation a third of a grid step above 3.500: the canonical grid
        // rounds it back down, the raw source rings keep it.
        let measured = measure_parent_fixture_depths(
            "probe",
            &[GeneralFastPlacement {
                piece_id: "piece".to_owned(),
                rotation_deg: 0.0,
                mirrored: false,
                translate_short_axis: 3.0,
                translate_long_axis: 3.5003,
            }],
            &effective,
            &sample_owned_pieces(),
        )
        .unwrap();
        assert_eq!(measured.source_snapped_mm, sample_depth(3.5));
        assert_eq!(measured.source_raw_mm, sample_depth(3.5003));
        assert_eq!(
            measured.envelope_excess_mm,
            effective.clearance_safety_margin_mm + effective.search_offset_allowance_mm
        );
        assert!(measured.envelope_excess_mm > PARENT_FIXTURE_DEPTH_TOLERANCE_MM / 2.0);

        let candidate = |claimed: f64| {
            let mut fixture = sample_pinned_vacancy_fixture(None);
            fixture["placements"] = json!([sample_placement(3.5003)]);
            fixture["independentDepthMm"] = json!(claimed);
            fixture["reportedDepthMm"] = json!(claimed.max(measured.shallowest_mm()));
            fixture
        };

        let mut cases = measured
            .candidates()
            .map(|candidate| (candidate, true))
            .to_vec();
        cases.push((measured.source_raw_mm + 0.5, false));
        cases.push((measured.source_snapped_mm - 0.5, false));
        for (claimed, ok) in cases {
            let path = write_temp_fixture("depth_convention", &candidate(claimed));
            let result = load_sample(&path, &effective);
            assert_eq!(result.is_ok(), ok, "claimed {claimed}: {result:?}");
        }
    }

    /// The `usedLongAxisDepthMm` convention exceeds the source depth by exactly
    /// the part of the collision expansion that is not half the pair clearance.
    /// Anchors in the corpus were written from that field, so the identity has
    /// to hold rather than be absorbed by a wide tolerance.
    #[test]
    fn envelope_depth_convention_is_margin_plus_allowance_above_the_source() {
        let mut effective = sample_effective_settings();
        effective.clearance_safety_margin_mm = 0.0;
        effective.search_offset_allowance_mm = 0.002;
        let measured = measure_parent_fixture_depths(
            "probe",
            &[GeneralFastPlacement {
                piece_id: "piece".to_owned(),
                rotation_deg: 0.0,
                mirrored: false,
                translate_short_axis: 3.0,
                translate_long_axis: 3.5,
            }],
            &effective,
            &sample_owned_pieces(),
        )
        .unwrap();
        assert_eq!(measured.envelope_excess_mm, 0.002);
        assert!(measured
            .candidates()
            .contains(&(measured.source_raw_mm + 0.002)));
        // A claim 0.002 above the source depth - exactly what an anchor written
        // from `usedLongAxisDepthMm` at the default allowance carries - lands on
        // a convention rather than 0.002 away from the nearest one.
        assert_eq!(measured.distance_from(measured.source_raw_mm + 0.002), 0.0);
    }

    #[test]
    fn parent_fixture_placing_an_unknown_piece_hard_errors() {
        let effective = sample_effective_settings();
        let mut fixture = sample_pinned_vacancy_fixture(None);
        let mut placement = sample_placement(3.5);
        placement["pieceId"] = json!("stranger");
        fixture["placements"] = json!([placement]);
        let path = write_temp_fixture("unknown_piece", &fixture);
        let error = load_sample(&path, &effective).unwrap_err();
        assert!(
            error.to_string().contains("places unknown piece stranger"),
            "{error}"
        );
    }

    #[test]
    fn persistent_vacancy_unrun_reason_flags_missing_population_block() {
        assert_eq!(
            persistent_vacancy_unrun_reason(None),
            Some("persistent-vacancy population diagnostics were not produced".to_owned())
        );
    }

    #[test]
    fn persistent_vacancy_unrun_reason_flags_unattempted_arm() {
        let diagnostics = GeneralPersistentVacancyDiagnostics {
            attempted: false,
            failure_reason: Some("invalid parent".to_owned()),
            ..GeneralPersistentVacancyDiagnostics::default()
        };
        assert_eq!(
            persistent_vacancy_unrun_reason(Some(&diagnostics)),
            Some("invalid parent".to_owned())
        );
    }

    #[test]
    fn persistent_vacancy_unrun_reason_defaults_message_when_reason_absent() {
        let diagnostics = GeneralPersistentVacancyDiagnostics {
            attempted: false,
            failure_reason: None,
            ..GeneralPersistentVacancyDiagnostics::default()
        };
        assert_eq!(
            persistent_vacancy_unrun_reason(Some(&diagnostics)),
            Some("no failure reason was recorded".to_owned())
        );
    }

    #[test]
    fn persistent_vacancy_unrun_reason_is_none_when_arm_ran() {
        let diagnostics = GeneralPersistentVacancyDiagnostics {
            attempted: true,
            ..GeneralPersistentVacancyDiagnostics::default()
        };
        assert_eq!(persistent_vacancy_unrun_reason(Some(&diagnostics)), None);
    }
}
