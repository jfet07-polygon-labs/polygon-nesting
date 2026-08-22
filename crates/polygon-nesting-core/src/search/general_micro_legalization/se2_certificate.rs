//! The SE(2) rigidity certificate.
//!
//! # The question
//!
//! [`super::global_legalize`] asks a translation-only program whether the
//! current contact front can be pushed open at all. This module asks a
//! narrower and much more useful question of the same front, with one more
//! degree of freedom per piece:
//!
//! > **How much shallower can the published depth be made**, by moving every
//! > piece within a trust box — `|dx|, |dy| <= trust` and a bounded rotation
//! > about its own centre — *without breaking any other constraint*?
//!
//! # What the previous certificate got wrong, and what changed
//!
//! The branch this replaces (`sol5/se2-rigidity-certificate`) solved
//!
//! ```text
//! max_x min_i (a_i . x - rhs_i)
//! ```
//!
//! which asks to open **every** row by the same amount at once. Sol review 6
//! §3: "reducing the depth by 0.422 mm does not require simultaneously opening
//! every pair contact, the left edge, the bottom and the short edge by
//! 0.422 mm." That program's answer is a statement about the least slack
//! anywhere in the layout, not about the depth, and it is bounded above by the
//! tightest contact in the packing no matter how much room the depth direction
//! has. The whole point of the diagnostic was lost.
//!
//! The program here introduces `delta` and puts it **only on the rows that
//! measure the depth**:
//!
//! ```text
//! max delta
//!   pair and non-depth boundary rows:   a_i . x >= rhs_i
//!   depth rows:                         a_i . x >= rhs_i + delta
//!   x in Box
//! ```
//!
//! Six further corrections from the same review, each of which changes what
//! the certificate reports:
//!
//! 1. **Material depth and collision strip are separated.** The old code
//!    clamped one `sheet_long_axis_mm` onto both gates' containment rows, so
//!    the miter join of a collision envelope — which reaches measurably
//!    further out than the material outline at a sharp corner — was measured
//!    against the *material's* published depth. Two of four parents then had
//!    to have their bound "calibrated" upward by 0.15–0.28 mm before the
//!    program would call the state feasible at all. That recalibration was a
//!    red flag on the formulation, not a property of the layout. Here the two
//!    are different numbers measured off different geometry: the material
//!    depth family is built against the publication measure itself
//!    ([`raw_source_long_axis_depth_mm`], `max_y + edge clearance` over outer
//!    rings), and the envelope strip family against the engine's own
//!    `tight_strip_depth` quantity (`max_y` of the collision envelopes plus
//!    the collision inset). Nothing is calibrated; both are read off the
//!    parent. [`Se2Program::StripCoupled`] then reports what happens when the
//!    strip is *also* required to shrink by `delta`, so the difference between
//!    the two readings is visible instead of being papered over.
//! 2. **Boundary rows carry the rotational coefficient.** The old `Axis` rows
//!    had `theta = 0`, so a rotation could open a pair contact without paying
//!    for the extreme vertex it drives into the sheet edge — which
//!    *overestimates* the rotational room. Every boundary row here carries
//!    `a_theta = n . J(p - c)` for its own vertex `p`, and there is a row per
//!    vertex that can become extreme inside the box (see
//!    [`push_boundary_rows`]), not just for the one that is extreme now.
//! 3. **The witness survives the touch.** `measure_approach` used to discard
//!    its witness pair exactly when the outlines met — so every *active*
//!    contact, the rows that are actually holding the front, got a zero
//!    rotational coefficient. It now returns the contact point.
//! 4. **Envelope rows exist for reachable pairs.** The production row builder
//!    opens a collision-envelope row only for pairs that already overlap. A
//!    legal pair that can be driven into collision inside the trust box had no
//!    row at all, so the program was free to drive it there. Here every pair
//!    within the guard band gets an envelope row.
//! 5. **The guard band accounts for rotation.** `2 * trust` is the reach of
//!    two translations. Each piece here also has a rotation that can move one
//!    of its own vertices by up to another `trust`, so the band is
//!    `2 * trust + Theta_i * reach_i + Theta_j * reach_j`.
//! 6. **The result carries the vector, and the vector is validated exactly.**
//!    A lower bound nobody can apply is not a constructive lower bound. The
//!    best `(dx, dy, dtheta)` per piece is applied to the parent's placements
//!    exactly — not through the linearization — and run through
//!    [`validate_publication`] and the publication depth measure. The number
//!    reported as the achievable depth reduction is that exactly-validated
//!    one.
//!
//!    The model supplies the *direction* and the exact validator decides the
//!    *length*: see [`WitnessOutcome`]. This is not a convenience. Because the
//!    rows are relaxed outward to keep the upper bound valid (below), the
//!    model's own optimum sits a few microns outside the true constraint and
//!    the validator's strict boundary test rejects it every time — measured,
//!    on the 155.422 parent at a 1 mm trust radius. A line search along the
//!    model's direction turns that into a number instead of a shrug.
//!
//! # What the two bounds mean, precisely
//!
//! These are three different numbers and the report keeps them apart:
//!
//! * `lp.primal_lower_mm` and `lp.dual_upper_mm` bracket the optimum of the
//!   **linearized program above**, in real arithmetic, evaluated in `f64` with
//!   an outward rounding allowance (see [`RoundedSum`]). `primal_lower` comes
//!   from a feasible point, `dual_upper` from weak duality on a dual-feasible
//!   weighting; both are valid whatever the solver's convergence did, so a
//!   wide gap means "run more iterations", never "a bound might be wrong".
//!   The bracket is asserted, not clamped: if the outward-rounded lower ever
//!   exceeds the outward-rounded upper, that is a bug in this file and it is
//!   returned as an error rather than hidden behind a `.max(0.0)`.
//! * `witness.delta_mm` is the depth reduction the returned vector **actually
//!   achieves**, measured by the publication measure on the moved placements
//!   after [`validate_publication`] accepted them. This is a real lower bound
//!   on what SE(2) motion inside the box can do, and it owes nothing to the
//!   linearization.
//!
//! The linearization is a first-order model of the geometry, so
//! `lp.dual_upper_mm` bounds *that model*, not the geometry. The boundary and
//! depth rows are relaxed by the exact second-order rotation term
//! ([`rotation_chord_slack_mm`]) so that for those rows the model really does
//! contain the truth; the pair rows are a first-order contact model and are
//! not claimed to. This is why the achievable number is the validated one and
//! the model number is labelled as a model number.
//!
//! One consequence deserves stating plainly, because it is the difference
//! between this certificate and a proof. `lp.primal_lower_mm` is the best
//! objective reached by a point feasible for the **relaxed** rows. Relaxed
//! feasibility does not imply real feasibility, so `primal_lower_mm` is *not*
//! a lower bound on anything physical — it is one end of a bracket on the
//! model, and it is named and documented as such. The only number in this
//! document that lower-bounds the achievable depth reduction is
//! `witness.delta_mm`, and the only number that upper-bounds it is
//! `lp.dual_upper_mm`, and those two do not meet.

use std::collections::BTreeMap;

use serde::Serialize;

use super::{build_outline, measure_approach, Contracts, Outline};
use crate::domain::IrregularPoint;
use crate::geometry::general_polygon::PolygonSet;
use crate::search::general_fast::{
    collision_expansion_mm, collision_sheet_inset_mm, effective_sheet_edge_clearance_mm,
    GeneralFastPiece, GeneralFastPlacement, GeneralFastSettings,
};
use crate::validation::general_polygon::{
    raw_source_long_axis_depth_mm, validate_publication, GeneralPlacement,
    PublicationValidationSettings,
};

/// Penalty weights tried in sequence by [`solve_program`].
///
/// The primal ascent maximizes the exact penalty
/// `min_{depth} residual - rho * max(0, worst non-depth violation)`. `rho`
/// has to exceed the optimal dual multipliers of the constraints it prices for
/// the penalty to be exact; rather than estimate that, the solve runs the whole
/// ladder and keeps the best *feasible* point any of them reached. Every point
/// kept is feasible for every non-depth row, so a badly chosen `rho` can only
/// cost tightness, never validity.
const PENALTY_LADDER: [f64; 4] = [1.0, 4.0, 16.0, 64.0];

/// How often the dual weighting is evaluated, in primal iterations.
///
/// Every evaluation is a full pass over the rows, so doing it every iteration
/// doubles the solve for a bound that moves slowly. Each evaluation is valid on
/// its own, so this only controls how tight the reported upper bound gets.
const DUAL_EVERY: usize = 8;

/// Relative floor under a piece's rotation cap, so a piece whose outline is
/// degenerate at grid scale cannot produce an unbounded angular box.
const MIN_REACH_MM: f64 = 0.001;

/// Which rows the objective's `delta` sits on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Se2Program {
    /// `delta` on the published **material** depth rows only. The collision
    /// envelope's strip rows are ordinary constraints held at the parent's own
    /// strip bound: the envelopes may stay exactly where they are.
    ///
    /// This is the program that answers "how much shallower can the published
    /// number be", which is the question the record line is asking.
    DepthOnly,
    /// `delta` on the material depth rows **and** the collision strip rows: the
    /// envelope strip is required to shrink by the same amount. Strictly
    /// harder than [`Se2Program::DepthOnly`], and the gap between the two is
    /// the honest measure of how much the collision envelope's miter reach
    /// costs — the quantity the previous branch absorbed into a hand
    /// recalibration of the depth bound.
    StripCoupled,
}

/// Which motion each piece is allowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Se2Motion {
    /// `dtheta` pinned to zero everywhere: the program `global_legalize`
    /// already solves, restated with a `delta` objective so the two are
    /// comparable.
    TranslationOnly,
    /// Translation plus a bounded rotation per piece, for pieces the request
    /// actually allows to rotate.
    Se2,
}

/// Which family a row belongs to, for reporting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RowFamily {
    /// Two transformed source outlines against the pair clearance contract.
    MaterialPair,
    /// Two collision envelopes against each other.
    EnvelopePair,
    /// A material outer vertex against the published depth measure. Carries
    /// `delta` in both programs.
    MaterialDepth,
    /// A material outer vertex against one of the other three sheet edges.
    MaterialBoundary,
    /// A collision-envelope outer vertex against the parent's own strip bound.
    /// Carries `delta` only in [`Se2Program::StripCoupled`].
    EnvelopeStrip,
    /// A collision-envelope outer vertex against one of the other three inset
    /// sheet edges.
    EnvelopeBoundary,
}

impl RowFamily {
    /// Whether this family carries the objective's `delta` under `program`.
    fn carries_delta(self, program: Se2Program) -> bool {
        match self {
            RowFamily::MaterialDepth => true,
            RowFamily::EnvelopeStrip => program == Se2Program::StripCoupled,
            _ => false,
        }
    }
}

/// One row: `normal . t_first (- normal . t_second) + theta_first * dtheta_first
/// (+ theta_second * dtheta_second) >= rhs_mm`.
///
/// `theta_second` is stored already carrying its sign, so evaluation is a plain
/// sum over the four coefficients and never has to know the row's shape.
#[derive(Clone, Copy, Debug)]
struct Row {
    first: usize,
    second: Option<usize>,
    normal: (f64, f64),
    theta_first: f64,
    theta_second: f64,
    rhs_mm: f64,
    family: RowFamily,
}

impl Row {
    /// This row with its angular coefficients rescaled so that the program's
    /// box becomes a **cube of side `2 * trust` in every coordinate**.
    ///
    /// The angular unknown of piece `i` lives in `[-Theta_i, Theta_i]` while
    /// its translations live in `[-trust, trust]`, and `Theta_i` is smaller by
    /// the piece's own reach — a factor of a hundred or more on real geometry.
    /// A single scalar step size applied to that box is not a step size at all:
    /// with a lever arm of `10 mm` on the angular coefficient and
    /// `Theta = 0.09 rad`, the first `1 mm` step drives `dtheta` straight into
    /// its cap and every later step is still large enough to keep it pinned
    /// there, so the solve oscillates between the two caps and never visits
    /// the interior where the optimum sits. (Measured, before this fix: an
    /// isolated piece with a full millimetre of room reported 0.127 mm.)
    ///
    /// Substituting `dtheta = scale_i * u_i` with `scale_i = Theta_i / trust`
    /// puts `u_i` in `[-trust, trust]` alongside the translations, which is
    /// exactly the isotropy a single step size assumes. It is a change of
    /// variables, not of the program: the feasible set and the optimum are
    /// identical, and [`solve_program`] multiplies the scale back out before
    /// anyone sees the vector.
    fn scaled(&self, scales: &[f64]) -> Row {
        let mut row = *self;
        row.theta_first *= scales[self.first];
        if let Some(second) = self.second {
            row.theta_second *= scales[second];
        }
        row
    }

