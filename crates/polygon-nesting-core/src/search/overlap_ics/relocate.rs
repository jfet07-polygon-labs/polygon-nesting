//! **The routine move: one global relocate of one colliding piece.**
//!
//! This is the Algorithm 5-6 analogue of arXiv:2509.13329, implemented on our
//! state types, our deterministic counter-based sampler, and our source-ring
//! signed-gap Φ. The dynamics were read at Sparrow rev `14f4868f` -
//! `sample/search.rs::search_placement` (the 25 + 50 pool, the three unique
//! finalists, the two coordinate-descent stages),
//! `sample/coord_descent.rs::refine_coord_desc` (the five axes, the 1.1/0.5
//! step schedule, the accept-equal `tell`), `sample/best_samples.rs`
//! (uniqueness and the acceptance upper bound), `sample/uniform_sampler.rs`
//! (the 16 sampled orientations) and `optimizer/worker.rs::move_items`
//! (the Gauss-Seidel sweep over the colliding set). **No source text is
//! copied**; the citations are so a reader can check the semantics, and the
//! differences are listed below rather than smoothed over.
//!
//! Our differences from what that source does, all deliberate:
//!
//! * **the field.** Their sample evaluation is a pole-based overlap-area proxy
//!   (Algorithms 3-4) on simplified polygons in `f32`. Ours is the incremental
//!   incident signed-gap Φ over *source* rings in `f64`, weighted by the same
//!   rows the fold uses. No `jagua-rs`, no pole proxy, no simplification.
//! * **the sampler.** They draw from `Xoshiro256PlusPlus`. We draw from
//!   `counter_hash`/`rotated_halton` keyed by
//!   `(seed, bite, iteration, worker, piece, ordinal)`, so a trajectory is a
//!   function of the key and two processes agree bit for bit.
//! * **the coordinate.** Their uniform sampler draws a *transformation
//!   translation* bounded so the rotated shape bbox stays in the container. We
//!   draw a **centroid** position bounded by the same construction, because our
//!   strip box already exists in that coordinate and carries the campaign's
//!   clearance split - physical `edge + sag` on left, right and bottom, and the
//!   sag-less `T - depth_top_inset` on top ([`strip_sample_box`]).
//! * **no `Invalid` verdict.** Their evaluator can return `Invalid` for a pose
//!   outside the container. Our four boundary rows are already part of Φ, so a
//!   pose hanging out of the strip is a *collision*, scored and ranked like any
//!   other. Only a non-finite pose is refused outright.
//!
//! What is deliberately **absent**, because it is what a neutered relocate is
//! made of (Grok review 12 Round 2 §6.4, the pre-named most-likely defect):
//! there is no `after < before` filter, no `ladder_top` step cap, no maximum
//! displacement, and no exact predicate anywhere in this file. The current pose
//! is in the choice set, so "never make this piece's weighted incident Φ worse"
//! is an *emergent* property of picking the best sample - not a gate that could
//! reject a distant winner.

use std::cmp::Ordering;

use super::descent::{counter_hash, rotated_halton};
use super::diagnostics::WorkVector;
use super::energy::{incident_totals, rebuild_piece_rows};
use super::state::{
    apply_pose, compose_proposal, pose_sin_cos, transform_piece, Contract, IcsState, PieceSource,
    Pose,
};

/// The frozen knobs of one relocate. Every number is Sparrow's published
/// default, cited at its use site, and fixed before any wall number exists;
/// docs/cutclose-relocate-spec.md lists "sample counts fitted to
/// mixed-61/168.484" among the forbidden rescues.
#[derive(Clone, Copy, Debug)]
pub struct RelocateConfig {
    /// 25 samples in the piece's own current AABB.
    pub focused_samples: usize,
    /// 50 samples across the whole usable strip at the current `T`.
    pub container_samples: usize,
    /// The equally spaced orientations a rotatable piece draws from:
    /// `k * 360 / 16` degrees. A frozen piece keeps its own angle.
    pub sampled_orientations: usize,
    /// How many unique finalists get a coarse coordinate descent.
    pub finalists: usize,
    /// Two samples are the same sample when both translation components differ
    /// by less than `unique_translation_ratio * min_dim` **and** the angles
    /// differ by less than `unique_angle_deg`.
    pub unique_translation_ratio: f64,
    pub unique_angle_deg: f64,
    /// The coarse stage, run from each finalist.
    pub coarse: CoordDescentStage,
    /// The fine stage, run once from the winner.
    pub fine: CoordDescentStage,
    /// Step multiplier on a strict improvement.
    pub step_success: f64,
    /// Step multiplier on anything else.
    pub step_fail: f64,
}

