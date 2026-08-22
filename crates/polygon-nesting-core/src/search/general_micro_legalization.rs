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
//!
//! # The global big brother
//!
//! Everything above is deliberately *local*: only pieces incident to a
//! violation move, the residue must be rounding-scale, and a component
//! spanning more than an eighth of the layout is refused outright. That is the
//! right instrument for a projection problem and the wrong one for a
//! **redistribution** problem, where the correction a violating pair needs is
//! millimetres and the room to make it exists only three pieces away.
//!
//! [`global_legalize`] is the same geometry under a different bound: every
//! piece is a variable, sheet containment and the depth bound are hard
//! constraints on all of them, and the step of each round is the *minimum-norm*
//! correction satisfying the whole linearized system at once rather than a
//! sequence of pairwise pushes. It shares this module's witnesses, contracts,
//! envelope predicate and grid discipline exactly, so the two passes always
//! agree on what a violation is; they disagree only on how much of the layout
//! is allowed to answer for it. See [`global_legalize`] for the model and the
//! solver.

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

/// The SE(2) rigidity certificate: a diagnostic that asks how much shallower
/// this module's own contact front can be made by a bounded SE(2) motion.
///
/// A child module rather than a sibling so it can reuse this one's private
/// geometry (`Outline`, `build_outline`, `measure_approach`, `Contracts`)
/// without any of it becoming crate-visible, and gated so that a default build
/// contains none of it. It never runs inside a request: the only caller is a
/// dedicated CLI path in `general_request_benchmark`. See
/// `docs/experiments/se2-rigidity/`.
#[cfg(feature = "se2-rigidity-certificate")]
pub mod se2_certificate;

/// The active-contact block SE(2) operator: Sol review 10 §3's new search
/// action.
///
/// A sibling of [`se2_certificate`] and a child of this module for the same
/// reason that one is — it reuses this module's private geometry and that
/// module's `Geometry`, `apply_se2` and exact closest-approach witness without
/// any of it becoming crate-visible. Stacked on the certificate's feature
/// because the witness pair it linearizes contacts at is behind that gate. See
/// `docs/experiments/contact-block/`.
#[cfg(feature = "contact-block-se2")]
pub mod contact_block;

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

/// Outer re-linearization rounds of [`global_legalize`], per margin
/// escalation. Every round re-measures the exact geometry, rebuilds the
/// constraint system from that measurement, and solves it once.
const GLOBAL_LEGALIZATION_ROUNDS: usize = 24;

/// Dual coordinate sweeps of Hildreth's method inside one round. The dual is
/// one variable per constraint and each sweep is a single pass over them, so
/// this is the only iteration count the quadratic program has.
const GLOBAL_LEGALIZATION_DUAL_SWEEPS: usize = 512;

/// How many times [`global_legalize`] re-runs with an enlarged margin when the
/// geometry closes but the authoritative validator still rejects. Each
/// escalation adds another [`MICRO_LEGALIZATION_MARGIN_MM`] and continues from
/// the translations already accumulated.
const GLOBAL_LEGALIZATION_ESCALATIONS: usize = 3;

/// Per-round trust radius, as a multiple of that round's own worst deficit.
///
/// The separation constraints are linearized around the current poses, so the
/// model is only faithful within a neighbourhood of the residue it was measured
/// from: past that the witness normal has rotated and the linear inequality is
/// extrapolation. Bounding each round's step by the residue's own scale is what
/// keeps the sequence inside the model, and it is also what makes the guard
/// band sound - a pair further apart than the target plus twice this radius
/// provably cannot be brought into contact by one round's step.
const GLOBAL_LEGALIZATION_TRUST_FACTOR: f64 = 1.0;

/// Cumulative per-piece displacement cap, as a multiple of the *initial* worst
/// deficit. Generous by design: this pass exists precisely for corrections a
/// bounded local nudge cannot express, and every intermediate state is
/// re-measured exactly rather than trusted. It is a runaway guard, not a
/// tuning knob.
const GLOBAL_LEGALIZATION_CAP_FACTOR: f64 = 16.0;

/// Exact pair probes one round may charge for a single pair of pieces.
///
/// A round measures each pair at most twice - once in the survey and once
/// while building its rows - and each measurement costs one material approach,
/// one envelope predicate and, when the envelopes interpenetrate, the 24-step
/// bisection that recovers the separating travel. Two such measurements
/// therefore top out at `2 * (1 + 1 + 26) = 56`; the ceiling carries headroom
/// over that so a future measurement cannot silently exceed the funded budget
/// instead of stopping on it.
const GLOBAL_LEGALIZATION_PAIR_PROBES_PER_ROUND: usize = 64;

/// The worst-case number of exact pair probes one [`global_legalize`] call may
/// charge on an instance of `piece_count` pieces.
///
/// Asserted against the aggregate experimental pair-visit ceiling in
/// `aggregate_quota_formulas_match_the_reviewed_contract`'s sibling,
/// `bounded_reinsertion_fits_the_construction_budget`, so the global tier is
/// funded by the terms already reviewed rather than by a new one.
pub(crate) fn global_legalization_worst_case_pair_visits(piece_count: usize) -> usize {
    let complete_pairs = piece_count
        .saturating_mul(piece_count.saturating_sub(1))
        .saturating_div(2);
    (GLOBAL_LEGALIZATION_ESCALATIONS + 1)
        .saturating_mul(GLOBAL_LEGALIZATION_ROUNDS)
        .saturating_mul(complete_pairs)
        .saturating_mul(GLOBAL_LEGALIZATION_PAIR_PROBES_PER_ROUND)
}

/// The worst-case number of collision-envelope builds one [`global_legalize`]
/// call may charge: the pass builds one envelope per piece per margin
/// escalation and never rebuilds one inside a round, because a grid-snapped
/// translation commutes with both the transform and the offset.
pub(crate) fn global_legalization_worst_case_collision_builds(piece_count: usize) -> usize {
    (GLOBAL_LEGALIZATION_ESCALATIONS + 1).saturating_mul(piece_count)
}

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

/// The largest ejection set the conflict-targeted re-placement repair will
/// attempt, in pieces.
///
/// Deliberately *the same* threshold as [`micro_legalization_component_limit`]
/// rather than a new tuned literal. The ejection set is a vertex cover of the
/// violation graph, so it is never larger than the graph's own components; a
/// residue whose cover exceeds the size a micro-repair would already have
/// refused as "a search problem, not a projection" is a search problem for the
/// re-placement repair too, and gets refused on the same terms.
pub(crate) fn replacement_ejection_limit(piece_count: usize) -> usize {
    micro_legalization_component_limit(piece_count)
}

/// One violating pair of a surveyed layout, with the travel that would clear
/// it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ViolationPair {
    /// Indices into the surveyed placement list, `first < second`.
    pub first: usize,
    pub second: usize,
    /// The pair's *violation mass*: the larger of its clearance shortfall
    /// against the material contract and the travel needed to pull its
    /// collision envelopes apart. The two are routinely orders of magnitude
    /// apart, so taking the larger is what keeps a millimetre-scale envelope
    /// conflict from reading as a rounding-scale one.
    pub mass_mm: f64,
    /// The unit direction `first` must travel to come apart from `second`:
    /// the material closest-approach witness, falling back to the centroid
    /// axis once the outlines have interpenetrated far enough to lose it.
    /// `second` comes apart along its negation.
    ///
    /// This is the pair's own escape geometry, and it is what a repair that
    /// re-places a piece has to aim along: in a record-density layout the
    /// feasible set around a conflicting pose is a sliver, and a displacement
    /// cloud that only samples the axes and diagonals misses it.
    pub separation_direction: (f64, f64),
}

/// The violation graph of a layout, measured against the bare publication
/// contracts on the geometry each gate actually looks at.
///
/// This is exactly the survey [`micro_legalize`] runs before it decides
/// whether to attempt a projection, exposed so a *different* repair - one that
/// re-places pieces instead of translating them - can be targeted at the same
/// residue without re-deriving the measurement.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct LayoutViolations {
    pub pairs: Vec<ViolationPair>,
    /// Pieces violating either sheet containment check, sorted.
    pub boundary_pieces: Vec<usize>,
    pub material_pairs: usize,
    pub collision_pairs: usize,
    pub max_material_deficit_mm: Option<f64>,
    pub max_envelope_push_mm: Option<f64>,
    pub max_boundary_deficit_mm: Option<f64>,
    /// Connected components over the violating pairs, with boundary-only
    /// pieces contributing singleton components.
    pub components: Vec<Vec<usize>>,
}