    /// `a_i . x`, with the running absolute sum needed for the outward
    /// rounding allowance.
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

    /// `a_i . x - rhs_i`, outward-rounded low: the value a *lower* bound may
    /// be built from.
    fn residual_low_mm(&self, x: &[(f64, f64, f64)]) -> f64 {
        let mut sum = self.value(x);
        sum.add(-self.rhs_mm);
        sum.low()
    }

    /// Accumulates `weight * a_i` into the objective vector `c`.
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

    /// One supergradient step: `x += step * a_i`.
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

/// A floating-point sum that carries a bound on its own rounding error.
///
/// Sol review 6 §3: the previous branch's evidence contained a `lower > upper`
/// bracket of a few ULPs and the code hid it behind `.max(0.0)`. Every quantity
/// a bound is built from is accumulated through this, which tracks
/// `sum |term|` alongside the sum. The standard running bound for `n`
/// floating-point additions is `n * eps * sum |term|` to first order; the
/// factor below is deliberately loose (`n + 2`) so the allowance covers the
/// products feeding each term as well as the additions.
///
/// [`RoundedSum::low`] and [`RoundedSum::high`] then give an interval that
/// really does contain the exact real value, so a bracket assertion on them is
/// meaningful rather than decorative.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct RoundedSum {
    sum: f64,
    abs_sum: f64,
    terms: usize,
}

impl RoundedSum {
    pub(super) fn add(&mut self, term: f64) {
        self.sum += term;
        self.abs_sum += term.abs();
        self.terms += 1;
    }

    /// The running sum itself, with no allowance applied. Only a caller that is
    /// about to fold this into another [`RoundedSum`] — and so will pay the
    /// allowance once, at the end — may read it.
    pub(super) fn value(&self) -> f64 {
        self.sum
    }

    pub(super) fn allowance(&self) -> f64 {
        (self.terms as f64 + 2.0) * f64::EPSILON * self.abs_sum
    }

    pub(super) fn low(&self) -> f64 {
        self.sum - self.allowance()
    }

    pub(super) fn high(&self) -> f64 {
        self.sum + self.allowance()
    }
}

/// One piece, at its parent pose, in both gates.
pub(super) struct Geometry {
    pub(super) material: Outline,
    pub(super) collision: Outline,
    /// Outer-ring vertices of the transformed material outline. The published
    /// depth measure and the sheet containment check both read outer rings
    /// only — a hole is inside its own outer ring by construction — so these
    /// are the vertices the boundary families are built from.
    pub(super) material_outer: Vec<IrregularPoint>,
    pub(super) collision_outer: Vec<IrregularPoint>,
    /// The rotation centre. The same point is used to build every rotational
    /// coefficient and to apply the resulting `dtheta` exactly, which is what
    /// makes the witness reproduce the linearization to first order.
    pub(super) centre: IrregularPoint,
    /// The farthest any vertex of *either* gate sits from `centre`. The
    /// rotation cap is `trust / reach`, so a rotation inside the box moves no
    /// vertex of either gate further than a translation inside the box may
    /// move the whole piece. That is what makes the pair guard band's
    /// rotational term a real bound rather than an estimate.
    pub(super) reach_mm: f64,
    /// Whether the request lets this piece rotate at all. A piece the request
    /// pins cannot be handed rotational freedom by a diagnostic: publishing
    /// the resulting placement would violate the request, so the witness would
    /// not be applicable — which is the whole point of returning one.
    pub(super) rotatable: bool,
}

/// The certificate for one parent.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Se2RigidityCertificate {
    pub piece_count: usize,
    pub rotatable_piece_count: usize,
    pub trust_radius_mm: f64,
    /// The parent's published depth, by the publication measure itself.
    pub published_depth_mm: f64,
    /// The parent's collision-envelope strip bound: `max_y` over the envelopes
    /// plus the collision inset, i.e. the engine's own `tight_strip_depth`.
    /// A *different* number from `published_depth_mm`, measured on different
    /// geometry — keeping the two apart is correction 1 of this rewrite.
    pub strip_bound_mm: f64,
    /// `strip_bound_mm - published_depth_mm`. The previous branch absorbed
    /// exactly this quantity into a hand recalibration of its depth bound.
    pub strip_excess_mm: f64,
    pub rows: usize,
    pub rows_by_family: BTreeMap<String, usize>,
    /// The **parent's own** worst residual `a_i . x - rhs_i` at `x = 0`, per
    /// family, outward-rounded down. Zero or positive means the parent
    /// satisfies every linearized row of that family; negative means the
    /// parent violates its own model, and by how much.
    ///
    /// This is the number the previous branch never printed. Two of its four
    /// parents came back infeasible, and rather than report that it raised the
    /// depth bound by 0.15–0.28 mm until the complaint stopped — which Sol
    /// review 6 §3 called a red flag on the formulation. With the material
    /// depth and the collision strip measured separately off the parent
    /// (correction 1), both depth families are feasible by construction, so
    /// anything negative here is a *pair* row and says the first-order contact
    /// model disagrees with the exact gate at grid scale. Either way it is
    /// published, not calibrated away.
    pub parent_worst_residual_mm: BTreeMap<String, f64>,
    pub reach_min_mm: f64,
    pub reach_max_mm: f64,
    /// Largest rotation any piece is allowed, in degrees.
    pub theta_cap_max_deg: f64,
    /// The four programs: {depth-only, strip-coupled} x {translation, SE(2)}.
    pub programs: Vec<Se2ProgramResult>,
    /// The verdict, over the headline program (depth-only, SE(2)).
    pub verdict: &'static str,
    /// The reference the verdict compares against, when one was supplied.
    pub reference_mm: Option<f64>,
}

/// One program's bracket, witness and verdict.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Se2ProgramResult {
    pub program: Se2Program,
    pub motion: Se2Motion,
    /// Rows carrying `delta` under this program.
    pub delta_rows: usize,
    pub lp: LpBracket,
    pub witness: Option<WitnessOutcome>,
    /// Why no witness was produced, when there is none.
    pub witness_error: Option<String>,
    pub verdict: &'static str,
}

/// The bracket on the linearized program's optimum.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LpBracket {
    /// Best `delta` reached by a point feasible for every non-`delta` row,
    /// outward-rounded **down**.
    ///
    /// `None` when the solve never visited a feasible point at all, which is a
    /// different statement from "the best `delta` was zero" and is not allowed
    /// to masquerade as one. It happens when the parent itself violates one of
    /// its own linearized rows — read `parent_worst_residual_mm` to see which
    /// family — and it is exactly the situation the old branch answered by
    /// hand-calibrating its depth bound upward until the complaint went away.
    pub primal_lower_mm: Option<f64>,
    /// Weak-duality bound from a dual-feasible weighting, outward-rounded
    /// **up**. `None` when no row carries `delta`, where the objective is
    /// unpriced and no finite bound exists.
    pub dual_upper_mm: Option<f64>,
    /// `dual_upper_mm - primal_lower_mm`, when both exist.
    pub gap_mm: Option<f64>,
    /// Whether the solve ever reached a point feasible for every non-`delta`
    /// row. `false` makes `primal_lower_mm` `None` and is reported rather than
    /// smoothed over.
    pub primal_feasible: bool,
    pub iterations: usize,
    /// Always the same string. It is emitted into every raw document on
    /// purpose: these are real-arithmetic bounds computed in `f64` with an
    /// outward allowance, not exact rational certificates, and a reader of the
    /// raw JSON should not have to find this README to learn that.
    pub arithmetic: &'static str,
}

/// What happened when the model's vector was applied to the parent for real.
///
/// # Why there is a scale here at all
///
/// The model's rows are *relaxed* outward by [`rotation_chord_slack_mm`], which
/// is what makes [`LpBracket::dual_upper_mm`] a valid upper bound on the rotated
/// geometry. The price is that the model's optimum sits up to that slack
/// **outside** the true constraint, and the solver always drives the binding
/// rows to equality — so the full-length vector lands a few microns past a sheet
/// edge and [`validate_publication`], whose boundary test is a strict
/// inequality, rejects it. Measured on the 155.422 parent at a 1 mm trust
/// radius: the model claimed 0.854 mm and the exact validator answered
/// "piece ... crosses the sheet clearance boundary".
///
/// Reporting that as "witness rejected" and stopping would leave the
/// certificate with no constructive lower bound at all, which is the specific
/// defect Sol review 6 §3 raised against the old branch ("the result does not
/// keep the best `(dx, dy, dtheta)`, so the constructive lower bound is not
/// usable"). So the vector is **line-searched against the exact validator**:
/// the direction comes from the model, the length is decided by the authority,
/// and `alpha = 0` — the parent, valid by assumption — is always available as a
/// floor. `delta_mm` is therefore always present and always exactly validated.
///
/// The full-length attempt is kept beside it, because *that* is the diagnostic:
/// `full_vector_exact_valid = false` with a large `lp.primal_lower_mm` is the
/// signature of a model writing cheques its geometry will not cash.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WitnessOutcome {
    /// Did [`validate_publication`] accept the layout at the reported `scale`?
    /// Always `true`: `scale = 0` reproduces the parent, and a parent this
    /// diagnostic is run on is legal.
    pub exact_valid: bool,
    /// The fraction of the model's vector that was actually applied, in
    /// `[0, 1]`. `1.0` means the model's own step survived the exact validator
    /// intact.
    pub scale: f64,
    /// Which candidate ray won: `modelFeasible` (the best point feasible for
    /// the model's own relaxed rows) or `modelObjective` (the best-objective
    /// point, feasible or not). See [`witness_outcome`].
    pub direction: &'static str,
    /// Did the **full-length** model vector validate? This is the honest
    /// measure of how far the linearization can be trusted on this parent.
    pub full_vector_exact_valid: bool,
    /// Why the full-length vector was rejected, when it was.
    pub full_vector_rejection: Option<String>,
    /// The publication measure on the moved layout, at `scale`.
    pub published_depth_mm: Option<f64>,
    /// `parent published depth - moved published depth`, at `scale`.
    /// **This is the certificate's only constructive lower bound** on the
    /// achievable depth reduction. It owes nothing to the linearization: it is
    /// the publication measure, run on placements the exact validator accepted.
    pub delta_mm: Option<f64>,
    /// How many exact validations the line search spent.
    pub validations: usize,
    /// Largest `|dtheta|` in the **applied** (scaled) vector, in degrees.
    pub max_abs_dtheta_deg: f64,
    /// Largest `|(dx, dy)|` in the applied vector. Compare against the trust
    /// radius with care: the box is `|dx|, |dy| <= trust`, so its corner has
    /// Euclidean norm `sqrt(2) * trust`.
    pub max_abs_translation_mm: f64,
    /// How many pieces the applied vector actually moves.
    pub moved_pieces: usize,
    /// The applied vector itself: `(piece id, dx, dy, dtheta in degrees)`.
    pub vector: Vec<(String, f64, f64, f64)>,
}

/// The exact chord error of the first-order rotation model, for a point at
/// radius `reach_mm` rotated by up to `theta_cap` radians.
///
/// Rotating `r` by `theta` moves it to `r + theta * J(r) + (R(theta) - I -
/// theta J) r`, and the residual term has norm `|r| * sqrt(2 - 2 cos theta -
/// ... )`; bounding it by the elementary `|r| * theta^2 / 2` is both correct
/// and tight enough at the angles in play (`theta <= trust / reach`, so
/// `reach * theta^2 / 2 <= trust^2 / (2 * reach)` — at a 1 mm trust radius on
/// a 100 mm piece, five microns).
///
/// Relaxing every boundary and depth row by this makes those rows a genuine
/// **relaxation** of the exact rotated geometry, so the upper bound really does
/// bound them. The pair rows are a first-order contact model and get no such
/// treatment, which is stated rather than glossed: the achievable number this
/// certificate reports is the exactly-validated one.
pub(super) fn rotation_chord_slack_mm(reach_mm: f64, theta_cap: f64) -> f64 {
    0.5 * reach_mm * theta_cap * theta_cap
}

/// `n . J(p - c)`, the rotational coefficient of a contact at `p` on a piece
/// centred at `c` against a constraint with unit normal `n`.
///
/// `J` is the quarter-turn generator `J(x, y) = (-y, x)`: rotating `p` about
/// `c` by `theta` moves it, to first order, by `theta * J(p - c)`, and the
/// constraint only feels the component of that motion along its own normal.
pub(super) fn rotation_coefficient(
    normal: (f64, f64),
    point: IrregularPoint,
    centre: IrregularPoint,
) -> f64 {
    let rx = point.x - centre.x;
    let ry = point.y - centre.y;
    normal.0 * (-ry) + normal.1 * rx
}

