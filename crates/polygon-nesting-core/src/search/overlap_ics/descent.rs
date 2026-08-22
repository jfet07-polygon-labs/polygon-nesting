//! The solver: a deterministic damped projected Gauss-Seidel descent over
//! continuous SE(2), a guided integer weight update after one stalled sweep,
//! and one topology jump after two guided stalls.
//!
//! What is *not* here is as load-bearing as what is:
//!
//! * **no exact predicate is consulted.** Not in the ladder, not in the
//!   acceptance test, not in the sweep order. That is the difference between
//!   this and contact-block, whose operator asked the exact composite to
//!   approve every line-search step and was reduced to 1% of its modelled
//!   vector.
//! * **no swaps, mirrors or restarts.** The converged spec holds them back so
//!   that a Round-1 failure is interpretable; the single jump is the one
//!   predeclared topology change.
//! * **no clock.** Budgets are work quotas. A wall-mode driver may stop the
//!   loop between whole sweeps, which is the only place the spec allows a
//!   deadline to be read.

use super::diagnostics::WorkVector;
use super::energy::{
    fold, incident_gradient, incident_guided, rebuild_piece_rows, sweep_order, Totals,
};
use super::state::{transform_piece, Contract, IcsState, PieceSource, Pose};

/// The frozen Round-1 knobs of the solver.
#[derive(Clone, Copy, Debug)]
pub struct DescentConfig {
    /// The top rung of the backtracking ladder, in millimetres.
    pub ladder_top_mm: f64,
    /// The bottom rung: 0.25 µm, the spec's floor.
    pub ladder_bottom_mm: f64,
    /// Stalled *guided* updates before the topology jump fires.
    pub stalls_before_jump: u32,
    /// Deterministic low-discrepancy relocations evaluated per jump.
    pub jump_samples: usize,
    /// Complete `n`-piece sweeps run from each relocation candidate — Sol R2
    /// §4's "one bounded local **sweep** from each", which is `n` proposals in
    /// this module's own vocabulary (`sweep`, below) and not four proposals of
    /// the relocated piece.
    pub jump_local_proposals: u32,
    /// How many jumps the whole trajectory may spend. Round 1 is 1.
    pub jump_allowance: u32,
    /// How many rejected proposals the census decomposes rung by rung, once it
    /// arms on the first stalled sweep. Read-only instrumentation; it cannot
    /// change a trajectory.
    pub rejection_census_samples: usize,
    /// Whether the jump commits its best candidate unconditionally.
    ///
    /// **`true`, and back to the spec's literal reading.** Sol R2 §2's jump
    /// "chooses by guided Φ" among the 16 relocations and "commits for a full
    /// epoch even if raw Φ temporarily worsens", with staying put not in the
    /// choice set. Gate 0 defaulted this to `false` on an A/B whose `true` arm
    /// was a *4-self-move strip teleport applied to a 12 µm residual* — both
    /// reviews' Finding 2 refuses that measurement as evidence about the
    /// mechanism, because the mechanism was never built. With a real `n`-piece
    /// sweep and the two-scale gate ([`JUMP_STRIP_THRESHOLD_MM`]), the
    /// unconditional commit is what makes a jump a topology change rather than
    /// one more descent step that "cannot change a topology at all".
    ///
    /// `false` stays reachable (`--jumpcommit=guided`) so the A/B is one
    /// command, but a `false` run can re-fire its jump on every second stall,
    /// because an uncommitted evaluation no longer spends the allowance.
    pub jump_commits_unconditionally: bool,
    /// The counter-based source's key. Never a clock, never an address.
    pub seed: u64,
}