/// One coordinate-descent stage: where the step starts and where it stops.
///
/// Translation steps are ratios of the piece's own `min_bbox_dim_mm`; rotation
/// steps are absolute degrees. Sparrow `consts.rs`, rev `14f4868f`:
/// `PRE_REFINE_CD_TL_RATIOS = (0.25, 0.02)`, `PRE_REFINE_CD_R_STEPS = (5°, 1°)`,
/// `SND_REFINE_CD_TL_RATIOS = (0.01, 0.001)`, `SND_REFINE_CD_R_STEPS =
/// (0.5°, 0.05°)`.
#[derive(Clone, Copy, Debug)]
pub struct CoordDescentStage {
    pub translation_init_ratio: f64,
    pub translation_limit_ratio: f64,
    pub rotation_init_deg: f64,
    pub rotation_limit_deg: f64,
}

impl Default for RelocateConfig {
    fn default() -> Self {
        Self {
            focused_samples: 25,
            container_samples: 50,
            sampled_orientations: 16,
            finalists: 3,
            unique_translation_ratio: 0.05,
            unique_angle_deg: 1.0,
            coarse: CoordDescentStage {
                translation_init_ratio: 0.25,
                translation_limit_ratio: 0.02,
                rotation_init_deg: 5.0,
                rotation_limit_deg: 1.0,
            },
            fine: CoordDescentStage {
                translation_init_ratio: 0.01,
                translation_limit_ratio: 0.001,
                rotation_init_deg: 0.5,
                rotation_limit_deg: 0.05,
            },
            step_success: 1.1,
            step_fail: 0.5,
        }
    }
}

/// The counter key one relocate draws its whole sample stream from.
///
/// Never a clock, never an address, never an iteration count another machine
/// could reach differently. `worker` is the Algorithm-10 ordinal the schedule
/// agent will fan out over; it is carried here so that the eight workers of one
/// master iteration draw *different* streams from the *same* master state
/// without any of them owning a generator.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RelocateKey {
    pub seed: u64,
    pub bite: u64,
    pub iteration: u64,
    pub worker: u64,
}

impl RelocateKey {
    /// The per-piece root of the stream.
    pub fn piece_key(self, piece: usize) -> u64 {
        counter_hash(&[
            self.seed,
            self.bite,
            self.iteration,
            self.worker,
            piece as u64,
            RELOCATE_STREAM_TAG,
        ])
    }
}

/// A domain tag so a relocate stream can never collide with a permutation
/// stream or a disruption draw keyed by the same tuple.
const RELOCATE_STREAM_TAG: u64 = 0x5245_4C4F_4341_5445; // "RELOCATE"
const PERMUTATION_STREAM_TAG: u64 = 0x5045_524D_5554_4531; // "PERMUTE1"
const AXIS_STREAM_TAG: u64 = 0x4344_4158_4953_5F31; // "CDAXIS_1"

/// One sample's score: the lexicographic `Clear < Collision{loss}` of
/// `eval/sample_eval.rs`, on our two incident totals.
///
/// `raw == 0` is "this piece is collision-free here", and it beats **every**
/// positive-Φ pose regardless of how good the weighted number is. Below that,
/// the order is the weighted incident Φ, which is where the guided weights do
/// their work.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SampleEval {
    pub raw: f64,
    pub weighted: f64,
}

impl SampleEval {
    /// The sentinel for a pose that cannot be evaluated at all - a non-finite
    /// transform. This is the one thing our field refuses outright; being
    /// outside the sheet is a boundary-row collision and is scored, not
    /// refused.
    pub const INVALID: Self = Self {
        raw: f64::INFINITY,
        weighted: f64::INFINITY,
    };

    pub fn is_clear(self) -> bool {
        self.raw <= 0.0
    }
}

/// The total order on sample evaluations.
///
/// Two clear samples compare **equal**, exactly as Sparrow's `SampleEval::Clear`
/// is a payload-free variant: once a pose is collision-free for this piece
/// there is nothing left in this objective to prefer between two of them, and
/// pretending otherwise would smuggle a second criterion into a lexicographic
/// rule that the spec froze.
pub fn eval_cmp(left: SampleEval, right: SampleEval) -> Ordering {
    match (left.is_clear(), right.is_clear()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => left
            .weighted
            .partial_cmp(&right.weighted)
            .unwrap_or(Ordering::Equal),
    }
}

