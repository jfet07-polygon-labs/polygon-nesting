//! Micro-legalization: a deterministic repair pass for *near-feasible* layouts.
//!
//! A converged deep layout that misses the exact clearance contract by a few
//! pairs at a ~0.001 mm scale is not a search problem, it is a projection
//! problem. Global relaxation re-opens the whole layout and generally walks
//! back out of the basin; this module instead treats the residue as a small
//! system of separation constraints and projects the offending pieces - and
//! only those pieces - onto the feasible side of it.
//!
//! # What has to be satisfied
//!
//! Publication passes through two independent gates, and a repair that clears
//! only one of them is worthless. Both are modelled here, on the geometry each
//! one actually measures:
//!
//! * the **material** gate ([`crate::validation::general_polygon`]): the
//!   transformed *source* outlines of every pair must stay
//!   `total_padding + 2 * sag` apart, and every outer vertex must sit inside
//!   the sheet clearance box;
//! * the **canonical-grid** gate ([`validate_and_measure_placements`]): the
//!   Clipper offset *collision envelopes* must not overlap, and must fit the
//!   inset sheet.
//!
//! Neither gate implies the other, and each is measured on its own terms.
//!
//! The envelope is offset with **miter** joins, so two convex corners meeting
//! diagonally push their envelopes together far faster than their material
//! distance suggests - a pair 5.9 mm apart in material can still meet on the
//! grid at a 2.5 mm expansion. Modelling the material contract alone therefore
//! produces repairs the authoritative validator rejects.
//!
//! The converse trap is subtler and cost this module a rewrite. The envelope
//! gate measures intersection *area* on the integer canonical grid, which
//! quietly tolerates crossings finer than the grid can represent. The certified
//! 166.855 mm layout in this repository has 39 pairs whose envelope edges
//! properly cross in exact arithmetic, by around a thousandth of a millimetre,
//! and Clipper's integer intersection rounds every one of them away to zero
//! area. A hand-rolled crossing predicate is thus *stricter* than the gate, and
//! the first version of this pass duly reported that certified layout as having
//! 39 violations. The envelope gate is therefore never reimplemented here: it
//! is asked of [`polygons_overlap_exact`] directly.
//!
//! # Constraint model
//!
//! Poses are frozen: rotation and mirror never change, only translations. For
//! a pair of outlines `P_i`, `P_j` in one family the requirement is
//!
//! ```text
//! dist(P_i + t_i, P_j + t_j) >= target
//! ```
//!
//! Where the outlines are separated, the distance function is differentiable
//! and its gradient is the unit vector along the closest-approach witness
//! segment, so the requirement linearizes to one inequality per pair,
//!
//! ```text
//! (t_i - t_j) . n_ij >= target - dist_ij
//! ```
//!
//! and boundary containment to four axis-aligned inequalities per piece. The
//! linearization is only ever used to pick a step: every round re-measures the
//! true distance rather than propagating the linear model.
//!
//! # Solver
//!
//! Projected Gauss-Seidel. Each round walks a fixed constraint order,
//! re-measures each constraint exactly, and immediately applies the minimal
//! translation that clears it, splitting the correction evenly between the two
//! endpoints when both may move and loading it entirely onto the movable
//! endpoint otherwise. Translations are snapped to the canonical 1/1000 mm
//! grid at the end of every round, so the fixpoint is grid-exact and no
//! post-hoc snap can re-open a constraint; corrections smaller than the snap
//! quantum are suppressed so the solver cannot ping-pong against its own
//! rounding. An overlapping envelope pair carries no magnitude at all - the
//! gate is a boolean - so the separating travel is recovered by bisecting
//! against the gate itself along the material witness direction, rather than
//! guessed at.
//!
//! # Bounds
//!
//! * Only pieces incident to a violation move. Every other piece is frozen.
//! * Each moving piece has a cumulative displacement cap.
//! * Constraints are collected for every pair within a *guard band* of twice
//!   the cap, so a pair outside the constraint set provably cannot be brought
//!   into violation by any admissible displacement. This is what makes "do not
//!   touch anything outside the affected component" safe rather than hopeful.
//! * A residue whose deficit is large relative to the contract, or whose
//!   violation component spans too much of the layout, is refused outright
//!   rather than attempted: that is a search problem, not a projection.
//! * Rounds and escalations are capped.
//!
//! The pass never publishes on its own authority: a result is returned only
//! after [`validate_and_measure_placements`] accepts it against the real
//! request.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::domain::IrregularPoint;
use crate::geometry::general_polygon::PolygonSet;
use crate::geometry::predicates::orientation;
use crate::search::general_fast::{
    collision_expansion_mm, collision_sheet_inset_mm, effective_sheet_edge_clearance_mm,
    polygons_overlap_exact, validate_and_measure_placements, GeneralFastPiece,
    GeneralFastPlacement, GeneralFastSettings,
};

/// Grid slack added on top of each exact contract so the projection lands
/// strictly inside it rather than on its boundary.
const MICRO_LEGALIZATION_MARGIN_MM: f64 = 0.002;

/// How many times the pass may re-run with an enlarged margin when the
/// geometric fixpoint is reached but the authoritative validator still
/// rejects. Each escalation adds another [`MICRO_LEGALIZATION_MARGIN_MM`].
const MICRO_LEGALIZATION_ESCALATIONS: usize = 3;

/// Projection rounds per escalation.
const MICRO_LEGALIZATION_ROUNDS: usize = 64;

/// Cumulative per-piece displacement cap, as a multiple of the largest deficit
/// the pass was asked to clear. A coordinated repair may legitimately need
/// several times the raw deficit because clearing one pair loads another.
const MICRO_LEGALIZATION_CAP_FACTOR: f64 = 8.0;

/// Floor under that cap, as a fraction of the requested pair clearance, so a
/// vanishing deficit still gets room to move off the contract boundary.
const MICRO_LEGALIZATION_MIN_CAP_RATIO: f64 = 0.01;

/// Absolute floor under the cap, for contracts with no clearance at all.
const MICRO_LEGALIZATION_MIN_CAP_MM: f64 = 0.01;

/// The largest deficit the pass will accept, as a fraction of the pair
/// clearance contract. Beyond it the residue is not a rounding-scale miss and
/// a bounded nudge is the wrong instrument.
const MICRO_LEGALIZATION_MAX_DEFICIT_RATIO: f64 = 0.25;

/// The same admission bound expressed against piece size, so a contract with a
/// negligible clearance still admits a rounding-scale residue.
const MICRO_LEGALIZATION_MAX_DEFICIT_EXTENT_RATIO: f64 = 0.005;

/// How far past the violation component the movable set may reach, as a
/// multiple of the component limit. One hop of neighbours is what lets a seed
/// wedged against an immovable neighbour move at all; the multiple keeps that
/// hop from quietly becoming a global relaxation.
const MICRO_LEGALIZATION_NEIGHBOURHOOD_LIMIT_FACTOR: usize = 4;

/// The canonical grid quantum, in millimetres.
const GRID_MM: f64 = 0.001;

/// Convergence dead band. Snapping both endpoints of a pair to the canonical
/// grid can shorten their separation by up to half a grid unit per axis per
/// endpoint, so a solver that insisted on the exact target would ping-pong
/// against its own rounding forever. Corrections below this band are therefore
/// not applied, which makes the fixpoint reachable;
/// [`MICRO_LEGALIZATION_MARGIN_MM`] is larger than the band, so even a state
/// sitting at its lower edge still clears the bare contract.
const MICRO_LEGALIZATION_SNAP_SLACK_MM: f64 = GRID_MM * std::f64::consts::SQRT_2;