impl DescentConfig {
    /// The ladder top is `max(clearance/4, median diameter/128, 8 µm)`, and the
    /// bottom is 0.25 µm — Sol review 14 §1's rungs, derived from the request
    /// rather than tuned.
    pub fn derive(contract: &Contract, sources: &[PieceSource], seed: u64) -> Self {
        let mut diameters: Vec<f64> = sources
            .iter()
            .map(|source| 2.0 * source.max_radius_mm)
            .collect();
        diameters.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
        let median = if diameters.is_empty() {
            0.0
        } else {
            diameters[diameters.len() / 2]
        };
        Self {
            ladder_top_mm: (contract.pair_clearance_mm() / 4.0)
                .max(median / 128.0)
                .max(0.008),
            ladder_bottom_mm: 0.00025,
            stalls_before_jump: 2,
            jump_samples: 16,
            jump_local_proposals: 1,
            jump_allowance: 1,
            jump_commits_unconditionally: true,
            rejection_census_samples: 32,
            seed,
        }
    }

    /// The ladder, top to bottom, halving. Deterministic and finite.
    pub fn ladder(&self) -> Vec<f64> {
        let mut rungs = Vec::new();
        let mut step = self.ladder_top_mm;
        while step > self.ladder_bottom_mm {
            rungs.push(step);
            step /= 2.0;
        }
        rungs.push(self.ladder_bottom_mm);
        rungs
    }
}

/// Reusable scratch so the descent allocates nothing per proposal.
pub struct Descent {
    pub config: DescentConfig,
    ladder: Vec<f64>,
    order: Vec<usize>,
    allow_rotation: Vec<bool>,
    /// Consecutive guided weight updates that produced no raw improvement.
    stalls: u32,
    jumps_spent: u32,
    /// Monotone proposal ordinal: the trajectory's own clock.
    pub proposals: u64,
    census: RejectionCensus,
    /// Scratch for the census's before/after row activity. Never read outside
    /// a recorded rejection.
    census_activity: Vec<bool>,
}

/// What one sweep did.
#[derive(Clone, Copy, Debug)]
pub struct SweepOutcome {
    pub accepted: usize,
    pub raw_before: f64,
    pub raw_after: f64,
    pub totals: Totals,
}

/// The scale a jump fired at. Derived from `max_g` alone, never from a knob.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JumpKind {
    /// `max_g > 0.100 mm`: the residual is millimetre-scale separation and the
    /// candidates are relocations anywhere in the usable strip.
    Strip,
    /// `max_g <= 0.100 mm`: no teleport. The candidates are drawn in a local
    /// SE(2) ball around the current pose.
    Ball,
}

impl JumpKind {
    pub fn label(self) -> &'static str {
        match self {
            JumpKind::Strip => "strip",
            JumpKind::Ball => "ball",
        }
    }
}

/// What one jump did — evaluated, installed, and at what scale.
///
/// `attempted` and `installed` are separate on purpose. The previous round's
/// documents could not tell a no-op from a committed guided improvement,
/// because the only counter was "the best candidate beat the baseline" and the
/// pose had already been restored (Grok review 10, "Same-class latent
/// defects"). Both numbers now reach the evidence.
#[derive(Clone, Copy, Debug)]
pub struct JumpOutcome {
    /// The 16 candidates were evaluated.
    pub attempted: bool,
    /// A candidate state was adopted. This, and only this, spends the
    /// trajectory's jump allowance.
    pub installed: bool,
    /// The best candidate's guided Φ beat the pre-jump guided Φ. Reported,
    /// never a gate under the default commit rule.
    pub improved_guided: bool,
    pub kind: JumpKind,
    pub piece: usize,
    /// The ball's translational radius; infinite for a strip relocation, whose
    /// candidates are bounded by the strip and not by a radius.
    pub radius_mm: f64,
    /// `max_g` at the moment the scale was chosen.
    pub max_violation_mm: f64,
    pub baseline_guided: f64,
    pub best_guided: f64,
}

impl Default for JumpOutcome {
    fn default() -> Self {
        Self {
            attempted: false,
            installed: false,
            improved_guided: false,
            kind: JumpKind::Ball,
            piece: 0,
            radius_mm: 0.0,
            max_violation_mm: 0.0,
            baseline_guided: 0.0,
            best_guided: 0.0,
        }
    }
}

