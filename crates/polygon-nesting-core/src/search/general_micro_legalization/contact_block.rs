//! The active-contact block SE(2) operator.
//!
//! # The action, and why it is not one of the three that already failed
//!
//! Sol review 10 §3 named this as the one search action left outside the
//! operator space `{m20, m22, m23, m26, m31, m33, m34 + continuous/sparse
//! rotation + overlay + race}`. Its shape:
//!
//! 1. build the graph of **near-binding contacts** that touch the pieces which
//!    set the published depth;
//! 2. take a **small connected component** (2–5 pieces) containing the
//!    depth-setter;
//! 3. propose `(dx, dy, dtheta)` for the whole component **jointly**, over the
//!    linearized contact constraints, inside a trust region;
//! 4. apply the component's motion **as one action**, validated exactly;
//! 5. re-derive the contact graph and repeat — the setter moves, so the block
//!    moves with it.
//!
//! Three things in this repository already look like it and are not it, and the
//! difference in each case is the reason this module exists rather than a knob
//! on one of them:
//!
//! * **m33 / the witness→m33 path** moves one piece and hopes the operators
//!   downstream compose an exit. Here the rotation of the deep piece, the
//!   lateral release of its neighbour and the compression of the partner are a
//!   single accepted-or-rejected action.
//! * **[`super::global_legalize`]** is translation-only, and the ledger's
//!   finding at `docs/next-generation-engine-plan.md` ~297–303 is that
//!   translation-only separation is insufficient at a fixpoint.
//! * **[`super::se2_certificate::se2_witness_proposal`]** — design C of
//!   `docs/experiments/sparse-rotation/` — already solves an SE(2) program over
//!   the **whole layout**, and it moved the final published depth on 0 of 12
//!   parents. That is the prior this module has to answer, so it is worth
//!   stating exactly what it says and what it does not.
//!
//! # The prior: why the whole-layout witness is not already this program
//!
//! For one trust radius the whole-layout program is a strict **superset** of
//! this one: everything a block of five pieces may do, sixty-one free pieces
//! may also do. A restriction cannot beat its own relaxation in the model, and
//! this module does not claim it does. What the restriction buys is three
//! things the whole-layout call structurally cannot have:
//!
//! 1. **Radius.** `docs/experiments/sparse-rotation/` §3.2 measured the
//!    whole-layout witness returning *exactly the trust radius* — 0.025000 mm
//!    at trust 0.025 — because at a radius small enough for the linearization
//!    to hold, the answer is the box and nothing else. Widening the box for all
//!    sixty-one pieces buys model error: the full-length vector stops
//!    validating. Widening it for five pieces is a different bet, because the
//!    five are chosen to be the ones with room.
//! 2. **Re-linearization.** The whole-layout witness is one shot per floor. The
//!    contact normals it linearizes at are the parent's; after any motion they
//!    are stale. This module runs a **trust-region sequential convexification**:
//!    solve, exact-validate, *rebuild the geometry and the graph*, solve again.
//!    A rejected step contracts the radius instead of ending the call.
//! 3. **A named ceiling.** The published depth is a maximum over all pieces, so
//!    a block containing the deepest piece can only buy depth down to the
//!    deepest piece *outside* it. That quantity — [`BlockRound::headroom_mm`] —
//!    is measured and reported here, because without it a null result cannot be
//!    told apart from a layout in which no action of this class could ever have
//!    paid.
//!
//! # The program
//!
//! Variables are three per **block** piece and none per pinned piece, so a
//! five-piece block is a fifteen-variable program. Rows are built exactly as
//! [`super::se2_certificate`] builds them — same normals from the same exact
//! closest-approach witness, same per-vertex boundary families, same outward
//! relaxation by the exact chord error of the first-order rotation model — with
//! one difference that follows from pinning:
//!
//! > A row between a block piece and a pinned piece keeps the block piece's
//! > coefficients and drops the pinned one's, because the pinned piece's
//! > variables are identically zero. It is still a row. Dropping it instead
//! > would let the block walk into the layout.
//!
//! The objective is `max delta` on the block's **material depth** rows, which
//! is [`super::se2_certificate::Se2Program::DepthOnly`] restricted to the block.
//! The collision-envelope strip rows are held at the parent's own strip bound
//! rather than being asked to shrink, for the reason that module documents: the
//! envelope's miter reach is a different number measured off different
//! geometry, and coupling them is how the previous branch ended up calibrating
//! a bound by hand.
//!
//! # Which gate decides — the correction this module was built wrong without
//!
//! The line search validates against [`validate_and_measure_placements`], the
//! **composite** acceptance check: the publication contract *and*
//! canonical-collision-grid admissibility. Not [`validate_publication`], which
//! is only the contract half.
//!
//! The first version of this module used the contract half, because that is
//! what [`super::se2_certificate`]'s witness line search uses. At that module's
//! 0.025 mm trust radius the difference never shows. At this one's 0.5–2 mm it
//! shows on every parent: a block step can open the source outlines' clearance
//! while pushing two collision envelopes into overlap on the canonical grid,
//! and `general_relaxed.rs:6413` runs the composite check on any parent handed
//! to mode 34 and refuses the entire run when it fails. The wrong gate produced
//! a median 0.506 mm across twelve parents that no downstream operator could
//! accept. [`BlockRound::contract_only_accepts`] counts the difference, per
//! round, so the correction stays visible in the data rather than only in this
//! comment.
//!
//! # What is a bound here and what is not
//!
//! Identical in kind to the certificate's, and worth repeating because the same
//! confusion is easy to reintroduce:
//!
//! * [`BlockRound::model_upper_mm`] is weak duality on the **linearized**
//!   program, in `f64` with an outward rounding allowance. It bounds the model,
//!   not the geometry.
//! * [`BlockRound::validated_delta_mm`] is the publication measure on
//!   placements the composite gate accepted. It is the only number here that
//!   lower-bounds what the action achieves, and it owes nothing to the
//!   linearization.
//!
//! The two do not meet, and the gap between them is a reported diagnostic
//! ([`BlockRound::full_step_exact_valid`]) rather than something to average
//! over.

use std::collections::BTreeMap;

use serde::Serialize;

use super::se2_certificate::{
    apply_se2, build_geometry, exact_placements, rotation_chord_slack_mm, rotation_coefficient,
    Geometry, RoundedSum, RowFamily,
};
use super::{measure_approach, Contracts};
use crate::domain::IrregularPoint;
use crate::search::general_fast::{
    collision_expansion_mm, collision_sheet_inset_mm, effective_sheet_edge_clearance_mm,
    validate_and_measure_placements, GeneralFastPiece, GeneralFastPlacement, GeneralFastSettings,
};
use crate::validation::general_polygon::{
    raw_source_long_axis_depth_mm, validate_publication, PublicationValidationSettings,
};

/// Penalty weights tried in sequence, as in [`super::se2_certificate`]: the
/// primal ascent maximizes an exact penalty whose `rho` has to exceed the
/// optimal dual multipliers, and running the ladder is cheaper than estimating
/// them. Every point kept is feasible for every non-depth row, so a badly
/// chosen `rho` costs tightness and never validity.
const PENALTY_LADDER: [f64; 4] = [1.0, 4.0, 16.0, 64.0];

/// How often the dual weighting is evaluated, in primal iterations.
const DUAL_EVERY: usize = 8;

/// Scales the exact line search tries, in the order tried. `0.0` is last and is
/// never rejected — it reproduces the layout the round started from.
const BLOCK_SCALES: [f64; 8] = [1.0, 0.75, 0.5, 0.25, 0.1, 0.05, 0.01, 0.0];

/// Bisection steps spent refining the accepted scale upward.
const BLOCK_BISECTIONS: usize = 6;

