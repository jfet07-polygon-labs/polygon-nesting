//! Deterministic short-side-first constructor for general polygons.
//!
//! This is the first reusable heuristic layer for the general engine. It is
//! intentionally independent from the legacy convex decoder and from any
//! later relaxed/Sparrow-like compaction phase: it must produce a strong,
//! legal incumbent on its own.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::{Display, Formatter};

use crate::canonical_grid::{from_grid, to_grid_mm};
use crate::domain::IrregularPoint;
use crate::geometry::convex::compute_convex_hull;
use crate::geometry::general_polygon::{
    GeneralPolygonError, PolygonRing, PolygonSet, GENERAL_MAX_JOB_VERTICES,
};
use crate::parallel::map_slice_with_job_pool;
use crate::profiling::{self, Counter, Phase};
use crate::validation::general_polygon::{
    validate_publication, GeneralPlacement, PublicationValidationError,
    PublicationValidationSettings,
};

#[path = "general_pair_cluster.rs"]
mod pair_cluster;

const DEFAULT_ANGLE_SEED_COUNT: usize = 16;
const DEFAULT_MAX_ANGLES_PER_PIECE: usize = 64;
const DEFAULT_MAX_EVALUATIONS_PER_PIECE: usize = 4_096;
const DEFAULT_MAX_ORDER_VARIANTS: usize = 4;
const DEFAULT_MAX_CATALOG_VARIANTS: usize = 2;
const DEFAULT_MAX_PAIRING_BAND_VARIANTS: usize = 4;
const DEFAULT_MAX_PARTIAL_LAYOUTS: usize = 16;
const DEFAULT_MAX_TIGHTENING_PASSES: usize = 4;
const DEFAULT_MAX_REPAIR_TARGETS: usize = 64;
/// Default width of the conservative allowance the *search* envelope adds on
/// top of the requested clearances (see
/// [`GeneralFastSettings::search_offset_allowance_mm`]). It is a search-side
/// safety buffer only: publication is always validated against the exact
/// requested clearances by [`validate_publication`], which this constant never
/// feeds.
pub const DEFAULT_SEARCH_OFFSET_ALLOWANCE_MM: f64 = 0.002;
const PROPOSAL_BUDGET_MULTIPLIER: usize = 8;
const PRIMARY_ORIENTATION_EVALUATION_NUMERATOR: usize = 1;
const PRIMARY_ORIENTATION_EVALUATION_DENOMINATOR: usize = 2;
const ANGLE_KEY_SCALE: f64 = 1_000_000.0;

#[derive(Clone, Copy, Debug)]
pub struct GeneralFastPiece<'a> {
    pub id: &'a str,
    pub polygon: &'a PolygonSet,
    pub allow_rotation: bool,
    pub allow_mirror: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct GeneralFastSettings {
    pub sheet_short_axis_mm: f64,
    pub sheet_long_axis_mm: f64,
    pub total_padding_mm: f64,
    pub sheet_edge_clearance_mm: Option<f64>,
    pub clearance_safety_margin_mm: f64,
    pub flattening_sag_tolerance_mm: f64,
    /// Extra width added to the collision offset used by the *search*
    /// envelope, on top of `total_padding_mm / 2 + clearance_safety_margin_mm`.
    ///
    /// The envelope is a conservative superset of the exact clearance
    /// contract: a pose that clears the envelope always clears the requested
    /// clearances, so search never has to run the exact validator to reject an
    /// obviously illegal pose. Publication is validated separately and
    /// exactly, so shrinking this allowance only widens the set of legal
    /// placements search may visit; it never relaxes what may be published.
    /// `0.0` removes the allowance, making the search envelope coincide with
    /// the requested clearances.
    pub search_offset_allowance_mm: f64,
    pub angle_seed_count: usize,
    pub max_angles_per_piece: usize,
    pub max_evaluations_per_piece: usize,
    pub max_exploratory_evaluations_per_piece: usize,
    pub max_order_variants: usize,
    pub max_catalog_variants: usize,
    pub max_catalog_evaluations_per_piece: usize,
    pub max_pairing_evaluations_per_piece: usize,
    pub max_pairing_band_variants: usize,
    pub max_partial_layouts: usize,
    pub max_beam_evaluations_per_state: usize,
    pub max_tightening_passes: usize,
    pub max_repair_targets: usize,
    pub max_repair_evaluations_per_piece: usize,
    pub max_local_angle_refinement_evaluations_per_piece: usize,
}