/// One rung of a rejected proposal, decomposed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RejectionRung {
    pub step_mm: f64,
    /// The quantity the acceptance test actually reads. A rejection means this
    /// was `>= 0` on **every** rung.
    pub delta_incident_guided: f64,
    /// What the same step did to the *global* raw Φ. A step that lowers raw Φ
    /// while raising the incident guided energy is the guided weights refusing
    /// a real improvement, and it is worth being able to see that separately.
    pub delta_raw: f64,
    pub delta_max_violation_mm: f64,
    /// Rows incident on this piece that were clear before the step and are not
    /// after it: the violation the move would transfer onto a neighbour.
    pub newly_activated_rows: usize,
}

/// One rejected proposal, with everything both reviews asked to see before any
/// verdict about the move set is written down.
#[derive(Clone, Debug, PartialEq)]
pub struct RejectionRecord {
    pub proposal_ordinal: u64,
    pub piece: usize,
    /// `"translation"`, `"rotation"` or `"combined"`, from the SE(2)-normalized
    /// direction the whole ladder is walked along.
    pub direction_class: &'static str,
    /// The translational and rotational shares of that unit direction; their
    /// squares sum to 1 because the metric is `|dt|^2 + (R dtheta)^2`.
    pub translation_share: f64,
    pub rotation_share: f64,
    pub incident_guided_before: f64,
    pub raw_before: f64,
    pub guided_before: f64,
    pub max_violation_before_mm: f64,
    /// Rows incident on this piece that carry a violation, and the guided
    /// penalties of **those rows only** - Sol review 15 §A.3's objection to
    /// `maxGuidedPenalty`, which includes inactive rows and so cannot show that
    /// the blocking row ever received the weight.
    pub active_incident_rows: usize,
    pub active_incident_penalty_max: u32,
    pub active_incident_penalty_sum: u64,
    pub rungs: Vec<RejectionRung>,
}

/// The rejection census: a cheap count over the whole population and a bounded
/// decomposition of the rejections at the stall.
///
/// Both reviews refuse a move-set verdict without it (Sol review 15 §A.3,
/// Grok review 10 §B.4). It is **read-only**: enabling it cannot move a
/// trajectory, which is why the cells run with it on rather than as a separate
/// probe whose numbers would be a different run's.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RejectionCensus {
    /// Proposals that formed a gradient and were rejected on every rung.
    pub rejected: u64,
    /// Proposals that formed a gradient and were accepted.
    pub accepted: u64,
    /// Proposals that returned before forming a gradient because the piece had
    /// no incident energy at all. A zero-energy neighbour is immovable by
    /// construction (`before <= 0.0`), and that is the complement Grok review
    /// 10 Finding 3 names.
    pub zero_energy: u64,
    /// `[translation, rotation, combined]`.
    pub rejected_by_class: [u64; 3],
    pub accepted_by_class: [u64; 3],
    /// Whether the bounded sample has started collecting: it arms on the first
    /// stalled sweep, so the recorded rejections are the ones at the deadlock
    /// and not the ones on the way down.
    pub armed: bool,
    pub records: Vec<RejectionRecord>,
}

/// The two-scale gate, derived from the publication band and frozen before the
/// re-run: `25 * EPSILON_GRID_MM`.
///
/// Above it a residual is millimetre-scale separation that a strip relocation
/// can address. At or below it the layout is within 25 canonical bands of
/// legality and a strip teleport is measured nonsense. Neither review fitted
/// this to a cell: Grok review 10 Finding 2 derives it from the band, Sol
/// review 15 §A.2 states the same number, and Grok review 9 Round 1's
/// independent 0.05 mm stall threshold is the corroboration.
pub const JUMP_STRIP_THRESHOLD_MM: f64 = 25.0 * super::publish::EPSILON_GRID_MM;

impl Descent {
    pub fn new(config: DescentConfig, allow_rotation: Vec<bool>) -> Self {
        Self {
            ladder: config.ladder(),
            config,
            order: Vec::new(),
            allow_rotation,
            stalls: 0,
            jumps_spent: 0,
            proposals: 0,
            census: RejectionCensus::default(),
            census_activity: Vec::new(),
        }
    }

    pub fn jumps_spent(&self) -> u32 {
        self.jumps_spent
    }