/// Diagnostics for one micro-legalization attempt.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralMicroLegalizationDiagnostics {
    pub attempted: bool,
    /// Separation the pass drives every pair of transformed source outlines
    /// to, including the grid margin.
    pub material_target_mm: f64,
    /// Separation the pass drives every pair of collision envelopes to. The
    /// envelopes only have to miss each other, so this is the margin alone.
    pub collision_target_mm: f64,
    pub displacement_cap_mm: f64,
    /// Violations measured on the input state, against the bare contracts.
    pub violating_pairs_before: usize,
    pub boundary_pieces_before: usize,
    /// How the input's violating pairs split between the two gates. A pair can
    /// appear in both.
    pub material_pairs_before: usize,
    pub collision_pairs_before: usize,
    /// The worst shortfall against the requested pair clearance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_material_deficit_mm: Option<f64>,
    /// The worst travel needed to pull an overlapping envelope pair apart.
    ///
    /// Reported separately from the clearance shortfall because the two are
    /// routinely orders of magnitude apart: a pair can miss the clearance
    /// contract by a thousandth of a millimetre while its miter-joined
    /// envelopes need whole millimetres of travel to come apart at a spike far
    /// from the closest-approach point. Collapsing them into one number is what
    /// makes a residue look micro when it is not.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_envelope_push_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_boundary_deficit_mm: Option<f64>,
    /// The admission bound the deficits were judged against.
    pub admissible_deficit_mm: f64,
    /// The violation graph: connected components over violating pairs, with
    /// boundary-only pieces contributing singleton components.
    pub component_count: usize,
    pub largest_component_pieces: usize,
    /// The admissible component size for this piece count.
    pub component_limit: usize,
    /// Pieces incident to a violation, which seed the repair.
    pub seed_pieces: usize,
    /// Pieces the pass was allowed to move - the seeds plus a bounded one-hop
    /// neighbourhood - and how many it actually moved.
    pub movable_pieces: usize,
    pub moved_pieces: usize,
    /// Pair constraints in the solved system, including guard-band pairs that
    /// were not violated but had to be protected.
    pub pair_constraints: usize,
    pub rounds_run: usize,
    pub escalations_run: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_displacement_mm: Option<f64>,
    pub displacement_capped: bool,
    /// Violations measured on the output state, against the bare contracts.
    pub violating_pairs_after: usize,
    pub boundary_pieces_after: usize,
    /// Whether the geometric constraint system reached a feasible fixpoint.
    pub resolved: bool,
    /// Whether the authoritative validator accepted the repaired state.
    pub exact_valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
}

/// The admissible size of a violation component, derived from the piece count
/// so the pass stays a *micro* repair at every scale: a residue that involves
/// more than an eighth of the layout is a search problem.
pub(crate) fn micro_legalization_component_limit(piece_count: usize) -> usize {
    (piece_count / 8).max(4)
}

/// Which of the two publication gates a constraint belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gate {
    /// Transformed source outlines against the requested clearance.
    Material,
    /// Clipper offset collision envelopes against each other.
    Collision,
}

const GATES: [Gate; 2] = [Gate::Material, Gate::Collision];

/// One piece's geometry in both gates, at its pose and original translation.
/// Repair deltas are applied as offsets at measurement time rather than
/// rebuilt, which is exact because a translation commutes with both the
/// transform and the offset once it is snapped to the canonical grid.
struct PieceGeometry {
    material: Outline,
    collision: Outline,
    /// The collision envelope as the engine's own polygon type, so the
    /// envelope gate can be asked of [`polygons_overlap_exact`] itself rather
    /// than of a reimplementation of it.
    ///
    /// This distinction is not pedantry. The gate measures *intersection area*
    /// on the integer canonical grid, and that quietly tolerates crossings
    /// finer than the grid can represent: a certified layout in this repository
    /// has 39 pairs whose envelope edges properly cross in exact arithmetic, by
    /// around a thousandth of a millimetre, every one of which Clipper's
    /// integer intersection rounds away to zero area and the validator
    /// therefore accepts. A hand-rolled crossing predicate is *stricter* than
    /// the gate, and a repair pass built on one condemns perfectly good
    /// layouts. Asking the gate directly is the only way to agree with it.
    collision_shape: PolygonSet,
}

impl PieceGeometry {
    fn outline(&self, gate: Gate) -> &Outline {
        match gate {
            Gate::Material => &self.material,
            Gate::Collision => &self.collision,
        }
    }
}

struct Outline {
    rings: Vec<Vec<IrregularPoint>>,
    ring_bounds: Vec<Bounds>,
    bounds: Bounds,
    /// Bounds of the outer rings alone, which are the only rings either sheet
    /// containment check looks at.
    outer_bounds: Bounds,
    centroid: IrregularPoint,
}

#[derive(Clone, Copy, Debug)]
struct Bounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Bounds {
    fn empty() -> Self {
        Self {
            min_x: f64::INFINITY,
            min_y: f64::INFINITY,
            max_x: f64::NEG_INFINITY,
            max_y: f64::NEG_INFINITY,
        }
    }

    fn observe(&mut self, point: IrregularPoint) {
        self.min_x = self.min_x.min(point.x);
        self.min_y = self.min_y.min(point.y);
        self.max_x = self.max_x.max(point.x);
        self.max_y = self.max_y.max(point.y);
    }

    fn translated(self, dx: f64, dy: f64) -> Self {
        Self {
            min_x: self.min_x + dx,
            min_y: self.min_y + dy,
            max_x: self.max_x + dx,
            max_y: self.max_y + dy,
        }
    }

    /// The axis-aligned gap between two boxes: `0.0` when they overlap. A lower
    /// bound on the true distance between anything inside them.
    fn gap(self, other: Self) -> f64 {
        let dx = (other.min_x - self.max_x)
            .max(self.min_x - other.max_x)
            .max(0.0);
        let dy = (other.min_y - self.max_y)
            .max(self.min_y - other.max_y)
            .max(0.0);
        dx.hypot(dy)
    }

    fn min_extent(self) -> f64 {
        (self.max_x - self.min_x).min(self.max_y - self.min_y)
    }
}

/// The closest-approach measurement between two outlines.
#[derive(Clone, Copy, Debug)]
struct Approach {
    distance: f64,
    /// Unit vector pointing from the witness point on the second outline to the
    /// witness point on the first: the direction the first must move to open
    /// the pair. `None` when the outlines meet, where the distance function is
    /// flat and carries no gradient.
    direction: Option<(f64, f64)>,
}

/// The separation each gate demands, before the solver's margin is added.
#[derive(Clone, Copy, Debug)]
struct Contracts {
    /// Minimum distance between two transformed source outlines.
    material_pair_mm: f64,
    /// Minimum distance from a source outer vertex to each sheet edge.
    material_edge_mm: f64,
    /// Minimum distance from a collision envelope to the inset sheet edge.
    collision_edge_mm: f64,
}

impl Contracts {
    fn pair(&self, gate: Gate) -> f64 {
        match gate {
            Gate::Material => self.material_pair_mm,
            // Envelopes only have to miss each other. Touching is legal to the
            // grid gate, which measures intersection *area*, but is treated as
            // a violation here so the repair lands strictly clear of it.
            Gate::Collision => 0.0,
        }
    }

    fn edge(&self, gate: Gate) -> f64 {
        match gate {
            Gate::Material => self.material_edge_mm,
            Gate::Collision => self.collision_edge_mm,
        }
    }
}

