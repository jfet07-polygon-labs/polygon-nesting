//! The overlap-tolerant continuous engine: locked-strip-then-shrink ICS.
//!
//! Specified by docs/overlap-ics-converged-spec.md, whose body is Sol review 14
//! Round 2 §4 plus §3 and Grok review 9 Round 2 §4. This module is the vertical
//! slice of that spec: state, decomposition, contact, broad phase, energy,
//! descent, publication and diagnostics. `homotopy.rs` is a stub by design -
//! Gate 0's cells all run at a locked strip, because a schedule is the one
//! thing that could turn a field that cannot legalize into a slow success.
//!
//! Four things this engine is *not*, each of which was a previous round's
//! failure:
//!
//! * it is not an `ExplorationKernel`. That seam consumes rotations baked into
//!   legacy surrogates and catalogues, which is the representation this
//!   experiment exists to escape.
//! * it is not contact-block. No exact predicate can shorten an intermediate
//!   move; exact geometry appears only at a publication attempt.
//! * it is not `global_legalize`. Repair is capped at 4 µm per row and 16 µm
//!   per piece, and a checkpoint that needs more is **discarded**, not inflated.
//! * it is not a proxy-quality engine. Φ, `max_g`, the guided energy and the
//!   target `T` are diagnostics; the only quality series is exact-valid raw
//!   source depth.
//!
//! ```text
//! constructor (exact floor)
//!   -> affine-compressed COPY of its poses at a locked T   (the ICS state)
//!   -> damped PGS, continuous theta, guided weights, one jump
//!   -> publication attempts inside the same T
//!   -> best_exact
//! ```

pub mod broad_phase;
pub mod contact;
/// The deterministic contact corpus and its independent score: Gate 0's
/// numeric-soundness cell. Diagnostic only; no acceptance path calls it.
pub mod corpus;
pub mod decomposition;
pub mod descent;
pub mod diagnostics;
pub mod energy;
pub mod homotopy;
pub mod publish;
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

use descent::{Descent, DescentConfig};
use diagnostics::{ProxySample, QualityPoint, Trace, WorkVector};
use publish::PublicationLimits;
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
    /// Builds a trajectory from a constructor layout.
    ///
    /// `constructor` is the exact anytime floor **and** the source of the ICS
    /// state - but the state is an affinely compressed *copy* of its poses, so
    /// the constructor fingerprint is never a child. Both designers converged
    /// on that distinction; Grok review 9 Round 2 §1.2 names it "the island".
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
    pub fn checkpoint(&mut self) -> bool {
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
            return false;
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
            return false;
        };
        self.last_attempt_pose_digest = Some(digest);
        self.trace.checkpoints.push(attempt.checkpoint);
        let Some(publication) = attempt.publication else {
            return false;
        };
        // The one write of `best_exact` in the whole engine, and it is
        // conditional on a strict improvement: a dual-valid layout that is not
        // better than the floor is recorded as a checkpoint and discarded as an
        // incumbent.
        let depth = publication.raw_source_depth_mm;
        if depth < self.incumbent.raw_source_depth_mm - self.config.limits.minimum_improvement_mm {
            self.incumbent = ExactIncumbent {
                placements: publication.placements,
                raw_source_depth_mm: depth,
                from_constructor: false,
                placement_fingerprint: publication.placement_fingerprint,
            };
            self.trace.quality.push(QualityPoint {
                proposal_ordinal: self.descent.proposals,
                raw_source_depth_mm: depth,
                strict_child: true,
            });
            return true;
        }
        false
    }

    /// Runs the trajectory to its work quota.
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
            if outcome.totals.raw <= 0.0 {
                // A converged state is not a stall. Without this clause a
                // trajectory that has reached Φ = 0 keeps "stalling" - a sweep
                // that changes nothing has `raw_after == raw_before` - and the
                // ladder eventually relocates a piece out of a *feasible*
                // layout. The basin sweep in docs/experiments/overlap-ics/
                // measured that: a 0.005 mm perturbation converged to Φ = 0 and
                // was then driven back to Φ = 142 by its own escape mechanism.
                // Guided weights and topology jumps exist to escape a
                // *violated* local minimum; at zero violation there is nothing
                // to escape and no utility to rank.
                self.descent.on_improving_sweep();
            } else if outcome.raw_after < outcome.raw_before {
                self.descent.on_improving_sweep();
            } else {
                self.trace.guided_stalls += 1;
                let jumped = {
                    let Engine {
                        ref mut state,
                        ref sources,
                        ref contract,
                        ref mut trace,
                        ref mut descent,
                        ..
                    } = *self;
                    descent.on_stalled_sweep(state, sources, contract, &mut trace.work)
                };
                if jumped.attempted {
                    self.trace.jump_attempted += 1;
                    if jumped.installed {
                        self.trace.jump_committed += 1;
                    }
                    if jumped.improved_guided {
                        self.trace.jumps_improving_guided += 1;
                    }
                    self.trace.jump_events.push(diagnostics::JumpEvent {
                        proposal_ordinal: self.descent.proposals,
                        piece: jumped.piece,
                        kind: jumped.kind.label(),
                        radius_mm: jumped.radius_mm,
                        max_violation_mm: jumped.max_violation_mm,
                        baseline_guided: jumped.baseline_guided,
                        best_guided: jumped.best_guided,
                        installed: jumped.installed,
                        improved_guided: jumped.improved_guided,
                    });
                }
                self.trace.jumps = self.descent.jumps_spent() as u64;
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