/// How far below the largest per-piece depth a piece may sit and still be
/// offered as a block seed, as a multiple of the round's trust radius.
///
/// A piece further under the front than the action's own reach cannot become
/// the setter inside this step, so seeding a block on it would be measuring a
/// contact graph the objective cannot pay for.
const SEED_BAND_TRUST_MULTIPLE: f64 = 2.0;

/// The floor the trust-region contraction stops at, as a fraction of the
/// round's starting radius. Below this the step is smaller than the grid the
/// publication measure quantizes to and a further contraction is arithmetic
/// rather than search.
const TRUST_CONTRACTION_FLOOR: f64 = 1.0 / 64.0;

/// The knobs. Every one of them is a spec-key field; none has a default that
/// arms the operator.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactBlockSettings {
    /// Trust radius for a block piece, in millimetres. This is the whole point
    /// of blocking: it is meant to be *large* compared with the whole-layout
    /// witness's, because it is spent on five pieces rather than sixty-one.
    pub trust_radius_mm: f64,
    /// Primal iterations per penalty weight, per solve.
    pub iterations: usize,
    /// Largest block the component walk may return. Sol review 10 §3 says 2–5.
    pub max_block_pieces: usize,
    /// Sequential-convexification rounds: solves, each re-deriving the geometry
    /// and the contact graph from the layout the previous round left.
    pub rounds: usize,
    /// How many distinct depth-setting seeds a round may try before giving up.
    /// The first seed is always the deepest piece.
    pub seeds: usize,
    /// The near-binding band, as a multiple of the trust radius. A contact
    /// wider than this cannot be reached inside the trust region, so it is not
    /// part of the block's contact graph — but it still gets a **row**, which
    /// is a different question and is decided by the guard band below.
    pub band_trust_multiple: f64,
}

impl ContactBlockSettings {
    fn validate(&self) -> Result<(), String> {
        if !self.trust_radius_mm.is_finite() || self.trust_radius_mm <= 0.0 {
            return Err("contact block trust radius must be positive and finite".to_owned());
        }
        if self.iterations == 0 {
            return Err("contact block requires at least one iteration".to_owned());
        }
        if self.max_block_pieces < 2 {
            return Err("contact block requires a block of at least two pieces".to_owned());
        }
        if self.rounds == 0 {
            return Err("contact block requires at least one round".to_owned());
        }
        if self.seeds == 0 {
            return Err("contact block requires at least one seed".to_owned());
        }
        if !self.band_trust_multiple.is_finite() || self.band_trust_multiple <= 0.0 {
            return Err("contact block band multiple must be positive and finite".to_owned());
        }
        Ok(())
    }
}

/// One measured contact between two pieces, by layout index.
///
/// The reported form is [`ContactEdge`], which carries piece ids; this is the
/// internal one the walk uses, and the two are kept apart so an index never has
/// to be recovered from a string.
#[derive(Clone, Copy, Debug)]
struct BlockEdge {
    first: usize,
    second: usize,
    slack_mm: f64,
    gate: &'static str,
}

/// One neighbour of a piece in the contact graph, and the gate that measured it.
#[derive(Clone, Copy, Debug)]
struct Neighbour {
    piece: usize,
    slack_mm: f64,
    gate: &'static str,
}

/// One edge of the contact graph, as it was measured, named for reporting.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactEdge {
    pub first: String,
    pub second: String,
    /// `approach distance - contract`: zero or less is a binding contact, and
    /// the band admits anything under `band_trust_multiple * trust`.
    pub slack_mm: f64,
    pub gate: &'static str,
}

/// What one sequential-convexification round did.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockRound {
    pub round: usize,
    /// The trust radius this round solved at, after any contraction.
    pub trust_radius_mm: f64,
    /// The published depth the round started from.
    pub parent_depth_mm: f64,
    /// The piece that set that depth, and the depth it set.
    pub setter: String,
    /// The block, in the order the component walk found it. The seed is first.
    pub block: Vec<String>,
    /// Contacts inside the band that touch the block.
    pub edges: Vec<ContactEdge>,
    /// How many pieces sat within the seed band of the front.
    pub depth_band_pieces: usize,
    /// Which seed of [`ContactBlockSettings::seeds`] this round used.
    pub seed_rank: usize,
    /// **The ceiling.** `parent_depth_mm` minus the deepest piece *outside* the
    /// block: the most any motion of this block could possibly buy, whatever
    /// the program says. A round whose `validated_delta_mm` equals this bought
    /// everything there was; a round whose headroom is ~0 was never going to
    /// pay, and that is a fact about the layout and not about the operator.
    pub headroom_mm: f64,
    pub rows: usize,
    pub rows_by_family: BTreeMap<String, usize>,
    pub delta_rows: usize,
    /// Weak-duality bound on the **linearized** program. `None` when no row
    /// carries the objective.
    pub model_upper_mm: Option<f64>,
    /// Best objective at a point feasible for every non-objective row of the
    /// model. `None` when the ascent never visited one.
    pub model_lower_mm: Option<f64>,
    /// Did the model's own full-length vector survive the **composite** gate,
    /// [`validate_and_measure_placements`]?
    pub full_step_exact_valid: bool,
    /// The validator's message when it did not.
    pub full_step_rejection: Option<String>,
    /// Steps this round's line search offered that the **contract** gate would
    /// have accepted and the composite gate refused — the collision envelopes
    /// overlapping on the canonical grid while the source outlines still clear
    /// their contract. See [`evaluate_scale`]. A nonzero value is the first
    /// version of this operator being wrong, counted.
    pub contract_only_accepts: usize,
    /// The fraction of the model's vector the exact validator accepted.
    pub scale: f64,
    /// Exact validations this round spent. The dominant price.
    pub validations: usize,
    /// `parent_depth_mm` minus the published depth of the accepted layout.
    /// Zero when the line search fell back to the layout it started from.
    pub validated_delta_mm: f64,
    /// Largest applied `|dtheta|`, in degrees.
    pub max_abs_dtheta_deg: f64,
    /// Largest applied `|(dx, dy)|`, in millimetres.
    pub max_abs_translation_mm: f64,
    /// Why the round produced nothing, when it produced nothing. One of
    /// `no-component`, `no-priced-row`, `model-blocked`, `exact-rejected`,
    /// `no-depth-gain`; `None` when the round moved the depth.
    pub refusal: Option<&'static str>,
}

/// What one whole call did.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactBlockReport {
    pub settings: ContactBlockSettings,
    pub piece_count: usize,
    pub parent_depth_mm: f64,
    /// The publication measure on the layout this call ends holding.
    pub final_depth_mm: f64,
    /// `parent_depth_mm - final_depth_mm`, exactly validated end to end.
    pub delta_mm: f64,
    pub rounds: Vec<BlockRound>,
    /// Rounds that produced a strictly shallower exactly-valid layout.
    pub rounds_accepted: usize,
    /// Solves run, including the ones a contraction repeated.
    pub solves: usize,
    pub validations: usize,
    pub rows_total: usize,
    pub elapsed_ms: f64,
}

/// The moved layout, when a call bought anything. Separate from the report so a
/// search can take the placements and a driver can take the decomposition.
#[derive(Clone, Debug)]
pub struct ContactBlockProposal {
    /// Already accepted by [`validate_publication`]: the line search never
    /// returns a scale it did not validate.
    pub placements: Vec<GeneralFastPlacement>,
    pub published_depth_mm: f64,
    pub delta_mm: f64,
    pub moved_pieces: usize,
    pub validations: usize,
}