/// Runs the micro-legalization pass over `placements`.
///
/// Returns the diagnostics for the attempt, plus the repaired placements when
/// - and only when - the authoritative validator accepted them.
pub(crate) fn micro_legalize(
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
    settings: GeneralFastSettings,
) -> (
    GeneralMicroLegalizationDiagnostics,
    Option<Vec<GeneralFastPlacement>>,
) {
    let mut diagnostics = GeneralMicroLegalizationDiagnostics::default();
    if placements.len() != pieces.len() {
        diagnostics.skipped_reason =
            Some("micro-legalization requires a complete layout".to_owned());
        return (diagnostics, None);
    }
    if pieces.is_empty() {
        diagnostics.skipped_reason =
            Some("micro-legalization requires at least one piece".to_owned());
        return (diagnostics, None);
    }

    let expansion_mm = collision_expansion_mm(settings);
    let contracts = Contracts {
        material_pair_mm: settings.total_padding_mm + 2.0 * settings.flattening_sag_tolerance_mm,
        material_edge_mm: effective_sheet_edge_clearance_mm(settings)
            + settings.flattening_sag_tolerance_mm,
        collision_edge_mm: collision_sheet_inset_mm(settings),
    };
    if !contracts.material_pair_mm.is_finite()
        || !contracts.material_edge_mm.is_finite()
        || !contracts.collision_edge_mm.is_finite()
        || !expansion_mm.is_finite()
    {
        diagnostics.skipped_reason =
            Some("micro-legalization requires a finite clearance contract".to_owned());
        return (diagnostics, None);
    }

    // Placements carry a piece id and are *not* required to arrive in the piece
    // order, which is why the authoritative validator resolves them through a
    // map. Doing anything else here silently measures one piece's outline at
    // another piece's pose.
    let pieces_by_id = pieces
        .iter()
        .enumerate()
        .map(|(index, piece)| (piece.id, index))
        .collect::<BTreeMap<_, _>>();
    let geometries = match placements
        .iter()
        .map(|placement| {
            let piece = pieces_by_id
                .get(placement.piece_id.as_str())
                .map(|index| pieces[*index])?;
            build_geometry(piece.polygon, placement, expansion_mm)
        })
        .collect::<Option<Vec<_>>>()
    {
        Some(geometries) => geometries,
        None => {
            diagnostics.skipped_reason =
                Some("micro-legalization could not build a piece envelope".to_owned());
            return (diagnostics, None);
        }
    };

    // Survey the input state against the bare contracts: this is the residue
    // the caller wants reported, independent of how the solver is tuned.
    let zero = vec![(0.0f64, 0.0f64); pieces.len()];
    let survey = survey_violations(&geometries, &zero, &contracts, settings);
    diagnostics.violating_pairs_before = survey.pairs.len();
    diagnostics.boundary_pieces_before = survey.boundary_pieces.len();
    diagnostics.material_pairs_before = survey.material_pairs;
    diagnostics.collision_pairs_before = survey.collision_pairs;
    diagnostics.max_material_deficit_mm = survey.max_material_deficit;
    diagnostics.max_envelope_push_mm = survey.max_envelope_push;
    diagnostics.max_boundary_deficit_mm = survey.max_boundary_deficit;

    let component_limit = micro_legalization_component_limit(pieces.len());
    diagnostics.component_limit = component_limit;
    if survey.pairs.is_empty() && survey.boundary_pieces.is_empty() {
        // Nothing to repair. Report the input's own validity so a caller can
        // distinguish "already legal" from "repaired".
        diagnostics.resolved = true;
        match validate_and_measure_placements(pieces, placements, settings) {
            Ok(metrics) => {
                diagnostics.exact_valid = true;
                diagnostics.depth_mm = Some(metrics.used_long_axis_depth_mm);
                return (diagnostics, Some(placements.to_vec()));
            }
            Err(error) => {
                // The survey found no violation the model can see, yet the
                // authoritative validator disagrees: report it rather than
                // silently claiming success.
                diagnostics.rejection_reason = Some(error.to_string());
                return (diagnostics, None);
            }
        }
    }

    let (components, seeds) = violation_components(&survey, pieces.len());
    diagnostics.component_count = components.len();
    diagnostics.largest_component_pieces = components.iter().map(Vec::len).max().unwrap_or(0);
    diagnostics.seed_pieces = seeds.iter().filter(|seed| **seed).count();
    if diagnostics.largest_component_pieces > component_limit {
        diagnostics.skipped_reason = Some(format!(
            "violation component spans {} pieces, above the micro-legalization limit of {component_limit}",
            diagnostics.largest_component_pieces
        ));
        return (diagnostics, None);
    }

    // Admission: this pass is only meant for rounding-scale residue. Sizing the
    // bound against both the contract and the involved pieces keeps it
    // meaningful whether or not the request asks for a real clearance.
    let smallest_extent = seeds
        .iter()
        .enumerate()
        .filter(|(_, seed)| **seed)
        .map(|(index, _)| geometries[index].material.bounds.min_extent())
        .fold(f64::INFINITY, f64::min);
    let deficit_scale = survey.deficit_scale();
    let admissible_deficit_mm = (contracts.material_pair_mm * MICRO_LEGALIZATION_MAX_DEFICIT_RATIO)
        .max(smallest_extent * MICRO_LEGALIZATION_MAX_DEFICIT_EXTENT_RATIO);
    diagnostics.admissible_deficit_mm = admissible_deficit_mm;
    if deficit_scale > admissible_deficit_mm {
        diagnostics.skipped_reason = Some(format!(
            "largest deficit {deficit_scale:.6} mm exceeds the micro-legalization admission bound of {admissible_deficit_mm:.6} mm"
        ));
        return (diagnostics, None);
    }

    diagnostics.attempted = true;
    let displacement_cap_mm = (deficit_scale * MICRO_LEGALIZATION_CAP_FACTOR)
        .max(contracts.material_pair_mm * MICRO_LEGALIZATION_MIN_CAP_RATIO)
        .max(MICRO_LEGALIZATION_MIN_CAP_MM);
    diagnostics.displacement_cap_mm = displacement_cap_mm;

    // A seed wedged between the sheet edge and an immovable neighbour has
    // nowhere to go: the residue is resolvable, but not by moving the guilty
    // piece alone. Letting the seeds' immediate neighbours give way turns those
    // states from refusals into repairs, and stays honest about "do not touch
    // anything outside the affected component" because the expansion is exactly
    // one hop and is itself capped.
    let movable = expand_movable_neighbourhood(
        &geometries,
        &seeds,
        displacement_cap_mm,
        component_limit.saturating_mul(MICRO_LEGALIZATION_NEIGHBOURHOOD_LIMIT_FACTOR),
    );
    diagnostics.movable_pieces = movable.iter().filter(|movable| **movable).count();

    for escalation in 0..=MICRO_LEGALIZATION_ESCALATIONS {
        let margin_mm = MICRO_LEGALIZATION_MARGIN_MM * (escalation + 1) as f64;
        diagnostics.material_target_mm = contracts.material_pair_mm + margin_mm;
        diagnostics.collision_target_mm = margin_mm;
        diagnostics.escalations_run = escalation;

        let solution = solve(
            &geometries,
            &movable,
            &survey,
            &contracts,
            margin_mm,
            displacement_cap_mm,
            settings,
        );
        diagnostics.pair_constraints = solution.pair_constraints;
        diagnostics.rounds_run = solution.rounds;
        diagnostics.displacement_capped = solution.capped;
        diagnostics.max_displacement_mm = Some(
            solution
                .translations
                .iter()
                .map(|(dx, dy)| dx.hypot(*dy))
                .fold(0.0f64, f64::max),
        );
        diagnostics.moved_pieces = solution
            .translations
            .iter()
            .filter(|(dx, dy)| *dx != 0.0 || *dy != 0.0)
            .count();

        let after = survey_violations(&geometries, &solution.translations, &contracts, settings);
        diagnostics.violating_pairs_after = after.pairs.len();
        diagnostics.boundary_pieces_after = after.boundary_pieces.len();
        diagnostics.resolved =
            solution.converged && after.pairs.is_empty() && after.boundary_pieces.is_empty();
        if !diagnostics.resolved {
            // A larger margin cannot rescue a state the solver could not drive
            // to its own fixpoint; escalate only when the geometry closed and
            // the authoritative gate is the thing still in doubt.
            diagnostics.rejection_reason = Some(
                "micro-legalization did not reach a feasible fixpoint within its round budget"
                    .to_owned(),
            );
            return (diagnostics, None);
        }

        let repaired = apply_translations(placements, &solution.translations);
        match validate_and_measure_placements(pieces, &repaired, settings) {
            Ok(metrics) => {
                diagnostics.exact_valid = true;
                diagnostics.depth_mm = Some(metrics.used_long_axis_depth_mm);
                diagnostics.rejection_reason = None;
                return (diagnostics, Some(repaired));
            }
            Err(error) => {
                diagnostics.rejection_reason = Some(error.to_string());
            }
        }
    }
    (diagnostics, None)
}

struct Survey {
    /// Violating pairs as `(first, second)`, sorted and deduplicated across
    /// gates.
    pairs: Vec<(usize, usize)>,
    /// Pieces violating either sheet containment check, sorted.
    boundary_pieces: Vec<usize>,
    material_pairs: usize,
    collision_pairs: usize,
    max_material_deficit: Option<f64>,
    max_envelope_push: Option<f64>,
    max_boundary_deficit: Option<f64>,
}