    pub fn rejection_census(&self) -> &RejectionCensus {
        &self.census
    }

    pub fn stalls(&self) -> u32 {
        self.stalls
    }

    /// One complete piece proposal: gradient, SE(2)-normalized direction,
    /// backtracking ladder, strict-decrease acceptance.
    pub fn propose(
        &mut self,
        state: &mut IcsState,
        sources: &[PieceSource],
        contract: &Contract,
        piece: usize,
        work: &mut WorkVector,
    ) -> bool {
        self.proposals += 1;
        work.piece_proposals += 1;
        let before = incident_guided(state, piece);
        if before <= 0.0 {
            self.census.zero_energy += 1;
            return false;
        }
        let gradient = incident_gradient(state, piece);
        let radius = sources[piece].max_radius_mm.max(1e-9);
        let angular = if self.allow_rotation[piece] {
            gradient[2] / (radius * radius)
        } else {
            0.0
        };
        let norm = libm::hypot(libm::hypot(gradient[0], gradient[1]), radius * angular);
        if !(norm > 0.0) || !norm.is_finite() {
            self.census.zero_energy += 1;
            return false;
        }
        let direction = [
            gradient[0] / norm,
            gradient[1] / norm,
            angular / norm,
        ];
        let translation_share = libm::hypot(direction[0], direction[1]);
        let rotation_share = (radius * direction[2]).abs();
        let class = if rotation_share <= 0.0 {
            0
        } else if translation_share <= 0.0 {
            1
        } else {
            2
        };
        // The bounded sample arms on the first stalled sweep, so what it holds
        // is the deadlock and not the descent on the way down.
        let recording =
            self.census.armed && self.census.records.len() < self.config.rejection_census_samples;
        let entry = if recording {
            let totals = fold(state);
            incident_activity(state, piece, &mut self.census_activity);
            Some((totals, self.census_activity.clone()))
        } else {
            None
        };
        let mut rungs = Vec::new();
        let original = state.poses[piece];
        for rung in 0..self.ladder.len() {
            let step = self.ladder[rung];
            let candidate = Pose {
                tx_mm: original.tx_mm + step * direction[0],
                ty_mm: original.ty_mm + step * direction[1],
                // The direction's angular component is in radians per unit of
                // SE(2) step, because the metric `|dt|^2 + (R dtheta)^2` is a
                // statement about arc length. It is converted once, here, so
                // the stored coordinate stays the degrees the publication
                // transform reads.
                theta_deg: original.theta_deg + (step * direction[2]).to_degrees(),
                mirrored: original.mirrored,
            };
            if !candidate.tx_mm.is_finite()
                || !candidate.ty_mm.is_finite()
                || !candidate.theta_deg.is_finite()
            {
                continue;
            }
            state.poses[piece] = candidate;
            transform_piece(sources, &mut state.geometry, &state.poses, piece);
            work.pose_transforms += 1;
            rebuild_piece_rows(state, contract, piece, work);
            let after = incident_guided(state, piece);
            if let Some((totals, ref activity)) = entry {
                let totals_after = fold(state);
                incident_activity(state, piece, &mut self.census_activity);
                let newly_activated = activity
                    .iter()
                    .zip(self.census_activity.iter())
                    .filter(|(was, is)| !**was && **is)
                    .count();
                rungs.push(RejectionRung {
                    step_mm: step,
                    delta_incident_guided: after - before,
                    delta_raw: totals_after.raw - totals.raw,
                    delta_max_violation_mm: totals_after.max_violation_mm
                        - totals.max_violation_mm,
                    newly_activated_rows: newly_activated,
                });
            }
            if after < before {
                work.accepted_moves += 1;
                self.census.accepted += 1;
                self.census.accepted_by_class[class] += 1;
                return true;
            }
        }
        state.poses[piece] = original;
        transform_piece(sources, &mut state.geometry, &state.poses, piece);
        work.pose_transforms += 1;
        rebuild_piece_rows(state, contract, piece, work);
        self.census.rejected += 1;
        self.census.rejected_by_class[class] += 1;
        if let Some((totals, _)) = entry {
            let (active, penalty_max, penalty_sum) = incident_active_penalties(state, piece);
            self.census.records.push(RejectionRecord {
                proposal_ordinal: self.proposals,
                piece,
                direction_class: ["translation", "rotation", "combined"][class],
                translation_share,
                rotation_share,
                incident_guided_before: before,
                raw_before: totals.raw,
                guided_before: totals.guided,
                max_violation_before_mm: totals.max_violation_mm,
                active_incident_rows: active,
                active_incident_penalty_max: penalty_max,
                active_incident_penalty_sum: penalty_sum,
                rungs,
            });
        }
        false
    }

