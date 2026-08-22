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
use polygon_nesting_core::search::overlap_ics::contact::convex_cell_gap;
use polygon_nesting_core::search::overlap_ics::corpus;
use polygon_nesting_core::search::overlap_ics::descent::{counter_hash, DescentConfig};
use polygon_nesting_core::search::overlap_ics::diagnostics::WorkVector;
use polygon_nesting_core::search::overlap_ics::homotopy;
use polygon_nesting_core::search::overlap_ics::publish::{
    placement_fingerprint, raw_depth_of, PublicationLimits,
};
use polygon_nesting_core::search::overlap_ics::state::{
    piece_sources, Contract, ExactIncumbent, PieceSource, Pose,
};
use polygon_nesting_core::search::overlap_ics::{
    poses_of, Engine, IcsConfig, IcsOutcome, InitialLayoutProvider,
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

fn outcome_json(outcome: &IcsOutcome, constructor_fingerprint: &str) -> Value {
    json!({
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
        },
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
        }
        "constructor" | "c175" | "c168" | "triangle" | "run" => {
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
                "c175" => constructor_depth - 0.10 * (constructor_depth - lower_scale_mm),
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
                // the constructor's poses, affinely compressed onto the locked
                // target, then displaced by a seed-keyed SE(2) vector.
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
                let factor =
                    homotopy::affine_compression_factor(&sources, &parent, &contract, target);
                let compressed = homotopy::compressed(&sources, &parent, &contract, factor);
                let shocked = perturb(
                    &compressed,
                    seed,
                    options.number("shockmm", 0.25)?,
                    options.number("shockdeg", 1.0)?,
                );
                let config = IcsConfig {
                    target_depth_mm: target,
                    proposal_budget: options.integer("budget", 100_000)?,
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
                let outcome = engine.run();
                wall.insert(
                    "solverSeconds".to_owned(),
                    json!(solver_started.elapsed().as_secs_f64()),
                );
                document["shock"] = json!({
                    "affineFactor": factor,
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
                document["finalPoseDigest"] = json!(pose_digest(&outcome.final_poses));
            }
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
    document["executableSha256"] = json!(env::current_exe()
        .ok()
        .and_then(|path| fs::read(path).ok())
        .map(|bytes| format!("{:x}", Sha256::digest(&bytes))));
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
    let accepted_before = engine.work().accepted_moves;
    let started = Instant::now();
    let mut done = 0u64;
    while done < proposals {
        engine.propose_once((done as usize) % count);
        done += 1;
    }
    let proposal_seconds = started.elapsed().as_secs_f64();
    let proposals_per_second = done as f64 / proposal_seconds;
    let phi_after = engine.cold_rebuild().raw;
    let accepted = engine.work().accepted_moves - accepted_before;

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
        "projectedAtLeast100K": proposals_per_second * 8.0 >= 100_000.0,
        "pass": cold_micros <= 200.0
            && row_micros <= 20.0
            && (evaluations as f64 / gap_seconds) >= 1.0e6
            && proposals_per_second * 8.0 >= 100_000.0,
    })
}