impl GeneralFastSettings {
    pub fn deterministic_test(sheet_short_axis_mm: f64, sheet_long_axis_mm: f64) -> Self {
        Self {
            sheet_short_axis_mm,
            sheet_long_axis_mm,
            total_padding_mm: 0.0,
            sheet_edge_clearance_mm: None,
            clearance_safety_margin_mm: 0.0,
            flattening_sag_tolerance_mm: 0.0,
            search_offset_allowance_mm: DEFAULT_SEARCH_OFFSET_ALLOWANCE_MM,
            angle_seed_count: DEFAULT_ANGLE_SEED_COUNT,
            max_angles_per_piece: DEFAULT_MAX_ANGLES_PER_PIECE,
            max_evaluations_per_piece: DEFAULT_MAX_EVALUATIONS_PER_PIECE,
            max_exploratory_evaluations_per_piece: 0,
            max_order_variants: 1,
            max_catalog_variants: 1,
            max_catalog_evaluations_per_piece: 0,
            max_pairing_evaluations_per_piece: 0,
            max_pairing_band_variants: 1,
            max_partial_layouts: 1,
            max_beam_evaluations_per_state: 0,
            max_tightening_passes: 0,
            max_repair_targets: 0,
            max_repair_evaluations_per_piece: 0,
            max_local_angle_refinement_evaluations_per_piece: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeneralFastPlacement {
    pub piece_id: String,
    pub rotation_deg: f64,
    pub mirrored: bool,
    pub translate_short_axis: f64,
    pub translate_long_axis: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeneralFastResult {
    pub placements: Vec<GeneralFastPlacement>,
    pub unplaced_piece_ids: Vec<String>,
    pub exact_evaluations: usize,
    pub primary_exact_evaluations: usize,
    pub order_portfolio_exact_evaluations: usize,
    pub catalog_portfolio_exact_evaluations: usize,
    pub pairing_exact_evaluations: usize,
    pub beam_exact_evaluations: usize,
    pub tightening_exact_evaluations: usize,
    pub tightening_passes_attempted: usize,
    pub tightening_passes_improved: usize,
    pub catalog_candidate_placed_count: Option<usize>,
    pub catalog_candidate_depth_mm: Option<f64>,
    pub pairing_candidate_placed_count: Option<usize>,
    pub pairing_candidate_depth_mm: Option<f64>,
    pub beam_candidate_placed_count: Option<usize>,
    pub beam_candidate_depth_mm: Option<f64>,
    pub exploratory_exact_evaluations: usize,
    pub repair_exact_evaluations: usize,
    pub local_angle_refinement_exact_evaluations: usize,
    pub repair_targets_considered: usize,
    pub order_variants_attempted: usize,
    pub catalog_variants_attempted: usize,
    pub order_portfolio_failed: bool,
    pub catalog_portfolio_failed: bool,
    pub pairing_failed: bool,
    pub beam_failed: bool,
    pub exploratory_failed: bool,
    pub repair_failed: bool,
    pub used_short_axis_span_mm: f64,
    pub used_long_axis_depth_mm: f64,
    pub unused_short_axis_projection_mm: f64,
    pub occupied_envelope_area_mm2: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GeneralPairTemplateDiagnostics {
    pub eligible_pairs: usize,
    pub pairs_with_templates: usize,
    pub fallback_pairs: usize,
    pub orientation_tuples: usize,
    pub contact_attempts: usize,
    pub exact_pair_rows: usize,
    pub retained_templates: usize,
    pub transformed_source_vertices: usize,
    pub offset_output_vertices: usize,
    pub intersection_input_vertices: usize,
    pub intersection_output_vertices: usize,
    pub transient_rejected_output_vertices: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GeneralPairClusterArmDiagnostics {
    pub result: Option<GeneralFastResult>,
    pub band_variants_attempted: usize,
    pub completed_bands: usize,
    pub band_failures: Vec<String>,
    pub proposal_attempts: usize,
    pub generated_proposals: usize,
    pub exact_child_fixed_visits: usize,
    pub exact_candidate_rows: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GeneralPairClusterExperiment {
    pub templates: GeneralPairTemplateDiagnostics,
    pub control: GeneralPairClusterArmDiagnostics,
    pub treatment: GeneralPairClusterArmDiagnostics,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GeneralPlacementMetrics {
    pub used_short_axis_span_mm: f64,
    pub used_long_axis_depth_mm: f64,
    pub unused_short_axis_projection_mm: f64,
    pub occupied_envelope_area_mm2: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GeneralFastError {
    InvalidSettings(String),
    InvalidInput(String),
    Geometry(GeneralPolygonError),
    Publication(PublicationValidationError),
}

impl From<GeneralPolygonError> for GeneralFastError {
    fn from(error: GeneralPolygonError) -> Self {
        Self::Geometry(error)
    }
}

impl From<PublicationValidationError> for GeneralFastError {
    fn from(error: PublicationValidationError) -> Self {
        Self::Publication(error)
    }
}

impl Display for GeneralFastError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSettings(message) => formatter.write_str(message),
            Self::InvalidInput(message) => formatter.write_str(message),
            Self::Geometry(error) => Display::fmt(error, formatter),
            Self::Publication(error) => Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for GeneralFastError {}

#[derive(Clone)]
struct PreparedGeneralPiece<'a> {
    input_index: usize,
    input: GeneralFastPiece<'a>,
    collision: PolygonSet,
    area_mm2: f64,
    convex_hull_area_mm2: f64,
    diameter_mm: f64,
    reflex_vertices: usize,
    vertex_count: usize,
    long_span_mm: f64,
    short_span_mm: f64,
    fill_ratio: f64,
    shape_family_key: Vec<i64>,
}

#[derive(Clone)]
struct PlacedState {
    input_index: usize,
    placement: GeneralFastPlacement,
    collision: PolygonSet,
}

#[derive(Clone)]
struct PartialLayout {
    placed: Vec<PlacedState>,
    unplaced_piece_ids: Vec<String>,
}

type CanonicalPlacementKey = (String, i64, bool, i64, i64);
type PartialLayoutStateKey = (Vec<CanonicalPlacementKey>, Vec<String>);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum CandidateKind {
    SheetSupport,
    VertexVertex,
    MovingVertexFixedEdge,
    FixedVertexMovingEdge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CandidateKey {
    angle_key: i64,
    mirrored: bool,
    kind: CandidateKind,
    fixed_piece_id_rank: usize,
    fixed_feature_ordinal: usize,
    moving_feature_ordinal: usize,
    translate_x_grid: i64,
    translate_y_grid: i64,
}

#[derive(Clone)]
struct Candidate {
    key: CandidateKey,
    rotation_deg: f64,
    mirrored: bool,
    translate_x: f64,
    translate_y: f64,
    collision: PolygonSet,
}

#[derive(Clone, Copy)]
struct CandidateProposal {
    key: CandidateKey,
    rotation_deg: f64,
    mirrored: bool,
    translate_x: f64,
    translate_y: f64,
    score: CandidateScore,
    broad_phase_overlap_count: usize,
}

#[derive(Clone, Copy, Debug)]
struct CandidateScore {
    candidate_long_axis_position: f64,
    candidate_short_axis_position: f64,
    long_axis_depth: f64,
    unused_short_axis_projection: f64,
    envelope_area: f64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConstructorArm {
    Primary,
    Catalog,
    Pairing,
    Exploratory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PieceOrderStrategy {
    AreaDiameterReflex,
    HullAreaDiameter,
    LongSpan,
    Concavity,
    Elongation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FixedPieceOrder {
    Id,
    ShortSideFrontier,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ContactCoverage {
    Fair,
    FrontierGreedy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AngleScope {
    Orthogonal,
    Full,
}

fn validate_piece_set(pieces: &[GeneralFastPiece<'_>]) -> Result<(), GeneralFastError> {
    let mut piece_ids = BTreeSet::new();
    for piece in pieces {
        if !piece_ids.insert(piece.id) {
            return Err(GeneralFastError::InvalidInput(format!(
                "piece IDs must be unique; duplicate ID: {}",
                piece.id
            )));
        }
    }
    let job_vertices = pieces.iter().try_fold(0usize, |total, piece| {
        total.checked_add(piece.polygon.vertex_count())
    });
    if job_vertices.is_none_or(|count| count > GENERAL_MAX_JOB_VERTICES) {
        return Err(GeneralPolygonError::from_message(format!(
            "a general constructor job may contain at most {GENERAL_MAX_JOB_VERTICES} vertices"
        ))
        .into());
    }
    Ok(())
}

fn prepare_general_pieces<'a>(
    pieces: &[GeneralFastPiece<'a>],
    settings: GeneralFastSettings,
) -> Result<Vec<PreparedGeneralPiece<'a>>, GeneralFastError> {
    let expansion = collision_expansion_mm(settings);
    pieces
        .iter()
        .copied()
        .enumerate()
        .map(|(input_index, input)| {
            let bounds = input.polygon.bounds().ok_or_else(|| {
                GeneralFastError::Geometry(GeneralPolygonError::from_message(
                    "cannot prepare empty geometry",
                ))
            })?;
            let width = bounds.max_x - bounds.min_x;
            let height = bounds.max_y - bounds.min_y;
            let area_mm2 = input.polygon.area_mm2();
            let convex_hull_area_mm2 = polygon_convex_hull_area_mm2(input.polygon);
            let collision = input.polygon.offset(expansion)?;
            Ok(PreparedGeneralPiece {
                input_index,
                input,
                shape_family_key: polygon_shape_family_key(&collision),
                collision,
                area_mm2,
                convex_hull_area_mm2,
                diameter_mm: polygon_diameter(input.polygon),
                reflex_vertices: reflex_vertex_count(input.polygon),
                vertex_count: input.polygon.vertex_count(),
                long_span_mm: width.max(height),
                short_span_mm: width.min(height),
                fill_ratio: area_mm2 / (width * height),
            })
        })
        .collect()
}

pub fn construct_short_side_first(
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
) -> Result<GeneralFastResult, GeneralFastError> {
    validate_settings(settings)?;
    validate_piece_set(pieces)?;
    let prepared = prepare_general_pieces(pieces, settings)?;
    let order_portfolio = piece_order_portfolio(&prepared, settings.max_order_variants);
    let order_outcomes = map_slice_with_job_pool(&order_portfolio, |order| {
        let mut exact_evaluations = 0usize;
        let result = run_constructor_arm(
            order,
            pieces,
            settings,
            ConstructorArm::Primary,
            FixedPieceOrder::Id,
            &mut exact_evaluations,
        );
        (result, exact_evaluations)
    });
    let mut winning_order = order_portfolio[0].clone();
    let mut order_outcomes = order_outcomes.into_iter();
    let (primary, primary_exact_evaluations) = order_outcomes
        .next()
        .expect("a validated order portfolio is non-empty");
    let mut primary = primary?;
    let mut order_portfolio_exact_evaluations = 0usize;
    let mut order_portfolio_failed = false;
    for (order_index, (candidate, candidate_exact_evaluations)) in order_outcomes
        .enumerate()
        .map(|(index, outcome)| (index + 1, outcome))
    {
        order_portfolio_exact_evaluations =
            order_portfolio_exact_evaluations.saturating_add(candidate_exact_evaluations);
        match candidate {
            Ok(candidate) => {
                if compare_result_quality(&candidate, &primary) == Ordering::Less {
                    primary = candidate;
                    winning_order = order_portfolio[order_index].clone();
                }
            }
            Err(_) => order_portfolio_failed = true,
        }
    }
    let mut winning_catalog = FixedPieceOrder::Id;
    let mut catalog_portfolio_exact_evaluations = 0usize;
    let mut catalog_portfolio_failed = false;
    let mut catalog_candidate_quality = None;
    if settings.max_catalog_variants > 1 {
        let mut candidate_exact_evaluations = 0usize;
        let candidate = run_constructor_arm(
            &winning_order,
            pieces,
            settings,
            ConstructorArm::Catalog,
            FixedPieceOrder::ShortSideFrontier,
            &mut candidate_exact_evaluations,
        );
        catalog_portfolio_exact_evaluations = candidate_exact_evaluations;
        match candidate {
            Ok(candidate) => {
                catalog_candidate_quality = Some((
                    candidate.placements.len(),
                    candidate.used_long_axis_depth_mm,
                ));
                if compare_result_quality(&candidate, &primary) == Ordering::Less {
                    primary = candidate;
                    winning_catalog = FixedPieceOrder::ShortSideFrontier;
                }
            }
            Err(_) => catalog_portfolio_failed = true,
        }
    }
    let mut pairing_exact_evaluations = 0usize;
    let mut pairing_failed = false;
    let mut pairing_candidate_quality = None;
    if settings.max_pairing_evaluations_per_piece > 0 {
        let pairing_order = shape_family_order(&prepared);
        let mut best_pairing = None;
        for factor in [1.05, 1.25, 1.5, 2.0]
            .into_iter()
            .take(settings.max_pairing_band_variants)
        {
            let mut pairing_settings = settings;
            pairing_settings.sheet_long_axis_mm = fast_band_depth(&prepared, settings, factor);
            let mut candidate_exact_evaluations = 0usize;
            let candidate = run_constructor_arm(
                &pairing_order,
                pieces,
                pairing_settings,
                ConstructorArm::Pairing,
                FixedPieceOrder::ShortSideFrontier,
                &mut candidate_exact_evaluations,
            );
            pairing_exact_evaluations =
                pairing_exact_evaluations.saturating_add(candidate_exact_evaluations);
            match candidate {
                Ok(candidate) => {
                    if best_pairing.as_ref().is_none_or(|incumbent| {
                        compare_result_quality(&candidate, incumbent) == Ordering::Less
                    }) {
                        best_pairing = Some(candidate);
                    }
                }
                Err(_) => pairing_failed = true,
            }
        }
        if let Some(candidate) = best_pairing {
            pairing_candidate_quality = Some((
                candidate.placements.len(),
                candidate.used_long_axis_depth_mm,
            ));
            if compare_result_quality(&candidate, &primary) == Ordering::Less {
                primary = candidate;
                winning_catalog = FixedPieceOrder::ShortSideFrontier;
            }
        }
    }
    let mut beam_exact_evaluations = 0usize;
    let mut tightening_exact_evaluations = 0usize;
    let mut tightening_passes_attempted = 0usize;
    let mut tightening_passes_improved = 0usize;
    let mut beam_failed = false;
    let mut beam_candidate_quality = None;
    if settings.max_partial_layouts > 1 {
        let primary_incomplete = primary.placements.len() < pieces.len();
        let beam_order = shape_family_order(&prepared);
        let mut best_beam = None::<(GeneralFastResult, AngleScope)>;
        for scope in beam_angle_scopes(&beam_order, settings) {
            let (candidate, order_exact_evaluations) =
                run_beam_order(&beam_order, pieces, settings, primary_incomplete, scope);
            beam_exact_evaluations = beam_exact_evaluations.saturating_add(order_exact_evaluations);
            match candidate {
                Ok(candidate) => {
                    if best_beam.as_ref().is_none_or(|(incumbent, _)| {
                        compare_result_quality(&candidate, incumbent) == Ordering::Less
                    }) {
                        best_beam = Some((candidate, scope));
                    }
                }
                Err(_) => beam_failed = true,
            }
        }
        match best_beam {
            Some((mut candidate, winning_scope)) => {
                if candidate.placements.len() == pieces.len() {
                    for _ in 0..settings.max_tightening_passes {
                        let mut tightened_settings = settings;
                        tightened_settings.sheet_long_axis_mm =
                            candidate.used_long_axis_depth_mm * 0.995;
                        let (tightened, pass_exact_evaluations) = run_beam_order(
                            &beam_order,
                            pieces,
                            tightened_settings,
                            false,
                            winning_scope,
                        );
                        tightening_passes_attempted += 1;
                        tightening_exact_evaluations =
                            tightening_exact_evaluations.saturating_add(pass_exact_evaluations);
                        beam_exact_evaluations =
                            beam_exact_evaluations.saturating_add(pass_exact_evaluations);
                        let Ok(tightened) = tightened else {
                            break;
                        };
                        if tightened.placements.len() != pieces.len()
                            || compare_result_quality(&tightened, &candidate) != Ordering::Less
                        {
                            break;
                        }
                        candidate = tightened;
                        tightening_passes_improved += 1;
                    }
                }
                beam_candidate_quality = Some((
                    candidate.placements.len(),
                    candidate.used_long_axis_depth_mm,
                ));
                if compare_result_quality(&candidate, &primary) == Ordering::Less {
                    primary = candidate;
                }
            }
            None => beam_failed = true,
        }
    }
    primary.exact_evaluations = primary_exact_evaluations
        .saturating_add(order_portfolio_exact_evaluations)
        .saturating_add(catalog_portfolio_exact_evaluations)
        .saturating_add(pairing_exact_evaluations)
        .saturating_add(beam_exact_evaluations);
    primary.primary_exact_evaluations = primary_exact_evaluations;
    primary.order_portfolio_exact_evaluations = order_portfolio_exact_evaluations;
    primary.catalog_portfolio_exact_evaluations = catalog_portfolio_exact_evaluations;
    primary.pairing_exact_evaluations = pairing_exact_evaluations;
    primary.beam_exact_evaluations = beam_exact_evaluations;
    primary.tightening_exact_evaluations = tightening_exact_evaluations;
    primary.tightening_passes_attempted = tightening_passes_attempted;
    primary.tightening_passes_improved = tightening_passes_improved;
    primary.catalog_candidate_placed_count = catalog_candidate_quality.map(|quality| quality.0);
    primary.catalog_candidate_depth_mm = catalog_candidate_quality.map(|quality| quality.1);
    primary.pairing_candidate_placed_count = pairing_candidate_quality.map(|quality| quality.0);
    primary.pairing_candidate_depth_mm = pairing_candidate_quality.map(|quality| quality.1);
    primary.beam_candidate_placed_count = beam_candidate_quality.map(|quality| quality.0);
    primary.beam_candidate_depth_mm = beam_candidate_quality.map(|quality| quality.1);
    primary.order_variants_attempted = order_portfolio.len();
    primary.catalog_variants_attempted = settings.max_catalog_variants;
    primary.order_portfolio_failed = order_portfolio_failed;
    primary.catalog_portfolio_failed = catalog_portfolio_failed;
    primary.pairing_failed = pairing_failed;
    primary.beam_failed = beam_failed;
    let selected = if settings.max_exploratory_evaluations_per_piece == 0 {
        primary
    } else {
        let mut exploratory_exact_evaluations = 0usize;
        let exploratory = run_constructor_arm(
            &winning_order,
            pieces,
            settings,
            ConstructorArm::Exploratory,
            winning_catalog,
            &mut exploratory_exact_evaluations,
        );
        select_optional_arm(primary, exploratory, exploratory_exact_evaluations)
    };
    repair_result(selected, &prepared, pieces, settings)
}

pub fn diagnose_congruent_pair_templates(
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
) -> Result<GeneralPairTemplateDiagnostics, GeneralFastError> {
    validate_settings(settings)?;
    validate_piece_set(pieces)?;
    let prepared = prepare_general_pieces(pieces, settings)?;
    Ok(pair_cluster::build_pair_template_catalog(&prepared, settings)?.diagnostics)
}

pub fn diagnose_congruent_pair_constructor(
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
) -> Result<GeneralPairClusterExperiment, GeneralFastError> {
    validate_settings(settings)?;
    validate_piece_set(pieces)?;
    let prepared = prepare_general_pieces(pieces, settings)?;
    pair_cluster::run_pair_cluster_experiment(&prepared, pieces, settings)
}

fn run_constructor_arm(
    prepared: &[PreparedGeneralPiece<'_>],
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    arm: ConstructorArm,
    fixed_piece_order_strategy: FixedPieceOrder,
    exact_evaluations: &mut usize,
) -> Result<GeneralFastResult, GeneralFastError> {
    debug_assert_eq!(*exact_evaluations, 0);
    let mut placed = Vec::<PlacedState>::new();
    let mut unplaced_piece_ids = Vec::new();
    for piece in prepared {
        if let Some(candidate) = run_constructor_step(
            piece,
            &placed,
            settings,
            arm,
            fixed_piece_order_strategy,
            exact_evaluations,
        )? {
            placed.push(candidate);
        } else {
            unplaced_piece_ids.push(piece.input.id.to_owned());
        }
    }

    validate_result(pieces, &placed, settings)?;
    let layout_metrics = layout_metrics(&placed, settings);
    let (primary_exact_evaluations, exploratory_exact_evaluations) = match arm {
        ConstructorArm::Primary | ConstructorArm::Catalog | ConstructorArm::Pairing => {
            (*exact_evaluations, 0)
        }
        ConstructorArm::Exploratory => (0, *exact_evaluations),
    };

    Ok(GeneralFastResult {
        placements: placed.into_iter().map(|state| state.placement).collect(),
        unplaced_piece_ids,
        exact_evaluations: *exact_evaluations,
        primary_exact_evaluations,
        order_portfolio_exact_evaluations: 0,
        catalog_portfolio_exact_evaluations: 0,
        pairing_exact_evaluations: 0,
        beam_exact_evaluations: 0,
        tightening_exact_evaluations: 0,
        tightening_passes_attempted: 0,
        tightening_passes_improved: 0,
        catalog_candidate_placed_count: None,
        catalog_candidate_depth_mm: None,
        pairing_candidate_placed_count: None,
        pairing_candidate_depth_mm: None,
        beam_candidate_placed_count: None,
        beam_candidate_depth_mm: None,
        exploratory_exact_evaluations,
        repair_exact_evaluations: 0,
        local_angle_refinement_exact_evaluations: 0,
        repair_targets_considered: 0,
        order_variants_attempted: 1,
        catalog_variants_attempted: 1,
        order_portfolio_failed: false,
        catalog_portfolio_failed: false,
        pairing_failed: false,
        beam_failed: false,
        exploratory_failed: false,
        repair_failed: false,
        used_short_axis_span_mm: layout_metrics.used_short_axis_span_mm,
        used_long_axis_depth_mm: layout_metrics.used_long_axis_depth_mm,
        unused_short_axis_projection_mm: layout_metrics.unused_short_axis_projection_mm,
        occupied_envelope_area_mm2: layout_metrics.occupied_envelope_area_mm2,
    })
}

fn run_constructor_step(
    piece: &PreparedGeneralPiece<'_>,
    placed: &[PlacedState],
    settings: GeneralFastSettings,
    arm: ConstructorArm,
    fixed_piece_order_strategy: FixedPieceOrder,
    exact_evaluations: &mut usize,
) -> Result<Option<PlacedState>, GeneralFastError> {
    let angles = angle_candidates(piece, placed, settings, AngleScope::Full);
    let mut oriented_by_key = BTreeMap::<(i64, bool), PolygonSet>::new();
    let mut proposals = Vec::<CandidateProposal>::new();
    let exact_budget = match arm {
        ConstructorArm::Primary => settings.max_evaluations_per_piece,
        ConstructorArm::Catalog => settings.max_catalog_evaluations_per_piece,
        ConstructorArm::Pairing => settings.max_pairing_evaluations_per_piece,
        ConstructorArm::Exploratory => settings.max_exploratory_evaluations_per_piece,
    };
    let proposal_budget = match arm {
        ConstructorArm::Primary | ConstructorArm::Catalog | ConstructorArm::Pairing => exact_budget,
        ConstructorArm::Exploratory => exact_budget.saturating_mul(PROPOSAL_BUDGET_MULTIPLIER),
    };
    let considered_angles = match arm {
        ConstructorArm::Catalog => 1,
        ConstructorArm::Primary | ConstructorArm::Pairing | ConstructorArm::Exploratory => {
            angles.len()
        }
    };
    let per_angle_budget = proposal_budget.div_ceil(considered_angles.max(1)).max(1);
    let global_attempt_budget = proposal_budget.saturating_mul(PROPOSAL_BUDGET_MULTIPLIER);
    let mut remaining_proposal_budget = proposal_budget;
    let mut remaining_attempt_budget = global_attempt_budget;
    let mut proposal_attempts = 0usize;
    for (rotation_deg, mirrored) in angles.into_iter().take(considered_angles) {
        if remaining_proposal_budget == 0 || remaining_attempt_budget == 0 {
            break;
        }
        let oriented = match oriented_collision(piece, rotation_deg, mirrored, settings) {
            Ok(oriented) => oriented,
            Err(_) if arm != ConstructorArm::Primary => continue,
            Err(error) => return Err(error.into()),
        };
        let angle_proposal_budget = per_angle_budget.min(remaining_proposal_budget);
        let angle_attempt_budget = angle_proposal_budget
            .saturating_mul(PROPOSAL_BUDGET_MULTIPLIER)
            .min(remaining_attempt_budget);
        let (angle_proposals, angle_attempts) = match translation_proposals(
            rotation_deg,
            mirrored,
            TranslationProposalInput {
                oriented: &oriented,
                placed,
                settings,
                max_proposals: angle_proposal_budget,
                max_attempts: angle_attempt_budget,
                fixed_piece_order_strategy,
                contact_coverage: if arm == ConstructorArm::Catalog {
                    ContactCoverage::FrontierGreedy
                } else {
                    ContactCoverage::Fair
                },
            },
        ) {
            Ok(proposals) => proposals,
            Err(_) if arm != ConstructorArm::Primary => continue,
            Err(error) => return Err(error),
        };
        remaining_proposal_budget = remaining_proposal_budget.saturating_sub(angle_proposals.len());
        remaining_attempt_budget = remaining_attempt_budget.saturating_sub(angle_attempts);
        proposal_attempts = proposal_attempts.saturating_add(angle_attempts);
        proposals.extend(angle_proposals);
        oriented_by_key.insert((angle_key(rotation_deg), mirrored), oriented);
    }
    debug_assert!(proposals.len() <= proposal_budget);
    debug_assert!(proposal_attempts <= global_attempt_budget);
    proposals = match arm {
        ConstructorArm::Primary | ConstructorArm::Catalog | ConstructorArm::Pairing => {
            if arm == ConstructorArm::Pairing {
                shortlist_proposals(proposals, exact_budget)
            } else {
                proposals
            }
        }
        ConstructorArm::Exploratory => shortlist_proposals(proposals, exact_budget),
    };

    let mut best: Option<(Candidate, CandidateScore)> = None;
    for proposal in proposals {
        *exact_evaluations += 1;
        let oriented = oriented_by_key
            .get(&(proposal.key.angle_key, proposal.mirrored))
            .expect("every proposal retains its oriented polygon");
        let Ok(collision) = oriented.translated(proposal.translate_x, proposal.translate_y) else {
            continue;
        };
        let candidate = Candidate {
            key: proposal.key,
            rotation_deg: proposal.rotation_deg,
            mirrored: proposal.mirrored,
            translate_x: proposal.translate_x,
            translate_y: proposal.translate_y,
            collision,
        };
        if !collision_fits_sheet(&candidate.collision, settings) {
            continue;
        }
        if placed
            .iter()
            .map(|fixed| polygons_overlap_exact(&candidate.collision, &fixed.collision))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .any(std::convert::identity)
        {
            continue;
        }
        let score = score_candidate(placed, &candidate, settings);
        if best.as_ref().is_none_or(|(incumbent, incumbent_score)| {
            compare_candidate_scores(score, candidate.key, *incumbent_score, incumbent.key)
                == Ordering::Less
        }) {
            let Some(candidate) =
                publication_confirmed_candidate(piece, candidate, placed, settings)?
            else {
                continue;
            };
            let score = score_candidate(placed, &candidate, settings);
            if best.as_ref().is_none_or(|(incumbent, incumbent_score)| {
                compare_candidate_scores(score, candidate.key, *incumbent_score, incumbent.key)
                    == Ordering::Less
            }) {
                best = Some((candidate, score));
            }
        }
    }

    Ok(best.map(|(candidate, _)| PlacedState {
        input_index: piece.input_index,
        placement: GeneralFastPlacement {
            piece_id: piece.input.id.to_owned(),
            rotation_deg: candidate.rotation_deg,
            mirrored: candidate.mirrored,
            translate_short_axis: candidate.translate_x,
            translate_long_axis: candidate.translate_y,
        },
        collision: candidate.collision,
    }))
}

fn run_beam_order(
    order: &[PreparedGeneralPiece<'_>],
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    allow_recovery: bool,
    angle_scope: AngleScope,
) -> (Result<GeneralFastResult, GeneralFastError>, usize) {
    let mut total_exact_evaluations = 0usize;
    let mut compactness_exact_evaluations = 0usize;
    let mut candidate = run_partial_layout_beam(
        order,
        pieces,
        settings,
        false,
        angle_scope,
        &mut compactness_exact_evaluations,
    );
    total_exact_evaluations = total_exact_evaluations.saturating_add(compactness_exact_evaluations);
    if candidate.is_err() && allow_recovery {
        let mut recovery_exact_evaluations = 0usize;
        candidate = run_partial_layout_beam(
            order,
            pieces,
            settings,
            true,
            angle_scope,
            &mut recovery_exact_evaluations,
        );
        total_exact_evaluations =
            total_exact_evaluations.saturating_add(recovery_exact_evaluations);
    }
    (candidate, total_exact_evaluations)
}

fn run_partial_layout_beam(
    prepared: &[PreparedGeneralPiece<'_>],
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    allow_skips: bool,
    angle_scope: AngleScope,
    exact_evaluations: &mut usize,
) -> Result<GeneralFastResult, GeneralFastError> {
    debug_assert_eq!(*exact_evaluations, 0);
    let mut beam = vec![PartialLayout {
        placed: Vec::new(),
        unplaced_piece_ids: Vec::new(),
    }];
    for piece in prepared {
        let mut successors = Vec::new();
        let successor_batches = map_slice_with_job_pool(&beam, |state| {
            let orientations = angle_candidates(piece, &state.placed, settings, angle_scope);
            let search = best_candidate_for_orientations(
                piece,
                &state.placed,
                settings,
                &orientations,
                settings.max_beam_evaluations_per_state,
                settings.max_partial_layouts,
            )?;
            let mut state_successors = Vec::new();
            if allow_skips {
                let mut skipped = state.clone();
                skipped.unplaced_piece_ids.push(piece.input.id.to_owned());
                state_successors.push(skipped);
            }
            if search.candidates.is_empty() {
                return Ok::<_, GeneralFastError>((state_successors, search.exact_evaluations));
            }
            for candidate in search.candidates {
                let mut successor = state.clone();
                successor.placed.push(PlacedState {
                    input_index: piece.input_index,
                    placement: GeneralFastPlacement {
                        piece_id: piece.input.id.to_owned(),
                        rotation_deg: candidate.rotation_deg,
                        mirrored: candidate.mirrored,
                        translate_short_axis: candidate.translate_x,
                        translate_long_axis: candidate.translate_y,
                    },
                    collision: candidate.collision,
                });
                state_successors.push(successor);
            }
            Ok((state_successors, search.exact_evaluations))
        });
        for batch in successor_batches {
            let (mut state_successors, state_exact_evaluations) = batch?;
            *exact_evaluations = exact_evaluations.saturating_add(state_exact_evaluations);
            successors.append(&mut state_successors);
        }
        let mut seen_states = BTreeSet::new();
        successors.retain(|state| seen_states.insert(partial_layout_state_key(state)));
        beam = select_diverse_partial_layouts(successors, settings);
    }

    let mut best = None;
    for mut state in beam {
        retry_skipped_pieces(
            &mut state,
            prepared,
            settings,
            angle_scope,
            exact_evaluations,
        )?;
        if validate_result(pieces, &state.placed, settings).is_err() {
            continue;
        }
        let candidate = result_from_partial_layout(state, settings, *exact_evaluations);
        if best
            .as_ref()
            .is_none_or(|incumbent| compare_result_quality(&candidate, incumbent) == Ordering::Less)
        {
            best = Some(candidate);
        }
    }
    best.ok_or_else(|| {
        GeneralFastError::InvalidInput(
            "the partial-layout beam produced no independently valid incumbent".to_owned(),
        )
    })
}

fn retry_skipped_pieces(
    state: &mut PartialLayout,
    prepared: &[PreparedGeneralPiece<'_>],
    settings: GeneralFastSettings,
    angle_scope: AngleScope,
    exact_evaluations: &mut usize,
) -> Result<(), GeneralFastError> {
    let by_id = prepared
        .iter()
        .map(|piece| (piece.input.id, piece))
        .collect::<BTreeMap<_, _>>();
    let mut remaining = std::mem::take(&mut state.unplaced_piece_ids);
    loop {
        let mut next_remaining = Vec::new();
        let mut progress = false;
        for piece_id in remaining {
            let piece = by_id
                .get(piece_id.as_str())
                .expect("beam skip IDs reference prepared pieces");
            let orientations = angle_candidates(piece, &state.placed, settings, angle_scope);
            let search = best_candidate_for_orientations(
                piece,
                &state.placed,
                settings,
                &orientations,
                settings.max_beam_evaluations_per_state,
                1,
            )?;
            *exact_evaluations = exact_evaluations.saturating_add(search.exact_evaluations);
            let Some(candidate) = search.candidates.into_iter().next() else {
                next_remaining.push(piece_id);
                continue;
            };
            state.placed.push(PlacedState {
                input_index: piece.input_index,
                placement: GeneralFastPlacement {
                    piece_id,
                    rotation_deg: candidate.rotation_deg,
                    mirrored: candidate.mirrored,
                    translate_short_axis: candidate.translate_x,
                    translate_long_axis: candidate.translate_y,
                },
                collision: candidate.collision,
            });
            progress = true;
        }
        if !progress || next_remaining.is_empty() {
            state.unplaced_piece_ids = next_remaining;
            break;
        }
        remaining = next_remaining;
    }
    Ok(())
}

fn compare_partial_layouts(
    first: &PartialLayout,
    second: &PartialLayout,
    settings: GeneralFastSettings,
) -> Ordering {
    let first_metrics = layout_metrics(&first.placed, settings);
    let second_metrics = layout_metrics(&second.placed, settings);
    second
        .placed
        .len()
        .cmp(&first.placed.len())
        .then_with(|| {
            first_metrics
                .used_long_axis_depth_mm
                .total_cmp(&second_metrics.used_long_axis_depth_mm)
        })
        .then_with(|| {
            first_metrics
                .unused_short_axis_projection_mm
                .total_cmp(&second_metrics.unused_short_axis_projection_mm)
        })
        .then_with(|| {
            first_metrics
                .occupied_envelope_area_mm2
                .total_cmp(&second_metrics.occupied_envelope_area_mm2)
        })
        .then_with(|| {
            canonical_placed_key(&first.placed).cmp(&canonical_placed_key(&second.placed))
        })
}

fn select_diverse_partial_layouts(
    mut candidates: Vec<PartialLayout>,
    settings: GeneralFastSettings,
) -> Vec<PartialLayout> {
    candidates.sort_by(|first, second| compare_partial_layouts(first, second, settings));
    if candidates.len() <= settings.max_partial_layouts || settings.max_partial_layouts == 1 {
        candidates.truncate(settings.max_partial_layouts);
        return candidates;
    }

    let mut selected = vec![candidates[0].clone()];
    let mut selected_keys = BTreeSet::from([partial_layout_state_key(&candidates[0])]);
    let mut best_by_skipped_count = BTreeMap::<usize, PartialLayout>::new();
    for candidate in &candidates {
        best_by_skipped_count
            .entry(candidate.unplaced_piece_ids.len())
            .or_insert_with(|| candidate.clone());
    }
    for candidate in best_by_skipped_count.into_values() {
        if selected.len() >= settings.max_partial_layouts {
            break;
        }
        if selected_keys.insert(partial_layout_state_key(&candidate)) {
            selected.push(candidate);
        }
    }
    let mut frontier_representatives = candidates.clone();
    frontier_representatives
        .sort_by(|first, second| compare_partial_layout_frontier(first, second, settings));
    for candidate in frontier_representatives.into_iter().take(4) {
        if selected.len() >= settings.max_partial_layouts {
            break;
        }
        if selected_keys.insert(partial_layout_state_key(&candidate)) {
            selected.push(candidate);
        }
    }
    let mut best_by_last_orientation = BTreeMap::<(i64, bool), PartialLayout>::new();
    for candidate in &candidates {
        let Some(last) = candidate.placed.last() else {
            continue;
        };
        best_by_last_orientation
            .entry((
                angle_key(last.placement.rotation_deg),
                last.placement.mirrored,
            ))
            .or_insert_with(|| candidate.clone());
    }
    let mut orientation_representatives =
        best_by_last_orientation.into_values().collect::<Vec<_>>();
    orientation_representatives.sort_by(|first, second| {
        compare_partial_layout_orientation(first, second)
            .then_with(|| compare_partial_layouts(first, second, settings))
    });
    for candidate in orientation_representatives {
        if selected.len() >= settings.max_partial_layouts {
            break;
        }
        if selected_keys.insert(partial_layout_state_key(&candidate)) {
            selected.push(candidate);
        }
    }
    let mut best_by_last_short_axis_bin = BTreeMap::<usize, PartialLayout>::new();
    for candidate in &candidates {
        let Some(last) = candidate.placed.last() else {
            continue;
        };
        let Some(bounds) = last.collision.bounds() else {
            continue;
        };
        let center = (bounds.min_x + bounds.max_x) / 2.0;
        let bin = short_axis_bin(center, settings, 8);
        best_by_last_short_axis_bin
            .entry(bin)
            .or_insert_with(|| candidate.clone());
    }
    for candidate in best_by_last_short_axis_bin.into_values() {
        if selected.len() >= settings.max_partial_layouts {
            break;
        }
        if selected_keys.insert(partial_layout_state_key(&candidate)) {
            selected.push(candidate);
        }
    }
    for candidate in candidates {
        if selected.len() >= settings.max_partial_layouts {
            break;
        }
        if selected_keys.insert(partial_layout_state_key(&candidate)) {
            selected.push(candidate);
        }
    }
    selected
}

fn compare_partial_layout_frontier(
    first: &PartialLayout,
    second: &PartialLayout,
    settings: GeneralFastSettings,
) -> Ordering {
    let first_frontier = layout_frontier_metrics(&first.placed, settings);
    let second_frontier = layout_frontier_metrics(&second.placed, settings);
    second
        .placed
        .len()
        .cmp(&first.placed.len())
        .then_with(|| {
            first_frontier
                .void_area_mm2
                .total_cmp(&second_frontier.void_area_mm2)
        })
        .then_with(|| {
            first_frontier
                .roughness_mm
                .total_cmp(&second_frontier.roughness_mm)
        })
        .then_with(|| compare_partial_layouts(first, second, settings))
}

#[derive(Clone, Copy)]
struct LayoutFrontierMetrics {
    void_area_mm2: f64,
    roughness_mm: f64,
}

fn layout_frontier_metrics(
    placed: &[PlacedState],
    settings: GeneralFastSettings,
) -> LayoutFrontierMetrics {
    frontier_metrics_from_geometry(placed.iter().map(|state| &state.collision), settings)
}

fn frontier_metrics_from_geometry<'a>(
    geometry: impl Iterator<Item = &'a PolygonSet>,
    settings: GeneralFastSettings,
) -> LayoutFrontierMetrics {
    const BIN_COUNT: usize = 64;
    let sheet_inset = collision_sheet_inset_mm(settings);
    let bin_width = collision_sheet_short_axis_mm(settings) / BIN_COUNT as f64;
    let mut heights = [0.0_f64; BIN_COUNT];
    let mut occupied_area_mm2 = 0.0;
    for polygon in geometry {
        occupied_area_mm2 += polygon.area_mm2();
        let Some(bounds) = polygon.bounds() else {
            continue;
        };
        for (index, height) in heights.iter_mut().enumerate() {
            let min_x = sheet_inset + index as f64 * bin_width;
            let max_x = min_x + bin_width;
            if bounds.max_x > min_x && bounds.min_x < max_x {
                *height = height.max((bounds.max_y - sheet_inset).max(0.0));
            }
        }
    }
    let frontier_area_mm2 = heights.iter().sum::<f64>() * bin_width;
    let roughness_mm = heights
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs())
        .sum::<f64>();
    LayoutFrontierMetrics {
        void_area_mm2: (frontier_area_mm2 - occupied_area_mm2).max(0.0),
        roughness_mm,
    }
}

fn compare_partial_layout_orientation(first: &PartialLayout, second: &PartialLayout) -> Ordering {
    let orientation = |state: &PartialLayout| {
        state
            .placed
            .last()
            .map(|placed| {
                (
                    angle_key(placed.placement.rotation_deg),
                    placed.placement.mirrored,
                )
            })
            .unwrap_or((0, false))
    };
    let first_orientation = orientation(first);
    let second_orientation = orientation(second);
    compare_orientation_keys(first_orientation, second_orientation)
}

fn partial_layout_state_key(state: &PartialLayout) -> PartialLayoutStateKey {
    let mut unplaced = state.unplaced_piece_ids.clone();
    unplaced.sort();
    (canonical_placed_key(&state.placed), unplaced)
}

fn canonical_placed_key(placed: &[PlacedState]) -> Vec<CanonicalPlacementKey> {
    let mut key = placed
        .iter()
        .map(|state| {
            (
                state.placement.piece_id.clone(),
                angle_key(state.placement.rotation_deg),
                state.placement.mirrored,
                grid_key(state.placement.translate_short_axis).unwrap_or(i64::MAX),
                grid_key(state.placement.translate_long_axis).unwrap_or(i64::MAX),
            )
        })
        .collect::<Vec<_>>();
    key.sort();
    key
}

fn result_from_partial_layout(
    state: PartialLayout,
    settings: GeneralFastSettings,
    exact_evaluations: usize,
) -> GeneralFastResult {
    let metrics = layout_metrics(&state.placed, settings);
    GeneralFastResult {
        placements: state
            .placed
            .into_iter()
            .map(|placed| placed.placement)
            .collect(),
        unplaced_piece_ids: state.unplaced_piece_ids,
        exact_evaluations,
        primary_exact_evaluations: 0,
        order_portfolio_exact_evaluations: 0,
        catalog_portfolio_exact_evaluations: 0,
        pairing_exact_evaluations: 0,
        beam_exact_evaluations: exact_evaluations,
        tightening_exact_evaluations: 0,
        tightening_passes_attempted: 0,
        tightening_passes_improved: 0,
        catalog_candidate_placed_count: None,
        catalog_candidate_depth_mm: None,
        pairing_candidate_placed_count: None,
        pairing_candidate_depth_mm: None,
        beam_candidate_placed_count: None,
        beam_candidate_depth_mm: None,
        exploratory_exact_evaluations: 0,
        repair_exact_evaluations: 0,
        local_angle_refinement_exact_evaluations: 0,
        repair_targets_considered: 0,
        order_variants_attempted: 1,
        catalog_variants_attempted: 1,
        order_portfolio_failed: false,
        catalog_portfolio_failed: false,
        pairing_failed: false,
        beam_failed: false,
        exploratory_failed: false,
        repair_failed: false,
        used_short_axis_span_mm: metrics.used_short_axis_span_mm,
        used_long_axis_depth_mm: metrics.used_long_axis_depth_mm,
        unused_short_axis_projection_mm: metrics.unused_short_axis_projection_mm,
        occupied_envelope_area_mm2: metrics.occupied_envelope_area_mm2,
    }
}

#[derive(Clone, Copy)]
struct LayoutMetrics {
    used_short_axis_span_mm: f64,
    used_long_axis_depth_mm: f64,
    unused_short_axis_projection_mm: f64,
    occupied_envelope_area_mm2: f64,
}

fn layout_metrics(placed: &[PlacedState], settings: GeneralFastSettings) -> LayoutMetrics {
    let layout_bounds = combined_bounds(placed);
    let used_short_axis_span_mm = layout_bounds
        .map(|bounds| bounds.max_x - bounds.min_x)
        .unwrap_or(0.0);
    let used_long_axis_depth_mm = layout_bounds
        .map(|bounds| bounds.max_y + collision_sheet_inset_mm(settings))
        .unwrap_or(0.0);
    let mut intervals = placed
        .iter()
        .filter_map(|state| {
            state
                .collision
                .bounds()
                .map(|bounds| (bounds.min_x, bounds.max_x))
        })
        .collect::<Vec<_>>();
    intervals.sort_by(|first, second| first.0.total_cmp(&second.0));
    let unused_short_axis_projection_mm =
        (collision_sheet_short_axis_mm(settings) - merged_interval_length(&intervals)).max(0.0);
    let occupied_envelope_area_mm2 = layout_bounds
        .map(|bounds| (bounds.max_x - bounds.min_x) * (bounds.max_y - bounds.min_y))
        .unwrap_or(0.0);
    LayoutMetrics {
        used_short_axis_span_mm,
        used_long_axis_depth_mm,
        unused_short_axis_projection_mm,
        occupied_envelope_area_mm2,
    }
}

fn select_optional_arm(
    mut primary: GeneralFastResult,
    exploratory: Result<GeneralFastResult, GeneralFastError>,
    exploratory_exact_evaluations: usize,
) -> GeneralFastResult {
    match exploratory {
        Ok(exploratory) => {
            debug_assert_eq!(exploratory.exact_evaluations, exploratory_exact_evaluations);
            select_best_arm(primary, exploratory)
        }
        Err(_) => {
            primary.exact_evaluations = primary
                .exact_evaluations
                .saturating_add(exploratory_exact_evaluations);
            primary.exploratory_exact_evaluations = primary
                .exploratory_exact_evaluations
                .saturating_add(exploratory_exact_evaluations);
            primary.exploratory_failed = true;
            primary
        }
    }
}

fn select_best_arm(
    mut primary: GeneralFastResult,
    mut exploratory: GeneralFastResult,
) -> GeneralFastResult {
    let total_exact_evaluations = primary
        .exact_evaluations
        .saturating_add(exploratory.exact_evaluations);
    let primary_exact_evaluations = primary.primary_exact_evaluations;
    let order_portfolio_exact_evaluations = primary.order_portfolio_exact_evaluations;
    let catalog_portfolio_exact_evaluations = primary.catalog_portfolio_exact_evaluations;
    let pairing_exact_evaluations = primary.pairing_exact_evaluations;
    let beam_exact_evaluations = primary.beam_exact_evaluations;
    let tightening_exact_evaluations = primary.tightening_exact_evaluations;
    let tightening_passes_attempted = primary.tightening_passes_attempted;
    let tightening_passes_improved = primary.tightening_passes_improved;
    let catalog_candidate_placed_count = primary.catalog_candidate_placed_count;
    let catalog_candidate_depth_mm = primary.catalog_candidate_depth_mm;
    let pairing_candidate_placed_count = primary.pairing_candidate_placed_count;
    let pairing_candidate_depth_mm = primary.pairing_candidate_depth_mm;
    let beam_candidate_placed_count = primary.beam_candidate_placed_count;
    let beam_candidate_depth_mm = primary.beam_candidate_depth_mm;
    let order_variants_attempted = primary.order_variants_attempted;
    let catalog_variants_attempted = primary.catalog_variants_attempted;
    let order_portfolio_failed = primary.order_portfolio_failed;
    let catalog_portfolio_failed = primary.catalog_portfolio_failed;
    let pairing_failed = primary.pairing_failed;
    let beam_failed = primary.beam_failed;
    let exploratory_exact_evaluations = exploratory.exact_evaluations;
    let repair_exact_evaluations = primary.repair_exact_evaluations;
    let local_angle_refinement_exact_evaluations = primary.local_angle_refinement_exact_evaluations;
    let repair_targets_considered = primary.repair_targets_considered;
    let repair_failed = primary.repair_failed;
    let selected = if compare_result_quality(&exploratory, &primary) == Ordering::Less {
        &mut exploratory
    } else {
        &mut primary
    };
    selected.exact_evaluations = total_exact_evaluations;
    selected.primary_exact_evaluations = primary_exact_evaluations;
    selected.order_portfolio_exact_evaluations = order_portfolio_exact_evaluations;
    selected.catalog_portfolio_exact_evaluations = catalog_portfolio_exact_evaluations;
    selected.pairing_exact_evaluations = pairing_exact_evaluations;
    selected.beam_exact_evaluations = beam_exact_evaluations;
    selected.tightening_exact_evaluations = tightening_exact_evaluations;
    selected.tightening_passes_attempted = tightening_passes_attempted;
    selected.tightening_passes_improved = tightening_passes_improved;
    selected.catalog_candidate_placed_count = catalog_candidate_placed_count;
    selected.catalog_candidate_depth_mm = catalog_candidate_depth_mm;
    selected.pairing_candidate_placed_count = pairing_candidate_placed_count;
    selected.pairing_candidate_depth_mm = pairing_candidate_depth_mm;
    selected.beam_candidate_placed_count = beam_candidate_placed_count;
    selected.beam_candidate_depth_mm = beam_candidate_depth_mm;
    selected.exploratory_exact_evaluations = exploratory_exact_evaluations;
    selected.repair_exact_evaluations = repair_exact_evaluations;
    selected.local_angle_refinement_exact_evaluations = local_angle_refinement_exact_evaluations;
    selected.repair_targets_considered = repair_targets_considered;
    selected.order_variants_attempted = order_variants_attempted;
    selected.catalog_variants_attempted = catalog_variants_attempted;
    selected.order_portfolio_failed = order_portfolio_failed;
    selected.catalog_portfolio_failed = catalog_portfolio_failed;
    selected.pairing_failed = pairing_failed;
    selected.beam_failed = beam_failed;
    selected.exploratory_failed = false;
    selected.repair_failed = repair_failed;
    selected.clone()
}

fn repair_result(
    result: GeneralFastResult,
    prepared: &[PreparedGeneralPiece<'_>],
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
) -> Result<GeneralFastResult, GeneralFastError> {
    if settings.max_repair_targets == 0 {
        return Ok(result);
    }
    let fallback = result.clone();
    let mut diagnostics = RepairDiagnostics::default();
    match attempt_repair_result(result, prepared, pieces, settings, &mut diagnostics) {
        Ok(result) => Ok(result),
        Err(_) => {
            let mut fallback = fallback;
            fallback.exact_evaluations = fallback
                .exact_evaluations
                .saturating_add(diagnostics.exact_evaluations);
            fallback.repair_exact_evaluations = diagnostics.exact_evaluations;
            fallback.local_angle_refinement_exact_evaluations =
                diagnostics.local_angle_exact_evaluations;
            fallback.repair_targets_considered = diagnostics.targets_considered;
            fallback.repair_failed = true;
            Ok(fallback)
        }
    }
}

#[derive(Default)]
struct RepairDiagnostics {
    exact_evaluations: usize,
    local_angle_exact_evaluations: usize,
    targets_considered: usize,
}

fn attempt_repair_result(
    mut result: GeneralFastResult,
    prepared: &[PreparedGeneralPiece<'_>],
    pieces: &[GeneralFastPiece<'_>],
    settings: GeneralFastSettings,
    diagnostics: &mut RepairDiagnostics,
) -> Result<GeneralFastResult, GeneralFastError> {
    let prepared_by_id = prepared
        .iter()
        .map(|piece| (piece.input.id, piece))
        .collect::<BTreeMap<_, _>>();
    let mut placed = result
        .placements
        .iter()
        .map(|placement| {
            let piece = prepared_by_id
                .get(placement.piece_id.as_str())
                .ok_or_else(|| {
                    GeneralFastError::Geometry(GeneralPolygonError::from_message(
                        "a result placement must reference a prepared piece",
                    ))
                })?;
            Ok(PlacedState {
                input_index: piece.input_index,
                placement: placement.clone(),
                collision: transformed_collision(
                    piece,
                    placement.rotation_deg,
                    placement.mirrored,
                    placement.translate_short_axis,
                    placement.translate_long_axis,
                    settings,
                )?,
            })
        })
        .collect::<Result<Vec<_>, GeneralFastError>>()?;
    let target_input_indices = repair_target_input_indices(&placed, settings.max_repair_targets);
    for target_input_index in target_input_indices.iter().copied() {
        diagnostics.targets_considered += 1;
        let Some(target_position) = placed
            .iter()
            .position(|state| state.input_index == target_input_index)
        else {
            continue;
        };
        let incumbent_state = placed.remove(target_position);
        let target = prepared
            .iter()
            .find(|piece| piece.input_index == target_input_index)
            .expect("repair targets originate from prepared pieces");
        let search = search_reinsert_candidate(
            target,
            &incumbent_state,
            &placed,
            settings,
            &mut diagnostics.exact_evaluations,
            &mut diagnostics.local_angle_exact_evaluations,
        )?;

        let Some(candidate) = search.best else {
            placed.insert(target_position, incumbent_state);
            continue;
        };
        let mut trial_placed = placed.clone();
        trial_placed.insert(
            target_position,
            PlacedState {
                input_index: target_input_index,
                placement: GeneralFastPlacement {
                    piece_id: target.input.id.to_owned(),
                    rotation_deg: candidate.rotation_deg,
                    mirrored: candidate.mirrored,
                    translate_short_axis: candidate.translate_x,
                    translate_long_axis: candidate.translate_y,
                },
                collision: candidate.collision,
            },
        );
        if validate_result(pieces, &trial_placed, settings).is_err() {
            placed.insert(target_position, incumbent_state);
            continue;
        }
        let trial_result = result_with_placements(&result, &trial_placed, settings);
        if compare_result_quality(&trial_result, &result) == Ordering::Less {
            placed = trial_placed;
            result = trial_result;
        } else {
            placed.insert(target_position, incumbent_state);
        }
    }
    result.exact_evaluations = result
        .exact_evaluations
        .saturating_add(diagnostics.exact_evaluations);
    result.repair_exact_evaluations = diagnostics.exact_evaluations;
    result.local_angle_refinement_exact_evaluations = diagnostics.local_angle_exact_evaluations;
    result.repair_targets_considered = diagnostics.targets_considered;
    Ok(result)
}

fn result_with_placements(
    template: &GeneralFastResult,
    placed: &[PlacedState],
    settings: GeneralFastSettings,
) -> GeneralFastResult {
    let metrics = layout_metrics(placed, settings);
    let mut result = template.clone();
    result.placements = placed.iter().map(|state| state.placement.clone()).collect();
    result.used_short_axis_span_mm = metrics.used_short_axis_span_mm;
    result.used_long_axis_depth_mm = metrics.used_long_axis_depth_mm;
    result.unused_short_axis_projection_mm = metrics.unused_short_axis_projection_mm;
    result.occupied_envelope_area_mm2 = metrics.occupied_envelope_area_mm2;
    result
}

fn repair_target_input_indices(placed: &[PlacedState], max_targets: usize) -> Vec<usize> {
    let current_depth = combined_bounds(placed)
        .map(|bounds| bounds.max_y)
        .unwrap_or(0.0);
    let mut targets = placed
        .iter()
        .map(|state| {
            let without_depth = placed
                .iter()
                .filter(|candidate| candidate.input_index != state.input_index)
                .filter_map(|candidate| candidate.collision.bounds())
                .map(|bounds| bounds.max_y)
                .max_by(f64::total_cmp)
                .unwrap_or(0.0);
            let max_y = state
                .collision
                .bounds()
                .map(|bounds| bounds.max_y)
                .unwrap_or(0.0);
            (
                state.input_index,
                current_depth - without_depth,
                max_y,
                state.placement.piece_id.as_str(),
            )
        })
        .collect::<Vec<_>>();
    targets.sort_by(|first, second| {
        second
            .1
            .total_cmp(&first.1)
            .then_with(|| second.2.total_cmp(&first.2))
            .then_with(|| first.3.cmp(second.3))
            .then_with(|| first.0.cmp(&second.0))
    });
    targets
        .into_iter()
        .take(max_targets)
        .map(|target| target.0)
        .collect()
}

struct ReinsertSearch {
    best: Option<Candidate>,
}

fn search_reinsert_candidate(
    target: &PreparedGeneralPiece<'_>,
    incumbent: &PlacedState,
    fixed: &[PlacedState],
    settings: GeneralFastSettings,
    repair_exact_evaluations: &mut usize,
    local_angle_refinement_exact_evaluations: &mut usize,
) -> Result<ReinsertSearch, GeneralFastError> {
    let local_budget = settings.max_local_angle_refinement_evaluations_per_piece;
    let coarse_budget = settings
        .max_repair_evaluations_per_piece
        .saturating_sub(local_budget);
    let incumbent_orientation = (
        angle_key(incumbent.placement.rotation_deg),
        incumbent.placement.mirrored,
    );
    let mut seen = BTreeSet::from([incumbent_orientation]);
    let mut coarse_orientations = vec![(
        angle_from_key(incumbent_orientation.0),
        incumbent_orientation.1,
    )];
    for (angle, mirrored) in angle_candidates(target, fixed, settings, AngleScope::Full) {
        if seen.insert((angle_key(angle), mirrored)) {
            coarse_orientations.push((angle, mirrored));
        }
    }
    let coarse = best_candidate_for_orientations(
        target,
        fixed,
        settings,
        &coarse_orientations,
        coarse_budget,
        1,
    )?;
    *repair_exact_evaluations = repair_exact_evaluations.saturating_add(coarse.exact_evaluations);
    let incumbent_candidate = Candidate {
        key: CandidateKey {
            angle_key: incumbent_orientation.0,
            mirrored: incumbent_orientation.1,
            kind: CandidateKind::SheetSupport,
            fixed_piece_id_rank: 0,
            fixed_feature_ordinal: 0,
            moving_feature_ordinal: 0,
            translate_x_grid: grid_key(incumbent.placement.translate_short_axis)
                .unwrap_or(i64::MAX),
            translate_y_grid: grid_key(incumbent.placement.translate_long_axis).unwrap_or(i64::MAX),
        },
        rotation_deg: incumbent.placement.rotation_deg,
        mirrored: incumbent.placement.mirrored,
        translate_x: incumbent.placement.translate_short_axis,
        translate_y: incumbent.placement.translate_long_axis,
        collision: incumbent.collision.clone(),
    };
    let mut best = candidate_is_feasible(&incumbent_candidate, fixed, settings)?
        .then_some(incumbent_candidate);
    if let Some(candidate) = coarse.candidates.into_iter().next() {
        if best.as_ref().is_none_or(|incumbent| {
            compare_candidate_scores(
                score_candidate(fixed, &candidate, settings),
                candidate.key,
                score_candidate(fixed, incumbent, settings),
                incumbent.key,
            ) == Ordering::Less
        }) {
            best = Some(candidate);
        }
    }
    if local_budget > 0 && target.input.allow_rotation {
        if let Some(anchor) = &best {
            let local_orientations = local_angle_neighborhood(
                anchor.rotation_deg,
                anchor.mirrored,
                settings.angle_seed_count,
            );
            let local = best_candidate_for_orientations(
                target,
                fixed,
                settings,
                &local_orientations,
                local_budget,
                1,
            )?;
            *repair_exact_evaluations =
                repair_exact_evaluations.saturating_add(local.exact_evaluations);
            *local_angle_refinement_exact_evaluations =
                local_angle_refinement_exact_evaluations.saturating_add(local.exact_evaluations);
            if let Some(candidate) = local.candidates.into_iter().next() {
                if best.as_ref().is_none_or(|incumbent| {
                    compare_candidate_scores(
                        score_candidate(fixed, &candidate, settings),
                        candidate.key,
                        score_candidate(fixed, incumbent, settings),
                        incumbent.key,
                    ) == Ordering::Less
                }) {
                    best = Some(candidate);
                }
            }
        }
    }
    Ok(ReinsertSearch { best })
}

fn candidate_is_feasible(
    candidate: &Candidate,
    fixed: &[PlacedState],
    settings: GeneralFastSettings,
) -> Result<bool, GeneralFastError> {
    if !collision_fits_sheet(&candidate.collision, settings) {
        return Ok(false);
    }
    for placed in fixed {
        if polygons_overlap_exact(&candidate.collision, &placed.collision)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn publication_confirmed_candidate(
    piece: &PreparedGeneralPiece<'_>,
    mut candidate: Candidate,
    fixed: &[PlacedState],
    settings: GeneralFastSettings,
) -> Result<Option<Candidate>, GeneralFastError> {
    let _span = profiling::span(Phase::PublicationConfirm);
    candidate.collision = transformed_collision(
        piece,
        candidate.rotation_deg,
        candidate.mirrored,
        candidate.translate_x,
        candidate.translate_y,
        settings,
    )?;
    candidate_is_feasible(&candidate, fixed, settings).map(|feasible| feasible.then_some(candidate))
}

struct CandidateSearch {
    candidates: Vec<Candidate>,
    exact_evaluations: usize,
}

fn best_candidate_for_orientations(
    target: &PreparedGeneralPiece<'_>,
    fixed: &[PlacedState],
    settings: GeneralFastSettings,
    orientations: &[(f64, bool)],
    exact_budget: usize,
    max_results: usize,
) -> Result<CandidateSearch, GeneralFastError> {
    if exact_budget == 0 || orientations.is_empty() || max_results == 0 {
        return Ok(CandidateSearch {
            candidates: Vec::new(),
            exact_evaluations: 0,
        });
    }
    let baseline_proposal_budget = exact_budget.saturating_mul(PROPOSAL_BUDGET_MULTIPLIER);
    let core_orientation_count = orientations
        .iter()
        .filter(|(rotation_deg, _)| is_quarter_turn(*rotation_deg))
        .count()
        .max(1);
    let expanded_orientation_count = orientations.len().saturating_sub(core_orientation_count);
    let core_per_angle_budget = baseline_proposal_budget
        .div_ceil(core_orientation_count)
        .max(1);
    let expanded_per_angle_budget = (core_per_angle_budget / 2).max(1);
    let proposal_budget = core_per_angle_budget
        .saturating_mul(core_orientation_count)
        .saturating_add(expanded_per_angle_budget.saturating_mul(expanded_orientation_count));
    let attempt_budget = proposal_budget.saturating_mul(PROPOSAL_BUDGET_MULTIPLIER);
    let mut remaining_proposals = proposal_budget;
    let mut remaining_attempts = attempt_budget;
    let mut proposals = Vec::new();
    let mut oriented_by_key = BTreeMap::<(i64, bool), PolygonSet>::new();
    for (rotation_deg, mirrored) in orientations.iter().copied() {
        if remaining_proposals == 0 || remaining_attempts == 0 {
            break;
        }
        let Ok(oriented) = oriented_collision(target, rotation_deg, mirrored, settings) else {
            continue;
        };
        let per_angle_budget = if is_quarter_turn(rotation_deg) {
            core_per_angle_budget
        } else {
            expanded_per_angle_budget
        };
        let angle_proposal_budget = per_angle_budget.min(remaining_proposals);
        let angle_attempt_budget = angle_proposal_budget
            .saturating_mul(PROPOSAL_BUDGET_MULTIPLIER)
            .min(remaining_attempts);
        let (angle_proposals, angle_attempts) = translation_proposals(
            rotation_deg,
            mirrored,
            TranslationProposalInput {
                oriented: &oriented,
                placed: fixed,
                settings,
                max_proposals: angle_proposal_budget,
                max_attempts: angle_attempt_budget,
                fixed_piece_order_strategy: FixedPieceOrder::ShortSideFrontier,
                contact_coverage: ContactCoverage::Fair,
            },
        )?;
        remaining_proposals = remaining_proposals.saturating_sub(angle_proposals.len());
        remaining_attempts = remaining_attempts.saturating_sub(angle_attempts);
        proposals.extend(angle_proposals);
        oriented_by_key.insert((angle_key(rotation_deg), mirrored), oriented);
    }
    let proposals = shortlist_proposals(proposals, exact_budget);
    let mut candidates = Vec::new();
    let mut exact_evaluations = 0usize;
    for proposal in proposals {
        exact_evaluations += 1;
        let oriented = oriented_by_key
            .get(&(proposal.key.angle_key, proposal.mirrored))
            .expect("repair proposals retain their oriented polygon");
        let Ok(collision) = oriented.translated(proposal.translate_x, proposal.translate_y) else {
            continue;
        };
        let candidate = Candidate {
            key: proposal.key,
            rotation_deg: proposal.rotation_deg,
            mirrored: proposal.mirrored,
            translate_x: proposal.translate_x,
            translate_y: proposal.translate_y,
            collision,
        };
        if !collision_fits_sheet(&candidate.collision, settings) {
            continue;
        }
        if fixed
            .iter()
            .map(|placed| polygons_overlap_exact(&candidate.collision, &placed.collision))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .any(std::convert::identity)
        {
            continue;
        }
        candidates.push(candidate);
    }
    candidates.sort_by(|first, second| {
        compare_candidate_scores(
            score_candidate(fixed, first, settings),
            first.key,
            score_candidate(fixed, second, settings),
            second.key,
        )
    });
    let mut seen_geometry = BTreeSet::new();
    candidates.retain(|candidate| seen_geometry.insert(polygon_absolute_key(&candidate.collision)));
    candidates = select_diverse_candidates(candidates, max_results, settings);
    candidates = candidates
        .into_iter()
        .filter_map(|candidate| {
            publication_confirmed_candidate(target, candidate, fixed, settings).transpose()
        })
        .collect::<Result<Vec<_>, _>>()?;
    candidates.sort_by(|first, second| {
        compare_candidate_scores(
            score_candidate(fixed, first, settings),
            first.key,
            score_candidate(fixed, second, settings),
            second.key,
        )
    });
    debug_assert!(exact_evaluations <= exact_budget);
    Ok(CandidateSearch {
        candidates,
        exact_evaluations,
    })
}

fn select_diverse_candidates(
    candidates: Vec<Candidate>,
    max_results: usize,
    settings: GeneralFastSettings,
) -> Vec<Candidate> {
    if candidates.len() <= max_results || max_results == 1 {
        return candidates.into_iter().take(max_results).collect();
    }
    let mut selected = vec![candidates[0].clone()];
    let mut seen_candidates = BTreeSet::from([candidates[0].key]);
    let mut best_by_orientation = BTreeMap::<(i64, bool), Candidate>::new();
    for candidate in &candidates {
        best_by_orientation
            .entry((candidate.key.angle_key, candidate.mirrored))
            .or_insert_with(|| candidate.clone());
    }
    let mut orientations = best_by_orientation.into_values().collect::<Vec<_>>();
    orientations.sort_by(|first, second| {
        compare_orientation_diversity(first, second).then_with(|| {
            compare_candidate_scores(
                score_single_candidate(first),
                first.key,
                score_single_candidate(second),
                second.key,
            )
        })
    });
    let orientation_target = max_results.div_ceil(2);
    for candidate in orientations {
        if selected.len() >= orientation_target {
            break;
        }
        if seen_candidates.insert(candidate.key) {
            selected.push(candidate);
        }
    }
    let mut best_by_short_axis_bin = BTreeMap::<usize, Candidate>::new();
    for candidate in &candidates {
        let Some(bounds) = candidate.collision.bounds() else {
            continue;
        };
        let center = (bounds.min_x + bounds.max_x) / 2.0;
        let bin = short_axis_bin(center, settings, 8);
        best_by_short_axis_bin
            .entry(bin)
            .or_insert_with(|| candidate.clone());
    }
    for candidate in best_by_short_axis_bin.into_values() {
        if selected.len() >= max_results {
            break;
        }
        if seen_candidates.insert(candidate.key) {
            selected.push(candidate);
        }
    }
    for candidate in candidates {
        if selected.len() >= max_results {
            break;
        }
        if seen_candidates.insert(candidate.key) {
            selected.push(candidate);
        }
    }
    selected
}

fn compare_orientation_diversity(first: &Candidate, second: &Candidate) -> Ordering {
    compare_orientation_keys(
        (first.key.angle_key, first.mirrored),
        (second.key.angle_key, second.mirrored),
    )
}

fn compare_orientation_keys(first: (i64, bool), second: (i64, bool)) -> Ordering {
    let quarter_turn_key = angle_key(90.0);
    let first_remainder = first.0.rem_euclid(quarter_turn_key);
    let second_remainder = second.0.rem_euclid(quarter_turn_key);
    let first_distance = first_remainder.min(quarter_turn_key - first_remainder);
    let second_distance = second_remainder.min(quarter_turn_key - second_remainder);
    first_distance
        .cmp(&second_distance)
        .then_with(|| first.0.cmp(&second.0))
        .then_with(|| first.1.cmp(&second.1))
}

fn score_single_candidate(candidate: &Candidate) -> CandidateScore {
    let bounds = candidate
        .collision
        .bounds()
        .expect("candidate geometry is non-empty");
    CandidateScore {
        candidate_long_axis_position: 3.0 * bounds.min_y + bounds.max_y,
        candidate_short_axis_position: 3.0 * bounds.min_x + bounds.max_x,
        long_axis_depth: bounds.max_y,
        unused_short_axis_projection: bounds.min_x,
        envelope_area: (bounds.max_x - bounds.min_x) * (bounds.max_y - bounds.min_y),
    }
}

fn local_angle_neighborhood(
    anchor_angle_deg: f64,
    mirrored: bool,
    angle_seed_count: usize,
) -> Vec<(f64, bool)> {
    let full_turn_key = angle_key(360.0 - 1.0 / ANGLE_KEY_SCALE) + 1;
    let step_key = (full_turn_key / angle_seed_count as i64).max(4);
    let anchor_key = angle_key(anchor_angle_deg);
    let mut keys = BTreeSet::new();
    for offset in [-(step_key / 2), -(step_key / 4), step_key / 4, step_key / 2] {
        keys.insert((anchor_key + offset).rem_euclid(full_turn_key));
    }
    keys.into_iter()
        .filter(|key| *key != anchor_key)
        .map(|key| (angle_from_key(key), mirrored))
        .collect()
}

fn compare_result_quality(first: &GeneralFastResult, second: &GeneralFastResult) -> Ordering {
    second
        .placements
        .len()
        .cmp(&first.placements.len())
        .then_with(|| {
            first
                .used_long_axis_depth_mm
                .total_cmp(&second.used_long_axis_depth_mm)
        })
        .then_with(|| {
            first
                .unused_short_axis_projection_mm
                .total_cmp(&second.unused_short_axis_projection_mm)
        })
        .then_with(|| {
            first
                .occupied_envelope_area_mm2
                .total_cmp(&second.occupied_envelope_area_mm2)
        })
        .then_with(|| canonical_result_key(first).cmp(&canonical_result_key(second)))
}

fn canonical_result_key(result: &GeneralFastResult) -> Vec<CanonicalPlacementKey> {
    let mut key = result
        .placements
        .iter()
        .map(|placement| {
            (
                placement.piece_id.clone(),
                angle_key(placement.rotation_deg),
                placement.mirrored,
                grid_key(placement.translate_short_axis).unwrap_or(i64::MAX),
                grid_key(placement.translate_long_axis).unwrap_or(i64::MAX),
            )
        })
        .collect::<Vec<_>>();
    key.sort();
    key
}

fn combined_bounds(placed: &[PlacedState]) -> Option<crate::domain::IrregularBounds> {
    let mut bounds = placed.iter().filter_map(|state| state.collision.bounds());
    let first = bounds.next()?;
    Some(bounds.fold(first, |combined, current| {
        crate::domain::IrregularBounds::new(
            combined.min_x.min(current.min_x),
            combined.min_y.min(current.min_y),
            combined.max_x.max(current.max_x),
            combined.max_y.max(current.max_y),
        )
    }))
}

pub(crate) fn effective_sheet_edge_clearance_mm(settings: GeneralFastSettings) -> f64 {
    settings
        .sheet_edge_clearance_mm
        .unwrap_or(settings.total_padding_mm / 2.0)
}

pub(crate) fn collision_expansion_mm(settings: GeneralFastSettings) -> f64 {
    settings.total_padding_mm / 2.0
        + settings.clearance_safety_margin_mm
        + settings.search_offset_allowance_mm
}

fn oriented_collision(
    piece: &PreparedGeneralPiece<'_>,
    rotation_deg: f64,
    mirrored: bool,
    settings: GeneralFastSettings,
) -> Result<PolygonSet, GeneralPolygonError> {
    transformed_collision(piece, rotation_deg, mirrored, 0.0, 0.0, settings)
}

fn transformed_collision(
    piece: &PreparedGeneralPiece<'_>,
    rotation_deg: f64,
    mirrored: bool,
    translate_x: f64,
    translate_y: f64,
    settings: GeneralFastSettings,
) -> Result<PolygonSet, GeneralPolygonError> {
    let _span = profiling::span(Phase::CollisionPolygonBuild);
    profiling::count(Counter::CollisionPolygonBuilds, 1);
    piece
        .input
        .polygon
        .transformed(rotation_deg, mirrored, translate_x, translate_y)?
        .offset(collision_expansion_mm(settings))
}

pub(crate) fn collision_sheet_inset_mm(settings: GeneralFastSettings) -> f64 {
    effective_sheet_edge_clearance_mm(settings) - settings.total_padding_mm / 2.0
}

pub(crate) fn collision_sheet_short_axis_mm(settings: GeneralFastSettings) -> f64 {
    settings.sheet_short_axis_mm - 2.0 * collision_sheet_inset_mm(settings)
}

pub(crate) fn collision_sheet_long_axis_mm(settings: GeneralFastSettings) -> f64 {
    settings.sheet_long_axis_mm - 2.0 * collision_sheet_inset_mm(settings)
}

fn collision_fits_sheet(polygon: &PolygonSet, settings: GeneralFastSettings) -> bool {
    let _span = profiling::span(Phase::SheetFitTest);
    let inset = collision_sheet_inset_mm(settings);
    polygon.fits_rect(
        inset,
        inset,
        settings.sheet_short_axis_mm - inset,
        settings.sheet_long_axis_mm - inset,
    )
}

pub(crate) fn polygons_overlap_exact(
    first: &PolygonSet,
    second: &PolygonSet,
) -> Result<bool, GeneralPolygonError> {
    let (Some(first_bounds), Some(second_bounds)) = (first.bounds(), second.bounds()) else {
        return Err(GeneralPolygonError::from_message(
            "an exact overlap query requires non-empty polygons",
        ));
    };
    if !bounds_have_positive_overlap(first_bounds, second_bounds) {
        return Ok(false);
    }
    // Instrumented past the broad-phase reject on purpose. The reject arm runs
    // hundreds of millions of times in a deep-operator stream, and it is not
    // exact-overlap work anyway - it is the bounds filter in front of it.
    // Guarding it measurably slowed the stream; guarding only the narrow phase
    // measures the cost that matters and costs nothing on the common path.
    let _span = profiling::span(Phase::ExactOverlapTest);
    profiling::count(Counter::ExactPairTests, 1);
    Ok(first.intersection_area_mm2(second)? > 0.0)
}

fn short_axis_bin(center: f64, settings: GeneralFastSettings, bin_count: usize) -> usize {
    let normalized =
        (center - collision_sheet_inset_mm(settings)) / collision_sheet_short_axis_mm(settings);
    (normalized * bin_count as f64)
        .floor()
        .clamp(0.0, (bin_count - 1) as f64) as usize
}

fn validate_settings(settings: GeneralFastSettings) -> Result<(), GeneralFastError> {
    for (name, value) in [
        ("sheet short axis", settings.sheet_short_axis_mm),
        ("sheet long axis", settings.sheet_long_axis_mm),
        ("total padding", settings.total_padding_mm),
        (
            "clearance safety margin",
            settings.clearance_safety_margin_mm,
        ),
        (
            "flattening sag tolerance",
            settings.flattening_sag_tolerance_mm,
        ),
        (
            "search offset allowance",
            settings.search_offset_allowance_mm,
        ),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(GeneralFastError::InvalidSettings(format!(
                "{name} must be finite and non-negative"
            )));
        }
    }
    if settings.sheet_short_axis_mm == 0.0 || settings.sheet_long_axis_mm == 0.0 {
        return Err(GeneralFastError::InvalidSettings(
            "sheet axes must be positive".to_owned(),
        ));
    }
    if settings
        .sheet_edge_clearance_mm
        .is_some_and(|value| !value.is_finite() || value < 0.0)
    {
        return Err(GeneralFastError::InvalidSettings(
            "sheet edge clearance must be finite and non-negative".to_owned(),
        ));
    }
    if collision_sheet_short_axis_mm(settings) <= 0.0
        || collision_sheet_long_axis_mm(settings) <= 0.0
    {
        return Err(GeneralFastError::InvalidSettings(
            "sheet edge clearance leaves no usable sheet interior".to_owned(),
        ));
    }
    if settings.clearance_safety_margin_mm < settings.flattening_sag_tolerance_mm {
        return Err(GeneralFastError::InvalidSettings(
            "clearance safety margin must cover the flattening sag tolerance".to_owned(),
        ));
    }
    if settings.angle_seed_count == 0
        || settings.max_angles_per_piece == 0
        || settings.max_evaluations_per_piece == 0
        || settings.max_order_variants == 0
        || settings.max_catalog_variants == 0
        || settings.max_pairing_band_variants == 0
        || settings.max_partial_layouts == 0
    {
        return Err(GeneralFastError::InvalidSettings(
            "constructor quotas must be positive".to_owned(),
        ));
    }
    if settings.angle_seed_count > DEFAULT_ANGLE_SEED_COUNT
        || settings.max_angles_per_piece > DEFAULT_MAX_ANGLES_PER_PIECE
        || settings.max_evaluations_per_piece > DEFAULT_MAX_EVALUATIONS_PER_PIECE
        || settings.max_exploratory_evaluations_per_piece > DEFAULT_MAX_EVALUATIONS_PER_PIECE
        || settings.max_order_variants > DEFAULT_MAX_ORDER_VARIANTS
        || settings.max_catalog_variants > DEFAULT_MAX_CATALOG_VARIANTS
        || settings.max_pairing_band_variants > DEFAULT_MAX_PAIRING_BAND_VARIANTS
        || settings.max_partial_layouts > DEFAULT_MAX_PARTIAL_LAYOUTS
        || settings.max_tightening_passes > DEFAULT_MAX_TIGHTENING_PASSES
        || settings.max_beam_evaluations_per_state > DEFAULT_MAX_EVALUATIONS_PER_PIECE
        || settings.max_catalog_evaluations_per_piece > DEFAULT_MAX_EVALUATIONS_PER_PIECE
        || settings.max_pairing_evaluations_per_piece > DEFAULT_MAX_EVALUATIONS_PER_PIECE
        || settings.max_repair_targets > DEFAULT_MAX_REPAIR_TARGETS
        || settings.max_repair_evaluations_per_piece > DEFAULT_MAX_EVALUATIONS_PER_PIECE
        || settings.max_local_angle_refinement_evaluations_per_piece
            > DEFAULT_MAX_EVALUATIONS_PER_PIECE
    {
        return Err(GeneralFastError::InvalidSettings(
            "constructor quotas exceed the supported deterministic limits".to_owned(),
        ));
    }
    if settings.max_catalog_variants > 1 && settings.max_catalog_evaluations_per_piece == 0 {
        return Err(GeneralFastError::InvalidSettings(
            "catalog variants require a positive catalog evaluation quota".to_owned(),
        ));
    }
    if settings.max_partial_layouts > 1 && settings.max_beam_evaluations_per_state == 0 {
        return Err(GeneralFastError::InvalidSettings(
            "multiple partial layouts require a positive beam evaluation quota".to_owned(),
        ));
    }
    if settings.max_partial_layouts == 1 && settings.max_beam_evaluations_per_state > 0 {
        return Err(GeneralFastError::InvalidSettings(
            "a beam evaluation quota requires multiple partial layouts".to_owned(),
        ));
    }
    if settings.max_tightening_passes > 0 && settings.max_partial_layouts == 1 {
        return Err(GeneralFastError::InvalidSettings(
            "tightening passes require multiple partial layouts".to_owned(),
        ));
    }
    if settings.max_local_angle_refinement_evaluations_per_piece
        > settings.max_repair_evaluations_per_piece
        || (settings.max_local_angle_refinement_evaluations_per_piece > 0
            && settings.max_local_angle_refinement_evaluations_per_piece
                == settings.max_repair_evaluations_per_piece)
    {
        return Err(GeneralFastError::InvalidSettings(
            "local angle refinement must be a strict subquota of repair evaluations".to_owned(),
        ));
    }
    if settings.max_repair_targets > 0 && settings.max_repair_evaluations_per_piece == 0 {
        return Err(GeneralFastError::InvalidSettings(
            "repair targets require a positive repair evaluation quota".to_owned(),
        ));
    }
    Ok(())
}

fn compare_prepared_pieces(
    first: &PreparedGeneralPiece<'_>,
    second: &PreparedGeneralPiece<'_>,
) -> Ordering {
    second
        .area_mm2
        .total_cmp(&first.area_mm2)
        .then_with(|| second.diameter_mm.total_cmp(&first.diameter_mm))
        .then_with(|| second.reflex_vertices.cmp(&first.reflex_vertices))
        .then_with(|| first.input.id.cmp(second.input.id))
}

fn piece_order_portfolio<'a>(
    prepared: &[PreparedGeneralPiece<'a>],
    max_order_variants: usize,
) -> Vec<Vec<PreparedGeneralPiece<'a>>> {
    let topology_complex_piece_count = prepared
        .iter()
        .filter(|piece| piece.reflex_vertices > 0)
        .count();
    let topology_complex_job = topology_complex_piece_count.saturating_mul(3) >= prepared.len();
    let strategies = if topology_complex_job {
        [
            PieceOrderStrategy::AreaDiameterReflex,
            PieceOrderStrategy::LongSpan,
            PieceOrderStrategy::HullAreaDiameter,
            PieceOrderStrategy::Concavity,
            PieceOrderStrategy::Elongation,
        ]
    } else {
        [
            PieceOrderStrategy::LongSpan,
            PieceOrderStrategy::HullAreaDiameter,
            PieceOrderStrategy::AreaDiameterReflex,
            PieceOrderStrategy::Concavity,
            PieceOrderStrategy::Elongation,
        ]
    };
    let mut seen = BTreeSet::<Vec<String>>::new();
    let mut portfolio = Vec::new();
    for strategy in strategies.into_iter().take(max_order_variants) {
        let mut order = prepared.to_vec();
        order.sort_by(|first, second| compare_prepared_by_strategy(first, second, strategy));
        let key = order
            .iter()
            .map(|piece| piece.input.id.to_owned())
            .collect::<Vec<_>>();
        if seen.insert(key) {
            portfolio.push(order);
        }
    }
    debug_assert!(!portfolio.is_empty());
    portfolio
}

fn shape_family_order<'a>(prepared: &[PreparedGeneralPiece<'a>]) -> Vec<PreparedGeneralPiece<'a>> {
    let mut families = BTreeMap::<Vec<i64>, Vec<PreparedGeneralPiece<'a>>>::new();
    for piece in prepared.iter().cloned() {
        families
            .entry(piece.shape_family_key.clone())
            .or_default()
            .push(piece);
    }
    let mut families = families.into_values().collect::<Vec<_>>();
    for family in &mut families {
        family.sort_by(|first, second| first.input.id.cmp(second.input.id));
    }
    families.sort_by(|first, second| {
        second[0]
            .area_mm2
            .total_cmp(&first[0].area_mm2)
            .then_with(|| second[0].diameter_mm.total_cmp(&first[0].diameter_mm))
            .then_with(|| second[0].reflex_vertices.cmp(&first[0].reflex_vertices))
            .then_with(|| second.len().cmp(&first.len()))
            .then_with(|| first[0].shape_family_key.cmp(&second[0].shape_family_key))
    });
    families.into_iter().flatten().collect()
}

fn polygon_convex_hull_area_mm2(polygon: &PolygonSet) -> f64 {
    let hull = compute_convex_hull(&contour_points(polygon));
    if hull.points.len() < 3 {
        return 0.0;
    }
    hull.points
        .iter()
        .zip(hull.points.iter().cycle().skip(1))
        .take(hull.points.len())
        .map(|(first, second)| first.x * second.y - second.x * first.y)
        .sum::<f64>()
        .abs()
        / 2.0
}

fn fast_band_depth(
    prepared: &[PreparedGeneralPiece<'_>],
    settings: GeneralFastSettings,
    factor: f64,
) -> f64 {
    let sheet_inset = collision_sheet_inset_mm(settings);
    let area_lower_bound = prepared
        .iter()
        .map(|piece| piece.collision.area_mm2())
        .sum::<f64>()
        / collision_sheet_short_axis_mm(settings);
    let piece_lower_bound = prepared
        .iter()
        .filter_map(|piece| piece.collision.bounds().map(|bounds| (piece, bounds)))
        .map(|(piece, bounds)| {
            let width = bounds.max_x - bounds.min_x;
            let height = bounds.max_y - bounds.min_y;
            if piece.input.allow_rotation {
                width.min(height)
            } else {
                height
            }
        })
        .max_by(f64::total_cmp)
        .unwrap_or(0.0);
    let lower_bound = area_lower_bound.max(piece_lower_bound) * factor + 2.0 * sheet_inset;
    ((lower_bound * 1_000.0).ceil() / 1_000.0).min(settings.sheet_long_axis_mm)
}

fn compare_prepared_by_strategy(
    first: &PreparedGeneralPiece<'_>,
    second: &PreparedGeneralPiece<'_>,
    strategy: PieceOrderStrategy,
) -> Ordering {
    match strategy {
        PieceOrderStrategy::AreaDiameterReflex => compare_prepared_pieces(first, second),
        PieceOrderStrategy::HullAreaDiameter => {
            let first_difficulty = first.convex_hull_area_mm2 * first.diameter_mm;
            let second_difficulty = second.convex_hull_area_mm2 * second.diameter_mm;
            second_difficulty
                .total_cmp(&first_difficulty)
                .then_with(|| {
                    second
                        .convex_hull_area_mm2
                        .total_cmp(&first.convex_hull_area_mm2)
                })
                .then_with(|| second.diameter_mm.total_cmp(&first.diameter_mm))
                .then_with(|| second.area_mm2.total_cmp(&first.area_mm2))
                .then_with(|| second.reflex_vertices.cmp(&first.reflex_vertices))
                .then_with(|| first.input.id.cmp(second.input.id))
        }
        PieceOrderStrategy::LongSpan => second
            .long_span_mm
            .total_cmp(&first.long_span_mm)
            .then_with(|| second.short_span_mm.total_cmp(&first.short_span_mm))
            .then_with(|| second.area_mm2.total_cmp(&first.area_mm2))
            .then_with(|| second.reflex_vertices.cmp(&first.reflex_vertices))
            .then_with(|| first.input.id.cmp(second.input.id)),
        PieceOrderStrategy::Concavity => second
            .reflex_vertices
            .cmp(&first.reflex_vertices)
            .then_with(|| first.fill_ratio.total_cmp(&second.fill_ratio))
            .then_with(|| second.vertex_count.cmp(&first.vertex_count))
            .then_with(|| second.area_mm2.total_cmp(&first.area_mm2))
            .then_with(|| first.input.id.cmp(second.input.id)),
        PieceOrderStrategy::Elongation => {
            let first_elongation = first.long_span_mm / first.short_span_mm;
            let second_elongation = second.long_span_mm / second.short_span_mm;
            second_elongation
                .total_cmp(&first_elongation)
                .then_with(|| second.long_span_mm.total_cmp(&first.long_span_mm))
                .then_with(|| second.area_mm2.total_cmp(&first.area_mm2))
                .then_with(|| second.reflex_vertices.cmp(&first.reflex_vertices))
                .then_with(|| first.input.id.cmp(second.input.id))
        }
    }
}

fn polygon_shape_family_key(polygon: &PolygonSet) -> Vec<i64> {
    let bounds = polygon
        .bounds()
        .expect("prepared polygon geometry is non-empty");
    let origin_x = grid_key(bounds.min_x).expect("prepared bounds use the contractual grid");
    let origin_y = grid_key(bounds.min_y).expect("prepared bounds use the contractual grid");
    let mut key = Vec::with_capacity(polygon.vertex_count() * 2 + polygon.regions().len() * 4);
    key.push(polygon.regions().len() as i64);
    for region in polygon.regions() {
        key.push(region.outer.points().len() as i64);
        for point in region.outer.points() {
            key.push(
                grid_key(point.x).expect("prepared points use the contractual grid") - origin_x,
            );
            key.push(
                grid_key(point.y).expect("prepared points use the contractual grid") - origin_y,
            );
        }
        key.push(region.holes.len() as i64);
        for hole in &region.holes {
            key.push(hole.points().len() as i64);
            for point in hole.points() {
                key.push(
                    grid_key(point.x).expect("prepared points use the contractual grid") - origin_x,
                );
                key.push(
                    grid_key(point.y).expect("prepared points use the contractual grid") - origin_y,
                );
            }
        }
    }
    key
}

fn polygon_absolute_key(polygon: &PolygonSet) -> Vec<i64> {
    let mut key = Vec::with_capacity(polygon.vertex_count() * 2 + polygon.regions().len() * 4);
    key.push(polygon.regions().len() as i64);
    for region in polygon.regions() {
        key.push(region.outer.points().len() as i64);
        for point in region.outer.points() {
            key.push(grid_key(point.x).expect("candidate points use the contractual grid"));
            key.push(grid_key(point.y).expect("candidate points use the contractual grid"));
        }
        key.push(region.holes.len() as i64);
        for hole in &region.holes {
            key.push(hole.points().len() as i64);
            for point in hole.points() {
                key.push(grid_key(point.x).expect("candidate points use the contractual grid"));
                key.push(grid_key(point.y).expect("candidate points use the contractual grid"));
            }
        }
    }
    key
}

fn angle_candidates(
    piece: &PreparedGeneralPiece<'_>,
    placed: &[PlacedState],
    settings: GeneralFastSettings,
    scope: AngleScope,
) -> Vec<(f64, bool)> {
    let non_mirrored_angle_keys = orientation_angle_keys(piece, placed, settings, scope, false);
    let mirrored_angle_keys = piece
        .input
        .allow_mirror
        .then(|| orientation_angle_keys(piece, placed, settings, scope, true))
        .unwrap_or_default();
    let max_orientations = settings.max_angles_per_piece;
    let mirror_target = if piece.input.allow_mirror && max_orientations > 1 {
        (max_orientations / 4).max(1)
    } else {
        0
    };
    let non_mirror_target = max_orientations.saturating_sub(mirror_target);
    let mut orientations = non_mirrored_angle_keys
        .iter()
        .take(non_mirror_target)
        .map(|key| (angle_from_key(*key), false))
        .collect::<Vec<_>>();
    orientations.extend(
        mirrored_angle_keys
            .iter()
            .take(mirror_target)
            .map(|key| (angle_from_key(*key), true)),
    );
    if orientations.len() < max_orientations {
        for key in non_mirrored_angle_keys.iter().skip(non_mirror_target) {
            if orientations.len() == max_orientations {
                break;
            }
            orientations.push((angle_from_key(*key), false));
        }
    }
    orientations
}

fn orientation_angle_keys(
    piece: &PreparedGeneralPiece<'_>,
    placed: &[PlacedState],
    settings: GeneralFastSettings,
    scope: AngleScope,
    mirrored: bool,
) -> Vec<i64> {
    let discovery_attempt_limit = settings
        .max_angles_per_piece
        .saturating_mul(PROPOSAL_BUDGET_MULTIPLIER);
    let angle_key_limit = discovery_attempt_limit.max(4);
    let mut seen_angle_keys = BTreeSet::new();
    let mut angle_keys = Vec::new();
    push_angle_candidate(&mut angle_keys, &mut seen_angle_keys, 0.0, angle_key_limit);
    if piece.input.allow_rotation {
        push_angle_candidate(&mut angle_keys, &mut seen_angle_keys, 90.0, angle_key_limit);
        if scope == AngleScope::Full {
            let moving_edges =
                prioritized_edge_angles(&piece.collision, discovery_attempt_limit, mirrored);
            for moving_angle in &moving_edges {
                push_angle_candidate(
                    &mut angle_keys,
                    &mut seen_angle_keys,
                    -*moving_angle,
                    angle_key_limit,
                );
                push_angle_candidate(
                    &mut angle_keys,
                    &mut seen_angle_keys,
                    90.0 - *moving_angle,
                    angle_key_limit,
                );
            }
            let mut fixed_edges = placed
                .iter()
                .flat_map(|fixed| {
                    prioritized_edge_angles(&fixed.collision, discovery_attempt_limit, false)
                })
                .collect::<Vec<_>>();
            fixed_edges.truncate(discovery_attempt_limit);
            let mut attempts = 0usize;
            'angle_pairs: for moving_angle in &moving_edges {
                for fixed_angle in &fixed_edges {
                    if attempts >= discovery_attempt_limit || angle_keys.len() >= angle_key_limit {
                        break 'angle_pairs;
                    }
                    attempts += 1;
                    push_angle_candidate(
                        &mut angle_keys,
                        &mut seen_angle_keys,
                        *fixed_angle - *moving_angle,
                        angle_key_limit,
                    );
                }
            }
        }
        let seed_count = if scope == AngleScope::Orthogonal {
            4
        } else {
            settings.angle_seed_count
        };
        for index in 1..seed_count {
            push_angle_candidate(
                &mut angle_keys,
                &mut seen_angle_keys,
                360.0 * index as f64 / seed_count as f64,
                angle_key_limit,
            );
        }
    }
    angle_keys
}

fn push_angle_candidate(
    angle_keys: &mut Vec<i64>,
    seen: &mut BTreeSet<i64>,
    angle_deg: f64,
    limit: usize,
) {
    if angle_keys.len() >= limit {
        return;
    }
    let key = angle_key(angle_deg);
    if seen.insert(key) {
        angle_keys.push(key);
    }
}

fn beam_angle_scopes(
    prepared: &[PreparedGeneralPiece<'_>],
    settings: GeneralFastSettings,
) -> Vec<AngleScope> {
    let has_expanded_orientations = prepared.iter().any(|piece| {
        angle_candidates(piece, &[], settings, AngleScope::Full)
            != angle_candidates(piece, &[], settings, AngleScope::Orthogonal)
    });
    if has_expanded_orientations {
        vec![AngleScope::Full]
    } else {
        vec![AngleScope::Orthogonal]
    }
}

struct TranslationProposalInput<'a> {
    oriented: &'a PolygonSet,
    placed: &'a [PlacedState],
    settings: GeneralFastSettings,
    max_proposals: usize,
    max_attempts: usize,
    fixed_piece_order_strategy: FixedPieceOrder,
    contact_coverage: ContactCoverage,
}

fn translation_proposals(
    rotation_deg: f64,
    mirrored: bool,
    input: TranslationProposalInput<'_>,
) -> Result<(Vec<CandidateProposal>, usize), GeneralFastError> {
    let _span = profiling::span(Phase::ConstructorProposals);
    let TranslationProposalInput {
        oriented,
        placed,
        settings,
        max_proposals,
        max_attempts,
        fixed_piece_order_strategy,
        contact_coverage,
    } = input;
    let mut proposals = Vec::with_capacity(max_proposals.min(512));
    let mut seen_translations = BTreeSet::<(i64, i64)>::new();
    let mut attempts = 0usize;
    let angle_key = angle_key(rotation_deg);
    let bounds = oriented.bounds().ok_or_else(|| {
        GeneralFastError::Geometry(GeneralPolygonError::from_message(
            "cannot place empty geometry",
        ))
    })?;
    let sheet_inset = collision_sheet_inset_mm(settings);
    let current_depth = combined_bounds(placed)
        .map(|placed_bounds| placed_bounds.max_y)
        .unwrap_or(sheet_inset);
    let mut sheet_supports = vec![
        (sheet_inset - bounds.min_x, sheet_inset - bounds.min_y),
        (
            settings.sheet_short_axis_mm - sheet_inset - bounds.max_x,
            sheet_inset - bounds.min_y,
        ),
    ];
    if contact_coverage == ContactCoverage::Fair {
        sheet_supports.extend([
            (sheet_inset - bounds.min_x, current_depth - bounds.min_y),
            (
                settings.sheet_short_axis_mm - sheet_inset - bounds.max_x,
                current_depth - bounds.min_y,
            ),
        ]);
    }
    for (ordinal, (x, y)) in sheet_supports.into_iter().enumerate() {
        if !push_translation_proposal(
            &mut proposals,
            &mut seen_translations,
            &mut attempts,
            oriented,
            placed,
            settings,
            max_proposals,
            max_attempts,
            angle_key,
            mirrored,
            CandidateKind::SheetSupport,
            0,
            ordinal,
            0,
            x,
            y,
        )? {
            return Ok((proposals, attempts));
        }
    }

    let moving_points = contour_points(oriented);
    let moving_edges = contour_edges(oriented);
    let fixed_piece_order = fixed_piece_order(placed, fixed_piece_order_strategy);
    if contact_coverage == ContactCoverage::FrontierGreedy {
        for (fixed_piece_id_rank, fixed_index) in fixed_piece_order.iter().copied().enumerate() {
            let fixed_points = contour_points(&placed[fixed_index].collision);
            for (fixed_ordinal, fixed_point) in fixed_points.iter().enumerate() {
                for (moving_ordinal, moving_point) in moving_points.iter().enumerate() {
                    if !push_translation_proposal(
                        &mut proposals,
                        &mut seen_translations,
                        &mut attempts,
                        oriented,
                        placed,
                        settings,
                        max_proposals,
                        max_attempts,
                        angle_key,
                        mirrored,
                        CandidateKind::VertexVertex,
                        fixed_piece_id_rank,
                        fixed_ordinal,
                        moving_ordinal,
                        fixed_point.x - moving_point.x,
                        fixed_point.y - moving_point.y,
                    )? {
                        return Ok((proposals, attempts));
                    }
                }
            }
        }
        for (fixed_piece_id_rank, fixed_index) in fixed_piece_order.iter().copied().enumerate() {
            let fixed_edges = contour_edges(&placed[fixed_index].collision);
            for (fixed_ordinal, (fixed_start, fixed_end)) in fixed_edges.iter().enumerate() {
                for (moving_ordinal, moving_point) in moving_points.iter().enumerate() {
                    let target = closest_point(*moving_point, *fixed_start, *fixed_end);
                    if !push_translation_proposal(
                        &mut proposals,
                        &mut seen_translations,
                        &mut attempts,
                        oriented,
                        placed,
                        settings,
                        max_proposals,
                        max_attempts,
                        angle_key,
                        mirrored,
                        CandidateKind::MovingVertexFixedEdge,
                        fixed_piece_id_rank,
                        fixed_ordinal,
                        moving_ordinal,
                        target.x - moving_point.x,
                        target.y - moving_point.y,
                    )? {
                        return Ok((proposals, attempts));
                    }
                }
            }
        }
        for (fixed_piece_id_rank, fixed_index) in fixed_piece_order.iter().copied().enumerate() {
            let fixed_points = contour_points(&placed[fixed_index].collision);
            for (fixed_ordinal, fixed_point) in fixed_points.iter().enumerate() {
                for (moving_ordinal, (moving_start, moving_end)) in moving_edges.iter().enumerate()
                {
                    let projected = closest_point(*fixed_point, *moving_start, *moving_end);
                    if !push_translation_proposal(
                        &mut proposals,
                        &mut seen_translations,
                        &mut attempts,
                        oriented,
                        placed,
                        settings,
                        max_proposals,
                        max_attempts,
                        angle_key,
                        mirrored,
                        CandidateKind::FixedVertexMovingEdge,
                        fixed_piece_id_rank,
                        fixed_ordinal,
                        moving_ordinal,
                        fixed_point.x - projected.x,
                        fixed_point.y - projected.y,
                    )? {
                        return Ok((proposals, attempts));
                    }
                }
            }
        }
        return Ok((proposals, attempts));
    }
    for (fixed_piece_id_rank, fixed_index) in fixed_piece_order.iter().copied().enumerate() {
        if proposals.len() >= max_proposals || attempts >= max_attempts {
            break;
        }
        let fixed_pieces_remaining = fixed_piece_order.len() - fixed_piece_id_rank;
        let fixed_proposal_limit = match contact_coverage {
            ContactCoverage::Fair => {
                proposals.len() + (max_proposals - proposals.len()).div_ceil(fixed_pieces_remaining)
            }
            ContactCoverage::FrontierGreedy => max_proposals,
        };
        let fixed_attempt_limit = match contact_coverage {
            ContactCoverage::Fair => {
                attempts + (max_attempts - attempts).div_ceil(fixed_pieces_remaining)
            }
            ContactCoverage::FrontierGreedy => max_attempts,
        };
        let fixed = &placed[fixed_index];
        let fixed_points = contour_points(&fixed.collision);
        let fixed_edges = contour_edges(&fixed.collision);

        let vertex_proposal_limit = match contact_coverage {
            ContactCoverage::Fair => {
                proposals.len() + (fixed_proposal_limit - proposals.len()).div_ceil(3)
            }
            ContactCoverage::FrontierGreedy => fixed_proposal_limit,
        };
        let vertex_attempt_limit = match contact_coverage {
            ContactCoverage::Fair => attempts + (fixed_attempt_limit - attempts).div_ceil(3),
            ContactCoverage::FrontierGreedy => fixed_attempt_limit,
        };
        'vertex_contacts: for (fixed_ordinal, fixed_point) in fixed_points.iter().enumerate() {
            for (moving_ordinal, moving_point) in moving_points.iter().enumerate() {
                if !push_translation_proposal(
                    &mut proposals,
                    &mut seen_translations,
                    &mut attempts,
                    oriented,
                    placed,
                    settings,
                    vertex_proposal_limit,
                    vertex_attempt_limit,
                    angle_key,
                    mirrored,
                    CandidateKind::VertexVertex,
                    fixed_piece_id_rank,
                    fixed_ordinal,
                    moving_ordinal,
                    fixed_point.x - moving_point.x,
                    fixed_point.y - moving_point.y,
                )? {
                    break 'vertex_contacts;
                }
            }
        }

        let moving_vertex_proposal_limit = match contact_coverage {
            ContactCoverage::Fair => {
                proposals.len() + (fixed_proposal_limit - proposals.len()).div_ceil(2)
            }
            ContactCoverage::FrontierGreedy => fixed_proposal_limit,
        };
        let moving_vertex_attempt_limit = match contact_coverage {
            ContactCoverage::Fair => attempts + (fixed_attempt_limit - attempts).div_ceil(2),
            ContactCoverage::FrontierGreedy => fixed_attempt_limit,
        };
        'moving_vertex_contacts: for (fixed_ordinal, (fixed_start, fixed_end)) in
            fixed_edges.iter().enumerate()
        {
            for (moving_ordinal, moving_point) in moving_points.iter().enumerate() {
                let target = closest_point(*moving_point, *fixed_start, *fixed_end);
                if !push_translation_proposal(
                    &mut proposals,
                    &mut seen_translations,
                    &mut attempts,
                    oriented,
                    placed,
                    settings,
                    moving_vertex_proposal_limit,
                    moving_vertex_attempt_limit,
                    angle_key,
                    mirrored,
                    CandidateKind::MovingVertexFixedEdge,
                    fixed_piece_id_rank,
                    fixed_ordinal,
                    moving_ordinal,
                    target.x - moving_point.x,
                    target.y - moving_point.y,
                )? {
                    break 'moving_vertex_contacts;
                }
            }
        }

        'fixed_vertex_contacts: for (fixed_ordinal, fixed_point) in fixed_points.iter().enumerate()
        {
            for (moving_ordinal, (moving_start, moving_end)) in moving_edges.iter().enumerate() {
                let projected = closest_point(*fixed_point, *moving_start, *moving_end);
                if !push_translation_proposal(
                    &mut proposals,
                    &mut seen_translations,
                    &mut attempts,
                    oriented,
                    placed,
                    settings,
                    fixed_proposal_limit,
                    fixed_attempt_limit,
                    angle_key,
                    mirrored,
                    CandidateKind::FixedVertexMovingEdge,
                    fixed_piece_id_rank,
                    fixed_ordinal,
                    moving_ordinal,
                    fixed_point.x - projected.x,
                    fixed_point.y - projected.y,
                )? {
                    break 'fixed_vertex_contacts;
                }
            }
        }
    }

    Ok((proposals, attempts))
}

#[allow(clippy::too_many_arguments)]
fn push_translation_proposal(
    proposals: &mut Vec<CandidateProposal>,
    seen_translations: &mut BTreeSet<(i64, i64)>,
    attempts: &mut usize,
    oriented: &PolygonSet,
    placed: &[PlacedState],
    settings: GeneralFastSettings,
    max_proposals: usize,
    max_attempts: usize,
    angle_key: i64,
    mirrored: bool,
    kind: CandidateKind,
    fixed_piece_id_rank: usize,
    fixed_feature_ordinal: usize,
    moving_feature_ordinal: usize,
    translate_x: f64,
    translate_y: f64,
) -> Result<bool, GeneralFastError> {
    if proposals.len() >= max_proposals || *attempts >= max_attempts {
        return Ok(false);
    }
    *attempts += 1;
    let (Some(translate_x_grid), Some(translate_y_grid)) =
        (grid_key(translate_x), grid_key(translate_y))
    else {
        return Ok(true);
    };
    if !seen_translations.insert((translate_x_grid, translate_y_grid)) {
        return Ok(true);
    }
    let translate_x = from_grid(translate_x_grid as f64);
    let translate_y = from_grid(translate_y_grid as f64);
    let key = CandidateKey {
        angle_key,
        mirrored,
        kind,
        fixed_piece_id_rank,
        fixed_feature_ordinal,
        moving_feature_ordinal,
        translate_x_grid,
        translate_y_grid,
    };
    let bounds = oriented.bounds().ok_or_else(|| {
        GeneralFastError::Geometry(GeneralPolygonError::from_message(
            "cannot score empty geometry",
        ))
    })?;
    let Some((min_x_grid, min_y_grid, max_x_grid, max_y_grid)) = grid_bounds(&bounds) else {
        return Ok(true);
    };
    let Some(translated_min_x_grid) = min_x_grid.checked_add(translate_x_grid) else {
        return Ok(true);
    };
    let Some(translated_min_y_grid) = min_y_grid.checked_add(translate_y_grid) else {
        return Ok(true);
    };
    let Some(translated_max_x_grid) = max_x_grid.checked_add(translate_x_grid) else {
        return Ok(true);
    };
    let Some(translated_max_y_grid) = max_y_grid.checked_add(translate_y_grid) else {
        return Ok(true);
    };
    let translated_bounds = crate::domain::IrregularBounds::new(
        bounds.min_x + translate_x,
        bounds.min_y + translate_y,
        bounds.max_x + translate_x,
        bounds.max_y + translate_y,
    );
    let sheet_inset = collision_sheet_inset_mm(settings);
    let (Some(sheet_min_grid), Some(sheet_max_x_grid), Some(sheet_max_y_grid)) = (
        grid_key(sheet_inset),
        grid_key(settings.sheet_short_axis_mm - sheet_inset),
        grid_key(settings.sheet_long_axis_mm - sheet_inset),
    ) else {
        return Ok(true);
    };
    if translated_min_x_grid < sheet_min_grid
        || translated_min_y_grid < sheet_min_grid
        || translated_max_x_grid > sheet_max_x_grid
        || translated_max_y_grid > sheet_max_y_grid
    {
        return Ok(true);
    }
    proposals.push(CandidateProposal {
        key,
        rotation_deg: angle_from_key(angle_key),
        mirrored,
        translate_x,
        translate_y,
        score: score_bounds(placed, translated_bounds, settings),
        broad_phase_overlap_count: placed
            .iter()
            .filter_map(|state| state.collision.bounds())
            .filter(|fixed_bounds| bounds_have_positive_overlap(translated_bounds, *fixed_bounds))
            .count(),
    });
    Ok(proposals.len() < max_proposals)
}

fn shortlist_proposals(
    proposals: Vec<CandidateProposal>,
    max_evaluations: usize,
) -> Vec<CandidateProposal> {
    let baseline_target = max_evaluations.saturating_mul(PRIMARY_ORIENTATION_EVALUATION_NUMERATOR)
        / PRIMARY_ORIENTATION_EVALUATION_DENOMINATOR;
    let mut by_orientation = BTreeMap::<(i64, bool), Vec<CandidateProposal>>::new();
    for proposal in proposals {
        by_orientation
            .entry((proposal.key.angle_key, proposal.mirrored))
            .or_default()
            .push(proposal);
    }
    let mut by_orientation = by_orientation
        .into_iter()
        .map(|(orientation, mut proposals)| {
            proposals.sort_by_key(|proposal| proposal.key);
            (orientation, VecDeque::from(proposals))
        })
        .collect::<BTreeMap<_, _>>();
    let mut selected = Vec::with_capacity(max_evaluations);
    while selected.len() < baseline_target {
        let mut progressed = false;
        for proposals in by_orientation.values_mut() {
            if let Some(proposal) = proposals.pop_front() {
                selected.push(proposal);
                progressed = true;
                if selected.len() == baseline_target {
                    break;
                }
            }
        }
        if !progressed {
            break;
        }
    }
    let mut selected_keys = selected
        .iter()
        .map(|proposal| proposal.key)
        .collect::<BTreeSet<_>>();
    let mut proposals = by_orientation
        .into_values()
        .flat_map(VecDeque::into_iter)
        .collect::<Vec<_>>();
    proposals.sort_by(|first, second| {
        compare_candidate_scores(first.score, first.key, second.score, second.key)
    });
    let quality_budget = max_evaluations.saturating_sub(selected.len());
    let clear_target = quality_budget.saturating_mul(3) / 4;
    let overlap_target = quality_budget.saturating_sub(clear_target);
    let mut clear = Vec::with_capacity(clear_target);
    let mut overlap = Vec::with_capacity(overlap_target);
    let mut clear_overflow = Vec::new();
    let mut overlap_overflow = Vec::new();
    for proposal in proposals {
        if proposal.broad_phase_overlap_count == 0 {
            if clear.len() < clear_target {
                clear.push(proposal);
            } else {
                clear_overflow.push(proposal);
            }
        } else if overlap.len() < overlap_target {
            overlap.push(proposal);
        } else {
            overlap_overflow.push(proposal);
        }
    }
    if clear.len() < clear_target {
        overlap.extend(
            overlap_overflow
                .into_iter()
                .take(clear_target - clear.len()),
        );
    } else if overlap.len() < overlap_target {
        clear.extend(
            clear_overflow
                .into_iter()
                .take(overlap_target - overlap.len()),
        );
    }
    clear.extend(overlap);
    for proposal in clear {
        if selected_keys.insert(proposal.key) {
            selected.push(proposal);
        }
    }
    selected.sort_by(|first, second| {
        compare_candidate_scores(first.score, first.key, second.score, second.key)
    });
    selected.truncate(max_evaluations);
    selected
}

fn bounds_have_positive_overlap(
    first: crate::domain::IrregularBounds,
    second: crate::domain::IrregularBounds,
) -> bool {
    first.min_x < second.max_x
        && first.max_x > second.min_x
        && first.min_y < second.max_y
        && first.max_y > second.min_y
}

fn score_candidate(
    placed: &[PlacedState],
    candidate: &Candidate,
    settings: GeneralFastSettings,
) -> CandidateScore {
    let _span = profiling::span(Phase::ConstructorScore);
    score_bounds(
        placed,
        candidate
            .collision
            .bounds()
            .expect("a candidate has non-empty geometry"),
        settings,
    )
}

fn score_bounds(
    placed: &[PlacedState],
    candidate_bounds: crate::domain::IrregularBounds,
    settings: GeneralFastSettings,
) -> CandidateScore {
    let mut bounds = placed
        .iter()
        .filter_map(|state| state.collision.bounds())
        .chain(std::iter::once(candidate_bounds));
    let first = bounds.next().expect("a candidate has non-empty geometry");
    let (min_x, min_y, max_x, max_y) = bounds.fold(
        (first.min_x, first.min_y, first.max_x, first.max_y),
        |(min_x, min_y, max_x, max_y), bounds| {
            (
                min_x.min(bounds.min_x),
                min_y.min(bounds.min_y),
                max_x.max(bounds.max_x),
                max_y.max(bounds.max_y),
            )
        },
    );
    let mut intervals = placed
        .iter()
        .filter_map(|state| {
            state
                .collision
                .bounds()
                .map(|bounds| (bounds.min_x, bounds.max_x))
        })
        .chain(std::iter::once((
            candidate_bounds.min_x,
            candidate_bounds.max_x,
        )))
        .collect::<Vec<_>>();
    intervals.sort_by(|first, second| first.0.total_cmp(&second.0));
    let covered = merged_interval_length(&intervals);
    CandidateScore {
        candidate_long_axis_position: 3.0 * candidate_bounds.min_y + candidate_bounds.max_y,
        candidate_short_axis_position: 3.0 * candidate_bounds.min_x + candidate_bounds.max_x,
        long_axis_depth: max_y + collision_sheet_inset_mm(settings),
        unused_short_axis_projection: (collision_sheet_short_axis_mm(settings) - covered).max(0.0),
        envelope_area: (max_x - min_x) * (max_y - min_y),
    }
}

fn compare_candidate_scores(
    first: CandidateScore,
    first_key: CandidateKey,
    second: CandidateScore,
    second_key: CandidateKey,
) -> Ordering {
    first
        .candidate_long_axis_position
        .total_cmp(&second.candidate_long_axis_position)
        .then_with(|| {
            first
                .candidate_short_axis_position
                .total_cmp(&second.candidate_short_axis_position)
        })
        .then_with(|| first.long_axis_depth.total_cmp(&second.long_axis_depth))
        .then_with(|| {
            first
                .unused_short_axis_projection
                .total_cmp(&second.unused_short_axis_projection)
        })
        .then_with(|| first.envelope_area.total_cmp(&second.envelope_area))
        .then_with(|| first_key.cmp(&second_key))
}

fn validate_result(
    pieces: &[GeneralFastPiece<'_>],
    placed: &[PlacedState],
    settings: GeneralFastSettings,
) -> Result<(), GeneralFastError> {
    let placements = placed
        .iter()
        .map(|state| state.placement.clone())
        .collect::<Vec<_>>();
    validate_and_measure_placements(pieces, &placements, settings)?;
    Ok(())
}

/// The *contract* half of [`validate_and_measure_placements`]: the raw-source
/// exact validator, and nothing else.
///
/// This answers one question - "is this layout legal?" - against the requested
/// clearance contract, on the untouched `f64` source rings. No search envelope
/// appears anywhere in it: no offset, no canonical grid, no
/// `search_offset_allowance_mm`.
///
/// [`validate_and_measure_placements`] is a *composite* of this and a second,
/// stricter question - "may the search visit this layout?" - which it asks
/// first, on collision polygons expanded by [`collision_expansion_mm`]. Because
/// that expansion includes the search allowance, the composite can reject a
/// layout that this function accepts: a pinned fixture found under a narrow
/// allowance is contract-valid but sits outside the wider envelope a later run
/// searches in. Keeping the two separable is what lets a report say which of
/// the two verdicts it means. Acceptance still uses the composite.
pub(crate) fn validate_placements_against_contract(
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
    settings: GeneralFastSettings,
) -> Result<(), GeneralFastError> {
    let pieces_by_id = pieces
        .iter()
        .map(|piece| (piece.id, piece))
        .collect::<BTreeMap<_, _>>();
    let independent = placements
        .iter()
        .map(|placement| {
            let piece = pieces_by_id
                .get(placement.piece_id.as_str())
                .copied()
                .ok_or_else(|| {
                    GeneralFastError::InvalidInput(format!(
                        "a result placement references unknown piece {}",
                        placement.piece_id
                    ))
                })?;
            Ok(GeneralPlacement {
                piece_id: piece.id,
                polygon: piece.polygon,
                rotation_deg: placement.rotation_deg,
                mirrored: placement.mirrored,
                translate_x: placement.translate_short_axis,
                translate_y: placement.translate_long_axis,
            })
        })
        .collect::<Result<Vec<_>, GeneralFastError>>()?;
    validate_publication(
        &independent,
        PublicationValidationSettings {
            sheet_width_mm: settings.sheet_short_axis_mm,
            sheet_height_mm: settings.sheet_long_axis_mm,
            total_padding_mm: settings.total_padding_mm,
            sheet_edge_clearance_mm: settings.sheet_edge_clearance_mm,
            flattening_sag_tolerance_mm: settings.flattening_sag_tolerance_mm,
        },
    )?;
    Ok(())
}

/// The composite acceptance check: search-envelope admissibility *and* contract
/// validity, in that order.
///
/// The envelope half rebuilds every placement's canonical collision polygon -
/// the source offset by [`collision_expansion_mm`], which folds in half the pair
/// clearance, the safety margin, *and* `search_offset_allowance_mm` - and
/// requires each to fit the sheet and to be pairwise disjoint on the canonical
/// grid. The contract half is [`validate_placements_against_contract`].
///
/// A caller that wants only the legality verdict must call that function
/// directly; a failure here does not distinguish the two.
pub(crate) fn validate_and_measure_placements(
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
    settings: GeneralFastSettings,
) -> Result<GeneralPlacementMetrics, GeneralFastError> {
    let _span = profiling::span(Phase::PublicationValidate);
    profiling::count(Counter::PublicationAttempts, 1);
    let pieces_by_id = pieces
        .iter()
        .enumerate()
        .map(|(index, piece)| (piece.id, (index, piece)))
        .collect::<BTreeMap<_, _>>();
    let mut placed_piece_ids = BTreeSet::new();
    for placement in placements {
        if !placed_piece_ids.insert(placement.piece_id.as_str()) {
            return Err(GeneralFastError::InvalidInput(format!(
                "a result contains duplicate placement for piece {}",
                placement.piece_id
            )));
        }
    }
    let expansion = collision_expansion_mm(settings);
    let rebuilt = placements
        .iter()
        .map(|placement| {
            let (input_index, piece) = pieces_by_id
                .get(placement.piece_id.as_str())
                .copied()
                .ok_or_else(|| {
                    GeneralFastError::InvalidInput(format!(
                        "a result placement references unknown piece {}",
                        placement.piece_id
                    ))
                })?;
            if !placement.rotation_deg.is_finite() {
                return Err(GeneralFastError::InvalidInput(format!(
                    "piece {} has a non-finite rotation",
                    placement.piece_id
                )));
            }
            if !piece.allow_rotation && angle_key(placement.rotation_deg) != 0 {
                return Err(GeneralFastError::InvalidInput(format!(
                    "piece {} uses a forbidden rotation",
                    placement.piece_id
                )));
            }
            if placement.mirrored && !piece.allow_mirror {
                return Err(GeneralFastError::InvalidInput(format!(
                    "piece {} uses a forbidden mirror transform",
                    placement.piece_id
                )));
            }
            let collision = piece
                .polygon
                .transformed(
                    placement.rotation_deg,
                    placement.mirrored,
                    placement.translate_short_axis,
                    placement.translate_long_axis,
                )?
                .offset(expansion)?;
            if !collision_fits_sheet(&collision, settings) {
                return Err(GeneralFastError::InvalidInput(format!(
                    "piece {} violates the canonical-grid sheet boundary",
                    placement.piece_id
                )));
            }
            Ok(PlacedState {
                input_index,
                placement: placement.clone(),
                collision,
            })
        })
        .collect::<Result<Vec<_>, GeneralFastError>>()?;

    for first_index in 0..rebuilt.len() {
        for second_index in (first_index + 1)..rebuilt.len() {
            if polygons_overlap_exact(
                &rebuilt[first_index].collision,
                &rebuilt[second_index].collision,
            )? {
                return Err(GeneralFastError::InvalidInput(format!(
                    "pieces {} and {} overlap on the canonical collision grid",
                    rebuilt[first_index].placement.piece_id,
                    rebuilt[second_index].placement.piece_id
                )));
            }
        }
    }

    validate_placements_against_contract(pieces, placements, settings)?;
    let metrics = layout_metrics(&rebuilt, settings);
    Ok(GeneralPlacementMetrics {
        used_short_axis_span_mm: metrics.used_short_axis_span_mm,
        used_long_axis_depth_mm: metrics.used_long_axis_depth_mm,
        unused_short_axis_projection_mm: metrics.unused_short_axis_projection_mm,
        occupied_envelope_area_mm2: metrics.occupied_envelope_area_mm2,
    })
}

fn contour_points(polygon: &PolygonSet) -> Vec<IrregularPoint> {
    polygon
        .regions
        .iter()
        .flat_map(|region| {
            std::iter::once(&region.outer)
                .chain(region.holes.iter())
                .flat_map(PolygonRing::points)
        })
        .copied()
        .collect()
}

fn fixed_piece_order(placed: &[PlacedState], strategy: FixedPieceOrder) -> Vec<usize> {
    let mut order = (0..placed.len()).collect::<Vec<_>>();
    order.sort_by(|first, second| match strategy {
        FixedPieceOrder::Id => placed[*first]
            .placement
            .piece_id
            .cmp(&placed[*second].placement.piece_id),
        FixedPieceOrder::ShortSideFrontier => {
            let first_bounds = placed[*first]
                .collision
                .bounds()
                .expect("placed geometry is non-empty");
            let second_bounds = placed[*second]
                .collision
                .bounds()
                .expect("placed geometry is non-empty");
            second_bounds
                .max_y
                .total_cmp(&first_bounds.max_y)
                .then_with(|| first_bounds.min_x.total_cmp(&second_bounds.min_x))
                .then_with(|| first_bounds.max_x.total_cmp(&second_bounds.max_x))
                .then_with(|| {
                    placed[*first]
                        .placement
                        .piece_id
                        .cmp(&placed[*second].placement.piece_id)
                })
        }
    });
    order
}

fn contour_edges(polygon: &PolygonSet) -> Vec<(IrregularPoint, IrregularPoint)> {
    polygon
        .regions
        .iter()
        .flat_map(|region| std::iter::once(&region.outer).chain(region.holes.iter()))
        .flat_map(|ring| {
            (0..ring.points().len()).map(|index| {
                (
                    ring.points()[index],
                    ring.points()[(index + 1) % ring.points().len()],
                )
            })
        })
        .collect()
}

fn prioritized_edge_angles(polygon: &PolygonSet, limit: usize, mirrored: bool) -> Vec<f64> {
    let half_turn_key = angle_key(180.0);
    let mut edges = contour_edges(polygon)
        .into_iter()
        .filter_map(|(start, end)| {
            let dx = if mirrored {
                start.x - end.x
            } else {
                end.x - start.x
            };
            let dy = end.y - start.y;
            let length_squared = dx * dx + dy * dy;
            (length_squared > 0.0).then(|| (dy.atan2(dx).to_degrees(), length_squared))
        })
        .collect::<Vec<_>>();
    edges.sort_by(|first, second| {
        second
            .1
            .total_cmp(&first.1)
            .then_with(|| angle_key(first.0).cmp(&angle_key(second.0)))
    });
    let mut seen = BTreeSet::new();
    edges
        .into_iter()
        .filter_map(|(angle, _)| {
            let direction_key = angle_key(angle).rem_euclid(half_turn_key);
            seen.insert(direction_key).then_some(angle)
        })
        .take(limit)
        .collect()
}

fn polygon_diameter(polygon: &PolygonSet) -> f64 {
    let points = contour_points(polygon);
    let mut diameter = 0.0_f64;
    for first_index in 0..points.len() {
        for second in &points[(first_index + 1)..] {
            diameter = diameter
                .max((points[first_index].x - second.x).hypot(points[first_index].y - second.y));
        }
    }
    diameter
}

fn reflex_vertex_count(polygon: &PolygonSet) -> usize {
    polygon
        .regions
        .iter()
        .map(|region| {
            reflex_vertices(&region.outer) + region.holes.iter().map(reflex_vertices).sum::<usize>()
        })
        .sum()
}

fn reflex_vertices(ring: &PolygonRing) -> usize {
    let points = ring.points();
    (0..points.len())
        .filter(|index| {
            let previous = points[(index + points.len() - 1) % points.len()];
            let current = points[*index];
            let next = points[(*index + 1) % points.len()];
            crate::geometry::predicates::orientation(
                previous.x, previous.y, current.x, current.y, next.x, next.y,
            ) < 0
        })
        .count()
}

fn closest_point(
    point: IrregularPoint,
    start: IrregularPoint,
    end: IrregularPoint,
) -> IrregularPoint {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx * dx + dy * dy;
    if length_squared == 0.0 {
        return start;
    }
    let parameter =
        (((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared).clamp(0.0, 1.0);
    IrregularPoint::new(start.x + parameter * dx, start.y + parameter * dy)
}

fn merged_interval_length(intervals: &[(f64, f64)]) -> f64 {
    let Some(&(first_start, first_end)) = intervals.first() else {
        return 0.0;
    };
    let mut total = 0.0;
    let mut current_start = first_start;
    let mut current_end = first_end;
    for &(start, end) in &intervals[1..] {
        if start <= current_end {
            current_end = current_end.max(end);
        } else {
            total += current_end - current_start;
            current_start = start;
            current_end = end;
        }
    }
    total + current_end - current_start
}

fn angle_key(angle_deg: f64) -> i64 {
    let normalized = angle_deg.rem_euclid(360.0);
    (normalized * ANGLE_KEY_SCALE).round() as i64
}

fn angle_from_key(key: i64) -> f64 {
    key as f64 / ANGLE_KEY_SCALE
}

fn is_quarter_turn(angle_deg: f64) -> bool {
    angle_key(angle_deg).rem_euclid(angle_key(90.0)) == 0
}

fn grid_key(value_mm: f64) -> Option<i64> {
    to_grid_mm(value_mm).map(|value| value as i64)
}

fn grid_bounds(bounds: &crate::domain::IrregularBounds) -> Option<(i64, i64, i64, i64)> {
    Some((
        grid_key(bounds.min_x)?,
        grid_key(bounds.min_y)?,
        grid_key(bounds.max_x)?,
        grid_key(bounds.max_y)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::convex::compute_convex_hull;
    use crate::parallel::JobPool;

    fn point(x: f64, y: f64) -> IrregularPoint {
        IrregularPoint::new(x, y)
    }

    fn square(side: f64) -> PolygonSet {
        PolygonSet::from_outer(vec![
            point(0.0, 0.0),
            point(side, 0.0),
            point(side, side),
            point(0.0, side),
        ])
        .unwrap()
    }

    fn rectangle(width: f64, height: f64) -> PolygonSet {
        PolygonSet::from_outer(vec![
            point(0.0, 0.0),
            point(width, 0.0),
            point(width, height),
            point(0.0, height),
        ])
        .unwrap()
    }

    /// The search-envelope half of the composite check is strictly stronger
    /// than the contract half, and by exactly the search allowance: a layout
    /// parked against the sheet's clearance boundary is contract-valid at every
    /// allowance, but only admissible to a search whose envelope still fits.
    ///
    /// This is the split a `contractValid` report exists to make readable: a
    /// composite rejection here says "this run may not visit that layout", not
    /// "that layout is illegal".
    #[test]
    fn contract_validity_is_independent_from_search_envelope_admissibility() {
        let piece = square(1.0);
        let pieces = [GeneralFastPiece {
            id: "boundary",
            polygon: &piece,
            allow_rotation: false,
            allow_mirror: false,
        }];
        // Edge clearance defaults to half the pair clearance, so the contract
        // admits an outer point sitting exactly on 1.0 mm.
        let placements = [GeneralFastPlacement {
            piece_id: "boundary".to_owned(),
            rotation_deg: 0.0,
            mirrored: false,
            translate_short_axis: 1.0,
            translate_long_axis: 1.0,
        }];
        let mut settings = GeneralFastSettings::deterministic_test(20.0, 20.0);
        settings.total_padding_mm = 2.0;

        settings.search_offset_allowance_mm = 0.0;
        assert!(validate_placements_against_contract(&pieces, &placements, settings).is_ok());
        assert!(validate_and_measure_placements(&pieces, &placements, settings).is_ok());

        // The same layout, replayed under a wider envelope. The contract has
        // not moved; only the envelope has.
        settings.search_offset_allowance_mm = 0.5;
        assert!(
            validate_placements_against_contract(&pieces, &placements, settings).is_ok(),
            "the clearance contract does not depend on the search allowance"
        );
        let composite = validate_and_measure_placements(&pieces, &placements, settings)
            .expect_err("the widened envelope must leave the sheet");
        assert_eq!(
            composite.to_string(),
            "piece boundary violates the canonical-grid sheet boundary"
        );
    }

    /// The split must not have loosened the composite: a genuine contract
    /// violation still fails it, and fails the contract validator alone.
    #[test]
    fn contract_validator_still_refuses_a_real_clearance_violation() {
        let piece = square(2.0);
        let pieces = [
            GeneralFastPiece {
                id: "a",
                polygon: &piece,
                allow_rotation: false,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "b",
                polygon: &piece,
                allow_rotation: false,
                allow_mirror: false,
            },
        ];
        let placements = [
            GeneralFastPlacement {
                piece_id: "a".to_owned(),
                rotation_deg: 0.0,
                mirrored: false,
                translate_short_axis: 5.0,
                translate_long_axis: 5.0,
            },
            GeneralFastPlacement {
                piece_id: "b".to_owned(),
                rotation_deg: 0.0,
                mirrored: false,
                translate_short_axis: 6.0,
                translate_long_axis: 5.0,
            },
        ];
        let settings = GeneralFastSettings::deterministic_test(20.0, 20.0);
        assert!(validate_placements_against_contract(&pieces, &placements, settings).is_err());
        assert!(validate_and_measure_placements(&pieces, &placements, settings).is_err());
    }

    #[test]
    fn contract_validator_rejects_an_unknown_piece_reference() {
        let piece = square(1.0);
        let pieces = [GeneralFastPiece {
            id: "known",
            polygon: &piece,
            allow_rotation: false,
            allow_mirror: false,
        }];
        let placements = [GeneralFastPlacement {
            piece_id: "stranger".to_owned(),
            rotation_deg: 0.0,
            mirrored: false,
            translate_short_axis: 5.0,
            translate_long_axis: 5.0,
        }];
        let settings = GeneralFastSettings::deterministic_test(20.0, 20.0);
        assert_eq!(
            validate_placements_against_contract(&pieces, &placements, settings)
                .unwrap_err()
                .to_string(),
            "a result placement references unknown piece stranger"
        );
    }

    fn regular_polygon(vertex_count: usize, radius: f64) -> Vec<IrregularPoint> {
        (0..vertex_count)
            .map(|index| {
                let angle = std::f64::consts::TAU * index as f64 / vertex_count as f64;
                point(radius * angle.cos(), radius * angle.sin())
            })
            .collect()
    }

    fn l_shape() -> PolygonSet {
        PolygonSet::from_outer(vec![
            point(0.0, 0.0),
            point(4.0, 0.0),
            point(4.0, 1.0),
            point(1.0, 1.0),
            point(1.0, 4.0),
            point(0.0, 4.0),
        ])
        .unwrap()
    }

    #[test]
    fn broad_phase_skips_over_limit_exact_queries_for_disjoint_polygons() {
        let complex = PolygonSet::new(vec![
            crate::geometry::general_polygon::PolygonRegion::new(
                regular_polygon(2_048, 100.0),
                Vec::new(),
            )
            .unwrap(),
            crate::geometry::general_polygon::PolygonRegion::new(
                vec![
                    point(200.0, 0.0),
                    point(201.0, 0.0),
                    point(201.0, 1.0),
                    point(200.0, 1.0),
                ],
                Vec::new(),
            )
            .unwrap(),
        ])
        .unwrap();
        let disjoint = complex.translated(1_000.0, 0.0).unwrap();

        assert!(complex.intersection_area_mm2(&disjoint).is_err());
        assert!(!polygons_overlap_exact(&complex, &disjoint).unwrap());
    }

    #[test]
    fn canonical_publication_rebuild_rejects_out_of_sheet_source_geometry() {
        let source = square(2.0);
        let pieces = [GeneralFastPiece {
            id: "source",
            polygon: &source,
            allow_rotation: true,
            allow_mirror: false,
        }];
        let placements = [GeneralFastPlacement {
            piece_id: "source".to_owned(),
            rotation_deg: 37.0,
            mirrored: false,
            translate_short_axis: -5.0,
            translate_long_axis: -5.0,
        }];

        let error = validate_and_measure_placements(
            &pieces,
            &placements,
            GeneralFastSettings::deterministic_test(20.0, 20.0),
        )
        .unwrap_err();
        assert!(error.to_string().contains("canonical-grid sheet boundary"));
    }

    #[test]
    fn canonical_publication_rebuild_rejects_forbidden_piece_transforms() {
        let source = square(2.0);
        let pieces = [GeneralFastPiece {
            id: "source",
            polygon: &source,
            allow_rotation: false,
            allow_mirror: false,
        }];
        let settings = GeneralFastSettings::deterministic_test(20.0, 20.0);
        let rotated = [GeneralFastPlacement {
            piece_id: "source".to_owned(),
            rotation_deg: 45.0,
            mirrored: false,
            translate_short_axis: 5.0,
            translate_long_axis: 5.0,
        }];
        let mirrored = [GeneralFastPlacement {
            piece_id: "source".to_owned(),
            rotation_deg: 0.0,
            mirrored: true,
            translate_short_axis: 5.0,
            translate_long_axis: 5.0,
        }];

        let rotation_error =
            validate_and_measure_placements(&pieces, &rotated, settings).unwrap_err();
        assert!(rotation_error.to_string().contains("forbidden rotation"));
        let mirror_error =
            validate_and_measure_placements(&pieces, &mirrored, settings).unwrap_err();
        assert!(mirror_error.to_string().contains("forbidden mirror"));
    }

    #[test]
    fn canonical_publication_rebuild_rejects_duplicate_piece_placements() {
        let source = square(2.0);
        let pieces = [GeneralFastPiece {
            id: "source",
            polygon: &source,
            allow_rotation: true,
            allow_mirror: true,
        }];
        let placements = [
            GeneralFastPlacement {
                piece_id: "source".to_owned(),
                rotation_deg: 0.0,
                mirrored: false,
                translate_short_axis: 1.0,
                translate_long_axis: 1.0,
            },
            GeneralFastPlacement {
                piece_id: "source".to_owned(),
                rotation_deg: 0.0,
                mirrored: false,
                translate_short_axis: 4.0,
                translate_long_axis: 1.0,
            },
        ];

        let error = validate_and_measure_placements(
            &pieces,
            &placements,
            GeneralFastSettings::deterministic_test(20.0, 20.0),
        )
        .unwrap_err();
        assert!(error.to_string().contains("duplicate placement"));
    }

    #[test]
    fn constructor_discovers_a_real_concavity_contact() {
        let l = l_shape();
        let pocket = square(2.5);
        let pieces = [
            GeneralFastPiece {
                id: "l",
                polygon: &l,
                allow_rotation: false,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "pocket",
                polygon: &pocket,
                allow_rotation: false,
                allow_mirror: false,
            },
        ];
        let result = construct_short_side_first(
            &pieces,
            GeneralFastSettings::deterministic_test(4.01, 4.01),
        )
        .unwrap();

        assert!(result.unplaced_piece_ids.is_empty());
        assert_eq!(result.placements.len(), 2);
        let nested = result
            .placements
            .iter()
            .find(|placement| placement.piece_id == "pocket")
            .unwrap();
        assert!(nested.translate_short_axis >= 1.0);
        assert!(nested.translate_long_axis >= 1.0);
        assert!(nested.translate_short_axis + 2.5 <= 4.01);
        assert!(nested.translate_long_axis + 2.5 <= 4.01);

        let hull = compute_convex_hull(l.regions[0].outer.points());
        let hull_set = PolygonSet::from_outer(hull.points).unwrap();
        let translated_pocket = pocket
            .translated(nested.translate_short_axis, nested.translate_long_axis)
            .unwrap();
        assert!(hull_set.intersection_area_mm2(&translated_pocket).unwrap() > 0.0);
    }

    #[test]
    fn replay_is_stable_for_the_same_work_quota() {
        let l = l_shape();
        let pocket = square(2.5);
        let pieces = [
            GeneralFastPiece {
                id: "l",
                polygon: &l,
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "pocket",
                polygon: &pocket,
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let settings = GeneralFastSettings::deterministic_test(4.01, 4.01);
        let first = construct_short_side_first(&pieces, settings).unwrap();
        let second = construct_short_side_first(&pieces, settings).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn partial_layout_beam_is_deterministic_bounded_and_never_worsens_primary() {
        let wide = rectangle(3.0, 1.0);
        let tall = rectangle(1.0, 2.0);
        let pieces = [
            GeneralFastPiece {
                id: "wide-a",
                polygon: &wide,
                allow_rotation: true,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "wide-b",
                polygon: &wide,
                allow_rotation: true,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "tall-a",
                polygon: &tall,
                allow_rotation: true,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "tall-b",
                polygon: &tall,
                allow_rotation: true,
                allow_mirror: false,
            },
        ];
        let mut primary_settings = GeneralFastSettings::deterministic_test(5.0, 10.0);
        primary_settings.max_evaluations_per_piece = 32;
        let primary = construct_short_side_first(&pieces, primary_settings).unwrap();

        let mut beam_settings = primary_settings;
        beam_settings.max_partial_layouts = 4;
        beam_settings.max_beam_evaluations_per_state = 16;
        let first = construct_short_side_first(&pieces, beam_settings).unwrap();
        let second = construct_short_side_first(&pieces, beam_settings).unwrap();

        assert_eq!(first, second);
        assert_ne!(first.beam_candidate_placed_count, None);
        let construction_bound = pieces.len() * 4 * 16;
        let completion_bound = 4 * pieces.len() * (pieces.len() + 1) / 2 * 16;
        assert!(first.beam_exact_evaluations <= construction_bound + completion_bound);
        assert_ne!(compare_result_quality(&first, &primary), Ordering::Greater);
        assert_eq!(
            first.exact_evaluations,
            first.primary_exact_evaluations + first.beam_exact_evaluations
        );
    }

    #[test]
    fn expanded_angle_beam_cannot_discard_the_orthogonal_incumbent() {
        let skewed =
            PolygonSet::from_outer(vec![point(0.0, 0.0), point(3.0, 0.0), point(0.5, 1.0)])
                .unwrap();
        let pieces = [
            GeneralFastPiece {
                id: "skewed-a",
                polygon: &skewed,
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "skewed-b",
                polygon: &skewed,
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let mut orthogonal_settings = GeneralFastSettings::deterministic_test(5.0, 10.0);
        orthogonal_settings.angle_seed_count = 4;
        orthogonal_settings.max_angles_per_piece = 8;
        orthogonal_settings.max_partial_layouts = 4;
        orthogonal_settings.max_beam_evaluations_per_state = 64;
        let orthogonal = construct_short_side_first(&pieces, orthogonal_settings).unwrap();
        let mut expanded_settings = orthogonal_settings;
        expanded_settings.max_angles_per_piece = 16;
        let expanded = construct_short_side_first(&pieces, expanded_settings).unwrap();

        assert_ne!(
            compare_result_quality(&expanded, &orthogonal),
            Ordering::Greater
        );
        assert!(expanded.beam_exact_evaluations >= orthogonal.beam_exact_evaluations);
    }

    #[test]
    fn partial_layout_beam_is_identical_across_thread_counts() {
        let wide = rectangle(3.0, 1.0);
        let tall = rectangle(1.0, 2.0);
        let pieces = [
            GeneralFastPiece {
                id: "wide-a",
                polygon: &wide,
                allow_rotation: true,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "wide-b",
                polygon: &wide,
                allow_rotation: true,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "tall-a",
                polygon: &tall,
                allow_rotation: true,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "tall-b",
                polygon: &tall,
                allow_rotation: true,
                allow_mirror: false,
            },
        ];
        let mut settings = GeneralFastSettings::deterministic_test(5.0, 10.0);
        settings.max_evaluations_per_piece = 32;
        settings.max_partial_layouts = 4;
        settings.max_beam_evaluations_per_state = 16;
        settings.max_tightening_passes = 1;

        let serial_pool = JobPool::new(Some(1));
        let serial = serial_pool
            .run_scoped(|| construct_short_side_first(&pieces, settings))
            .unwrap();
        let parallel_pool = JobPool::new(Some(4));
        let parallel = parallel_pool
            .run_scoped(|| construct_short_side_first(&pieces, settings))
            .unwrap();

        assert_eq!(parallel, serial);
    }

    #[test]
    fn explicit_sheet_edge_clearance_is_independent_from_pair_padding() {
        let polygon = square(1.0);
        let pieces = [GeneralFastPiece {
            id: "piece",
            polygon: &polygon,
            allow_rotation: false,
            allow_mirror: false,
        }];
        let mut legacy_settings = GeneralFastSettings::deterministic_test(10.0, 20.0);
        legacy_settings.total_padding_mm = 2.0;
        let legacy = construct_short_side_first(&pieces, legacy_settings).unwrap();
        let mut explicit_settings = legacy_settings;
        explicit_settings.sheet_edge_clearance_mm = Some(3.0);
        let explicit = construct_short_side_first(&pieces, explicit_settings).unwrap();

        assert_eq!(explicit.placements.len(), 1, "{explicit:?}");

        let edge_clearance = |placement: &GeneralFastPlacement| {
            placement
                .translate_short_axis
                .min(10.0 - placement.translate_short_axis - 1.0)
                .min(placement.translate_long_axis)
                .min(20.0 - placement.translate_long_axis - 1.0)
        };
        assert!(edge_clearance(&legacy.placements[0]) < 2.0);
        assert!(edge_clearance(&explicit.placements[0]) >= 3.0);
        assert!(explicit.used_long_axis_depth_mm > legacy.used_long_axis_depth_mm);
    }

    #[test]
    fn tightening_is_bounded_and_never_discards_the_incumbent() {
        let wide = rectangle(3.0, 1.0);
        let tall = rectangle(1.0, 2.0);
        let pieces = [
            GeneralFastPiece {
                id: "wide-a",
                polygon: &wide,
                allow_rotation: true,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "wide-b",
                polygon: &wide,
                allow_rotation: true,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "tall-a",
                polygon: &tall,
                allow_rotation: true,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "tall-b",
                polygon: &tall,
                allow_rotation: true,
                allow_mirror: false,
            },
        ];
        let mut settings = GeneralFastSettings::deterministic_test(5.0, 10.0);
        settings.max_evaluations_per_piece = 32;
        settings.max_partial_layouts = 4;
        settings.max_beam_evaluations_per_state = 16;
        let baseline = construct_short_side_first(&pieces, settings).unwrap();
        settings.max_tightening_passes = 1;
        let tightened = construct_short_side_first(&pieces, settings).unwrap();

        assert_ne!(
            compare_result_quality(&tightened, &baseline),
            Ordering::Greater
        );
        assert!(tightened.tightening_passes_attempted <= 1);
        assert!(tightened.tightening_passes_improved <= tightened.tightening_passes_attempted);
        assert!(tightened.tightening_exact_evaluations <= tightened.beam_exact_evaluations);
    }

    #[test]
    fn partial_layout_beam_can_skip_multiple_feasible_blockers_for_more_parts() {
        let blocker = square(4.0);
        let small = square(2.0);
        let pieces = [
            GeneralFastPiece {
                id: "blocker-a",
                polygon: &blocker,
                allow_rotation: false,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "blocker-b",
                polygon: &blocker,
                allow_rotation: false,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "small-a",
                polygon: &small,
                allow_rotation: false,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "small-b",
                polygon: &small,
                allow_rotation: false,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "small-c",
                polygon: &small,
                allow_rotation: false,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "small-d",
                polygon: &small,
                allow_rotation: false,
                allow_mirror: false,
            },
        ];
        let mut primary_settings = GeneralFastSettings::deterministic_test(4.1, 4.1);
        primary_settings.max_evaluations_per_piece = 64;
        let primary = construct_short_side_first(&pieces, primary_settings).unwrap();
        assert_eq!(primary.placements.len(), 1);

        let mut beam_settings = primary_settings;
        beam_settings.max_partial_layouts = 4;
        beam_settings.max_beam_evaluations_per_state = 128;
        let result = construct_short_side_first(&pieces, beam_settings).unwrap();

        assert_eq!(result.placements.len(), 4);
        assert_eq!(result.unplaced_piece_ids, vec!["blocker-a", "blocker-b"]);
    }

    #[test]
    fn partial_layout_identity_keeps_distinct_piece_assignments() {
        let polygon = square(1.0);
        let placed = |piece_id: &str, input_index: usize| PartialLayout {
            placed: vec![PlacedState {
                input_index,
                placement: GeneralFastPlacement {
                    piece_id: piece_id.to_owned(),
                    rotation_deg: 0.0,
                    mirrored: false,
                    translate_short_axis: 0.0,
                    translate_long_axis: 0.0,
                },
                collision: polygon.clone(),
            }],
            unplaced_piece_ids: Vec::new(),
        };

        assert_ne!(
            partial_layout_state_key(&placed("a", 0)),
            partial_layout_state_key(&placed("b", 1))
        );
    }

    #[test]
    fn angle_quota_reserves_core_non_mirrored_and_mirrored_variants() {
        let polygon = square(1.0);
        let input = GeneralFastPiece {
            id: "piece",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: true,
        };
        let prepared = PreparedGeneralPiece {
            input_index: 0,
            input,
            collision: polygon.clone(),
            area_mm2: polygon.area_mm2(),
            convex_hull_area_mm2: polygon_convex_hull_area_mm2(&polygon),
            diameter_mm: polygon_diameter(&polygon),
            reflex_vertices: 0,
            vertex_count: polygon.vertex_count(),
            long_span_mm: 1.0,
            short_span_mm: 1.0,
            fill_ratio: 1.0,
            shape_family_key: polygon_shape_family_key(&polygon),
        };
        let mut settings = GeneralFastSettings::deterministic_test(10.0, 20.0);
        settings.max_angles_per_piece = 3;

        assert_eq!(
            angle_candidates(&prepared, &[], settings, AngleScope::Full),
            vec![(0.0, false), (90.0, false), (0.0, true)]
        );
    }

    #[test]
    fn longest_edge_alignments_precede_uniform_angle_seeds() {
        let polygon =
            PolygonSet::from_outer(vec![point(0.0, 0.0), point(4.0, 1.0), point(0.0, 2.0)])
                .unwrap();
        let input = GeneralFastPiece {
            id: "skewed",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: false,
        };
        let prepared = PreparedGeneralPiece {
            input_index: 0,
            input,
            collision: polygon.clone(),
            area_mm2: polygon.area_mm2(),
            convex_hull_area_mm2: polygon_convex_hull_area_mm2(&polygon),
            diameter_mm: polygon_diameter(&polygon),
            reflex_vertices: 0,
            vertex_count: polygon.vertex_count(),
            long_span_mm: 4.0,
            short_span_mm: 2.0,
            fill_ratio: 1.0,
            shape_family_key: polygon_shape_family_key(&polygon),
        };
        let mut settings = GeneralFastSettings::deterministic_test(10.0, 20.0);
        settings.angle_seed_count = 16;
        settings.max_angles_per_piece = 4;

        let angles = angle_candidates(&prepared, &[], settings, AngleScope::Full);
        assert_eq!(angles.len(), 4);
        assert_eq!(angles[0], (0.0, false));
        assert_eq!(angles[1], (90.0, false));
        assert!((angles[2].0 - 345.963757).abs() < 1e-6);
        assert!((angles[3].0 - 75.963757).abs() < 1e-6);
    }

    #[test]
    fn mirrored_edge_alignments_use_reflected_edge_directions() {
        let polygon =
            PolygonSet::from_outer(vec![point(0.0, 0.0), point(5.0, 1.0), point(0.0, 3.0)])
                .unwrap();
        let input = GeneralFastPiece {
            id: "skewed",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: true,
        };
        let prepared = PreparedGeneralPiece {
            input_index: 0,
            input,
            collision: polygon.clone(),
            area_mm2: polygon.area_mm2(),
            convex_hull_area_mm2: polygon_convex_hull_area_mm2(&polygon),
            diameter_mm: polygon_diameter(&polygon),
            reflex_vertices: 0,
            vertex_count: polygon.vertex_count(),
            long_span_mm: 5.0,
            short_span_mm: 3.0,
            fill_ratio: 1.0,
            shape_family_key: polygon_shape_family_key(&polygon),
        };
        let mut settings = GeneralFastSettings::deterministic_test(10.0, 20.0);
        settings.max_angles_per_piece = 12;

        let angles = angle_candidates(&prepared, &[], settings, AngleScope::Full);
        let first_non_mirrored_edge_angle = angles
            .iter()
            .find(|(angle, mirrored)| !mirrored && *angle != 0.0 && *angle != 90.0)
            .unwrap()
            .0;
        let first_mirrored_edge_angle = angles
            .iter()
            .find(|(angle, mirrored)| *mirrored && *angle != 0.0 && *angle != 90.0)
            .unwrap()
            .0;

        assert!((first_non_mirrored_edge_angle - 201.801409).abs() < 1e-6);
        assert!((first_mirrored_edge_angle - 338.198591).abs() < 1e-6);
    }

    #[test]
    fn candidate_score_prioritizes_long_axis_gravity_then_short_axis_gravity() {
        let key = CandidateKey {
            angle_key: 0,
            mirrored: false,
            kind: CandidateKind::SheetSupport,
            fixed_piece_id_rank: 0,
            fixed_feature_ordinal: 0,
            moving_feature_ordinal: 0,
            translate_x_grid: 0,
            translate_y_grid: 0,
        };
        let low_long_axis = CandidateScore {
            candidate_long_axis_position: 1.0,
            candidate_short_axis_position: 100.0,
            long_axis_depth: 100.0,
            unused_short_axis_projection: 100.0,
            envelope_area: 100.0,
        };
        let high_long_axis = CandidateScore {
            candidate_long_axis_position: 2.0,
            candidate_short_axis_position: 0.0,
            long_axis_depth: 0.0,
            unused_short_axis_projection: 0.0,
            envelope_area: 0.0,
        };
        assert_eq!(
            compare_candidate_scores(low_long_axis, key, high_long_axis, key),
            Ordering::Less
        );

        let low_short_axis = CandidateScore {
            candidate_long_axis_position: 1.0,
            candidate_short_axis_position: 1.0,
            ..low_long_axis
        };
        let high_short_axis = CandidateScore {
            candidate_long_axis_position: 1.0,
            candidate_short_axis_position: 2.0,
            ..high_long_axis
        };
        assert_eq!(
            compare_candidate_scores(low_short_axis, key, high_short_axis, key),
            Ordering::Less
        );
    }

    #[test]
    fn exact_evaluation_quota_bounds_candidate_generation() {
        let polygon = square(1.0);
        let pieces = [GeneralFastPiece {
            id: "piece",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: true,
        }];
        let mut settings = GeneralFastSettings::deterministic_test(10.0, 20.0);
        settings.max_evaluations_per_piece = 3;

        let result = construct_short_side_first(&pieces, settings).unwrap();
        assert!(result.exact_evaluations <= 3);
        assert_eq!(result.primary_exact_evaluations, result.exact_evaluations);
        assert_eq!(result.exploratory_exact_evaluations, 0);
    }

    #[test]
    fn single_primary_evaluation_is_a_hard_quota() {
        let polygon = square(1.0);
        let pieces = [GeneralFastPiece {
            id: "piece",
            polygon: &polygon,
            allow_rotation: false,
            allow_mirror: false,
        }];
        let mut settings = GeneralFastSettings::deterministic_test(10.0, 20.0);
        settings.max_evaluations_per_piece = 1;

        let result = construct_short_side_first(&pieces, settings).unwrap();
        assert_eq!(result.primary_exact_evaluations, 1);
        assert_eq!(result.exact_evaluations, 1);
    }

    #[test]
    fn frontier_sheet_support_keeps_a_tiny_evaluation_constructor_feasible() {
        let strip = rectangle(9.0, 1.0);
        let pieces = ["a", "b", "c"].map(|id| GeneralFastPiece {
            id,
            polygon: &strip,
            allow_rotation: false,
            allow_mirror: false,
        });
        let mut settings = GeneralFastSettings::deterministic_test(10.0, 4.0);
        settings.max_evaluations_per_piece = 3;

        let result = construct_short_side_first(&pieces, settings).unwrap();

        assert_eq!(result.placements.len(), 3);
        assert!(result.unplaced_piece_ids.is_empty());
        assert_eq!(result.primary_exact_evaluations, 8);
        assert!((result.used_long_axis_depth_mm - 3.012).abs() < 1e-9);
    }

    #[test]
    fn single_exploratory_evaluation_bounds_the_global_proposal_portfolio() {
        let polygon = square(1.0);
        let pieces = [GeneralFastPiece {
            id: "piece",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: true,
        }];
        let mut settings = GeneralFastSettings::deterministic_test(10.0, 20.0);
        settings.max_angles_per_piece = 64;
        settings.max_exploratory_evaluations_per_piece = 1;

        let result = construct_short_side_first(&pieces, settings).unwrap();
        assert_eq!(result.exploratory_exact_evaluations, 1);
    }

    #[test]
    fn fast_arm_uses_arbitrary_rotation_when_orthogonal_poses_do_not_fit() {
        let rectangle = PolygonSet::from_outer(vec![
            point(0.0, 0.0),
            point(2.0, 0.0),
            point(2.0, 0.5),
            point(0.0, 0.5),
        ])
        .unwrap();
        let pieces = [GeneralFastPiece {
            id: "rectangle",
            polygon: &rectangle,
            allow_rotation: true,
            allow_mirror: false,
        }];
        let fast_settings = GeneralFastSettings::deterministic_test(1.8, 1.8);
        let fast = construct_short_side_first(&pieces, fast_settings).unwrap();
        assert!(fast.unplaced_piece_ids.is_empty());
        assert_eq!(fast.placements.len(), 1);
        assert_eq!(fast.placements[0].rotation_deg.rem_euclid(90.0), 45.0);
        validate_and_measure_placements(&pieces, &fast.placements, fast_settings).unwrap();

        let mut exploratory_settings = fast_settings;
        exploratory_settings.angle_seed_count = 8;
        exploratory_settings.max_exploratory_evaluations_per_piece = 64;
        let exploratory = construct_short_side_first(&pieces, exploratory_settings).unwrap();

        assert!(exploratory.unplaced_piece_ids.is_empty());
        assert_eq!(exploratory.placements.len(), 1);
        assert!(exploratory.exploratory_exact_evaluations > 0);
        assert_ne!(
            compare_result_quality(&exploratory, &fast),
            Ordering::Greater
        );
        assert_eq!(
            exploratory.exact_evaluations,
            exploratory.primary_exact_evaluations + exploratory.exploratory_exact_evaluations
        );
    }

    #[test]
    fn exploratory_arm_cannot_return_a_worse_complete_layout() {
        let l = l_shape();
        let pocket = square(2.5);
        let pieces = [
            GeneralFastPiece {
                id: "l",
                polygon: &l,
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "pocket",
                polygon: &pocket,
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let fast_settings = GeneralFastSettings::deterministic_test(4.01, 4.01);
        let fast = construct_short_side_first(&pieces, fast_settings).unwrap();
        let mut exploratory_settings = fast_settings;
        exploratory_settings.max_exploratory_evaluations_per_piece = 64;
        let exploratory = construct_short_side_first(&pieces, exploratory_settings).unwrap();

        assert_ne!(exploratory.exploratory_exact_evaluations, 0);
        assert_ne!(
            compare_result_quality(&exploratory, &fast),
            Ordering::Greater
        );
        assert!(!exploratory.exploratory_failed);
    }

    #[test]
    fn failed_optional_arm_retains_the_completed_primary_incumbent() {
        let polygon = square(1.0);
        let pieces = [GeneralFastPiece {
            id: "piece",
            polygon: &polygon,
            allow_rotation: false,
            allow_mirror: false,
        }];
        let primary = construct_short_side_first(
            &pieces,
            GeneralFastSettings::deterministic_test(10.0, 20.0),
        )
        .unwrap();
        let retained = select_optional_arm(
            primary.clone(),
            Err(GeneralFastError::InvalidSettings(
                "synthetic optional-arm failure".to_owned(),
            )),
            7,
        );
        assert_eq!(retained.placements, primary.placements);
        assert_eq!(
            retained.primary_exact_evaluations,
            primary.primary_exact_evaluations
        );
        assert_eq!(retained.exact_evaluations, primary.exact_evaluations + 7);
        assert_eq!(retained.exploratory_exact_evaluations, 7);
        assert!(retained.exploratory_failed);
    }

    #[test]
    fn exploratory_winner_retains_primary_tightening_diagnostics() {
        let polygon = square(1.0);
        let pieces = [GeneralFastPiece {
            id: "piece",
            polygon: &polygon,
            allow_rotation: false,
            allow_mirror: false,
        }];
        let mut primary = construct_short_side_first(
            &pieces,
            GeneralFastSettings::deterministic_test(10.0, 20.0),
        )
        .unwrap();
        primary.tightening_exact_evaluations = 17;
        primary.tightening_passes_attempted = 2;
        primary.tightening_passes_improved = 1;
        let mut exploratory = primary.clone();
        exploratory.used_long_axis_depth_mm = primary.used_long_axis_depth_mm - 0.1;
        exploratory.tightening_exact_evaluations = 0;
        exploratory.tightening_passes_attempted = 0;
        exploratory.tightening_passes_improved = 0;

        let selected = select_best_arm(primary, exploratory);

        assert_eq!(selected.tightening_exact_evaluations, 17);
        assert_eq!(selected.tightening_passes_attempted, 2);
        assert_eq!(selected.tightening_passes_improved, 1);
    }

    #[test]
    fn rejects_constructor_quotas_above_the_supported_work_bounds() {
        let polygon = square(1.0);
        let pieces = [GeneralFastPiece {
            id: "piece",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: true,
        }];
        let mut settings = GeneralFastSettings::deterministic_test(10.0, 20.0);
        settings.max_angles_per_piece = DEFAULT_MAX_ANGLES_PER_PIECE + 1;
        assert!(matches!(
            construct_short_side_first(&pieces, settings),
            Err(GeneralFastError::InvalidSettings(_))
        ));
    }

    #[test]
    fn input_permutation_does_not_change_the_constructor_result() {
        let l = l_shape();
        let pocket = square(2.5);
        let forward = [
            GeneralFastPiece {
                id: "l",
                polygon: &l,
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "pocket",
                polygon: &pocket,
                allow_rotation: true,
                allow_mirror: true,
            },
        ];
        let reverse = [forward[1], forward[0]];
        let settings = GeneralFastSettings::deterministic_test(4.01, 4.01);

        assert_eq!(
            construct_short_side_first(&forward, settings).unwrap(),
            construct_short_side_first(&reverse, settings).unwrap()
        );
    }

    #[test]
    fn long_span_first_avoids_the_greedy_dead_end_before_portfolio_expansion() {
        let first_square = square(3.0);
        let second_square = square(3.0);
        let long_piece = rectangle(1.0, 5.0);
        let pieces = [
            GeneralFastPiece {
                id: "square-a",
                polygon: &first_square,
                allow_rotation: false,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "square-b",
                polygon: &second_square,
                allow_rotation: false,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "long",
                polygon: &long_piece,
                allow_rotation: false,
                allow_mirror: false,
            },
        ];
        let baseline_settings = GeneralFastSettings::deterministic_test(6.01, 10.0);
        let baseline = construct_short_side_first(&pieces, baseline_settings).unwrap();
        let mut portfolio_settings = baseline_settings;
        portfolio_settings.max_order_variants = 4;
        let portfolio = construct_short_side_first(&pieces, portfolio_settings).unwrap();

        assert_eq!(baseline.placements.len(), 3);
        assert_eq!(portfolio.placements.len(), 3);
        assert!(baseline.unplaced_piece_ids.is_empty());
        assert_ne!(
            compare_result_quality(&portfolio, &baseline),
            Ordering::Greater
        );
        assert_eq!(portfolio.order_variants_attempted, 2);
        assert!(portfolio.order_portfolio_exact_evaluations > 0);
        assert_eq!(
            portfolio.exact_evaluations,
            portfolio.primary_exact_evaluations
                + portfolio.order_portfolio_exact_evaluations
                + portfolio.exploratory_exact_evaluations
        );
    }

    #[test]
    fn duplicate_piece_orders_do_not_consume_portfolio_work() {
        let first = square(2.0);
        let second = square(1.0);
        let pieces = [
            GeneralFastPiece {
                id: "large",
                polygon: &first,
                allow_rotation: false,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "small",
                polygon: &second,
                allow_rotation: false,
                allow_mirror: false,
            },
        ];
        let mut settings = GeneralFastSettings::deterministic_test(10.0, 20.0);
        settings.max_order_variants = 4;
        let result = construct_short_side_first(&pieces, settings).unwrap();

        assert_eq!(result.order_variants_attempted, 1);
        assert_eq!(result.order_portfolio_exact_evaluations, 0);
    }

    #[test]
    fn order_portfolio_is_permutation_invariant_and_globally_bounded() {
        let first_square = square(3.0);
        let second_square = square(3.0);
        let long_piece = rectangle(1.0, 5.0);
        let forward = [
            GeneralFastPiece {
                id: "square-a",
                polygon: &first_square,
                allow_rotation: false,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "square-b",
                polygon: &second_square,
                allow_rotation: false,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "long",
                polygon: &long_piece,
                allow_rotation: false,
                allow_mirror: false,
            },
        ];
        let reverse = [forward[2], forward[1], forward[0]];
        let mut settings = GeneralFastSettings::deterministic_test(6.01, 10.0);
        settings.max_order_variants = 4;
        settings.max_evaluations_per_piece = 8;
        let first = construct_short_side_first(&forward, settings).unwrap();
        let second = construct_short_side_first(&reverse, settings).unwrap();

        assert_eq!(first, second);
        assert!(
            first.order_portfolio_exact_evaluations
                <= (first.order_variants_attempted - 1)
                    * forward.len()
                    * settings.max_evaluations_per_piece
        );
    }

    #[test]
    fn repair_is_deterministic_bounded_and_never_worsens_the_incumbent() {
        let l = l_shape();
        let pocket = square(2.5);
        let rectangle = rectangle(0.8, 3.5);
        let pieces = [
            GeneralFastPiece {
                id: "l",
                polygon: &l,
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "pocket",
                polygon: &pocket,
                allow_rotation: true,
                allow_mirror: true,
            },
            GeneralFastPiece {
                id: "rectangle",
                polygon: &rectangle,
                allow_rotation: true,
                allow_mirror: false,
            },
        ];
        let baseline_settings = GeneralFastSettings::deterministic_test(5.0, 8.0);
        let baseline = construct_short_side_first(&pieces, baseline_settings).unwrap();
        let mut repair_settings = baseline_settings;
        repair_settings.max_repair_targets = 3;
        repair_settings.max_repair_evaluations_per_piece = 32;
        repair_settings.max_local_angle_refinement_evaluations_per_piece = 8;
        let first = construct_short_side_first(&pieces, repair_settings).unwrap();
        let second = construct_short_side_first(&pieces, repair_settings).unwrap();

        assert_eq!(first, second);
        assert_ne!(compare_result_quality(&first, &baseline), Ordering::Greater);
        assert!(!first.repair_failed);
        assert_eq!(first.repair_targets_considered, 3);
        assert!(first.repair_exact_evaluations <= 3 * 32);
        assert!(first.local_angle_refinement_exact_evaluations <= 3 * 8);
        assert!(first.local_angle_refinement_exact_evaluations <= first.repair_exact_evaluations);
        assert_eq!(
            first.exact_evaluations,
            first.primary_exact_evaluations
                + first.order_portfolio_exact_evaluations
                + first.exploratory_exact_evaluations
                + first.repair_exact_evaluations
        );
    }

    #[test]
    fn failed_reinsert_keeps_the_original_placement() {
        let polygon = square(2.0);
        let pieces = [GeneralFastPiece {
            id: "piece",
            polygon: &polygon,
            allow_rotation: false,
            allow_mirror: false,
        }];
        let baseline_settings = GeneralFastSettings::deterministic_test(2.01, 2.01);
        let baseline = construct_short_side_first(&pieces, baseline_settings).unwrap();
        let mut repair_settings = baseline_settings;
        repair_settings.max_repair_targets = 1;
        repair_settings.max_repair_evaluations_per_piece = 4;
        let repaired = construct_short_side_first(&pieces, repair_settings).unwrap();

        assert_eq!(repaired.placements, baseline.placements);
        assert_eq!(
            repaired.used_long_axis_depth_mm,
            baseline.used_long_axis_depth_mm
        );
        assert_eq!(repaired.repair_targets_considered, 1);
    }

    #[test]
    fn local_angle_neighborhood_is_canonical_and_wrap_safe() {
        let angles = local_angle_neighborhood(359.0, true, 16);
        let keys = angles
            .iter()
            .map(|(angle, mirrored)| (angle_key(*angle), *mirrored))
            .collect::<Vec<_>>();
        assert_eq!(keys.len(), 4);
        assert_eq!(keys.iter().copied().collect::<BTreeSet<_>>().len(), 4);
        assert!(keys.iter().all(|(key, mirrored)| {
            (0..angle_key(360.0 - 1.0 / ANGLE_KEY_SCALE) + 1).contains(key) && *mirrored
        }));
    }

    #[test]
    fn local_angle_quota_must_be_a_strict_repair_subquota() {
        let polygon = square(1.0);
        let pieces = [GeneralFastPiece {
            id: "piece",
            polygon: &polygon,
            allow_rotation: true,
            allow_mirror: false,
        }];
        let mut settings = GeneralFastSettings::deterministic_test(10.0, 20.0);
        settings.max_repair_targets = 1;
        settings.max_repair_evaluations_per_piece = 8;
        settings.max_local_angle_refinement_evaluations_per_piece = 8;
        assert!(matches!(
            construct_short_side_first(&pieces, settings),
            Err(GeneralFastError::InvalidSettings(_))
        ));
    }

    #[test]
    fn repair_never_refines_the_angle_of_a_nonrotatable_piece() {
        let polygon = rectangle(1.0, 2.0);
        let pieces = [GeneralFastPiece {
            id: "fixed-angle",
            polygon: &polygon,
            allow_rotation: false,
            allow_mirror: false,
        }];
        let mut settings = GeneralFastSettings::deterministic_test(4.0, 4.0);
        settings.max_repair_targets = 1;
        settings.max_repair_evaluations_per_piece = 16;
        settings.max_local_angle_refinement_evaluations_per_piece = 4;

        let result = construct_short_side_first(&pieces, settings).unwrap();
        assert_eq!(result.placements[0].rotation_deg, 0.0);
        assert_eq!(result.local_angle_refinement_exact_evaluations, 0);
    }

    #[test]
    fn duplicate_piece_ids_are_rejected_before_search() {
        let first = square(1.0);
        let second = square(2.0);
        let pieces = [
            GeneralFastPiece {
                id: "duplicate",
                polygon: &first,
                allow_rotation: false,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "duplicate",
                polygon: &second,
                allow_rotation: false,
                allow_mirror: false,
            },
        ];

        assert!(matches!(
            construct_short_side_first(&pieces, GeneralFastSettings::deterministic_test(4.0, 4.0)),
            Err(GeneralFastError::InvalidInput(_))
        ));
    }

    #[test]
    fn out_of_range_translations_never_alias_to_the_origin() {
        assert_eq!(grid_key(f64::INFINITY), None);
        assert_eq!(grid_key(f64::MAX), None);
        assert_eq!(grid_key(0.0), Some(0));
    }
}