pub(super) fn build_geometry(
    polygon: &PolygonSet,
    placement: &GeneralFastPlacement,
    expansion_mm: f64,
    rotatable: bool,
) -> Option<Geometry> {
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
    let material = build_outline(&transformed)?;
    let collision_outline = build_outline(&collision)?;
    let centre = material.centroid;

    let outer_of = |set: &PolygonSet| -> Vec<IrregularPoint> {
        set.regions
            .iter()
            .flat_map(|region| region.outer.source_points().iter().copied())
            .collect()
    };
    let material_outer = outer_of(&transformed);
    let collision_outer = outer_of(&collision);
    if material_outer.is_empty() || collision_outer.is_empty() {
        return None;
    }

    // The reach spans BOTH gates: a rotation moves the collision envelope's
    // vertices too, and they sit further out than the material's.
    let mut reach_mm: f64 = MIN_REACH_MM;
    for outline in [&material, &collision_outline] {
        for point in outline.rings.iter().flatten() {
            reach_mm = reach_mm.max((point.x - centre.x).hypot(point.y - centre.y));
        }
    }

    Some(Geometry {
        material,
        collision: collision_outline,
        material_outer,
        collision_outer,
        centre,
        reach_mm,
        rotatable,
    })
}

/// Applies `(dx, dy, dtheta)` to a placement **exactly**: a real rotation about
/// `centre`, not the linearization the program reasons with.
///
/// A placement maps a source point `s` to `R(r) M s + T`, so the piece's centre
/// sits at `C = R(r) M c_src + T`. Rotating the piece about `C` by `d` and then
/// translating by `(dx, dy)` gives `R(r + d) M s + T'` with
/// `T' = C + (dx, dy) - R(r + d) M c_src`, and `R(r + d) M c_src = R(d) (C -
/// T)` — so the new translation needs only the world centre, the old
/// translation and `d`, and never has to reach into the source frame.
pub(super) fn apply_se2(
    placement: &GeneralFastPlacement,
    centre: IrregularPoint,
    delta: (f64, f64, f64),
) -> GeneralFastPlacement {
    let (dx, dy, dtheta) = delta;
    let (sin, cos) = dtheta.sin_cos();
    let ex = centre.x - placement.translate_short_axis;
    let ey = centre.y - placement.translate_long_axis;
    GeneralFastPlacement {
        piece_id: placement.piece_id.clone(),
        rotation_deg: placement.rotation_deg + dtheta.to_degrees(),
        mirrored: placement.mirrored,
        translate_short_axis: centre.x + dx - (cos * ex - sin * ey),
        translate_long_axis: centre.y + dy - (sin * ex + cos * ey),
    }
}

/// Pushes one side's boundary rows for one piece and one gate.
///
/// `normal` is the inward normal of the side and `bound_mm` the value
/// `normal . p` must stay at or above. **One row per vertex that can become
/// the extreme one inside the box**, not one row for the vertex that is
/// extreme now: the extreme of a rotating outline is a maximum over vertices,
/// and linearizing it at the current argmax alone underestimates that maximum,
/// which would overestimate the room — the same direction of error Sol review 6
/// §3 flagged on the old `theta = 0` rows.
///
/// The domination test that prunes the rest is exact. Vertex `p`'s row is
/// implied by the current extreme `p*`'s row over the whole box exactly when
/// `n . p - n . p* >= Theta * |n . J(p - p*)|`: the translation part is common
/// to both rows and cancels, so only the rotational spread has to be covered.
/// With `Theta = 0` this keeps the single extreme vertex and the row set
/// reduces to the production program's, which is what makes the
/// translation-only column comparable to `global_legalize`.
#[allow(clippy::too_many_arguments)]
fn push_boundary_rows(
    rows: &mut Vec<Row>,
    piece: usize,
    vertices: &[IrregularPoint],
    centre: IrregularPoint,
    normal: (f64, f64),
    bound_mm: f64,
    theta_cap: f64,
    slack_mm: f64,
    family: RowFamily,
) {
    let along = |point: &IrregularPoint| normal.0 * point.x + normal.1 * point.y;
    let Some(extreme) = vertices.iter().copied().reduce(|best, point| {
        if along(&point) < along(&best) {
            point
        } else {
            best
        }
    }) else {
        return;
    };
    let extreme_along = along(&extreme);

    for point in vertices {
        let spread = along(point) - extreme_along;
        if spread > 0.0 {
            let rotational_spread = rotation_coefficient(
                normal,
                IrregularPoint::new(
                    point.x - extreme.x + centre.x,
                    point.y - extreme.y + centre.y,
                ),
                centre,
            )
            .abs();
            if spread >= theta_cap * rotational_spread {
                continue;
            }
        }
        rows.push(Row {
            first: piece,
            second: None,
            normal,
            theta_first: rotation_coefficient(normal, *point, centre),
            theta_second: 0.0,
            // `n . p + n . t + theta * a_theta >= bound`, relaxed outward by
            // the exact chord error of the first-order rotation model so this
            // row really does contain the rotated geometry.
            rhs_mm: bound_mm - along(point) - slack_mm,
            family,
        });
    }
}

