//! The sweep: a Gauss-Seidel pass of [`super::relocate`] over the colliding
//! set, followed by one Algorithm-8 weight update over every row.
//!
//! **This file used to be the solver and is now the loop around one.** Under
//! docs/cutclose-relocate-spec.md the routine move is a global relocate, so the
//! three things this module was built out of are gone:
//!
//! * the **gradient proposal** - `incident_gradient`, the SE(2)-normalized
//!   direction and the halving backtracking ladder. A coordinate descent does
//!   not need a gradient and the member's acceptance rule is not a ladder's.
//! * the **`if after < before` acceptance test**. Grok review 12 Round 2 §6.3
//!   deletes it by name. It is the first half of the pre-named "neutered
//!   relocate" defect: with it in place the 50 container-wide samples would be
//!   evaluated and then thrown away, and the member would be PGS in a sampling
//!   costume.
//! * the **topology jump ladder** (`jump`, `JumpKind`, the strip/ball scale
//!   gate, `stalls_before_jump`). Sparrow has no such operator; disruption is a
//!   two-large-piece swap on a *failed separation* ([`super::disrupt`]), which
//!   is the explore loop's business and not a stalled sweep's.
//!
//! What survives here is what the member still needs: the counter-based
//! deterministic source, the request-derived configuration, and the sweep
//! itself. There is still **no clock** anywhere in this file; a wall-mode
//! driver may stop between whole sweeps, which is where the spec's arbitration
//! 2 puts a deadline read.

use super::diagnostics::WorkVector;
use super::energy::{fold, gls_update, Totals};
use super::relocate::{
    colliding_permutation, relocate, RelocateConfig, RelocateKey, RelocateOutcome, SampleOrigin,
};
use super::state::{Contract, IcsState, PieceSource};

/// The frozen knobs of the sweep.
#[derive(Clone, Copy, Debug)]
pub struct DescentConfig {
    /// The old backtracking ladder's top rung, `max(clearance/4, median
    /// diameter/128, 8 µm)`.
    ///
    /// **No move is bounded by it any more.** It is kept because it is the
    /// scale the neutered-relocate tripwire is written against: "a relocate
    /// must be able to commit a pose farther than `ladder_top` from the current
    /// pose" (Grok review 12 §6.3.1). A number that no longer caps anything but
    /// still measures the neighbourhood the old member could not leave is
    /// exactly the right reference for that vector.
    pub ladder_top_mm: f64,
    /// The relocate's own frozen parameters.
    pub relocate: RelocateConfig,
    /// The counter-based source's key. Never a clock, never an address.
    pub seed: u64,
    /// Vestigial: the four knobs the Gate-0 driver's option surface still
    /// writes (`--jumps`, `--stalls`, `--rejectioncensus`, `--jumpcommit`).
    ///
    /// Nothing reads them. The jump they configured does not exist, and the
    /// rejection census no longer records rungs because there are no rungs.
    /// They are held here so that the evidence agent - who owns the driver and
    /// the cell definitions - can retire the flags and the fields in one
    /// commit, rather than this one breaking a battery it is not allowed to
    /// edit. See the report accompanying this wave.
    pub jump_allowance: u32,
    pub stalls_before_jump: u32,
    pub rejection_census_samples: usize,
    pub jump_commits_unconditionally: bool,
}

impl DescentConfig {
    /// The request-derived configuration. `ladder_top_mm` keeps its old
    /// derivation because it is now a *measurement* reference and changing it
    /// would move the tripwire's bar.
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
            relocate: RelocateConfig::default(),
            seed,
            jump_allowance: 0,
            stalls_before_jump: 0,
            rejection_census_samples: 0,
            jump_commits_unconditionally: false,
        }
    }
}

/// Reusable scratch so a sweep allocates only its permutation.
pub struct Descent {
    pub config: DescentConfig,
    order: Vec<usize>,
    allow_rotation: Vec<bool>,
    /// Monotone proposal ordinal: the trajectory's own clock. One sweep still
    /// advances it by exactly one per piece, whether or not that piece was in
    /// the colliding set, because it is the currency `Engine::run`'s work quota
    /// is denominated in and a quota that stopped advancing at `Φ = 0` would
    /// not terminate.
    pub proposals: u64,
    /// The bite ordinal and worker ordinal of the sample stream. The schedule
    /// agent sets them; a locked-`T` cell leaves them at zero.
    bite: u64,
    worker: u64,
    /// Master iterations completed, part of the sample key.
    iteration: u64,
    census: RejectionCensus,
}

/// What one sweep did.
#[derive(Clone, Copy, Debug)]
pub struct SweepOutcome {
    /// Relocates that committed a pose different from the one they entered
    /// with.
    pub accepted: usize,
    /// Pieces that were in the colliding set and were actually relocated.
    pub relocated: usize,
    /// Relocates whose winner came from the container-wide half of the pool and
    /// moved the piece.
    pub container_commits: usize,
    /// The largest centroid displacement any relocate in this sweep committed.
    pub max_displacement_mm: f64,
    /// Rows the Algorithm-8 pass found active.
    pub active_rows: u64,
    pub raw_before: f64,
    pub raw_after: f64,
    pub totals: Totals,
}