/// One row of the block program.
///
/// `first` and `second` index **block slots**, not layout pieces. A row against
/// a pinned piece has `second: None` and keeps only the block piece's
/// coefficients, which is exactly what pinning that piece's variables to zero
/// does to the row it would otherwise have had.
#[derive(Clone, Copy, Debug)]
struct BlockRow {
    first: usize,
    second: Option<usize>,
    normal: (f64, f64),
    theta_first: f64,
    theta_second: f64,
    rhs_mm: f64,
    family: RowFamily,
}

impl BlockRow {
    fn scaled(&self, scales: &[f64]) -> BlockRow {
        let mut row = *self;
        row.theta_first *= scales[self.first];
        if let Some(second) = self.second {
            row.theta_second *= scales[second];
        }
        row
    }

    fn value(&self, x: &[(f64, f64, f64)]) -> RoundedSum {
        let mut sum = RoundedSum::default();
        sum.add(self.normal.0 * x[self.first].0);
        sum.add(self.normal.1 * x[self.first].1);
        sum.add(self.theta_first * x[self.first].2);
        if let Some(second) = self.second {
            sum.add(-self.normal.0 * x[second].0);
            sum.add(-self.normal.1 * x[second].1);
            sum.add(self.theta_second * x[second].2);
        }
        sum
    }

    fn residual_low_mm(&self, x: &[(f64, f64, f64)]) -> f64 {
        let mut sum = self.value(x);
        sum.add(-self.rhs_mm);
        sum.low()
    }

    fn accumulate(&self, weight: f64, c: &mut [(f64, f64, f64)]) {
        c[self.first].0 += weight * self.normal.0;
        c[self.first].1 += weight * self.normal.1;
        c[self.first].2 += weight * self.theta_first;
        if let Some(second) = self.second {
            c[second].0 -= weight * self.normal.0;
            c[second].1 -= weight * self.normal.1;
            c[second].2 += weight * self.theta_second;
        }
    }

    fn step(&self, x: &mut [(f64, f64, f64)], step: f64) {
        x[self.first].0 += step * self.normal.0;
        x[self.first].1 += step * self.normal.1;
        x[self.first].2 += step * self.theta_first;
        if let Some(second) = self.second {
            x[second].0 -= step * self.normal.0;
            x[second].1 -= step * self.normal.1;
            x[second].2 += step * self.theta_second;
        }
    }
}

/// A piece's own contribution to the publication measure: the highest outer
/// vertex of its transformed source outline, plus the sheet edge clearance.
///
/// The maximum of this over the layout is what
/// [`raw_source_long_axis_depth_mm`] reports, which is why the depth rows can
/// be built against it piece by piece.
fn piece_depth_mm(geometry: &Geometry, edge_clearance_mm: f64) -> f64 {
    geometry
        .material_outer
        .iter()
        .fold(f64::NEG_INFINITY, |best, point| best.max(point.y))
        + edge_clearance_mm
}

/// Pushes one side's boundary rows for one block piece and one gate.
///
/// A transcription of [`super::se2_certificate`]'s `push_boundary_rows` onto
/// block slots: one row per vertex that can become the extreme one inside the
/// box, with the exact domination test that prunes the rest, and every row
/// relaxed outward by the chord error of the first-order rotation model so the
/// row really does contain the rotated geometry.
#[allow(clippy::too_many_arguments)]
fn push_boundary_rows(
    rows: &mut Vec<BlockRow>,
    slot: usize,
    vertices: &[IrregularPoint],
    centre: IrregularPoint,
    normal: (f64, f64),
    bound_mm: f64,
    theta_cap: f64,
    slack_mm: f64,
    family: RowFamily,
) {
    let along = |point: &IrregularPoint| normal.0 * point.x + normal.1 * point.y;
    let Some(extreme) = vertices
        .iter()
        .copied()
        .reduce(|best, point| if along(&point) < along(&best) { point } else { best })
    else {
        return;
    };
    let extreme_along = along(&extreme);
    for point in vertices {
        let spread = along(point) - extreme_along;
        if spread > 0.0 {
            let rotational_spread = rotation_coefficient(
                normal,
                IrregularPoint::new(point.x - extreme.x + centre.x, point.y - extreme.y + centre.y),
                centre,
            )
            .abs();
            if spread >= theta_cap * rotational_spread {
                continue;
            }
        }
        rows.push(BlockRow {
            first: slot,
            second: None,
            normal,
            theta_first: rotation_coefficient(normal, *point, centre),
            theta_second: 0.0,
            rhs_mm: bound_mm - along(point) - slack_mm,
            family,
        });
    }
}

