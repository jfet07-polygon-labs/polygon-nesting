//! The overlap-tolerant continuous engine: `CutCloseRelocate`.
//!
//! Specified by docs/cutclose-relocate-spec.md, whose body is Grok review 12
//! Round 2 §6 as amended by Sol review 17 Round 2 §4-§5. This module owns the
//! state, the publication seam, and **the two loops**:
//!
//! ```text
//! constructor (exact floor, wall charged, no internal cap)
//!   -> its OWN poses at W = D*                      (no affine shock)
//!   -> explore   W <- W (1 - 0.001), centre cut, close the far side
//!                separate: 8 Algorithm-10 workers -> barrier -> min weighted Phi
//!                          -> install winner -> ONE master Algorithm-8 pass
//!                publication attempt inside the 4 um band
//!                  dual-valid  -> INSTALL Publication.poses, D = published depth,
//!                                 rebuild every cache, next bite
//!                  refused     -> persist at W, pool the least-infeasible,
//!                                 Normal-biased draw, disrupt, separate again
//!   -> compress  restore the last dual-valid parent, TimeBased bite, uniform cut,
//!                discard a failed child
//!   -> best_exact
//! ```
//!
//! Five things this engine is *not*, each of which was a previous round's
//! failure or a pre-named self-deception:
//!
//! * it is not an `ExplorationKernel`. That seam consumes rotations baked into
//!   legacy surrogates and catalogues, which is the representation this
//!   experiment exists to escape.
//! * it is not contact-block. No exact predicate can shorten an intermediate
//!   move; exact geometry appears only at a publication attempt.
//! * it is not `global_legalize`. Repair is capped at 4 µm per row and 16 µm
//!   per piece, and a checkpoint that needs more is **discarded**, not inflated.
//! * it is not a proxy-quality engine. Φ, `max_g`, the guided energy and the
//!   target `W` are diagnostics; the only quality series is exact-valid raw
//!   source depth. **A Φ = 0 state whose publication is refused does not
//!   advance the width** - it is a failed separation.
//! * it does not bite from a proxy-legal parent. The repaired
//!   [`publish::Publication::poses`] become the continuous state atomically, and
//!   the next `D` is the *published* raw depth - the exact-parent-drift defect
//!   Sol review 17 Round 2 §2 named at this file's old `checkpoint()`.
//!
//! **There is no clock inside a bite, a sweep, a relocate or a coordinate
//! descent.** Wall mode reads the deadline at a worker-sweep barrier
//! (arbitration 2) and at a phase boundary, and nowhere else; fixed-work mode
//! constructs no `Instant` at all and runs the identical trajectory code.

pub mod broad_phase;
pub mod contact;
/// The deterministic contact corpus and its independent score: Gate 0's
/// numeric-soundness cell. Diagnostic only; no acceptance path calls it.
pub mod corpus;
pub mod decomposition;
pub mod descent;
pub mod diagnostics;
/// Algorithm 12's fail path: swap two large pieces and carry their interior
/// followers with the same rigid map. Fired by the explore loop on a failed
/// separation, never by a stalled sweep.
pub mod disrupt;
pub mod energy;
pub mod homotopy;
pub mod publish;
/// The routine move: Algorithm 5-6's global relocate of one colliding piece.
pub mod relocate;
pub mod state;

#[cfg(test)]
mod tests;

use crate::search::general_fast::{GeneralFastPiece, GeneralFastPlacement, GeneralFastSettings};

/// The seam between the engine and whatever produces its first complete
/// layout.
///
/// Sol review 14 §5 asks for `construct_short_side_first` "behind an
/// `InitialLayoutProvider` adapter", and this is that adapter: the engine names
/// the trait, the driver names the constructor. Nothing in this module imports
/// the constructor, so the ICS tree does not acquire a dependency on the
/// constructor's own portfolio, catalogues or settings surface.
pub trait InitialLayoutProvider {
    fn layout(
        &self,
        pieces: &[GeneralFastPiece<'_>],
        settings: GeneralFastSettings,
    ) -> Result<Vec<GeneralFastPlacement>, String>;
}

use descent::{Descent, DescentConfig, SweepOutcome};
use diagnostics::{ProxySample, QualityPoint, Trace, WorkVector};
use publish::{Publication, PublicationLimits};
use state::{
    build_geometry, pair_count, Contract, ExactIncumbent, Geometry, IcsState, PairRow, PieceSource,
    Pose,
};

/// The engine's configuration for one locked-strip run.
#[derive(Clone, Copy, Debug)]
pub struct IcsConfig {
    /// The locked strip depth. `homotopy.rs` will own the schedule that moves
    /// it; this round every cell pins it.
    pub target_depth_mm: f64,
    /// The work quota, in complete piece proposals. Fixed work, no clock.
    pub proposal_budget: u64,
    /// Sweeps between publication attempts, so a checkpoint cadence is a
    /// deterministic function of the trajectory rather than of the wall.
    pub checkpoint_every_sweeps: u64,
    pub descent: DescentConfig,
    pub limits: PublicationLimits,
}

/// One complete trajectory's result.
#[derive(Clone, Debug)]
pub struct IcsOutcome {
    /// The protected exact incumbent. Never `None`: the constructor floor is
    /// the first one.
    pub incumbent: ExactIncumbent,
    pub trace: Trace,
    /// The final continuous poses. Diagnostics only.
    pub final_poses: Vec<Pose>,
    pub final_raw_phi: f64,
    pub final_guided_phi: f64,
    pub final_max_violation_mm: f64,
    pub final_raw_depth_mm: f64,
    /// How many dual-valid publications the trajectory produced.
    pub publications: u64,
    /// The proposal ordinal of the first strict non-constructor child.
    pub first_strict_child_proposal: Option<u64>,
    /// Which rows are still active at the end, and how bad the worst is.
    pub final_census: energy::RowCensus,
    /// Why the proposals that were refused were refused.
    pub rejection_census: descent::RejectionCensus,
}

/// What one publication attempt produced.
#[derive(Clone, Debug)]
pub struct CheckpointOutcome {
    /// The dual-valid layout, present whenever both exact authorities accepted
    /// - **including** when it did not beat the incumbent by the 1 µm minimum.
    pub publication: Option<Publication>,
    /// The protected exact incumbent strictly improved.
    pub improved: bool,
}

impl CheckpointOutcome {
    fn none() -> Self {
        Self {
            publication: None,
            improved: false,
        }
    }
}

/// Everything one trajectory owns. Built once; the descent allocates nothing
/// per proposal after this.
pub struct Engine<'a> {
    pub pieces: &'a [GeneralFastPiece<'a>],
    pub sources: Vec<PieceSource>,
    pub settings: GeneralFastSettings,
    pub contract: Contract,
    pub state: IcsState,
    pub incumbent: ExactIncumbent,
    pub trace: Trace,
    pub config: IcsConfig,
    descent: Descent,
    /// The pose-bits digest of the last state offered for publication.
    last_attempt_pose_digest: Option<[u8; 32]>,
}

