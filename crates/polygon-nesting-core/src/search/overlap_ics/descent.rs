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
    /// Bounded local proposals run from each relocation candidate.
    pub jump_local_proposals: u32,
    /// How many jumps the whole trajectory may spend. Round 1 is 1.
    pub jump_allowance: u32,
    /// Whether the jump commits its best candidate unconditionally.
    ///
    /// `true` is the spec's literal reading - Sol R2 §2's jump "chooses by
    /// guided Φ" among the 16 relocations and "commits for a full epoch even
    /// if raw Φ temporarily worsens", with staying put not in the choice set.
    /// `false` keeps the pre-jump pose when no relocation beat it, and is the
    /// **default**, on Gate 0's own measurement:
    ///
    /// | cell | `true` | `false` |
    /// |---|---|---|
    /// | S1 (0.5 mm / 2°) final `max_g` | 2.552630 mm | **0.012635 mm** |
    /// | S2 (2 mm / 10°) final raw Φ | 1308.79 | **362.36** |
    ///
    /// Two hundred times closer on the fatal cell. This is a **knob** in the
    /// spec's own words - "jump type/order/stall threshold are KNOBS, not
    /// architectural disagreements" - so choosing it on evidence is what Gate 0
    /// is for, and both settings stay reachable so the next round can re-run
    /// the comparison. See docs/experiments/overlap-ics/README.md §4.
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
            jump_local_proposals: 4,
            jump_allowance: 1,
            jump_commits_unconditionally: false,
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
}

/// What one sweep did.
#[derive(Clone, Copy, Debug)]
pub struct SweepOutcome {
    pub accepted: usize,
    pub raw_before: f64,
    pub raw_after: f64,
    pub totals: Totals,
}

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
        }
    }

    pub fn jumps_spent(&self) -> u32 {
        self.jumps_spent
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
            return false;
        }
        let direction = [
            gradient[0] / norm,
            gradient[1] / norm,
            angular / norm,
        ];
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
            if incident_guided(state, piece) < before {
                work.accepted_moves += 1;
                return true;
            }
        }
        state.poses[piece] = original;
        transform_piece(sources, &mut state.geometry, &state.poses, piece);
        work.pose_transforms += 1;
        rebuild_piece_rows(state, contract, piece, work);
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
    /// Returns `true` when a jump was committed.
    pub fn on_stalled_sweep(
        &mut self,
        state: &mut IcsState,
        sources: &[PieceSource],
        contract: &Contract,
        work: &mut WorkVector,
    ) -> bool {
        if super::energy::guided_update(state).is_some() {
            work.weight_updates += 1;
        }
        self.stalls += 1;
        if self.stalls < self.config.stalls_before_jump || self.jumps_spent >= self.config.jump_allowance
        {
            return false;
        }
        self.stalls = 0;
        self.jumps_spent += 1;
        self.jump(state, sources, contract, work)
    }

    /// Records that a sweep improved raw Φ, which resets the stall ladder.
    pub fn on_improving_sweep(&mut self) {
        self.stalls = 0;
    }

    /// The single topology jump: `jump_samples` deterministic low-discrepancy
    /// relocations of the highest-pressure piece, each followed by a bounded
    /// local sweep, chosen by guided Φ with a stable ordinal tie-break.
    ///
    /// It commits even when raw Φ temporarily worsens - that is the point of a
    /// topology change - and it never touches the protected exact incumbent.
    ///
    /// Returns whether the committed relocation *improved* guided Φ. The jump
    /// happens either way; this is a fact reported about it, not a gate on it.
    pub fn jump(
        &mut self,
        state: &mut IcsState,
        sources: &[PieceSource],
        contract: &Contract,
        work: &mut WorkVector,
    ) -> bool {
        let piece = super::energy::highest_pressure_piece(state);
        let original = state.poses[piece];
        let baseline = fold(state).guided;
        let mut best: Option<(f64, Pose)> = None;
        let edge = contract.edge_clearance_mm();
        let radius = sources[piece].max_radius_mm;
        let low_x = edge + radius;
        let high_x = contract.sheet_short_axis_mm - edge - radius;
        let low_y = edge + radius;
        let high_y = state.target_depth_mm - edge - radius;
        for ordinal in 0..self.config.jump_samples {
            work.jump_proposals += 1;
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
            let centre_x = mix(low_x, high_x, u[0]);
            let centre_y = mix(low_y, high_y, u[1]);
            let theta = if self.allow_rotation[piece] {
                u[2] * 360.0
            } else {
                original.theta_deg
            };
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
            let candidate = Pose {
                tx_mm: centre_x - rotated[0],
                ty_mm: centre_y - rotated[1],
                theta_deg: theta,
                mirrored: original.mirrored,
            };
            state.poses[piece] = candidate;
            transform_piece(sources, &mut state.geometry, &state.poses, piece);
            work.pose_transforms += 1;
            rebuild_piece_rows(state, contract, piece, work);
            for _ in 0..self.config.jump_local_proposals {
                if !self.propose(state, sources, contract, piece, work) {
                    break;
                }
            }
            let guided = fold(state).guided;
            let settled = state.poses[piece];
            let replace = match &best {
                None => true,
                Some((current, _)) => guided < *current,
            };
            if replace {
                best = Some((guided, settled));
            }
        }
        // The spec commits the best candidate, and that is not an oversight
        // being repaired here: a topology jump whose acceptance test is "the
        // guided total improved" is just another descent step and cannot change
        // a topology at all. Sol R2 §2 says it "commits for a full epoch even
        // if raw Φ temporarily worsens"; the 16 candidates are the choice set
        // and staying put is not one of them.
        //
        // `improved_guided` is reported so the evidence can say how often the
        // jump was a step backwards, which is a fact about the field.
        let improved_guided = best.map(|(guided, _)| guided < baseline).unwrap_or(false);
        let chosen = match best {
            Some((_, pose)) if self.config.jump_commits_unconditionally || improved_guided => pose,
            _ => original,
        };
        state.poses[piece] = chosen;
        transform_piece(sources, &mut state.geometry, &state.poses, piece);
        work.pose_transforms += 1;
        rebuild_piece_rows(state, contract, piece, work);
        improved_guided
    }

    /// The piece a jump would move next. Diagnostic.
    pub fn pressure_piece(&self, state: &IcsState) -> usize {
        super::energy::highest_pressure_piece(state)
    }
}

#[inline]
fn mix(low: f64, high: f64, unit: f64) -> f64 {
    if high <= low {
        (low + high) / 2.0
    } else {
        low + unit * (high - low)
    }
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
