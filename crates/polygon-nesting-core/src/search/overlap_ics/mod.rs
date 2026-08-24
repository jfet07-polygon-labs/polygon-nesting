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
//!
//! The one qualification, added by the economics round's profile census and
//! stated here rather than left for a reader of [`profile`] to discover: under
//! the **`ics-profile`** cargo feature - off by default, off in the
//! `overlap-ics` set every gate is measured on - `Engine::tournament` and
//! `Engine::separate` read a clock around six named regions and add the
//! nanoseconds to a [`profile::PhaseProfile`] that no engine decision ever
//! reads. The audit's "an `Instant` appears in exactly one place" stays true of
//! the shipped binary, and the census asserts the two builds' whole fixed-work
//! documents against each other rather than asking to be believed.

pub mod broad_phase;
pub mod contact;
#[cfg(feature = "conflict-cluster-budget")]
pub mod cluster_budget;
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
/// The persisted calibrated-work plan's file format, and its writer.
///
/// Schema and write path only. The **reader** is [`icscal_read`], a separate
/// compilation unit, because the spec's "read/write separate" is a layout rule
/// and not a style note. See both module docs.
pub mod icscal;
/// The `icscal/v1` **reader**, and nothing else: no writer, no filesystem, no
/// measurement. Wave 3.
pub mod icscal_read;
/// The master-iteration phase census behind the `ics-profile` feature.
/// Measurement only; every field is zero in a default build.
pub mod profile;
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

use crate::search::overlap_ics_meter::currency::WorkTerms;
use crate::search::overlap_ics_meter::pacer::{NoClock, WorkPlanPacer};
use crate::search::overlap_ics_meter::strike_meter::{ShadowCounters, StrikeConfig, StrikeMeter};
use descent::{Descent, DescentConfig, SweepOutcome};
use diagnostics::{ProxySample, QualityPoint, Trace, WorkVector};
use icscal::PlanPhase;
use profile::{ics_time, PhaseProfile};
use publish::{Publication, PublicationLimits};
use state::{
    build_geometry, pair_count, Contract, ExactIncumbent, Geometry, IcsState, PairRow, PieceSource,
    Pose,
};
#[cfg(feature = "conflict-cluster-budget")]
use cluster_budget::{
    AtomicOrderTrace, ClusterField, PartitionArm, PartitionCostArmSample, PartitionTrace,
};

/// The engine's configuration for one locked-strip run.
#[derive(Clone, Copy, Debug)]
pub struct IcsConfig {
    /// The locked strip depth. `homotopy.rs` will own the schedule that moves
    /// it; this round every cell pins it.
    pub target_depth_mm: f64,
    /// The work quota, in complete piece proposals. Fixed work, no clock.
    pub proposal_budget: u64,
    /// **The same quota in the member's own currency**: one incremental
    /// incident-Φ evaluation of one candidate pose
    /// ([`diagnostics::WorkVector::sample_evaluations`]). `u64::MAX` - the
    /// default everywhere except where a caller names it - leaves
    /// [`Engine::run`] stopping on `proposal_budget` alone, bit for bit.
    ///
    /// It exists because the spec of record re-denominates the locked-strip
    /// regressions and refuses to let the old unit be renamed into the new one:
    /// Grok review 12 Round 1 §4.3 - "Work quota for S1: 200,000
    /// **relocate-evals** (not PGS proposals)" - and arbitration 4, "no silent
    /// renaming of the 100K pin". A `piece_proposal` is now a *slot*, most of
    /// which are empty once a layout is nearly feasible, so a budget counted in
    /// slots buys a different amount of the operator on every fixture. This
    /// counts what the operator actually spent.
    ///
    /// Read at the same place `proposal_budget` is - between whole sweeps, never
    /// inside one - so a trajectory that stops on it is still a fixed-work
    /// trajectory two processes reproduce bit for bit.
    pub relocate_eval_budget: u64,
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
    #[cfg(feature = "conflict-cluster-budget")]
    partition_field: Option<ClusterField>,
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
        #[cfg(feature = "conflict-cluster-budget")]
        let partition_field = if config.descent.partition_arm.is_off() {
            None
        } else {
            Some(ClusterField::from_sources(&sources))
        };
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
            #[cfg(feature = "conflict-cluster-budget")]
            partition_field,
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