/// The coordinate descent's acceptance rule: **accept anything not worse.**
///
/// `sample/coord_descent.rs::tell`, rev `14f4868f`: `if !worse { pos = candidate }`.
/// Equal is legal, and that is the half of the member the previous round's
/// `if after < before` deleted. A plateau is crossed rather than sat on.
#[inline]
pub fn cd_accepts(current: SampleEval, candidate: SampleEval) -> bool {
    eval_cmp(candidate, current) != Ordering::Greater
}

/// Where a committed pose came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleOrigin {
    /// The pose the piece already had. Always in the pool.
    StayPut,
    /// One of the 25 draws inside the piece's own current AABB.
    Focused,
    /// One of the 50 draws across the usable strip.
    Container,
}

impl SampleOrigin {
    pub fn label(self) -> &'static str {
        match self {
            SampleOrigin::StayPut => "stayPut",
            SampleOrigin::Focused => "focused",
            SampleOrigin::Container => "container",
        }
    }
}

/// What one relocate did.
#[derive(Clone, Copy, Debug)]
pub struct RelocateOutcome {
    pub piece: usize,
    /// `false` when the piece had no incident raw Φ and was therefore not in
    /// the colliding set at all - the `ct.get_loss(pk) > 0.0` filter of
    /// `optimizer/worker.rs::move_items`. Nothing was sampled.
    pub ran: bool,
    /// The committed pose differs from the entry pose in at least one bit.
    pub moved: bool,
    /// The origin of the seed the winning pose descended from.
    pub origin: SampleOrigin,
    /// How far the piece's transformed centroid travelled.
    pub displacement_mm: f64,
    /// The signed change in the pose angle, degrees.
    pub rotation_deg: f64,
    pub before: SampleEval,
    pub after: SampleEval,
    pub sample_evaluations: u64,
}

impl RelocateOutcome {
    fn skipped(piece: usize, before: SampleEval) -> Self {
        Self {
            piece,
            ran: false,
            moved: false,
            origin: SampleOrigin::StayPut,
            displacement_mm: 0.0,
            rotation_deg: 0.0,
            before,
            after: before,
            sample_evaluations: 0,
        }
    }
}

/// One candidate in the pool, with the provenance the counters are keyed on.
#[derive(Clone, Copy, Debug)]
struct Candidate {
    pose: Pose,
    eval: SampleEval,
    origin: SampleOrigin,
}

/// The `n` best **unique** samples, worst-evictable, with the acceptance upper
/// bound Sparrow's `BestSamples` uses to let an evaluator stop early.
///
/// The uniqueness rule is theirs (`sample/best_samples.rs`, rev `14f4868f`):
/// a new sample similar to existing ones is accepted only if it beats *all* of
/// them, and then it evicts all of them. The similarity test is
/// `|dx| < t && |dy| < t && angle diff < 1°`, and the angle comparison is ours
/// only in that we normalize into `[0, 360)` first - their `r % 2π` inherits a
/// wrap-around blind spot from the sign of the remainder, and our `theta_deg`
/// is documented to accumulate over the whole circle, so the raw remainder
/// would call `359.5°` and `0.5°` distinct.
struct BestSamples {
    size: usize,
    translation_threshold_mm: f64,
    angle_threshold_deg: f64,
    samples: Vec<Candidate>,
}

impl BestSamples {
    fn new(size: usize, translation_threshold_mm: f64, angle_threshold_deg: f64) -> Self {
        Self {
            size,
            translation_threshold_mm,
            angle_threshold_deg,
            samples: Vec::with_capacity(size + 1),
        }
    }

    fn upper_bound(&self) -> SampleEval {
        match self.samples.get(self.size.saturating_sub(1)) {
            Some(candidate) => candidate.eval,
            None => SampleEval::INVALID,
        }
    }

    fn similar(&self, left: Pose, right: Pose) -> bool {
        if (left.tx_mm - right.tx_mm).abs() >= self.translation_threshold_mm
            || (left.ty_mm - right.ty_mm).abs() >= self.translation_threshold_mm
        {
            return false;
        }
        angle_gap_deg(left.theta_deg, right.theta_deg) < self.angle_threshold_deg
    }