impl LayoutViolations {
    pub(crate) fn largest_component_pieces(&self) -> usize {
        self.components.iter().map(Vec::len).max().unwrap_or(0)
    }

    /// Total violation mass incident to each surveyed slot.
    pub(crate) fn incident_mass(&self, slots: usize) -> Vec<f64> {
        let mut mass = vec![0.0f64; slots];
        for pair in &self.pairs {
            mass[pair.first] += pair.mass_mm;
            mass[pair.second] += pair.mass_mm;
        }
        mass
    }

    /// The slots of every component that carries a violating *pair*, each
    /// component sorted and deduplicated, in `components` order.
    ///
    /// A repair that ejects a vertex cover of the violation graph leaves the
    /// other endpoint of every pair exactly where it was, so the re-placed
    /// piece has to find room against an occupancy that is itself part of the
    /// conflict. Ejecting a whole component instead vacates both sides at once,
    /// which is the only way a *coordinated* move - two pieces trading pockets,
    /// say - can be expressed at all. Components are returned separately rather
    /// than unioned: independent conflicts are independent repairs, and pooling
    /// them made a set of small clusters refuse on an ejection cap that none of
    /// them individually trips.
    ///
    /// Boundary-only slots contribute singleton components to `components`;
    /// those are a projection problem, not a re-placement one, so they are not
    /// included here unless the same component also carries a pair.
    pub(crate) fn pair_components(&self) -> Vec<Vec<usize>> {
        let mut incident = BTreeSet::new();
        for pair in &self.pairs {
            incident.insert(pair.first);
            incident.insert(pair.second);
        }
        self.components
            .iter()
            .filter(|component| component.iter().any(|slot| incident.contains(slot)))
            .map(|component| {
                let mut slots = component.clone();
                slots.sort_unstable();
                slots.dedup();
                slots
            })
            .collect()
    }
}