impl Survey {
    /// The largest travel any single piece has to make to clear the residue.
    fn deficit_scale(&self) -> f64 {
        self.max_material_deficit
            .unwrap_or(0.0)
            .max(self.max_envelope_push.unwrap_or(0.0))
            .max(self.max_boundary_deficit.unwrap_or(0.0))
    }
}

/// Measures every pair and every boundary in both gates, against the bare
/// contracts.
fn survey_violations(
    geometries: &[PieceGeometry],
    translations: &[(f64, f64)],
    contracts: &Contracts,
    settings: GeneralFastSettings,
) -> Survey {
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    let mut material_pairs = 0usize;
    let mut collision_pairs = 0usize;
    let mut max_material_deficit: Option<f64> = None;
    let mut max_envelope_push: Option<f64> = None;

    for first in 0..geometries.len() {
        for second in (first + 1)..geometries.len() {
            let mut violating = false;
            // Material gate: a shortfall against the requested clearance.
            let material_contract = contracts.material_pair_mm;
            let first_bounds = geometries[first]
                .material
                .bounds
                .translated(translations[first].0, translations[first].1);
            let second_bounds = geometries[second]
                .material
                .bounds
                .translated(translations[second].0, translations[second].1);
            if first_bounds.gap(second_bounds) <= material_contract {
                let approach = measure_approach(
                    &geometries[first].material,
                    translations[first],
                    &geometries[second].material,
                    translations[second],
                    material_contract.max(GRID_MM),
                );
                if approach.distance < material_contract {
                    violating = true;
                    material_pairs += 1;
                    let deficit = material_contract - approach.distance;
                    max_material_deficit = Some(
                        max_material_deficit.map_or(deficit, |current: f64| current.max(deficit)),
                    );
                }
            }
            // Envelope gate: the gate's own predicate, no reimplementation.
            let first_envelope = geometries[first]
                .collision
                .bounds
                .translated(translations[first].0, translations[first].1);
            let second_envelope = geometries[second]
                .collision
                .bounds
                .translated(translations[second].0, translations[second].1);
            if first_envelope.gap(second_envelope) <= 0.0
                && envelopes_overlap(
                    geometries,
                    first,
                    second,
                    translations[first],
                    translations[second],
                )
            {
                violating = true;
                collision_pairs += 1;
                // Overlap carries no magnitude of its own, so measure the
                // travel that ends it. Both endpoints of a violating pair are
                // movable, so each of them only has to cover half of it.
                let push = 0.5
                    * separation_push(
                        geometries,
                        first,
                        second,
                        translations,
                        displacement_ceiling(contracts),
                    );
                max_envelope_push =
                    Some(max_envelope_push.map_or(push, |current: f64| current.max(push)));
            }
            if violating {
                pairs.push((first, second));
            }
        }
    }

    let mut boundary_pieces = Vec::new();
    let mut max_boundary_deficit: Option<f64> = None;
    for (index, geometry) in geometries.iter().enumerate() {
        let mut deficit = 0.0f64;
        for gate in GATES {
            let bounds = geometry
                .outline(gate)
                .outer_bounds
                .translated(translations[index].0, translations[index].1);
            deficit = deficit.max(boundary_deficit(bounds, contracts.edge(gate), settings));
        }
        if deficit > 0.0 {
            boundary_pieces.push(index);
            max_boundary_deficit =
                Some(max_boundary_deficit.map_or(deficit, |current: f64| current.max(deficit)));
        }
    }

    Survey {
        pairs,
        boundary_pieces,
        material_pairs,
        collision_pairs,
        max_material_deficit,
        max_envelope_push,
        max_boundary_deficit,
    }
}

/// The furthest a single projection step is ever allowed to reach for. Sized
/// from the clearance contract so it is scale-free, and only ever used as a
/// bisection ceiling and an admission backstop.
fn displacement_ceiling(contracts: &Contracts) -> f64 {
    contracts
        .material_pair_mm
        .max(MICRO_LEGALIZATION_MIN_CAP_MM)
        * 2.0
}

/// How far the two collision envelopes have to be driven apart along the
/// material witness direction before they stop interpenetrating.
///
/// A zero distance carries no gradient, so the magnitude of an envelope
/// violation cannot be read off a measurement the way a clearance shortfall
/// can. Bisecting the separating translation recovers it exactly to the grid
/// quantum, which is what lets the pass size its displacement budget honestly
/// and refuse residues that are really search problems. Returns `ceiling` when
/// the pair does not come apart within it.
fn separation_push(
    geometries: &[PieceGeometry],
    first: usize,
    second: usize,
    translations: &[(f64, f64)],
    ceiling: f64,
) -> f64 {
    let direction = separation_direction(geometries, first, second, translations);
    let separated = |push: f64| -> bool {
        let moved = (
            snap_grid(translations[first].0 + direction.0 * push),
            snap_grid(translations[first].1 + direction.1 * push),
        );
        !envelopes_overlap(geometries, first, second, moved, translations[second])
    };
    if separated(0.0) {
        return 0.0;
    }
    if !separated(ceiling) {
        return ceiling;
    }
    let mut low = 0.0;
    let mut high = ceiling;
    // A fixed iteration count keeps the cost and the result deterministic.
    // Halving a ceiling of a few millimetres 24 times lands far below the grid
    // quantum, so the answer is exact at the resolution anything downstream
    // can represent.
    for _ in 0..24 {
        let middle = 0.5 * (low + high);
        if separated(middle) {
            high = middle;
        } else {
            low = middle;
        }
    }
    high
}

/// The direction the first piece of a pair should travel to come apart: the
/// material witness where the source outlines are still separated, and the
/// centroid axis where even that has collapsed.
fn separation_direction(
    geometries: &[PieceGeometry],
    first: usize,
    second: usize,
    translations: &[(f64, f64)],
) -> (f64, f64) {
    let material = measure_approach(
        &geometries[first].material,
        translations[first],
        &geometries[second].material,
        translations[second],
        f64::INFINITY,
    );
    material.direction.unwrap_or_else(|| {
        let dx = (geometries[first].material.centroid.x + translations[first].0)
            - (geometries[second].material.centroid.x + translations[second].0);
        let dy = (geometries[first].material.centroid.y + translations[first].1)
            - (geometries[second].material.centroid.y + translations[second].1);
        normalize(dx, dy).unwrap_or((1.0, 0.0))
    })
}

/// The worst of the four sheet-edge overruns, or `0.0` when contained.
fn boundary_deficit(bounds: Bounds, edge_mm: f64, settings: GeneralFastSettings) -> f64 {
    (edge_mm - bounds.min_x)
        .max(edge_mm - bounds.min_y)
        .max(bounds.max_x - (settings.sheet_short_axis_mm - edge_mm))
        .max(bounds.max_y - (settings.sheet_long_axis_mm - edge_mm))
        .max(0.0)
}

/// Groups violations into connected components and derives the movable set.
fn violation_components(survey: &Survey, piece_count: usize) -> (Vec<Vec<usize>>, Vec<bool>) {
    fn find(parent: &mut [usize], index: usize) -> usize {
        let mut root = index;
        while parent[root] != root {
            root = parent[root];
        }
        let mut walk = index;
        while parent[walk] != root {
            let next = parent[walk];
            parent[walk] = root;
            walk = next;
        }
        root
    }

    let mut parent = (0..piece_count).collect::<Vec<_>>();
    let mut movable = vec![false; piece_count];
    for (first, second) in &survey.pairs {
        movable[*first] = true;
        movable[*second] = true;
        let first_root = find(&mut parent, *first);
        let second_root = find(&mut parent, *second);
        if first_root != second_root {
            // Union by index keeps the structure a deterministic function of
            // the violation list alone.
            parent[first_root.max(second_root)] = first_root.min(second_root);
        }
    }
    for index in &survey.boundary_pieces {
        movable[*index] = true;
    }

    let mut components: Vec<Vec<usize>> = Vec::new();
    let mut roots: Vec<(usize, usize)> = Vec::new();
    for index in 0..piece_count {
        if !movable[index] {
            continue;
        }
        let root = find(&mut parent, index);
        match roots.iter().find(|(candidate, _)| *candidate == root) {
            Some((_, slot)) => components[*slot].push(index),
            None => {
                roots.push((root, components.len()));
                components.push(vec![index]);
            }
        }
    }
    (components, movable)
}