    fn report(&mut self, candidate: Candidate) -> bool {
        if eval_cmp(candidate.eval, self.upper_bound()) != Ordering::Less {
            return false;
        }
        let any_similar = self
            .samples
            .iter()
            .any(|held| self.similar(held.pose, candidate.pose));
        if any_similar {
            let better_than_all_similar = self
                .samples
                .iter()
                .filter(|held| self.similar(held.pose, candidate.pose))
                .all(|held| eval_cmp(candidate.eval, held.eval) == Ordering::Less);
            if !better_than_all_similar {
                return false;
            }
            let threshold = self.translation_threshold_mm;
            let angle = self.angle_threshold_deg;
            let pose = candidate.pose;
            self.samples
                .retain(|held| !poses_are_similar(held.pose, pose, threshold, angle));
        } else if self.samples.len() == self.size {
            self.samples.pop();
        }
        self.samples.push(candidate);
        self.samples
            .sort_by(|left, right| eval_cmp(left.eval, right.eval));
        true
    }

    fn best(&self) -> Option<Candidate> {
        self.samples.first().copied()
    }
}

fn poses_are_similar(left: Pose, right: Pose, translation_mm: f64, angle_deg: f64) -> bool {
    (left.tx_mm - right.tx_mm).abs() < translation_mm
        && (left.ty_mm - right.ty_mm).abs() < translation_mm
        && angle_gap_deg(left.theta_deg, right.theta_deg) < angle_deg
}

/// The unsigned angular distance between two accumulated degree coordinates,
/// in `[0, 180]`.
pub fn angle_gap_deg(left: f64, right: f64) -> f64 {
    let mut difference = (left - right) % 360.0;
    if difference < 0.0 {
        difference += 360.0;
    }
    if difference > 180.0 {
        360.0 - difference
    } else {
        difference
    }
}

/// The transformed centroid of a piece at an arbitrary pose.
///
/// Computed from the pose and the source centroid rather than read out of
/// `Geometry::centroids`, because a coordinate descent asks for the pivot of a
/// pose it has **not** installed yet - reading the cache would pivot each step
/// about the previous candidate's centroid and make the walk path-dependent.
#[inline]
pub fn transformed_centroid(source: &PieceSource, pose: Pose) -> [f64; 2] {
    let (sin, cos) = pose_sin_cos(pose.theta_deg);
    apply_pose(
        source.centroid,
        pose.mirrored,
        sin,
        cos,
        pose.tx_mm,
        pose.ty_mm,
    )
}

/// One wiggle step: a rotation of `dtheta_deg` **about the piece's transformed
/// centroid**, and nothing else.
///
/// The pivot is not a preference. `state::compose_proposal`'s own doc records
/// what turning about the pose origin instead cost the previous round: `|c − t|
/// · dtheta` of unmodelled rigid translation on every rotational step, 1.00 to
/// 1.35 times the modelled rotation on both campaign fixtures. A wiggle that
/// slid the piece sideways while claiming to test an angle would make the
/// rotation axis of this coordinate descent a lie in exactly the same way.
#[inline]
pub fn wiggle_pose(source: &PieceSource, pose: Pose, dtheta_deg: f64) -> Pose {
    compose_proposal(
        pose,
        transformed_centroid(source, pose),
        0.0,
        0.0,
        dtheta_deg,
    )
}

/// One axis of a low-discrepancy draw inside `[low, high]`.
///
/// **The infeasible case is clamped per axis and only per axis.** When a piece
/// cannot fit the interval at all (`high <= low`) this returns that interval's
/// midpoint, which is the deterministic best-centred position on *that* axis
/// and leaves the other axis and the angle free to keep varying. What it must
/// never do is let one jammed axis collapse the whole sample set onto a single
/// pose - the latent defect Grok review 10 found in the old jump's circumradius
/// box.
#[inline]
pub fn mix(low: f64, high: f64, unit: f64) -> f64 {
    if high <= low {
        (low + high) / 2.0
    } else {
        low + unit * (high - low)
    }
}

/// The extent of a piece's transformed outer ring relative to its own
/// transformed centroid, `[min dx, min dy, max dx, max dy]`, at one rotation.
pub fn centroid_relative_extents(source: &PieceSource, theta_deg: f64, mirrored: bool) -> [f64; 4] {
    let (sin, cos) = pose_sin_cos(theta_deg);
    let centre = apply_pose(source.centroid, mirrored, sin, cos, 0.0, 0.0);
    let mut out = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for point in &source.decomposition.ring {
        let placed = apply_pose(*point, mirrored, sin, cos, 0.0, 0.0);
        out[0] = out[0].min(placed[0] - centre[0]);
        out[1] = out[1].min(placed[1] - centre[1]);
        out[2] = out[2].max(placed[0] - centre[0]);
        out[3] = out[3].max(placed[1] - centre[1]);
    }
    out
}