/// The contact graph's edges that touch `piece`, tightest first.
///
/// The band is `band_trust_multiple * trust`: a contact wider than the reach of
/// the action cannot bind inside this step, so it is not what "near-binding"
/// means here. The **rows** use a wider guard band, built separately in
/// [`build_block_rows`], because a pair that is merely reachable still has to
/// be prevented from being driven into.
fn neighbours_of(
    geometries: &[Geometry],
    contracts: &Contracts,
    piece: usize,
    band_mm: f64,
) -> Vec<Neighbour> {
    let mut out: Vec<Neighbour> = Vec::new();
    for other in 0..geometries.len() {
        if other == piece {
            continue;
        }
        let mut best: Option<(f64, &'static str)> = None;
        for (gate, contract_mm, first, second) in [
            (
                "material",
                contracts.material_pair_mm,
                &geometries[piece].material,
                &geometries[other].material,
            ),
            (
                "envelope",
                0.0,
                &geometries[piece].collision,
                &geometries[other].collision,
            ),
        ] {
            let ceiling = contract_mm + band_mm;
            if first.bounds.gap(second.bounds) >= ceiling {
                continue;
            }
            let approach = measure_approach(first, (0.0, 0.0), second, (0.0, 0.0), ceiling);
            if approach.distance >= ceiling {
                continue;
            }
            let slack_mm = approach.distance - contract_mm;
            if best.is_none_or(|(incumbent, _)| slack_mm < incumbent) {
                best = Some((slack_mm, gate));
            }
        }
        if let Some((slack_mm, gate)) = best {
            out.push(Neighbour {
                piece: other,
                slack_mm,
                gate,
            });
        }
    }
    // Tightest contact first, piece index as the deterministic tie-break: two
    // processes must walk the same component.
    out.sort_by(|left, right| {
        left.slack_mm
            .partial_cmp(&right.slack_mm)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(left.piece.cmp(&right.piece))
    });
    out
}

/// Grows a connected component of the contact graph outward from `seed`,
/// tightest contact first, to at most `max_pieces`.
///
/// Breadth-first over a queue ordered by contact tightness, which is the walk
/// that keeps the block *coherent*: the pieces added are the ones actually
/// holding the seed, not the ones that happen to be nearby.
fn grow_block(
    geometries: &[Geometry],
    contracts: &Contracts,
    seed: usize,
    band_mm: f64,
    max_pieces: usize,
) -> (Vec<usize>, Vec<BlockEdge>) {
    let mut block = vec![seed];
    let mut edges: Vec<BlockEdge> = Vec::new();
    // `block` is its own breadth-first queue: a piece is admitted and visited in
    // the same order, so `cursor` walks exactly the pieces already admitted and
    // no separate frontier is needed.
    //
    // The walk stops as soon as the block is full rather than draining the
    // queue. Expanding a piece costs one `neighbours_of` call, which is an
    // approach measurement against every other piece in the layout, and the only
    // thing further expansion could add once the cap is reached is more
    // *reported* edges - membership is already decided. `build_block_rows` gives
    // every reachable contact a row whether or not the walk recorded an edge for
    // it, so nothing about the program depends on the edge list.
    let mut cursor = 0usize;
    while cursor < block.len() && block.len() < max_pieces {
        let piece = block[cursor];
        cursor += 1;
        for neighbour in neighbours_of(geometries, contracts, piece, band_mm) {
            if !block.contains(&neighbour.piece) {
                if block.len() >= max_pieces {
                    continue;
                }
                block.push(neighbour.piece);
            }
            edges.push(BlockEdge {
                first: piece,
                second: neighbour.piece,
                slack_mm: neighbour.slack_mm,
                gate: neighbour.gate,
            });
        }
    }
    (block, edges)
}

/// Builds the block program's rows at the current geometry.
///
/// Guard bands follow the certificate's correction 5 — `trust_i + trust_j` plus
/// each piece's rotational reach — with the pinned pieces' translation term
/// zero, because a pinned piece does not translate. That is narrower than the
/// certificate's unconditional `2 * trust` and it is narrower *correctly*: the
/// certificate lets every piece translate, and this program does not.
#[allow(clippy::too_many_arguments)]
fn build_block_rows(
    geometries: &[Geometry],
    block: &[usize],
    slot_of: &BTreeMap<usize, usize>,
    contracts: &Contracts,
    settings: GeneralFastSettings,
    published_depth_mm: f64,
    strip_bound_mm: f64,
    edge_clearance_mm: f64,
    collision_inset_mm: f64,
    trust_radius_mm: f64,
    theta_caps: &[f64],
) -> Vec<BlockRow> {
    let mut rows: Vec<BlockRow> = Vec::new();

    // Pair rows. Every pair with at least one block member inside the guard
    // band gets one per gate; a pair of two pinned pieces cannot move and needs
    // none.
    for (slot, &piece) in block.iter().enumerate() {
        let piece_reach = theta_caps[slot] * geometries[piece].reach_mm;
        for other in 0..geometries.len() {
            if other == piece {
                continue;
            }
            let other_slot = slot_of.get(&other).copied();
            // Each block-block pair once, from the lower slot.
            if let Some(other_slot) = other_slot {
                if other_slot < slot {
                    continue;
                }
            }
            let (other_trust_mm, other_reach) = match other_slot {
                Some(other_slot) => (
                    trust_radius_mm,
                    theta_caps[other_slot] * geometries[other].reach_mm,
                ),
                None => (0.0, 0.0),
            };
            let reach_mm = trust_radius_mm + other_trust_mm + piece_reach + other_reach;
            for (family, contract_mm) in [
                (RowFamily::MaterialPair, contracts.material_pair_mm),
                (RowFamily::EnvelopePair, 0.0),
            ] {
                let (first_outline, second_outline) = match family {
                    RowFamily::MaterialPair => {
                        (&geometries[piece].material, &geometries[other].material)
                    }
                    _ => (&geometries[piece].collision, &geometries[other].collision),
                };
                let guard_mm = contract_mm + reach_mm;
                if first_outline.bounds.gap(second_outline.bounds) >= guard_mm {
                    continue;
                }
                let approach =
                    measure_approach(first_outline, (0.0, 0.0), second_outline, (0.0, 0.0), guard_mm);
                if approach.distance >= guard_mm {
                    continue;
                }
                let Some((on_first, on_second)) = approach.witness else {
                    continue;
                };
                let normal = match approach.direction {
                    Some(direction) => direction,
                    None => {
                        let dx = geometries[piece].centre.x - geometries[other].centre.x;
                        let dy = geometries[piece].centre.y - geometries[other].centre.y;
                        let length = dx.hypot(dy);
                        if length <= f64::MIN_POSITIVE {
                            continue;
                        }
                        (dx / length, dy / length)
                    }
                };
                rows.push(BlockRow {
                    first: slot,
                    second: other_slot,
                    normal,
                    theta_first: rotation_coefficient(normal, on_first, geometries[piece].centre),
                    theta_second: match other_slot {
                        Some(_) => {
                            -rotation_coefficient(normal, on_second, geometries[other].centre)
                        }
                        None => 0.0,
                    },
                    rhs_mm: contract_mm - approach.distance,
                    family,
                });
            }
        }
    }

    // Boundary rows, block pieces only.
    for (slot, &piece) in block.iter().enumerate() {
        let geometry = &geometries[piece];
        let theta_cap = theta_caps[slot];
        let slack_mm = rotation_chord_slack_mm(geometry.reach_mm, theta_cap);
        push_boundary_rows(
            &mut rows,
            slot,
            &geometry.material_outer,
            geometry.centre,
            (0.0, -1.0),
            -(published_depth_mm - edge_clearance_mm),
            theta_cap,
            slack_mm,
            RowFamily::MaterialDepth,
        );
        push_boundary_rows(
            &mut rows,
            slot,
            &geometry.collision_outer,
            geometry.centre,
            (0.0, -1.0),
            -(strip_bound_mm - collision_inset_mm),
            theta_cap,
            slack_mm,
            RowFamily::EnvelopeStrip,
        );
        for (vertices, edge_mm, family) in [
            (
                &geometry.material_outer,
                contracts.material_edge_mm,
                RowFamily::MaterialBoundary,
            ),
            (
                &geometry.collision_outer,
                collision_inset_mm,
                RowFamily::EnvelopeBoundary,
            ),
        ] {
            for (normal, bound_mm) in [
                ((1.0, 0.0), edge_mm),
                ((-1.0, 0.0), -(settings.sheet_short_axis_mm - edge_mm)),
                ((0.0, 1.0), edge_mm),
            ] {
                push_boundary_rows(
                    &mut rows,
                    slot,
                    vertices,
                    geometry.centre,
                    normal,
                    bound_mm,
                    theta_cap,
                    slack_mm,
                    family,
                );
            }
        }
    }

    rows
}

fn box_corner(c: &[(f64, f64, f64)], radius: f64) -> Vec<(f64, f64, f64)> {
    c.iter()
        .map(|&(cx, cy, ctheta)| {
            (
                if cx >= 0.0 { radius } else { -radius },
                if cy >= 0.0 { radius } else { -radius },
                if ctheta >= 0.0 { radius } else { -radius },
            )
        })
        .collect()
}

fn project_cube(x: &mut [(f64, f64, f64)], radius: f64) {
    for slot in x.iter_mut() {
        slot.0 = slot.0.clamp(-radius, radius);
        slot.1 = slot.1.clamp(-radius, radius);
        slot.2 = slot.2.clamp(-radius, radius);
    }
}

/// Weak duality on the block program: an upper bound from any `lambda >= 0`
/// whose weight on the objective-carrying rows sums to one. Closed form over
/// the cube, outward-rounded up.
fn dual_bound_mm(rows: &[BlockRow], weights: &[f64], slots: usize, radius: f64) -> f64 {
    let mut c = vec![(0.0f64, 0.0f64, 0.0f64); slots];
    let mut rhs = RoundedSum::default();
    for (row, &weight) in rows.iter().zip(weights) {
        if weight == 0.0 {
            continue;
        }
        row.accumulate(weight, &mut c);
        rhs.add(-weight * row.rhs_mm);
    }
    let mut total = RoundedSum::default();
    total.add(rhs.value());
    for &(cx, cy, ctheta) in &c {
        total.add(radius * cx.abs());
        total.add(radius * cy.abs());
        total.add(radius * ctheta.abs());
    }
    total.high() + rhs.allowance()
}

struct Solved {
    model_lower_mm: Option<f64>,
    model_upper_mm: Option<f64>,
    feasible: Vec<(f64, f64, f64)>,
    objective: Vec<(f64, f64, f64)>,
}

/// The block program's solve: projected supergradient ascent on the exact
/// penalty, over the isotropic cube, with the same guarantees the certificate's
/// solve carries — every iterate feasible for the non-objective rows is a valid
/// lower bound on the model in its own right, so iterations control tightness
/// and nothing else.
fn solve_block(
    unscaled_rows: &[BlockRow],
    delta_rows: &[usize],
    other_rows: &[usize],
    slots: usize,
    trust_radius_mm: f64,
    theta_caps: &[f64],
    iterations: usize,
) -> Solved {
    let scales: Vec<f64> = theta_caps
        .iter()
        .map(|cap| cap / trust_radius_mm)
        .collect();
    let rows: Vec<BlockRow> = unscaled_rows.iter().map(|row| row.scaled(&scales)).collect();
    let rows = rows.as_slice();

    let mut best_primal_mm = f64::NEG_INFINITY;
    let mut best_x = vec![(0.0f64, 0.0f64, 0.0f64); slots];
    let mut best_dual_mm = f64::INFINITY;
    let mut best_objective_mm = f64::NEG_INFINITY;
    let mut best_objective_x = vec![(0.0f64, 0.0f64, 0.0f64); slots];

    if delta_rows.is_empty() {
        return Solved {
            model_lower_mm: None,
            model_upper_mm: None,
            feasible: best_x,
            objective: best_objective_x,
        };
    }

    for &index in delta_rows {
        let mut weights = vec![0.0f64; rows.len()];
        weights[index] = 1.0;
        best_dual_mm = best_dual_mm.min(dual_bound_mm(rows, &weights, slots, trust_radius_mm));
    }

    for &rho in &PENALTY_LADDER {
        let mut x = vec![(0.0f64, 0.0f64, 0.0f64); slots];
        let mut depth_frequency = vec![0.0f64; rows.len()];
        let mut other_frequency = vec![0.0f64; rows.len()];
        let mut depth_mass = 0.0f64;

        for t in 0..iterations {
            let mut worst_depth = (0usize, f64::INFINITY);
            for &index in delta_rows {
                let residual = rows[index].residual_low_mm(&x);
                if residual < worst_depth.1 {
                    worst_depth = (index, residual);
                }
            }
            let mut worst_other = (usize::MAX, f64::INFINITY);
            for &index in other_rows {
                let residual = rows[index].residual_low_mm(&x);
                if residual < worst_other.1 {
                    worst_other = (index, residual);
                }
            }

            let feasible = worst_other.0 == usize::MAX || worst_other.1 >= 0.0;
            if feasible && worst_depth.1 > best_primal_mm {
                best_primal_mm = worst_depth.1;
                best_x.copy_from_slice(&x);
            }
            if worst_depth.1 > best_objective_mm {
                best_objective_mm = worst_depth.1;
                best_objective_x.copy_from_slice(&x);
            }

            depth_frequency[worst_depth.0] += 1.0;
            depth_mass += 1.0;
            let violated = worst_other.0 != usize::MAX && worst_other.1 < 0.0;
            if violated {
                other_frequency[worst_other.0] += 1.0;
            }

            if t % DUAL_EVERY == 0 && depth_mass > 0.0 {
                let mut weights = vec![0.0f64; rows.len()];
                for (slot, count) in weights.iter_mut().zip(&depth_frequency) {
                    *slot = count / depth_mass;
                }
                for (slot, count) in weights.iter_mut().zip(&other_frequency) {
                    *slot += rho * count / depth_mass;
                }
                let mut c = vec![(0.0f64, 0.0f64, 0.0f64); slots];
                for (row, &weight) in rows.iter().zip(&weights) {
                    if weight != 0.0 {
                        row.accumulate(weight, &mut c);
                    }
                }
                best_dual_mm =
                    best_dual_mm.min(dual_bound_mm(rows, &weights, slots, trust_radius_mm));

                let corner = box_corner(&c, trust_radius_mm);
                let corner_feasible = other_rows
                    .iter()
                    .all(|&index| rows[index].residual_low_mm(&corner) >= 0.0);
                if corner_feasible {
                    let corner_delta = delta_rows
                        .iter()
                        .map(|&index| rows[index].residual_low_mm(&corner))
                        .fold(f64::INFINITY, f64::min);
                    if corner_delta > best_primal_mm {
                        best_primal_mm = corner_delta;
                        best_x.copy_from_slice(&corner);
                    }
                }
            }

            let step = trust_radius_mm / ((t as f64) + 1.0).sqrt();
            rows[worst_depth.0].step(&mut x, step);
            if violated {
                rows[worst_other.0].step(&mut x, rho * step);
            }
            project_cube(&mut x, trust_radius_mm);
        }
    }

    for (slot, &scale) in best_x.iter_mut().zip(&scales) {
        slot.2 *= scale;
    }
    for (slot, &scale) in best_objective_x.iter_mut().zip(&scales) {
        slot.2 *= scale;
    }

    Solved {
        model_lower_mm: best_primal_mm.is_finite().then_some(best_primal_mm),
        model_upper_mm: best_dual_mm.is_finite().then_some(best_dual_mm),
        feasible: best_x,
        objective: best_objective_x,
    }
}

/// The layout with the block's motion applied at `scale`, and nothing else
/// touched.
fn apply_block(
    placements: &[GeneralFastPlacement],
    geometries: &[Geometry],
    block: &[usize],
    x: &[(f64, f64, f64)],
    scale: f64,
) -> Vec<GeneralFastPlacement> {
    let mut moved = placements.to_vec();
    for (slot, &piece) in block.iter().enumerate() {
        let delta = x[slot];
        moved[piece] = apply_se2(
            &placements[piece],
            geometries[piece].centre,
            (scale * delta.0, scale * delta.1, scale * delta.2),
        );
    }
    moved
}

enum ScaleOutcome {
    Accepted(f64),
    Refused(String),
}

struct ExactContext<'a, 'p> {
    pieces: &'a [GeneralFastPiece<'p>],
    pieces_by_id: &'a BTreeMap<&'a str, usize>,
    placements: &'a [GeneralFastPlacement],
    geometries: &'a [Geometry],
    block: &'a [usize],
    fast_settings: GeneralFastSettings,
    validation_settings: PublicationValidationSettings,
    edge_clearance_mm: f64,
}