/// Grows the movable set from the violation seeds by one hop, bounded.
///
/// A neighbour is any piece whose collision envelope lies within the guard band
/// of a seed's, i.e. exactly the pieces a seed could collide with if it used its
/// whole displacement budget. Nothing further away can be reached, so nothing
/// further away is unfrozen. The expansion stops at `limit` pieces, preferring
/// lower indices so the result is a deterministic function of the layout.
fn expand_movable_neighbourhood(
    geometries: &[PieceGeometry],
    seeds: &[bool],
    displacement_cap_mm: f64,
    limit: usize,
) -> Vec<bool> {
    let mut movable = seeds.to_vec();
    let guard_mm = 2.0 * displacement_cap_mm;
    let mut budget = limit.saturating_sub(seeds.iter().filter(|seed| **seed).count());
    for candidate in 0..geometries.len() {
        if budget == 0 {
            break;
        }
        if movable[candidate] {
            continue;
        }
        let adjacent = seeds.iter().enumerate().any(|(seed, is_seed)| {
            *is_seed
                && geometries[seed]
                    .collision
                    .bounds
                    .gap(geometries[candidate].collision.bounds)
                    < guard_mm
        });
        if adjacent {
            movable[candidate] = true;
            budget -= 1;
        }
    }
    movable
}

struct Solution {
    translations: Vec<(f64, f64)>,
    rounds: usize,
    converged: bool,
    capped: bool,
    pair_constraints: usize,
}

/// One pair constraint: a gate, the two pieces, and the separation they must
/// reach.
struct PairConstraint {
    first: usize,
    second: usize,
    gate: Gate,
    target_mm: f64,
}

/// Projected Gauss-Seidel over the pair and boundary constraints of both gates.
fn solve(
    geometries: &[PieceGeometry],
    movable: &[bool],
    survey: &Survey,
    contracts: &Contracts,
    margin_mm: f64,
    displacement_cap_mm: f64,
    settings: GeneralFastSettings,
) -> Solution {
    let mut translations = vec![(0.0f64, 0.0f64); geometries.len()];

    // The constraint system. Two kinds of pair constraint, and the distinction
    // matters as much as the solver does:
    //
    // * *repair* constraints, on pairs that violate a gate on the input. These
    //   are driven to the contract plus the margin, so the repaired pair ends
    //   with daylight rather than balanced on the boundary.
    // * *protect* constraints, on every other pair inside the guard band.
    //   These are held at the bare contract. A tightly packed layout has many
    //   pairs sitting exactly on it - envelopes in contact are legal and
    //   commonplace - and asking those for the margin too would drag the whole
    //   neighbourhood along behind every repair.
    //
    // The guard band is the target plus twice the displacement cap, so a pair
    // outside it cannot be brought into violation by any admissible
    // displacement and needs no constraint at all.
    let violating = survey.pairs.iter().copied().collect::<BTreeSet<_>>();
    let mut constraints: Vec<PairConstraint> = Vec::new();
    for first in 0..geometries.len() {
        for second in (first + 1)..geometries.len() {
            if !movable[first] && !movable[second] {
                continue;
            }
            let repair = violating.contains(&(first, second));
            for gate in GATES {
                let contract_mm = contracts.pair(gate);
                let target_mm = if repair && gate == Gate::Material {
                    contract_mm + margin_mm
                } else {
                    contract_mm
                };
                let guard_mm = target_mm + 2.0 * displacement_cap_mm;
                let first_outline = geometries[first].outline(gate);
                let second_outline = geometries[second].outline(gate);
                if first_outline.bounds.gap(second_outline.bounds) >= guard_mm {
                    continue;
                }
                constraints.push(PairConstraint {
                    first,
                    second,
                    gate,
                    target_mm,
                });
            }
        }
    }
    let pair_constraints = constraints.len();

    let boundary_indices = (0..geometries.len())
        .filter(|index| movable[*index])
        .collect::<Vec<_>>();
    let ceiling_mm = displacement_ceiling(contracts);

    let mut capped = false;
    let mut converged = false;
    let mut rounds = 0;
    for _ in 0..MICRO_LEGALIZATION_ROUNDS {
        rounds += 1;
        let mut corrected = false;

        for constraint in constraints.iter() {
            let PairConstraint {
                first,
                second,
                gate,
                target_mm,
            } = *constraint;
            let (need, direction) = match gate {
                // The envelope gate is a bare non-overlap, asked of the gate's
                // own predicate. It carries no magnitude, so when it fires the
                // separating travel is measured directly rather than guessed
                // at, and the margin is added on top so the repaired pair ends
                // with daylight instead of balanced on the boundary. A pair
                // that is merely *touching* is legal and is left alone, which
                // is what keeps a tightly packed neighbourhood still.
                Gate::Collision => {
                    if !envelopes_overlap(
                        geometries,
                        first,
                        second,
                        translations[first],
                        translations[second],
                    ) {
                        continue;
                    }
                    let push =
                        separation_push(geometries, first, second, &translations, ceiling_mm);
                    (
                        push + margin_mm,
                        separation_direction(geometries, first, second, &translations),
                    )
                }
                Gate::Material => {
                    let approach = measure_approach(
                        &geometries[first].material,
                        translations[first],
                        &geometries[second].material,
                        translations[second],
                        target_mm.max(GRID_MM),
                    );
                    if approach.distance >= target_mm {
                        continue;
                    }
                    let need = target_mm - approach.distance;
                    if need <= MICRO_LEGALIZATION_SNAP_SLACK_MM {
                        // Inside the snap dead band: correcting here would only
                        // fight the grid rounding that put it there.
                        continue;
                    }
                    match approach.direction {
                        Some(direction) => (need, direction),
                        None => (
                            need.max(GRID_MM),
                            separation_direction(geometries, first, second, &translations),
                        ),
                    }
                }
            };
            let (share_first, share_second) = match (movable[first], movable[second]) {
                (true, true) => (need * 0.5, need * 0.5),
                (true, false) => (need, 0.0),
                (false, true) => (0.0, need),
                // Guard-band pairs always have a movable endpoint by
                // construction; this arm is unreachable but must not panic.
                (false, false) => (0.0, 0.0),
            };
            if share_first > 0.0 {
                translations[first].0 += direction.0 * share_first;
                translations[first].1 += direction.1 * share_first;
                corrected = true;
            }
            if share_second > 0.0 {
                translations[second].0 -= direction.0 * share_second;
                translations[second].1 -= direction.1 * share_second;
                corrected = true;
            }
        }

        for index in boundary_indices.iter().copied() {
            // Same repair/protect split as the pair constraints: a piece that
            // already sits inside the sheet is held there, not dragged inward.
            let repair = survey.boundary_pieces.contains(&index);
            for gate in GATES {
                let edge_mm = contracts.edge(gate) + if repair { margin_mm } else { 0.0 };
                let bounds = geometries[index]
                    .outline(gate)
                    .outer_bounds
                    .translated(translations[index].0, translations[index].1);
                let left = edge_mm - bounds.min_x;
                let bottom = edge_mm - bounds.min_y;
                let right = bounds.max_x - (settings.sheet_short_axis_mm - edge_mm);
                let top = bounds.max_y - (settings.sheet_long_axis_mm - edge_mm);
                if left > MICRO_LEGALIZATION_SNAP_SLACK_MM {
                    translations[index].0 += left;
                    corrected = true;
                } else if right > MICRO_LEGALIZATION_SNAP_SLACK_MM {
                    translations[index].0 -= right;
                    corrected = true;
                }
                if bottom > MICRO_LEGALIZATION_SNAP_SLACK_MM {
                    translations[index].1 += bottom;
                    corrected = true;
                } else if top > MICRO_LEGALIZATION_SNAP_SLACK_MM {
                    translations[index].1 -= top;
                    corrected = true;
                }
            }
        }

        // Clamp to the cap, then snap to the canonical grid, so the state the
        // next round measures is exactly the state a publication would carry.
        for (index, translation) in translations.iter_mut().enumerate() {
            if !movable[index] {
                *translation = (0.0, 0.0);
                continue;
            }
            let length = translation.0.hypot(translation.1);
            if length > displacement_cap_mm && length > 0.0 {
                let scale = displacement_cap_mm / length;
                translation.0 *= scale;
                translation.1 *= scale;
                capped = true;
            }
            translation.0 = snap_grid(translation.0);
            translation.1 = snap_grid(translation.1);
        }

        if !corrected {
            converged = true;
            break;
        }
    }

    Solution {
        translations,
        rounds,
        converged,
        capped,
        pair_constraints,
    }
}