impl<'a> Engine<'a> {
    /// **The live start: the constructor's own poses, at `W = D*`.**
    ///
    /// No shock, no affine squeeze, no `T_0 = D* - 0.10 (D* - L)`. Grok review
    /// 12 Round 2 §6.5 - "Start at constructor legal raw depth `D*`
    /// (mixed-61: 182.976). No C175, no affine live-start" - and Round 1 §4.4
    /// lists "affine-compressing the constructor as the live start" among the
    /// forbidden rescues. The width the loop bites from is the constructor's own
    /// exact depth, so the *first* bite is the first thing that ever makes this
    /// layout infeasible.
    ///
    /// The constructor stays the anytime floor and its fingerprint is still
    /// never a child: [`ExactIncumbent::from_constructor`] is `true` here and
    /// only a publication clears it.
    ///
    /// `config.target_depth_mm` is **overridden** with `constructor_depth_mm`.
    /// A caller cannot hand the live loop a different entry width by accident,
    /// which is the one way an affine-free start could still become a shock.
    pub fn from_constructor_at_depth(
        pieces: &'a [GeneralFastPiece<'a>],
        settings: GeneralFastSettings,
        constructor: &[GeneralFastPlacement],
        constructor_depth_mm: f64,
        config: IcsConfig,
    ) -> Result<Self, String> {
        let contract = Contract::from_settings(settings);
        let sources = state::piece_sources(pieces)?;
        let poses = poses_of(pieces, &sources, constructor)?;
        let incumbent = ExactIncumbent {
            placement_fingerprint: publish::placement_fingerprint(constructor),
            placements: constructor.to_vec(),
            raw_source_depth_mm: constructor_depth_mm,
            from_constructor: true,
        };
        let mut config = config;
        config.target_depth_mm = constructor_depth_mm;
        Ok(Self::from_poses(
            pieces, settings, sources, contract, poses, incumbent, config,
        ))
    }

    /// Builds a trajectory from an **affinely shocked copy** of a constructor
    /// layout.
    ///
    /// **This is a diagnostic-cell factory and is not on the live path.** The
    /// throughput cell needs a state with a colliding set to measure a relocate
    /// in, and the retired C175 cell needed a state at `T_0`; a global squeeze
    /// produces one deterministically and cheaply. The live loop enters through
    /// [`Engine::from_constructor_at_depth`], and `homotopy::compressed`'s own
    /// doc records that split. Grok review 12 Round 2 §6.2 keeps the affine
    /// compression "as a corpus factory, not the live start".
    pub fn from_constructor(
        pieces: &'a [GeneralFastPiece<'a>],
        settings: GeneralFastSettings,
        constructor: &[GeneralFastPlacement],
        constructor_depth_mm: f64,
        config: IcsConfig,
    ) -> Result<Self, String> {
        let contract = Contract::from_settings(settings);
        let sources = state::piece_sources(pieces)?;
        let poses = poses_of(pieces, &sources, constructor)?;
        let incumbent = ExactIncumbent {
            placement_fingerprint: publish::placement_fingerprint(constructor),
            placements: constructor.to_vec(),
            raw_source_depth_mm: constructor_depth_mm,
            from_constructor: true,
        };
        let factor = homotopy::affine_compression_factor(
            &sources,
            &poses,
            &contract,
            config.target_depth_mm,
        );
        let compressed = homotopy::compressed(&sources, &poses, &contract, factor);
        Ok(Self::from_poses(
            pieces, settings, sources, contract, compressed, incumbent, config,
        ))
    }

    /// Builds a trajectory directly from a pose set: the S0/S1/S2 cells, which
    /// import poses rather than construct them.
    #[allow(clippy::too_many_arguments)]
    pub fn from_poses(
        pieces: &'a [GeneralFastPiece<'a>],
        settings: GeneralFastSettings,
        sources: Vec<PieceSource>,
        contract: Contract,
        poses: Vec<Pose>,
        incumbent: ExactIncumbent,
        config: IcsConfig,
    ) -> Self {
        let geometry = build_geometry(&sources, &poses);
        let count = poses.len();
        let mut state = IcsState {
            poses,
            geometry,
            pair_rows: vec![PairRow::default(); pair_count(count)],
            edge_rows: vec![[state::EdgeRow::default(); 4]; count],
            target_depth_mm: config.target_depth_mm,
        };
        let mut trace = Trace::default();
        energy::rebuild_all(&mut state, &contract, &mut trace.work);
        trace.work.pose_transforms += count as u64;
        let allow_rotation = pieces.iter().map(|piece| piece.allow_rotation).collect();
        let descent = Descent::new(config.descent, allow_rotation);
        Self {
            pieces,
            sources,
            settings,
            contract,
            state,
            incumbent,
            trace,
            config,
            descent,
            last_attempt_pose_digest: None,
        }
    }

    pub fn totals(&self) -> energy::Totals {
        energy::fold(&self.state)
    }

    pub fn raw_depth_mm(&self) -> f64 {
        state::raw_source_depth_mm(&self.state.geometry, &self.contract)
    }

    pub fn geometry(&self) -> &Geometry {
        &self.state.geometry
    }

    pub fn state(&self) -> &IcsState {
        &self.state
    }

    pub fn proposals(&self) -> u64 {
        self.descent.proposals
    }

    /// A cold reconstruction of every row. The throughput cell times this.
    pub fn cold_rebuild(&mut self) -> energy::Totals {
        energy::rebuild_all(&mut self.state, &self.contract, &mut self.trace.work);
        energy::fold(&self.state)
    }

    /// One moved piece's transform plus its `n-1` pair rows and four boundary
    /// rows. The other half of the throughput cell.
    pub fn rebuild_piece(&mut self, piece: usize) {
        state::transform_piece(&self.sources, &mut self.state.geometry, &self.state.poses, piece);
        self.trace.work.pose_transforms += 1;
        energy::rebuild_piece_rows(&mut self.state, &self.contract, piece, &mut self.trace.work);
    }

    /// Translates and turns one piece, without asking anything. Used by the
    /// microbenchmarks and the perturbation cells, never by the solver.
    pub fn displace(&mut self, piece: usize, dx_mm: f64, dy_mm: f64, dtheta_deg: f64) {
        self.state.poses[piece].tx_mm += dx_mm;
        self.state.poses[piece].ty_mm += dy_mm;
        self.state.poses[piece].theta_deg += dtheta_deg;
        self.rebuild_piece(piece);
    }

    /// One complete piece proposal, for the corpus and the microbenchmarks.
    pub fn propose_once(&mut self, piece: usize) -> bool {
        let Engine {
            ref mut state,
            ref sources,
            ref contract,
            ref mut trace,
            ref mut descent,
            ..
        } = *self;
        descent.propose(state, sources, contract, piece, &mut trace.work)
    }

    /// One publication attempt at the current state, recorded whatever it does.
    ///
    /// `true` iff the protected exact incumbent strictly improved. Unchanged
    /// meaning, unchanged trajectory: the locked-strip cells (S0, S1,
    /// triangle-20, the corpus) read exactly this.
    pub fn checkpoint(&mut self) -> bool {
        self.attempt_publication().improved
    }

    /// **One publication attempt, returning the dual-valid layout itself.**
    ///
    /// This is the seam the shrink loop needs and `checkpoint()` could not
    /// give it. `checkpoint()` answers "did `best_exact` move", which is a
    /// question about *quality*; a homotopy needs the repaired poses, which are
    /// a question about the *next legal parent*, and they exist even when the
    /// publication was not a record. Sol review 17 Round 2 §2 named the gap at
    /// the old `mod.rs:295`: `ExactIncumbent` was written from
    /// `publication.placements` while `state.poses` stayed at the pre-repair
    /// continuous layout, so a loop that bit from `state.poses` was biting from
    /// a layout no exact authority had ever accepted, and the repair giveback
    /// accumulated where nothing measured it.
    ///
    /// Installing is [`Engine::install_publication`]'s job, not this one's -
    /// the locked-strip cells must keep publishing without their continuous
    /// state moving underneath them.
    pub fn attempt_publication(&mut self) -> CheckpointOutcome {
        let totals = energy::fold(&self.state);
        // Do not re-attempt an **unchanged** state. The attempt gate compares
        // the *proxy* depth against the incumbent, and publication repair can
        // give back more than the 1 µm the gate asks for - so a converged state
        // whose repaired depth is worse than its proxy depth passed the gate on
        // every sweep, republished the identical layout, and never improved the
        // incumbent. One basin row spent 3,266 exact checkpoints that way.
        //
        // "Unchanged" is a statement about the **poses**, not about the depth.
        // Comparing depth alone skipped a genuinely different layout that
        // happened to be equally deep - and equal depth is not rare, it is what
        // a strip-bound layout converges to (Sol review 15 §D, `mod.rs:257`).
        // The digest is over every `x`, `y`, `theta` bit and the mirror flag,
        // so two states compare equal here only if they are the same state.
        let digest = pose_bits_digest(&self.state.poses);
        if self.last_attempt_pose_digest == Some(digest) {
            return CheckpointOutcome::none();
        }
        let Some(attempt) = publish::attempt(
            &self.state,
            &self.sources,
            self.pieces,
            self.settings,
            &self.contract,
            self.config.limits,
            totals.max_violation_mm,
            self.incumbent.raw_source_depth_mm,
            self.descent.proposals,
            &mut self.trace.work,
        ) else {
            return CheckpointOutcome::none();
        };
        self.last_attempt_pose_digest = Some(digest);
        self.trace.checkpoints.push(attempt.checkpoint);
        let Some(publication) = attempt.publication else {
            return CheckpointOutcome::none();
        };
        // The one write of `best_exact` in the whole engine, and it is
        // conditional on a strict improvement: a dual-valid layout that is not
        // better than the floor is recorded as a checkpoint and discarded as an
        // incumbent.
        //
        // **The publication is returned either way.** A shrink that only ever
        // adopted record-setting parents would refuse to bite from a layout the
        // exact authorities accepted at the new width just because the depth
        // gain was under 1 µm, and would then be biting from an unpublished
        // state instead - which is the drift this seam exists to prevent.
        let depth = publication.raw_source_depth_mm;
        let mut improved = false;
        if depth < self.incumbent.raw_source_depth_mm - self.config.limits.minimum_improvement_mm {
            self.incumbent = ExactIncumbent {
                placements: publication.placements.clone(),
                raw_source_depth_mm: depth,
                from_constructor: false,
                placement_fingerprint: publication.placement_fingerprint.clone(),
            };
            self.trace.quality.push(QualityPoint {
                proposal_ordinal: self.descent.proposals,
                raw_source_depth_mm: depth,
                strict_child: true,
            });
            improved = true;
        }
        CheckpointOutcome {
            publication: Some(publication),
            improved,
        }
    }

    /// **Installs a dual-valid publication as the continuous state.**
    ///
    /// The repaired poses replace the state's, every transform is recomputed
    /// and *every* row is rebuilt cold - not the moved pieces' rows, all of
    /// them, because the width changed too and the four boundary rows of every
    /// piece are measured against it. `width_mm` is the published raw depth,
    /// which is the `D` the next bite shrinks from: not the target `T` the
    /// separation was aiming at, and not the pre-repair proxy depth.
    ///
    /// After this the continuous state *is* a layout both exact authorities
    /// accepted, so the next bite is legal-to-infeasible rather than
    /// proxy-legal-to-infeasible, and nothing is given back invisibly.
    pub fn install_publication(&mut self, publication: &Publication) {
        self.install_poses(&publication.poses, publication.raw_source_depth_mm);
    }

    /// Replaces the poses and the width, then rebuilds every cache cold.
    fn install_poses(&mut self, poses: &[Pose], width_mm: f64) {
        self.state.poses.clear();
        self.state.poses.extend_from_slice(poses);
        self.state.target_depth_mm = width_mm;
        self.refresh_all();
    }

    /// Re-transforms every piece and rebuilds every row from the geometry.
    fn refresh_all(&mut self) {
        let Engine {
            ref mut state,
            ref sources,
            ref contract,
            ref mut trace,
            ..
        } = *self;
        for piece in 0..state.poses.len() {
            state::transform_piece(sources, &mut state.geometry, &state.poses, piece);
        }
        trace.work.pose_transforms += state.poses.len() as u64;
        energy::rebuild_all(state, contract, &mut trace.work);
    }

    /// **The locked-strip trajectory: sweeps to a work quota, at one `W`.**
    ///
    /// This is *not* the shrink loop - [`Engine::run_cutclose`] is. It is the
    /// zero-bite regime, and it stays because the regression floor is written
    /// in it: S0 is budget 0 and one checkpoint; S1 and triangle-20 are
    /// locked-`T` relocate regressions ("if relocate cannot republish a 0.5 mm
    /// perturbation of a known-legal layout, no shrink is licensed" - Grok
    /// review 12 Round 2 M16); the corpus and the throughput cell measure the
    /// member at a pinned width. None of those may move when the schedule does,
    /// which is exactly what makes them a floor.
    ///
    /// One worker, no tournament, no clock, no bite.
    pub fn run(&mut self) -> IcsOutcome {
        let count = self.state.poses.len().max(1) as u64;
        let mut sweeps_since_checkpoint = 0u64;
        let mut first_strict_child = None;
        self.sample_proxy();
        while self.descent.proposals + count <= self.config.proposal_budget {
            let outcome = {
                let Engine {
                    ref mut state,
                    ref sources,
                    ref contract,
                    ref mut trace,
                    ref mut descent,
                    ..
                } = *self;
                descent.sweep(state, sources, contract, &mut trace.work)
            };
            self.trace.sweeps += 1;
            // A converged state is not a stall: a sweep that changes nothing
            // has `raw_after == raw_before`, and at Φ = 0 there is no violated
            // minimum to escape and no utility to rank. The counter is a
            // diagnostic only - the escape mechanism it used to arm is gone
            // (disruption fires on a failed *separation*, in `run_cutclose`).
            if outcome.totals.raw > 0.0 && outcome.raw_after >= outcome.raw_before {
                self.trace.guided_stalls += 1;
            }
            sweeps_since_checkpoint += 1;
            if sweeps_since_checkpoint >= self.config.checkpoint_every_sweeps {
                sweeps_since_checkpoint = 0;
                if self.checkpoint() && first_strict_child.is_none() {
                    first_strict_child = Some(self.descent.proposals);
                }
            }
        }
        // One last attempt at the end of the quota, so a trajectory that
        // converged on its final sweep is not thrown away by the cadence.
        if self.checkpoint() && first_strict_child.is_none() {
            first_strict_child = Some(self.descent.proposals);
        }
        self.sample_proxy();
        let totals = energy::fold(&self.state);
        IcsOutcome {
            incumbent: self.incumbent.clone(),
            trace: self.trace.clone(),
            final_poses: self.state.poses.clone(),
            final_raw_phi: totals.raw,
            final_guided_phi: totals.guided,
            final_max_violation_mm: totals.max_violation_mm,
            final_raw_depth_mm: self.raw_depth_mm(),
            publications: self
                .trace
                .checkpoints
                .iter()
                .filter(|row| row.published_raw_depth_mm.is_some())
                .count() as u64,
            first_strict_child_proposal: first_strict_child,
            final_census: energy::census(&self.state),
            rejection_census: self.descent.rejection_census().clone(),
        }
    }

    fn sample_proxy(&mut self) {
        let totals = energy::fold(&self.state);
        self.trace.proxy_samples.push(ProxySample {
            proposal_ordinal: self.descent.proposals,
            target_depth_mm: self.state.target_depth_mm,
            raw_phi: totals.raw,
            guided_phi: totals.guided,
            max_violation_mm: totals.max_violation_mm,
            raw_source_depth_mm: self.raw_depth_mm(),
        });
    }

    /// The work vector so far. Exposed so a driver can report the six counters
    /// without owning the trace.
    pub fn work(&self) -> WorkVector {
        self.trace.work
    }

    /// The sweep engine.
    ///
    /// The schedule agent needs this at two boundaries this wave does not own:
    /// [`Descent::set_stream`] at a bite, so successive bites do not re-draw one
    /// bite's sample stream, and again when a master state is cloned into
    /// worker `ordinal`, so the eight Algorithm-10 workers sweep the same state
    /// in eight different orders. Everything else the loop needs is already a
    /// public field.
    pub fn descent_mut(&mut self) -> &mut Descent {
        &mut self.descent
    }

    pub fn descent(&self) -> &Descent {
        &self.descent
    }

    // ------------------------------------------- Algorithm 10: the tournament --

    /// **One master iteration: eight competitive workers, a barrier, a serial
    /// ordinal merge, and one Algorithm-8 pass.**
    ///
    /// Sol review 17 Round 2 §5, which Grok review 12 Round 2 M6 converged onto,
    /// in order:
    ///
    /// 1. clone the identical pose, row and weight state into workers `0..8`;
    /// 2. give each a counter-derived colliding-piece permutation and an
    ///    independent sample stream, keyed by
    ///    `(request seed, bite, iteration, worker ordinal)`;
    /// 3. run one complete sequential relocate sweep in each;
    /// 4. finish **equal work** - no early cancellation;
    /// 5. select the minimum total weighted Φ, stable by worker ordinal;
    /// 6. install only that state;
    /// 7. update all master weights **once**.
    ///
    /// **Completion order is never observable.** Every worker owns its own
    /// state, its own descent clone and its own work vector; nothing is shared
    /// but the immutable sources and contract, and the merge is a scan in
    /// ordinal order after the barrier. Two processes therefore agree bit for
    /// bit however the operating system happened to schedule the threads, and
    /// so do a threaded run and a `workers = 1` run of worker 0's own stream.
    ///
    /// The work of **all eight** workers is charged to the trace, in ordinal
    /// order. That is what the master iteration cost; charging only the winner
    /// would make `sampleEvaluations` a story about one eighth of the machine.
    ///
    /// One difference from Grok M6, stated rather than smoothed over: his tie
    /// key is `(weighted_loss, fingerprint, worker_ordinal)` and ours is
    /// `(weighted_loss, worker_ordinal)`. A digest between the two would only
    /// reorder exact ties by a hash, which is not more meaningful than the
    /// ordinal and costs a fingerprint per worker per iteration.
    fn tournament(&mut self, workers: usize, bite: u64) -> (SweepOutcome, usize) {
        let workers = workers.max(1);
        let mut slots: Vec<(IcsState, Descent, WorkVector)> = Vec::with_capacity(workers);
        for ordinal in 0..workers {
            let mut descent = self.descent.clone();
            descent.set_stream(bite, ordinal as u64);
            slots.push((self.state.clone(), descent, WorkVector::default()));
        }

        let sources: &[PieceSource] = &self.sources;
        let contract: &Contract = &self.contract;
        let mut outcomes: Vec<SweepOutcome> = Vec::with_capacity(workers);
        if workers == 1 {
            let (state, descent, work) = &mut slots[0];
            outcomes.push(descent.worker_sweep(state, sources, contract, work));
        } else {
            std::thread::scope(|scope| {
                let handles: Vec<_> = slots
                    .iter_mut()
                    .map(|slot| {
                        scope.spawn(move || {
                            let (state, descent, work) = slot;
                            descent.worker_sweep(state, sources, contract, work)
                        })
                    })
                    .collect();
                // The barrier. Joined in ordinal order, so the vector is a
                // function of the ordinals and not of who finished first.
                for handle in handles {
                    outcomes.push(handle.join().expect("a separator worker panicked"));
                }
            });
        }

        for (_, _, work) in &slots {
            self.trace.work.saturating_add(work);
        }

        let mut winner = 0usize;
        for ordinal in 1..workers {
            if outcomes[ordinal].totals.guided < outcomes[winner].totals.guided {
                winner = ordinal;
            }
        }
        let outcome = outcomes[winner];
        let (state, descent, _) = slots.swap_remove(winner);
        self.state = state;
        self.descent = descent;
        self.trace.sweeps += 1;

        // Step 7. One Algorithm-8 pass, on the master, over every row.
        let active_rows = energy::gls_update(&mut self.state);
        self.trace.work.weight_updates += 1;
        let totals = energy::fold(&self.state);
        (
            SweepOutcome {
                active_rows,
                raw_after: totals.raw,
                totals,
                ..outcome
            },
            winner,
        )
    }

    // ------------------------------------------ Algorithm 9: one separation --

    /// **Separate at the current width, until it publishes or gives up.**
    ///
    /// Master iterations of [`Engine::tournament`], the minimum-raw snapshot,
    /// the strike ladder with its 2 % improving reset, and the publication
    /// attempt. It never bites and it never touches `W`.
    ///
    /// Three rules that are not obvious and are all in the spec:
    ///
    /// * **the publication attempt is the feasibility test.** `raw Φ = 0` is a
    ///   proxy verdict and it is not re-legalization; a dual-valid publication
    ///   is. So the band test comes first in the iteration, before any sweep,
    ///   which also means an entry state that is already publishable is offered
    ///   immediately instead of after a sweep that would relocate nothing.
    /// * **a refused Φ = 0 ends the separation** ([`SeparateStop::Refused`]).
    /// * **the rollback keeps the weights.** `tracker.rs::restore_but_keep_weights`:
    ///   the landscape a separation learned is not undone by a rollback inside
    ///   the same width. Only a width change resets it.
    #[allow(clippy::too_many_arguments)]
    fn separate(
        &mut self,
        phase: Phase,
        limits: SeparateLimits,
        pacer: &Pacer,
        workers: usize,
        bite: u64,
        attempt: u64,
        record_fingerprints: bool,
        fingerprints: &mut Vec<IterationFingerprint>,
    ) -> SeparateOutcome {
        let band = self.config.limits.band_mm;
        let entry_raw = energy::fold(&self.state).raw;
        let mut snapshot = self.state.clone();
        let mut min_raw = f64::INFINITY;
        let mut strike_entry_raw = entry_raw;
        let mut since_improvement = 0u64;
        let mut strikes = 0u32;
        let mut iterations = 0u64;
        let mut band_reached = false;
        let mut exact_attempts = 0u64;
        // The clock, read at the previous barrier. Wall mode refreshes it once
        // per master iteration and never inside one.
        let mut elapsed_s = pacer.elapsed_s();
        let deadline_s = pacer.deadline_s(phase);
        let iteration_cap = pacer.iteration_cap();

        let stop = loop {
            let totals = energy::fold(&self.state);
            if totals.raw < min_raw {
                min_raw = totals.raw;
                snapshot.clone_from(&self.state);
                since_improvement = 0;
            } else {
                since_improvement += 1;
            }

            if totals.max_violation_mm <= band {
                band_reached = true;
                exact_attempts += 1;
                let outcome = self.attempt_publication();
                if let Some(publication) = outcome.publication {
                    return SeparateOutcome {
                        published: Some(publication),
                        stop: SeparateStop::Published,
                        iterations,
                        strikes,
                        min_raw,
                        band_reached,
                        exact_attempts,
                    };
                }
                if totals.raw <= 0.0 {
                    break SeparateStop::Refused;
                }
            }

            if since_improvement >= limits.iterations_without_improvement {
                restore_keeping_weights(&mut self.state, &snapshot);
                // The improving strike: a strike that still beat the previous
                // strike's entry by 2 % does not count against the cap.
                if min_raw < STRIKE_IMPROVEMENT_RATIO * strike_entry_raw {
                    strikes = 0;
                } else {
                    strikes += 1;
                }
                strike_entry_raw = min_raw;
                since_improvement = 0;
                if strikes >= limits.strikes {
                    break SeparateStop::Struck;
                }
            }

            if let (Some(elapsed), Some(deadline)) = (elapsed_s, deadline_s) {
                if elapsed >= deadline {
                    break SeparateStop::Deadline;
                }
            }
            if let Some(cap) = iteration_cap {
                if iterations >= cap {
                    break SeparateStop::WorkCap;
                }
            }

            let (_, winner) = self.tournament(workers, bite);
            iterations += 1;
            // **The barrier.** This is the one clock read of a master
            // iteration, and it is after the eight workers have joined.
            elapsed_s = pacer.elapsed_s();
            if record_fingerprints {
                fingerprints.push(IterationFingerprint {
                    bite,
                    attempt,
                    iteration: iterations,
                    winner,
                    state: state_fingerprint(&self.state),
                });
            }
        };

        // Whatever stopped it, the state the caller receives is the best raw Φ
        // this separation reached - the pool entry, and the layout a disruption
        // will perturb.
        if min_raw.is_finite() {
            restore_keeping_weights(&mut self.state, &snapshot);
        }
        SeparateOutcome {
            published: None,
            stop,
            iterations,
            strikes,
            min_raw,
            band_reached,
            exact_attempts,
        }
    }

    // ------------------------------------ Algorithms 11-13: the two phases --

    /// **`CutCloseRelocate`: explore, then compress.**
    ///
    /// The trajectory of docs/cutclose-relocate-spec.md, "The regime (frozen)",
    /// from a state that is already the constructor's own dual-valid layout at
    /// `W = D*` ([`Engine::from_constructor_at_depth`]).
    ///
    /// * **Explore.** `W <- W (1 - 0.001)`, centre cut, far side closed;
    ///   separate; a dual-valid publication installs its repaired poses, sets
    ///   `D` to the published raw depth and licenses the next bite. A failure
    ///   **persists at `W`** - the width is never grown and the parent is never
    ///   restored to skip a width - pools the least-infeasible layout, restores
    ///   a `Normal(0, 0.25)`-biased rank, disrupts, and separates again.
    /// * **Compress.** The last 20 % of the wall, always from the last
    ///   dual-valid parent, with a TimeBased bite and a uniform cut. A failed
    ///   child is discarded and the parent does not move.
    ///
    /// The clock is read at worker-sweep barriers and at phase boundaries. In
    /// [`Budget::FixedWork`] there is no clock at all and the same code runs
    /// against quotas.
    pub fn run_cutclose(&mut self, schedule: ScheduleConfig, budget: Budget) -> ScheduleOutcome {
        let seed = self.config.descent.seed;
        let workers = schedule.workers.max(1);
        let pacer = Pacer::new(budget, schedule.explore_time_ratio);
        let mut bites: Vec<BiteRecord> = Vec::new();
        let mut publications: Vec<PublishedBite> = Vec::new();
        let mut fingerprints: Vec<IterationFingerprint> = Vec::new();

        // The entry width is the exact incumbent's own depth: `D*`. A
        // trajectory entered from a pose fixture that carries no incumbent
        // starts at the state's own measured raw depth instead.
        let start_depth_mm = if self.incumbent.raw_source_depth_mm.is_finite() {
            self.incumbent.raw_source_depth_mm
        } else {
            self.raw_depth_mm()
        };
        let mut depth_mm = start_depth_mm;
        let mut width_mm = start_depth_mm;
        let mut parent_poses = self.state.poses.clone();
        let mut parent_fingerprint = self.incumbent.placement_fingerprint.clone();
        self.state.target_depth_mm = width_mm;
        self.refresh_all();
        energy::reset_weights(&mut self.state);
        self.sample_proxy();

        // ---------------------------------------------------------- explore --
        let mut bite_ordinal = 0u64;
        let mut explore_bites = 0u64;
        while !pacer.phase_done(Phase::Explore, explore_bites) {
            bite_ordinal += 1;
            let bite =
                homotopy::explore_bite(&self.sources, &mut self.state.poses, width_mm);
            width_mm = bite.width_after_mm;
            self.state.target_depth_mm = width_mm;
            self.refresh_all();
            // A new width is a new landscape: `change_strip_width` rebuilds
            // their tracker, so the weights start again at the floor.
            energy::reset_weights(&mut self.state);
            self.last_attempt_pose_digest = None;

            let mut record = BiteRecord {
                ordinal: bite_ordinal,
                phase: Phase::Explore,
                bite,
                attempts: 0,
                disruptions: 0,
                master_iterations: 0,
                strikes: 0,
                min_raw_phi: f64::INFINITY,
                proxy_band_reached: false,
                exact_attempts: 0,
                published: None,
            };
            let mut pool: Vec<PoolEntry> = Vec::new();
            let mut attempt = 0u64;
            let mut published = None;

            loop {
                let separation = self.separate(
                    Phase::Explore,
                    schedule.explore,
                    &pacer,
                    workers,
                    bite_ordinal,
                    attempt,
                    schedule.record_fingerprints,
                    &mut fingerprints,
                );
                record.master_iterations += separation.iterations;
                record.strikes += separation.strikes;
                record.exact_attempts += separation.exact_attempts;
                record.proxy_band_reached |= separation.band_reached;
                record.min_raw_phi = record.min_raw_phi.min(separation.min_raw);
                if let Some(publication) = separation.published {
                    published = Some(publication);
                    break;
                }
                // A failed separation. Persist at `W`; pool what we reached.
                push_pool(&mut pool, PoolEntry::of(&self.state, separation.min_raw));
                record.attempts += 1;
                // A phase-boundary clock read, between separations. It is not
                // redundant with the barrier read inside `separate`: a
                // separation that ends at `Refused` on its entry state returns
                // without ever reaching a barrier, and the explore phase must
                // still be able to end.
                if separation.stop == SeparateStop::Deadline
                    || pacer.phase_done(Phase::Explore, explore_bites)
                    || pacer.attempts_exhausted(record.attempts)
                {
                    break;
                }
                attempt += 1;
                let rank = homotopy::normal_biased_rank(pool.len(), seed, bite_ordinal, attempt);
                let entry = &pool[rank];
                self.install_poses(&entry.poses, width_mm);
                entry.restore_weights(&mut self.state);
                self.last_attempt_pose_digest = None;
                let Engine {
                    ref mut state,
                    ref sources,
                    ref contract,
                    ref mut trace,
                    ref descent,
                    ..
                } = *self;
                let disruption = disrupt::disrupt(
                    state,
                    sources,
                    contract,
                    descent.allow_rotation(),
                    seed,
                    bite_ordinal,
                    attempt,
                    &mut trace.work,
                );
                if disruption.fired {
                    record.disruptions += 1;
                }
            }

            match published {
                Some(publication) => {
                    let row = self.commit_publication(
                        &publication,
                        Phase::Explore,
                        &record,
                        attempt,
                        &parent_fingerprint,
                        pacer.elapsed_s(),
                    );
                    depth_mm = publication.raw_source_depth_mm;
                    width_mm = depth_mm;
                    parent_poses = publication.poses.clone();
                    parent_fingerprint = publication.placement_fingerprint.clone();
                    self.install_publication(&publication);
                    energy::reset_weights(&mut self.state);
                    publications.push(row.clone());
                    record.published = Some(row);
                    bites.push(record);
                    explore_bites += 1;
                }
                None => {
                    bites.push(record);
                    break;
                }
            }
        }
        let explore_seconds = pacer.elapsed_s();

        // --------------------------------------------------------- compress --
        // Always from the best exact parent, never from the infeasible state
        // exploration stopped in.
        let phase_started_s = explore_seconds.unwrap_or(0.0);
        let mut compress_bites = 0u64;
        while !pacer.phase_done(Phase::Compress, compress_bites) {
            bite_ordinal += 1;
            self.install_poses(&parent_poses, depth_mm);
            energy::reset_weights(&mut self.state);
            self.last_attempt_pose_digest = None;
            let step = pacer.compress_step(compress_bites, phase_started_s);
            let bite = homotopy::compress_bite(
                &self.sources,
                &mut self.state.poses,
                &self.contract,
                depth_mm,
                step,
                seed,
                bite_ordinal,
            );
            width_mm = bite.width_after_mm;
            self.state.target_depth_mm = width_mm;
            self.refresh_all();

            let separation = self.separate(
                Phase::Compress,
                schedule.compress,
                &pacer,
                workers,
                bite_ordinal,
                0,
                schedule.record_fingerprints,
                &mut fingerprints,
            );
            let mut record = BiteRecord {
                ordinal: bite_ordinal,
                phase: Phase::Compress,
                bite,
                attempts: u64::from(separation.published.is_none()),
                disruptions: 0,
                master_iterations: separation.iterations,
                strikes: separation.strikes,
                min_raw_phi: separation.min_raw,
                proxy_band_reached: separation.band_reached,
                exact_attempts: separation.exact_attempts,
                published: None,
            };
            compress_bites += 1;
            if let Some(publication) = separation.published {
                let row = self.commit_publication(
                    &publication,
                    Phase::Compress,
                    &record,
                    0,
                    &parent_fingerprint,
                    pacer.elapsed_s(),
                );
                depth_mm = publication.raw_source_depth_mm;
                width_mm = depth_mm;
                parent_poses = publication.poses.clone();
                parent_fingerprint = publication.placement_fingerprint.clone();
                self.install_publication(&publication);
                publications.push(row.clone());
                record.published = Some(row);
            }
            bites.push(record);
        }

        self.sample_proxy();
        let totals = energy::fold(&self.state);
        ScheduleOutcome {
            incumbent: self.incumbent.clone(),
            trace: self.trace.clone(),
            bites,
            publications,
            fingerprints,
            start_depth_mm,
            depth_mm,
            final_width_mm: width_mm,
            explore_bites,
            compress_bites,
            final_poses: self.state.poses.clone(),
            final_raw_phi: totals.raw,
            final_guided_phi: totals.guided,
            final_max_violation_mm: totals.max_violation_mm,
            final_raw_depth_mm: self.raw_depth_mm(),
            final_census: energy::census(&self.state),
            rejection_census: self.descent.rejection_census().clone(),
            search_seconds: pacer.elapsed_s(),
            explore_seconds,
        }
    }

    fn commit_publication(
        &self,
        publication: &Publication,
        phase: Phase,
        record: &BiteRecord,
        attempt: u64,
        parent_fingerprint: &str,
        wall_seconds: Option<f64>,
    ) -> PublishedBite {
        PublishedBite {
            ordinal: WorkOrdinal {
                bite: record.ordinal,
                attempt,
                iteration: record.master_iterations,
                proposals: self.descent.proposals,
            },
            phase,
            target_depth_mm: record.bite.width_after_mm,
            published_raw_depth_mm: publication.raw_source_depth_mm,
            repair_rows: publication.repair_rows,
            repair_max_displacement_mm: publication.repair_max_displacement_mm,
            repair_depth_giveback_mm: publication.repair_depth_giveback_mm,
            parent_fingerprint: parent_fingerprint.to_owned(),
            placement_fingerprint: publication.placement_fingerprint.clone(),
            improved_incumbent: self.incumbent.placement_fingerprint
                == publication.placement_fingerprint,
            wall_seconds,
        }
    }
}