    /// Paired-cost Gate-0 primitive: reset to one immutable C175 entry state
    /// before every complete worker sweep. `ComputeIgnore` pays for the source
    /// field inside the measured interval, builds and discards the B schedule,
    /// then consumes the same current Off order.
    #[cfg(feature = "conflict-cluster-budget")]
    pub fn partition_cost_arm(
        &self,
        arm: PartitionArm,
        warmup_sweeps: usize,
        measured_sweeps: usize,
    ) -> PartitionCostArmSample {
        use sha2::{Digest, Sha256};

        assert!(
            matches!(arm, PartitionArm::Off | PartitionArm::ComputeIgnore),
            "the cost cell compares only Off and ComputeIgnore"
        );
        let snapshot = self.state.clone();
        let piece_count = snapshot.poses.len();
        let entry_colliding_pieces = (0..piece_count)
            .filter(|piece| energy::incident_raw(&snapshot, *piece) > 0.0)
            .count();
        let mut state = snapshot.clone();
        let mut config = self.config.descent;
        config.partition_arm = arm;
        let mut descent = Descent::new(config, self.descent.allow_rotation().to_vec());

        let warm_field = (arm == PartitionArm::ComputeIgnore)
            .then(|| ClusterField::from_sources(&self.sources));
        let mut warm_trace = PartitionTrace::default();
        for _ in 0..warmup_sweeps {
            state.clone_from(&snapshot);
            let mut work = WorkVector::default();
            if arm == PartitionArm::ComputeIgnore {
                descent.worker_sweep_partitioned(
                    &mut state,
                    &self.sources,
                    &self.contract,
                    warm_field.as_ref().expect("compute-ignore has a warm field"),
                    &mut warm_trace,
                    &mut work,
                );
            } else {
                descent.worker_sweep(
                    &mut state,
                    &self.sources,
                    &self.contract,
                    &mut work,
                );
            }
        }

        let proposals_before = descent.proposals;
        let mut work = WorkVector::default();
        let mut partition = PartitionTrace::default();
        let mut order_trace = AtomicOrderTrace::default();
        let mut pose_sequence = Sha256::new();
        let started = std::time::Instant::now();
        // Runtime caches this immutable field once. Reconstructing it here,
        // after the clock starts, charges that one-time cost to treatment.
        let measured_field = (arm == PartitionArm::ComputeIgnore)
            .then(|| ClusterField::from_sources(&self.sources));
        for _ in 0..measured_sweeps {
            state.clone_from(&snapshot);
            let mut sweep_work = WorkVector::default();
            if arm == PartitionArm::ComputeIgnore {
                descent.worker_sweep_partitioned_cost_traced(
                    &mut state,
                    &self.sources,
                    &self.contract,
                    measured_field
                        .as_ref()
                        .expect("compute-ignore has a measured field"),
                    &mut partition,
                    &mut order_trace,
                    &mut sweep_work,
                );
            } else {
                descent.worker_sweep_cost_traced(
                    &mut state,
                    &self.sources,
                    &self.contract,
                    &mut order_trace,
                    &mut sweep_work,
                );
            }
            work.saturating_add(&sweep_work);
            pose_sequence.update(pose_bits_digest(&state.poses));
        }
        let elapsed_seconds = started.elapsed().as_secs_f64();
        let expected_atomic_slots = entry_colliding_pieces as u64 * measured_sweeps as u64;
        let completed_atomic_slots = order_trace.actual_slots;

        PartitionCostArmSample {
            arm,
            warmup_sweeps,
            measured_sweeps,
            piece_count,
            entry_colliding_pieces,
            expected_atomic_slots,
            completed_atomic_slots,
            legacy_proposals: descent.proposals - proposals_before,
            elapsed_seconds,
            slots_per_second: completed_atomic_slots as f64 / elapsed_seconds,
            pose_sequence_digest_sha256: format!("{:x}", pose_sequence.finalize()),
            consumed_order_digest_sha256: order_trace.digest_hex(),
            work,
            partition,
        }
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
        while self.descent.proposals + count <= self.config.proposal_budget
            && self.trace.work.sample_evaluations < self.config.relocate_eval_budget
        {
            #[cfg(not(feature = "conflict-cluster-budget"))]
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
            #[cfg(feature = "conflict-cluster-budget")]
            let outcome = {
                let Engine {
                    ref mut state,
                    ref sources,
                    ref contract,
                    ref mut trace,
                    ref mut descent,
                    ref partition_field,
                    ..
                } = *self;
                if descent.partition_arm() == PartitionArm::Off {
                    descent.sweep(state, sources, contract, &mut trace.work)
                } else {
                    descent.sweep_partitioned(
                        state,
                        sources,
                        contract,
                        partition_field
                            .as_ref()
                            .expect("an armed descent has a cluster field"),
                        &mut trace.partition,
                        &mut trace.work,
                    )
                }
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
    ///    `(request seed, bite, iteration, worker ordinal)`. All eight share the
    ///    master's `iteration` and differ only in the ordinal, which is what
    ///    makes them eight views of *one* state rather than eight trajectories.
    ///    `iteration` is [`Descent`]'s own counter and is **trajectory-global**:
    ///    it is never reset at a bite or an attempt, so a second separation of
    ///    the same width cannot re-draw the first one's 75 poses, pay for them
    ///    again and land in the same place;
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
    fn tournament(
        &mut self,
        workers: usize,
        bite: u64,
        profile: &mut PhaseProfile,
    ) -> (SweepOutcome, Merge) {
        let workers = workers.max(1);
        // **Step 1-2, and the first half of the spawn tax.** A persistent
        // executor keeps these slots alive across iterations and refills them
        // with `clone_from`; this loop allocates and clones eight of them every
        // master iteration. `prep_ns` is what that costs, measured rather than
        // asserted, and it is one of the two terms in the spec's 10 % clause.
        let mut slots: Vec<Slot> = ics_time!(profile, prep_ns, {
            let mut slots: Vec<Slot> = Vec::with_capacity(workers);
            for ordinal in 0..workers {
                let mut descent = self.descent.clone();
                descent.set_stream(bite, ordinal as u64);
                slots.push(Slot {
                    state: self.state.clone(),
                    descent,
                    work: WorkVector::default(),
                    #[cfg(feature = "conflict-cluster-budget")]
                    partition: PartitionTrace::default(),
                    sweep_ns: 0,
                });
            }
            slots
        });

        let sources: &[PieceSource] = &self.sources;
        let contract: &Contract = &self.contract;
        #[cfg(feature = "conflict-cluster-budget")]
        let partition_field = self.partition_field.as_ref();
        let mut outcomes: Vec<SweepOutcome> = Vec::with_capacity(workers);
        #[cfg(feature = "ics-profile")]
        let dispatch_started = std::time::Instant::now();
        if workers == 1 {
            let slot = &mut slots[0];
            #[cfg(not(feature = "conflict-cluster-budget"))]
            outcomes.push(slot.sweep(sources, contract));
            #[cfg(feature = "conflict-cluster-budget")]
            outcomes.push(slot.sweep(sources, contract, partition_field));
        } else {
            std::thread::scope(|scope| {
                #[cfg(not(feature = "conflict-cluster-budget"))]
                let handles: Vec<_> = slots
                    .iter_mut()
                    .map(|slot| scope.spawn(move || slot.sweep(sources, contract)))
                    .collect();
                #[cfg(feature = "conflict-cluster-budget")]
                let handles: Vec<_> = slots
                    .iter_mut()
                    .map(|slot| {
                        scope.spawn(move || slot.sweep(sources, contract, partition_field))
                    })
                    .collect();
                // The barrier. Joined in ordinal order, so the vector is a
                // function of the ordinals and not of who finished first.
                for handle in handles {
                    outcomes.push(handle.join().expect("a separator worker panicked"));
                }
            });
        }
        // **The second half of the spawn tax.** `std::thread::scope`'s whole
        // wall minus the *critical-path* sweep is what thread creation, the
        // scheduler placing eight threads on eight cores, and the join cost.
        // At `workers == 1` no thread is created and this is zero by
        // construction - which is what makes the 1/2/4/8 ladder a measurement
        // of the tax rather than of the box.
        #[cfg(feature = "ics-profile")]
        {
            let scope_ns = dispatch_started.elapsed().as_nanos() as u64;
            let critical = slots.iter().map(|slot| slot.sweep_ns).max().unwrap_or(0);
            let total: u64 = slots.iter().map(|slot| slot.sweep_ns).sum();
            profile.sweep_critical_ns += critical;
            profile.sweep_total_ns += total;
            profile.dispatch_ns += scope_ns.saturating_sub(critical);
        }

        for slot in &slots {
            self.trace.work.saturating_add(&slot.work);
            #[cfg(feature = "conflict-cluster-budget")]
            self.trace.partition.append(&slot.partition);
        }

        // Steps 5-7: the ordinal merge, the install, and one Algorithm-8 pass.
        // Timed as one region because a persistent executor changes none of it
        // and the census must not be able to flatter the executor by counting
        // merge work as dispatch work.
        ics_time!(profile, merge_gls_ns, {
            let mut winner = 0usize;
            for ordinal in 1..workers {
                if outcomes[ordinal].totals.guided < outcomes[winner].totals.guided {
                    winner = ordinal;
                }
            }
            let contested = outcomes
                .iter()
                .any(|other| other.totals.guided != outcomes[0].totals.guided);
            let outcome = outcomes[winner];
            let merge = Merge {
                winner,
                guided: outcome.totals.guided,
                contested,
            };
            let slot = slots.swap_remove(winner);
            self.state = slot.state;
            self.descent = slot.descent;
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
                merge,
            )
        })
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
    /// * **the 2 % governs both counters.** [`observe_raw`] is the inner one -
    ///   200 iterations without a 2 % improvement on the strike-best, not 200
    ///   without *any* improvement - and [`STRIKE_IMPROVEMENT_RATIO`] against
    ///   `strike_entry_raw` is the outer one. Round 1 shipped only the outer,
    ///   and that is the whole of this round's repair.
    /// * **the rollback keeps the weights.** `tracker.rs::restore_but_keep_weights`:
    ///   the landscape a separation learned is not undone by a rollback inside
    ///   the same width. Only a width change resets it.
    ///
    /// # The two arms, and the one line that differs between them
    ///
    /// Wave 3 of docs/economics-round-spec.md replaces the inline strike ladder
    /// with [`StrikeMeter`]. The ladder is unchanged - the same
    /// [`observe_raw`], the same `min_raw`/`strike_entry_raw` seeding, the same
    /// improving reset, the same cap - and the meter is a *transcription* of it
    /// that the meter's own property vectors check against two independent
    /// references. What the meter adds is that `patience_exhausted` is a
    /// **property of the configuration** rather than a literal: the control arm
    /// asks "200 batches without a 2 % improvement?" and the treatment arm asks
    /// "1_630_000 sample evaluations of None-batches?", and those are the only
    /// two sentences that differ between the arms. The classifier, the
    /// snapshot, the rollback, the ladder and the tournament are shared code.
    ///
    /// The control arm is therefore **bit-identical to the pre-Wave-3
    /// trajectory** and that is a cross-binary measurement, not a claim:
    /// `economics-round/integration/armgate.py` runs the round's base binary
    /// against this one on four fixed-work cells.
    ///
    /// # Where a calibrated plan is charged, and where it may stop
    ///
    /// Exactly at the barrier. `charge_batch` is called after the eight workers
    /// have joined, on the delta of the trajectory's own five counters since
    /// the previous charge - never on a cumulative reading, which is the
    /// spec's worst-ranked defect ("double-debit") - and the verdict it returns
    /// is consulted at the top of the *next* turn, beside the wall deadline.
    /// So "stop only between master batches" is where the code can ask, not a
    /// rule about when it chooses to.
    #[allow(clippy::too_many_arguments)]
    fn separate(
        &mut self,
        phase: Phase,
        strikes_config: StrikeConfig,
        pacer: &mut Pacer,
        workers: usize,
        bite: u64,
        attempt: u64,
        record_fingerprints: bool,
        fingerprints: &mut Vec<IterationFingerprint>,
    ) -> SeparateOutcome {
        let band = self.config.limits.band_mm;
        let entry_raw = energy::fold(&self.state).raw;
        let mut snapshot = self.state.clone();
        let mut meter = StrikeMeter::for_phase(strikes_config, phase, entry_raw);
        // The cost of the batch that produced the reading the next turn will
        // classify. The entry turn was produced by no batch and charges `0`.
        let mut batch_sample_evaluations = 0u64;
        let mut strike_accumulated = 0u64;
        let mut strike_overshoot = 0u64;
        let mut iterations = 0u64;
        let mut band_reached = false;
        // The calibrated plan's verdict, taken at the previous barrier. A
        // phase that was already spent when the separation opened stops before
        // its first batch, which is what `entry_boundary` is for.
        let mut plan_exhausted = pacer.phase_exhausted_at_entry(phase);
        // **The two numbers the audit's F4 says the funnel was missing.**
        //
        // `exact_band_entries` is the counter round 1 shipped under the name
        // `exact_attempts`: it is incremented when `max_g` falls inside the
        // 4 µm band, *before* `attempt_publication` is called, and 73 % of its
        // increments never reach exact geometry at all - the unchanged-pose
        // digest returns early (`Engine::attempt_publication`), and
        // `publish::attempt` returns `None` on `max_g > band`, `proxy > T` or
        // `proxy > incumbent - 1 µm`. `exact_checkpoint_calls` is the delta of
        // `work.exact_checkpoints`, which `publish::attempt` increments only
        // once it is past all three gates: **the count of times the exact
        // authorities were actually asked.** Neither is a rename of the other
        // and the two are emitted side by side, because the funnel needs both
        // rungs and had neither.
        let mut exact_band_entries = 0u64;
        let mut exact_checkpoint_calls = 0u64;
        let mut profile = PhaseProfile::default();
        // The clock, read at the previous barrier. Wall mode refreshes it once
        // per master iteration and never inside one.
        let mut elapsed_s = pacer.elapsed_s();
        let deadline_s = pacer.deadline_s(phase);
        let iteration_cap = pacer.iteration_cap();

        let stop = loop {
            // **Barrier to barrier**: the master iteration the spec's 10 %
            // clause is denominated in, opened here and closed at the bottom
            // of the turn. Under `ics-profile` only.
            #[cfg(feature = "ics-profile")]
            let turn_started = std::time::Instant::now();
            let totals = ics_time!(profile, band_fold_ns, energy::fold(&self.state));
            // The one transition, from the one place that owns it. A new
            // minimum inside the 2 % band moves the snapshot and leaves the
            // counter where it is; only a 2 % improvement forgives it. The
            // meter calls the frozen `observe_raw` and charges the batch that
            // produced this reading to whichever patience counter its arm
            // spends; both counters are maintained in both arms.
            if meter
                .observe(totals.raw, batch_sample_evaluations)
                .is_new_minimum()
            {
                ics_time!(profile, snapshot_ns, snapshot.clone_from(&self.state));
            }

            if totals.max_violation_mm <= band {
                band_reached = true;
                exact_band_entries += 1;
                profile.band_entries += 1;
                let called_before = self.trace.work.exact_checkpoints;
                let repaired_before = self.trace.work.repair_rows;
                let outcome = ics_time!(profile, exact_ns, self.attempt_publication());
                let called = self.trace.work.exact_checkpoints - called_before;
                exact_checkpoint_calls += called;
                profile.exact_calls += called;
                profile.repair_rows += self.trace.work.repair_rows - repaired_before;
                if let Some(publication) = outcome.publication {
                    #[cfg(feature = "ics-profile")]
                    {
                        profile.barrier_to_barrier_ns +=
                            turn_started.elapsed().as_nanos() as u64;
                    }
                    return SeparateOutcome {
                        published: Some(publication),
                        stop: SeparateStop::Published,
                        iterations,
                        strikes: meter.strikes(),
                        min_raw: meter.min_raw(),
                        band_reached,
                        exact_band_entries,
                        exact_checkpoint_calls,
                        profile,
                        strike_shadow: meter.shadow(),
                        strike_accumulated,
                        strike_overshoot,
                    };
                }
                if totals.raw <= 0.0 {
                    #[cfg(feature = "ics-profile")]
                    {
                        profile.barrier_to_barrier_ns +=
                            turn_started.elapsed().as_nanos() as u64;
                    }
                    break SeparateStop::Refused;
                }
            }

            // **The only match on the arm in the whole trajectory.** The
            // rollback, the ladder and the cap below it are shared.
            if meter.patience_exhausted() {
                ics_time!(
                    profile,
                    snapshot_ns,
                    restore_keeping_weights(&mut self.state, &snapshot)
                );
                // The improving strike: a strike that still beat the previous
                // strike's entry by 2 % does not count against the cap.
                let event = meter.strike();
                strike_accumulated = strike_accumulated.saturating_add(event.accumulated);
                strike_overshoot = strike_overshoot.saturating_add(event.crossing_batch);
                if event.struck_out {
                    #[cfg(feature = "ics-profile")]
                    {
                        profile.barrier_to_barrier_ns +=
                            turn_started.elapsed().as_nanos() as u64;
                    }
                    break SeparateStop::Struck;
                }
            }

            if let (Some(elapsed), Some(deadline)) = (elapsed_s, deadline_s) {
                if elapsed >= deadline {
                    #[cfg(feature = "ics-profile")]
                    {
                        profile.barrier_to_barrier_ns +=
                            turn_started.elapsed().as_nanos() as u64;
                    }
                    break SeparateStop::Deadline;
                }
            }
            // The calibrated plan's own deadline, read from the verdict the
            // previous barrier returned. Same stop, same place in the turn,
            // and no clock: `Budget::CalibratedWork` cannot construct one.
            if plan_exhausted {
                #[cfg(feature = "ics-profile")]
                {
                    profile.barrier_to_barrier_ns += turn_started.elapsed().as_nanos() as u64;
                }
                break SeparateStop::Deadline;
            }
            if let Some(cap) = iteration_cap {
                if iterations >= cap {
                    #[cfg(feature = "ics-profile")]
                    {
                        profile.barrier_to_barrier_ns +=
                            turn_started.elapsed().as_nanos() as u64;
                    }
                    break SeparateStop::WorkCap;
                }
            }

            let samples_before = self.trace.work.sample_evaluations;
            let (_, merge) = self.tournament(workers, bite, &mut profile);
            iterations += 1;
            // The currency's two per-batch terms, counted rather than timed,
            // so a build without `ics-profile` still carries them.
            profile.iterations += 1;
            batch_sample_evaluations = self.trace.work.sample_evaluations - samples_before;
            profile.sample_evaluations += batch_sample_evaluations;
            // **The barrier.** This is the one clock read of a master
            // iteration, and it is after the eight workers have joined.
            elapsed_s = pacer.elapsed_s();
            // **And the one charge.** The delta of the trajectory's own five
            // counters since the previous barrier - never a cumulative
            // reading. In wall and fixed-work mode this is a no-op that
            // returns `false`.
            plan_exhausted = pacer.charge_batch(phase, self.work_terms());
            if record_fingerprints {
                fingerprints.push(IterationFingerprint {
                    bite,
                    attempt,
                    iteration: iterations,
                    winner: merge.winner,
                    winner_guided: merge.guided,
                    contested: merge.contested,
                    state: state_fingerprint(&self.state),
                });
            }
            // Only a turn that reached the tournament is a *master iteration*;
            // a turn that broke out above published, struck or ran out of
            // budget without sweeping, and charging its wall to the iteration
            // denominator would deflate every phase share. Its wall is still
            // added to `barrier_to_barrier_ns`, so the shares stay honest
            // about the whole separation.
            #[cfg(feature = "ics-profile")]
            {
                profile.barrier_to_barrier_ns += turn_started.elapsed().as_nanos() as u64;
            }
        };

        // Whatever stopped it, the state the caller receives is the best raw Φ
        // this separation reached - the pool entry, and the layout a disruption
        // will perturb.
        if meter.min_raw().is_finite() {
            restore_keeping_weights(&mut self.state, &snapshot);
        }
        SeparateOutcome {
            published: None,
            stop,
            iterations,
            strikes: meter.strikes(),
            min_raw: meter.min_raw(),
            band_reached,
            exact_band_entries,
            exact_checkpoint_calls,
            profile,
            strike_shadow: meter.shadow(),
            strike_accumulated,
            strike_overshoot,
        }
    }

    /// **The trajectory's own cumulative five-term work vector.**
    ///
    /// Every term is a counter that already exists and is already summed by
    /// the engine, so a calibrated plan is charged out of the same numbers the
    /// evidence documents print. `master_batches` is `Trace::sweeps`, which
    /// [`Engine::tournament`] increments exactly once per master iteration.
    ///
    /// This is deliberately *cumulative*: the pacer takes the delta itself, in
    /// one place, which is what makes "batch two's aggregate equals the sum of
    /// its deltas" a property of the wiring rather than of the caller's care.
    fn work_terms(&self) -> WorkTerms {
        WorkTerms {
            sample_evaluations: self.trace.work.sample_evaluations,
            master_batches: self.trace.sweeps,
            actual_publication_attempt_calls: self.trace.work.exact_checkpoints,
            repair_rows: self.trace.work.repair_rows,
            disruption_moves: self.trace.work.disruption_moves,
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
        let strikes_config = schedule.strikes;
        let mut pacer = Pacer::new(budget, schedule.explore_time_ratio);
        // A calibrated plan charges deltas against the trajectory's own
        // counters, and this engine may already carry some: `run_cutclose` is
        // called twice on one engine by the spawn-tax cell. The cursor opens
        // where the trajectory is now, so the plan is never charged for work
        // it did not pace.
        pacer.open_at(self.work_terms());
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
                exact_band_entries: 0,
                exact_checkpoint_calls: 0,
                profile: PhaseProfile::default(),
                strike_shadow: ShadowCounters::default(),
                strike_accumulated: 0,
                strike_overshoot: 0,
                published: None,
            };
            let mut pool: Vec<PoolEntry> = Vec::new();
            let mut attempt = 0u64;
            let mut published = None;

            loop {
                let separation = self.separate(
                    Phase::Explore,
                    strikes_config,
                    &mut pacer,
                    workers,
                    bite_ordinal,
                    attempt,
                    schedule.record_fingerprints,
                    &mut fingerprints,
                );
                record.master_iterations += separation.iterations;
                record.strikes += separation.strikes;
                record.exact_band_entries += separation.exact_band_entries;
                record.exact_checkpoint_calls += separation.exact_checkpoint_calls;
                record.profile.add(&separation.profile);
                record.add_strike_accounting(&separation);
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
                let moves_before = trace.work.disruption_moves;
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
                // The currency's `D` term, charged to the bite that paid it.
                record.profile.disruption_moves += trace.work.disruption_moves - moves_before;
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
            // The calibrated plan's decay is `time_based_step` of the consumed
            // compress fraction, and it is read here, between bites, exactly
            // where the wall mode reads its clock.
            let bite = homotopy::compress_bite(
                &self.sources,
                &mut self.state.poses,
                &self.contract,
                depth_mm,
                step,
                seed,
                bite_ordinal,
            );
            self.state.target_depth_mm = bite.width_after_mm;
            self.refresh_all();

            let separation = self.separate(
                Phase::Compress,
                strikes_config,
                &mut pacer,
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
                exact_band_entries: separation.exact_band_entries,
                exact_checkpoint_calls: separation.exact_checkpoint_calls,
                profile: separation.profile,
                strike_shadow: separation.strike_shadow,
                strike_accumulated: separation.strike_accumulated,
                strike_overshoot: separation.strike_overshoot,
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
                parent_poses = publication.poses.clone();
                parent_fingerprint = publication.placement_fingerprint.clone();
                self.install_publication(&publication);
                publications.push(row.clone());
                record.published = Some(row);
            }
            bites.push(record);
        }

        // **A failed child is discarded**, including the one a phase deadline
        // interrupted. The continuous state the loop hands back is the last
        // dual-valid parent, so "legal-to-legal" is true of the trajectory's
        // ends as well as of its middle, and no reader can mistake an
        // interrupted child's proxy depth for progress. What that child reached
        // is not lost - every bite record carries its own `min_raw_phi`,
        // `proxy_band_reached` and its two exact counters, which is the funnel row the
        // failure license asks for, and `final_width_mm` below is the target it
        // was reaching for.
        self.install_poses(&parent_poses, depth_mm);
        self.sample_proxy();
        let totals = energy::fold(&self.state);
        let final_width_mm = bites
            .last()
            .map(|row| row.bite.width_after_mm)
            .unwrap_or(start_depth_mm);
        // The plan's closing ledger, including the tail: everything the
        // trajectory counted after the last barrier and therefore never
        // charged. `charged + uncharged_tail` is the trajectory's own work
        // vector, which is the double-debit identity written down.
        let calibrated = pacer.close(self.work_terms());
        ScheduleOutcome {
            incumbent: self.incumbent.clone(),
            trace: self.trace.clone(),
            bites,
            publications,
            fingerprints,
            start_depth_mm,
            depth_mm,
            final_width_mm,
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
            strike_arm: strikes_config,
            calibrated,
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
            poses: publication.poses.clone(),
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
///
/// The **same** 2 % governs the no-improvement counter *inside* one strike.
/// That is [`observe_raw`], and round 1 shipped without it.
pub const STRIKE_IMPROVEMENT_RATIO: f64 = 0.98;

/// What one raw-Φ reading did to the strike-best, and therefore to the
/// no-improvement counter.
///
/// Three classes, not two. The middle one is the whole point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RawObservation {
    /// A new minimum that beat the strike-best by **at least 2 %**. The
    /// snapshot moves and the no-improvement counter **resets**.
    Substantial,
    /// A new minimum **inside** the 2 % band. The snapshot moves - it is still
    /// the best layout this separation has seen - but the counter is
    /// **paused**: neither reset nor incremented. A trickle of 1e-15 minima
    /// therefore cannot hold one separation open until the deadline.
    Marginal,
    /// Not a new minimum. The counter **increments**.
    None,
}

impl RawObservation {
    /// True when the reading is a new minimum, i.e. when the caller must take
    /// the minimum-raw snapshot.
    pub fn is_new_minimum(self) -> bool {
        matches!(self, Self::Substantial | Self::Marginal)
    }
}

/// **The no-improvement transition, in one place.**
///
/// One raw-Φ reading against `min_raw` (this separation's best) and the running
/// `since_improvement` counter. Both are updated in place; the return value
/// tells the caller whether the snapshot must be taken.
///
/// This function is the whole of the rule. [`Engine::separate`] calls it and the
/// state-machine vector calls it, so there is exactly one copy of the predicate
/// in the tree and no test can pass by agreeing with a duplicate of it.
///
/// Grok review 12 Round 2 §6.5, the frozen sentence: *"explore 200 iterations
/// without 2 % raw-Φ improvement vs strike-best -> strike"*. Sparrow
/// `separator.rs:102-115` (rev `14f4868f`) is the source of that 2 %: a new best
/// that is not below `min_loss * 0.98` updates the incumbent and falls through
/// **without touching** `n_iter_no_improvement`; only a non-improvement
/// increments it.
///
/// Round 1 shipped `raw < min_raw => reset`, which is this function with
/// [`RawObservation::Marginal`] folded into [`RawObservation::Substantial`].
/// That is the one line both implementation reviews named
/// (docs/sol-review-18-the-strike-predicate.md §P0,
/// docs/grok-review-13-the-strike-predicate.md flag 3): at the Φ ≈ 1e-4 floor of
/// mixed-61's 22nd bite it forgave the counter on every microscopic minimum, so
/// no separation there ever struck out and Algorithm 12 - the disruption built
/// to cross exactly that shelf - never ran.
pub fn observe_raw(raw: f64, min_raw: &mut f64, since_improvement: &mut u64) -> RawObservation {
    if raw < *min_raw {
        // The 2 % is measured against the strike-best **before** it moves,
        // which is Sparrow's order: `loss < min_loss * 0.98` is evaluated while
        // `min_loss` still holds the incumbent.
        let substantial = raw < STRIKE_IMPROVEMENT_RATIO * *min_raw;
        *min_raw = raw;
        if substantial {
            *since_improvement = 0;
            RawObservation::Substantial
        } else {
            RawObservation::Marginal
        }
    } else {
        *since_improvement += 1;
        RawObservation::None
    }
}

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
    /// **Which strike arm this trajectory runs.**
    ///
    /// docs/economics-round-spec.md funded change 1: the control arm is the
    /// frozen literals `200 / 3 / 100 / 5 / 0.98`, the treatment arm is the
    /// work-denominated impatient policy at the KNOB quanta `1_630_000` /
    /// `815_000`, and *strike semantics are the only delta between the arms*.
    /// This field is that delta, and the only one: the executor, the pacer,
    /// the classifier, the ladder and the tournament are shared code.
    ///
    /// The default is [`StrikeConfig::CONTROL`], so a caller that says nothing
    /// gets the member exactly as it was closed. The two phases' limits used
    /// to be two fields here; they are inside the control variant now, still
    /// read off [`SeparateLimits::EXPLORE`] and [`SeparateLimits::COMPRESS`],
    /// because two places to write `200` is one place for them to drift.
    pub strikes: StrikeConfig,
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
            strikes: StrikeConfig::CONTROL,
            explore_time_ratio: homotopy::EXPLORE_TIME_RATIO,
            record_fingerprints: false,
        }
    }
}

/// **The three budgets, sharing one trajectory.**
///
/// The reported curves run the wall arm; every FAST cell runs the fixed-work
/// arm; the economics round's third funded change adds the calibrated-work
/// arm. They are the same code with the same schedule: the only difference is
/// what stops a phase, and in **two** of the three arms nothing anywhere
/// constructs an `Instant`.
///
/// It is no longer `Copy`: [`Budget::CalibratedWork`] carries a plan, and a
/// plan carries the sha256 and the feature list of the binary it was measured
/// on. A caller that wants to print the budget after spending it prints it
/// first - which is the right order anyway, because a spent plan's consumption
/// is in [`ScheduleOutcome::calibrated`] and not here.
#[derive(Debug)]
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
    /// **The 10-second calibrated work plan.** No clock: two processes agree
    /// bit for bit, exactly as [`Budget::FixedWork`] does, while the quantity
    /// being counted down is a wall budget converted at a *previously
    /// measured* rate.
    ///
    /// docs/economics-round-spec.md, funded change 3. The pacer is handed in
    /// already built, because [`WorkPlanPacer::from_plan`] can refuse - a plan
    /// for a different currency, a plan missing a phase, a budget that is not
    /// a positive number of seconds - and a refusal belongs at the caller's
    /// boundary where it can be reported, never inside a trajectory as a
    /// panic. **The engine therefore cannot acquire a plan**: it cannot read a
    /// file, it cannot measure a rate, and it cannot construct this variant
    /// out of anything it holds. That is "read/write separate; no live probe
    /// on a gated trajectory" as a type rather than as a rule.
    ///
    /// The wording the spec fixes: *"10-second calibrated work plan" - quality
    /// deterministic, wall a distribution (no governor exists)*.
    CalibratedWork {
        plan: Box<WorkPlanPacer<NoClock>>,
        /// **A termination guard, not a schedule knob.** `0` is unlimited,
        /// which is what [`Budget::Wall`] does, and is the setting every gate
        /// cell uses. A calibrated phase ends when its units are spent, and
        /// every master batch spends some; but a separation that returns
        /// without reaching a barrier - [`SeparateStop::Refused`] on its entry
        /// state - spends nothing, and wall mode is protected from an
        /// unbounded retry loop there only by a clock that this arm does not
        /// have. Naming the guard is cheaper than proving the loop cannot
        /// happen.
        attempts_per_bite: u64,
    },
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
    /// **The repaired layout itself.** The evidence audit's revalidation
    /// chapter closes on this sentence: "No pose is recorded for any of the
    /// 1,701 publications. `161.05499`, `163.56062`, `167.31508` and
    /// `167.95169` are re-validatable only by the process that produced them."
    /// They are recorded now.
    ///
    /// These are the **post-repair** poses - `Publication::poses`, the same
    /// array `install_publication` installs - so
    /// `placements_of(sources, &poses)` reproduces `Publication::placements`
    /// exactly (`publish::attempt` re-derives the placements from the poses
    /// after every repair row), and therefore reproduces both
    /// `placement_fingerprint` and, through `raw_depth_of`,
    /// `published_raw_depth_mm`. A reader with the request and this array can
    /// re-run both exact authorities without the process that produced it.
    pub poses: Vec<Pose>,
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
    /// **Entries into the 4 µm band**, not calls to the exact authorities.
    /// This is the counter round 1 emitted as `exactAttempts`; the value is
    /// unchanged and the name now says what it counts (audit F4).
    pub exact_band_entries: u64,
    /// **Calls that reached exact geometry.** Measured as the delta of
    /// `work.exact_checkpoints`, so `sum(exact_checkpoint_calls)` over the
    /// bites reconciles exactly with the work vector's own total, and
    /// `exact_checkpoint_calls <= exact_band_entries` always.
    pub exact_checkpoint_calls: u64,
    /// Where this bite's master iterations went. All zeros without
    /// `ics-profile`; never read by the engine.
    pub profile: PhaseProfile,
    /// **Both arms' patience counters, for the paired comparison.**
    ///
    /// The control arm carries the treatment's work counter and the treatment
    /// carries the control's batch counter, because the spec's promotion
    /// clause is a paired comparison of two runs that differ in one sentence,
    /// and a comparison whose two documents do not carry the same terms is not
    /// paired. Shadow only: nothing here is read by a decision.
    pub strike_shadow: ShadowCounters,
    /// The patience that had accumulated at each strike of this bite, summed:
    /// batches in the control arm, sample evaluations in the treatment. The
    /// evidence document's `strikeCost`.
    pub strike_accumulated: u64,
    /// The cost of the batch that crossed the threshold, summed over this
    /// bite's strikes. `strike_accumulated - strike_overshoot` is strictly
    /// below `strikes * quantum`, which **is** the spec's overshoot clause.
    pub strike_overshoot: u64,
    pub published: Option<PublishedBite>,
}

impl BiteRecord {
    fn add_strike_accounting(&mut self, separation: &SeparateOutcome) {
        let shadow = separation.strike_shadow;
        self.strike_shadow.batches += shadow.batches;
        self.strike_shadow.charged_work =
            self.strike_shadow.charged_work.saturating_add(shadow.charged_work);
        self.strike_shadow.substantial += shadow.substantial;
        self.strike_shadow.marginal += shadow.marginal;
        self.strike_shadow.none += shadow.none;
        self.strike_accumulated = self
            .strike_accumulated
            .saturating_add(separation.strike_accumulated);
        self.strike_overshoot = self
            .strike_overshoot
            .saturating_add(separation.strike_overshoot);
    }
}

/// The fingerprint of one master iteration: what the eight-worker merge
/// determinism vector compares.
#[derive(Clone, Debug, PartialEq)]
pub struct IterationFingerprint {
    pub bite: u64,
    pub attempt: u64,
    pub iteration: u64,
    /// The winning worker's ordinal.
    pub winner: usize,
    /// The winner's total weighted Φ, **before** the master's Algorithm-8 pass:
    /// the number the tournament actually ranked on.
    pub winner_guided: f64,
    /// At least two workers reached different totals, so the merge had
    /// something to choose. `false` is not a defect - eight workers that all
    /// clear a roomy strip tie at zero and the ordinal breaks it - but a run in
    /// which *no* iteration is ever contested has not exercised the tournament,
    /// and the evidence should be able to say which happened.
    pub contested: bool,
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
    /// `D`: the last exact-valid depth the loop is standing on, and the width
    /// its continuous state was restored to.
    pub depth_mm: f64,
    /// The target of the **last bite taken**, successful or not.
    ///
    /// It is smaller than `depth_mm` exactly when the last bite failed, and the
    /// gap between them is how far the trajectory was still reaching when the
    /// wall stopped it. It is never a quality number: nothing was published
    /// there.
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
    /// Which strike arm ran. Emitted so that a paired document can never be
    /// mislabelled by the driver that wrote it.
    pub strike_arm: StrikeConfig,
    /// The calibrated plan's ledger, or `None` in the wall and fixed-work
    /// arms. `None` is not "nothing was spent": it is "no plan was spending".
    pub calibrated: Option<CalibratedSummary>,
}

/// **What a calibrated plan spent, and what it did not charge.**
///
/// This is the spec's worst-ranked defect written as two identities a reader
/// can check without the engine:
///
/// > batch two's aggregate == the sum of the eight batch-two deltas, **not**
/// > cumulative slot totals.
///
/// * `charged + uncharged_tail == trajectory`, term by term. `charged` is
///   built one delta at a time as the trajectory runs; `trajectory` is one
///   subtraction of this run's two endpoints. They agree only while the cursor
///   is advanced by `charge_batch` and by nothing else.
/// * `currency(charged) == consumed_units`. The pacer's own scalars against
///   the currency applied to the vector it was handed - two accumulations that
///   never touch, so a batch charged to the wrong phase or a saturating add
///   shows up as a disagreement instead of as a plausible number.
///
/// If either stopped holding, work would be being charged twice, or to nobody,
/// and every rate derived from it would be stable and false.
#[derive(Clone, Debug, PartialEq)]
pub struct CalibratedSummary {
    pub explore_allocation: u64,
    pub compress_allocation: u64,
    pub explore_consumed: u64,
    pub compress_consumed: u64,
    pub explore_batches: u64,
    pub compress_batches: u64,
    /// **The spec's overshoot clause, as a number rather than a mean.**
    ///
    /// The units of the batch that first spent the phase's allocation, or `0`
    /// if the phase ended some other way. `consumed - allocation` can never
    /// exceed it, because the verdict is only read at a barrier and the phase
    /// stops at the next one.
    pub explore_crossing_batch_units: u64,
    pub compress_crossing_batch_units: u64,
    /// Every delta the pacer charged, summed. Its `master_batches` is the
    /// number of times `charge_batch` was called.
    pub charged: WorkTerms,
    /// What the trajectory counted after the last barrier and therefore never
    /// charged: the closing publication commit, the final restore, and - when
    /// a phase ended between separations - a disruption.
    pub uncharged_tail: WorkTerms,
    /// **This trajectory's own five counters**, as one subtraction of its two
    /// endpoints - the opening reading and the closing one. It does not pass
    /// through the per-batch accumulation, which is the whole point: it is the
    /// other side of the identity, not a restatement of it.
    pub trajectory: WorkTerms,
    /// `charged + uncharged_tail == trajectory`, term by term.
    pub charge_identity_holds: bool,
    /// `explore_consumed + compress_consumed`: what the pacer believes it
    /// spent, in units.
    pub consumed_units: u64,
    /// `currency(charged) == consumed_units`: the second identity.
    pub consumed_units_match_charged: bool,
    pub plan_key: icscal::PlanKey,
    pub currency_version: icscal::CurrencyVersion,
    pub budget_seconds: f64,
    pub explore_ratio: f64,
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

/// What the barrier decided.
#[derive(Clone, Copy, Debug)]
struct Merge {
    winner: usize,
    guided: f64,
    contested: bool,
}

/// One competitive worker's private world for one master iteration.
///
/// It was an anonymous `(IcsState, Descent, WorkVector)` triple until the
/// profile census needed somewhere to put a per-worker duration that is
/// **written by the worker thread itself**, before the join, so that the
/// critical-path sweep can be separated from the dispatch that surrounds it.
/// Naming the triple is the whole change; the contents, the ordering and the
/// merge are untouched.
struct Slot {
    state: IcsState,
    descent: Descent,
    work: WorkVector,
    #[cfg(feature = "conflict-cluster-budget")]
    partition: PartitionTrace,
    /// Nanoseconds this worker spent inside `worker_sweep`. Written only under
    /// `ics-profile`; one `u64` in an already-allocated vector otherwise, and
    /// read by nothing the engine decides on.
    #[cfg_attr(not(feature = "ics-profile"), allow(dead_code))]
    sweep_ns: u64,
}

impl Slot {
    /// Step 3: one complete sequential relocate sweep, in this worker's own
    /// state, descent and work vector.
    #[cfg(not(feature = "conflict-cluster-budget"))]
    fn sweep(&mut self, sources: &[PieceSource], contract: &Contract) -> SweepOutcome {
        #[cfg(feature = "ics-profile")]
        let started = std::time::Instant::now();
        let outcome = self
            .descent
            .worker_sweep(&mut self.state, sources, contract, &mut self.work);
        #[cfg(feature = "ics-profile")]
        {
            self.sweep_ns = started.elapsed().as_nanos() as u64;
        }
        outcome
    }