/// The population census of the sweep's moves.
///
/// **The name and the first five fields are the old ladder census's**, because
/// `Engine::run` and the Gate-0 driver still read them and this wave may edit
/// neither. Three of them keep their exact meaning under the new member - a
/// piece was visited and moved, was visited and stayed, or was not in the
/// colliding set at all - and the two that do not are left at zero rather than
/// repurposed: there are no direction classes and no ladder rungs any more, so
/// `acceptedByDirectionClass` and `rejections` reading empty in the evidence is
/// the honest statement that the mechanism they described is gone. The
/// relocate's own economics are in [`super::diagnostics::WorkVector`] under the
/// names arbitration 4 of docs/cutclose-relocate-spec.md gave them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RejectionCensus {
    /// Relocates that committed a pose different from the one they entered
    /// with.
    pub accepted: u64,
    /// Relocates that ran the whole pool and committed the pose they started
    /// with, because nothing in it was better. **Not** a refusal: the current
    /// pose is a pool member and winning is what it did.
    pub rejected: u64,
    /// Piece visits skipped because the piece's incident raw Phi was zero, so
    /// it was not in the colliding set. Their `ct.get_loss(pk) > 0.0` filter.
    pub zero_energy: u64,
    /// Vestigial: the old `[translation, rotation, combined]` split of a
    /// gradient direction. Always zero.
    pub accepted_by_class: [u64; 3],
    pub rejected_by_class: [u64; 3],
    /// Vestigial: the bounded rung sample armed on the first stalled sweep.
    /// Always `false`; there are no rungs.
    pub armed: bool,
    /// Vestigial: always empty.
    pub records: Vec<RejectionRecord>,
    /// Committed relocates by winning origin, `[stayPut, focused, container]`.
    pub accepted_by_origin: [u64; 3],
    /// Uncommitted relocates by winning origin, same order.
    pub rejected_by_origin: [u64; 3],
    /// The largest centroid displacement any committed relocate produced. The
    /// neutered-relocate tripwire reads this against `ladder_top_mm`.
    pub max_displacement_mm: f64,
}

/// Vestigial: one rung of a rejected gradient proposal. Never constructed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RejectionRung {
    pub step_mm: f64,
    pub delta_incident_guided: f64,
    pub delta_raw: f64,
    pub delta_max_violation_mm: f64,
    pub newly_activated_rows: usize,
}

/// Vestigial: one rejected gradient proposal. Never constructed.
#[derive(Clone, Debug, PartialEq)]
pub struct RejectionRecord {
    pub proposal_ordinal: u64,
    pub piece: usize,
    pub direction_class: &'static str,
    pub translation_share: f64,
    pub rotation_share: f64,
    pub incident_guided_before: f64,
    pub raw_before: f64,
    pub guided_before: f64,
    pub max_violation_before_mm: f64,
    pub active_incident_rows: usize,
    pub active_incident_penalty_max: f64,
    pub active_incident_penalty_sum: f64,
    pub rungs: Vec<RejectionRung>,
}

impl Descent {
    pub fn new(config: DescentConfig, allow_rotation: Vec<bool>) -> Self {
        Self {
            config,
            order: Vec::new(),
            allow_rotation,
            proposals: 0,
            bite: 0,
            worker: 0,
            iteration: 0,
            census: RejectionCensus::default(),
        }
    }

    /// The sample stream's coordinates. The schedule agent calls this at a bite
    /// boundary and when it clones a master state into worker `ordinal`; the
    /// locked-`T` cells never touch it.
    pub fn set_stream(&mut self, bite: u64, worker: u64) {
        self.bite = bite;
        self.worker = worker;
    }

    pub fn stream_key(&self) -> RelocateKey {
        RelocateKey {
            seed: self.config.seed,
            bite: self.bite,
            iteration: self.iteration,
            worker: self.worker,
        }
    }

    pub fn rejection_census(&self) -> &RejectionCensus {
        &self.census
    }