/// Rolls a state back to a snapshot **keeping the weights it has now**.
///
/// `tracker.rs::restore_but_keep_weights`, rev `14f4868f`: the guided landscape
/// a separation learned is not undone by a rollback inside the same width. Only
/// a width change resets it (`energy::reset_weights`).
fn restore_keeping_weights(state: &mut IcsState, snapshot: &IcsState) {
    for (row, saved) in state.pair_rows.iter_mut().zip(&snapshot.pair_rows) {
        row.violation_mm = saved.violation_mm;
        row.contact = saved.contact;
    }
    for (rows, saved) in state.edge_rows.iter_mut().zip(&snapshot.edge_rows) {
        for (row, saved) in rows.iter_mut().zip(saved) {
            row.violation_mm = saved.violation_mm;
            row.witness = saved.witness;
        }
    }
    state.poses.copy_from_slice(&snapshot.poses);
    state.geometry.clone_from(&snapshot.geometry);
}

/// Inserts a failed separation into the loss-sorted pool, worst evicted first.
fn push_pool(pool: &mut Vec<PoolEntry>, entry: PoolEntry) {
    let at = pool
        .iter()
        .position(|held| entry.raw_phi < held.raw_phi)
        .unwrap_or(pool.len());
    pool.insert(at, entry);
    pool.truncate(POOL_CAPACITY);
}