    #[cfg(feature = "conflict-cluster-budget")]
    fn sweep(
        &mut self,
        sources: &[PieceSource],
        contract: &Contract,
        field: Option<&ClusterField>,
    ) -> SweepOutcome {
        #[cfg(feature = "ics-profile")]
        let started = std::time::Instant::now();
        let outcome = if self.descent.partition_arm() == PartitionArm::Off {
            self.descent
                .worker_sweep(&mut self.state, sources, contract, &mut self.work)
        } else {
            self.descent.worker_sweep_partitioned(
                &mut self.state,
                sources,
                contract,
                field.expect("an armed worker has a cluster field"),
                &mut self.partition,
                &mut self.work,
            )
        };
        #[cfg(feature = "ics-profile")]
        {
            self.sweep_ns = started.elapsed().as_nanos() as u64;
        }
        outcome
    }
}

struct SeparateOutcome {
    published: Option<Publication>,
    stop: SeparateStop,
    iterations: u64,
    strikes: u32,
    min_raw: f64,
    band_reached: bool,
    /// Turns that entered the 4 µm band. Round 1's `exact_attempts`, renamed
    /// for what it counts (audit F4).
    exact_band_entries: u64,
    /// Band entries that reached exact geometry: the delta of
    /// `work.exact_checkpoints` across `attempt_publication`.
    exact_checkpoint_calls: u64,
    profile: PhaseProfile,
    /// Both arms' patience counters. Shadow only.
    strike_shadow: ShadowCounters,
    /// Patience accumulated at each strike of this separation, summed.
    strike_accumulated: u64,
    /// The crossing batch's cost at each strike, summed: the overshoot.
    strike_overshoot: u64,
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