/// **The composite gate, not the contract half of it.**
///
/// This is the correction the round trip forced, and it is worth stating in
/// full because the first version of this module got it wrong and the wrong
/// version produced numbers that looked like a result.
///
/// [`validate_publication`] is the *contract* gate: source outlines against the
/// pair clearance and the sheet edge. The engine's acceptance authority for a
/// layout is [`validate_and_measure_placements`], which is that check **plus**
/// canonical-collision-grid admissibility — every placement's collision
/// envelope inside the sheet and pairwise disjoint on the grid.
/// `general_relaxed.rs:6413` runs exactly that function on any parent handed to
/// mode 34 and refuses the whole run when it fails.
///
/// A block step at a 0.5–2 mm trust radius can open the material contract while
/// driving two collision envelopes into grid overlap. Validating only the
/// contract half accepts that step; the engine then refuses the resulting
/// layout as a parent with "pieces ... overlap on the canonical collision
/// grid", and every millimetre the operator reported was unreachable. Measured:
/// on all twelve pinned parents, before this correction.
///
/// So the composite gate decides, and the contract-only verdict is kept beside
/// it — as [`Searched::contract_only_accepts`] — because the *gap* between the
/// two is the size of the defect and is worth publishing rather than erasing.
fn evaluate_scale(
    context: &ExactContext<'_, '_>,
    x: &[(f64, f64, f64)],
    scale: f64,
    contract_only_accepts: &mut usize,
) -> Result<ScaleOutcome, String> {
    let moved = apply_block(
        context.placements,
        context.geometries,
        context.block,
        x,
        scale,
    );
    let composite = validate_and_measure_placements(context.pieces, &moved, context.fast_settings);
    let exact = exact_placements(context.pieces, context.pieces_by_id, &moved)?;
    if composite.is_err() && validate_publication(&exact, context.validation_settings).is_ok() {
        // The step the contract gate would have taken and the envelope gate
        // will not. This counter is the retraction, in numbers.
        *contract_only_accepts += 1;
    }
    match composite {
        Ok(_) => {
            let depth_mm = raw_source_long_axis_depth_mm(&exact, context.edge_clearance_mm)
                .map_err(|error| error.message().to_owned())?;
            Ok(ScaleOutcome::Accepted(depth_mm))
        }
        Err(error) => Ok(ScaleOutcome::Refused(error.to_string())),
    }
}