/// A hex digest over the poses, the row violations **and** the row weights.
///
/// Not `pose_bits_digest`: two master iterations can install the same poses on
/// two different landscapes, and the merge-determinism vector has to be able to
/// tell those apart - the weights are half of what the next tournament ranks on.
pub fn state_fingerprint(state: &IcsState) -> String {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    for pose in &state.poses {
        digest.update(pose.tx_mm.to_bits().to_le_bytes());
        digest.update(pose.ty_mm.to_bits().to_le_bytes());
        digest.update(pose.theta_deg.to_bits().to_le_bytes());
        digest.update([u8::from(pose.mirrored)]);
    }
    for row in &state.pair_rows {
        digest.update(row.violation_mm.to_bits().to_le_bytes());
        digest.update(row.weight.to_bits().to_le_bytes());
    }
    for rows in &state.edge_rows {
        for row in rows {
            digest.update(row.violation_mm.to_bits().to_le_bytes());
            digest.update(row.weight.to_bits().to_le_bytes());
        }
    }
    digest.update(state.target_depth_mm.to_bits().to_le_bytes());
    format!("{:x}", digest.finalize())
}

// ============================================================ the schedule ===
//
// Everything below is the explore/compress regime of
// docs/cutclose-relocate-spec.md, "The regime (frozen)". It is the only caller
// of `homotopy::split_and_close`, `disrupt::disrupt` and
// `Engine::install_publication`, and it is the only place a wall clock is ever
// read.