    /// One complete `n`-piece sweep, visiting pieces in descending incident
    /// guided energy with a stable tie by input index.
    pub fn sweep(
        &mut self,
        state: &mut IcsState,
        sources: &[PieceSource],
        contract: &Contract,
        work: &mut WorkVector,
    ) -> SweepOutcome {
        let raw_before = fold(state).raw;
        sweep_order(state, &mut self.order);
        let mut accepted = 0usize;
        let order = std::mem::take(&mut self.order);
        for piece in &order {
            if self.propose(state, sources, contract, *piece, work) {
                accepted += 1;
            }
        }
        self.order = order;
        let totals = fold(state);
        SweepOutcome {
            accepted,
            raw_before,
            raw_after: totals.raw,
            totals,
        }
    }

    /// The stall ladder: one guided weight update after one stalled sweep, one
    /// topology jump after `stalls_before_jump` guided updates that did not
    /// recover raw Φ.
    ///
    /// **The allowance is spent on an installed relocation, not on an
    /// evaluation.** `jumps_spent += 1` used to run *before* `jump()`, so a
    /// jump that evaluated 16 candidates and restored the original pose still
    /// consumed the trajectory's one-shot: every fatal cell in the previous
    /// round reported `jumpProposals: 16, jumps: 1` and had no jump left for
    /// the remaining 190,000 proposals (Grok review 10 Finding 2.3, Sol review
    /// 15 §A.2). A suppressed or uncommitted evaluation is now free.
    pub fn on_stalled_sweep(
        &mut self,
        state: &mut IcsState,
        sources: &[PieceSource],
        contract: &Contract,
        work: &mut WorkVector,
    ) -> JumpOutcome {
        self.census.armed = true;
        if super::energy::guided_update(state).is_some() {
            work.weight_updates += 1;
        }
        self.stalls += 1;
        if self.stalls < self.config.stalls_before_jump || self.jumps_spent >= self.config.jump_allowance
        {
            return JumpOutcome::default();
        }
        self.stalls = 0;
        let outcome = self.jump(state, sources, contract, work);
        if outcome.installed {
            self.jumps_spent += 1;
        }
        outcome
    }

    /// Records that a sweep improved raw Φ, which resets the stall ladder.
    pub fn on_improving_sweep(&mut self) {
        self.stalls = 0;
    }