/// What the exact line search settled on for one candidate direction.
struct Searched {
    scale: f64,
    depth_mm: f64,
    validations: usize,
    full_step_exact_valid: bool,
    full_step_rejection: Option<String>,
    /// Scales this search offered that the **contract** gate would have taken
    /// and the composite gate refused. See [`evaluate_scale`]: this is the
    /// operator's first version measured against its corrected one.
    contract_only_accepts: usize,
}

/// Line-searches one direction against the exact validator, over the whole
/// layout. The direction is the model's and the length is the validator's, for
/// the reason [`super::se2_certificate::WitnessOutcome`] documents: the rows are
/// relaxed outward to keep the upper bound valid, so the model's own optimum
/// sits microns outside the true constraint and the strict boundary test refuses
/// it every time.
fn search_along(
    context: &ExactContext<'_, '_>,
    x: &[(f64, f64, f64)],
) -> Result<Searched, String> {
    let mut validations = 0usize;
    let mut contract_only_accepts = 0usize;
    let mut best: Option<(f64, f64)> = None;
    let mut smallest_rejected = f64::INFINITY;
    let mut full_step_exact_valid = false;
    let mut full_step_rejection = None;

    for &scale in &BLOCK_SCALES {
        validations += 1;
        match evaluate_scale(context, x, scale, &mut contract_only_accepts)? {
            ScaleOutcome::Accepted(depth_mm) => {
                if scale == 1.0 {
                    full_step_exact_valid = true;
                }
                best = Some((scale, depth_mm));
                break;
            }
            ScaleOutcome::Refused(rejection) => {
                if scale == 1.0 {
                    full_step_rejection = Some(rejection);
                }
                smallest_rejected = smallest_rejected.min(scale);
            }
        }
    }

    let (mut scale, mut depth_mm) = best.ok_or_else(|| {
        "contact block: even a zero-length step failed the composite gate, so the \
         layout this round started from is not an acceptable parent"
            .to_owned()
    })?;

    if smallest_rejected.is_finite() {
        let mut low = scale;
        let mut high = smallest_rejected;
        for _ in 0..BLOCK_BISECTIONS {
            let middle = 0.5 * (low + high);
            validations += 1;
            match evaluate_scale(context, x, middle, &mut contract_only_accepts)? {
                ScaleOutcome::Accepted(candidate_depth) => {
                    if candidate_depth < depth_mm {
                        scale = middle;
                        depth_mm = candidate_depth;
                    }
                    low = middle;
                }
                ScaleOutcome::Refused(_) => high = middle,
            }
        }
    }

    Ok(Searched {
        scale,
        depth_mm,
        validations,
        full_step_exact_valid,
        full_step_rejection,
        contract_only_accepts,
    })
}