/// Which half of Algorithm 11 a bite belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Explore,
    Compress,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Phase::Explore => "explore",
            Phase::Compress => "compress",
        }
    }
}

/// One separation's stall caps: `200 / 3` in explore, `100 / 5` in compress.
///
/// Grok review 12 Round 1 §1.7, read off `separator.rs`: "200 (explore) / 100
/// (compress) iterations without raw-loss improvement -> strike. 3 / 5 strikes
/// -> return best snapshot", with the improving-strike reset at 2 %.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeparateLimits {
    pub iterations_without_improvement: u64,
    pub strikes: u32,
}

impl SeparateLimits {
    pub const EXPLORE: Self = Self {
        iterations_without_improvement: 200,
        strikes: 3,
    };
    pub const COMPRESS: Self = Self {
        iterations_without_improvement: 100,
        strikes: 5,
    };
}

/// The improving-strike reset: a strike whose min raw Φ beat the previous
/// strike's entry by 2 % does not count. `separator.rs`: `min_loss < 0.98 *
/// initial_strike_loss`.
pub const STRIKE_IMPROVEMENT_RATIO: f64 = 0.98;

/// How many failed separations of one width are kept for the Normal-biased
/// draw.
///
/// Their pool is unbounded within a width. Ours is capped, and the cap is a
/// **memory guard rather than a schedule knob**: the entries are the best ones
/// by raw Φ, which are the ones the `Normal(0, 0.25)` bias draws from anyway,
/// so a run that never reaches 64 failed separations at one width - which is
/// every run the 10 s wall admits - is unaffected to the last bit.
pub const POOL_CAPACITY: usize = 64;