    /// Puts **this entry's own** weights back, after the rows have been rebuilt
    /// from its poses.
    ///
    /// This is a difference from the source and it is deliberate. Their
    /// `tracker.rs::restore_but_keep_weights` keeps whatever weights the tracker
    /// currently holds; ours restores the landscape the pooled layout was
    /// standing in when it was pooled, which is the reading Sol review 17 Round
    /// 2 §5 gives the explore-fail path ("reset weights for the restored pool
    /// state"). The two rules coincide for the *rollback inside* a separation -
    /// which [`restore_keeping_weights`] implements theirs for, because Grok
    /// review 12 Round 2 §6.4 is explicit that weights persist across a rollback
    /// inside a width - and diverge only here, where a different layout is being
    /// restored and pairing it with a landscape learned on a different one would
    /// rank rows by pressure they never carried.
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
    /// **The calibrated plan.** Units in at every barrier, a verdict out, and
    /// no clock anywhere - `elapsed_s` and `deadline_s` are `None` here for
    /// the same reason they are `None` in [`Pacer::FixedWork`].
    Calibrated {
        plan: Box<WorkPlanPacer<NoClock>>,
        attempts_per_bite: u64,
        /// The trajectory's cumulative five counters at the previous charge.
        /// **The one subtraction in the wiring**, so no caller can hand the
        /// pacer a running total by mistake.
        cursor: WorkTerms,
        /// Where the cursor started. Kept beside it so that the closing ledger
        /// can reach this trajectory's work by a route that does not pass
        /// through the per-batch accumulation it is checking.
        opened_at: WorkTerms,
        charged: WorkTerms,
        /// The units of the batch that first spent a phase's allocation, per
        /// phase, `0` while the phase still has room. It is the numerator of
        /// the spec's *"overshoot <= one batch"* clause, and emitting it is
        /// what makes that clause checkable from the document instead of
        /// being asserted about a mean.
        explore_crossing_batch_units: u64,
        compress_crossing_batch_units: u64,
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
            Budget::CalibratedWork {
                plan,
                attempts_per_bite,
            } => Pacer::Calibrated {
                plan,
                attempts_per_bite,
                opened_at: WorkTerms::default(),
                cursor: WorkTerms::default(),
                charged: WorkTerms::default(),
                explore_crossing_batch_units: 0,
                compress_crossing_batch_units: 0,
            },
        }
    }