    /// The topology jump, at the scale the residual is on.
    ///
    /// `jump_samples` deterministic low-discrepancy relocations of the
    /// highest-pressure piece; **one full `n`-piece sweep from each**; the best
    /// by guided Φ, with a stable ordinal tie-break; one total adoption. Three
    /// things here are the frozen fix list, not new design:
    ///
    /// 1. **a sweep is a sweep.** The previous implementation ran four
    ///    `propose(piece)` calls on the relocated piece alone, so no neighbour
    ///    ever settled and the candidate's guided Φ was measured on a state
    ///    nobody had accommodated. That is why `jumpsImprovingGuided` was 0 on
    ///    every near-minimum cell and 4/8 on the wild random throw (Grok review
    ///    10 Finding 2.2). `jump_local_proposals` is now the number of full
    ///    sweeps, and it is 1.
    /// 2. **the full state is snapshotted between candidates.** Once
    ///    neighbours move, saving one pose leaves candidate `k+1` starting from
    ///    candidate `k`'s wreckage (Grok review 10 Finding 2, `descent.rs:291`).
    /// 3. **two scales, both derived.** Above `25 * EPSILON_GRID_MM` = 0.100 mm
    ///    the residual is millimetre-scale separation and the candidates are
    ///    strip-wide relocations. At or below it the layout is a micrometre
    ///    from its band, a strip teleport is measured nonsense (S1 under the
    ///    old unconditional commit: 12.6 µm → 2.55 mm), and the same 16
    ///    candidates are drawn in a local SE(2) ball of translational radius
    ///    `max(4 * max_g, ladder_top)` with the metric-equivalent angular
    ///    radius `rho / R_piece`. The threshold is the publication band's own
    ///    25×; neither number is fitted to a cell.
    ///
    /// It never touches the protected exact incumbent.
    pub fn jump(
        &mut self,
        state: &mut IcsState,
        sources: &[PieceSource],
        contract: &Contract,
        work: &mut WorkVector,
    ) -> JumpOutcome {
        let piece = super::energy::highest_pressure_piece(state);
        let original = state.poses[piece];
        let entry = fold(state);
        let baseline = entry.guided;
        let max_violation_mm = entry.max_violation_mm;
        let kind = if max_violation_mm > JUMP_STRIP_THRESHOLD_MM {
            JumpKind::Strip
        } else {
            JumpKind::Ball
        };
        let radius = sources[piece].max_radius_mm.max(1e-9);
        // Frozen and derived. `ladder_top_mm` is the descent's own top rung, so
        // the floor of the ball is one full step of the move set it is trying
        // to unstick - not a number read off S1's 12.635 µm.
        let ball_radius_mm = (4.0 * max_violation_mm).max(self.config.ladder_top_mm);
        let snapshot = state.clone();
        let mut best: Option<(f64, IcsState)> = None;
        for ordinal in 0..self.config.jump_samples {
            work.jump_proposals += 1;
            if ordinal > 0 {
                state.clone_from(&snapshot);
            }
            let key = counter_hash(&[
                self.config.seed,
                self.jumps_spent as u64,
                self.stalls as u64,
                piece as u64,
                ordinal as u64,
            ]);
            let u = [
                rotated_halton(2, ordinal as u64 + 1, key),
                rotated_halton(3, ordinal as u64 + 1, key >> 21),
                rotated_halton(5, ordinal as u64 + 1, key >> 42),
            ];
            let candidate = match kind {
                JumpKind::Strip => {
                    let theta = if self.allow_rotation[piece] {
                        u[2] * 360.0
                    } else {
                        original.theta_deg
                    };
                    // The usable strip, for **this candidate's own rotation**.
                    // The circumradius box this replaced was wrong twice over:
                    // it charged one clearance to all four sides, and it
                    // bounded a piece by its circumradius, which for a 70x60
                    // triangle in a 60.24 mm strip made `low_y > high_y` and
                    // collapsed all 16 relocations onto one point (Grok review
                    // 10, "Same-class latent defects"). The spec asked for
                    // positions *in the strip*; that is an axis-aligned box
                    // question and the piece's own rotated AABB is the honest
                    // half-extent.
                    let extents =
                        centroid_relative_extents(&sources[piece], theta, original.mirrored);
                    let box_mm = strip_sample_box(contract, state.target_depth_mm, extents);
                    let centre_x = mix(box_mm[0], box_mm[2], u[0]);
                    let centre_y = mix(box_mm[1], box_mm[3], u[1]);
                    let (sin, cos) = super::state::pose_sin_cos(theta);
                    let source_centroid = sources[piece].centroid;
                    let mirror_x = if original.mirrored {
                        -source_centroid[0]
                    } else {
                        source_centroid[0]
                    };
                    let rotated = [
                        mirror_x * cos - source_centroid[1] * sin,
                        mirror_x * sin + source_centroid[1] * cos,
                    ];
                    Pose {
                        tx_mm: centre_x - rotated[0],
                        ty_mm: centre_y - rotated[1],
                        theta_deg: theta,
                        mirrored: original.mirrored,
                    }
                }
                JumpKind::Ball => {
                    // The SE(2) ball around the *current* pose. The angular
                    // radius is `rho / R` radians, which is the rotation whose
                    // arc length at the piece's own radius is `rho` - the same
                    // `|dt|^2 + (R dtheta)^2` metric the ladder steps in.
                    let angular = if self.allow_rotation[piece] {
                        mix(-ball_radius_mm / radius, ball_radius_mm / radius, u[2])
                    } else {
                        0.0
                    };
                    Pose {
                        tx_mm: original.tx_mm + mix(-ball_radius_mm, ball_radius_mm, u[0]),
                        ty_mm: original.ty_mm + mix(-ball_radius_mm, ball_radius_mm, u[1]),
                        theta_deg: original.theta_deg + angular.to_degrees(),
                        mirrored: original.mirrored,
                    }
                }
            };
            if !candidate.tx_mm.is_finite()
                || !candidate.ty_mm.is_finite()
                || !candidate.theta_deg.is_finite()
            {
                continue;
            }
            state.poses[piece] = candidate;
            transform_piece(sources, &mut state.geometry, &state.poses, piece);
            work.pose_transforms += 1;
            rebuild_piece_rows(state, contract, piece, work);
            for _ in 0..self.config.jump_local_proposals {
                self.sweep(state, sources, contract, work);
            }
            let guided = fold(state).guided;
            let replace = match &best {
                None => true,
                Some((current, _)) => guided < *current,
            };
            if replace {
                best = Some((guided, state.clone()));
            }
        }
        // The spec commits the best candidate, and that is not an oversight
        // being repaired here: a topology jump whose acceptance test is "the
        // guided total improved" is just another descent step and cannot change
        // a topology at all. Sol R2 §2 says it "commits for a full epoch even
        // if raw Φ temporarily worsens"; the 16 candidates are the choice set
        // and staying put is not one of them. What makes that safe at ball
        // scale is that the ball *contains* near-zero offsets - a stratified
        // draw inside `[-rho, rho]` has candidates at every scale below `rho` -
        // so the choice set is not "16 places far away" the way a strip
        // teleport's is.
        //
        // `improved_guided` is reported so the evidence can say how often the
        // jump was a step backwards, which is a fact about the field, not a
        // gate on it.
        let improved_guided = best
            .as_ref()
            .map(|(guided, _)| *guided < baseline)
            .unwrap_or(false);
        let install = self.config.jump_commits_unconditionally || improved_guided;
        let mut outcome = JumpOutcome {
            attempted: true,
            installed: false,
            improved_guided,
            kind,
            piece,
            radius_mm: match kind {
                JumpKind::Strip => f64::INFINITY,
                JumpKind::Ball => ball_radius_mm,
            },
            max_violation_mm,
            baseline_guided: baseline,
            best_guided: best.as_ref().map(|(guided, _)| *guided).unwrap_or(baseline),
        };
        match best {
            Some((_, adopted)) if install => {
                *state = adopted;
                outcome.installed = true;
            }
            _ => {
                state.clone_from(&snapshot);
            }
        }
        outcome
    }