/// The frozen knobs of the regime. Every number is Sparrow's published default
/// and none of them may be fitted to a wall number.
#[derive(Clone, Copy, Debug)]
pub struct ScheduleConfig {
    /// Algorithm 10's competitive workers. Eight, from the start: Sol review 17
    /// Round 2's remaining refusal is the one-worker version, and the 150.165
    /// log this regime is a test of is `--workers 8`.
    pub workers: usize,
    pub explore: SeparateLimits,
    pub compress: SeparateLimits,
    /// The share of the post-constructor wall that belongs to exploration.
    pub explore_time_ratio: f64,
    /// Record a pose+weight fingerprint and a winning ordinal for **every**
    /// master iteration.
    ///
    /// Off by default: a 10 s wall run does tens of thousands of iterations and
    /// the eight-worker merge-determinism vector is the only thing that needs
    /// the sequence. Sol review 17 Round 2's mandatory addition 2 asks for
    /// identity of "each master snapshot, winning worker ordinal, pose and
    /// weight fingerprint after every master iteration"; this is that record.
    pub record_fingerprints: bool,
}

impl Default for ScheduleConfig {
    fn default() -> Self {
        Self {
            workers: 8,
            explore: SeparateLimits::EXPLORE,
            compress: SeparateLimits::COMPRESS,
            explore_time_ratio: homotopy::EXPLORE_TIME_RATIO,
            record_fingerprints: false,
        }
    }
}