    /// One complete relocate of one piece, with the sweep's own bookkeeping.
    ///
    /// This is the single-piece entry the corpus, the microbenchmarks and the
    /// unit vectors use. A piece with no incident raw Φ is not in the colliding
    /// set and returns immediately, which is the same early return the sweep
    /// takes.
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
        let outcome = relocate(
            state,
            sources,
            contract,
            &self.allow_rotation,
            piece,
            &self.config.relocate,
            self.stream_key(),
            work,
        );
        self.record(&outcome);
        outcome.moved
    }

    fn record(&mut self, outcome: &RelocateOutcome) {
        if !outcome.ran {
            self.census.zero_energy += 1;
            return;
        }
        let slot = match outcome.origin {
            SampleOrigin::StayPut => 0,
            SampleOrigin::Focused => 1,
            SampleOrigin::Container => 2,
        };
        if outcome.moved {
            self.census.accepted += 1;
            self.census.accepted_by_origin[slot] += 1;
            self.census.max_displacement_mm = self
                .census
                .max_displacement_mm
                .max(outcome.displacement_mm);
        } else {
            self.census.rejected += 1;
            self.census.rejected_by_origin[slot] += 1;
        }
    }

    /// **One master iteration: a Gauss-Seidel relocate sweep over the colliding
    /// set, then one Algorithm-8 weight update over every row.**
    ///
    /// The colliding set is collected once, at the top, and permuted from the
    /// counter stream - `optimizer/worker.rs::move_items`, rev `14f4868f`. Each
    /// member is then re-tested before it is relocated, because an earlier
    /// relocate in the same sweep may already have cleared it, and a piece that
    /// *became* colliding during the sweep is deliberately not added: it gets
    /// its turn in the next one.
    ///
    /// The weight update is here, and only here. That is what "all rows, every
    /// master iteration" means operationally, and it is why the stall path
    /// below is empty - a second call site would be the second dialect both
    /// consultants refused.
    pub fn sweep(
        &mut self,
        state: &mut IcsState,
        sources: &[PieceSource],
        contract: &Contract,
        work: &mut WorkVector,
    ) -> SweepOutcome {
        let count = state.poses.len();
        let entry_proposals = self.proposals;
        let raw_before = fold(state).raw;
        let key = self.stream_key();
        colliding_permutation(state, key, &mut self.order);
        let order = std::mem::take(&mut self.order);
        let mut accepted = 0usize;
        let mut relocated = 0usize;
        let mut container_commits = 0usize;
        let mut max_displacement_mm = 0.0f64;
        for piece in &order {
            self.proposals += 1;
            work.piece_proposals += 1;
            let outcome = relocate(
                state,
                sources,
                contract,
                &self.allow_rotation,
                *piece,
                &self.config.relocate,
                key,
                work,
            );
            self.record(&outcome);
            if outcome.ran {
                relocated += 1;
            }
            if outcome.moved {
                accepted += 1;
                max_displacement_mm = max_displacement_mm.max(outcome.displacement_mm);
                if outcome.origin == SampleOrigin::Container {
                    container_commits += 1;
                }
            }
        }
        self.order = order;
        // A sweep advances the work quota by one per piece whatever the
        // colliding set's size was, so a converged trajectory still reaches its
        // budget instead of spinning.
        self.proposals = entry_proposals + count as u64;
        let active_rows = gls_update(state);
        work.weight_updates += 1;
        self.iteration += 1;
        let totals = fold(state);
        SweepOutcome {
            accepted,
            relocated,
            container_commits,
            max_displacement_mm,
            active_rows,
            raw_before,
            raw_after: totals.raw,
            totals,
        }
    }

    /// The stall hook, now empty.
    ///
    /// Algorithm 8 fires inside [`Descent::sweep`] on every master iteration,
    /// so there is nothing left for a stall to trigger: no second weight
    /// dialect, and no jump. A stalled separation is the explore loop's signal
    /// to disrupt ([`super::disrupt`]), which is the schedule agent's call site
    /// and not this one's.
    pub fn on_stalled_sweep(
        &mut self,
        _state: &mut IcsState,
        _sources: &[PieceSource],
        _contract: &Contract,
        _work: &mut WorkVector,
    ) -> JumpOutcome {
        JumpOutcome::default()
    }

    /// Vestigial: the old stall ladder's reset. Nothing accumulates now.
    pub fn on_improving_sweep(&mut self) {}

    /// Vestigial: the old one-shot jump allowance, permanently unspent.
    pub fn jumps_spent(&self) -> u32 {
        0
    }

    /// Vestigial: the old consecutive-stall counter.
    pub fn stalls(&self) -> u32 {
        0
    }

    /// The piece carrying the most incident guided energy. Diagnostic.
    pub fn pressure_piece(&self, state: &IcsState) -> usize {
        super::energy::highest_pressure_piece(state)
    }
}

// ------------------------------------------------- the vestigial jump seam ---
//
// `Engine::run` still names these three types. The loop is the schedule
// agent's file and this wave is forbidden to edit it, so the seam is kept and
// emptied rather than removed: `attempted` is never true, so the whole branch
// behind it is dead and every `jump*` counter in the evidence document reads
// zero. The schedule agent deletes the seam together with the loop.

/// Vestigial. The two scales the old topology jump fired at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JumpKind {
    Strip,
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

/// Vestigial. Always `attempted: false` under `CutCloseRelocate`.
#[derive(Clone, Copy, Debug)]
pub struct JumpOutcome {
    pub attempted: bool,
    pub installed: bool,
    pub improved_guided: bool,
    pub kind: JumpKind,
    pub piece: usize,
    pub radius_mm: f64,
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

/// SplitMix64 over a fixed key vector: the counter-based source Sol review 14
/// §4 asks for, keyed by the trajectory's own coordinates and never by a clock,
/// an address, or an iteration count that a different machine could reach
/// differently. This is the whole random source of the member - there is no
/// `Xoshiro` and no `rand::` anywhere under `search/overlap_ics/`, which the
/// FAST tier's hygiene grep enforces.
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