fn snap_grid(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    (value / GRID_MM).round() * GRID_MM
}

fn normalize(dx: f64, dy: f64) -> Option<(f64, f64)> {
    let length = dx.hypot(dy);
    if !length.is_finite() || length <= 0.0 {
        return None;
    }
    Some((dx / length, dy / length))
}

fn apply_translations(
    placements: &[GeneralFastPlacement],
    translations: &[(f64, f64)],
) -> Vec<GeneralFastPlacement> {
    placements
        .iter()
        .zip(translations)
        .map(|(placement, (dx, dy))| GeneralFastPlacement {
            piece_id: placement.piece_id.clone(),
            rotation_deg: placement.rotation_deg,
            mirrored: placement.mirrored,
            translate_short_axis: placement.translate_short_axis + dx,
            translate_long_axis: placement.translate_long_axis + dy,
        })
        .collect()
}

/// Builds a piece's geometry in both gates through exactly the calls the
/// engine's own validator and collision builder make, so the pass optimizes
/// the geometry that will be measured rather than a model of it.
fn build_geometry(
    polygon: &PolygonSet,
    placement: &GeneralFastPlacement,
    expansion_mm: f64,
) -> Option<PieceGeometry> {
    if !placement.rotation_deg.is_finite()
        || !placement.translate_short_axis.is_finite()
        || !placement.translate_long_axis.is_finite()
    {
        return None;
    }
    let transformed = polygon
        .transformed(
            placement.rotation_deg,
            placement.mirrored,
            placement.translate_short_axis,
            placement.translate_long_axis,
        )
        .ok()?;
    let collision = transformed.offset(expansion_mm).ok()?;
    Some(PieceGeometry {
        material: build_outline(&transformed)?,
        collision: build_outline(&collision)?,
        collision_shape: collision,
    })
}

/// Asks the canonical-grid gate itself whether two envelopes overlap at the
/// given repair translations.
///
/// Translations are snapped to the canonical grid before they reach here, and a
/// grid-aligned translation commutes exactly with the grid quantization, so
/// translating the prebuilt envelope is identical to rebuilding it at the moved
/// pose - without paying for another offset.
fn envelopes_overlap(
    geometries: &[PieceGeometry],
    first: usize,
    second: usize,
    first_translation: (f64, f64),
    second_translation: (f64, f64),
) -> bool {
    let Some(first_shape) = translated_shape(&geometries[first].collision_shape, first_translation)
    else {
        return true;
    };
    let Some(second_shape) =
        translated_shape(&geometries[second].collision_shape, second_translation)
    else {
        return true;
    };
    // A geometry failure is reported as an overlap: the pass must never talk
    // itself into publishing a state it could not measure.
    polygons_overlap_exact(first_shape.as_ref(), second_shape.as_ref()).unwrap_or(true)
}

fn translated_shape(shape: &PolygonSet, translation: (f64, f64)) -> Option<Cow<'_, PolygonSet>> {
    if translation.0 == 0.0 && translation.1 == 0.0 {
        return Some(Cow::Borrowed(shape));
    }
    shape
        .transformed(0.0, false, translation.0, translation.1)
        .ok()
        .map(Cow::Owned)
}

/// Reads a world-space polygon's rings into the measurement representation.
fn build_outline(polygon: &PolygonSet) -> Option<Outline> {
    let mut rings: Vec<Vec<IrregularPoint>> = Vec::new();
    let mut bounds = Bounds::empty();
    let mut outer_bounds = Bounds::empty();
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut count = 0usize;

    for region in &polygon.regions {
        let outer = region.outer.source_points().to_vec();
        for point in &outer {
            if !point.x.is_finite() || !point.y.is_finite() {
                return None;
            }
            bounds.observe(*point);
            outer_bounds.observe(*point);
            sum_x += point.x;
            sum_y += point.y;
            count += 1;
        }
        rings.push(outer);
        for hole in &region.holes {
            let hole = hole.source_points().to_vec();
            for point in &hole {
                if !point.x.is_finite() || !point.y.is_finite() {
                    return None;
                }
                bounds.observe(*point);
            }
            rings.push(hole);
        }
    }
    if rings.is_empty() || count == 0 {
        return None;
    }
    let ring_bounds = rings
        .iter()
        .map(|ring| {
            let mut ring_bounds = Bounds::empty();
            for point in ring {
                ring_bounds.observe(*point);
            }
            ring_bounds
        })
        .collect();
    Some(Outline {
        rings,
        ring_bounds,
        bounds,
        outer_bounds,
        centroid: IrregularPoint::new(sum_x / count as f64, sum_y / count as f64),
    })
}

/// Minimum boundary distance between two translated outlines, together with
/// the witness direction.
///
/// Mirrors `validation::general_polygon::minimum_boundary_distance` (all rings,
/// all segment pairs, zero on touch or cross) and adds two things it does not
/// need: a witness direction for the projection, and bound-based pruning
/// against a running minimum. Pruning is exact - a segment pair whose bounding
/// boxes are further apart than the running minimum can neither improve it nor
/// intersect - so whenever the result is below `ceiling` it is identical to the
/// validator's measurement.
fn measure_approach(
    first: &Outline,
    first_translation: (f64, f64),
    second: &Outline,
    second_translation: (f64, f64),
    ceiling: f64,
) -> Approach {
    // Measure in the first outline's own frame by folding the pair's relative
    // translation onto the second: exact, and it avoids rebuilding point
    // vectors every round.
    let dx = second_translation.0 - first_translation.0;
    let dy = second_translation.1 - first_translation.1;
    let mut best = ceiling;
    let mut witness: Option<(IrregularPoint, IrregularPoint)> = None;
    let mut touching = false;

    'outer: for (first_ring_index, first_ring) in first.rings.iter().enumerate() {
        let first_bounds = first.ring_bounds[first_ring_index];
        for (second_ring_index, second_ring) in second.rings.iter().enumerate() {
            let second_bounds = second.ring_bounds[second_ring_index].translated(dx, dy);
            // Pruning is only sound while the running minimum is positive. Once
            // the outlines have been found to touch it drops to zero, every gap
            // trivially clears it, and the scan has to run to completion to
            // learn whether the contact is also a crossing.
            if best > 0.0 && first_bounds.gap(second_bounds) >= best {
                continue;
            }
            for first_index in 0..first_ring.len() {
                let first_start = first_ring[first_index];
                let first_end = first_ring[(first_index + 1) % first_ring.len()];
                let first_segment_bounds = segment_bounds(first_start, first_end);
                if best > 0.0 && first_segment_bounds.gap(second_bounds) >= best {
                    continue;
                }
                for second_index in 0..second_ring.len() {
                    let second_start = shift(second_ring[second_index], dx, dy);
                    let second_end =
                        shift(second_ring[(second_index + 1) % second_ring.len()], dx, dy);
                    let other_bounds = segment_bounds(second_start, second_end);
                    if best > 0.0 && first_segment_bounds.gap(other_bounds) >= best {
                        continue;
                    }
                    if bounds_intersect(first_segment_bounds, other_bounds)
                        && segments_touch_or_cross(first_start, first_end, second_start, second_end)
                    {
                        touching = true;
                        break 'outer;
                    }
                    if let Some((distance, from, to)) =
                        segment_witness(first_start, first_end, second_start, second_end, best)
                    {
                        best = distance;
                        witness = Some((from, to));
                    }
                }
            }
        }
    }

    if touching {
        return Approach {
            distance: 0.0,
            direction: None,
        };
    }
    let direction = witness.and_then(|(from, to)| normalize(from.x - to.x, from.y - to.y));
    Approach {
        distance: best,
        direction,
    }
}