    /// The piece a jump would move next. Diagnostic.
    pub fn pressure_piece(&self, state: &IcsState) -> usize {
        super::energy::highest_pressure_piece(state)
    }
}

/// One axis of a low-discrepancy draw inside `[low, high]`.
///
/// **The infeasible case is clamped per axis and only per axis.** When a piece
/// cannot fit the interval at all (`high <= low`) this returns that interval's
/// midpoint, which is the deterministic best-centred position on *that* axis
/// and leaves the other axis and the angle free to keep varying. What it must
/// never do is let one jammed axis collapse the whole sample set to a single
/// pose, which is what the circumradius box did on triangle-20.
#[inline]
fn mix(low: f64, high: f64, unit: f64) -> f64 {
    if high <= low {
        (low + high) / 2.0
    } else {
        low + unit * (high - low)
    }
}

/// Which rows incident on one piece carry a violation: its `n-1` pair rows in
/// the module's fixed pair order, then its four boundary rows `L, R, B, T`.
fn incident_activity(state: &IcsState, piece: usize, out: &mut Vec<bool>) {
    out.clear();
    let count = state.poses.len();
    for other in 0..count {
        if other == piece {
            continue;
        }
        let (first, second) = if other < piece {
            (other, piece)
        } else {
            (piece, other)
        };
        let row = &state.pair_rows[super::state::pair_index(count, first, second)];
        out.push(row.violation_mm > 0.0);
    }
    for row in &state.edge_rows[piece] {
        out.push(row.violation_mm > 0.0);
    }
}