    /// Open the charging cursor where the trajectory already is.
    ///
    /// One engine may run more than one `run_cutclose` - the spawn-tax cell
    /// runs a prefix and then a probe - and `Trace` is cumulative across both.
    /// Without this the second trajectory's first batch would be charged for
    /// the whole of the first one, which is the double-debit defect arriving
    /// by the front door.
    fn open_at(&mut self, now: WorkTerms) {
        if let Pacer::Calibrated {
            opened_at, cursor, ..
        } = self
        {
            *opened_at = now;
            *cursor = now;
        }
    }

    /// The plan's ledger, and the tail it never charged. `None` unless a plan
    /// was spending.
    fn close(&self, now: WorkTerms) -> Option<CalibratedSummary> {
        let Pacer::Calibrated {
            plan,
            opened_at,
            cursor,
            charged,
            explore_crossing_batch_units,
            compress_crossing_batch_units,
            ..
        } = self
        else {
            return None;
        };
        let uncharged_tail = now.since(cursor);
        // **Two independent routes to the same vector.** `charged` was built
        // batch by batch, adding one delta at a time; `trajectory` is a single
        // subtraction of this trajectory's two endpoints, and `now` is
        // cumulative over the whole engine so the opening reading has to come
        // off it. They agree only while the cursor is advanced by
        // `charge_batch` and by nothing else - the moment some other line
        // moves it, or a batch is charged from a stale reading, the sum stops
        // telescoping and this goes red.
        let trajectory = now.since(opened_at);
        let mut sum = *charged;
        sum.add(&uncharged_tail);
        // And the third route: what the pacer *believes* it consumed, in
        // units, against the currency applied to the terms it was handed. The
        // two accumulate separately - scalars inside `WorkPlanPacer`, vectors
        // here - so a batch charged to the wrong phase, or a saturation, shows
        // up as a disagreement rather than as a plausible number.
        let consumed_units = plan
            .consumed(PlanPhase::Explore)
            .saturating_add(plan.consumed(PlanPhase::Compress));
        Some(CalibratedSummary {
            explore_allocation: plan.allocation(PlanPhase::Explore),
            compress_allocation: plan.allocation(PlanPhase::Compress),
            explore_consumed: plan.consumed(PlanPhase::Explore),
            compress_consumed: plan.consumed(PlanPhase::Compress),
            explore_batches: plan.batches(PlanPhase::Explore),
            compress_batches: plan.batches(PlanPhase::Compress),
            explore_crossing_batch_units: *explore_crossing_batch_units,
            compress_crossing_batch_units: *compress_crossing_batch_units,
            charged: *charged,
            uncharged_tail,
            trajectory,
            charge_identity_holds: sum == trajectory,
            consumed_units,
            consumed_units_match_charged: plan.currency().units(charged) == consumed_units,
            plan_key: plan.key().clone(),
            currency_version: plan.currency().version,
            budget_seconds: plan.budget_seconds(),
            explore_ratio: plan.explore_ratio(),
        })
    }