/// Builds the whole row set once, at the widest rotation caps in play.
///
/// One row set serves all four programs. Rows the translation-only column does
/// not need are *implied* at `Theta = 0` rather than wrong, so sharing the set
/// costs a little solve time and buys the guarantee that the four columns are
/// the same program with different boxes — which is the only way their
/// comparison means anything.
#[allow(clippy::too_many_arguments)]
fn build_rows(
    geometries: &[Geometry],
    contracts: &Contracts,
    settings: GeneralFastSettings,
    published_depth_mm: f64,
    strip_bound_mm: f64,
    edge_clearance_mm: f64,
    collision_inset_mm: f64,
    trust_radius_mm: f64,
    theta_caps: &[f64],
) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();

    for first in 0..geometries.len() {
        for second in (first + 1)..geometries.len() {
            // Correction 5: the reach of two translations PLUS the reach of
            // two rotations.
            //
            // The two translation terms are `trust_radius_mm` each and are
            // *unconditional*: every piece may translate the full radius,
            // including one the request pins against rotation. An earlier draft
            // of this file recovered the radius from `theta_cap * reach`, which
            // is the radius for a rotatable piece and **zero** for a pinned one
            // — so a pair of pinned pieces got a guard band of just the
            // clearance contract and no row at all, which is precisely the
            // insufficient-reach defect Sol review 6 §3 flagged on the old
            // branch, reintroduced through the back door. Two pinned pieces
            // 1.5 mm apart at a 1 mm trust radius can be driven into each other
            // and must have a row.
            //
            // `theta_cap * reach` is the right rotational term: it is the
            // furthest a rotation inside the box can move any vertex of either
            // gate, and it is zero exactly when the piece may not turn.
            let reach_mm = 2.0 * trust_radius_mm
                + theta_caps[first] * geometries[first].reach_mm
                + theta_caps[second] * geometries[second].reach_mm;

            for (family, contract_mm) in [
                (RowFamily::MaterialPair, contracts.material_pair_mm),
                // Envelopes only have to miss each other; touching is legal to
                // the grid gate. Correction 4: this row is opened for every
                // pair inside the band, not only for pairs already overlapping.
                (RowFamily::EnvelopePair, 0.0),
            ] {
                let (first_outline, second_outline) = match family {
                    RowFamily::MaterialPair => {
                        (&geometries[first].material, &geometries[second].material)
                    }
                    _ => (&geometries[first].collision, &geometries[second].collision),
                };
                let guard_mm = contract_mm + reach_mm;
                if first_outline.bounds.gap(second_outline.bounds) >= guard_mm {
                    continue;
                }
                let approach = measure_approach(
                    first_outline,
                    (0.0, 0.0),
                    second_outline,
                    (0.0, 0.0),
                    guard_mm,
                );
                if approach.distance >= guard_mm {
                    continue;
                }
                // The geometries are already at their world poses, so the
                // measurement ran with a zero relative shift and the witness
                // points come back in world coordinates directly.
                let Some((on_first, on_second)) = approach.witness else {
                    continue;
                };
                let normal = match approach.direction {
                    Some(direction) => direction,
                    // At a touch the distance function is flat and carries no
                    // gradient. Fall back to the centroid axis, which is the
                    // same fallback the production builder uses, and keep the
                    // contact point: whichever direction is chosen, the row is
                    // a linear model of separation along *that* direction, and
                    // the rotational coefficients belong to it.
                    None => {
                        let dx = geometries[first].centre.x - geometries[second].centre.x;
                        let dy = geometries[first].centre.y - geometries[second].centre.y;
                        let length = dx.hypot(dy);
                        if length <= f64::MIN_POSITIVE {
                            continue;
                        }
                        (dx / length, dy / length)
                    }
                };
                rows.push(Row {
                    first,
                    second: Some(second),
                    normal,
                    theta_first: rotation_coefficient(normal, on_first, geometries[first].centre),
                    theta_second: -rotation_coefficient(
                        normal,
                        on_second,
                        geometries[second].centre,
                    ),
                    rhs_mm: contract_mm - approach.distance,
                    family,
                });
            }
        }
    }

    for (index, geometry) in geometries.iter().enumerate() {
        let theta_cap = theta_caps[index];
        let slack_mm = rotation_chord_slack_mm(geometry.reach_mm, theta_cap);
        let material_edge = contracts.material_edge_mm;

        // Correction 1, the whole point: the material family is measured
        // against the PUBLICATION measure (`max_y + edge clearance`), and the
        // envelope family against the parent's own strip bound. Neither is the
        // request's `sheet_long_axis_mm`, and they are not the same number.
        push_boundary_rows(
            &mut rows,
            index,
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
            index,
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
                material_edge,
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
                    index,
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

/// The box corner maximizing `c . x`.
fn box_corner(
    c: &[(f64, f64, f64)],
    trust_radius_mm: f64,
    theta_caps: &[f64],
) -> Vec<(f64, f64, f64)> {
    c.iter()
        .zip(theta_caps)
        .map(|(&(cx, cy, ctheta), &cap)| {
            (
                if cx >= 0.0 {
                    trust_radius_mm
                } else {
                    -trust_radius_mm
                },
                if cy >= 0.0 {
                    trust_radius_mm
                } else {
                    -trust_radius_mm
                },
                if ctheta >= 0.0 { cap } else { -cap },
            )
        })
        .collect()
}

fn project_box(x: &mut [(f64, f64, f64)], trust_radius_mm: f64, theta_caps: &[f64]) {
    for (slot, &cap) in x.iter_mut().zip(theta_caps) {
        slot.0 = slot.0.clamp(-trust_radius_mm, trust_radius_mm);
        slot.1 = slot.1.clamp(-trust_radius_mm, trust_radius_mm);
        slot.2 = slot.2.clamp(-cap, cap);
    }
}

/// Weak duality: an upper bound on the program's optimum from any `lambda >= 0`
/// whose weight on the `delta`-carrying rows sums to one.
///
/// The Lagrangian of `max delta` subject to `a_i . x >= rhs_i (+ delta)` is
///
/// ```text
/// delta * (1 - sum_{i in D} lambda_i) + sum_i lambda_i (a_i . x - rhs_i)
/// ```
///
/// so `sum_{i in D} lambda_i = 1` is exactly the condition for the supremum
/// over an unbounded `delta` to be finite, and what is left is a linear
/// function of `x` whose maximum over the box is a sum of absolute values —
/// computed exactly, in closed form, in one pass. Every `lambda` meeting the
/// normalization therefore gives a valid upper bound whatever the solver did
/// to find it, which is why a poor `lambda` can only loosen this number.
fn dual_bound_mm(
    rows: &[Row],
    weights: &[f64],
    slots: usize,
    trust_radius_mm: f64,
    theta_caps: &[f64],
) -> f64 {
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
    total.add(rhs.sum);
    for (&(cx, cy, ctheta), &cap) in c.iter().zip(theta_caps) {
        total.add(trust_radius_mm * cx.abs());
        total.add(trust_radius_mm * cy.abs());
        total.add(cap * ctheta.abs());
    }
    // Outward: an upper bound rounds UP.
    total.high() + rhs.allowance()
}

/// What one solve produced: the bracket, and the two candidate directions the
/// witness line search will try.
///
/// `feasible` satisfies every non-`delta` row of the model; `objective` has the
/// best objective whether or not it does. Only `feasible` may inform
/// [`LpBracket::primal_lower_mm`]; both are offered to the exact validator,
/// which is the authority on what is publishable.
struct Solved {
    bracket: LpBracket,
    feasible: Vec<(f64, f64, f64)>,
    objective: Vec<(f64, f64, f64)>,
}

/// One program's solve.
///
/// The primal is projected supergradient ascent on the exact penalty
/// `min_{i in D} residual_i - rho * max(0, worst violation among the rest)`,
/// which is concave and piecewise linear over a box that is trivial to project
/// onto. The objective is not smooth, so a smooth-method rate does not apply
/// and none is claimed; the textbook `O(1/sqrt(t))` projected-supergradient
/// schedule is used, and — this is the part that matters — **every iterate that
/// is feasible for the non-delta rows contributes a valid lower bound on its
/// own**, so convergence controls tightness and nothing else.
///
/// `x = 0` starts feasible whenever the parent is legal, which is the only
/// case this diagnostic is run in: a legal parent's rows all have `rhs <= 0`.
fn solve_program(
    unscaled_rows: &[Row],
    delta_rows: &[usize],
    other_rows: &[usize],
    slots: usize,
    trust_radius_mm: f64,
    theta_caps: &[f64],
    iterations: usize,
) -> Solved {
    // Work in the isotropic cube throughout: see [`Row::scaled`]. Everything
    // below - the box projection, the step size, the dual's closed-form box
    // maximum - then uses one radius for all six coordinates of a piece, and
    // the scale is multiplied back out at the end.
    let scales: Vec<f64> = theta_caps.iter().map(|cap| cap / trust_radius_mm).collect();
    let rows: Vec<Row> = unscaled_rows
        .iter()
        .map(|row| row.scaled(&scales))
        .collect();
    let rows = rows.as_slice();
    let cube = vec![trust_radius_mm; slots];

    let mut best_primal_mm = f64::NEG_INFINITY;
    let mut best_x = vec![(0.0f64, 0.0f64, 0.0f64); slots];
    let mut best_dual_mm = f64::INFINITY;
    let mut ran = 0usize;

    // The best-objective iterate REGARDLESS of feasibility, kept as a second
    // candidate direction for the witness line search and for nothing else.
    //
    // It never touches `primal_lower_mm`: that number has to come from a point
    // feasible for every non-delta row or it is not a bound on the model at
    // all. But the witness's length is decided by the exact validator, not by
    // the model, so handing the search a direction the model merely *wanted*
    // costs nothing and recovers the common case where the subgradient walk
    // never lands exactly feasible and the feasible best stays at the origin —
    // measured on the 155.422 parent, where the translation-only column
    // otherwise reports a zero vector and a zero depth reduction.
    let mut best_objective_mm = f64::NEG_INFINITY;
    let mut best_objective_x = vec![(0.0f64, 0.0f64, 0.0f64); slots];

    if delta_rows.is_empty() {
        // No depth row means the objective is unpriced: nothing constrains
        // `delta` at all. Report the absence rather than inventing a number —
        // an infinity here would serialize as a bare `null` and read to a
        // driver exactly like a missing measurement.
        return Solved {
            bracket: LpBracket {
                primal_lower_mm: None,
                dual_upper_mm: None,
                gap_mm: None,
                primal_feasible: false,
                iterations: 0,
                arithmetic: ARITHMETIC_NOTE,
            },
            feasible: best_x,
            objective: best_objective_x,
        };
    }

    // A first dual candidate that needs no solver at all: put all the weight on
    // the single tightest depth row. Often close to tight, always valid.
    for &index in delta_rows {
        let mut weights = vec![0.0f64; rows.len()];
        weights[index] = 1.0;
        best_dual_mm =
            best_dual_mm.min(dual_bound_mm(rows, &weights, slots, trust_radius_mm, &cube));
    }

    for &rho in &PENALTY_LADDER {
        let mut x = vec![(0.0f64, 0.0f64, 0.0f64); slots];
        let mut depth_frequency = vec![0.0f64; rows.len()];
        let mut other_frequency = vec![0.0f64; rows.len()];
        let mut depth_mass = 0.0f64;

        for t in 0..iterations {
            ran += 1;

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

            // A feasible iterate is a lower bound in its own right.
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
                    best_dual_mm.min(dual_bound_mm(rows, &weights, slots, trust_radius_mm, &cube));

                // The same weighting's box maximizer is a point in the box, so
                // it is a primal candidate for free. It is only *used* when it
                // is feasible for every non-delta row, which is checked here
                // rather than assumed - the dual side of the solve carries no
                // feasibility guarantee of its own.
                let corner = box_corner(&c, trust_radius_mm, &cube);
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
            project_box(&mut x, trust_radius_mm, &cube);
        }
    }

    // Back out of the change of variables: `dtheta_i = scale_i * u_i`. Every
    // number reported from here on is in real millimetres and real radians.
    for (slot, &scale) in best_x.iter_mut().zip(&scales) {
        slot.2 *= scale;
    }
    for (slot, &scale) in best_objective_x.iter_mut().zip(&scales) {
        slot.2 *= scale;
    }

    let primal_lower_mm = best_primal_mm.is_finite().then_some(best_primal_mm);
    let dual_upper_mm = best_dual_mm.is_finite().then_some(best_dual_mm);
    Solved {
        bracket: LpBracket {
            primal_lower_mm,
            dual_upper_mm,
            gap_mm: match (primal_lower_mm, dual_upper_mm) {
                (Some(lower), Some(upper)) => Some(upper - lower),
                _ => None,
            },
            primal_feasible: primal_lower_mm.is_some(),
            iterations: ran,
            arithmetic: ARITHMETIC_NOTE,
        },
        feasible: best_x,
        objective: best_objective_x,
    }
}

const ARITHMETIC_NOTE: &str =
    "real-arithmetic bounds evaluated in f64 with an outward rounding allowance; \
     not exact rational certificates";

/// Runs the SE(2) rigidity certificate on one parent.
///
/// `reference_mm`, when supplied, is the depth reduction the caller wants to
/// know about — the record line's outstanding 0.422 mm, in practice. It only
/// selects among the verdict strings; it never changes a number.
pub fn se2_rigidity_certificate(
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
    settings: GeneralFastSettings,
    trust_radius_mm: f64,
    iterations: usize,
    reference_mm: Option<f64>,
) -> Result<Se2RigidityCertificate, String> {
    if pieces.is_empty() || placements.is_empty() {
        return Err("se2 rigidity certificate requires at least one placement".to_owned());
    }
    if !trust_radius_mm.is_finite() || trust_radius_mm <= 0.0 {
        return Err("se2 rigidity certificate trust radius must be positive and finite".to_owned());
    }
    if iterations == 0 {
        return Err("se2 rigidity certificate requires at least one iteration".to_owned());
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
        return Err("se2 rigidity certificate requires a finite clearance contract".to_owned());
    }

    let geometries = placements
        .iter()
        .map(|placement| {
            let index = pieces_by_id
                .get(placement.piece_id.as_str())
                .ok_or_else(|| format!("unknown piece id `{}`", placement.piece_id))?;
            let piece = pieces[*index];
            build_geometry(piece.polygon, placement, expansion_mm, piece.allow_rotation)
                .ok_or_else(|| format!("could not build a geometry for `{}`", placement.piece_id))
        })
        .collect::<Result<Vec<_>, String>>()?;

    // The parent's own two depths, measured rather than assumed. Correction 1.
    let parent_exact = exact_placements(pieces, &pieces_by_id, placements)?;
    let published_depth_mm = raw_source_long_axis_depth_mm(&parent_exact, edge_clearance_mm)
        .map_err(|error| error.message().to_owned())?;
    let strip_bound_mm = geometries
        .iter()
        .map(|geometry| geometry.collision.outer_bounds.max_y)
        .fold(f64::NEG_INFINITY, f64::max)
        + collision_inset_mm;
    if !strip_bound_mm.is_finite() {
        return Err("se2 rigidity certificate could not measure a strip bound".to_owned());
    }

    let slots = placements.len();
    let theta_caps_se2: Vec<f64> = geometries
        .iter()
        .map(|geometry| {
            if geometry.rotatable {
                trust_radius_mm / geometry.reach_mm
            } else {
                0.0
            }
        })
        .collect();
    let theta_caps_translation = vec![0.0f64; slots];

    let rows = build_rows(
        &geometries,
        &contracts,
        settings,
        published_depth_mm,
        strip_bound_mm,
        edge_clearance_mm,
        collision_inset_mm,
        trust_radius_mm,
        &theta_caps_se2,
    );

    let mut rows_by_family: BTreeMap<String, usize> = BTreeMap::new();
    let mut parent_worst_residual_mm: BTreeMap<String, f64> = BTreeMap::new();
    let origin = vec![(0.0f64, 0.0f64, 0.0f64); slots];
    for row in &rows {
        let family = format!("{:?}", row.family);
        *rows_by_family.entry(family.clone()).or_insert(0) += 1;
        let residual = row.residual_low_mm(&origin);
        parent_worst_residual_mm
            .entry(family)
            .and_modify(|worst| *worst = worst.min(residual))
            .or_insert(residual);
    }

    let validation_settings = PublicationValidationSettings {
        sheet_width_mm: settings.sheet_short_axis_mm,
        sheet_height_mm: settings.sheet_long_axis_mm,
        total_padding_mm: settings.total_padding_mm,
        sheet_edge_clearance_mm: settings.sheet_edge_clearance_mm,
        flattening_sag_tolerance_mm: settings.flattening_sag_tolerance_mm,
    };

    // Built once: it is the parent plus the settings it is judged under, and
    // none of it varies across the four programs.
    let exact_context = ExactContext {
        pieces,
        pieces_by_id: &pieces_by_id,
        placements,
        geometries: &geometries,
        validation_settings,
        edge_clearance_mm,
    };

    let mut programs = Vec::new();
    for program in [Se2Program::DepthOnly, Se2Program::StripCoupled] {
        let mut delta_rows = Vec::new();
        let mut other_rows = Vec::new();
        for (index, row) in rows.iter().enumerate() {
            if row.family.carries_delta(program) {
                delta_rows.push(index);
            } else {
                other_rows.push(index);
            }
        }
        for motion in [Se2Motion::TranslationOnly, Se2Motion::Se2] {
            let caps = match motion {
                Se2Motion::TranslationOnly => &theta_caps_translation,
                Se2Motion::Se2 => &theta_caps_se2,
            };
            let Solved {
                bracket: lp,
                feasible: feasible_x,
                objective: objective_x,
            } = solve_program(
                &rows,
                &delta_rows,
                &other_rows,
                slots,
                trust_radius_mm,
                caps,
                iterations,
            );

            // Sol review 6 §3: never hide `lower > upper` behind `max(0.0)`.
            // Both numbers are already outward-rounded, so if the bracket is
            // still inverted the arithmetic in this file is wrong and the run
            // must say so.
            if let (Some(lower), Some(upper)) = (lp.primal_lower_mm, lp.dual_upper_mm) {
                if lower > upper {
                    return Err(format!(
                        "se2 certificate bracket inverted for {program:?}/{motion:?}: \
                         outward-rounded lower {lower} exceeds outward-rounded upper {upper}"
                    ));
                }
            }

            let (witness, witness_error) = match witness_outcome(
                &exact_context,
                &feasible_x,
                &objective_x,
                published_depth_mm,
            ) {
                Ok(outcome) => (Some(outcome), None),
                Err(error) => (None, Some(error)),
            };

            let verdict = verdict_for(&lp, witness.as_ref(), reference_mm);
            programs.push(Se2ProgramResult {
                program,
                motion,
                delta_rows: delta_rows.len(),
                lp,
                witness,
                witness_error,
                verdict,
            });
        }
    }

    let headline = programs
        .iter()
        .find(|result| result.program == Se2Program::DepthOnly && result.motion == Se2Motion::Se2)
        .map(|result| result.verdict)
        .unwrap_or("no-program");

    let reach_min_mm = geometries
        .iter()
        .map(|g| g.reach_mm)
        .fold(f64::INFINITY, f64::min);
    let reach_max_mm = geometries.iter().map(|g| g.reach_mm).fold(0.0f64, f64::max);

    Ok(Se2RigidityCertificate {
        piece_count: slots,
        rotatable_piece_count: geometries.iter().filter(|g| g.rotatable).count(),
        trust_radius_mm,
        published_depth_mm,
        strip_bound_mm,
        strip_excess_mm: strip_bound_mm - published_depth_mm,
        rows: rows.len(),
        rows_by_family,
        parent_worst_residual_mm,
        reach_min_mm,
        reach_max_mm,
        theta_cap_max_deg: theta_caps_se2
            .iter()
            .fold(0.0f64, |best, cap| best.max(*cap))
            .to_degrees(),
        programs,
        verdict: headline,
        reference_mm,
    })
}

/// One exactly-validated layout the certificate's witness reached, and what it
/// cost to reach it.
///
/// This is the certificate reduced to the only part a *search* can spend: the
/// moved placements themselves. Everything else the certificate reports —
/// brackets, dual bounds, per-family residuals, the four programs — is a
/// diagnostic about what is possible, and a schedule slice has no budget for it.
#[cfg(feature = "sparse-rotation")]
#[derive(Clone, Debug)]
pub struct Se2WitnessProposal {
    /// The moved layout. Already accepted by `validate_publication`, because
    /// the line search never returns a scale it did not validate.
    pub placements: Vec<GeneralFastPlacement>,
    /// The publication measure on `placements`.
    pub published_depth_mm: f64,
    /// `parent published depth - published_depth_mm`. Zero when the line search
    /// fell back to the parent, which is always available and never rejected.
    pub delta_mm: f64,
    /// The fraction of the model's vector that survived exact validation.
    pub scale: f64,
    /// Largest applied `|dtheta|` in degrees, so a caller can tell a rotation
    /// witness from a translation one without re-deriving it.
    pub max_abs_dtheta_deg: f64,
    pub moved_pieces: usize,
    /// Exact validations the line search spent. The dominant per-call price
    /// after the row build, and the number a slice budget is charged.
    pub validations: usize,
    /// Rows the program carried, for the same reason.
    pub rows: usize,
}

/// The certificate's **witness only**, on the one program a search can use.
///
/// [`se2_rigidity_certificate`] solves four programs and line-searches all four,
/// because it is a diagnostic and the comparison is its subject. A proposal
/// source cannot afford that: docs/experiments/se2-rigidity/ measured a
/// certificate call at up to a second, against a mode-34 slice that is 0.78 s
/// whole. This runs `{depth-only} x {SE(2)}` — the headline column, the one that
/// actually carries rotation — once, and returns the moved placements.
///
/// It is still not cheap, and it is not pretended to be: the row build is
/// `O(pairs)` and the solve is `O(iterations * rows)`. `iterations` and
/// `trust_radius_mm` are the caller's two knobs and
/// docs/experiments/sparse-rotation/ §3 prices the call against a slice at the
/// values the schedule uses. The honest reason to keep the entry point narrow
/// is that a wide one would invite a caller to run it per step.
///
/// `Ok(None)` means the solve produced no witness — a parent the line search
/// could not evaluate — which is a refusal, not an error, and leaves the caller
/// with the layout it already had.
#[cfg(feature = "sparse-rotation")]
pub fn se2_witness_proposal(
    pieces: &[GeneralFastPiece<'_>],
    placements: &[GeneralFastPlacement],
    settings: GeneralFastSettings,
    trust_radius_mm: f64,
    iterations: usize,
) -> Result<Option<Se2WitnessProposal>, String> {
    if pieces.is_empty() || placements.is_empty() {
        return Err("se2 witness proposal requires at least one placement".to_owned());
    }
    if !trust_radius_mm.is_finite() || trust_radius_mm <= 0.0 {
        return Err("se2 witness proposal trust radius must be positive and finite".to_owned());
    }
    if iterations == 0 {
        return Err("se2 witness proposal requires at least one iteration".to_owned());
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
        return Err("se2 witness proposal requires a finite clearance contract".to_owned());
    }

    let geometries = placements
        .iter()
        .map(|placement| {
            let index = pieces_by_id
                .get(placement.piece_id.as_str())
                .ok_or_else(|| format!("unknown piece id `{}`", placement.piece_id))?;
            let piece = pieces[*index];
            build_geometry(piece.polygon, placement, expansion_mm, piece.allow_rotation)
                .ok_or_else(|| format!("could not build a geometry for `{}`", placement.piece_id))
        })
        .collect::<Result<Vec<_>, String>>()?;

    let parent_exact = exact_placements(pieces, &pieces_by_id, placements)?;
    let published_depth_mm = raw_source_long_axis_depth_mm(&parent_exact, edge_clearance_mm)
        .map_err(|error| error.message().to_owned())?;
    let strip_bound_mm = geometries
        .iter()
        .map(|geometry| geometry.collision.outer_bounds.max_y)
        .fold(f64::NEG_INFINITY, f64::max)
        + collision_inset_mm;
    if !strip_bound_mm.is_finite() {
        return Err("se2 witness proposal could not measure a strip bound".to_owned());
    }

    let slots = placements.len();
    let theta_caps: Vec<f64> = geometries
        .iter()
        .map(|geometry| {
            if geometry.rotatable {
                trust_radius_mm / geometry.reach_mm
            } else {
                0.0
            }
        })
        .collect();
    let rows = build_rows(
        &geometries,
        &contracts,
        settings,
        published_depth_mm,
        strip_bound_mm,
        edge_clearance_mm,
        collision_inset_mm,
        trust_radius_mm,
        &theta_caps,
    );

    let program = Se2Program::DepthOnly;
    let mut delta_rows = Vec::new();
    let mut other_rows = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if row.family.carries_delta(program) {
            delta_rows.push(index);
        } else {
            other_rows.push(index);
        }
    }
    let Solved {
        feasible: feasible_x,
        objective: objective_x,
        ..
    } = solve_program(
        &rows,
        &delta_rows,
        &other_rows,
        slots,
        trust_radius_mm,
        &theta_caps,
        iterations,
    );

    let validation_settings = PublicationValidationSettings {
        sheet_width_mm: settings.sheet_short_axis_mm,
        sheet_height_mm: settings.sheet_long_axis_mm,
        total_padding_mm: settings.total_padding_mm,
        sheet_edge_clearance_mm: settings.sheet_edge_clearance_mm,
        flattening_sag_tolerance_mm: settings.flattening_sag_tolerance_mm,
    };
    let exact_context = ExactContext {
        pieces,
        pieces_by_id: &pieces_by_id,
        placements,
        geometries: &geometries,
        validation_settings,
        edge_clearance_mm,
    };
    let Ok(witness) = witness_outcome(
        &exact_context,
        &feasible_x,
        &objective_x,
        published_depth_mm,
    ) else {
        return Ok(None);
    };
    let (Some(depth_mm), Some(delta_mm)) = (witness.published_depth_mm, witness.delta_mm) else {
        return Ok(None);
    };
    // The vector the witness reports is the *applied* one - already multiplied
    // by the scale the validator accepted - so it is applied at unit scale here.
    // Rebuilding the layout rather than carrying it out of the line search keeps
    // `witness_outcome` the single owner of "which scale won".
    let moved = placements
        .iter()
        .zip(&geometries)
        .zip(&witness.vector)
        .map(|((placement, geometry), (_, dx, dy, dtheta_deg))| {
            apply_se2(
                placement,
                geometry.centre,
                (*dx, *dy, dtheta_deg.to_radians()),
            )
        })
        .collect();

    Ok(Some(Se2WitnessProposal {
        placements: moved,
        published_depth_mm: depth_mm,
        delta_mm,
        scale: witness.scale,
        max_abs_dtheta_deg: witness.max_abs_dtheta_deg,
        moved_pieces: witness.moved_pieces,
        validations: witness.validations,
        rows: rows.len(),
    }))
}

/// Sol review 6 §3's three cases, plus the two the rewrite makes reachable.
///
/// The reference is compared against the **validated** delta, never against
/// the model's own primal: the whole complaint about the old branch was that
/// "no parent reaches 0.422" was asserted from a number that could not be
/// applied to anything.
fn verdict_for(
    lp: &LpBracket,
    witness: Option<&WitnessOutcome>,
    reference_mm: Option<f64>,
) -> &'static str {
    // No priced row, or a solve that never found a feasible point: there is no
    // bracket to read a verdict off, and saying so is the verdict.
    let Some(dual_upper_mm) = lp.dual_upper_mm else {
        return "unpriced";
    };
    if dual_upper_mm <= 0.0 {
        return "blocked";
    }
    if !lp.primal_feasible {
        return "no-feasible-point";
    }
    let achieved_mm = witness.and_then(|outcome| outcome.delta_mm);
    match (achieved_mm, reference_mm) {
        (Some(achieved), Some(reference)) if achieved >= reference => "positive-reaches-reference",
        (Some(achieved), Some(_)) if achieved > 0.0 => "positive-below-reference",
        (Some(achieved), None) if achieved > 0.0 => "positive",
        (Some(_), _) => {
            // The model says there is room and the exact move does not deliver
            // any. That is a statement about the linearization, and it is the
            // interesting case, so it gets its own name instead of being
            // rounded into "positive".
            if lp.primal_lower_mm.is_some_and(|lower| lower > 0.0) {
                "model-positive-witness-flat"
            } else {
                "ambiguous"
            }
        }
        (None, _) => "witness-rejected",
    }
}

/// The layout as the exact validator wants it.
///
/// The returned placements borrow their polygon and id from `pieces`, never
/// from the `GeneralFastPlacement` list — which is why this is a free function
/// with an explicit `'p` rather than a closure: inferred, the borrow checker
/// ties the result to the *placements* and the whole witness path then demands
/// `'static` pieces.
pub(super) fn exact_placements<'p>(
    pieces: &[GeneralFastPiece<'p>],
    pieces_by_id: &BTreeMap<&str, usize>,
    placements: &[GeneralFastPlacement],
) -> Result<Vec<GeneralPlacement<'p>>, String> {
    placements
        .iter()
        .map(|placement| {
            let index = pieces_by_id
                .get(placement.piece_id.as_str())
                .ok_or_else(|| format!("unknown piece id `{}`", placement.piece_id))?;
            Ok(GeneralPlacement {
                piece_id: pieces[*index].id,
                polygon: pieces[*index].polygon,
                rotation_deg: placement.rotation_deg,
                mirrored: placement.mirrored,
                translate_x: placement.translate_short_axis,
                translate_y: placement.translate_long_axis,
            })
        })
        .collect()
}