/// The box of **centroid** positions whose piece AABB lies inside the usable
/// strip, given that piece's centroid-relative extents, as
/// `[low x, low y, high x, high y]`.
///
/// The four sides carry the clearance split of Sol review 15 §B.1 / Grok review
/// 10 §B.1: left, right and bottom are physical sheet edges at `edge + sag`;
/// the top is the tighter of the locked strip in the sag-less depth convention
/// and the physical sheet top. Charging one `edge_clearance` on all four sides
/// was the defect that manufactured a sag tolerance of phantom top-row
/// violation on every request with `sag > 0`.
pub fn strip_sample_box(contract: &Contract, target_depth_mm: f64, extents: [f64; 4]) -> [f64; 4] {
    let physical = contract.physical_edge_clearance_mm();
    let top = (target_depth_mm - contract.depth_top_inset_mm())
        .min(contract.sheet_long_axis_mm - physical);
    [
        physical - extents[0],
        physical - extents[1],
        contract.sheet_short_axis_mm - physical - extents[2],
        top - extents[3],
    ]
}

/// The pose that puts a piece's transformed centroid at `centre` with angle
/// `theta_deg`.
fn pose_at_centroid(
    source: &PieceSource,
    centre: [f64; 2],
    theta_deg: f64,
    mirrored: bool,
) -> Pose {
    let (sin, cos) = pose_sin_cos(theta_deg);
    let rotated = apply_pose(source.centroid, mirrored, sin, cos, 0.0, 0.0);
    Pose {
        tx_mm: centre[0] - rotated[0],
        ty_mm: centre[1] - rotated[1],
        theta_deg,
        mirrored,
    }
}

/// Installs a pose, refreshes only the rows it can have changed, and scores it.
///
/// Every relocate-eval in the whole member goes through here, which is why the
/// work counters are incremented here and nowhere else.
fn evaluate(
    state: &mut IcsState,
    sources: &[PieceSource],
    contract: &Contract,
    piece: usize,
    pose: Pose,
    work: &mut WorkVector,
) -> SampleEval {
    if !pose.tx_mm.is_finite() || !pose.ty_mm.is_finite() || !pose.theta_deg.is_finite() {
        return SampleEval::INVALID;
    }
    state.poses[piece] = pose;
    transform_piece(sources, &mut state.geometry, &state.poses, piece);
    work.pose_transforms += 1;
    rebuild_piece_rows(state, contract, piece, work);
    work.sample_evaluations += 1;
    let (raw, weighted) = incident_totals(state, piece);
    SampleEval { raw, weighted }
}

/// The five coordinate-descent axes of `sample/coord_descent.rs::CDAxis`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CdAxis {
    Horizontal,
    Vertical,
    ForwardDiagonal,
    BackwardDiagonal,
    Wiggle,
}

/// Their `CDAxis::random`: a uniform draw over `0..6` when the wiggle axis is
/// enabled (so a rotation axis is chosen a third of the time) and over `0..4`
/// when it is not. The draw is `counter_hash` rather than `Xoshiro`.
fn draw_axis(key: u64, wiggle: bool) -> CdAxis {
    let span = if wiggle { 6 } else { 4 };
    match key % span {
        0 => CdAxis::Horizontal,
        1 => CdAxis::Vertical,
        2 => CdAxis::ForwardDiagonal,
        3 => CdAxis::BackwardDiagonal,
        _ => CdAxis::Wiggle,
    }
}