/// Surveys `placements` against the bare publication contracts and returns the
/// violation graph.
///
/// `placements` may be a strict sub-layout of `pieces`; indices in the result
/// are into `placements`, not into `pieces`.
pub(crate) fn survey_layout_violations(
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
    settings: GeneralFastSettings,
) -> Result<LayoutViolations, String> {
    if placements.len() > pieces.len() {
        return Err("violation survey requires a sub-layout of the request".to_owned());
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
        return Err("violation survey requires a finite clearance contract".to_owned());
    }
    let pieces_by_id = pieces
        .iter()
        .enumerate()
        .map(|(index, piece)| (piece.id, index))
        .collect::<BTreeMap<_, _>>();
    let geometries = placements
        .iter()
        .map(|placement| {
            let piece = pieces_by_id
                .get(placement.piece_id.as_str())
                .map(|index| pieces[*index])
                .ok_or_else(|| format!("unknown piece {}", placement.piece_id))?;
            build_geometry(piece.polygon, placement, expansion_mm).ok_or_else(|| {
                format!(
                    "could not build an envelope for piece {}",
                    placement.piece_id
                )
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let zero = vec![(0.0f64, 0.0f64); placements.len()];
    let survey = survey_violations(&geometries, &zero, &contracts, settings);
    let (components, _) = violation_components(&survey, placements.len());
    Ok(LayoutViolations {
        pairs: survey
            .pairs
            .iter()
            .zip(&survey.pair_mass)
            .map(|((first, second), mass_mm)| ViolationPair {
                first: *first,
                second: *second,
                mass_mm: *mass_mm,
                separation_direction: separation_direction(&geometries, *first, *second, &zero),
            })
            .collect(),
        boundary_pieces: survey.boundary_pieces.clone(),
        material_pairs: survey.material_pairs,
        collision_pairs: survey.collision_pairs,
        max_material_deficit_mm: survey.max_material_deficit,
        max_envelope_push_mm: survey.max_envelope_push,
        max_boundary_deficit_mm: survey.max_boundary_deficit,
        components,
    })
}

/// Projects one slot of `placements` clear of everything it violates, with the
/// rest of the layout held exactly where it is.
///
/// This is the micro-legalizer's projection restricted to a single movable
/// piece, and it exists for a repair that *re-places* rather than translates:
/// a candidate generator working from a piece's vacated pose knows the pose is
/// in conflict but not which way out, and at record density the feasible set
/// around that pose is a sliver that a displacement cloud on the axes and
/// diagonals reliably misses. The projection is the one displacement that is
/// derived from the conflict's own geometry rather than sampled near it.
///
/// It differs from [`micro_legalize`] in what it is for, and so in what it
/// promises: nothing is published, nothing is admitted, no margin escalation
/// is run, and no residue class is refused. It returns a *displacement to try*.
/// The caller confirms the resulting pose through its own machinery or throws
/// it away - which is why the round budget is the only thing bounding it.
///
/// Returns the projection's *trajectory* - the distinct accumulated
/// translations it passed through, on the canonical grid, in round order and
/// capped at `max_iterates` - together with whether it reached a fixpoint
/// inside [`MICRO_LEGALIZATION_ROUNDS`].
///
/// The trajectory rather than the endpoint, because a single movable piece
/// wedged between several neighbours makes Gauss-Seidel oscillate: clearing
/// one pair walks into the next and back again, so the iteration can pass
/// straight through a feasible pose without stopping on one. The caller
/// confirms poses anyway, so handing it the whole trajectory costs a bounded
/// number of charged rows and lets its own gate decide - which is strictly
/// better than picking a favourite iterate here on a criterion this function
/// cannot check. An empty trajectory means the slot was already clear.
pub(crate) fn separating_translation(
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
    settings: GeneralFastSettings,
    slot: usize,
    max_iterates: usize,
) -> Result<(Vec<(f64, f64)>, bool), String> {
    if slot >= placements.len() {
        return Err("separating translation requires a surveyed slot".to_owned());
    }
    if placements.len() > pieces.len() {
        return Err("separating translation requires a sub-layout of the request".to_owned());
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
        return Err("separating translation requires a finite clearance contract".to_owned());
    }
    let pieces_by_id = pieces
        .iter()
        .enumerate()
        .map(|(index, piece)| (piece.id, index))
        .collect::<BTreeMap<_, _>>();
    let geometries = placements
        .iter()
        .map(|placement| {
            let piece = pieces_by_id
                .get(placement.piece_id.as_str())
                .map(|index| pieces[*index])
                .ok_or_else(|| format!("unknown piece {}", placement.piece_id))?;
            build_geometry(piece.polygon, placement, expansion_mm).ok_or_else(|| {
                format!(
                    "could not build an envelope for piece {}",
                    placement.piece_id
                )
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    // Only `slot` moves, so it absorbs the whole separating travel of each
    // constraint rather than the half a two-movable pair would share.
    let mut translations = vec![(0.0f64, 0.0f64); placements.len()];
    let ceiling_mm = displacement_ceiling(&contracts);
    let margin_mm = MICRO_LEGALIZATION_MARGIN_MM;
    let mut converged = false;
    let mut trajectory: Vec<(f64, f64)> = Vec::new();
    for _ in 0..MICRO_LEGALIZATION_ROUNDS {
        if trajectory.len() >= max_iterates {
            break;
        }
        let mut corrected = false;
        for other in 0..placements.len() {
            if other == slot {
                continue;
            }
            for gate in GATES {
                let target_mm = contracts.pair(gate) + margin_mm;
                let (need, direction) = match gate {
                    Gate::Collision => {
                        if !envelopes_overlap(
                            &geometries,
                            slot,
                            other,
                            translations[slot],
                            translations[other],
                        ) {
                            continue;
                        }
                        let push =
                            separation_push(&geometries, slot, other, &translations, ceiling_mm);
                        (
                            push + margin_mm,
                            separation_direction(&geometries, slot, other, &translations),
                        )
                    }
                    Gate::Material => {
                        let approach = measure_approach(
                            &geometries[slot].material,
                            translations[slot],
                            &geometries[other].material,
                            translations[other],
                            target_mm.max(GRID_MM),
                        );
                        if approach.distance >= target_mm {
                            continue;
                        }
                        let need = target_mm - approach.distance;
                        if need <= MICRO_LEGALIZATION_SNAP_SLACK_MM {
                            continue;
                        }
                        match approach.direction {
                            Some(direction) => (need, direction),
                            None => (
                                need.max(GRID_MM),
                                separation_direction(&geometries, slot, other, &translations),
                            ),
                        }
                    }
                };
                if need > 0.0 {
                    translations[slot].0 += direction.0 * need;
                    translations[slot].1 += direction.1 * need;
                    corrected = true;
                }
            }
        }
        // Containment, against the sheet the caller handed in - which for a
        // bounded re-placement is the clamped one, so the projection cannot
        // walk the piece out past the bound it is being re-placed under.
        for gate in GATES {
            let edge_mm = contracts.edge(gate) + margin_mm;
            let bounds = geometries[slot]
                .outline(gate)
                .outer_bounds
                .translated(translations[slot].0, translations[slot].1);
            let left = edge_mm - bounds.min_x;
            let bottom = edge_mm - bounds.min_y;
            let right = bounds.max_x - (settings.sheet_short_axis_mm - edge_mm);
            let top = bounds.max_y - (settings.sheet_long_axis_mm - edge_mm);
            if left > MICRO_LEGALIZATION_SNAP_SLACK_MM {
                translations[slot].0 += left;
                corrected = true;
            } else if right > MICRO_LEGALIZATION_SNAP_SLACK_MM {
                translations[slot].0 -= right;
                corrected = true;
            }
            if bottom > MICRO_LEGALIZATION_SNAP_SLACK_MM {
                translations[slot].1 += bottom;
                corrected = true;
            } else if top > MICRO_LEGALIZATION_SNAP_SLACK_MM {
                translations[slot].1 -= top;
                corrected = true;
            }
        }
        translations[slot].0 = snap_grid(translations[slot].0);
        translations[slot].1 = snap_grid(translations[slot].1);
        let iterate = translations[slot];
        if grid_key_pair(iterate) != (0, 0)
            && !trajectory
                .iter()
                .any(|kept| grid_key_pair(*kept) == grid_key_pair(iterate))
        {
            trajectory.push(iterate);
        }
        if !corrected {
            converged = true;
            break;
        }
    }
    Ok((trajectory, converged))
}

/// A translation's identity on the canonical grid.
fn grid_key_pair(translation: (f64, f64)) -> (i64, i64) {
    (
        (translation.0 / GRID_MM).round() as i64,
        (translation.1 / GRID_MM).round() as i64,
    )
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
    /// The closest-approach point pair itself, `(on the first, on the second)`,
    /// in the *first* outline's frame — which is the frame `measure_approach`
    /// measures in, so the second point carries the pair's relative
    /// translation and a caller that wants it in the second outline's own
    /// frame must subtract that shift back off.
    ///
    /// Compiled out entirely without the certificate feature. It is only ever
    /// read to build a rotational coefficient, which no production path has,
    /// and `global_legalize` runs this function on every pair of every round —
    /// so carrying two extra points through it, and through the `Copy` this
    /// struct relies on, would be a cost on the shipping path bought for a
    /// diagnostic. Sol review 6, §3 called that out on the previous branch,
    /// where the field was unconditional.
    ///
    /// `Some` **including at a touch**. A contact at distance zero is exactly
    /// where a rotational coefficient matters most — those are the rows that
    /// are actually holding the front — and returning `None` there would give
    /// every active contact a zero rotational coefficient, which reads as "no
    /// rotation can open this pair" when the truth is "nobody measured".
    #[cfg(feature = "se2-rigidity-certificate")]
    witness: Option<(IrregularPoint, IrregularPoint)>,
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
    // A *sub*-layout is admissible: the conflict-targeted re-placement repair
    // removes pieces before it asks for the remainder to be cleaned up, and
    // every gate the pass models is measured per placement, not per request.
    // Anything longer than the request cannot be a layout of it at all.
    if placements.len() > pieces.len() {
        diagnostics.skipped_reason =
            Some("micro-legalization requires a sub-layout of the request".to_owned());
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
    // Slot-indexed, i.e. indexed by *placement*, which equals the piece count
    // for every complete layout and is smaller for a sub-layout.
    let zero = vec![(0.0f64, 0.0f64); placements.len()];
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

    let (components, seeds) = violation_components(&survey, placements.len());
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
    /// Per-pair violation mass, parallel to `pairs`: the larger of the pair's
    /// clearance shortfall and its envelope separating travel. The solver does
    /// not need it - it re-measures every constraint anyway - but a repair
    /// that has to *choose between* pieces does.
    pair_mass: Vec<f64>,
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
    let mut pair_mass: Vec<f64> = Vec::new();
    let mut material_pairs = 0usize;
    let mut collision_pairs = 0usize;
    let mut max_material_deficit: Option<f64> = None;
    let mut max_envelope_push: Option<f64> = None;

    for first in 0..geometries.len() {
        for second in (first + 1)..geometries.len() {
            let mut violating = false;
            let mut mass = 0.0f64;
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
                    mass = mass.max(deficit);
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
                mass = mass.max(push);
                max_envelope_push =
                    Some(max_envelope_push.map_or(push, |current: f64| current.max(push)));
            }
            if violating {
                pairs.push((first, second));
                pair_mass.push(mass);
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
        pair_mass,
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
                        // Keep the contact itself as the witness rather than
                        // dropping it. `segment_witness` with no ceiling is the
                        // closest endpoint-to-segment pair of the two touching
                        // segments, which at a touch is the contact point (its
                        // distance is zero whenever an endpoint lies on the
                        // other segment, which is what a touch between two
                        // grid-snapped outlines is). For a *proper* crossing —
                        // an overlap, not a contact — it is the nearest
                        // endpoint pair instead, which is an approximation; the
                        // certificate that reads this field says so, and never
                        // treats a linearized row as an exact statement about
                        // the geometry.
                        #[cfg(feature = "se2-rigidity-certificate")]
                        {
                            witness = segment_witness(
                                first_start,
                                first_end,
                                second_start,
                                second_end,
                                f64::INFINITY,
                            )
                            .map(|(_distance, from, to)| (from, to))
                            .or(witness);
                        }
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
            #[cfg(feature = "se2-rigidity-certificate")]
            witness,
        };
    }
    let direction = witness.and_then(|(from, to)| normalize(from.x - to.x, from.y - to.y));
    Approach {
        distance: best,
        direction,
        #[cfg(feature = "se2-rigidity-certificate")]
        witness,
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

// ---------------------------------------------------------------------------
// Global pressure-balanced legalization
// ---------------------------------------------------------------------------

/// Diagnostics for one global pressure-balanced legalization attempt.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralGlobalLegalizationDiagnostics {
    pub attempted: bool,
    /// The depth bound the containment constraints were built against, when one
    /// was requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bound_mm: Option<f64>,
    /// The sheet long axis the pass actually solved under: the bound when one
    /// was requested and it is tighter than the request's own sheet, and the
    /// request's sheet otherwise.
    pub effective_long_axis_mm: f64,
    /// Every piece is a variable, so this is the layout's own piece count and
    /// `2 * piece_count` scalar unknowns.
    pub piece_count: usize,
    pub variables: usize,
    /// Violations of the input state, measured against the bare contracts under
    /// the *effective* sheet - so `boundaryPiecesBefore` counts pieces past the
    /// depth bound as well as pieces past the real sheet.
    pub violating_pairs_before: usize,
    pub boundary_pieces_before: usize,
    pub material_pairs_before: usize,
    pub collision_pairs_before: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_material_deficit_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_envelope_push_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_boundary_deficit_mm: Option<f64>,
    /// The violation graph of the input, reported but *not* used to admit or
    /// refuse: this pass has no component limit, which is the whole point of it.
    pub component_count: usize,
    pub largest_component_pieces: usize,
    /// The cumulative per-piece displacement cap, and the trust radius of the
    /// first round.
    pub displacement_cap_mm: f64,
    pub initial_trust_radius_mm: f64,
    pub rounds_run: usize,
    pub escalations_run: usize,
    /// Dual sweeps summed over every round, and the widest constraint residual
    /// the last solved round still carried when its sweeps ran out. A residual
    /// at or above the grid quantum means the quadratic program itself did not
    /// close, which is a different failure from a program that closed onto a
    /// state the geometry then disagreed with.
    pub dual_sweeps_run: usize,
    pub max_dual_residual_mm: f64,
    /// The largest constraint system built, split by kind. Pair rows include
    /// the guard-band rows that protect currently legal pairs.
    pub max_rows: usize,
    pub max_pair_rows: usize,
    pub max_boundary_rows: usize,
    /// Exact pair probes actually charged, against the ceiling the aggregate
    /// quota test funds, and the envelope builds this run cost against theirs.
    pub pair_visits: usize,
    pub funded_pair_visits: usize,
    pub collision_builds: usize,
    pub funded_collision_builds: usize,
    pub moved_pieces: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_displacement_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_displacement_mm: Option<f64>,
    pub displacement_capped: bool,
    /// Violations of the output state, on the same terms as the input's.
    pub violating_pairs_after: usize,
    pub boundary_pieces_after: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_material_deficit_after_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_envelope_push_after_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_boundary_deficit_after_mm: Option<f64>,
    /// Whether the exact geometry reached a feasible fixpoint.
    pub resolved: bool,
    /// Whether the authoritative validator accepted the result against the real
    /// request.
    pub exact_valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cap_exhausted: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
}

/// One row of the global program: a linear inequality `a . t >= rhs` over the
/// stacked translation vector `t = (dx_0, dy_0, ..., dx_n, dy_n)`.
#[derive(Clone, Copy, Debug)]
enum GlobalRow {
    /// A separation constraint, `normal . (t_first - t_second) >= rhs_mm`. The
    /// normal is a unit vector, so the row's squared norm is exactly two.
    Pair {
        first: usize,
        second: usize,
        normal: (f64, f64),
        rhs_mm: f64,
    },
    /// A containment constraint, `sign * t_piece[axis] >= rhs_mm`, with `axis`
    /// zero for the short axis and one for the long axis. Squared norm one.
    ///
    /// These are *exact*, not linearized: the outer bounds of a translated
    /// outline are the translated outer bounds, so an axis-aligned containment
    /// check is already linear in the translation. The depth bound is the
    /// `axis == 1, sign == -1` row of every piece.
    Axis {
        piece: usize,
        axis: usize,
        sign: f64,
        rhs_mm: f64,
    },
}

impl GlobalRow {
    fn rhs_mm(self) -> f64 {
        match self {
            GlobalRow::Pair { rhs_mm, .. } | GlobalRow::Axis { rhs_mm, .. } => rhs_mm,
        }
    }

    fn norm_squared(self) -> f64 {
        match self {
            GlobalRow::Pair { .. } => 2.0,
            GlobalRow::Axis { .. } => 1.0,
        }
    }

    fn value(self, translations: &[(f64, f64)]) -> f64 {
        match self {
            GlobalRow::Pair {
                first,
                second,
                normal,
                ..
            } => {
                normal.0 * (translations[first].0 - translations[second].0)
                    + normal.1 * (translations[first].1 - translations[second].1)
            }
            GlobalRow::Axis {
                piece, axis, sign, ..
            } => {
                let component = if axis == 0 {
                    translations[piece].0
                } else {
                    translations[piece].1
                };
                sign * component
            }
        }
    }

    fn apply(self, translations: &mut [(f64, f64)], step: f64) {
        match self {
            GlobalRow::Pair {
                first,
                second,
                normal,
                ..
            } => {
                translations[first].0 += step * normal.0;
                translations[first].1 += step * normal.1;
                translations[second].0 -= step * normal.0;
                translations[second].1 -= step * normal.1;
            }
            GlobalRow::Axis {
                piece, axis, sign, ..
            } => {
                if axis == 0 {
                    translations[piece].0 += step * sign;
                } else {
                    translations[piece].1 += step * sign;
                }
            }
        }
    }
}

/// The global pass's own probe ledger.
///
/// Charged per round at the round's analytic worst case rather than per probe,
/// which is what makes the ceiling checkable in a test:
/// [`global_legalization_worst_case_pair_visits`] is exactly this cap summed
/// over every round of every escalation, and
/// `bounded_reinsertion_fits_the_construction_budget` asserts that sum against
/// the aggregate experimental pair-visit quota. An instance whose geometry
/// somehow outran the plan stops on `capExhausted` rather than overrunning it.
struct GlobalLegalizationBudget {
    pair_visits_remaining: usize,
}

impl GlobalLegalizationBudget {
    fn for_piece_count(piece_count: usize) -> Self {
        Self {
            pair_visits_remaining: global_legalization_worst_case_pair_visits(piece_count),
        }
    }

    /// Charges one round over `complete_pairs` pairs.
    fn charge_round(&mut self, complete_pairs: usize) -> Result<(), &'static str> {
        let charge = complete_pairs.saturating_mul(GLOBAL_LEGALIZATION_PAIR_PROBES_PER_ROUND);
        if self.pair_visits_remaining < charge {
            return Err("global legalization pair-probe budget exhausted");
        }
        self.pair_visits_remaining -= charge;
        Ok(())
    }
}

/// Runs the global pressure-balanced legalization pass over `placements`.
///
/// # What it is for
///
/// [`micro_legalize`] freezes everything outside the violation component and
/// refuses residues above a rounding scale, which is correct for a projection
/// problem and useless for the residue a deep compression frontier actually
/// carries: multi-millimetre deficits in components whose own pieces have no
/// in-bound pose, proven by the per-component re-placement beam. Sequential
/// repair cannot answer those because the room to answer them is not inside the
/// component. This pass lets the *whole layout* answer.
///
/// # Constraint model
///
/// Poses are frozen - rotation and mirror never change - and the unknowns are a
/// translation `t_i = (dx_i, dy_i)` for **every** piece, violating or not.
///
/// * **Separation.** For a pair whose outlines are apart, the distance function
///   is differentiable and its gradient is the unit vector along the
///   closest-approach witness, so the requirement `dist(P_i + t_i, P_j + t_j)
///   >= target` linearizes to `n_ij . (t_i - t_j) >= target - dist_ij`. Rows are
///   generated for **every pair within a guard band**, not just violating ones:
///   a violating pair's row has a positive right-hand side and asks for
///   correction, while a legal pair's row has a negative one and *protects* the
///   clearance it already has. The guard band is the target plus twice the
///   round's trust radius, so a pair outside it cannot be brought into contact
///   by any admissible step and needs no row at all.
/// * **Containment and the depth bound.** Four rows per piece per gate,
///   exactly and not by linearization. When a bound is requested the sheet long
///   axis is clamped to it, so "no piece may end deeper than the bound" is a
///   hard constraint of the program on every piece rather than a filter applied
///   afterwards.
/// * **The envelope gate.** An overlapping collision envelope carries no
///   magnitude - the gate is a boolean - so its travel is recovered by
///   bisecting against [`polygons_overlap_exact`] itself, exactly as the local
///   pass does. A pair that has ever overlapped keeps its row for the rest of
///   the run even once it comes apart, which is what stops it oscillating back.
///
/// # Solver
///
/// Each round solves `min ||t||^2 subject to A t >= b` by **Hildreth's method**,
/// which is projected Gauss-Seidel on the dual of that program: one multiplier
/// per row, swept in a fixed order, each updated by its own scaled residual and
/// clipped at zero, with the primal iterate `t = sum_k lambda_k a_k` maintained
/// incrementally. With the multipliers non-negative the fixpoint is the exact
/// minimum-norm point of the polyhedron, which is what makes the answer a
/// *redistribution*: when the piece that must move is blocked, the chain of
/// rows behind it carries the correction outward and pieces that violate
/// nothing move to make room. No sequential repair can express that, because
/// nothing in a sequential repair ever asks a legal piece to move.
///
/// The round then applies the step under a trust radius, snaps to the canonical
/// grid, re-measures the true geometry, regenerates the rows from that
/// measurement, and repeats. The linear model is never propagated - it only
/// ever picks a step.
///
/// # Bounds
///
/// Rounds, dual sweeps and margin escalations are capped; each round's step is
/// capped by the trust radius and the run by a cumulative displacement cap; and
/// the pair probes are charged against a ledger whose ceiling
/// [`global_legalization_worst_case_pair_visits`] is asserted against the
/// aggregate quota. There is deliberately **no admission bound and no component
/// limit**: refusing a large residue is what the local pass is for.
///
/// Like every other pass here it never publishes on its own authority - a
/// layout comes back only after [`validate_and_measure_placements`] accepted it
/// against the real request, which is `settings`, not the clamped sheet.
pub(crate) fn global_legalize(
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
    settings: GeneralFastSettings,
    bound_mm: Option<f64>,
) -> (
    GeneralGlobalLegalizationDiagnostics,
    Option<Vec<GeneralFastPlacement>>,
) {
    let mut diagnostics = GeneralGlobalLegalizationDiagnostics {
        bound_mm,
        effective_long_axis_mm: settings.sheet_long_axis_mm,
        piece_count: placements.len(),
        variables: placements.len().saturating_mul(2),
        ..GeneralGlobalLegalizationDiagnostics::default()
    };
    if placements.len() > pieces.len() {
        diagnostics.skipped_reason =
            Some("global legalization requires a sub-layout of the request".to_owned());
        return (diagnostics, None);
    }
    if pieces.is_empty() || placements.is_empty() {
        diagnostics.skipped_reason =
            Some("global legalization requires at least one placement".to_owned());
        return (diagnostics, None);
    }
    if let Some(bound_mm) = bound_mm {
        if !bound_mm.is_finite() || bound_mm <= 0.0 {
            diagnostics.skipped_reason =
                Some("global legalization depth bound must be positive and finite".to_owned());
            return (diagnostics, None);
        }
    }

    // The sheet the containment rows - and therefore the depth bound - are
    // built against. Clamping is only ever tightening: a bound above the
    // request's own sheet would otherwise legalize states the request rejects.
    let clamped_settings = match bound_mm {
        Some(bound_mm) => GeneralFastSettings {
            sheet_long_axis_mm: bound_mm.min(settings.sheet_long_axis_mm),
            ..settings
        },
        None => settings,
    };
    diagnostics.effective_long_axis_mm = clamped_settings.sheet_long_axis_mm;

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
            Some("global legalization requires a finite clearance contract".to_owned());
        return (diagnostics, None);
    }

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
                Some("global legalization could not build a piece envelope".to_owned());
            return (diagnostics, None);
        }
    };

    let slots = placements.len();
    // One envelope per piece, built once for the whole run: a grid-snapped
    // translation commutes with both the transform and the offset, so every
    // round measures the same geometry at a different offset rather than
    // rebuilding it. The funded ceiling allows one build per piece per
    // escalation, which is the bound a pass that *did* rebuild would need.
    diagnostics.collision_builds = slots;
    diagnostics.funded_collision_builds = global_legalization_worst_case_collision_builds(slots);
    diagnostics.funded_pair_visits = global_legalization_worst_case_pair_visits(slots);
    let complete_pairs = slots.saturating_mul(slots.saturating_sub(1)) / 2;
    let zero = vec![(0.0f64, 0.0f64); slots];
    let before = survey_violations(&geometries, &zero, &contracts, clamped_settings);
    diagnostics.violating_pairs_before = before.pairs.len();
    diagnostics.boundary_pieces_before = before.boundary_pieces.len();
    diagnostics.material_pairs_before = before.material_pairs;
    diagnostics.collision_pairs_before = before.collision_pairs;
    diagnostics.max_material_deficit_mm = before.max_material_deficit;
    diagnostics.max_envelope_push_mm = before.max_envelope_push;
    diagnostics.max_boundary_deficit_mm = before.max_boundary_deficit;
    let (components, _) = violation_components(&before, slots);
    diagnostics.component_count = components.len();
    diagnostics.largest_component_pieces = components.iter().map(Vec::len).max().unwrap_or(0);

    if before.pairs.is_empty() && before.boundary_pieces.is_empty() {
        // Already feasible under the effective sheet. Report the input's own
        // validity rather than claiming a repair.
        diagnostics.resolved = true;
        diagnostics.violating_pairs_after = 0;
        diagnostics.boundary_pieces_after = 0;
        match validate_and_measure_placements(pieces, placements, settings) {
            Ok(metrics) => {
                diagnostics.exact_valid = true;
                diagnostics.depth_mm = Some(metrics.used_long_axis_depth_mm);
                return (diagnostics, Some(placements.to_vec()));
            }
            Err(error) => {
                diagnostics.rejection_reason = Some(error.to_string());
                return (diagnostics, None);
            }
        }
    }

    diagnostics.attempted = true;
    let floor_mm = (contracts.material_pair_mm * MICRO_LEGALIZATION_MIN_CAP_RATIO)
        .max(MICRO_LEGALIZATION_MIN_CAP_MM);
    let initial_deficit_mm = before.deficit_scale();
    let displacement_cap_mm = (initial_deficit_mm * GLOBAL_LEGALIZATION_CAP_FACTOR).max(floor_mm);
    diagnostics.displacement_cap_mm = displacement_cap_mm;
    diagnostics.initial_trust_radius_mm =
        (initial_deficit_mm * GLOBAL_LEGALIZATION_TRUST_FACTOR).max(floor_mm);

    let mut budget = GlobalLegalizationBudget::for_piece_count(slots);
    let mut accumulated = vec![(0.0f64, 0.0f64); slots];
    // Envelope conflicts are boolean, so a pair that stops overlapping loses
    // its magnitude and would lose its row with it. Once seen, always
    // constrained: this is the pass's cutting-plane memory.
    let mut enforced_collision: BTreeSet<(usize, usize)> = BTreeSet::new();
    let mut after = before;
    let mut published: Option<Vec<GeneralFastPlacement>> = None;

    'escalation: for escalation in 0..=GLOBAL_LEGALIZATION_ESCALATIONS {
        let margin_mm = MICRO_LEGALIZATION_MARGIN_MM * (escalation + 1) as f64;
        diagnostics.escalations_run = escalation;
        for _ in 0..GLOBAL_LEGALIZATION_ROUNDS {
            let survey = survey_violations(&geometries, &accumulated, &contracts, clamped_settings);
            let resolved = survey.pairs.is_empty() && survey.boundary_pieces.is_empty();
            after = survey;
            if resolved {
                diagnostics.resolved = true;
                let repaired = apply_translations(placements, &accumulated);
                match validate_and_measure_placements(pieces, &repaired, settings) {
                    Ok(metrics) => {
                        diagnostics.exact_valid = true;
                        diagnostics.depth_mm = Some(metrics.used_long_axis_depth_mm);
                        diagnostics.rejection_reason = None;
                        published = Some(repaired);
                        break 'escalation;
                    }
                    Err(error) => {
                        // The geometry closed and the authoritative gate still
                        // disagrees: a wider margin is the only thing that can
                        // help, so escalate rather than spend more rounds here.
                        diagnostics.rejection_reason = Some(error.to_string());
                        continue 'escalation;
                    }
                }
            }
            if let Err(reason) = budget.charge_round(complete_pairs) {
                diagnostics.cap_exhausted = Some(reason.to_owned());
                break 'escalation;
            }

            let trust_radius_mm = (after.deficit_scale() * GLOBAL_LEGALIZATION_TRUST_FACTOR)
                .max(floor_mm)
                .min(displacement_cap_mm);
            let mut probes = 0usize;
            let (rows, pair_rows, boundary_rows) = build_global_rows(
                &geometries,
                &accumulated,
                &contracts,
                margin_mm,
                trust_radius_mm,
                clamped_settings,
                &mut enforced_collision,
                &mut probes,
            );
            diagnostics.pair_visits = diagnostics.pair_visits.saturating_add(probes);
            diagnostics.max_rows = diagnostics.max_rows.max(rows.len());
            diagnostics.max_pair_rows = diagnostics.max_pair_rows.max(pair_rows);
            diagnostics.max_boundary_rows = diagnostics.max_boundary_rows.max(boundary_rows);
            diagnostics.rounds_run = diagnostics.rounds_run.saturating_add(1);

            let solution = solve_minimum_norm_step(&rows, slots);
            diagnostics.dual_sweeps_run = diagnostics.dual_sweeps_run.saturating_add(solution.1);
            diagnostics.max_dual_residual_mm = solution.2;
            let mut step = solution.0;

            // Trust region. Scaling the whole step uniformly keeps the
            // correction's shape - which is the part the global solve
            // contributed - and only ever undershoots the rows that ask for
            // correction: the rows that merely protect a legal pair are
            // satisfied at zero as well as at the full step, so every point of
            // the segment between them satisfies those too.
            let longest_mm = step
                .iter()
                .map(|(dx, dy)| dx.hypot(*dy))
                .fold(0.0f64, f64::max);
            if longest_mm > trust_radius_mm && longest_mm > 0.0 {
                let scale = trust_radius_mm / longest_mm;
                for translation in step.iter_mut() {
                    translation.0 *= scale;
                    translation.1 *= scale;
                }
            }

            let mut moved = false;
            for (index, translation) in accumulated.iter_mut().enumerate() {
                let next = (
                    snap_grid(translation.0 + step[index].0),
                    snap_grid(translation.1 + step[index].1),
                );
                if grid_key_pair(next) != grid_key_pair(*translation) {
                    moved = true;
                }
                *translation = next;
            }
            let travelled_mm = accumulated
                .iter()
                .map(|(dx, dy)| dx.hypot(*dy))
                .fold(0.0f64, f64::max);
            if travelled_mm > displacement_cap_mm {
                diagnostics.displacement_capped = true;
                break 'escalation;
            }
            if !moved {
                // A fixpoint of the program that is not a fixpoint of the
                // geometry: more rounds at this margin cannot move it, so hand
                // the state to the next escalation instead of spinning.
                continue 'escalation;
            }
        }
    }

    diagnostics.violating_pairs_after = after.pairs.len();
    diagnostics.boundary_pieces_after = after.boundary_pieces.len();
    diagnostics.max_material_deficit_after_mm = after.max_material_deficit;
    diagnostics.max_envelope_push_after_mm = after.max_envelope_push;
    diagnostics.max_boundary_deficit_after_mm = after.max_boundary_deficit;
    diagnostics.moved_pieces = accumulated
        .iter()
        .filter(|(dx, dy)| *dx != 0.0 || *dy != 0.0)
        .count();
    let displacements = accumulated
        .iter()
        .map(|(dx, dy)| dx.hypot(*dy))
        .collect::<Vec<_>>();
    diagnostics.max_displacement_mm = Some(displacements.iter().copied().fold(0.0f64, f64::max));
    diagnostics.mean_displacement_mm =
        Some(displacements.iter().sum::<f64>() / displacements.len() as f64);
    if published.is_none() && diagnostics.rejection_reason.is_none() {
        diagnostics.rejection_reason = Some(if diagnostics.resolved {
            "global legalization closed the geometry but the validator rejected it".to_owned()
        } else {
            format!(
                "global legalization did not reach a feasible fixpoint: {} violating pairs and {} boundary pieces remain",
                after.pairs.len(),
                after.boundary_pieces.len()
            )
        });
    }
    (diagnostics, published)
}

/// Builds the round's constraint system from the exact geometry at
/// `translations`.
///
/// Returns the rows together with the pair-row and boundary-row counts. Rows
/// are emitted in a fixed order - pairs by `(first, second, gate)` and then
/// containment by `(piece, gate, side)` - because the dual sweep is
/// order-dependent and the pass has to be a deterministic function of the
/// layout alone.
#[allow(clippy::too_many_arguments)]
fn build_global_rows(
    geometries: &[PieceGeometry],
    translations: &[(f64, f64)],
    contracts: &Contracts,
    margin_mm: f64,
    trust_radius_mm: f64,
    settings: GeneralFastSettings,
    enforced_collision: &mut BTreeSet<(usize, usize)>,
    probes: &mut usize,
) -> (Vec<GlobalRow>, usize, usize) {
    let mut rows: Vec<GlobalRow> = Vec::new();
    let ceiling_mm = displacement_ceiling(contracts);
    // A pair further apart than its target plus the relative travel two pieces
    // can make in one round cannot reach the target inside this round, so it
    // needs no row.
    let reach_mm = 2.0 * trust_radius_mm;

    for first in 0..geometries.len() {
        for second in (first + 1)..geometries.len() {
            // Material gate. Every pair inside the guard band gets a row: the
            // violating ones ask for the contract plus the margin, and the
            // legal ones hold the bare contract they already have.
            let material_guard_mm = contracts.material_pair_mm + margin_mm + reach_mm;
            let first_bounds = geometries[first]
                .material
                .bounds
                .translated(translations[first].0, translations[first].1);
            let second_bounds = geometries[second]
                .material
                .bounds
                .translated(translations[second].0, translations[second].1);
            if first_bounds.gap(second_bounds) < material_guard_mm {
                *probes += 1;
                let approach = measure_approach(
                    &geometries[first].material,
                    translations[first],
                    &geometries[second].material,
                    translations[second],
                    material_guard_mm,
                );
                if approach.distance < material_guard_mm {
                    let target_mm = if approach.distance < contracts.material_pair_mm {
                        contracts.material_pair_mm + margin_mm
                    } else {
                        contracts.material_pair_mm
                    };
                    let normal = match approach.direction {
                        Some(direction) => direction,
                        None => {
                            *probes += 1;
                            separation_direction(geometries, first, second, translations)
                        }
                    };
                    rows.push(GlobalRow::Pair {
                        first,
                        second,
                        normal,
                        rhs_mm: target_mm - approach.distance,
                    });
                }
            }

            // Envelope gate. Only an actual overlap opens a row - a touching
            // envelope pair is legal to the grid gate and commonplace in a
            // record-density layout - but once opened the row stays for the
            // rest of the run, held at the margin so the pair cannot drift
            // back into contact.
            let overlapping = {
                let first_envelope = geometries[first]
                    .collision
                    .bounds
                    .translated(translations[first].0, translations[first].1);
                let second_envelope = geometries[second]
                    .collision
                    .bounds
                    .translated(translations[second].0, translations[second].1);
                first_envelope.gap(second_envelope) <= 0.0 && {
                    *probes += 1;
                    envelopes_overlap(
                        geometries,
                        first,
                        second,
                        translations[first],
                        translations[second],
                    )
                }
            };
            if overlapping {
                enforced_collision.insert((first, second));
                *probes += 26;
                let push_mm = separation_push(geometries, first, second, translations, ceiling_mm)
                    + margin_mm;
                *probes += 1;
                let normal = separation_direction(geometries, first, second, translations);
                rows.push(GlobalRow::Pair {
                    first,
                    second,
                    normal,
                    rhs_mm: push_mm,
                });
            } else if enforced_collision.contains(&(first, second)) {
                let guard_mm = margin_mm + reach_mm;
                *probes += 1;
                let approach = measure_approach(
                    &geometries[first].collision,
                    translations[first],
                    &geometries[second].collision,
                    translations[second],
                    guard_mm,
                );
                if approach.distance < guard_mm {
                    let normal = match approach.direction {
                        Some(direction) => direction,
                        None => {
                            *probes += 1;
                            separation_direction(geometries, first, second, translations)
                        }
                    };
                    rows.push(GlobalRow::Pair {
                        first,
                        second,
                        normal,
                        rhs_mm: margin_mm - approach.distance,
                    });
                }
            }
        }
    }
    let pair_rows = rows.len();

    // Containment, on every piece and both gates. The `top` row of each piece
    // is the depth bound whenever the caller clamped the sheet to one, and it
    // is a hard constraint of the program exactly like the other three.
    for (index, geometry) in geometries.iter().enumerate() {
        for gate in GATES {
            let edge_mm = contracts.edge(gate);
            let bounds = geometry
                .outline(gate)
                .outer_bounds
                .translated(translations[index].0, translations[index].1);
            // The margin is applied per side, and only to a side that is
            // actually overrun: a piece resting legally against one edge must
            // not be dragged inward because the opposite edge needed repair.
            let sides = [
                (0usize, 1.0f64, edge_mm - bounds.min_x),
                (
                    0usize,
                    -1.0f64,
                    bounds.max_x - (settings.sheet_short_axis_mm - edge_mm),
                ),
                (1usize, 1.0f64, edge_mm - bounds.min_y),
                (
                    1usize,
                    -1.0f64,
                    bounds.max_y - (settings.sheet_long_axis_mm - edge_mm),
                ),
            ];
            for (axis, sign, overrun_mm) in sides {
                let rhs_mm = if overrun_mm > 0.0 {
                    overrun_mm + margin_mm
                } else {
                    overrun_mm
                };
                rows.push(GlobalRow::Axis {
                    piece: index,
                    axis,
                    sign,
                    rhs_mm,
                });
            }
        }
    }
    let boundary_rows = rows.len() - pair_rows;
    (rows, pair_rows, boundary_rows)
}

/// Solves `min ||t||^2 subject to A t >= b` by Hildreth's method.
///
/// Hildreth's is projected Gauss-Seidel on the dual of that program. Each row
/// `k` owns one multiplier `lambda_k >= 0`; a sweep visits the rows in order,
/// moves each multiplier by its own residual scaled by the row's squared norm,
/// clips it at zero, and pushes the difference straight into the primal iterate
/// `t = sum_k lambda_k a_k`. Its fixpoint is the exact projection of the origin
/// onto the polyhedron, i.e. the minimum-norm correction - so an inactive row
/// costs nothing (its multiplier stays at zero) and an active one pays exactly
/// the pressure needed to hold it.
///
/// Returns the step, the sweeps spent, and the widest residual still standing.
/// A residual above the grid quantum means the program did not close, which the
/// caller reports rather than papers over: an infeasible system is a real
/// answer about the layout.
fn solve_minimum_norm_step(rows: &[GlobalRow], slots: usize) -> (Vec<(f64, f64)>, usize, f64) {
    let mut translations = vec![(0.0f64, 0.0f64); slots];
    if rows.is_empty() {
        return (translations, 0, 0.0);
    }
    let mut multipliers = vec![0.0f64; rows.len()];
    let mut sweeps = 0usize;
    let mut max_residual_mm = 0.0f64;
    for _ in 0..GLOBAL_LEGALIZATION_DUAL_SWEEPS {
        sweeps += 1;
        max_residual_mm = 0.0;
        for (index, row) in rows.iter().enumerate() {
            let residual_mm = row.rhs_mm() - row.value(&translations);
            if residual_mm > max_residual_mm {
                max_residual_mm = residual_mm;
            }
            let updated = (multipliers[index] + residual_mm / row.norm_squared()).max(0.0);
            let step = updated - multipliers[index];
            if step != 0.0 {
                multipliers[index] = updated;
                row.apply(&mut translations, step);
            }
        }
        // Half the grid quantum: below that the snap the caller applies next
        // decides the answer anyway, so further sweeps buy nothing real.
        if max_residual_mm <= 0.5 * GRID_MM {
            break;
        }
    }
    (translations, sweeps, max_residual_mm)
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

    #[test]
    fn the_ejection_limit_is_the_component_limit_at_every_scale() {
        // The re-placement repair refuses exactly what a micro-repair would
        // have refused, so the two thresholds have to agree at every piece
        // count rather than only at the ones this experiment happens to run.
        for piece_count in [0usize, 1, 2, 3, 17, 20, 61, 137, 400, 4_000] {
            assert_eq!(
                replacement_ejection_limit(piece_count),
                micro_legalization_component_limit(piece_count),
                "piece count {piece_count}"
            );
            // Never zero, so a violating pair always has a legal cover, and
            // never more than an eighth of the layout once the layout is big
            // enough for that to bite.
            assert!(replacement_ejection_limit(piece_count) >= 4);
            if piece_count >= 32 {
                assert!(replacement_ejection_limit(piece_count) <= piece_count / 8);
            }
        }
    }

    #[test]
    fn the_violation_survey_reports_pairs_with_their_mass() {
        let square = rectangle(20.0, 20.0);
        let pieces = vec![
            piece("a", &square),
            piece("b", &square),
            piece("c", &square),
        ];
        // `a` and `b` sit 1 mm apart under a 5 mm contract - a 4 mm deficit,
        // the millimetre-scale residue the micro-legalizer refuses. `c` is far
        // away and must not appear at all.
        let placements = vec![
            placement("a", 20.0, 20.0),
            placement("b", 41.0, 20.0),
            placement("c", 20.0, 120.0),
        ];
        let settings = settings();

        let violations =
            survey_layout_violations(&pieces, &placements, settings).expect("a surveyable layout");
        assert_eq!(violations.pairs.len(), 1, "{violations:?}");
        let pair = violations.pairs[0];
        assert_eq!((pair.first, pair.second), (0, 1));
        assert!(
            (pair.mass_mm - 4.0).abs() < 1e-6,
            "expected a 4 mm mass, got {}",
            pair.mass_mm
        );
        assert!(violations.boundary_pieces.is_empty());
        assert_eq!(violations.components, vec![vec![0, 1]]);
        assert_eq!(violations.largest_component_pieces(), 2);

        // Incident mass is what the ejection choice maximizes, so it has to
        // charge both endpoints and leave everything else at zero.
        let mass = violations.incident_mass(placements.len());
        assert!((mass[0] - 4.0).abs() < 1e-6);
        assert!((mass[1] - 4.0).abs() < 1e-6);
        assert_eq!(mass[2], 0.0);
    }

    #[test]
    fn the_violation_survey_and_the_repair_accept_a_sub_layout() {
        // The re-placement repair surveys and micro-legalizes the layout with
        // its ejection set removed, so both entry points have to work on a
        // strict sub-layout - with indices into the placements, not the
        // request.
        let square = rectangle(20.0, 20.0);
        let pieces = vec![
            piece("a", &square),
            piece("b", &square),
            piece("c", &square),
        ];
        let settings = settings();
        // `b` removed: what is left is legal, and `c` is now slot 1.
        let kept = vec![placement("a", 20.0, 20.0), placement("c", 20.0, 120.0)];

        let violations =
            survey_layout_violations(&pieces, &kept, settings).expect("a surveyable sub-layout");
        assert!(violations.pairs.is_empty(), "{violations:?}");
        assert!(violations.boundary_pieces.is_empty());

        let (diagnostics, repaired) = micro_legalize(&pieces, &kept, settings);
        assert!(diagnostics.skipped_reason.is_none(), "{diagnostics:?}");
        assert!(diagnostics.exact_valid, "{diagnostics:?}");
        assert_eq!(
            repaired.expect("an already-legal sub-layout comes back"),
            kept
        );
    }

    #[test]
    fn a_millimetre_scale_pair_deficit_is_refused_by_projection() {
        // The premise of the re-placement repair: this residue class is
        // outside the micro-legalizer's admission bound by construction, so
        // the second tier is the only thing that can ever see it.
        let square = rectangle(20.0, 20.0);
        let pieces = vec![piece("a", &square), piece("b", &square)];
        let placements = vec![placement("a", 20.0, 20.0), placement("b", 41.0, 20.0)];
        let settings = settings();

        let (diagnostics, repaired) = micro_legalize(&pieces, &placements, settings);
        assert!(repaired.is_none(), "{diagnostics:?}");
        assert!(!diagnostics.attempted, "{diagnostics:?}");
        assert!(
            diagnostics
                .skipped_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("admission bound")),
            "{diagnostics:?}"
        );
    }

    // ---------------------------------------------------------------------
    // Global pressure-balanced legalization
    // ---------------------------------------------------------------------

    /// A tall, narrow corridor: three 20 mm wide pieces that reach nearly the
    /// full length of the sheet, so no pair can ever separate along the long
    /// axis and every correction has to be made along the short one.
    ///
    /// The arithmetic that makes the case interesting, at the module's test
    /// contract (5 mm pair clearance, 5 mm edge clearance, 2.5005 mm envelope
    /// expansion against a 2.5 mm inset, so pieces need 5.001 mm of daylight
    /// and 5.0005 mm from each edge): the usable width is
    /// `short_axis - 10.001` and three pieces with two gaps need `70.002`, so
    /// an 82 mm sheet carries about two millimetres of total slack.
    fn corridor_pieces() -> PolygonSet {
        rectangle(20.0, 200.0)
    }

    #[test]
    fn a_millimetre_scale_pair_deficit_beyond_the_local_pass_is_solved_globally() {
        // The residue class `a_millimetre_scale_pair_deficit_is_refused_by_projection`
        // documents: a 2 mm shortfall, four times the micro-legalizer's
        // admission bound. The global pass has no admission bound, because
        // refusing this is what the local pass is for.
        let square = rectangle(20.0, 20.0);
        let pieces = vec![piece("a", &square), piece("b", &square)];
        let placements = vec![placement("a", 20.0, 20.0), placement("b", 43.0, 20.0)];
        let settings = settings();
        assert!(validate_and_measure_placements(&pieces, &placements, settings).is_err());
        let (local, locally_repaired) = micro_legalize(&pieces, &placements, settings);
        assert!(locally_repaired.is_none(), "{local:?}");

        let (diagnostics, repaired) = global_legalize(&pieces, &placements, settings, None);
        assert!(diagnostics.attempted, "{diagnostics:?}");
        assert_eq!(diagnostics.violating_pairs_before, 1, "{diagnostics:?}");
        assert_eq!(diagnostics.violating_pairs_after, 0, "{diagnostics:?}");
        assert!(diagnostics.resolved, "{diagnostics:?}");
        assert!(diagnostics.exact_valid, "{diagnostics:?}");
        let repaired = repaired.expect("a solvable two-piece deficit publishes");
        validate_and_measure_placements(&pieces, &repaired, settings)
            .expect("the globally legalized state validates against the real request");
        // Minimum-norm means the correction is shared, not loaded onto one
        // endpoint: each square carries about half of the 2 mm.
        assert_eq!(diagnostics.moved_pieces, 2, "{diagnostics:?}");
        let left = repaired[0].translate_short_axis - placements[0].translate_short_axis;
        let right = repaired[1].translate_short_axis - placements[1].translate_short_axis;
        assert!(left < 0.0 && right > 0.0, "{repaired:?}");
        assert!((left.abs() - right.abs()).abs() < 0.01, "{repaired:?}");
    }

    #[test]
    fn a_corridor_residue_is_solved_only_by_moving_a_piece_that_violates_nothing() {
        // `a` and `b` are 3 mm apart against a 5 mm contract; `c` is 5.01 mm
        // from `b` and violates nothing at all. Opening the a-b conflict needs
        // about 2 mm, and there is nowhere to put it except through `c`: the
        // sheet's left edge is 0.5 mm from `a`, the pieces are too tall to
        // pass each other along the long axis, and any feasible arrangement
        // puts `c` at 55.0 mm or beyond. A repair that only ever moves
        // violating pieces cannot solve this state at any magnitude.
        let slab = corridor_pieces();
        let pieces = vec![piece("a", &slab), piece("b", &slab), piece("c", &slab)];
        let placements = vec![
            placement("a", 5.5, 20.0),
            placement("b", 28.5, 20.0),
            placement("c", 53.51, 20.0),
        ];
        let settings = sheet_settings(82.0, 300.0);
        assert!(validate_and_measure_placements(&pieces, &placements, settings).is_err());

        // The premise: `c` is legal, so it is not even in the local pass's
        // violation component.
        let survey = survey_layout_violations(&pieces, &placements, settings)
            .expect("the corridor state surveys");
        assert_eq!(survey.pairs.len(), 1, "{survey:?}");
        assert_eq!((survey.pairs[0].first, survey.pairs[0].second), (0, 1));

        let (diagnostics, repaired) = global_legalize(&pieces, &placements, settings, None);
        assert!(diagnostics.attempted, "{diagnostics:?}");
        assert!(diagnostics.resolved, "{diagnostics:?}");
        assert!(diagnostics.exact_valid, "{diagnostics:?}");
        let repaired = repaired.expect("the corridor residue is globally solvable");
        validate_and_measure_placements(&pieces, &repaired, settings)
            .expect("the globally legalized corridor validates");
        // The point of the test: the piece that violated nothing had to move,
        // and had to move by millimetres rather than by a rounding quantum.
        let moved_c = repaired[2].translate_short_axis - placements[2].translate_short_axis;
        assert!(moved_c > 1.0, "c moved {moved_c} mm: {repaired:?}");
        assert_eq!(diagnostics.moved_pieces, 3, "{diagnostics:?}");
    }

    #[test]
    fn the_depth_bound_is_a_hard_constraint_on_every_piece() {
        // Neither square violates anything: the layout is exactly legal at a
        // depth of 80 mm. The bound alone is what makes it a repair problem,
        // and satisfying it needs *both* pieces to move, the lower one first.
        let square = rectangle(20.0, 20.0);
        let pieces = vec![piece("a", &square), piece("b", &square)];
        let placements = vec![placement("a", 20.0, 20.0), placement("b", 20.0, 60.0)];
        let settings = settings();
        let metrics = validate_and_measure_placements(&pieces, &placements, settings)
            .expect("the unbounded state is already legal");
        assert!(metrics.used_long_axis_depth_mm > 60.0);

        let (diagnostics, repaired) = global_legalize(&pieces, &placements, settings, Some(60.0));
        assert_eq!(diagnostics.bound_mm, Some(60.0), "{diagnostics:?}");
        assert_eq!(diagnostics.effective_long_axis_mm, 60.0, "{diagnostics:?}");
        assert_eq!(diagnostics.violating_pairs_before, 0, "{diagnostics:?}");
        assert_eq!(diagnostics.boundary_pieces_before, 1, "{diagnostics:?}");
        assert!(diagnostics.exact_valid, "{diagnostics:?}");
        let repaired = repaired.expect("the bounded state is solvable");
        let bounded = validate_and_measure_placements(&pieces, &repaired, settings)
            .expect("the bounded state validates against the real request");
        assert!(
            bounded.used_long_axis_depth_mm <= 60.0,
            "published depth {} exceeds the bound",
            bounded.used_long_axis_depth_mm
        );
        assert!(repaired[0].translate_long_axis < placements[0].translate_long_axis);
        assert!(repaired[1].translate_long_axis < placements[1].translate_long_axis);
    }

    #[test]
    fn a_layout_its_sheet_cannot_hold_fails_cleanly() {
        // Two 20 mm squares in a 40 mm sheet. The usable box is 29.999 mm on
        // each axis and separating them needs 25.001 mm along one of them, so
        // no translation of any magnitude legalizes this. The pass must say so
        // rather than publish, loop, or run away with the displacement.
        let square = rectangle(20.0, 20.0);
        let pieces = vec![piece("a", &square), piece("b", &square)];
        let placements = vec![placement("a", 6.0, 6.0), placement("b", 12.0, 12.0)];
        let settings = sheet_settings(40.0, 40.0);

        let (diagnostics, repaired) = global_legalize(&pieces, &placements, settings, None);
        assert!(repaired.is_none(), "{diagnostics:?}");
        assert!(diagnostics.attempted, "{diagnostics:?}");
        assert!(!diagnostics.resolved, "{diagnostics:?}");
        assert!(!diagnostics.exact_valid, "{diagnostics:?}");
        assert!(diagnostics.violating_pairs_after > 0, "{diagnostics:?}");
        assert!(
            diagnostics
                .rejection_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("feasible fixpoint")),
            "{diagnostics:?}"
        );
        assert!(
            diagnostics.max_displacement_mm.unwrap_or(0.0)
                <= diagnostics.displacement_cap_mm + GRID_MM,
            "{diagnostics:?}"
        );
        assert!(diagnostics.cap_exhausted.is_none(), "{diagnostics:?}");
    }

    #[test]
    fn the_global_pass_is_deterministic() {
        let slab = corridor_pieces();
        let pieces = vec![piece("a", &slab), piece("b", &slab), piece("c", &slab)];
        let placements = vec![
            placement("a", 5.5, 20.0),
            placement("b", 28.5, 20.0),
            placement("c", 53.51, 20.0),
        ];
        let settings = sheet_settings(82.0, 300.0);
        let first = global_legalize(&pieces, &placements, settings, Some(240.0));
        let second = global_legalize(&pieces, &placements, settings, Some(240.0));
        assert_eq!(first.0, second.0);
        assert_eq!(first.1, second.1);
        // And an unsolvable state is deterministic in its failure too.
        let square = rectangle(20.0, 20.0);
        let cramped = vec![piece("a", &square), piece("b", &square)];
        let cramped_placements = vec![placement("a", 6.0, 6.0), placement("b", 12.0, 12.0)];
        let cramped_settings = sheet_settings(40.0, 40.0);
        let first = global_legalize(&cramped, &cramped_placements, cramped_settings, None);
        let second = global_legalize(&cramped, &cramped_placements, cramped_settings, None);
        assert_eq!(first.0, second.0);
        assert_eq!(first.1, second.1);
    }

    #[test]
    fn an_already_legal_state_is_returned_unchanged() {
        let square = rectangle(20.0, 20.0);
        let pieces = vec![piece("a", &square), piece("b", &square)];
        let placements = vec![placement("a", 20.0, 20.0), placement("b", 46.0, 20.0)];
        let settings = settings();
        let (diagnostics, repaired) = global_legalize(&pieces, &placements, settings, None);
        assert!(!diagnostics.attempted, "{diagnostics:?}");
        assert!(diagnostics.resolved, "{diagnostics:?}");
        assert!(diagnostics.exact_valid, "{diagnostics:?}");
        assert_eq!(repaired.as_deref(), Some(placements.as_slice()));
    }

    #[test]
    fn the_probe_ledger_is_the_ceiling_the_quota_test_asserts() {
        for piece_count in [1usize, 2, 3, 17, 61] {
            let mut budget = GlobalLegalizationBudget::for_piece_count(piece_count);
            let complete_pairs = piece_count * piece_count.saturating_sub(1) / 2;
            let rounds = (GLOBAL_LEGALIZATION_ESCALATIONS + 1) * GLOBAL_LEGALIZATION_ROUNDS;
            for round in 0..rounds {
                assert!(
                    budget.charge_round(complete_pairs).is_ok(),
                    "piece count {piece_count} round {round}"
                );
            }
            if complete_pairs > 0 {
                assert!(budget.charge_round(complete_pairs).is_err());
            }
            assert_eq!(
                global_legalization_worst_case_pair_visits(piece_count),
                rounds * complete_pairs * GLOBAL_LEGALIZATION_PAIR_PROBES_PER_ROUND
            );
            assert_eq!(
                global_legalization_worst_case_collision_builds(piece_count),
                (GLOBAL_LEGALIZATION_ESCALATIONS + 1) * piece_count
            );
        }
    }
}