/// The guided penalties of the rows incident on one piece **that are actually
/// active**: `(count, max, sum)`.
fn incident_active_penalties(state: &IcsState, piece: usize) -> (usize, u32, u64) {
    let count = state.poses.len();
    let mut active = 0usize;
    let mut max = 0u32;
    let mut sum = 0u64;
    let mut fold_row = |violation: f64, penalty: u32| {
        if violation > 0.0 {
            active += 1;
            max = max.max(penalty);
            sum += penalty as u64;
        }
    };
    for other in 0..count {
        if other == piece {
            continue;
        }
        let (first, second) = if other < piece {
            (other, piece)
        } else {
            (piece, other)
        };
        let row = &state.pair_rows[super::state::pair_index(count, first, second)];
        fold_row(row.violation_mm, row.penalty);
    }
    for row in &state.edge_rows[piece] {
        fold_row(row.violation_mm, row.penalty);
    }
    (active, max, sum)
}

/// The extent of a piece's transformed outer ring relative to its own
/// transformed centroid, `[min dx, min dy, max dx, max dy]`, at one rotation.
fn centroid_relative_extents(source: &PieceSource, theta_deg: f64, mirrored: bool) -> [f64; 4] {
    let (sin, cos) = super::state::pose_sin_cos(theta_deg);
    let centre = super::state::apply_pose(source.centroid, mirrored, sin, cos, 0.0, 0.0);
    let mut out = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for point in &source.decomposition.ring {
        let placed = super::state::apply_pose(*point, mirrored, sin, cos, 0.0, 0.0);
        out[0] = out[0].min(placed[0] - centre[0]);
        out[1] = out[1].min(placed[1] - centre[1]);
        out[2] = out[2].max(placed[0] - centre[0]);
        out[3] = out[3].max(placed[1] - centre[1]);
    }
    out
}

/// The box of **centroid** positions whose piece AABB lies inside the usable
/// strip, given that piece's centroid-relative extents.
///
/// The four sides carry the split of Sol review 15 §B.1 / Grok review 10 §B.1:
/// left, right and bottom are physical sheet edges at `edge + sag`; the top is
/// the tighter of the locked strip in the sag-less depth convention and the
/// physical sheet top.
fn strip_sample_box(contract: &Contract, target_depth_mm: f64, extents: [f64; 4]) -> [f64; 4] {
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

/// SplitMix64 over a fixed key vector: the counter-based source Sol review 14
/// §4 asks for, keyed by `(seed, jump, stall, piece, ordinal)` and never by a
/// clock, an address, or an iteration count that a different machine could
/// reach differently.
pub fn counter_hash(key: &[u64]) -> u64 {
    let mut state = 0x9E3779B97F4A7C15u64;
    for value in key {
        state = state.wrapping_add(*value).wrapping_add(0x9E3779B97F4A7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        state = z ^ (z >> 31);
    }
    state
}

/// The radical inverse of `index` in `base`, Cranley-Patterson-rotated by the
/// counter key. Low-discrepancy and reproducible; not a random number.
pub fn rotated_halton(base: u64, index: u64, key: u64) -> f64 {
    let mut result = 0.0f64;
    let mut fraction = 1.0f64 / base as f64;
    let mut current = index;
    while current > 0 {
        result += (current % base) as f64 * fraction;
        current /= base;
        fraction /= base as f64;
    }
    let shift = (key >> 11) as f64 / (1u64 << 53) as f64;
    let rotated = result + shift;
    rotated - rotated.floor()
}