/// **The two-stage axis coordinate descent, one stage per call.**
///
/// From a start pose, walk the five axes until every step size has fallen under
/// its limit, accepting anything not worse and rescaling the active axis by
/// `1.1` on a strict improvement and `0.5` otherwise. The diagonals rescale
/// both translation steps by `sqrt(m)` because both are involved. A step that
/// fails to improve also re-draws the axis.
///
/// The walk is bounded twice over: the step sizes are strictly decreasing
/// whenever they fail, and `MAX_CD_STEPS` is a hard ceiling matching the
/// `debug_assert!(n_evals < 1000)` their own descent carries.
pub fn coord_descent(
    state: &mut IcsState,
    sources: &[PieceSource],
    contract: &Contract,
    piece: usize,
    start: Pose,
    start_eval: SampleEval,
    stage: CoordDescentStage,
    config: &RelocateConfig,
    allow_rotation: bool,
    stream: u64,
    work: &mut WorkVector,
) -> (Pose, SampleEval) {
    let min_dim = sources[piece].min_bbox_dim_mm.max(f64::MIN_POSITIVE);
    let translation_limit = min_dim * stage.translation_limit_ratio;
    let rotation_limit = stage.rotation_limit_deg;
    let mut steps = (
        min_dim * stage.translation_init_ratio,
        min_dim * stage.translation_init_ratio,
    );
    let mut rotation_step = stage.rotation_init_deg;
    let mut pose = start;
    let mut eval = start_eval;
    let mut axis = draw_axis(counter_hash(&[stream, AXIS_STREAM_TAG, 0]), allow_rotation);
    for step_ordinal in 0..MAX_CD_STEPS {
        let stalled = steps.0 < translation_limit
            && steps.1 < translation_limit
            && (rotation_step < rotation_limit || !allow_rotation);
        if stalled {
            break;
        }
        let candidates = match axis {
            CdAxis::Horizontal => [
                translate(pose, steps.0, 0.0),
                translate(pose, -steps.0, 0.0),
            ],
            CdAxis::Vertical => [
                translate(pose, 0.0, steps.1),
                translate(pose, 0.0, -steps.1),
            ],
            CdAxis::ForwardDiagonal => [
                translate(pose, steps.0, steps.1),
                translate(pose, -steps.0, -steps.1),
            ],
            CdAxis::BackwardDiagonal => [
                translate(pose, -steps.0, steps.1),
                translate(pose, steps.0, -steps.1),
            ],
            CdAxis::Wiggle => [
                wiggle_pose(&sources[piece], pose, rotation_step),
                wiggle_pose(&sources[piece], pose, -rotation_step),
            ],
        };
        let first = evaluate(state, sources, contract, piece, candidates[0], work);
        let second = evaluate(state, sources, contract, piece, candidates[1], work);
        // `min_by_key` on their side; the first of a tie wins on ours too.
        let (candidate_pose, candidate_eval) = if eval_cmp(second, first) == Ordering::Less {
            (candidates[1], second)
        } else {
            (candidates[0], first)
        };
        let order = eval_cmp(candidate_eval, eval);
        let better = order == Ordering::Less;
        if order != Ordering::Greater {
            pose = candidate_pose;
            eval = candidate_eval;
        }
        let multiplier = if better {
            config.step_success
        } else {
            config.step_fail
        };
        match axis {
            CdAxis::Horizontal => steps.0 *= multiplier,
            CdAxis::Vertical => steps.1 *= multiplier,
            CdAxis::ForwardDiagonal | CdAxis::BackwardDiagonal => {
                let root = multiplier.sqrt();
                steps.0 *= root;
                steps.1 *= root;
            }
            CdAxis::Wiggle => rotation_step *= multiplier,
        }
        if !better {
            axis = draw_axis(
                counter_hash(&[stream, AXIS_STREAM_TAG, step_ordinal as u64 + 1]),
                allow_rotation,
            );
        }
    }
    (pose, eval)
}

/// The hard ceiling on one coordinate-descent walk. The step schedule already
/// terminates - every failure halves a step and the limits are positive - so
/// this only bounds a pathological alternation of successes and failures, and
/// it matches the `n_evals < 1000` assertion their own walk carries.
const MAX_CD_STEPS: usize = 500;

#[inline]
fn translate(pose: Pose, dtx: f64, dty: f64) -> Pose {
    Pose {
        tx_mm: pose.tx_mm + dtx,
        ty_mm: pose.ty_mm + dty,
        theta_deg: pose.theta_deg,
        mirrored: pose.mirrored,
    }
}