    /// **Charge one completed master batch**, at the barrier, on the delta.
    ///
    /// Returns whether the phase has no room for another batch. `false` in
    /// every arm that is not a plan, which is why the caller needs no branch.
    fn charge_batch(&mut self, phase: Phase, now: WorkTerms) -> bool {
        let Pacer::Calibrated {
            plan,
            cursor,
            charged,
            explore_crossing_batch_units,
            compress_crossing_batch_units,
            ..
        } = self
        else {
            return false;
        };
        let delta = now.since(cursor);
        *cursor = now;
        charged.add(&delta);
        let phase = plan_phase(phase);
        let boundary = plan.charge_batch(phase, &delta);
        // The first batch to spend the allocation is the crossing batch, and
        // only the first: a later one would report the overshoot of a phase
        // that had already ended.
        let crossing = match phase {
            PlanPhase::Explore => explore_crossing_batch_units,
            PlanPhase::Compress => compress_crossing_batch_units,
        };
        if boundary.phase_exhausted && *crossing == 0 {
            *crossing = boundary.units_charged;
        }
        boundary.phase_exhausted || batch_ceiling_reached(plan, phase)
    }

    /// Whether the phase was already spent when a separation opened, so that a
    /// separation cannot run one batch past the allocation just by starting.
    fn phase_exhausted_at_entry(&self, phase: Phase) -> bool {
        match self {
            Pacer::Calibrated { plan, .. } => {
                plan.entry_boundary(plan_phase(phase)).phase_exhausted
            }
            _ => false,
        }
    }