/// **The two budgets, sharing one trajectory.**
///
/// The gate runs the wall arm; every FAST cell runs the fixed-work arm. They
/// are the same code with the same schedule: the only difference is what stops
/// a phase, and in the fixed-work arm nothing anywhere constructs an `Instant`.
#[derive(Clone, Copy, Debug)]
pub enum Budget {
    /// Fixed work, no clock. Two processes agree bit for bit.
    FixedWork {
        /// Successful explore bites the phase may take.
        explore_bites: u64,
        /// Compress bites the phase may take.
        compress_bites: u64,
        /// Failed separations allowed at one explore width before the phase
        /// gives up. Sparrow's `max_conseq_failed_attempts` is `None` on the
        /// fixture, so the wall is what stops them there; a fixed-work replay
        /// needs a number and this is it.
        attempts_per_bite: u64,
        /// Master iterations one separation may run.
        iterations_per_separation: u64,
    },
    /// Wall mode: what is left of the request's budget **after** the
    /// constructor, which is charged against the 10.000 s but never capped
    /// (arbitration 3 - a load-dependent start would break the determinism
    /// contract).
    Wall { remaining_seconds: f64 },
}

/// Where a publication sat in the fixed-work coordinate system, so a wall run
/// and a replay can be lined up.
///
/// "Wall publications record their fixed-work ordinal" - Grok review 12 Round 2
/// §6.8. Without it, a wall trajectory and its fixed-work twin can only be
/// compared on depth, and a schedule that published at a different point of the
/// same trajectory would look like a different trajectory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkOrdinal {
    pub bite: u64,
    pub attempt: u64,
    pub iteration: u64,
    pub proposals: u64,
}

/// One dual-valid publication, with everything the anytime curve needs.
#[derive(Clone, Debug)]
pub struct PublishedBite {
    pub ordinal: WorkOrdinal,
    pub phase: Phase,
    /// The width the separation was aiming at.
    pub target_depth_mm: f64,
    /// The exact-valid raw source depth. **The only quality series.**
    pub published_raw_depth_mm: f64,
    pub repair_rows: u64,
    pub repair_max_displacement_mm: f64,
    pub repair_depth_giveback_mm: f64,
    /// The fingerprint of the exact parent this bite started from, and the one
    /// it installed. Sol review 17 Round 2 §2's "exact parent after every bite".
    pub parent_fingerprint: String,
    pub placement_fingerprint: String,
    /// `true` when this publication also improved the protected incumbent.
    pub improved_incumbent: bool,
    /// Seconds since the loop entered, in wall mode; `None` in fixed work.
    /// A publication whose seconds exceed a checkpoint does not count for it.
    pub wall_seconds: Option<f64>,
}

/// One bite, successful or not: the funnel row
/// `bitesStarted -> proxyBandReached -> exactAttempted -> dualValidPublished`.
#[derive(Clone, Debug)]
pub struct BiteRecord {
    pub ordinal: u64,
    pub phase: Phase,
    pub bite: homotopy::Bite,
    /// Failed separations spent at this width.
    pub attempts: u64,
    pub disruptions: u64,
    pub master_iterations: u64,
    pub strikes: u32,
    /// The smallest raw Φ any separation of this bite reached.
    pub min_raw_phi: f64,
    /// `max_g` fell inside the 4 µm band at least once.
    pub proxy_band_reached: bool,
    pub exact_attempts: u64,
    pub published: Option<PublishedBite>,
}

/// The fingerprint of one master iteration: what the eight-worker merge
/// determinism vector compares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IterationFingerprint {
    pub bite: u64,
    pub attempt: u64,
    pub iteration: u64,
    /// The winning worker's ordinal.
    pub winner: usize,
    /// Poses, row violations and row weights of the installed master state,
    /// after its Algorithm-8 pass.
    pub state: String,
}

/// One `CutCloseRelocate` trajectory's result.
#[derive(Clone, Debug)]
pub struct ScheduleOutcome {
    pub incumbent: ExactIncumbent,
    pub trace: Trace,
    pub bites: Vec<BiteRecord>,
    pub publications: Vec<PublishedBite>,
    pub fingerprints: Vec<IterationFingerprint>,
    /// `D*`: the width the loop entered at.
    pub start_depth_mm: f64,
    /// `D`: the last exact-valid depth the loop is standing on.
    pub depth_mm: f64,
    /// `W`: the width the loop stopped at, which may be smaller than `D` when
    /// the last bite failed.
    pub final_width_mm: f64,
    pub explore_bites: u64,
    pub compress_bites: u64,
    pub final_poses: Vec<Pose>,
    pub final_raw_phi: f64,
    pub final_guided_phi: f64,
    pub final_max_violation_mm: f64,
    pub final_raw_depth_mm: f64,
    pub final_census: energy::RowCensus,
    pub rejection_census: descent::RejectionCensus,
    /// Wall spent inside the loop, in wall mode. The constructor's own wall is
    /// the caller's to report; both go in the evidence.
    pub search_seconds: Option<f64>,
    pub explore_seconds: Option<f64>,
}