/// Scales tried by the witness line search, in the order tried.
///
/// A coarse geometric ladder rather than a pure bisection because the first
/// question is "does any usable fraction of this direction survive at all",
/// and on a rejected full step the answer is usually found in two or three
/// probes. [`WITNESS_BISECTIONS`] then refines upward between the best accepted
/// rung and the smallest rejected one.
///
/// `0.0` is deliberately the last entry and is never rejected: it reproduces
/// the parent exactly ([`apply_se2`] with a zero delta is the identity, which
/// `applying_a_zero_vector_returns_the_placement_unchanged` pins), so the search
/// always terminates with a validated layout in hand.
const WITNESS_SCALES: [f64; 8] = [1.0, 0.75, 0.5, 0.25, 0.1, 0.05, 0.01, 0.0];

/// Bisection steps spent refining the accepted scale upward.
const WITNESS_BISECTIONS: usize = 8;

/// What the exact validator said about one scaled vector.
///
/// A two-level `Result` would say the same thing and read as a typo: the outer
/// error is "this layout could not be evaluated", the inner one is "it was
/// evaluated and refused", and those are different events with different
/// consequences.
enum ScaleOutcome {
    /// Accepted, with the publication measure on the moved layout.
    Accepted(f64),
    /// Refused, with the validator's own message.
    Refused(String),
}

/// Everything an exact evaluation needs that does not change between scales.
///
/// Bundled because passing the seven of them separately is what pushed
/// [`evaluate_scale`] past the argument limit, and because they really are one
/// thing: the parent, and the settings it is judged under.
struct ExactContext<'a, 'p> {
    pieces: &'a [GeneralFastPiece<'p>],
    pieces_by_id: &'a BTreeMap<&'a str, usize>,
    placements: &'a [GeneralFastPlacement],
    geometries: &'a [Geometry],
    validation_settings: PublicationValidationSettings,
    edge_clearance_mm: f64,
}

/// One exact evaluation of the vector at a scale: validate, then measure.
fn evaluate_scale(
    context: &ExactContext<'_, '_>,
    x: &[(f64, f64, f64)],
    scale: f64,
) -> Result<ScaleOutcome, String> {
    let ExactContext {
        pieces,
        pieces_by_id,
        placements,
        geometries,
        validation_settings,
        edge_clearance_mm,
    } = *context;
    let moved: Vec<GeneralFastPlacement> = placements
        .iter()
        .zip(geometries)
        .zip(x)
        .map(|((placement, geometry), delta)| {
            apply_se2(
                placement,
                geometry.centre,
                (scale * delta.0, scale * delta.1, scale * delta.2),
            )
        })
        .collect();
    let exact = exact_placements(pieces, pieces_by_id, &moved)?;
    match validate_publication(&exact, validation_settings) {
        Ok(()) => {
            let depth_mm = raw_source_long_axis_depth_mm(&exact, edge_clearance_mm)
                .map_err(|error| error.message().to_owned())?;
            Ok(ScaleOutcome::Accepted(depth_mm))
        }
        Err(error) => Ok(ScaleOutcome::Refused(error.message().to_owned())),
    }
}