/// Whether two segments meet at all. Same predicate arithmetic as
/// `validation::general_polygon::segments_touch_or_cross`, so the distance this
/// module measures agrees with the one the publication validator measures.
fn segments_touch_or_cross(
    first_start: IrregularPoint,
    first_end: IrregularPoint,
    second_start: IrregularPoint,
    second_end: IrregularPoint,
) -> bool {
    let first_side_start = orientation(
        first_start.x,
        first_start.y,
        first_end.x,
        first_end.y,
        second_start.x,
        second_start.y,
    );
    let first_side_end = orientation(
        first_start.x,
        first_start.y,
        first_end.x,
        first_end.y,
        second_end.x,
        second_end.y,
    );
    let second_side_start = orientation(
        second_start.x,
        second_start.y,
        second_end.x,
        second_end.y,
        first_start.x,
        first_start.y,
    );
    let second_side_end = orientation(
        second_start.x,
        second_start.y,
        second_end.x,
        second_end.y,
        first_end.x,
        first_end.y,
    );
    if first_side_start * first_side_end < 0 && second_side_start * second_side_end < 0 {
        return true;
    }
    (first_side_start == 0 && point_on_segment(second_start, first_start, first_end))
        || (first_side_end == 0 && point_on_segment(second_end, first_start, first_end))
        || (second_side_start == 0 && point_on_segment(first_start, second_start, second_end))
        || (second_side_end == 0 && point_on_segment(first_end, second_start, second_end))
}

fn shift(point: IrregularPoint, dx: f64, dy: f64) -> IrregularPoint {
    IrregularPoint::new(point.x + dx, point.y + dy)
}

fn segment_bounds(start: IrregularPoint, end: IrregularPoint) -> Bounds {
    Bounds {
        min_x: start.x.min(end.x),
        min_y: start.y.min(end.y),
        max_x: start.x.max(end.x),
        max_y: start.y.max(end.y),
    }
}

fn bounds_intersect(first: Bounds, second: Bounds) -> bool {
    first.min_x <= second.max_x
        && second.min_x <= first.max_x
        && first.min_y <= second.max_y
        && second.min_y <= first.max_y
}

/// The four point-segment candidates, returning the witness pair when the
/// distance improves on `best`.
fn segment_witness(
    first_start: IrregularPoint,
    first_end: IrregularPoint,
    second_start: IrregularPoint,
    second_end: IrregularPoint,
    best: f64,
) -> Option<(f64, IrregularPoint, IrregularPoint)> {
    let mut result: Option<(f64, IrregularPoint, IrregularPoint)> = None;
    let mut current = best;
    for (point, start, end, point_is_first) in [
        (first_start, second_start, second_end, true),
        (first_end, second_start, second_end, true),
        (second_start, first_start, first_end, false),
        (second_end, first_start, first_end, false),
    ] {
        let (distance, closest) = point_segment_witness(point, start, end);
        if distance < current {
            current = distance;
            result = Some(if point_is_first {
                (distance, point, closest)
            } else {
                (distance, closest, point)
            });
        }
    }
    result
}