/// **One relocate of one piece: Algorithm 5-6 on our field.**
///
/// 1. If the piece's incident **raw** Φ is zero it is not in the colliding set
///    and nothing happens (`ct.get_loss(pk) > 0.0`).
/// 2. The current pose is evaluated and reported first, so "stay put" is always
///    in the choice set and no filter is needed to protect it.
/// 3. 25 focused samples: centroid uniform in the piece's own current AABB,
///    narrowed to the usable strip; angle from the 16-orientation set when the
///    piece may rotate, otherwise its current angle.
/// 4. 50 container samples: centroid uniform in the whole usable strip at the
///    current `T`, same angle draw. **This is the operator.** A colliding piece
///    can leave its neighbourhood entirely.
/// 5. The three best unique samples get a coarse coordinate descent; the best
///    of everything then gets a fine one.
/// 6. The winner is committed. Not "the winner if it improves" - the winner.
///
/// The returned pose is always installed and every incident row is consistent
/// with it on return, whether or not the pose changed.
#[allow(clippy::too_many_arguments)]
pub fn relocate(
    state: &mut IcsState,
    sources: &[PieceSource],
    contract: &Contract,
    allow_rotation: &[bool],
    piece: usize,
    config: &RelocateConfig,
    key: RelocateKey,
    work: &mut WorkVector,
) -> RelocateOutcome {
    let entry_pose = state.poses[piece];
    let (entry_raw, entry_weighted) = incident_totals(state, piece);
    let entry_eval = SampleEval {
        raw: entry_raw,
        weighted: entry_weighted,
    };
    if entry_raw <= 0.0 {
        return RelocateOutcome::skipped(piece, entry_eval);
    }
    work.relocates += 1;
    let evaluations_before = work.sample_evaluations;
    let source = &sources[piece];
    let rotates = allow_rotation[piece];
    let min_dim = source.min_bbox_dim_mm.max(f64::MIN_POSITIVE);
    let stream = key.piece_key(piece);

    let mut pool = BestSamples::new(
        config.finalists,
        min_dim * config.unique_translation_ratio,
        config.unique_angle_deg,
    );
    pool.report(Candidate {
        pose: entry_pose,
        eval: entry_eval,
        origin: SampleOrigin::StayPut,
    });

    // The piece's own current AABB, in the centroid coordinate the strip box is
    // written in. Their focused sampler is `UniformBBoxSampler::new(pi_bbox,
    // item, container_bbox)`: the piece's placed bbox, intersected with the
    // container's feasible range.
    let focused_box = state.geometry.piece_bounds[piece];
    let orientation_step = if config.sampled_orientations == 0 {
        0.0
    } else {
        360.0 / config.sampled_orientations as f64
    };

    for ordinal in 0..config.focused_samples {
        let pose = draw_sample(
            source,
            contract,
            state.target_depth_mm,
            entry_pose,
            rotates,
            orientation_step,
            config.sampled_orientations,
            Some(focused_box),
            stream,
            ordinal as u64,
        );
        let eval = evaluate(state, sources, contract, piece, pose, work);
        work.focused_samples += 1;
        pool.report(Candidate {
            pose,
            eval,
            origin: SampleOrigin::Focused,
        });
    }
    for ordinal in 0..config.container_samples {
        let pose = draw_sample(
            source,
            contract,
            state.target_depth_mm,
            entry_pose,
            rotates,
            orientation_step,
            config.sampled_orientations,
            None,
            stream,
            (config.focused_samples + ordinal) as u64,
        );
        let eval = evaluate(state, sources, contract, piece, pose, work);
        work.container_samples += 1;
        pool.report(Candidate {
            pose,
            eval,
            origin: SampleOrigin::Container,
        });
    }

    // Stage 1: a coarse walk from every finalist, each reported back into the
    // pool so a descended sample can evict a raw one.
    let finalists: Vec<Candidate> = pool.samples.clone();
    for (walk, finalist) in finalists.iter().enumerate() {
        let (pose, eval) = coord_descent(
            state,
            sources,
            contract,
            piece,
            finalist.pose,
            finalist.eval,
            config.coarse,
            config,
            rotates,
            counter_hash(&[stream, walk as u64, 1]),
            work,
        );
        pool.report(Candidate {
            pose,
            eval,
            origin: finalist.origin,
        });
    }

    // Stage 2: one finer walk from the winner.
    let best = pool.best().unwrap_or(Candidate {
        pose: entry_pose,
        eval: entry_eval,
        origin: SampleOrigin::StayPut,
    });
    let (final_pose, final_eval) = coord_descent(
        state,
        sources,
        contract,
        piece,
        best.pose,
        best.eval,
        config.fine,
        config,
        rotates,
        counter_hash(&[stream, u64::MAX, 2]),
        work,
    );

    // The commit. Always: the sample pool contained the entry pose, so this is
    // "move to the best pose we found", never "move only if it is better".
    state.poses[piece] = final_pose;
    transform_piece(sources, &mut state.geometry, &state.poses, piece);
    work.pose_transforms += 1;
    rebuild_piece_rows(state, contract, piece, work);

    let entry_centre = transformed_centroid(source, entry_pose);
    let final_centre = transformed_centroid(source, final_pose);
    let displacement_mm = libm::hypot(
        final_centre[0] - entry_centre[0],
        final_centre[1] - entry_centre[1],
    );
    let moved = final_pose.tx_mm.to_bits() != entry_pose.tx_mm.to_bits()
        || final_pose.ty_mm.to_bits() != entry_pose.ty_mm.to_bits()
        || final_pose.theta_deg.to_bits() != entry_pose.theta_deg.to_bits();
    if moved {
        work.accepted_moves += 1;
    }
    match best.origin {
        SampleOrigin::Container => {
            work.container_winners += 1;
            if moved {
                work.container_commits += 1;
            }
        }
        SampleOrigin::Focused => work.focused_winners += 1,
        SampleOrigin::StayPut => work.stay_put_winners += 1,
    }
    RelocateOutcome {
        piece,
        ran: true,
        moved,
        origin: best.origin,
        displacement_mm,
        rotation_deg: final_pose.theta_deg - entry_pose.theta_deg,
        before: entry_eval,
        after: final_eval,
        sample_evaluations: work.sample_evaluations - evaluations_before,
    }
}