/// Runs the operator on one parent.
///
/// Returns the decomposition always, and the moved layout only when the whole
/// call ended strictly shallower than the parent under the exact validator.
/// A call that finds nothing is not an error: it is the measurement.
pub fn contact_block_proposal<'p>(
    pieces: &[GeneralFastPiece<'p>],
    placements: &[GeneralFastPlacement],
    settings: GeneralFastSettings,
    block_settings: ContactBlockSettings,
) -> Result<(ContactBlockReport, Option<ContactBlockProposal>), String> {
    let started = std::time::Instant::now();
    block_settings.validate()?;
    if pieces.is_empty() || placements.is_empty() {
        return Err("contact block requires at least one placement".to_owned());
    }

    let pieces_by_id = pieces
        .iter()
        .enumerate()
        .map(|(index, piece)| (piece.id, index))
        .collect::<BTreeMap<_, _>>();
    let expansion_mm = collision_expansion_mm(settings);
    let edge_clearance_mm = effective_sheet_edge_clearance_mm(settings);
    let collision_inset_mm = collision_sheet_inset_mm(settings);
    let contracts = Contracts {
        material_pair_mm: settings.total_padding_mm + 2.0 * settings.flattening_sag_tolerance_mm,
        material_edge_mm: edge_clearance_mm + settings.flattening_sag_tolerance_mm,
        collision_edge_mm: collision_inset_mm,
    };
    if !contracts.material_pair_mm.is_finite()
        || !contracts.material_edge_mm.is_finite()
        || !expansion_mm.is_finite()
    {
        return Err("contact block requires a finite clearance contract".to_owned());
    }
    let validation_settings = PublicationValidationSettings {
        sheet_width_mm: settings.sheet_short_axis_mm,
        sheet_height_mm: settings.sheet_long_axis_mm,
        total_padding_mm: settings.total_padding_mm,
        sheet_edge_clearance_mm: settings.sheet_edge_clearance_mm,
        flattening_sag_tolerance_mm: settings.flattening_sag_tolerance_mm,
    };

    let build_all = |current: &[GeneralFastPlacement]| -> Result<Vec<Geometry>, String> {
        current
            .iter()
            .map(|placement| {
                let index = pieces_by_id
                    .get(placement.piece_id.as_str())
                    .ok_or_else(|| format!("unknown piece id `{}`", placement.piece_id))?;
                let piece = pieces[*index];
                build_geometry(piece.polygon, placement, expansion_mm, piece.allow_rotation)
                    .ok_or_else(|| {
                        format!("could not build a geometry for `{}`", placement.piece_id)
                    })
            })
            .collect()
    };

    let parent_exact = exact_placements(pieces, &pieces_by_id, placements)?;
    let parent_depth_mm = raw_source_long_axis_depth_mm(&parent_exact, edge_clearance_mm)
        .map_err(|error| error.message().to_owned())?;

    let mut current: Vec<GeneralFastPlacement> = placements.to_vec();
    let mut current_depth_mm = parent_depth_mm;
    let mut trust_radius_mm = block_settings.trust_radius_mm;
    let mut rounds: Vec<BlockRound> = Vec::new();
    let mut rounds_accepted = 0usize;
    let mut solves = 0usize;
    let mut validations_total = 0usize;
    let mut rows_total = 0usize;
    let mut moved_pieces_total: Vec<bool> = vec![false; placements.len()];

    for round_index in 0..block_settings.rounds {
        let geometries = build_all(&current)?;
        let strip_bound_mm = geometries
            .iter()
            .map(|geometry| geometry.collision.outer_bounds.max_y)
            .fold(f64::NEG_INFINITY, f64::max)
            + collision_inset_mm;
        if !strip_bound_mm.is_finite() {
            return Err("contact block could not measure a strip bound".to_owned());
        }

        // The depth ladder: every piece's own contribution to the publication
        // measure, deepest first. The setter is the first, and the seed band is
        // everything within the action's own reach of it.
        let mut ladder: Vec<(usize, f64)> = geometries
            .iter()
            .enumerate()
            .map(|(index, geometry)| (index, piece_depth_mm(geometry, edge_clearance_mm)))
            .collect();
        ladder.sort_by(|left, right| {
            right
                .1
                .partial_cmp(&left.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(left.0.cmp(&right.0))
        });
        let seed_band_mm = SEED_BAND_TRUST_MULTIPLE * trust_radius_mm;
        let depth_band_pieces = ladder
            .iter()
            .filter(|(_, depth)| ladder[0].1 - depth <= seed_band_mm)
            .count();
        let band_mm = block_settings.band_trust_multiple * trust_radius_mm;

        // One `BlockRound` is published per round, not per seed attempt: a
        // round is one rebuild of the geometry and the contact graph, which is
        // the unit the trust region and the census are both denominated in. A
        // round that tries three seeds and takes the third records the third;
        // one that tries three and takes none records the last refusal. The
        // *cost* counters are not summarised that way - `validations_total`,
        // `rows_total` and `solves` accumulate every attempt - so the price of a
        // multi-seed round is never under-reported even though its round record
        // names one seed.
        let mut round: Option<BlockRound> = None;
        let mut accepted: Option<(Vec<GeneralFastPlacement>, Vec<usize>)> = None;

        for seed_rank in 0..block_settings.seeds.min(depth_band_pieces.max(1)) {
            let Some(&(seed, _)) = ladder.get(seed_rank) else {
                break;
            };
            let (block, edges) = grow_block(
                &geometries,
                &contracts,
                seed,
                band_mm,
                block_settings.max_block_pieces,
            );
            let block_names: Vec<String> = block
                .iter()
                .map(|&piece| current[piece].piece_id.clone())
                .collect();
            let named_edges: Vec<ContactEdge> = edges
                .iter()
                .map(|edge| ContactEdge {
                    first: current[edge.first].piece_id.clone(),
                    second: current[edge.second].piece_id.clone(),
                    slack_mm: edge.slack_mm,
                    gate: edge.gate,
                })
                .collect();
            // The ceiling: the deepest piece the block does not contain.
            let headroom_mm = ladder
                .iter()
                .find(|(piece, _)| !block.contains(piece))
                .map(|(_, depth)| current_depth_mm - depth)
                .unwrap_or(f64::INFINITY);

            let mut skeleton = BlockRound {
                round: round_index,
                trust_radius_mm,
                parent_depth_mm: current_depth_mm,
                setter: current[seed].piece_id.clone(),
                block: block_names,
                edges: named_edges,
                depth_band_pieces,
                seed_rank,
                headroom_mm,
                rows: 0,
                rows_by_family: BTreeMap::new(),
                delta_rows: 0,
                model_upper_mm: None,
                model_lower_mm: None,
                full_step_exact_valid: false,
                full_step_rejection: None,
                contract_only_accepts: 0,
                scale: 0.0,
                validations: 0,
                validated_delta_mm: 0.0,
                max_abs_dtheta_deg: 0.0,
                max_abs_translation_mm: 0.0,
                refusal: Some("no-component"),
            };
            if block.len() < 2 {
                round = Some(skeleton);
                continue;
            }

            let slot_of: BTreeMap<usize, usize> = block
                .iter()
                .enumerate()
                .map(|(slot, &piece)| (piece, slot))
                .collect();
            let theta_caps: Vec<f64> = block
                .iter()
                .map(|&piece| {
                    if geometries[piece].rotatable {
                        trust_radius_mm / geometries[piece].reach_mm
                    } else {
                        0.0
                    }
                })
                .collect();
            let rows = build_block_rows(
                &geometries,
                &block,
                &slot_of,
                &contracts,
                settings,
                current_depth_mm,
                strip_bound_mm,
                edge_clearance_mm,
                collision_inset_mm,
                trust_radius_mm,
                &theta_caps,
            );
            skeleton.rows = rows.len();
            rows_total += rows.len();
            for row in &rows {
                *skeleton
                    .rows_by_family
                    .entry(format!("{:?}", row.family))
                    .or_insert(0) += 1;
            }
            let mut delta_rows = Vec::new();
            let mut other_rows = Vec::new();
            for (index, row) in rows.iter().enumerate() {
                if row.family == RowFamily::MaterialDepth {
                    delta_rows.push(index);
                } else {
                    other_rows.push(index);
                }
            }
            skeleton.delta_rows = delta_rows.len();
            if delta_rows.is_empty() {
                skeleton.refusal = Some("no-priced-row");
                round = Some(skeleton);
                continue;
            }

            solves += 1;
            let solved = solve_block(
                &rows,
                &delta_rows,
                &other_rows,
                block.len(),
                trust_radius_mm,
                &theta_caps,
                block_settings.iterations,
            );
            skeleton.model_lower_mm = solved.model_lower_mm;
            skeleton.model_upper_mm = solved.model_upper_mm;
            if let (Some(lower), Some(upper)) = (solved.model_lower_mm, solved.model_upper_mm) {
                if lower > upper {
                    return Err(format!(
                        "contact block bracket inverted at round {round_index}: \
                         outward-rounded lower {lower} exceeds outward-rounded upper {upper}"
                    ));
                }
            }
            if solved.model_upper_mm.is_some_and(|upper| upper <= 0.0) {
                skeleton.refusal = Some("model-blocked");
                round = Some(skeleton);
                continue;
            }

            let context = ExactContext {
                pieces,
                pieces_by_id: &pieces_by_id,
                placements: &current,
                geometries: &geometries,
                block: &block,
                fast_settings: settings,
                validation_settings,
                edge_clearance_mm,
            };
            let mut best: Option<(Searched, &Vec<(f64, f64, f64)>)> = None;
            let mut spent = 0usize;
            for candidate in [&solved.feasible, &solved.objective] {
                let searched = search_along(&context, candidate)?;
                spent += searched.validations;
                let better = best
                    .as_ref()
                    .is_none_or(|(incumbent, _)| searched.depth_mm < incumbent.depth_mm);
                if better {
                    best = Some((searched, candidate));
                }
            }
            let (searched, winner) = best.expect("both directions are always searched");
            skeleton.validations = spent;
            validations_total += spent;
            skeleton.scale = searched.scale;
            skeleton.full_step_exact_valid = searched.full_step_exact_valid;
            skeleton.full_step_rejection = searched.full_step_rejection.clone();
            skeleton.contract_only_accepts = searched.contract_only_accepts;
            let applied: Vec<(f64, f64, f64)> = winner
                .iter()
                .map(|slot| {
                    (
                        searched.scale * slot.0,
                        searched.scale * slot.1,
                        searched.scale * slot.2,
                    )
                })
                .collect();
            skeleton.max_abs_dtheta_deg = applied
                .iter()
                .fold(0.0f64, |best, slot| best.max(slot.2.abs()))
                .to_degrees();
            skeleton.max_abs_translation_mm = applied
                .iter()
                .fold(0.0f64, |best, slot| best.max(slot.0.hypot(slot.1)));
            skeleton.validated_delta_mm = current_depth_mm - searched.depth_mm;

            if skeleton.validated_delta_mm > 0.0 {
                skeleton.refusal = None;
                accepted = Some((
                    apply_block(&current, &geometries, &block, winner, searched.scale),
                    block.clone(),
                ));
                current_depth_mm = searched.depth_mm;
                round = Some(skeleton);
                break;
            }
            skeleton.refusal = Some(if searched.full_step_exact_valid {
                "no-depth-gain"
            } else {
                "exact-rejected"
            });
            round = Some(skeleton);
        }

        let Some(round) = round else {
            break;
        };
        rounds.push(round);
        match accepted {
            Some((placements, block)) => {
                current = placements;
                for piece in block {
                    moved_pieces_total[piece] = true;
                }
                rounds_accepted += 1;
                // A productive round earns the full radius back: the geometry
                // it linearized at is gone and the next one is measured fresh.
                trust_radius_mm = block_settings.trust_radius_mm;
            }
            None => {
                // Trust-region contraction. A refused step is the model
                // over-reaching, and the answer to over-reach is a smaller
                // region, not a different component.
                trust_radius_mm *= 0.5;
                if trust_radius_mm
                    < block_settings.trust_radius_mm * TRUST_CONTRACTION_FLOOR
                {
                    break;
                }
            }
        }
    }

    let delta_mm = parent_depth_mm - current_depth_mm;
    let report = ContactBlockReport {
        settings: block_settings,
        piece_count: placements.len(),
        parent_depth_mm,
        final_depth_mm: current_depth_mm,
        delta_mm,
        rounds,
        rounds_accepted,
        solves,
        validations: validations_total,
        rows_total,
        elapsed_ms: started.elapsed().as_secs_f64() * 1_000.0,
    };
    let proposal = (delta_mm > 0.0).then(|| ContactBlockProposal {
        placements: current,
        published_depth_mm: report.final_depth_mm,
        delta_mm,
        moved_pieces: moved_pieces_total.iter().filter(|moved| **moved).count(),
        validations: validations_total,
    });
    Ok((report, proposal))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::general_polygon::{PolygonRegion, PolygonRing, PolygonSet, RingRole};

    fn rectangle(width_mm: f64, height_mm: f64) -> PolygonSet {
        PolygonSet {
            regions: vec![PolygonRegion {
                outer: PolygonRing::new(
                    vec![
                        IrregularPoint::new(0.0, 0.0),
                        IrregularPoint::new(width_mm, 0.0),
                        IrregularPoint::new(width_mm, height_mm),
                        IrregularPoint::new(0.0, height_mm),
                    ],
                    RingRole::Outer,
                )
                .unwrap(),
                holes: Vec::new(),
            }],
        }
    }

    fn settings() -> GeneralFastSettings {
        GeneralFastSettings {
            sheet_short_axis_mm: 200.0,
            sheet_long_axis_mm: 200.0,
            total_padding_mm: 5.0,
            sheet_edge_clearance_mm: Some(5.0),
            clearance_safety_margin_mm: 0.0,
            flattening_sag_tolerance_mm: 0.25,
            search_offset_allowance_mm: 0.0,
            ..GeneralFastSettings::deterministic_test(200.0, 200.0)
        }
    }

    fn probe_settings() -> ContactBlockSettings {
        ContactBlockSettings {
            trust_radius_mm: 0.5,
            iterations: 64,
            max_block_pieces: 4,
            rounds: 3,
            seeds: 2,
            band_trust_multiple: 2.0,
        }
    }

    #[test]
    fn a_block_of_two_stacked_pieces_is_found_and_the_deeper_one_seeds_it() {
        let square = rectangle(20.0, 20.0);
        let pieces = vec![
            GeneralFastPiece {
                id: "low",
                polygon: &square,
                allow_rotation: true,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "high",
                polygon: &square,
                allow_rotation: true,
                allow_mirror: false,
            },
        ];
        let placements = vec![
            GeneralFastPlacement {
                piece_id: "low".to_owned(),
                rotation_deg: 0.0,
                mirrored: false,
                translate_short_axis: 20.0,
                translate_long_axis: 10.0,
            },
            GeneralFastPlacement {
                piece_id: "high".to_owned(),
                rotation_deg: 0.0,
                mirrored: false,
                translate_short_axis: 20.0,
                translate_long_axis: 35.5,
            },
        ];
        let (report, _) =
            contact_block_proposal(&pieces, &placements, settings(), probe_settings())
                .expect("the probe layout is legal");
        let first = report.rounds.first().expect("at least one round runs");
        assert_eq!(first.setter, "high", "the deeper piece seeds the block");
        assert!(
            first.block.len() >= 2,
            "the stacked pair is one component: {:?}",
            first.block
        );
        assert!(
            first.headroom_mm > 0.0,
            "a two-piece block in a two-piece layout has room down to the other piece"
        );
    }

    #[test]
    fn the_reported_delta_is_the_exactly_validated_one() {
        let square = rectangle(20.0, 20.0);
        let pieces = vec![
            GeneralFastPiece {
                id: "a",
                polygon: &square,
                allow_rotation: true,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "b",
                polygon: &square,
                allow_rotation: true,
                allow_mirror: false,
            },
        ];
        let placements = vec![
            GeneralFastPlacement {
                piece_id: "a".to_owned(),
                rotation_deg: 0.0,
                mirrored: false,
                translate_short_axis: 20.0,
                translate_long_axis: 10.0,
            },
            GeneralFastPlacement {
                piece_id: "b".to_owned(),
                rotation_deg: 0.0,
                mirrored: false,
                translate_short_axis: 20.0,
                translate_long_axis: 40.0,
            },
        ];
        let (report, proposal) =
            contact_block_proposal(&pieces, &placements, settings(), probe_settings())
                .expect("the probe layout is legal");
        assert!(
            report.delta_mm >= 0.0,
            "the parent is always available as a floor"
        );
        if let Some(proposal) = proposal {
            // Whatever came back is the publication measure on placements the
            // exact validator accepted, so re-measuring reproduces it.
            let by_id = pieces
                .iter()
                .enumerate()
                .map(|(index, piece)| (piece.id, index))
                .collect::<BTreeMap<_, _>>();
            let exact = exact_placements(&pieces, &by_id, &proposal.placements)
                .expect("the proposal names known pieces");
            let clearance = effective_sheet_edge_clearance_mm(settings());
            let measured = raw_source_long_axis_depth_mm(&exact, clearance)
                .expect("the proposal is measurable");
            assert!(
                (measured - proposal.published_depth_mm).abs() < 1e-9,
                "reported {} vs measured {measured}",
                proposal.published_depth_mm
            );
            assert!(
                validate_publication(
                    &exact,
                    PublicationValidationSettings {
                        sheet_width_mm: 200.0,
                        sheet_height_mm: 200.0,
                        total_padding_mm: 5.0,
                        sheet_edge_clearance_mm: Some(5.0),
                        flattening_sag_tolerance_mm: 0.25,
                    }
                )
                .is_ok(),
                "the proposal is exactly valid"
            );
        }
    }

    #[test]
    fn a_call_is_a_deterministic_function_of_its_inputs() {
        let square = rectangle(20.0, 20.0);
        let pieces = vec![
            GeneralFastPiece {
                id: "a",
                polygon: &square,
                allow_rotation: true,
                allow_mirror: false,
            },
            GeneralFastPiece {
                id: "b",
                polygon: &square,
                allow_rotation: true,
                allow_mirror: false,
            },
        ];
        let placements = vec![
            GeneralFastPlacement {
                piece_id: "a".to_owned(),
                rotation_deg: 0.0,
                mirrored: false,
                translate_short_axis: 20.0,
                translate_long_axis: 10.0,
            },
            GeneralFastPlacement {
                piece_id: "b".to_owned(),
                rotation_deg: 0.0,
                mirrored: false,
                translate_short_axis: 20.0,
                translate_long_axis: 35.5,
            },
        ];
        let left = contact_block_proposal(&pieces, &placements, settings(), probe_settings())
            .expect("legal")
            .0;
        let right = contact_block_proposal(&pieces, &placements, settings(), probe_settings())
            .expect("legal")
            .0;
        assert_eq!(
            serde_json::to_string(&left.rounds).unwrap(),
            serde_json::to_string(&right.rounds).unwrap(),
            "two calls on the same input must be the same document"
        );
    }

    #[test]
    fn the_settings_refuse_a_block_of_one() {
        let mut probe = probe_settings();
        probe.max_block_pieces = 1;
        assert!(probe.validate().is_err());
    }
}