    /// The clock, read **only** at a worker-sweep barrier or a phase boundary.
    fn elapsed_s(&self) -> Option<f64> {
        match self {
            Pacer::FixedWork { .. } | Pacer::Calibrated { .. } => None,
            Pacer::Wall { start, .. } => Some(start.elapsed().as_secs_f64()),
        }
    }

    fn deadline_s(&self, phase: Phase) -> Option<f64> {
        match self {
            Pacer::FixedWork { .. } | Pacer::Calibrated { .. } => None,
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
            // Between bites, the plan's own question: has this phase any units
            // left? It is the same verdict `charge_batch` returns at a
            // barrier, asked at the other place a phase may end.
            Pacer::Calibrated { plan, .. } => {
                let phase = plan_phase(phase);
                plan.remaining(phase) == 0 || batch_ceiling_reached(plan, phase)
            }
        }
    }

    fn attempts_exhausted(&self, attempts: u64) -> bool {
        match self {
            Pacer::FixedWork {
                attempts_per_bite, ..
            } => attempts >= *attempts_per_bite,
            Pacer::Wall { .. } => false,
            // `0` is unlimited, matching `Pacer::Wall`. See the guard's note
            // on `Budget::CalibratedWork`.
            Pacer::Calibrated {
                attempts_per_bite, ..
            } => *attempts_per_bite != 0 && attempts >= *attempts_per_bite,
        }
    }