/// One pool sample: an orientation from the 16-set (or the frozen angle) and a
/// centroid drawn uniformly in the sample box for **that** orientation.
///
/// `focused_box` is `Some(piece AABB)` for a focused draw and `None` for a
/// container-wide one. A focused draw is narrowed to the strip box, exactly as
/// their focused sampler intersects the placed bbox with the container range;
/// when that intersection is empty on an axis, [`mix`] falls back to the
/// interval midpoint on that axis alone rather than collapsing the pool.
#[allow(clippy::too_many_arguments)]
fn draw_sample(
    source: &PieceSource,
    contract: &Contract,
    target_depth_mm: f64,
    entry_pose: Pose,
    rotates: bool,
    orientation_step: f64,
    orientations: usize,
    focused_box: Option<[f64; 4]>,
    stream: u64,
    ordinal: u64,
) -> Pose {
    let key = counter_hash(&[stream, ordinal, 0]);
    let unit = [
        rotated_halton(2, ordinal + 1, key),
        rotated_halton(3, ordinal + 1, key >> 21),
        rotated_halton(5, ordinal + 1, key >> 42),
    ];
    let theta = if rotates && orientations > 0 {
        // 16 equally spaced absolute orientations, `k * 360/16`, chosen
        // uniformly - `uniform_sampler.rs`'s `ROT_N_SAMPLES` for a continuously
        // rotatable item. Not a catalogue: the coordinate descent's wiggle axis
        // then moves the angle continuously off the seed.
        let index = ((unit[2] * orientations as f64) as usize).min(orientations - 1);
        index as f64 * orientation_step
    } else {
        entry_pose.theta_deg
    };
    let extents = centroid_relative_extents(source, theta, entry_pose.mirrored);
    let strip = strip_sample_box(contract, target_depth_mm, extents);
    let sample_box = match focused_box {
        None => strip,
        Some(focused) => [
            strip[0].max(focused[0]),
            strip[1].max(focused[1]),
            strip[2].min(focused[2]),
            strip[3].min(focused[3]),
        ],
    };
    let centre = [
        mix(sample_box[0], sample_box[2], unit[0]),
        mix(sample_box[1], sample_box[3], unit[1]),
    ];
    pose_at_centroid(source, centre, theta, entry_pose.mirrored)
}

/// **The colliding set, in a counter-derived permutation.**
///
/// `optimizer/worker.rs::move_items` collects every placed item whose tracked
/// loss is positive and shuffles it; the sweep then re-checks each one before
/// relocating, because an earlier relocate in the same sweep may already have
/// cleared it. The permutation here is a Fisher-Yates over the counter stream,
/// so it is a function of `(seed, bite, iteration, worker)` alone and the eight
/// Algorithm-10 workers get eight different orders from one master state.
pub fn colliding_permutation(state: &IcsState, key: RelocateKey, out: &mut Vec<usize>) {
    out.clear();
    for piece in 0..state.poses.len() {
        if super::energy::incident_raw(state, piece) > 0.0 {
            out.push(piece);
        }
    }
    let root = counter_hash(&[
        key.seed,
        key.bite,
        key.iteration,
        key.worker,
        PERMUTATION_STREAM_TAG,
    ]);
    for index in (1..out.len()).rev() {
        let draw = counter_hash(&[root, index as u64]);
        let target = (draw % (index as u64 + 1)) as usize;
        out.swap(index, target);
    }
}