fn point_segment_witness(
    point: IrregularPoint,
    start: IrregularPoint,
    end: IrregularPoint,
) -> (f64, IrregularPoint) {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx * dx + dy * dy;
    if length_squared == 0.0 {
        return ((point.x - start.x).hypot(point.y - start.y), start);
    }
    let projection =
        (((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared).clamp(0.0, 1.0);
    let closest = IrregularPoint::new(start.x + projection * dx, start.y + projection * dy);
    ((point.x - closest.x).hypot(point.y - closest.y), closest)
}

fn point_on_segment(point: IrregularPoint, start: IrregularPoint, end: IrregularPoint) -> bool {
    point.x >= start.x.min(end.x)
        && point.x <= start.x.max(end.x)
        && point.y >= start.y.min(end.y)
        && point.y <= start.y.max(end.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An axis-aligned rectangle piece, in source coordinates.
    fn rectangle(width_mm: f64, height_mm: f64) -> PolygonSet {
        PolygonSet::from_outer(vec![
            IrregularPoint::new(0.0, 0.0),
            IrregularPoint::new(width_mm, 0.0),
            IrregularPoint::new(width_mm, height_mm),
            IrregularPoint::new(0.0, height_mm),
        ])
        .expect("a rectangle is a valid polygon")
    }

    fn placement(id: &str, x_mm: f64, y_mm: f64) -> GeneralFastPlacement {
        GeneralFastPlacement {
            piece_id: id.to_owned(),
            rotation_deg: 0.0,
            mirrored: false,
            translate_short_axis: x_mm,
            translate_long_axis: y_mm,
        }
    }

    fn piece<'a>(id: &'a str, polygon: &'a PolygonSet) -> GeneralFastPiece<'a> {
        GeneralFastPiece {
            id,
            polygon,
            allow_rotation: false,
            allow_mirror: false,
        }
    }

    /// A settings block with a real clearance contract, matching how the
    /// benchmark driver configures the engine for the exact-clearance fixture.
    fn settings() -> GeneralFastSettings {
        sheet_settings(200.0, 300.0)
    }

    fn sheet_settings(short_axis_mm: f64, long_axis_mm: f64) -> GeneralFastSettings {
        let mut settings = GeneralFastSettings::deterministic_test(short_axis_mm, long_axis_mm);
        settings.total_padding_mm = 5.0;
        settings.sheet_edge_clearance_mm = Some(5.0);
        settings.clearance_safety_margin_mm = 0.0;
        settings.flattening_sag_tolerance_mm = 0.0;
        settings.search_offset_allowance_mm = 0.0005;
        settings
    }

    #[test]
    fn resolvable_pair_deficit_is_repaired_and_validates() {
        let square = rectangle(20.0, 20.0);
        let pieces = vec![piece("a", &square), piece("b", &square)];
        // 4.99 mm apart under a 5 mm contract: a 0.01 mm deficit, exactly the
        // residue class this pass exists for.
        let placements = vec![placement("a", 20.0, 20.0), placement("b", 44.99, 20.0)];
        let settings = settings();
        assert!(validate_and_measure_placements(&pieces, &placements, settings).is_err());

        let (diagnostics, repaired) = micro_legalize(&pieces, &placements, settings);
        assert!(diagnostics.attempted, "diagnostics: {diagnostics:?}");
        assert_eq!(diagnostics.violating_pairs_before, 1);
        assert_eq!(diagnostics.violating_pairs_after, 0);
        assert!(diagnostics.resolved);
        assert!(diagnostics.exact_valid, "diagnostics: {diagnostics:?}");
        let repaired = repaired.expect("a resolvable deficit publishes a repaired state");
        validate_and_measure_placements(&pieces, &repaired, settings)
            .expect("the repaired state validates against the real request");
        for (before, after) in placements.iter().zip(&repaired) {
            let dx = after.translate_short_axis - before.translate_short_axis;
            let dy = after.translate_long_axis - before.translate_long_axis;
            assert!(dx.hypot(dy) <= diagnostics.displacement_cap_mm + 1e-9);
        }
    }

    #[test]
    fn a_diagonal_corner_deficit_invisible_to_the_material_contract_is_repaired() {
        // The two squares are 7.05 mm apart in material - comfortably past the
        // 5 mm contract - yet their miter-joined envelopes still overlap. This
        // is the case a material-only model gets wrong.
        let square = rectangle(20.0, 20.0);
        let pieces = vec![piece("a", &square), piece("b", &square)];
        let placements = vec![placement("a", 20.0, 20.0), placement("b", 44.99, 44.985)];
        let settings = settings();
        assert!(validate_and_measure_placements(&pieces, &placements, settings).is_err());

        let (diagnostics, repaired) = micro_legalize(&pieces, &placements, settings);
        assert_eq!(diagnostics.material_pairs_before, 0);
        assert_eq!(diagnostics.collision_pairs_before, 1);
        assert!(diagnostics.exact_valid, "diagnostics: {diagnostics:?}");
        let repaired = repaired.expect("an envelope-only deficit is still repairable");
        validate_and_measure_placements(&pieces, &repaired, settings)
            .expect("the repaired state validates");
    }

    #[test]
    fn a_legal_state_is_reported_legal_and_returned_unchanged() {
        let square = rectangle(20.0, 20.0);
        let pieces = vec![piece("a", &square), piece("b", &square)];
        let placements = vec![placement("a", 20.0, 20.0), placement("b", 46.0, 20.0)];
        let settings = settings();
        let (diagnostics, repaired) = micro_legalize(&pieces, &placements, settings);
        assert_eq!(diagnostics.violating_pairs_before, 0);
        assert_eq!(diagnostics.boundary_pieces_before, 0);
        assert!(diagnostics.resolved);
        assert!(diagnostics.exact_valid);
        assert_eq!(repaired.as_deref(), Some(placements.as_slice()));
    }

    #[test]
    fn boundary_violation_is_pushed_back_inside_the_sheet() {
        let square = rectangle(20.0, 20.0);
        let pieces = vec![piece("a", &square)];
        // Just inside the 5 mm edge clearance on the left and bottom, which the
        // wider collision inset then pushes over.
        let placements = vec![placement("a", 4.999, 4.999)];
        let settings = settings();
        assert!(validate_and_measure_placements(&pieces, &placements, settings).is_err());

        let (diagnostics, repaired) = micro_legalize(&pieces, &placements, settings);
        assert_eq!(diagnostics.boundary_pieces_before, 1);
        assert_eq!(diagnostics.boundary_pieces_after, 0);
        assert!(diagnostics.exact_valid, "diagnostics: {diagnostics:?}");
        let repaired = repaired.expect("a boundary deficit is resolvable by translation");
        validate_and_measure_placements(&pieces, &repaired, settings)
            .expect("the repaired state validates");
        assert!(repaired[0].translate_short_axis > placements[0].translate_short_axis);
        assert!(repaired[0].translate_long_axis > placements[0].translate_long_axis);
    }

    #[test]
    fn a_deep_overlap_is_refused_at_admission_rather_than_nudged() {
        let square = rectangle(20.0, 20.0);
        let pieces = vec![piece("a", &square), piece("b", &square)];
        // A 15 mm interpenetration: not a rounding-scale residue.
        let placements = vec![placement("a", 20.0, 20.0), placement("b", 25.0, 20.0)];
        let settings = settings();
        let (diagnostics, repaired) = micro_legalize(&pieces, &placements, settings);
        assert!(repaired.is_none(), "an unresolvable state never publishes");
        assert!(!diagnostics.attempted);
        assert!(!diagnostics.exact_valid);
        assert!(diagnostics.violating_pairs_before > 0);
        assert!(
            diagnostics
                .skipped_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("admission bound")),
            "diagnostics: {diagnostics:?}"
        );
    }

    #[test]
    fn a_contradictory_residue_is_reported_cleanly_without_publishing() {
        // A 20 mm piece on a 29.9 mm sheet needs 5 mm of clearance on both
        // sides plus its own envelope: the two boundary constraints cannot be
        // satisfied at once, so the solver must fail loudly rather than
        // publish.
        let square = rectangle(20.0, 20.0);
        let pieces = vec![piece("a", &square)];
        let placements = vec![placement("a", 5.0, 20.0)];
        let settings = sheet_settings(29.9, 300.0);
        let (diagnostics, repaired) = micro_legalize(&pieces, &placements, settings);
        assert!(repaired.is_none(), "an unresolvable state never publishes");
        assert!(!diagnostics.exact_valid);
        assert!(
            diagnostics.rejection_reason.is_some() || diagnostics.skipped_reason.is_some(),
            "an unresolvable state reports why: {diagnostics:?}"
        );
    }

    #[test]
    fn an_oversized_violation_component_is_skipped_rather_than_relaxed() {
        // Nine pieces in a chain, every consecutive pair violating: the
        // component spans the whole layout, which is a search problem rather
        // than a micro repair.
        let square = rectangle(20.0, 20.0);
        let ids = ["a", "b", "c", "d", "e", "f", "g", "h", "i"];
        let pieces = ids.iter().map(|id| piece(id, &square)).collect::<Vec<_>>();
        let placements = ids
            .iter()
            .enumerate()
            .map(|(index, id)| placement(id, 20.0 + 24.99 * index as f64, 20.0))
            .collect::<Vec<_>>();
        let settings = sheet_settings(400.0, 300.0);
        let (diagnostics, repaired) = micro_legalize(&pieces, &placements, settings);
        assert!(repaired.is_none());
        assert!(!diagnostics.attempted);
        assert!(
            diagnostics.largest_component_pieces > diagnostics.component_limit,
            "diagnostics: {diagnostics:?}"
        );
        assert!(diagnostics
            .skipped_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("above the micro-legalization limit")));
    }

    #[test]
    fn the_pass_is_deterministic() {
        let square = rectangle(20.0, 20.0);
        let pieces = vec![
            piece("a", &square),
            piece("b", &square),
            piece("c", &square),
        ];
        let placements = vec![
            placement("a", 20.0, 20.0),
            placement("b", 44.99, 20.0),
            placement("c", 20.0, 44.985),
        ];
        let settings = settings();
        let (first_diagnostics, first) = micro_legalize(&pieces, &placements, settings);
        let (second_diagnostics, second) = micro_legalize(&pieces, &placements, settings);
        assert_eq!(first_diagnostics, second_diagnostics);
        assert_eq!(first, second);
        assert!(first.is_some(), "diagnostics: {first_diagnostics:?}");
    }

    #[test]
    fn pieces_outside_the_guard_band_never_move() {
        let square = rectangle(20.0, 20.0);
        let ids = ["a", "b", "c"];
        let pieces = ids.iter().map(|id| piece(id, &square)).collect::<Vec<_>>();
        // The first two violate; the third is far away and must be untouched.
        let placements = vec![
            placement("a", 20.0, 20.0),
            placement("b", 44.99, 20.0),
            placement("c", 140.0, 20.0),
        ];
        let settings = settings();
        let (diagnostics, repaired) = micro_legalize(&pieces, &placements, settings);
        let repaired = repaired.expect("the local deficit is resolvable");
        assert_eq!(diagnostics.movable_pieces, 2);
        assert_eq!(repaired[2], placements[2]);
    }

    #[test]
    fn the_component_limit_scales_with_the_piece_count() {
        assert_eq!(micro_legalization_component_limit(0), 4);
        assert_eq!(micro_legalization_component_limit(20), 4);
        assert_eq!(micro_legalization_component_limit(61), 7);
        assert_eq!(micro_legalization_component_limit(400), 50);
    }

    #[test]
    fn the_measured_distance_matches_the_geometry_it_repairs() {
        // The pass optimizes its own distance measurement, so that measurement
        // has to agree with the geometry the authoritative validator sees.
        // Sweep a pair across the contract boundary and check both the value
        // and the witness direction.
        let square = rectangle(20.0, 20.0);
        let pieces = vec![piece("a", &square), piece("b", &square)];
        let expansion = collision_expansion_mm(settings());
        for step in 0..40 {
            let gap = 4.98 + 0.001 * step as f64;
            let placements = vec![placement("a", 20.0, 20.0), placement("b", 40.0 + gap, 20.0)];
            let geometries = placements
                .iter()
                .zip(&pieces)
                .map(|(placement, piece)| {
                    build_geometry(piece.polygon, placement, expansion).expect("geometry")
                })
                .collect::<Vec<_>>();
            let approach = measure_approach(
                &geometries[0].material,
                (0.0, 0.0),
                &geometries[1].material,
                (0.0, 0.0),
                10.0,
            );
            assert!(
                (approach.distance - gap).abs() < 1e-9,
                "measured {} for a {gap} gap",
                approach.distance
            );
            let direction = approach.direction.expect("a separated pair has a witness");
            assert!(
                direction.0 < -0.99,
                "expected a leftward push, got {direction:?}"
            );
        }
    }
}