    fn iteration_cap(&self) -> Option<u64> {
        match self {
            Pacer::FixedWork {
                iterations_per_separation,
                ..
            } => Some(*iterations_per_separation),
            // The unit allocation is the cap. A second one denominated in
            // iterations would be the probe-on-cheap-bites defect wearing the
            // calibrated plan's clothes.
            Pacer::Wall { .. } | Pacer::Calibrated { .. } => None,
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
            // **Compress decay by consumed compress-work**, the spec's own
            // words, through the same frozen `time_based_step` the other two
            // arms call. The pacer computes it; nothing here re-derives it.
            Pacer::Calibrated { plan, .. } => plan.compress_step(),
        }
    }
}

/// **The termination guard, and why a plan needs one at all.**
///
/// A calibrated phase ends when its units are spent, and a master batch always
/// spends some - a state with `Φ > 0` has a non-empty colliding set, and
/// relocating it costs sample evaluations. "Always" there is an argument about
/// the operator, though, not a property of the pacer, and a plan that could not
/// terminate would hang a gate rather than fail it.
///
/// So: a phase is also over once it has charged as many batches as it has
/// units. One unit is the cheapest possible non-zero batch, so the ceiling is
/// unreachable by any trajectory that charges anything at all - the shelf's own
/// batches are five to six orders of magnitude above it - and it bounds the
/// loop even if some future batch charged nothing forever.
fn batch_ceiling_reached(plan: &WorkPlanPacer<NoClock>, phase: PlanPhase) -> bool {
    plan.batches(phase) >= plan.allocation(phase)
}

/// The engine's `Phase` in the plan file's vocabulary. One function, so the
/// two enums cannot drift into disagreeing about which phase is which.
fn plan_phase(phase: Phase) -> PlanPhase {
    match phase {
        Phase::Explore => PlanPhase::Explore,
        Phase::Compress => PlanPhase::Compress,
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