/// Line-searches the model's vector against the exact validator and returns
/// the best exactly-validated point on it.
///
/// The direction is the model's; the length is the validator's. See
/// [`WitnessOutcome`] for why the two have to be separated.
///
/// The search maximizes the **validated depth reduction**, not the scale: a
/// longer step that validates but publishes a deeper layout is not a better
/// witness, and on a non-convex geometry the two are not the same ordering.
#[allow(clippy::too_many_arguments)]
fn witness_along(
    context: &ExactContext<'_, '_>,
    x: &[(f64, f64, f64)],
    direction: &'static str,
    parent_depth_mm: f64,
) -> Result<WitnessOutcome, String> {
    let mut validations = 0usize;
    let mut probe = |scale: f64| -> Result<ScaleOutcome, String> {
        validations += 1;
        evaluate_scale(context, x, scale)
    };

    // Best accepted rung, and the smallest rejected scale above it.
    let mut best: Option<(f64, f64)> = None; // (scale, depth)
    let mut smallest_rejected = f64::INFINITY;
    let mut full_vector_exact_valid = false;
    let mut full_vector_rejection = None;

    for &scale in &WITNESS_SCALES {
        match probe(scale)? {
            ScaleOutcome::Accepted(depth_mm) => {
                if scale == 1.0 {
                    full_vector_exact_valid = true;
                }
                if best.is_none_or(|(_, best_depth)| depth_mm < best_depth) {
                    best = Some((scale, depth_mm));
                }
                // The ladder descends, and a shallower published depth is the
                // objective; once a rung is accepted the ones below it are
                // smaller steps in the same direction, so stop and refine.
                break;
            }
            ScaleOutcome::Refused(rejection) => {
                if scale == 1.0 {
                    full_vector_rejection = Some(rejection);
                }
                smallest_rejected = smallest_rejected.min(scale);
            }
        }
    }

    let (mut scale, mut depth_mm) = best.ok_or_else(|| {
        // Unreachable while `WITNESS_SCALES` ends in `0.0`: that rung is the
        // parent itself. If it ever fires, the parent handed to this
        // diagnostic was not legal, which is worth an error rather than a
        // silently degraded witness.
        "se2 certificate: even a zero-length witness failed exact validation, \
         so the parent itself is not publishable"
            .to_owned()
    })?;

    // Refine upward: everything in `(scale, smallest_rejected)` is untested.
    if smallest_rejected.is_finite() {
        let mut low = scale;
        let mut high = smallest_rejected;
        for _ in 0..WITNESS_BISECTIONS {
            let middle = 0.5 * (low + high);
            match probe(middle)? {
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

    let applied: Vec<(f64, f64, f64)> = x
        .iter()
        .map(|slot| (scale * slot.0, scale * slot.1, scale * slot.2))
        .collect();
    let max_abs_dtheta_deg = applied
        .iter()
        .fold(0.0f64, |best, slot| best.max(slot.2.abs()))
        .to_degrees();
    let max_abs_translation_mm = applied
        .iter()
        .fold(0.0f64, |best, slot| best.max(slot.0.hypot(slot.1)));
    let moved_pieces = applied
        .iter()
        .filter(|slot| slot.0 != 0.0 || slot.1 != 0.0 || slot.2 != 0.0)
        .count();
    let vector = context
        .placements
        .iter()
        .zip(&applied)
        .map(|(placement, slot)| {
            (
                placement.piece_id.clone(),
                slot.0,
                slot.1,
                slot.2.to_degrees(),
            )
        })
        .collect();

    Ok(WitnessOutcome {
        exact_valid: true,
        scale,
        direction,
        full_vector_exact_valid,
        full_vector_rejection,
        published_depth_mm: Some(depth_mm),
        delta_mm: Some(parent_depth_mm - depth_mm),
        validations,
        max_abs_dtheta_deg,
        max_abs_translation_mm,
        moved_pieces,
        vector,
    })
}

/// Line-searches **both** candidate directions and keeps the better result.
///
/// `feasible` is the best point the solve found that satisfies every non-delta
/// row of the model; `objective` is the best-objective point whether or not it
/// does. Neither is privileged here, because the exact validator — not the
/// model — decides what is publishable, and the model's feasibility notion is
/// a relaxed one anyway. The winner is simply whichever ray yields the larger
/// exactly-validated depth reduction.
fn witness_outcome(
    context: &ExactContext<'_, '_>,
    feasible: &[(f64, f64, f64)],
    objective: &[(f64, f64, f64)],
    parent_depth_mm: f64,
) -> Result<WitnessOutcome, String> {
    let mut best: Option<WitnessOutcome> = None;
    let mut spent = 0usize;
    for (x, label) in [(feasible, "modelFeasible"), (objective, "modelObjective")] {
        let outcome = witness_along(context, x, label, parent_depth_mm)?;
        spent += outcome.validations;
        let better = best
            .as_ref()
            .is_none_or(|incumbent| outcome.delta_mm > incumbent.delta_mm);
        if better {
            best = Some(outcome);
        }
    }
    let mut winner = best.expect("both directions are always searched");
    // Report the whole search's cost, not just the winning ray's.
    winner.validations = spent;
    Ok(winner)
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

    fn placement(id: &str, x_mm: f64, y_mm: f64) -> GeneralFastPlacement {
        GeneralFastPlacement {
            piece_id: id.to_owned(),
            rotation_deg: 0.0,
            mirrored: false,
            translate_short_axis: x_mm,
            translate_long_axis: y_mm,
        }
    }

    fn piece<'a>(
        id: &'a str,
        polygon: &'a PolygonSet,
        allow_rotation: bool,
    ) -> GeneralFastPiece<'a> {
        GeneralFastPiece {
            id,
            polygon,
            allow_rotation,
            allow_mirror: false,
        }
    }

    /// The bracket as two real numbers, asserting on the way through that both
    /// ends exist. `Option<f64>` has a `PartialOrd` — `None < Some(_)` — so a
    /// bare `assert!(lp.primal_lower_mm <= lp.dual_upper_mm)` would compile and
    /// pass on a missing bound. Every test that wants the bracket goes through
    /// here instead.
    fn bracket(result: &Se2ProgramResult) -> (f64, f64) {
        let lower = result.lp.primal_lower_mm.unwrap_or_else(|| {
            panic!(
                "{:?}/{:?} has no primal lower bound (primalFeasible={})",
                result.program, result.motion, result.lp.primal_feasible
            )
        });
        let upper = result.lp.dual_upper_mm.unwrap_or_else(|| {
            panic!(
                "{:?}/{:?} has no dual upper bound",
                result.program, result.motion
            )
        });
        assert!(
            lower <= upper,
            "{:?}/{:?} bracket inverted: [{lower}, {upper}]",
            result.program,
            result.motion
        );
        (lower, upper)
    }

    fn zero_clearance_settings(short_axis_mm: f64, long_axis_mm: f64) -> GeneralFastSettings {
        GeneralFastSettings {
            sheet_short_axis_mm: short_axis_mm,
            sheet_long_axis_mm: long_axis_mm,
            total_padding_mm: 0.0,
            sheet_edge_clearance_mm: Some(0.0),
            clearance_safety_margin_mm: 0.0,
            flattening_sag_tolerance_mm: 0.0,
            search_offset_allowance_mm: 0.0,
            ..GeneralFastSettings::deterministic_test(short_axis_mm, long_axis_mm)
        }
    }

    #[test]
    fn the_rotation_coefficient_is_the_normal_against_the_quarter_turn_generator() {
        let centre = IrregularPoint::new(1.0, 2.0);
        let point = IrregularPoint::new(4.0, 6.0);
        // J(p - c) = J(3, 4) = (-4, 3).
        assert!((rotation_coefficient((1.0, 0.0), point, centre) - -4.0).abs() < 1e-12);
        assert!((rotation_coefficient((0.0, 1.0), point, centre) - 3.0).abs() < 1e-12);
        // A point at the centre has no rotational lever arm at all.
        assert!(rotation_coefficient((0.6, 0.8), centre, centre).abs() < 1e-12);
    }

    /// The rotational coefficient must be the derivative of the real thing.
    ///
    /// Finite differences against [`apply_se2`], which performs an exact
    /// rotation, on a non-axis-aligned normal and an off-centre point: if the
    /// two ever disagreed, every row in the program would be a plausible-looking
    /// linearization of the wrong motion.
    #[test]
    fn the_rotation_coefficient_matches_a_finite_difference_of_the_exact_motion() {
        let polygon = rectangle(30.0, 12.0);
        let base = placement("p", 5.0, 7.0);
        let outline = build_outline(
            &polygon
                .transformed(
                    base.rotation_deg,
                    base.mirrored,
                    base.translate_short_axis,
                    base.translate_long_axis,
                )
                .unwrap(),
        )
        .unwrap();
        let centre = outline.centroid;
        let probe = IrregularPoint::new(35.0, 19.0); // the far corner
        let normal = (0.6, -0.8);

        for &h in &[1e-5, 1e-6, 1e-7] {
            let moved = apply_se2(&base, centre, (0.0, 0.0, h));
            let moved_set = polygon
                .transformed(
                    moved.rotation_deg,
                    moved.mirrored,
                    moved.translate_short_axis,
                    moved.translate_long_axis,
                )
                .unwrap();
            // The far corner is the third vertex of the only region.
            let moved_probe = moved_set.regions[0].outer.source_points()[2];
            let measured =
                (normal.0 * (moved_probe.x - probe.x) + normal.1 * (moved_probe.y - probe.y)) / h;
            let predicted = rotation_coefficient(normal, probe, centre);
            assert!(
                (measured - predicted).abs() < 1e-3 * predicted.abs().max(1.0),
                "h={h}: finite difference {measured} vs coefficient {predicted}"
            );
        }
    }

    /// Same check on a *mirrored* pose, where the source-to-world map has
    /// negative determinant. `apply_se2` never touches the mirror flag, so the
    /// rotation must still be a plain quarter-turn generator in world
    /// coordinates — if the mirror leaked into the derivative the sign would
    /// flip and every mirrored piece's rows would point the wrong way.
    #[test]
    fn the_rotation_coefficient_holds_on_a_mirrored_pose() {
        let polygon = rectangle(30.0, 12.0);
        let base = GeneralFastPlacement {
            mirrored: true,
            rotation_deg: 37.0,
            ..placement("p", 5.0, 7.0)
        };
        let transformed = polygon
            .transformed(
                base.rotation_deg,
                base.mirrored,
                base.translate_short_axis,
                base.translate_long_axis,
            )
            .unwrap();
        let centre = build_outline(&transformed).unwrap().centroid;
        let probe = transformed.regions[0].outer.source_points()[2];
        let normal = (-0.28, 0.96);
        let h = 1e-6;
        let moved = apply_se2(&base, centre, (0.0, 0.0, h));
        let moved_probe = polygon
            .transformed(
                moved.rotation_deg,
                moved.mirrored,
                moved.translate_short_axis,
                moved.translate_long_axis,
            )
            .unwrap()
            .regions[0]
            .outer
            .source_points()[2];
        let measured =
            (normal.0 * (moved_probe.x - probe.x) + normal.1 * (moved_probe.y - probe.y)) / h;
        let predicted = rotation_coefficient(normal, probe, centre);
        assert!(
            (measured - predicted).abs() < 1e-3 * predicted.abs().max(1.0),
            "finite difference {measured} vs coefficient {predicted}"
        );
    }

    /// The third geometry Sol review 6 §3 named: the **miter** join of a
    /// collision envelope.
    ///
    /// This is where the previous branch's `+0.15–0.28 mm` recalibration came
    /// from. Offsetting a sharp corner outward puts an envelope vertex measurably
    /// further out than any material vertex, so the `EnvelopeStrip` rows are
    /// built on points that exist only after the offset — points whose rotational
    /// coefficient nothing else in this file exercises. Two things are checked on
    /// them:
    ///
    /// 1. the coefficient is the derivative of the real motion, by finite
    ///    difference against an exact rotation with the envelope **rebuilt from
    ///    the rotated material** each time, so the offset is inside the loop
    ///    rather than assumed to commute with rotation;
    /// 2. the row really is a *relaxation* at the full angular cap —
    ///    `rotation_chord_slack_mm` has to cover the whole second-order chord
    ///    error, or the reported upper bound would not bound the rotated
    ///    geometry it claims to.
    #[test]
    fn the_rotational_coefficient_and_its_slack_hold_on_a_miter_envelope() {
        let triangle = peaked_triangle();
        let base = GeneralFastPlacement {
            rotation_deg: 11.0,
            ..placement("miter", 12.0, 30.0)
        };
        let settings = GeneralFastSettings {
            total_padding_mm: 3.0,
            clearance_safety_margin_mm: 0.75,
            ..zero_clearance_settings(200.0, 400.0)
        };
        let expansion_mm = collision_expansion_mm(settings);
        assert!(expansion_mm > 0.0, "the test needs a real offset");

        // The envelope's deepest point, and the material centroid it turns
        // about, both at the parent pose.
        let envelope_max_y = |placement: &GeneralFastPlacement| -> f64 {
            triangle
                .transformed(
                    placement.rotation_deg,
                    placement.mirrored,
                    placement.translate_short_axis,
                    placement.translate_long_axis,
                )
                .unwrap()
                .offset(expansion_mm)
                .unwrap()
                .regions
                .iter()
                .flat_map(|region| region.outer.source_points().iter().copied())
                .fold(f64::NEG_INFINITY, |best, point| best.max(point.y))
        };
        let transformed = triangle
            .transformed(
                base.rotation_deg,
                base.mirrored,
                base.translate_short_axis,
                base.translate_long_axis,
            )
            .unwrap();
        let centre = build_outline(&transformed).unwrap().centroid;
        let envelope = transformed.offset(expansion_mm).unwrap();
        let deepest = envelope
            .regions
            .iter()
            .flat_map(|region| region.outer.source_points().iter().copied())
            .reduce(|best, point| if point.y > best.y { point } else { best })
            .unwrap();
        // The miter really does reach past the material.
        let material_max_y = transformed
            .regions
            .iter()
            .flat_map(|region| region.outer.source_points().iter().copied())
            .fold(f64::NEG_INFINITY, |best, point| best.max(point.y));
        assert!(
            deepest.y > material_max_y + 0.5 * expansion_mm,
            "envelope {} barely clears material {material_max_y}",
            deepest.y
        );

        // (1) The strip row's normal is the inward `(0, -1)`, so the quantity
        // the row tracks is `n . p = -p.y` and its theta-derivative is the
        // coefficient. Finite-difference exactly that, with the offset rebuilt
        // inside the loop.
        // The strip row's normal is the inward `(0, -1)`, so the quantity the
        // row tracks is `n . p = -p.y` and its theta-derivative is the
        // coefficient.
        //
        // Two things force the step size here to be *large*, and both are real
        // properties of the object rather than test convenience:
        //
        // * `PolygonSet::offset` quantizes to the Clipper2 grid — 1000 units
        //   per millimetre, so one micron. The envelope's `max_y` is therefore a
        //   staircase in theta, and any step whose true effect is under a micron
        //   differences to exactly zero. At the `1e-5` a smooth function would
        //   want, this test measured `0` against a coefficient of `-5.05`.
        // * a large step then re-exposes the second-order chord term, so the
        //   difference is **central**: `(f(h) - f(-h)) / 2h` cancels it exactly
        //   and leaves `O(h^3)`.
        //
        // What is left is dominated by the micron quantum, `0.001 / (2h)`, which
        // at `h = 0.01` is `0.05` on a coefficient of `5` — hence the 1%
        // tolerance, and hence the fact that it does not tighten with smaller
        // `h`. Measured relative errors across this range: 0.07%.
        let normal = (0.0, -1.0);
        let predicted = rotation_coefficient(normal, deepest, centre);
        for &h in &[1e-2f64, 2e-2, 4e-2] {
            let plus = envelope_max_y(&apply_se2(&base, centre, (0.0, 0.0, h)));
            let minus = envelope_max_y(&apply_se2(&base, centre, (0.0, 0.0, -h)));
            let measured = (-plus - -minus) / (2.0 * h);
            assert!(
                (measured - predicted).abs() <= 1e-2 * predicted.abs(),
                "h={h}: miter central difference {measured} vs coefficient {predicted}"
            );
        }

        // (2) At the full cap the linear model plus the slack must still bound
        // the exactly rotated envelope.
        let reach_mm = envelope
            .regions
            .iter()
            .flat_map(|region| region.outer.source_points().iter().copied())
            .fold(0.0f64, |best, point| {
                best.max((point.x - centre.x).hypot(point.y - centre.y))
            });
        let theta_cap = 1.0 / reach_mm; // the cap a 1 mm trust radius produces
        let slack_mm = rotation_chord_slack_mm(reach_mm, theta_cap);
        for &sign in &[1.0f64, -1.0] {
            let theta = sign * theta_cap;
            let exact_max_y = envelope_max_y(&apply_se2(&base, centre, (0.0, 0.0, theta)));
            // The model's prediction for `max_y`, from the row's own linear
            // form, relaxed by the slack.
            let modelled_max_y = deepest.y - theta * predicted + slack_mm;
            assert!(
                exact_max_y <= modelled_max_y + 1e-9,
                "theta={theta}: exact miter depth {exact_max_y} exceeds the relaxed model \
                 {modelled_max_y} (slack {slack_mm})"
            );
        }
    }

    /// `apply_se2` with a zero delta must be the identity, to the bit — the
    /// witness path runs it on every piece of every program, including the ones
    /// the solver never moved, and a placement that drifts under a no-op would
    /// make every reported depth suspect.
    #[test]
    fn applying_a_zero_vector_returns_the_placement_unchanged() {
        for &(rotation_deg, mirrored) in &[(0.0, false), (17.5, false), (-93.25, true)] {
            let base = GeneralFastPlacement {
                rotation_deg,
                mirrored,
                ..placement("p", 12.5, -3.25)
            };
            let moved = apply_se2(&base, IrregularPoint::new(40.0, 21.0), (0.0, 0.0, 0.0));
            assert_eq!(moved.rotation_deg, base.rotation_deg);
            assert_eq!(moved.translate_short_axis, base.translate_short_axis);
            assert_eq!(moved.translate_long_axis, base.translate_long_axis);
        }
    }

    /// A lone piece well inside a deep sheet has room, and the certificate must
    /// both say so and hand back a vector that really achieves it.
    #[test]
    fn an_isolated_piece_gets_a_positive_and_exactly_validated_witness() {
        let polygon = rectangle(20.0, 10.0);
        let pieces = [piece("solo", &polygon, true)];
        let placements = [placement("solo", 10.0, 40.0)];
        let settings = zero_clearance_settings(200.0, 200.0);

        let certificate =
            se2_rigidity_certificate(&pieces, &placements, settings, 1.0, 400, None).unwrap();

        assert_eq!(certificate.piece_count, 1);
        assert_eq!(certificate.rotatable_piece_count, 1);
        // The publication measure is `max_y + edge clearance` = 50 + 0.
        assert!((certificate.published_depth_mm - 50.0).abs() < 1e-9);

        let headline = certificate
            .programs
            .iter()
            .find(|p| p.program == Se2Program::DepthOnly && p.motion == Se2Motion::Se2)
            .unwrap();
        assert_eq!(headline.verdict, "positive");
        let witness = headline.witness.as_ref().unwrap();
        assert!(witness.exact_valid, "{:?}", witness.full_vector_rejection);
        // A 1 mm trust radius moves the whole piece 1 mm shallower, and the
        // exact validator agrees.
        assert!(
            witness.delta_mm.unwrap() > 0.9,
            "delta {:?}",
            witness.delta_mm
        );
        bracket(headline);
    }

    /// The material depth family and the collision strip family must be
    /// different numbers off different geometry. With a positive collision
    /// expansion the envelope reaches further than the material outline, so the
    /// strip bound is strictly deeper — the separation Sol review 6 §3 asked
    /// for, and the quantity the previous branch hand-calibrated away.
    #[test]
    fn the_strip_bound_is_separated_from_the_published_depth() {
        let polygon = rectangle(20.0, 10.0);
        let pieces = [piece("solo", &polygon, true)];
        let placements = [placement("solo", 10.0, 40.0)];
        let settings = GeneralFastSettings {
            total_padding_mm: 5.0,
            sheet_edge_clearance_mm: Some(2.5),
            clearance_safety_margin_mm: 0.5,
            ..zero_clearance_settings(200.0, 200.0)
        };

        let certificate =
            se2_rigidity_certificate(&pieces, &placements, settings, 0.5, 200, None).unwrap();

        assert!(
            certificate.strip_excess_mm > 0.0,
            "strip {} vs published {}",
            certificate.strip_bound_mm,
            certificate.published_depth_mm
        );
        // Both families exist and are counted apart.
        assert!(certificate.rows_by_family.contains_key("MaterialDepth"));
        assert!(certificate.rows_by_family.contains_key("EnvelopeStrip"));
    }

    /// A triangle whose apex sits **off** the centroid's vertical.
    ///
    /// This is the shape the rotation test needs, and the reason is worth
    /// stating: an axis-aligned rectangle cannot benefit from rotation at all.
    /// Its depth is set by *two* top corners with equal and opposite lever
    /// arms (`a_theta = +L` and `-L`), so whichever way it turns, one of them
    /// goes deeper and the depth row's minimum gets worse — the certificate
    /// correctly reports `dtheta = 0` for it. A single peak whose lever arm is
    /// non-zero is what makes rotation a real lever, and a peak directly above
    /// the centre has `a_theta = n . J(0, h) = 0`, so it has to be off to one
    /// side as well.
    fn peaked_triangle() -> PolygonSet {
        PolygonSet {
            regions: vec![PolygonRegion {
                outer: PolygonRing::new(
                    vec![
                        IrregularPoint::new(0.0, 0.0),
                        IrregularPoint::new(40.0, 0.0),
                        IrregularPoint::new(35.0, 30.0),
                    ],
                    RingRole::Outer,
                )
                .unwrap(),
                holes: Vec::new(),
            }],
        }
    }

    /// The end-to-end test Sol review 6 §3 named as missing: a case where
    /// `dtheta != 0` **changes the verdict**, not merely the number.
    ///
    /// Both columns are the same program on the same rows over the same
    /// translation box; the only difference is whether the angular unknowns are
    /// pinned at zero. Against a reference that sits between the two
    /// achievable depths, translation alone comes back
    /// `positive-below-reference` and SE(2) comes back
    /// `positive-reaches-reference` — and both numbers are the exactly
    /// validated ones, so the verdict change survives the linearization rather
    /// than living inside it.
    #[test]
    fn rotation_changes_the_verdict_end_to_end() {
        let triangle = peaked_triangle();
        let pieces = [piece("peak", &triangle, true)];
        let placements = [placement("peak", 40.0, 60.0)];
        let settings = zero_clearance_settings(200.0, 400.0);
        // Translation alone can buy at most the trust radius. Rotation adds the
        // apex's lever arm on top, so a reference just above the trust radius
        // separates the two verdicts.
        let reference_mm = 1.15;

        let certificate = se2_rigidity_certificate(
            &pieces,
            &placements,
            settings,
            1.0,
            4_000,
            Some(reference_mm),
        )
        .unwrap();

        let translation = certificate
            .programs
            .iter()
            .find(|p| p.program == Se2Program::DepthOnly && p.motion == Se2Motion::TranslationOnly)
            .unwrap();
        let se2 = certificate
            .programs
            .iter()
            .find(|p| p.program == Se2Program::DepthOnly && p.motion == Se2Motion::Se2)
            .unwrap();

        let translation_witness = translation.witness.as_ref().unwrap();
        let se2_witness = se2.witness.as_ref().unwrap();
        assert!(
            translation_witness.exact_valid,
            "{:?}",
            translation_witness.full_vector_rejection
        );
        assert!(
            se2_witness.exact_valid,
            "{:?}",
            se2_witness.full_vector_rejection
        );

        assert_eq!(
            translation_witness.max_abs_dtheta_deg, 0.0,
            "the translation-only column must not turn anything"
        );
        assert!(
            se2_witness.max_abs_dtheta_deg > 0.0,
            "the SE(2) column must actually turn something"
        );

        let translation_delta = translation_witness.delta_mm.unwrap();
        let se2_delta = se2_witness.delta_mm.unwrap();
        assert!(
            se2_delta > translation_delta,
            "SE(2) {se2_delta} did not beat translation {translation_delta}"
        );

        // The verdict itself moves, which is the point of the test.
        assert_eq!(translation.verdict, "positive-below-reference");
        assert_eq!(se2.verdict, "positive-reaches-reference");
        assert_eq!(certificate.verdict, "positive-reaches-reference");

        for result in [translation, se2] {
            bracket(result);
        }
    }

    /// The companion negative: an axis-aligned rectangle's two top corners have
    /// equal and opposite lever arms, so rotation cannot help and the
    /// certificate must not pretend it does. This is the case that would fail
    /// loudly if the rotational coefficients ever lost their sign.
    #[test]
    fn rotation_does_not_help_a_shape_with_two_symmetric_peaks() {
        let polygon = rectangle(40.0, 20.0);
        let pieces = [piece("block", &polygon, true)];
        let placements = [placement("block", 40.0, 60.0)];
        let settings = zero_clearance_settings(200.0, 400.0);

        let certificate =
            se2_rigidity_certificate(&pieces, &placements, settings, 1.0, 2_000, None).unwrap();

        let translation = certificate
            .programs
            .iter()
            .find(|p| p.program == Se2Program::DepthOnly && p.motion == Se2Motion::TranslationOnly)
            .unwrap();
        let se2 = certificate
            .programs
            .iter()
            .find(|p| p.program == Se2Program::DepthOnly && p.motion == Se2Motion::Se2)
            .unwrap();

        let translation_delta = translation.witness.as_ref().unwrap().delta_mm.unwrap();
        let se2_delta = se2.witness.as_ref().unwrap().delta_mm.unwrap();
        // Rotation is available and simply does not pay: the two deltas agree
        // to well inside the grid quantum.
        assert!(
            (se2_delta - translation_delta).abs() < 0.001,
            "rotation changed a symmetric block's depth: {translation_delta} -> {se2_delta}"
        );
    }

    /// A piece the request pins must not be handed rotational freedom: a
    /// witness that rotated it could not be published, so it would not be a
    /// witness.
    #[test]
    fn a_piece_that_may_not_rotate_gets_no_angular_freedom() {
        let polygon = rectangle(20.0, 10.0);
        let pieces = [piece("pinned", &polygon, false)];
        let placements = [placement("pinned", 10.0, 40.0)];
        let settings = zero_clearance_settings(200.0, 200.0);

        let certificate =
            se2_rigidity_certificate(&pieces, &placements, settings, 1.0, 200, None).unwrap();

        assert_eq!(certificate.rotatable_piece_count, 0);
        assert_eq!(certificate.theta_cap_max_deg, 0.0);
        for result in &certificate.programs {
            assert_eq!(
                result.witness.as_ref().unwrap().max_abs_dtheta_deg,
                0.0,
                "a pinned piece was rotated by {:?}/{:?}",
                result.program,
                result.motion
            );
        }
    }

    /// The outward rounding has to be a real interval, not decoration.
    #[test]
    fn the_rounded_sum_brackets_its_own_exact_value() {
        let mut sum = RoundedSum::default();
        for term in [1e16, 1.0, -1e16, 0.5] {
            sum.add(term);
        }
        assert!(sum.low() <= sum.high());
        // The exact real sum is 1.5; f64 loses it, and the allowance must be
        // wide enough to still contain it.
        assert!(
            sum.low() <= 1.5 && 1.5 <= sum.high(),
            "[{}, {}] does not contain 1.5",
            sum.low(),
            sum.high()
        );
    }

    /// Every reported bracket must satisfy `lower <= upper` after outward
    /// rounding, on a real multi-piece front rather than a contrived one.
    #[test]
    fn every_program_reports_a_consistent_bracket_on_a_packed_front() {
        let polygon = rectangle(20.0, 10.0);
        let pieces: Vec<_> = ["a", "b", "c", "d"]
            .iter()
            .map(|id| piece(id, &polygon, true))
            .collect();
        let placements = [
            placement("a", 0.0, 0.0),
            placement("b", 20.0, 0.0),
            placement("c", 0.0, 10.0),
            placement("d", 20.0, 10.0),
        ];
        let settings = zero_clearance_settings(40.0, 100.0);

        let certificate =
            se2_rigidity_certificate(&pieces, &placements, settings, 0.25, 300, Some(0.422))
                .unwrap();

        assert_eq!(certificate.piece_count, 4);
        assert_eq!(certificate.programs.len(), 4);
        for result in &certificate.programs {
            bracket(result);
            assert!(result.delta_rows > 0);
        }
        // The strip-coupled program is strictly harder than the depth-only one,
        // so its upper bound can never exceed the depth-only bound at the same
        // motion.
        let depth_only = certificate
            .programs
            .iter()
            .find(|p| p.program == Se2Program::DepthOnly && p.motion == Se2Motion::Se2)
            .unwrap();
        let coupled = certificate
            .programs
            .iter()
            .find(|p| p.program == Se2Program::StripCoupled && p.motion == Se2Motion::Se2)
            .unwrap();
        assert!(bracket(coupled).0 <= bracket(depth_only).1);
    }

    /// The guard band must grow with the rotation, not stay at `2 * trust`.
    /// Turning rotation on can only add rows, never remove them.
    #[test]
    fn the_rotational_guard_band_admits_at_least_the_translational_one() {
        let polygon = rectangle(20.0, 10.0);
        let placements = [placement("a", 0.0, 0.0), placement("b", 25.0, 0.0)];
        let settings = zero_clearance_settings(80.0, 100.0);

        let pinned: Vec<_> = ["a", "b"]
            .iter()
            .map(|id| piece(id, &polygon, false))
            .collect();
        let free: Vec<_> = ["a", "b"]
            .iter()
            .map(|id| piece(id, &polygon, true))
            .collect();

        let pinned_certificate =
            se2_rigidity_certificate(&pinned, &placements, settings, 1.0, 100, None).unwrap();
        let free_certificate =
            se2_rigidity_certificate(&free, &placements, settings, 1.0, 100, None).unwrap();

        assert!(
            free_certificate.rows >= pinned_certificate.rows,
            "rotational band {} is narrower than the translational one {}",
            free_certificate.rows,
            pinned_certificate.rows
        );
        assert!(free_certificate.theta_cap_max_deg > 0.0);
        assert_eq!(pinned_certificate.theta_cap_max_deg, 0.0);
    }

    /// The guard band's **translation** term is unconditional.
    ///
    /// Two pieces the request pins against rotation still translate the full
    /// trust radius each, so a pair 1.5 mm apart at a 1 mm radius can be driven
    /// into contact and must get a row. An earlier draft recovered the radius
    /// from `theta_cap * reach`, which is zero for a pinned piece, so this pair
    /// got a band of just the clearance contract and no row at all — the
    /// program was then free to drive them straight through each other. That is
    /// the same insufficient-reach defect Sol review 6 §3 named, so it gets a
    /// test that fails on it rather than a comment.
    ///
    /// The check is one-sided on purpose: the row must exist. Asserting a row
    /// count would pin the boundary families' vertex pruning too, which is a
    /// different mechanism and free to change.
    #[test]
    fn a_pinned_pair_inside_the_translation_band_still_gets_a_row() {
        let polygon = rectangle(20.0, 10.0);
        // Two rectangles with a 1.5 mm horizontal gap: less than `2 * trust`,
        // more than any clearance contract here (there is none).
        let placements = [placement("a", 0.0, 0.0), placement("b", 21.5, 0.0)];
        let pinned: Vec<_> = ["a", "b"]
            .iter()
            .map(|id| piece(id, &polygon, false))
            .collect();
        let settings = zero_clearance_settings(80.0, 100.0);

        let certificate =
            se2_rigidity_certificate(&pinned, &placements, settings, 1.0, 100, None).unwrap();

        assert_eq!(certificate.theta_cap_max_deg, 0.0, "both pieces are pinned");
        let pair_rows = certificate
            .rows_by_family
            .get("MaterialPair")
            .copied()
            .unwrap_or(0)
            + certificate
                .rows_by_family
                .get("EnvelopePair")
                .copied()
                .unwrap_or(0);
        assert!(
            pair_rows > 0,
            "a pinned pair 1.5 mm apart at a 1 mm trust radius got no pair row: {:?}",
            certificate.rows_by_family
        );
    }

    /// The witness always comes back exactly validated, whatever the model did.
    ///
    /// This is the property the line search exists to guarantee, and it is the
    /// one Sol review 6 §3 asked for by name — a constructive lower bound that
    /// is "applicable and exactly validatable". On a packed front the model's
    /// full-length vector is routinely rejected (the rows are relaxed outward,
    /// so its optimum sits microns past a strict boundary test); the search
    /// must still return a validated layout and a real number, never a shrug.
    #[test]
    fn the_witness_is_exactly_valid_on_every_program_even_when_the_model_overreaches() {
        let polygon = rectangle(20.0, 10.0);
        let pieces: Vec<_> = ["a", "b", "c", "d"]
            .iter()
            .map(|id| piece(id, &polygon, true))
            .collect();
        // Deliberately tight: four pieces packed against each other and the
        // sheet edges, with a trust radius far larger than the room available.
        let placements = [
            placement("a", 0.0, 0.0),
            placement("b", 20.0, 0.0),
            placement("c", 0.0, 10.0),
            placement("d", 20.0, 10.0),
        ];
        let settings = zero_clearance_settings(40.0, 100.0);

        let certificate =
            se2_rigidity_certificate(&pieces, &placements, settings, 1.0, 500, Some(0.422))
                .unwrap();

        for result in &certificate.programs {
            let witness = result.witness.as_ref().unwrap_or_else(|| {
                panic!(
                    "{:?}/{:?} produced no witness at all: {:?}",
                    result.program, result.motion, result.witness_error
                )
            });
            assert!(
                witness.exact_valid,
                "{:?}/{:?} returned a witness the exact validator rejects",
                result.program, result.motion
            );
            assert!(
                witness.delta_mm.is_some(),
                "{:?}/{:?} returned no depth reduction",
                result.program,
                result.motion
            );
            assert!(
                witness.delta_mm.unwrap() >= -1e-9,
                "{:?}/{:?} returned a witness that made the layout DEEPER: {:?}",
                result.program,
                result.motion,
                witness.delta_mm
            );
            assert!(
                (0.0..=1.0).contains(&witness.scale),
                "{:?}/{:?} scale {} outside [0, 1]",
                result.program,
                result.motion,
                witness.scale
            );
            assert!(witness.validations > 0);
            // The scale really is applied: nothing in the reported vector may
            // exceed the box, and a zero scale must report a zero vector.
            for &(_, dx, dy, dtheta_deg) in &witness.vector {
                assert!(dx.abs() <= 1.0 + 1e-9 && dy.abs() <= 1.0 + 1e-9);
                if witness.scale == 0.0 {
                    assert_eq!((dx, dy, dtheta_deg), (0.0, 0.0, 0.0));
                }
            }
            // A verdict that depends on a rejected witness is no longer
            // reachable, so none of the four may report one.
            assert_ne!(result.verdict, "witness-rejected");
        }
    }

    /// The parent's own residuals are reported, not calibrated away.
    ///
    /// Correction 1's whole point: with the material depth read off the
    /// publication measure and the strip bound off the collision envelopes,
    /// both depth families are satisfied by the parent *by construction* — the
    /// bound is the parent's own maximum. The previous branch imposed one
    /// `sheet_long_axis_mm` on both and then raised it by hand until the
    /// violations stopped, so this asserts the violations never appear.
    #[test]
    fn the_parent_satisfies_its_own_depth_and_strip_rows_without_calibration() {
        let polygon = rectangle(20.0, 10.0);
        let pieces: Vec<_> = ["a", "b", "c"]
            .iter()
            .map(|id| piece(id, &polygon, true))
            .collect();
        let placements = [
            placement("a", 5.0, 5.0),
            placement("b", 30.0, 5.0),
            placement("c", 5.0, 20.0),
        ];
        let settings = GeneralFastSettings {
            total_padding_mm: 1.0,
            sheet_edge_clearance_mm: Some(2.0),
            clearance_safety_margin_mm: 0.25,
            ..zero_clearance_settings(120.0, 200.0)
        };

        let certificate =
            se2_rigidity_certificate(&pieces, &placements, settings, 0.5, 200, None).unwrap();

        for family in ["MaterialDepth", "EnvelopeStrip"] {
            let worst = certificate
                .parent_worst_residual_mm
                .get(family)
                .copied()
                .unwrap_or_else(|| panic!("no {family} rows at all"));
            assert!(
                worst >= -1e-9,
                "the parent violates its own {family} row by {worst} mm — the bound is \
                 measured off the parent, so this can only mean the row is built wrong"
            );
        }
        // And every program still found a feasible point to bound from.
        for result in &certificate.programs {
            assert!(
                result.lp.primal_feasible,
                "{:?}/{:?} found no feasible point on a legal parent",
                result.program, result.motion
            );
        }
    }
}