/// Why a separation stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeparateStop {
    /// A dual-valid publication at this bite's width. The only success.
    Published,
    /// `raw Φ = 0` and the publication was refused.
    ///
    /// **A failed separation, deliberately.** Sol review 17 Round 2 §5: "If
    /// proxy Φ reaches zero but exact publication refuses, classify it as a
    /// failed separation: otherwise every piece is skipped forever and the loop
    /// spins at a false legal state." The colliding set is empty at Φ = 0, so
    /// every further sweep would relocate nothing, for ever.
    Refused,
    /// The strike cap.
    Struck,
    /// The phase deadline, read at a worker-sweep barrier.
    Deadline,
    /// The fixed-work iteration cap.
    WorkCap,
}

struct SeparateOutcome {
    published: Option<Publication>,
    stop: SeparateStop,
    iterations: u64,
    strikes: u32,
    min_raw: f64,
    band_reached: bool,
    exact_attempts: u64,
}

/// One entry of the least-infeasible pool.
///
/// Poses and weights, not a whole state: the rows are a pure function of the
/// poses and the width, so restoring is a cold rebuild and 61 poses plus 1,830
/// weights is two orders of magnitude less memory than a hundred cloned
/// geometries.
struct PoolEntry {
    raw_phi: f64,
    poses: Vec<Pose>,
    pair_weights: Vec<f64>,
    edge_weights: Vec<[f64; 4]>,
}

impl PoolEntry {
    fn of(state: &IcsState, raw_phi: f64) -> Self {
        Self {
            raw_phi,
            poses: state.poses.clone(),
            pair_weights: state.pair_rows.iter().map(|row| row.weight).collect(),
            edge_weights: state
                .edge_rows
                .iter()
                .map(|rows| [rows[0].weight, rows[1].weight, rows[2].weight, rows[3].weight])
                .collect(),
        }
    }

    /// Puts the weights back after the rows have been rebuilt from the poses.
    fn restore_weights(&self, state: &mut IcsState) {
        for (row, weight) in state.pair_rows.iter_mut().zip(&self.pair_weights) {
            row.weight = *weight;
        }
        for (rows, weights) in state.edge_rows.iter_mut().zip(&self.edge_weights) {
            for (row, weight) in rows.iter_mut().zip(weights) {
                row.weight = *weight;
            }
        }
    }
}

/// What stops a phase: a deadline it reads, or a quota it counts.
enum Pacer {
    FixedWork {
        explore_bites: u64,
        compress_bites: u64,
        attempts_per_bite: u64,
        iterations_per_separation: u64,
    },
    Wall {
        start: std::time::Instant,
        explore_deadline_s: f64,
        total_s: f64,
    },
}

impl Pacer {
    fn new(budget: Budget, explore_time_ratio: f64) -> Self {
        match budget {
            Budget::FixedWork {
                explore_bites,
                compress_bites,
                attempts_per_bite,
                iterations_per_separation,
            } => Pacer::FixedWork {
                explore_bites,
                compress_bites,
                attempts_per_bite,
                iterations_per_separation,
            },
            Budget::Wall { remaining_seconds } => {
                let total_s = remaining_seconds.max(0.0);
                Pacer::Wall {
                    // The one `Instant::now` of the whole trajectory, taken at
                    // a phase boundary before any solver work.
                    start: std::time::Instant::now(),
                    explore_deadline_s: total_s * explore_time_ratio.clamp(0.0, 1.0),
                    total_s,
                }
            }
        }
    }

    /// The clock, read **only** at a worker-sweep barrier or a phase boundary.
    fn elapsed_s(&self) -> Option<f64> {
        match self {
            Pacer::FixedWork { .. } => None,
            Pacer::Wall { start, .. } => Some(start.elapsed().as_secs_f64()),
        }
    }

    fn deadline_s(&self, phase: Phase) -> Option<f64> {
        match self {
            Pacer::FixedWork { .. } => None,
            Pacer::Wall {
                explore_deadline_s,
                total_s,
                ..
            } => Some(match phase {
                Phase::Explore => *explore_deadline_s,
                Phase::Compress => *total_s,
            }),
        }
    }

    /// `true` when the phase has no room for another bite. In fixed work the
    /// quota is the judge; in wall mode the clock is, read here between bites.
    fn phase_done(&self, phase: Phase, taken: u64) -> bool {
        match self {
            Pacer::FixedWork {
                explore_bites,
                compress_bites,
                ..
            } => {
                taken
                    >= match phase {
                        Phase::Explore => *explore_bites,
                        Phase::Compress => *compress_bites,
                    }
            }
            Pacer::Wall { .. } => {
                let (Some(elapsed), Some(deadline)) = (self.elapsed_s(), self.deadline_s(phase))
                else {
                    return true;
                };
                elapsed >= deadline
            }
        }
    }

    fn attempts_exhausted(&self, attempts: u64) -> bool {
        match self {
            Pacer::FixedWork {
                attempts_per_bite, ..
            } => attempts >= *attempts_per_bite,
            Pacer::Wall { .. } => false,
        }
    }

    fn iteration_cap(&self) -> Option<u64> {
        match self {
            Pacer::FixedWork {
                iterations_per_separation,
                ..
            } => Some(*iterations_per_separation),
            Pacer::Wall { .. } => None,
        }
    }

    /// The compression phase's TimeBased parameter.
    ///
    /// In wall mode it is seconds since the phase began over the phase's own
    /// length - a clock read between bites, which is where Grok review 12 Round
    /// 2 §6.5 puts it. In fixed work it is the bite ordinal over the bite quota:
    /// the same monotone `[0, 1]`, with the wall removed and nothing else
    /// changed, so the gates run the identical decay code.
    fn compress_step(&self, taken: u64, phase_started_s: f64) -> f64 {
        match self {
            Pacer::FixedWork { compress_bites, .. } => {
                homotopy::time_based_step(taken as f64, *compress_bites as f64)
            }
            Pacer::Wall { total_s, .. } => {
                let elapsed = self.elapsed_s().unwrap_or(0.0) - phase_started_s;
                homotopy::time_based_step(elapsed, (total_s - phase_started_s).max(0.0))
            }
        }
    }
}

/// A digest over every pose bit: `x`, `y`, `theta` and the mirror flag, in
/// piece order.
///
/// Not a fingerprint. `publish::placement_fingerprint` keys the angle at 1e-6
/// degrees and the translations on the 1 µm canonical grid, because it exists
/// to answer "is this the constructor's layout"; this exists to answer "has
/// anything at all moved", so it compares raw bits and nothing is rounded away.
pub fn pose_bits_digest(poses: &[Pose]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    for pose in poses {
        digest.update(pose.tx_mm.to_bits().to_le_bytes());
        digest.update(pose.ty_mm.to_bits().to_le_bytes());
        digest.update(pose.theta_deg.to_bits().to_le_bytes());
        digest.update([u8::from(pose.mirrored)]);
    }
    digest.finalize().into()
}

/// Maps a constructor's placements onto poses in the piece set's own order.
pub fn poses_of(
    pieces: &[GeneralFastPiece<'_>],
    sources: &[PieceSource],
    placements: &[GeneralFastPlacement],
) -> Result<Vec<Pose>, String> {
    if placements.len() != pieces.len() {
        return Err(format!(
            "the overlap-ICS state needs a pose for every piece: {} placements for {} pieces",
            placements.len(),
            pieces.len()
        ));
    }
    let mut poses = Vec::with_capacity(pieces.len());
    for source in sources {
        let placement = placements
            .iter()
            .find(|placement| placement.piece_id == source.id)
            .ok_or_else(|| format!("no placement for piece {}", source.id))?;
        poses.push(Pose {
            tx_mm: placement.translate_short_axis,
            ty_mm: placement.translate_long_axis,
            theta_deg: placement.rotation_deg,
            mirrored: placement.mirrored,
        });
    }
    Ok(poses)
}
